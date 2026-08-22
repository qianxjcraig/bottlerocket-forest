//! Outcome reporting for grove updates.
//!
//! An update declines to act more often than it acts, so the reasons carry both a
//! description and the command that resolves them. A skip the user cannot act on
//! is indistinguishable from a bug.

/// Why a member checkout was left untouched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// The checkout has uncommitted changes.
    Dirty {
        /// Number of changed entries.
        files: usize,
    },
    /// HEAD is not on a branch.
    Detached,
    /// Local and upstream both hold commits the other lacks.
    Diverged {
        /// Commits only the checkout has.
        ahead: usize,
        /// Commits only upstream has.
        behind: usize,
    },
    /// The tracked upstream branch does not exist in the checkout.
    NoUpstream {
        /// The missing ref.
        branch: String,
    },
    /// A rebase hit conflicts and was rolled back.
    RebaseConflict {
        /// The revision the rebase targeted.
        onto: String,
    },
}

impl SkipReason {
    /// Describes why the member was skipped.
    pub fn describe(&self) -> String {
        match self {
            Self::Dirty { files } => format!("{files} uncommitted change(s)"),
            Self::Detached => "detached HEAD".to_string(),
            Self::Diverged { ahead, behind } => {
                format!("diverged (ahead {ahead}, behind {behind})")
            }
            Self::NoUpstream { branch } => format!("no {branch}"),
            Self::RebaseConflict { onto } => {
                format!("rebase onto {onto} conflicted and was rolled back")
            }
        }
    }

    /// The next step a user can take to resolve the skip themselves.
    pub fn recovery_hint(&self) -> String {
        match self {
            Self::Dirty { .. } => "commit or stash the changes, then re-run".to_string(),
            Self::Detached => "git switch <branch>".to_string(),
            Self::Diverged { .. } => {
                "re-run with '--strategy rebase', or merge manually".to_string()
            }
            Self::NoUpstream { .. } => "run 'forester sync-seed' first".to_string(),
            Self::RebaseConflict { onto } => {
                format!("git rebase {onto}, then resolve the conflicts")
            }
        }
    }
}

/// What happened to one member checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberOutcome {
    /// A missing checkout was created, and advanced by `behind` commits.
    Added {
        /// Branch the checkout landed on.
        branch: String,
        /// Commits it was advanced by after creation.
        behind: usize,
    },
    /// The branch fast-forwarded onto upstream.
    FastForwarded {
        /// Commits advanced.
        behind: usize,
    },
    /// Local commits were replayed onto the new base.
    Rebased {
        /// Local commits replayed.
        ahead: usize,
        /// Commits of new base picked up.
        behind: usize,
    },
    /// Already current with upstream.
    UpToDate,
    /// Carries local commits and is already on the newest base.
    Ahead {
        /// Local commits not yet upstream.
        ahead: usize,
    },
    /// Left alone; the reason says why.
    Skipped {
        /// Why the member was skipped.
        reason: SkipReason,
    },
    /// The update itself failed.
    Failed {
        /// What went wrong.
        message: String,
    },
}

impl MemberOutcome {
    /// Whether the checkout contents changed.
    pub fn changed_files(&self) -> bool {
        matches!(
            self,
            Self::Added { .. } | Self::FastForwarded { .. } | Self::Rebased { .. }
        )
    }

    /// Whether this outcome should make the command exit nonzero.
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Describes the outcome for a report line.
    pub fn describe(&self) -> String {
        match self {
            Self::Added { branch, behind } if *behind > 0 => {
                format!("added on {branch}, +{behind}")
            }
            Self::Added { branch, .. } => format!("added on {branch}"),
            Self::FastForwarded { behind } => format!("fast-forwarded +{behind}"),
            Self::Rebased { ahead, behind } => {
                format!("rebased {ahead} commit(s) onto +{behind}")
            }
            Self::UpToDate => "up to date".to_string(),
            Self::Ahead { ahead } => format!("ahead by {ahead}, base current"),
            Self::Skipped { reason } => format!("skipped: {}", reason.describe()),
            Self::Failed { message } => format!("failed: {message}"),
        }
    }
}

