//! How a grove member is checked out.
//!
//! Shared by grove creation and grove update: a member added to `forester.toml`
//! after a grove exists must land on the same branch it would have at creation
//! time, so the rule lives in one place.

use crate::domain::{GroveName, Member};

/// The conventional grove name that tracks each member's default branch.
pub const TRACKING_GROVE: &str = "develop";

/// The branch a grove member should end up on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checkout {
    /// Check out a branch that already exists.
    Existing {
        /// Branch to check out.
        branch: String,
    },
    /// Create a grove-local branch forked from `start`.
    New {
        /// Branch to create.
        branch: String,
        /// Revision to fork from.
        start: String,
    },
}

impl Checkout {
    /// Decides how `member` is checked out for `grove`.
    ///
    /// The `develop` grove tracks each member's default branch directly; any other
    /// grove gets its own `<grove>/<member>` branch so parallel work cannot
    /// collide. An explicit branch request overrides both.
    pub fn plan(grove: &GroveName, member: &Member, requested: Option<&str>) -> Self {
        if let Some(branch) = requested {
            return Self::Existing {
                branch: branch.to_string(),
            };
        }

        if grove.to_string() == TRACKING_GROVE {
            return Self::Existing {
                branch: member.branch().to_string(),
            };
        }

        Self::New {
            branch: format!("{}/{}", grove, member.name),
            start: format!("origin/{}", member.branch()),
        }
    }

    /// The branch name the member ends up on.
    pub fn branch(&self) -> &str {
        match self {
            Self::Existing { branch } | Self::New { branch, .. } => branch,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    fn member() -> Member {
        Member::builder()
            .name("twoliter")
            .remote("git@example.com:org/twoliter.git")
            .path("twoliter")
            .default_branch("develop".to_string())
            .build()
    }

    fn grove(name: &str) -> GroveName {
        GroveName::try_new(name).unwrap()
    }

    #[test]
    fn tracking_grove_uses_the_member_default_branch() {
        // Given the conventional tracking grove
        let grove = grove(TRACKING_GROVE);

        // When planning a member checkout
        let checkout = Checkout::plan(&grove, &member(), None);

        // Then the member's own default branch is used
        assert_eq!(
            checkout,
            Checkout::Existing {
                branch: "develop".to_string()
            }
        );
    }

    #[test]
    fn feature_grove_forks_a_namespaced_branch_from_upstream() {
        // Given a feature grove
        let grove = grove("my-feature");

        // When planning a member checkout
        let checkout = Checkout::plan(&grove, &member(), None);

        // Then a grove-local branch is forked from the tracked default branch
        assert_eq!(
            checkout,
            Checkout::New {
                branch: "my-feature/twoliter".to_string(),
                start: "origin/develop".to_string(),
            }
        );
    }

    #[test_case(TRACKING_GROVE ; "tracking grove")]
    #[test_case("my-feature" ; "feature grove")]
    fn explicit_branch_overrides_grove_convention(name: &str) {
        // Given an explicit branch request
        let requested = Some("release-1.2");

        // When planning a member checkout for any grove
        let checkout = Checkout::plan(&grove(name), &member(), requested);

        // Then the requested branch is used as-is
        assert_eq!(
            checkout,
            Checkout::Existing {
                branch: "release-1.2".to_string()
            }
        );
    }
}
