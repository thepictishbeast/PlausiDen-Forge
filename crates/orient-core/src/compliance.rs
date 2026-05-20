//! Compliance orientation — regulatory regimes an entity conforms
//! to.
//!
//! Per AVP-Doctrine `N_ORIENTATION_SUBSTRATE.md` § Orientation 7
//! and `MAPPING_TABLES.md` § `domain-to-compliance`: each entity
//! declares its compliance posture explicitly so the substrate
//! can:
//!
//!   - refuse to build a payment surface that doesn't declare
//!     `pci-dss-4`
//!   - refuse to ship a healthcare-domain entity that doesn't
//!     declare `hipaa` + `gdpr` (in EU jurisdictions)
//!   - export per-regulator compliance inventories on demand
//!
//! Multi-valued: a payment form typically declares
//! `[gdpr, ccpa, pci-dss-4, wcag-2.1-aa]`. Per
//! `[[backward-compat-version-discipline]]`: enum is
//! `#[non_exhaustive]` so new regimes (DORA / CRA / new state
//! privacy laws) are additive Cat-2 changes.
//!
//! Per `[[deterministic-first-lfi-optional]]`: discrete enum, no
//! AI inference — compliance values come from regulatory text +
//! legal review, never from heuristic classification.
//!
//! Closes `#191 [orient-v3]`.

use serde::{Deserialize, Serialize};

/// Regulatory regime an entity conforms to. Multi-valued in the
/// manifest projection. Closed enum; new regimes are added via
/// doctrine + capability-request with legal review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Compliance {
    /// EU General Data Protection Regulation. Applies to any
    /// entity handling EU-resident personal data.
    #[serde(rename = "gdpr")]
    Gdpr,

    /// California Consumer Privacy Act (+ CPRA extension).
    /// Applies to entities handling California-resident data.
    #[serde(rename = "ccpa")]
    Ccpa,

    /// US Health Insurance Portability and Accountability Act.
    /// Applies to any entity handling PHI.
    #[serde(rename = "hipaa")]
    Hipaa,

    /// Payment Card Industry Data Security Standard v4. Applies
    /// to entities that store / process / transmit cardholder data.
    /// Per `MAPPING_TABLES.md`: requires `cleartext-forbidden`
    /// sovereignty value.
    #[serde(rename = "pci-dss-4")]
    PciDss4,

    /// Service Organization Control 2 Type II. Independent audit
    /// of security / availability / integrity / confidentiality /
    /// privacy controls over a 6-12 month period.
    #[serde(rename = "soc2-type-ii")]
    Soc2TypeIi,

    /// ISO/IEC 27001 — information security management system.
    /// Applies to entities operating with formal infosec program.
    #[serde(rename = "iso-27001")]
    Iso27001,

    /// ISO/IEC 25010 — software product quality model
    /// (functional suitability / reliability / performance /
    /// usability / security / etc.).
    #[serde(rename = "iso-25010")]
    Iso25010,

    /// ISO/IEC 40500 — references WCAG 2.0 as an ISO standard
    /// (older standardization route; WCAG 2.1/2.2 specified
    /// separately below).
    #[serde(rename = "iso-40500")]
    Iso40500,

    /// W3C Web Content Accessibility Guidelines 2.1 AA. Substrate
    /// floor for any consumer-facing site.
    #[serde(rename = "wcag-2.1-aa")]
    Wcag21Aa,

    /// W3C WCAG 2.1 AAA. Aspirational; opt-in per primitive.
    #[serde(rename = "wcag-2.1-aaa")]
    Wcag21Aaa,

    /// W3C WCAG 2.2 AA. Substrate target for net-new primitives.
    #[serde(rename = "wcag-2.2-aa")]
    Wcag22Aa,

    /// W3C WCAG 2.2 AAA. Strongest standardized a11y target.
    #[serde(rename = "wcag-2.2-aaa")]
    Wcag22Aaa,

    /// EU Digital Operational Resilience Act. Financial-sector
    /// ICT resilience requirements.
    #[serde(rename = "dora")]
    Dora,

    /// EU Cyber Resilience Act. Mandatory security requirements
    /// for products with digital elements sold in the EU.
    #[serde(rename = "cra")]
    Cra,

    /// US state-level vote / election operational acts.
    /// Placeholder for the cluster (per-state acts may be
    /// disambiguated later under finer-grained variants).
    #[serde(rename = "state-vote-acts")]
    StateVoteActs,

    /// Explicit "not applicable" — declared, never implicit.
    /// Used when an entity's compliance posture is empty after
    /// review (not by default). Audit phases distinguish "declared
    /// none" from "didn't declare."
    #[serde(rename = "none-applicable")]
    NoneApplicable,
}

