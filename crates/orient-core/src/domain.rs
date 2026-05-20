//! Domain orientation — vertical / topic an entity binds to.
//!
//! Per AVP-Doctrine `N_ORIENTATION_SUBSTRATE.md` § Orientation 5
//! and rule prim-012: substrate primitives MUST declare
//! `Domain::Agnostic`. Site-specific composition binds a vertical
//! domain. Mapping-table `domain-to-compliance` drives implied
//! compliance posture (`Healthcare -> [Hipaa, Gdpr, ...]`).
//!
//! Closes `#195 [orient-v7]` (in batch).

use serde::{Deserialize, Serialize};

/// Vertical / topic an entity binds to. Multi-valued in the
/// projection (a site can be `Healthcare + Education`).
/// `#[non_exhaustive]` for additivity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Domain {
    /// Healthcare — touches PHI, implies HIPAA + GDPR in EU.
    Healthcare,
    /// Finance — implies PCI-DSS-4 / SOC2 / GDPR / CCPA.
    Finance,
    /// Hospitality — booking, travel, accommodations.
    Hospitality,
    /// Voting — implies state-vote-acts + WCAG 2.2 AAA.
    Voting,
    /// Education — student records, FERPA implications.
    Education,
    /// E-commerce — transactional sites with returns / shipping.
    Ecommerce,
    /// Legal — case management, regulatory filings.
    Legal,
    /// Journalism — content / news / editorial.
    Journalism,
    /// Philanthropy — nonprofits, charitable giving.
    Philanthropy,
    /// AI research — model evaluation, dataset hosting.
    AiResearch,
    /// Substrate-general — the default for Loom primitives per
    /// rule prim-012. Means "not bound to a specific vertical."
    Agnostic,
}

impl Domain {
    /// Canonical snake_case slug.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Healthcare => "healthcare",
            Self::Finance => "finance",
            Self::Hospitality => "hospitality",
            Self::Voting => "voting",
            Self::Education => "education",
            Self::Ecommerce => "ecommerce",
            Self::Legal => "legal",
            Self::Journalism => "journalism",
            Self::Philanthropy => "philanthropy",
            Self::AiResearch => "ai_research",
            Self::Agnostic => "agnostic",
        }
    }

    /// All canonical values.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::Healthcare,
            Self::Finance,
            Self::Hospitality,
            Self::Voting,
            Self::Education,
            Self::Ecommerce,
            Self::Legal,
            Self::Journalism,
            Self::Philanthropy,
            Self::AiResearch,
            Self::Agnostic,
        ]
    }

    /// Parse from slug.
    #[must_use]
    pub fn from_slug(s: &str) -> Option<Self> {
        Self::all().iter().copied().find(|d| d.slug() == s)
    }
}

impl std::fmt::Display for Domain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eleven_canonical_values() {
        assert_eq!(Domain::all().len(), 11);
    }

    #[test]
    fn all_unique_slugs() {
        let mut seen = std::collections::HashSet::new();
        for d in Domain::all() {
            assert!(seen.insert(d.slug()));
        }
    }

    #[test]
    fn from_slug_roundtrip() {
        for d in Domain::all() {
            assert_eq!(Domain::from_slug(d.slug()), Some(*d));
        }
    }

    #[test]
    fn agnostic_is_default_for_primitives() {
        // Per rule prim-012; documented invariant.
        assert_eq!(Domain::Agnostic.slug(), "agnostic");
    }
}
