//! Operations layer for forester.
//!
//! Each operation takes references to EventEmitter and HookRegistry,
//! uses domain types, and emits events for user feedback.

mod grove_create;
mod grove_list;
mod grove_remove;
mod grove_update;
mod init;
mod seed;
mod sync_seed;
mod update_report;

pub use grove_create::{GroveCreateError, GroveCreateOperation};
pub use grove_list::{GroveListError, GroveListOperation};
pub use grove_remove::{GroveRemoveError, GroveRemoveOperation};
pub use grove_update::{GroveUpdateError, GroveUpdateOperation, Strategy, UpdateOptions};
pub use init::{InitError, InitOperation};
pub use seed::{SeedError, SeedOperation};
pub use sync_seed::{SyncSeedError, SyncSeedOperation};
pub use update_report::{MemberOutcome, MemberUpdate, SkipReason, UpdateReport};
