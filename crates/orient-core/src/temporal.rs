//! Temporal orientation — time-binding behaviors.
//!
//! Per AVP-Doctrine `N_ORIENTATION_SUBSTRATE.md` § Orientation 12.
//! Multi-valued: an audit-chain entry is `Monotonic + Archival +
//! VersionedImmutable`; a session token is `SessionScoped +
//! Ephemeral`.
//!
//! Closes `#195 [orient-v7]` (in batch).

use serde::{Deserialize, Serialize};

/// Time-bound behavior declaration. Multi-valued.
/// `#[non_exhaustive]` for additivity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Temporal {
    /// Lives for one user session.
    SessionScoped,
    /// Lives for one HTTP request.
    RequestScoped,
    /// Materialized at `forge build` time.
    BuildScoped,
    /// Regenerated daily (token rotation, summary digests).
    DailyRecurring,
    /// Monthly cycle.
    MonthlyRecurring,
    /// Long-term retention for compliance.
    Archival,
    /// Destroyed immediately after use.
    Ephemeral,
    /// Every change produces a new version; old retained.
    VersionedImmutable,
    /// Increases monotonically (audit chain, sequence numbers).
    Monotonic,
    /// Keeps last `N` (operator-declared).
    BoundedHistory,
}

impl Temporal {
    /// Canonical kebab-case slug.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::SessionScoped => "session-scoped",
            Self::RequestScoped => "request-scoped",
            Self::BuildScoped => "build-scoped",
            Self::DailyRecurring => "daily-recurring",
            Self::MonthlyRecurring => "monthly-recurring",
            Self::Archival => "archival",
            Self::Ephemeral => "ephemeral",
            Self::VersionedImmutable => "versioned-immutable",
            Self::Monotonic => "monotonic",
            Self::BoundedHistory => "bounded-history",
        }
    }

    /// All canonical values.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::SessionScoped,
            Self::RequestScoped,
            Self::BuildScoped,
            Self::DailyRecurring,
            Self::MonthlyRecurring,
            Self::Archival,
            Self::Ephemeral,
            Self::VersionedImmutable,
            Self::Monotonic,
            Self::BoundedHistory,
        ]
    }

    /// Parse from slug.
    #[must_use]
    pub fn from_slug(s: &str) -> Option<Self> {
        Self::all().iter().copied().find(|t| t.slug() == s)
    }
}

impl std::fmt::Display for Temporal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_canonical_values() {
        assert_eq!(Temporal::all().len(), 10);
    }

    #[test]
    fn all_unique_slugs() {
        let mut seen = std::collections::HashSet::new();
        for t in Temporal::all() {
            assert!(seen.insert(t.slug()));
        }
    }

    #[test]
    fn from_slug_roundtrip() {
        for t in Temporal::all() {
            assert_eq!(Temporal::from_slug(t.slug()), Some(*t));
        }
    }

    #[test]
    fn serde_specific_slug() {
        assert_eq!(
            serde_json::to_string(&Temporal::VersionedImmutable).unwrap(),
            "\"versioned-immutable\""
        );
    }
}
