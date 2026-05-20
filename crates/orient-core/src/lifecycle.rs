//! Lifecycle orientation — entity's evolution stage.
//!
//! Per AVP-Doctrine `N_ORIENTATION_SUBSTRATE.md` § Orientation 6
//! and `VERSION_DISCIPLINE.md` § Per-artifact-class versioning:
//! lifecycle parallels doctrine-rule lifecycle. Single-valued.
//!
//! Closes `#195 [orient-v7]` (in batch).

use serde::{Deserialize, Serialize};

/// Where the entity sits in its evolution. Single-valued.
/// `#[non_exhaustive]` for future stages (e.g. `archived` may join).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Lifecycle {
    /// Trial; advisory enforcement; default for new entities.
    Experimental,
    /// Functional but unstable; gated.
    Beta,
    /// Binding; strict enforcement per severity.
    Stable,
    /// Sunset scheduled; requires `deprecated_at` + `replaced_by`.
    Deprecated,
    /// Removed; archived for citation. Read-only at runtime.
    Retired,
}

impl Lifecycle {
    /// Canonical snake_case slug.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Experimental => "experimental",
            Self::Beta => "beta",
            Self::Stable => "stable",
            Self::Deprecated => "deprecated",
            Self::Retired => "retired",
        }
    }

    /// All canonical values in ascending maturity order.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::Experimental,
            Self::Beta,
            Self::Stable,
            Self::Deprecated,
            Self::Retired,
        ]
    }

    /// Parse from slug.
    #[must_use]
    pub fn from_slug(s: &str) -> Option<Self> {
        Self::all().iter().copied().find(|l| l.slug() == s)
    }
}

impl std::fmt::Display for Lifecycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_canonical_states() {
        assert_eq!(Lifecycle::all().len(), 5);
    }

    #[test]
    fn from_slug_roundtrip() {
        for l in Lifecycle::all() {
            assert_eq!(Lifecycle::from_slug(l.slug()), Some(*l));
        }
    }

    #[test]
    fn serde_specific_slug() {
        assert_eq!(
            serde_json::to_string(&Lifecycle::Experimental).unwrap(),
            "\"experimental\""
        );
    }
}