/// Result of updating one member of a grove.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberUpdate {
    /// Member name.
    pub member: String,
    /// What happened to it.
    pub outcome: MemberOutcome,
}

/// Result of updating a whole grove.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateReport {
    /// Grove that was updated.
    pub grove: String,
    /// Per-member outcomes.
    pub members: Vec<MemberUpdate>,
}

impl UpdateReport {
    /// Whether any checkout contents changed, meaning derived indexes are stale.
    pub fn changed_files(&self) -> bool {
        self.members
            .iter()
            .any(|entry| entry.outcome.changed_files())
    }

    /// Whether any member failed to update.
    pub fn has_failures(&self) -> bool {
        self.members.iter().any(|entry| entry.outcome.is_failure())
    }

    /// Number of members whose checkout advanced.
    pub fn updated_count(&self) -> usize {
        self.members
            .iter()
            .filter(|entry| entry.outcome.changed_files())
            .count()
    }

    /// Members that were deliberately left untouched.
    pub fn skipped(&self) -> impl Iterator<Item = &MemberUpdate> {
        self.members
            .iter()
            .filter(|entry| matches!(entry.outcome, MemberOutcome::Skipped { .. }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(MemberOutcome::Added { branch: "develop".to_string(), behind: 0 }, true ; "added changes files")]
    #[test_case(MemberOutcome::FastForwarded { behind: 3 }, true ; "fast forward changes files")]
    #[test_case(MemberOutcome::Rebased { ahead: 1, behind: 2 }, true ; "rebase changes files")]
    #[test_case(MemberOutcome::UpToDate, false ; "up to date changes nothing")]
    #[test_case(MemberOutcome::Ahead { ahead: 2 }, false ; "ahead changes nothing")]
    #[test_case(MemberOutcome::Skipped { reason: SkipReason::Detached }, false ; "skip changes nothing")]
    #[test_case(MemberOutcome::Failed { message: "boom".to_string() }, false ; "failure changes nothing")]
    fn changed_files_tracks_checkout_mutation(outcome: MemberOutcome, expected: bool) {
        // Given an update outcome
        // When asking whether the checkout contents moved
        // Then only outcomes that touched files report true
        assert_eq!(outcome.changed_files(), expected);
    }

    #[test]
    fn report_separates_failures_from_skips() {
        // Given a report with one failure and one skip
        let report = UpdateReport {
            grove: "develop".to_string(),
            members: vec![
                MemberUpdate {
                    member: "twoliter".to_string(),
                    outcome: MemberOutcome::Failed {
                        message: "boom".to_string(),
                    },
                },
                MemberUpdate {
                    member: "core-kit".to_string(),
                    outcome: MemberOutcome::Skipped {
                        reason: SkipReason::Dirty { files: 2 },
                    },
                },
                MemberUpdate {
                    member: "sdk".to_string(),
                    outcome: MemberOutcome::FastForwarded { behind: 4 },
                },
            ],
        };

        // When inspecting the report
        // Then failures, skips, and advances are each counted separately
        assert!(report.has_failures());
        assert_eq!(report.skipped().count(), 1);
        assert_eq!(report.updated_count(), 1);
        assert!(report.changed_files());
    }

    #[test]
    fn every_skip_reason_offers_a_recovery_hint() {
        // Given each way an update can decline to act
        let reasons = [
            SkipReason::Dirty { files: 2 },
            SkipReason::Detached,
            SkipReason::Diverged {
                ahead: 1,
                behind: 4,
            },
            SkipReason::NoUpstream {
                branch: "origin/develop".to_string(),
            },
            SkipReason::RebaseConflict {
                onto: "origin/develop".to_string(),
            },
        ];

        // When describing each one
        // Then both an explanation and an actionable next step are produced
        for reason in reasons {
            assert!(!reason.describe().is_empty());
            assert!(!reason.recovery_hint().is_empty());
        }
    }
}
