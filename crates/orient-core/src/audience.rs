//! Audience orientation — consumers an entity serves.
//!
//! Per AVP-Doctrine `N_ORIENTATION_SUBSTRATE.md` § Orientation 4.
//! Multi-valued: `forge orient` serves `internal_engineer + ai_agent`;
//! a payment form serves `end_user`; a doctrine page serves
//! `internal_engineer + regulator + ai_agent`.
//!
//! Closes `#195 [orient-v7]` (in batch with domain/lifecycle/
//! accessibility/temporal).

use serde::{Deserialize, Serialize};

/// Consumer the entity serves. Multi-valued. `#[non_exhaustive]`
/// for additivity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Audience {
    /// The human site visitor.
    EndUser,
    /// Substrate / tenant administrator.
    Operator,
    /// Third-party integrating against APIs.
    PartnerDeveloper,
    /// Compliance officer / auditor.
    Regulator,
    /// Claude / Gemini / other AI agents (cross-AI per
    /// `[[priority-architectural-first-and-cross-ai]]`).
    AiAgent,
    /// Long-term compliance retention.
    LegalArchive,
    /// Substrate contributors.
    InternalEngineer,
}

impl Audience {
    /// Canonical snake_case slug.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::EndUser => "end_user",
            Self::Operator => "operator",
            Self::PartnerDeveloper => "partner_developer",
            Self::Regulator => "regulator",
            Self::AiAgent => "ai_agent",
            Self::LegalArchive => "legal_archive",
            Self::InternalEngineer => "internal_engineer",
        }
    }

    /// All canonical values.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::EndUser,
            Self::Operator,
            Self::PartnerDeveloper,
            Self::Regulator,
            Self::AiAgent,
            Self::LegalArchive,
            Self::InternalEngineer,
        ]
    }

    /// Parse from slug.
    #[must_use]
    pub fn from_slug(s: &str) -> Option<Self> {
        Self::all().iter().copied().find(|a| a.slug() == s)
    }
}

impl std::fmt::Display for Audience {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seven_canonical_values() {
        assert_eq!(Audience::all().len(), 7);
    }

    #[test]
    fn all_unique_slugs() {
        let mut seen = std::collections::HashSet::new();
        for a in Audience::all() {
            assert!(seen.insert(a.slug()));
        }
    }

    #[test]
    fn from_slug_roundtrip() {
        for a in Audience::all() {
            assert_eq!(Audience::from_slug(a.slug()), Some(*a));
        }
    }

    #[test]
    fn serde_specific_slug() {
        assert_eq!(
            serde_json::to_string(&Audience::AiAgent).unwrap(),
            "\"ai_agent\""
        );
    }
}
