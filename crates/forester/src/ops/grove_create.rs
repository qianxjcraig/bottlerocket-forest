//! Grove creation operation.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use snafu::{ResultExt, Snafu};

use crate::domain::{Checkout, ForestConfig, ForestRoot, GroveName, GroveRoot};
use crate::events::{EventEmitter, ForesterEvent};
use crate::git::BareRepository;
use crate::hooks::{HookContext, HookRegistry, Trigger};

/// Creates a grove with cloned repositories for all members.
pub struct GroveCreateOperation<'a> {
    forest_root: &'a ForestRoot,
    config: &'a ForestConfig,
    hooks: &'a HookRegistry,
    emitter: Arc<dyn EventEmitter>,
    verbose: bool,
}

impl<'a> GroveCreateOperation<'a> {
    /// Creates a new grove creation operation.
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

    /// Executes the grove creation.
    pub fn execute(
        &self,
        name: &GroveName,
        branch: Option<&str>,
    ) -> Result<GroveRoot, GroveCreateError> {
        use grove_create_error::*;

        let grove_path = self.forest_root.groves_dir().join(name.to_string());
        let grove_root = GroveRoot::builder().path(&grove_path).build();

        self.emitter.emit(&ForesterEvent::GroveCreating {
            name: name.to_string(),
        });

        if grove_path.exists() {
            return Err(GroveCreateError::AlreadyExists {
                name: name.to_string(),
            });
        }

        let pre_ctx = self.hook_context(name, &grove_path, Trigger::PreGroveCreate);
        self.hooks
            .run_hooks(Trigger::PreGroveCreate, &pre_ctx)
            .context(HookSnafu)?;

        std::fs::create_dir_all(&grove_path).context(CreateDirSnafu { path: &grove_path })?;
        std::fs::create_dir_all(grove_root.marker_dir()).context(CreateDirSnafu {
            path: grove_root.marker_dir(),
        })?;

        std::fs::write(
            grove_path.join(".git"),
            "# This file prevents git from searching parent directories.
# Each grove member has its own .git directory.
gitdir: /dev/null
",
        )
        .context(WriteGitFileSnafu {
            path: grove_path.join(".git"),
        })?;

        for member in &self.config.forest.member {
            self.create_member_clone(member, &grove_path, name, branch)?;
        }

        self.create_symlinks(&grove_path, self.forest_root.path())?;

        let post_ctx = self.hook_context(name, &grove_path, Trigger::PostGroveCreate);
        self.hooks
            .run_hooks(Trigger::PostGroveCreate, &post_ctx)
            .context(HookSnafu)?;

        self.emitter.emit(&ForesterEvent::GroveCreated {
            name: name.to_string(),
        });
        Ok(grove_root)
    }

    fn create_member_clone(
        &self,
        member: &crate::domain::Member,
        grove_path: &Path,
        grove_name: &GroveName,
        branch: Option<&str>,
    ) -> Result<(), GroveCreateError> {
        use grove_create_error::*;

        let bare_path = self
            .forest_root
            .bare_dir()
            .join(format!("{}.git", member.name));
        let member_path = grove_path.join(&member.path);

        if let Some(parent) = member_path.parent() {
            std::fs::create_dir_all(parent).context(CreateDirSnafu {
                path: parent.to_path_buf(),
            })?;
        }

        let bare = BareRepository::new(&bare_path, &member.name);
        bare.clone_to(&member_path, !self.verbose)
            .context(CloneSnafu {
                member: &member.name,
            })?;

        match Checkout::plan(grove_name, member, branch) {
            Checkout::New { branch, start } => {
                BareRepository::checkout_new_branch(&member_path, &branch, &start, !self.verbose)
            }
            Checkout::Existing { branch } => {
                BareRepository::checkout_branch(&member_path, &branch, !self.verbose)
            }
        }
        .context(CheckoutSnafu {
            member: &member.name,
        })?;

        self.emitter.emit(&ForesterEvent::GroveWorktreeCreated {
            member: member.name.clone(),
            path: member_path,
        });

        Ok(())
    }

    fn create_symlinks(
        &self,
        grove_path: &Path,
        forest_path: &Path,
    ) -> Result<(), GroveCreateError> {
        use grove_create_error::*;

        let Some(grove_config) = &self.config.grove else {
            return Ok(());
        };

        for entry in &grove_config.symlink {
            let src = forest_path.join(&entry.source);
            let tgt = grove_path.join(&entry.target);

            if let Some(parent) = tgt.parent() {
                std::fs::create_dir_all(parent).context(CreateDirSnafu {
                    path: parent.to_path_buf(),
                })?;
            }

            std::os::unix::fs::symlink(&src, &tgt).context(SymlinkSnafu {
                src: src.clone(),
                tgt: tgt.clone(),
            })?;
        }

        Ok(())
    }

    fn hook_context(&self, name: &GroveName, path: &Path, trigger: Trigger) -> HookContext {
        HookContext::builder()
            .forest_root(self.forest_root.clone())
            .trigger(trigger)
            .emitter(Arc::clone(&self.emitter))
            .grove_name(name.to_string())
            .grove_path(path.to_path_buf())
            .verbose(self.verbose)
            .build()
    }
}

/// Errors from creating a grove.
#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum GroveCreateError {
    /// Grove already exists.
    #[snafu(display("Grove '{name}' already exists"))]
    AlreadyExists {
        /// Grove name.
        name: String,
    },

    /// Failed to create a directory.
    #[snafu(display("Failed to create directory"))]
    CreateDir {
        /// Path that could not be created.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },

    /// Failed to write .git barrier file.
    #[snafu(display("Failed to write .git barrier file"))]
    WriteGitFile {
        /// Path that could not be written.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },

    /// Failed to clone a member repository.
    #[snafu(display("Failed to clone repository for '{member}'"))]
    Clone {
        /// Member name.
        member: String,
        /// Underlying clone error.
        source: crate::git::GitCommandError,
    },

    /// Failed to checkout branch.
    #[snafu(display("Failed to checkout branch for '{member}'"))]
    Checkout {
        /// Member name.
        member: String,
        /// Underlying checkout error.
        source: crate::git::GitCommandError,
    },

    /// Failed to create a symlink.
    #[snafu(display("Failed to create symlink"))]
    Symlink {
        /// Source path.
        src: PathBuf,
        /// Target path.
        tgt: PathBuf,
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
