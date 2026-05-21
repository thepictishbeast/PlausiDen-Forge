//! `orient-core` — typed projection of the 12 substrate N-orientations.
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

pub mod accessibility;
pub mod audience;
pub mod compliance;
pub mod domain;
pub mod lifecycle;
pub mod objective;
pub mod query;
pub mod resource;
pub mod risk;
pub mod sovereignty;
pub mod temporal;

pub use accessibility::Accessibility;
pub use audience::Audience;
pub use compliance::Compliance;
pub use domain::Domain;
pub use lifecycle::Lifecycle;
pub use objective::Objective;
pub use query::OrientationQuery;
pub use resource::Resource;
pub use risk::Risk;
pub use sovereignty::Sovereignty;
pub use temporal::Temporal;

// Marker modules for the remaining 10 axes (object is implicit in
// Rust type identity; the substrate doesn't need a separate enum).
// Each gets fleshed out in its own follow-on task.

/// Placeholder for the Outcome orientation.
///
/// Outcomes describe what an entity *causes* / aims to cause —
/// measurable effects like `page_lcp_under_2.5s`,
/// `audit_chain_verified`, `no_phantom_buttons`. Distinct from
/// Objective: the objective is the *goal*; the outcome is the
/// *measurable consequence*.
///
/// Stringly-typed today — outcome values are dynamic per-entity
/// (every Forge phase declares its own outcomes). Future typing
/// when the canonical outcome enum stabilizes.
pub mod outcome {}

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
    /// Consumers the entity serves. Multi-valued. Typed enum per
    /// [`Audience`] (task #195).
    pub audience: Option<Vec<Audience>>,
    /// Verticals the entity binds to. Multi-valued. Typed enum per
    /// [`Domain`] (task #195).
    pub domain: Option<Vec<Domain>>,
    /// Entity's evolution stage. Typed enum per [`Lifecycle`]
    /// (task #195).
    pub lifecycle: Option<Lifecycle>,
    /// Regulatory regimes the entity must conform to. Multi-valued.
    /// Typed enum per [`Compliance`] (task #191).
    pub compliance: Option<Vec<Compliance>>,
    /// AVP-2 risk tier. Typed enum per [`Risk`] (task #193).
    pub risk: Option<Risk>,
    /// Cost/budget envelopes. Multi-valued. Typed enum per
    /// [`Resource`] (task #194).
    pub resource: Option<Vec<Resource>>,
    /// A11y posture. Typed enum per [`Accessibility`] (task #195).
    pub accessibility: Option<Accessibility>,
    /// PSA posture. Multi-valued. Typed enum per [`Sovereignty`]
    /// (task #192).
    pub sovereignty: Option<Vec<Sovereignty>>,
    /// Time-binding behaviors. Multi-valued. Typed enum per
    /// [`Temporal`] (task #195).
    pub temporal: Option<Vec<Temporal>>,
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
        assert_eq!(p.lifecycle, Some(Lifecycle::Stable));
        // Typed orientations from prior iteration.
        assert_eq!(p.compliance, Some(vec![Compliance::Wcag21Aa]));
        assert_eq!(p.risk, Some(Risk::Tier4Property));
        assert_eq!(p.sovereignty, Some(vec![Sovereignty::Private]));
        // Newly-typed orientations from this iteration (#194/#195).
        assert_eq!(p.audience, Some(vec![Audience::EndUser]));
        assert_eq!(p.domain, Some(vec![Domain::Agnostic]));
        assert_eq!(
            p.resource,
            Some(vec![Resource::CpuCheap, Resource::NetworkFrugal])
        );
        assert_eq!(p.accessibility, Some(Accessibility::Wcag21Aa));
        assert_eq!(p.temporal, Some(vec![Temporal::BuildScoped]));
    }

    #[test]
    fn projection_with_all_twelve_orientations_roundtrips() {
        // Demonstrate that an entity can carry typed values on
        // every orientation simultaneously.
        let p = OrientationProjection {
            object: Some("Loom.Primitive.PaymentForm".into()),
            objective: Some(Objective::EnablePayment),
            outcome: Some(vec!["user_completes_signup".into()]),
            audience: Some(vec![Audience::EndUser]),
            domain: Some(vec![Domain::Ecommerce, Domain::Finance]),
            lifecycle: Some(Lifecycle::Stable),
            compliance: Some(vec![Compliance::PciDss4, Compliance::Gdpr]),
            risk: Some(Risk::Tier6Mutation),
            resource: Some(vec![Resource::CpuBounded, Resource::NetworkFrugal]),
            accessibility: Some(Accessibility::Wcag22Aa),
            sovereignty: Some(vec![Sovereignty::CleartextForbidden, Sovereignty::Private]),
            temporal: Some(vec![Temporal::SessionScoped]),
        };
        let json = serde_json::to_string(&p).expect("serialize");
        let back: OrientationProjection = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.objective, Some(Objective::EnablePayment));
        assert_eq!(back.lifecycle, Some(Lifecycle::Stable));
        assert_eq!(back.risk, Some(Risk::Tier6Mutation));
        assert_eq!(back.accessibility, Some(Accessibility::Wcag22Aa));
        assert_eq!(back.domain, Some(vec![Domain::Ecommerce, Domain::Finance]));
    }

    #[test]
    fn projection_with_multiple_typed_orientations() {
        // A payment form: pci-dss-4 + cleartext-forbidden +
        // tier-6-mutation minimum.
        let p = OrientationProjection {
            objective: Some(Objective::EnablePayment),
            compliance: Some(vec![Compliance::PciDss4, Compliance::Gdpr]),
            risk: Some(Risk::Tier6Mutation),
            sovereignty: Some(vec![Sovereignty::CleartextForbidden, Sovereignty::Private]),
            ..Default::default()
        };
        let json = serde_json::to_string(&p).expect("serialize");
        let back: OrientationProjection = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.objective, Some(Objective::EnablePayment));
        assert_eq!(
            back.compliance,
            Some(vec![Compliance::PciDss4, Compliance::Gdpr])
        );
        assert_eq!(back.risk, Some(Risk::Tier6Mutation));
    }

    #[test]
    fn projection_rejects_unknown_compliance_value() {
        let json = r#"{"compliance":["made-up-regime"]}"#;
        let r: Result<OrientationProjection, _> = serde_json::from_str(json);
        assert!(r.is_err());
    }

    #[test]
    fn projection_rejects_unknown_sovereignty_value() {
        let json = r#"{"sovereignty":["super-anonymous"]}"#;
        let r: Result<OrientationProjection, _> = serde_json::from_str(json);
        assert!(r.is_err());
    }

    #[test]
    fn projection_rejects_unknown_risk_tier() {
        let json = r#"{"risk":"tier-11-superhuman"}"#;
        let r: Result<OrientationProjection, _> = serde_json::from_str(json);
        assert!(r.is_err());
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
