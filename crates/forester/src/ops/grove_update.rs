//! Grove update operation.
//!
//! `sync-seed` brings upstream commits into the forest's bare repositories but
//! deliberately leaves groves alone. This operation is the other half: it advances
//! a grove's member checkouts onto those commits.
//!
//! Splitting the two means the network step stays safe to run at any time, while
//! the step that touches working trees — which may hold uncommitted work — is
//! explicit, per grove, and never a surprise.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use snafu::{ResultExt, Snafu};

use crate::domain::{Checkout, ForestConfig, ForestRoot, GroveName, Member};
use crate::events::{EventEmitter, ForesterEvent};
use crate::git::{AheadBehind, BareRepository, MemberRepository};
use crate::hooks::{HookContext, HookRegistry, Trigger};
use crate::ops::update_report::{MemberOutcome, MemberUpdate, SkipReason, UpdateReport};

/// How far an update may go to bring a checkout current.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Strategy {
    /// Only advance when no local commits would be rewritten.
    #[default]
    FfOnly,
    /// Replay local commits on top of the new upstream base.
    Rebase,
}

/// Options controlling a grove update.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct UpdateOptions {
    /// How far the update may go.
    pub strategy: Strategy,
    /// Restrict the update to these member names; empty means all members.
    pub members: Vec<String>,
    /// Report intended actions without touching any checkout.
    pub dry_run: bool,
}

/// A member checkout together with the upstream ref it tracks.
struct MemberTarget<'m> {
    repo: MemberRepository,
    member: &'m Member,
    upstream: String,
}

impl MemberTarget<'_> {
    fn name(&self) -> &str {
        &self.member.name
    }
}

/// Advances the member checkouts of a single grove.
pub struct GroveUpdateOperation<'a> {
    forest_root: &'a ForestRoot,
    config: &'a ForestConfig,
    hooks: &'a HookRegistry,
    emitter: Arc<dyn EventEmitter>,
    verbose: bool,
}

impl<'a> GroveUpdateOperation<'a> {
    /// Creates a new grove update operation.
    pub fn new(
        forest_root: &'a ForestRoot,
        config: &'a ForestConfig,
        hooks: &'a HookRegistry,
        emitter: Arc<dyn EventEmitter>,
        verbose: bool,
    ) -> Self {
        Self {
            forest_root,
            config,
            hooks,
            emitter,
            verbose,
        }
    }

    /// Executes the update, returning a per-member report.
    pub fn execute(
        &self,
        name: &GroveName,
        opts: &UpdateOptions,
    ) -> Result<UpdateReport, GroveUpdateError> {
        use grove_update_error::*;

        let grove_path = self.forest_root.groves_dir().join(name.to_string());
        snafu::ensure!(
            grove_path.is_dir(),
            NotFoundSnafu {
                name: name.to_string()
            }
        );

        self.emitter.emit(&ForesterEvent::GroveUpdateStarted {
            grove: name.to_string(),
        });
        self.run_hooks(Trigger::PreGroveUpdate, name, &grove_path, opts)?;

        let report = UpdateReport {
            grove: name.to_string(),
            members: self.update_members(name, &grove_path, opts),
        };

        self.emitter.emit(&ForesterEvent::GroveUpdateCompleted {
            grove: name.to_string(),
            updated: report.updated_count(),
            skipped: report.skipped().count(),
        });

        // Contents moved, so any derived index is now stale. Re-indexing runs as a
        // hook so each forest decides whether to pay for it.
        if report.changed_files() {
            self.run_hooks(Trigger::PostGroveUpdate, name, &grove_path, opts)?;
        }

        Ok(report)
    }

    fn update_members(
        &self,
        grove: &GroveName,
        grove_path: &Path,
        opts: &UpdateOptions,
    ) -> Vec<MemberUpdate> {
        let selected = self
            .config
            .forest
            .member
            .iter()
            .filter(|member| opts.members.is_empty() || opts.members.contains(&member.name));

        selected
            .map(|member| {
                self.emitter.emit(&ForesterEvent::MemberUpdating {
                    name: member.name.clone(),
                });

                // One member failing should not leave the rest of the grove stale,
                // so the failure is recorded and the update carries on.
                let outcome = self
                    .update_member(grove, grove_path, member, opts)
                    .unwrap_or_else(|err| MemberOutcome::Failed {
                        message: err.to_string(),
                    });

                self.announce(&member.name, &outcome);
                MemberUpdate {
                    member: member.name.clone(),
                    outcome,
                }
            })
            .collect()
    }