impl Compliance {
    /// Canonical kebab-case slug. Stable across versions; matches
    /// the `MAPPING_TABLES.md` source values.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Gdpr => "gdpr",
            Self::Ccpa => "ccpa",
            Self::Hipaa => "hipaa",
            Self::PciDss4 => "pci-dss-4",
            Self::Soc2TypeIi => "soc2-type-ii",
            Self::Iso27001 => "iso-27001",
            Self::Iso25010 => "iso-25010",
            Self::Iso40500 => "iso-40500",
            Self::Wcag21Aa => "wcag-2.1-aa",
            Self::Wcag21Aaa => "wcag-2.1-aaa",
            Self::Wcag22Aa => "wcag-2.2-aa",
            Self::Wcag22Aaa => "wcag-2.2-aaa",
            Self::Dora => "dora",
            Self::Cra => "cra",
            Self::StateVoteActs => "state-vote-acts",
            Self::NoneApplicable => "none-applicable",
        }
    }

    /// All canonical compliance values in stable iteration order.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::Gdpr,
            Self::Ccpa,
            Self::Hipaa,
            Self::PciDss4,
            Self::Soc2TypeIi,
            Self::Iso27001,
            Self::Iso25010,
            Self::Iso40500,
            Self::Wcag21Aa,
            Self::Wcag21Aaa,
            Self::Wcag22Aa,
            Self::Wcag22Aaa,
            Self::Dora,
            Self::Cra,
            Self::StateVoteActs,
            Self::NoneApplicable,
        ]
    }

    /// Parse from canonical slug. Returns `None` for unknown —
    /// callers fail-closed.
    #[must_use]
    pub fn from_slug(s: &str) -> Option<Self> {
        Self::all().iter().copied().find(|c| c.slug() == s)
    }

    /// True if this regime requires the entity to ship `cleartext-
    /// forbidden` sovereignty. Mapping-table helper.
    #[must_use]
    pub fn requires_cleartext_forbidden(self) -> bool {
        matches!(self, Self::PciDss4 | Self::Hipaa)
    }

    /// True if this regime is a privacy regulation (vs an a11y /
    /// security / resilience standard). Used by privacy-export
    /// flows to filter the per-entity compliance inventory.
    #[must_use]
    pub fn is_privacy_regulation(self) -> bool {
        matches!(self, Self::Gdpr | Self::Ccpa | Self::Hipaa)
    }
}

impl std::fmt::Display for Compliance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_canonical_slugs_non_empty() {
        for c in Compliance::all() {
            assert!(!c.slug().is_empty());
        }
    }

    #[test]
    fn all_unique_slugs() {
        let mut seen = std::collections::HashSet::new();
        for c in Compliance::all() {
            assert!(seen.insert(c.slug()), "duplicate slug: {:?}", c.slug());
        }
        assert_eq!(seen.len(), 16, "expected 16 canonical compliance values");
    }

    #[test]
    fn from_slug_roundtrip() {
        for c in Compliance::all() {
            let back = Compliance::from_slug(c.slug()).expect("known");
            assert_eq!(back, *c);
        }
    }

    #[test]
    fn from_slug_rejects_unknown() {
        assert!(Compliance::from_slug("").is_none());
        assert!(Compliance::from_slug("PCI").is_none()); // uppercase rejected
        assert!(Compliance::from_slug("gdpr-v2").is_none()); // not in enum
    }

    #[test]
    fn requires_cleartext_forbidden_for_pci_and_hipaa() {
        assert!(Compliance::PciDss4.requires_cleartext_forbidden());
        assert!(Compliance::Hipaa.requires_cleartext_forbidden());
        assert!(!Compliance::Gdpr.requires_cleartext_forbidden());
        assert!(!Compliance::Wcag21Aa.requires_cleartext_forbidden());
    }

    #[test]
    fn is_privacy_regulation_correctly_classified() {
        assert!(Compliance::Gdpr.is_privacy_regulation());
        assert!(Compliance::Ccpa.is_privacy_regulation());
        assert!(Compliance::Hipaa.is_privacy_regulation());
        assert!(!Compliance::PciDss4.is_privacy_regulation());
        assert!(!Compliance::Soc2TypeIi.is_privacy_regulation());
        assert!(!Compliance::Wcag21Aa.is_privacy_regulation());
    }

    #[test]
    fn serde_roundtrip_canonical() {
        for c in Compliance::all() {
            let json = serde_json::to_string(c).expect("serialize");
            assert!(json.contains(c.slug()), "{c:?} serialized as {json}");
            let back: Compliance = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, *c);
        }
    }

    #[test]
    fn serde_specific_slugs() {
        assert_eq!(
            serde_json::to_string(&Compliance::PciDss4).unwrap(),
            "\"pci-dss-4\""
        );
        assert_eq!(
            serde_json::to_string(&Compliance::Wcag22Aaa).unwrap(),
            "\"wcag-2.2-aaa\""
        );
        assert_eq!(
            serde_json::to_string(&Compliance::NoneApplicable).unwrap(),
            "\"none-applicable\""
        );
    }
}
