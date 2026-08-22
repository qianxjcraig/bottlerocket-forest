//! Hook execution triggers.

use std::fmt;

/// Events that can trigger hook execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Trigger {
    /// Before seeding the forest.
    PreSeed,
    /// After seeding the forest.
    PostSeed,
    /// Before creating a grove.
    PreGroveCreate,
    /// After creating a grove.
    PostGroveCreate,
    /// Before updating a grove.
    PreGroveUpdate,
    /// After updating a grove.
    PostGroveUpdate,
    /// Before removing a grove.
    PreGroveRemove,
    /// After removing a grove.
    PostGroveRemove,
}

impl Trigger {
    /// Parses a trigger from a string.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pre-seed" => Some(Self::PreSeed),
            "post-seed" => Some(Self::PostSeed),
            "pre-grove-create" => Some(Self::PreGroveCreate),
            "post-grove-create" => Some(Self::PostGroveCreate),
            "pre-grove-update" => Some(Self::PreGroveUpdate),
            "post-grove-update" => Some(Self::PostGroveUpdate),
            "pre-grove-remove" => Some(Self::PreGroveRemove),
            "post-grove-remove" => Some(Self::PostGroveRemove),
            _ => None,
        }
    }

    /// Returns the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PreSeed => "pre-seed",
            Self::PostSeed => "post-seed",
            Self::PreGroveCreate => "pre-grove-create",
            Self::PostGroveCreate => "post-grove-create",
            Self::PreGroveUpdate => "pre-grove-update",
            Self::PostGroveUpdate => "post-grove-update",
            Self::PreGroveRemove => "pre-grove-remove",
            Self::PostGroveRemove => "post-grove-remove",
        }
    }
}

impl fmt::Display for Trigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
