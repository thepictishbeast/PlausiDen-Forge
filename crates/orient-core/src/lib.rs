//! `orient-core` — typed projection of the 12 PlausiDen N-orientations.
//!
//! Per `[[n-orientation-substrate]]` doctrine + AVP-Doctrine
//! `N_ORIENTATION_SUBSTRATE.md`: every substrate entity is
//! simultaneously classifiable along 12 orthogonal axes — object /
//! objective / outcome / audience / domain / lifecycle / compliance
//! / risk / resource / accessibility / sovereignty / temporal. This
//! crate implements the closed enums + serde derivation that lets
//! the manifest carry orientation declarations for every Loom
//! primitive, CMS section, Forge phase, etc.
//!
//! ## Implementation order
//!
//! Per task #190 (this crate's initial scope): **Objective** is the
//! highest-leverage non-object axis — it answers "what is this
//! entity for?" and drives objective→primitive mapping tables. The
//! Objective enum is fully populated here. Remaining orientations
//! land in subsequent tasks:
//!
//! - `#191` [orient-v3]  Compliance (GDPR / CCPA / HIPAA / PCI / SOC2 / WCAG / DORA)
//! - `#192` [orient-v4]  Sovereignty (PSA — privacy/security/anonymity differentiator)
//! - `#193` [orient-v5]  Risk (AVP-2 tier ladder)
//! - `#194` [orient-v6]  Resource (per-tier budgets + cost attribution)
//! - `#195` [orient-v7]  Audience + Domain + Lifecycle + Accessibility + Temporal (batched)
//! - `#196` [orient-v8]  Cross-orientation query language + manifest projection
//! - `#197` [orient-v9]  Mapping table curation workflow (closed)
//!
//! Each follow-on task fills its module with the canonical enum
//! values from `N_ORIENTATION_SUBSTRATE.md`. Until those land, the
//! corresponding module is a marker — the Orientation enum's value
//! arm is `#[non_exhaustive]` so adding the populated variant later
//! is an additive (Cat 2) change per `[[backward-compat-version-
//! discipline]]`.
//!
//! ## Cross-AI parity
//!
//! All orientation values are stable string slugs (snake_case in
//! serde). Claude, Gemini, and other agents read the same
//! `objective: "enable_payment"` string. No agent-specific
//! extensions.
//!
//! Closes task #190 (orient-v2) for the Objective axis;
//! provides the crate scaffold for tasks #191-#195.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

pub mod objective;

pub use objective::Objective;

// Marker modules for the remaining 10 axes (object is implicit in
// Rust type identity; the substrate doesn't need a separate enum).
// Each gets fleshed out in its own follow-on task.

/// Placeholder for the Outcome orientation (`#195`).
///
/// Outcomes describe what an entity *causes* / aims to cause —
/// measurable effects like `page_lcp_under_2.5s`,
/// `audit_chain_verified`, `no_phantom_buttons`. Distinct from
/// Objective: the objective is the *goal*; the outcome is the
/// *measurable consequence*.
pub mod outcome {}

/// Placeholder for the Audience orientation (`#195`).
///
/// Audiences enumerate the consumers an entity serves —
/// `end_user`, `operator`, `partner_developer`, `regulator`,
/// `ai_agent`, `legal_archive`, `internal_engineer`. Multi-valued.
pub mod audience {}

/// Placeholder for the Domain orientation (`#195`).
///
/// Domains enumerate the verticals an entity binds to —
/// `healthcare`, `finance`, `hospitality`, `voting`, `education`,
/// `ecommerce`, `legal`, `journalism`, `philanthropy`,
/// `ai_research`, `agnostic` (substrate default).
pub mod domain {}

/// Placeholder for the Lifecycle orientation (`#195`).
///
/// Lifecycle states an entity's evolution stage —
/// `experimental` (trial; advisory enforcement), `beta` (functional
/// but unstable), `stable` (binding), `deprecated` (sunset
/// scheduled), `retired` (removed). Parallels doctrine rule
/// lifecycle.
pub mod lifecycle {}

/// Placeholder for the Compliance orientation (`#191`).
///
/// Compliance enumerates the regulatory regimes an entity must
/// conform to — `gdpr`, `ccpa`, `hipaa`, `pci-dss-4`,
/// `soc2-type-ii`, `iso-27001`, `iso-25010`, `iso-40500`,
/// `wcag-2.1-aa`, `wcag-2.2-aaa`, `dora`, `cra`.
pub mod compliance {}

/// Placeholder for the Risk orientation (`#193`).
///
/// Risk encodes the AVP-2 tier ladder — `tier-1-trivial` through
/// `tier-10-economic`. Per `[[avp2-tiers]]`: every substrate
/// entity declares its risk tier; substrate refuses to ship below
/// the minimum tier for its capability class.
pub mod risk {}

/// Placeholder for the Resource orientation (`#194`).
///
/// Resource enumerates cost/budget envelopes — `cpu-cheap`,
/// `cpu-bounded`, `cpu-expensive`, `memory-bounded`,
/// `memory-streaming`, `network-frugal`, `network-bursty`,
/// `carbon-budgeted`, `disk-frugal`, `disk-archival`. Multi-valued.
pub mod resource {}

/// Placeholder for the Accessibility orientation (`#195`).
///
/// Accessibility defines a11y target levels — `wcag-2.1-a`,
/// `wcag-2.1-aa` (substrate default), `wcag-2.1-aaa`, `wcag-2.2-aa`,
/// `wcag-2.2-aaa` — plus capability multi-values
/// (`screen-reader-first`, `keyboard-first`, etc.).
pub mod accessibility {}