    fn announce(&self, member: &str, outcome: &MemberOutcome) {
        let event = match outcome {
            MemberOutcome::UpToDate => ForesterEvent::MemberCurrent {
                name: member.to_string(),
            },
            MemberOutcome::Skipped { reason } => ForesterEvent::MemberSkipped {
                name: member.to_string(),
                reason: reason.describe(),
            },
            MemberOutcome::Failed { message } => {
                ForesterEvent::Error(format!("{member}: {message}"))
            }
            other => ForesterEvent::MemberUpdated {
                name: member.to_string(),
                detail: other.describe(),
            },
        };
        self.emitter.emit(&event);
    }

    fn update_member(
        &self,
        grove: &GroveName,
        grove_path: &Path,
        member: &'a Member,
        opts: &UpdateOptions,
    ) -> Result<MemberOutcome, GroveUpdateError> {
        let member_path = grove_path.join(&member.path);
        let target = MemberTarget {
            repo: MemberRepository::new(&member_path),
            member,
            upstream: format!("origin/{}", member.branch()),
        };

        // A member added to forester.toml after the grove existed has no checkout
        // yet. Creating it is only half the job: it still has to be advanced like
        // any other, so this falls through to the normal update path.
        let added = match target.repo.exists() {
            true => None,
            false => {
                let branch = self.checkout_member(grove, member, &member_path, opts)?;
                if opts.dry_run {
                    return Ok(MemberOutcome::Added { branch, behind: 0 });
                }
                Some(branch)
            }
        };

        let outcome = self.advance(&target, opts)?;
        match added {
            None => Ok(outcome),
            Some(branch) => Ok(MemberOutcome::Added {
                branch,
                behind: match outcome {
                    MemberOutcome::FastForwarded { behind } => behind,
                    _ => 0,
                },
            }),
        }
    }

    fn advance(
        &self,
        target: &MemberTarget<'_>,
        opts: &UpdateOptions,
    ) -> Result<MemberOutcome, GroveUpdateError> {
        use grove_update_error::*;

        // The checkout's origin is the forest's bare repo, so this is a local
        // operation that picks up whatever `sync-seed` already fetched.
        if !opts.dry_run {
            target
                .repo
                .fetch("origin", !self.verbose)
                .context(FetchSnafu {
                    member: target.name(),
                })?;
        }

        if let Some(reason) = self.blocker(target)? {
            return Ok(MemberOutcome::Skipped { reason });
        }

        let drift = target
            .repo
            .ahead_behind("HEAD", &target.upstream)
            .context(InspectSnafu {
                member: target.name(),
            })?;

        if drift.is_synced() {
            return Ok(MemberOutcome::UpToDate);
        }
        if drift.is_only_ahead() {
            return Ok(MemberOutcome::Ahead { ahead: drift.ahead });
        }

        // Checked after the no-op cases so a checkout with local edits that needs
        // nothing is reported as current rather than as a problem to resolve.
        let dirty = target.repo.dirty_count().context(InspectSnafu {
            member: target.name(),
        })?;
        if dirty > 0 {
            return Ok(MemberOutcome::Skipped {
                reason: SkipReason::Dirty { files: dirty },
            });
        }

        self.apply(target, drift, opts)
    }

    /// Returns a reason the checkout cannot be reasoned about, if any.
    fn blocker(&self, target: &MemberTarget<'_>) -> Result<Option<SkipReason>, GroveUpdateError> {
        use grove_update_error::*;

        let resolved = target
            .repo
            .rev_parse(&target.upstream)
            .context(InspectSnafu {
                member: target.name(),
            })?;
        if resolved.is_none() {
            return Ok(Some(SkipReason::NoUpstream {
                branch: target.upstream.clone(),
            }));
        }

        let branch = target.repo.current_branch().context(InspectSnafu {
            member: target.name(),
        })?;
        Ok(branch.is_none().then_some(SkipReason::Detached))
    }

