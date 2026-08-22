//! Event types for forester operations.

use std::path::PathBuf;

/// Events emitted during forester operations.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ForesterEvent {
    /// Forest seeding has started.
    SeedStarted,
    /// A member repository is being cloned.
    MemberCloning {
        /// Member name.
        name: String,
        /// Remote URL.
        remote: String,
    },
    /// A member repository has been cloned.
    MemberCloned {
        /// Member name.
        name: String,
    },
    /// Forest seeding has completed.
    SeedCompleted,
    /// A grove is being created.
    GroveCreating {
        /// Grove name.
        name: String,
    },
    /// A worktree has been created for a grove member.
    GroveWorktreeCreated {
        /// Member name.
        member: String,
        /// Worktree path.
        path: PathBuf,
    },
    /// A grove has been created.
    GroveCreated {
        /// Grove name.
        name: String,
    },
    /// A grove is being removed.
    GroveRemoving {
        /// Grove name.
        name: String,
    },
    /// A grove has been removed.
    GroveRemoved {
        /// Grove name.
        name: String,
    },
    /// Groves are being listed.
    GroveListing,
    /// A hook is being executed.
    HookExecuting {
        /// Hook name.
        name: String,
    },
    /// A hook has completed.
    HookCompleted {
        /// Hook name.
        name: String,
    },
    /// A warning message.
    Warning(String),
    /// An informational message.
    Info(String),
    /// Sync-seed has started.
    SyncSeedStarted,
    /// A member repository is being fetched.
    MemberFetching {
        /// Member name.
        name: String,
    },
    /// A member repository has been fetched.
    MemberFetched {
        /// Member name.
        name: String,
    },
    /// A member was skipped during sync-seed.
    MemberSkipped {
        /// Member name.
        name: String,
        /// Reason for skipping.
        reason: String,
    },
    /// Sync-seed has completed.
    SyncSeedCompleted,
    /// A grove update has started.
    GroveUpdateStarted {
        /// Grove name.
        grove: String,
    },
    /// A grove member is being updated.
    MemberUpdating {
        /// Member name.
        name: String,
    },
    /// A grove member was advanced.
    MemberUpdated {
        /// Member name.
        name: String,
        /// What changed, e.g. "fast-forwarded +12".
        detail: String,
    },
    /// A grove member was already current.
    MemberCurrent {
        /// Member name.
        name: String,
    },
    /// A grove update has completed.
    GroveUpdateCompleted {
        /// Grove name.
        grove: String,
        /// Members advanced.
        updated: usize,
        /// Members deliberately left untouched.
        skipped: usize,
    },
    /// An error message.
    Error(String),
}
