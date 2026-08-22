//! Git abstraction layer for forester.
//!
//! Provides operations for bare repositories and clones.

mod bare;
mod command;
mod member;

pub use bare::BareRepository;
pub use command::{GitCommandError, GitOutcome};
pub use member::{AheadBehind, MemberRepository, short_sha};