    fn apply(
        &self,
        target: &MemberTarget<'_>,
        drift: AheadBehind,
        opts: &UpdateOptions,
    ) -> Result<MemberOutcome, GroveUpdateError> {
        use grove_update_error::*;

        if drift.can_fast_forward() {
            if !opts.dry_run {
                target
                    .repo
                    .merge_ff_only(&target.upstream, !self.verbose)
                    .context(AdvanceSnafu {
                        member: target.name(),
                    })?;
            }
            return Ok(MemberOutcome::FastForwarded {
                behind: drift.behind,
            });
        }

        if opts.strategy == Strategy::FfOnly {
            return Ok(MemberOutcome::Skipped {
                reason: SkipReason::Diverged {
                    ahead: drift.ahead,
                    behind: drift.behind,
                },
            });
        }

        let replayed = MemberOutcome::Rebased {
            ahead: drift.ahead,
            behind: drift.behind,
        };
        if opts.dry_run {
            return Ok(replayed);
        }

        let rebased = target
            .repo
            .rebase_onto(&target.upstream)
            .context(AdvanceSnafu {
                member: target.name(),
            })?;

        match rebased {
            true => Ok(replayed),
            false => Ok(MemberOutcome::Skipped {
                reason: SkipReason::RebaseConflict {
                    onto: target.upstream.clone(),
                },
            }),
        }
    }

    fn checkout_member(
        &self,
        grove: &GroveName,
        member: &Member,
        member_path: &Path,
        opts: &UpdateOptions,
    ) -> Result<String, GroveUpdateError> {
        use grove_update_error::*;

        let checkout = Checkout::plan(grove, member, None);
        if opts.dry_run {
            return Ok(checkout.branch().to_string());
        }

        if let Some(parent) = member_path.parent() {
            std::fs::create_dir_all(parent).context(CreateDirSnafu {
                path: parent.to_path_buf(),
            })?;
        }

        let bare_path = self
            .forest_root
            .bare_dir()
            .join(format!("{}.git", member.name));
        BareRepository::new(&bare_path, &member.name)
            .clone_to(member_path, !self.verbose)
            .context(CloneSnafu {
                member: &member.name,
            })?;

        match &checkout {
            Checkout::New { branch, start } => {
                BareRepository::checkout_new_branch(member_path, branch, start, !self.verbose)
            }
            Checkout::Existing { branch } => {
                BareRepository::checkout_branch(member_path, branch, !self.verbose)
            }
        }
        .context(CheckoutSnafu {
            member: &member.name,
        })?;

        Ok(checkout.branch().to_string())
    }

    fn run_hooks(
        &self,
        trigger: Trigger,
        name: &GroveName,
        path: &Path,
        opts: &UpdateOptions,
    ) -> Result<(), GroveUpdateError> {
        use grove_update_error::*;

        if opts.dry_run {
            return Ok(());
        }

        let ctx = HookContext::builder()
            .forest_root(self.forest_root.clone())
            .trigger(trigger)
            .emitter(Arc::clone(&self.emitter))
            .grove_name(name.to_string())
            .grove_path(path.to_path_buf())
            .verbose(self.verbose)
            .build();

        self.hooks.run_hooks(trigger, &ctx).context(HookSnafu)
    }
}

/// Errors from updating a grove.
#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum GroveUpdateError {
    /// The named grove does not exist.
    #[snafu(display("Grove '{name}' not found"))]
    NotFound {
        /// Grove name.
        name: String,
    },

    /// Failed to fetch into a member checkout.
    #[snafu(display("Failed to fetch updates for '{member}'"))]
    Fetch {
        /// Member name.
        member: String,
        /// Underlying git error.
        source: crate::git::GitCommandError,
    },

    /// Failed to inspect a member checkout.
    #[snafu(display("Failed to inspect '{member}'"))]
    Inspect {
        /// Member name.
        member: String,
        /// Underlying git error.
        source: crate::git::GitCommandError,
    },

    /// Failed to advance a member checkout.
    #[snafu(display("Failed to advance '{member}'"))]
    Advance {
        /// Member name.
        member: String,
        /// Underlying git error.
        source: crate::git::GitCommandError,
    },

    /// Failed to clone a member missing from the grove.
    #[snafu(display("Failed to clone repository for '{member}'"))]
    Clone {
        /// Member name.
        member: String,
        /// Underlying git error.
        source: crate::git::GitCommandError,
    },

    /// Failed to check out a member missing from the grove.
    #[snafu(display("Failed to checkout branch for '{member}'"))]
    Checkout {
        /// Member name.
        member: String,
        /// Underlying git error.
        source: crate::git::GitCommandError,
    },

    /// Failed to create a directory.
    #[snafu(display("Failed to create directory"))]
    CreateDir {
        /// Path that could not be created.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },

    /// A hook failed during execution.
    #[snafu(display("Hook failed"))]
    Hook {
        /// Underlying hook error.
        source: crate::hooks::RunHooksError,
    },
}
