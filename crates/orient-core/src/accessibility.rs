//! Accessibility orientation — a11y target levels.
//!
//! Per AVP-Doctrine `N_ORIENTATION_SUBSTRATE.md` § Orientation 10
//! and rule a11y-003: substrate floor is `Wcag21Aa`; new primitives
//! target `Wcag22Aa`. Single-valued at the target level.
//!
//! Closes `#195 [orient-v7]` (in batch).

use serde::{Deserialize, Serialize};

/// W3C accessibility target level. Single-valued. Higher level
/// implies satisfaction of all lower levels per WCAG conformance.
/// `#[non_exhaustive]` for additivity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Accessibility {
    /// WCAG 2.1 Level A. Minimum legal floor in most jurisdictions.
    #[serde(rename = "wcag-2.1-a")]
    Wcag21A,
    /// WCAG 2.1 Level AA. Substrate default per rule a11y-003.
    #[serde(rename = "wcag-2.1-aa")]
    Wcag21Aa,
    /// WCAG 2.1 Level AAA. Aspirational; opt-in per primitive.
    #[serde(rename = "wcag-2.1-aaa")]
    Wcag21Aaa,
    /// WCAG 2.2 Level AA. Substrate target for net-new primitives.
    #[serde(rename = "wcag-2.2-aa")]
    Wcag22Aa,
    /// WCAG 2.2 Level AAA. Strongest standardized target.
    #[serde(rename = "wcag-2.2-aaa")]
    Wcag22Aaa,
}

impl Accessibility {
    /// Canonical slug.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Wcag21A => "wcag-2.1-a",
            Self::Wcag21Aa => "wcag-2.1-aa",
            Self::Wcag21Aaa => "wcag-2.1-aaa",
            Self::Wcag22Aa => "wcag-2.2-aa",
            Self::Wcag22Aaa => "wcag-2.2-aaa",
        }
    }

    /// All canonical values in ascending strictness order.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::Wcag21A,
            Self::Wcag21Aa,
            Self::Wcag21Aaa,
            Self::Wcag22Aa,
            Self::Wcag22Aaa,
        ]
    }

    /// Parse from slug.
    #[must_use]
    pub fn from_slug(s: &str) -> Option<Self> {
        Self::all().iter().copied().find(|a| a.slug() == s)
    }
}

impl std::fmt::Display for Accessibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_canonical_levels() {
        assert_eq!(Accessibility::all().len(), 5);
    }

    #[test]
    fn ord_matches_strictness() {
        // Note: this depends on derived Ord order matching variant
        // declaration order. The canonical-list order above matches
        // ascending strictness, so 2.1-A < 2.1-AA < 2.1-AAA < 2.2-AA
        // < 2.2-AAA holds.
        assert!(Accessibility::Wcag21A < Accessibility::Wcag21Aa);
        assert!(Accessibility::Wcag21Aa < Accessibility::Wcag22Aa);
        assert!(Accessibility::Wcag22Aa < Accessibility::Wcag22Aaa);
    }

    #[test]
    fn from_slug_roundtrip() {
        for a in Accessibility::all() {
            assert_eq!(Accessibility::from_slug(a.slug()), Some(*a));
        }
    }
}
