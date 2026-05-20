//! Risk orientation — AVP-2 tier ladder.
//!
//! Per AVP-Doctrine `N_ORIENTATION_SUBSTRATE.md` § Orientation 8
//! and the AVP-2 protocol (`AVP2_PROTOCOL.md`): every substrate
//! entity declares its risk tier; the substrate refuses to ship
//! capability-class entities below their minimum tier (e.g.
//! payment-related code must reach `tier-6-mutation`).
//!
//! Single-valued (an entity has exactly one risk tier — the
//! highest reached by the test surface that validates it). Mapping
//! tables encode capability-class → minimum-tier requirements per
//! `MAPPING_TABLES.md` § `risk-to-required-tests`.
//!
//! Per `[[backward-compat-version-discipline]]`: tier ladder is
//! `#[non_exhaustive]` so future AVP-2 protocol revisions can add
//! tiers additively without breaking downstream code.
//!
//! Closes `#193 [orient-v5]`.

use serde::{Deserialize, Serialize};

/// AVP-2 risk tier. Single-valued per entity. Ordered: higher
/// tier = stronger verification surface. Mapping tables in
/// `PlausiDen-AVP-Doctrine/mappings/risk-to-required-tests.toml`
/// declare which tier each test class establishes.
///
/// Stable slugs for serde rename_all: kebab-case-with-tier-prefix
/// (e.g. `tier-6-mutation`). Reads identically across Claude /
/// Gemini / other agents (cross-AI parity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Risk {
    /// Tier 1 — smoke test. The entity exists and doesn't panic
    /// on construction. Lowest acceptable for an experimental
    /// lifecycle entity.
    #[serde(rename = "tier-1-trivial")]
    Tier1Trivial,

    /// Tier 2 — unit tests on the entity's public surface.
    #[serde(rename = "tier-2-unit")]
    Tier2Unit,

    /// Tier 3 — integration tests against representative inputs.
    /// Minimum for `Forge.Subcommand` per
    /// `DETERMINISTIC_FIRST.md`.
    #[serde(rename = "tier-3-functional")]
    Tier3Functional,

    /// Tier 4 — proptest-style property tests at every input
    /// boundary. Catches the class of bugs where unit tests pass
    /// but generated inputs break.
    #[serde(rename = "tier-4-property")]
    Tier4Property,

    /// Tier 5 — cargo-fuzz / afl-style fuzzing on parsing /
    /// validation paths.
    #[serde(rename = "tier-5-fuzz")]
    Tier5Fuzz,

    /// Tier 6 — mutation testing (cargo-mutants) proves the test
    /// surface catches semantic regressions. Minimum for payment
    /// / auth / consent paths.
    #[serde(rename = "tier-6-mutation")]
    Tier6Mutation,

    /// Tier 7 — concurrent-execution model testing (loom /
    /// shuttle) proves invariants hold under any thread
    /// interleaving.
    #[serde(rename = "tier-7-concurrent")]
    Tier7Concurrent,

    /// Tier 8 — formal verification (TLA+ specification with
    /// proof, Lean4 mechanized proof). For protocol-critical
    /// surfaces.
    #[serde(rename = "tier-8-formal")]
    Tier8Formal,

    /// Tier 9 — red-team gated. Independent adversarial review
    /// found no remaining attacks at this tier. For security-
    /// critical surfaces only.
    #[serde(rename = "tier-9-adversarial")]
    Tier9Adversarial,

    /// Tier 10 — economic / incentive-compatibility proven.
    /// Used for cryptoeconomic protocols, voting / governance
    /// surfaces, mechanism design. Highest tier.
    #[serde(rename = "tier-10-economic")]
    Tier10Economic,
}

impl Risk {
    /// Return the canonical slug. Stable across versions.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Tier1Trivial => "tier-1-trivial",
            Self::Tier2Unit => "tier-2-unit",
            Self::Tier3Functional => "tier-3-functional",
            Self::Tier4Property => "tier-4-property",
            Self::Tier5Fuzz => "tier-5-fuzz",
            Self::Tier6Mutation => "tier-6-mutation",
            Self::Tier7Concurrent => "tier-7-concurrent",
            Self::Tier8Formal => "tier-8-formal",
            Self::Tier9Adversarial => "tier-9-adversarial",
            Self::Tier10Economic => "tier-10-economic",
        }
    }

    /// All canonical risk tiers in ascending order (Tier1 → Tier10).
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::Tier1Trivial,
            Self::Tier2Unit,
            Self::Tier3Functional,
            Self::Tier4Property,
            Self::Tier5Fuzz,
            Self::Tier6Mutation,
            Self::Tier7Concurrent,
            Self::Tier8Formal,
            Self::Tier9Adversarial,
            Self::Tier10Economic,
        ]
    }

    /// Parse a risk tier from its canonical slug.
    #[must_use]
    pub fn from_slug(s: &str) -> Option<Self> {
        Self::all().iter().copied().find(|r| r.slug() == s)
    }

    /// Tier as an integer 1..=10. Used by mapping-table lookups
    /// that compare against a minimum required tier
    /// (`Risk::at_least(Risk::Tier6Mutation)`).
    #[must_use]
    pub fn tier_number(self) -> u8 {
        match self {
            Self::Tier1Trivial => 1,
            Self::Tier2Unit => 2,
            Self::Tier3Functional => 3,
            Self::Tier4Property => 4,
            Self::Tier5Fuzz => 5,
            Self::Tier6Mutation => 6,
            Self::Tier7Concurrent => 7,
            Self::Tier8Formal => 8,
            Self::Tier9Adversarial => 9,
            Self::Tier10Economic => 10,
        }
    }
}

impl std::fmt::Display for Risk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_ten_tiers_present() {
        assert_eq!(Risk::all().len(), 10);
    }

    #[test]
    fn tier_numbers_are_ascending_one_through_ten() {
        let nums: Vec<u8> = Risk::all().iter().map(|r| r.tier_number()).collect();
        assert_eq!(nums, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn ord_matches_tier_number() {
        assert!(Risk::Tier1Trivial < Risk::Tier6Mutation);
        assert!(Risk::Tier6Mutation < Risk::Tier10Economic);
        assert!(Risk::Tier10Economic > Risk::Tier1Trivial);
    }

    #[test]
    fn from_slug_roundtrip() {
        for r in Risk::all() {
            let back = Risk::from_slug(r.slug()).expect("known");
            assert_eq!(back, *r);
        }
    }

    #[test]
    fn from_slug_rejects_unknown() {
        assert!(Risk::from_slug("").is_none());
        assert!(Risk::from_slug("tier-0").is_none());
        assert!(Risk::from_slug("tier-11-superhuman").is_none());
        assert!(Risk::from_slug("Tier6Mutation").is_none()); // not snake/kebab
    }

    #[test]
    fn serde_roundtrip_canonical_slugs() {
        for r in Risk::all() {
            let json = serde_json::to_string(r).expect("serialize");
            // Verify the slug is in the JSON.
            assert!(json.contains(r.slug()), "{r:?} serialized as {json}");
            let back: Risk = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, *r);
        }
    }

    #[test]
    fn serde_specific_slug() {
        let json = serde_json::to_string(&Risk::Tier6Mutation).expect("serialize");
        assert_eq!(json, "\"tier-6-mutation\"");
    }
}