/// Placeholder for the Sovereignty orientation (`#192`).
///
/// Sovereignty encodes the PSA differentiator (privacy / security
/// / anonymity) — `anonymous`, `pseudonymous`, `identified`,
/// `private`, `local-only`, `ephemeral`, `tor-compatible`,
/// `offline-capable`, `pq-secure`, `cleartext-forbidden`,
/// `zero-knowledge`. The PlausiDen differentiator.
pub mod sovereignty {}

/// Placeholder for the Temporal orientation (`#195`).
///
/// Temporal enumerates time-binding behaviors — `session-scoped`,
/// `request-scoped`, `build-scoped`, `daily-recurring`,
/// `monthly-recurring`, `archival`, `ephemeral`,
/// `versioned-immutable`, `monotonic`, `bounded-history`.
pub mod temporal {}

/// The full 12-orientation projection an entity may declare.
///
/// Each field is `Option<T>` because individual orientations may
/// not apply to every entity class (e.g., a Forge phase has no
/// `audience: end_user`). However, declaration is still positive —
/// `None` means "not declared," NOT "implicitly allowed." Audit
/// phases catch missing-but-required declarations per the
/// entity's class table (see `N_ORIENTATION_SUBSTRATE.md`
/// "Default-required traits per entity class").
///
/// Today only `objective` is fully typed. Other fields accept
/// strings for forward compatibility; they get typed enums as
/// `#191`-`#195` land.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrientationProjection {
    /// Object identity (Rust type / module slug).
    pub object: Option<String>,
    /// What the entity is for. See [`Objective`].
    pub objective: Option<Objective>,
    /// Measurable outcomes the entity causes / aims to cause.
    /// Multi-valued. Typed in task `#195`.
    pub outcome: Option<Vec<String>>,
    /// Consumers the entity serves. Multi-valued. Typed in task `#195`.
    pub audience: Option<Vec<String>>,
    /// Verticals the entity binds to. Multi-valued. Typed in task `#195`.
    pub domain: Option<Vec<String>>,
    /// Entity's evolution stage. Typed in task `#195`.
    pub lifecycle: Option<String>,
    /// Regulatory regimes the entity must conform to. Multi-valued.
    /// Typed in task `#191`.
    pub compliance: Option<Vec<String>>,
    /// AVP-2 risk tier. Typed in task `#193`.
    pub risk: Option<String>,
    /// Cost/budget envelopes. Multi-valued. Typed in task `#194`.
    pub resource: Option<Vec<String>>,
    /// A11y posture. Typed in task `#195`.
    pub accessibility: Option<String>,
    /// PSA posture. Multi-valued. Typed in task `#192`.
    pub sovereignty: Option<Vec<String>>,
    /// Time-binding behaviors. Multi-valued. Typed in task `#195`.
    pub temporal: Option<Vec<String>>,
}

impl OrientationProjection {
    /// True if at least one orientation has been declared.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.object.is_none()
            && self.objective.is_none()
            && self.outcome.is_none()
            && self.audience.is_none()
            && self.domain.is_none()
            && self.lifecycle.is_none()
            && self.compliance.is_none()
            && self.risk.is_none()
            && self.resource.is_none()
            && self.accessibility.is_none()
            && self.sovereignty.is_none()
            && self.temporal.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_default_is_empty() {
        let p = OrientationProjection::default();
        assert!(p.is_empty());
    }

    #[test]
    fn projection_with_objective_is_not_empty() {
        let p = OrientationProjection {
            objective: Some(Objective::EnablePayment),
            ..Default::default()
        };
        assert!(!p.is_empty());
    }

    #[test]
    fn projection_serializes_minimal_subset() {
        let p = OrientationProjection {
            objective: Some(Objective::DisplayContent),
            ..Default::default()
        };
        let json = serde_json::to_string(&p).expect("serialize");
        // Should contain just the objective field; None values are
        // skipped automatically by serde.
        assert!(json.contains("\"objective\":\"display_content\""));
    }

    #[test]
    fn projection_deserializes_minimal() {
        let json = r#"{"objective":"enable_payment"}"#;
        let p: OrientationProjection = serde_json::from_str(json).expect("deserialize");
        assert_eq!(p.objective, Some(Objective::EnablePayment));
        assert!(p.object.is_none());
    }

    #[test]
    fn projection_deserializes_full() {
        let json = r#"{
            "object": "Loom.Primitive.Hero",
            "objective": "display_content",
            "outcome": ["page_lcp_under_2.5s", "no_contrast_violations"],
            "audience": ["end_user"],
            "domain": ["agnostic"],
            "lifecycle": "stable",
            "compliance": ["wcag-2.1-aa"],
            "risk": "tier-4-property",
            "resource": ["cpu-cheap", "network-frugal"],
            "accessibility": "wcag-2.1-aa",
            "sovereignty": ["private"],
            "temporal": ["build-scoped"]
        }"#;
        let p: OrientationProjection = serde_json::from_str(json).expect("deserialize");
        assert_eq!(p.object.as_deref(), Some("Loom.Primitive.Hero"));
        assert_eq!(p.objective, Some(Objective::DisplayContent));
        assert_eq!(p.lifecycle.as_deref(), Some("stable"));
    }

    #[test]
    fn projection_rejects_unknown_fields() {
        // deny_unknown_fields means typos fail at parse — additive
        // changes must go through VERSION_DISCIPLINE.md.
        let json = r#"{"objective":"display_content","unknown_axis":"x"}"#;
        let r: Result<OrientationProjection, _> = serde_json::from_str(json);
        assert!(r.is_err());
    }
}
