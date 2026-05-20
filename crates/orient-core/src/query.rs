//! Cross-orientation query language for [`OrientationProjection`].
//!
//! Per AVP-Doctrine `N_ORIENTATION_SUBSTRATE.md` § Cross-orientation
//! query language: substrate entities are simultaneously classifiable
//! along 12 orthogonal axes, and the typed query API lets callers
//! express *"every entity matching this combination of axes"*
//! without ad-hoc string concatenation or per-axis filter chains.
//!
//! Examples:
//! ```text
//! # Every payment entity at risk-6 or higher
//! OrientationQuery::default()
//!     .with_objective(Objective::EnablePayment)
//!     .with_risk_at_least(Risk::Tier6Mutation)
//!
//! # Every healthcare entity that touches PHI in the EU
//! OrientationQuery::default()
//!     .with_domain(Domain::Healthcare)
//!     .with_compliance(Compliance::Gdpr)
//!     .with_sovereignty(Sovereignty::Private)
//!
//! # Every Tor-compatible anonymous publishing surface
//! OrientationQuery::default()
//!     .with_sovereignty(Sovereignty::TorCompatible)
//!     .with_sovereignty(Sovereignty::Anonymous)
//! ```
//!
//! Each filter is conjunctive (AND). Multi-valued filters (compliance,
//! sovereignty, audience, domain, resource, temporal) match when the
//! projection contains every requested value (subset semantics).
//! Single-valued filters match on equality (or `>=` for risk +
//! accessibility, which carry ordered semantics).
//!
//! Per `[[deterministic-first-lfi-optional]]`: pure-data
//! predicate evaluation. No AI involvement. Same projection → same
//! query result deterministically.
//!
//! Per `[[priority-architectural-first-and-cross-ai]]`: queries are
//! serializable JSON so MCP tools / Claude / Gemini / other agents
//! can construct queries identically.
//!
//! Closes `#196 [orient-v8]`.

use serde::{Deserialize, Serialize};

use crate::{
    Accessibility, Audience, Compliance, Domain, Lifecycle, Objective, OrientationProjection,
    Resource, Risk, Sovereignty, Temporal,
};

/// Typed cross-orientation query. Each field is optional; an unset
/// field places no constraint on that axis. Construct via
/// `OrientationQuery::default()` + chained `.with_*()` builders.
///
/// Multi-valued filter fields (e.g. `compliance: Vec<Compliance>`)
/// use subset semantics: a projection matches when its declared
/// values contain ALL the query's requested values.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrientationQuery {
    /// Optional object-identity filter. Substring match against
    /// the projection's `object` slug. Useful for type-class
    /// queries like `Loom.Primitive.*`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_substring: Option<String>,

    /// Filter on Objective (single-valued equality).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective: Option<Objective>,

    /// Filter on Audience values (subset semantics — every value
    /// must be declared by the projection).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audience: Vec<Audience>,

    /// Filter on Domain values (subset semantics).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub domain: Vec<Domain>,

    /// Filter on Lifecycle (single-valued equality).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<Lifecycle>,

    /// Filter on Compliance values (subset semantics).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compliance: Vec<Compliance>,

    /// Filter on Risk — exact equality.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk: Option<Risk>,

    /// Filter on Risk — minimum tier (projection's risk >= this).
    /// Used by mapping-table-driven gates like
    /// "every payment entity at risk-6 or higher."
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_at_least: Option<Risk>,

    /// Filter on Resource values (subset semantics).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resource: Vec<Resource>,

    /// Filter on Accessibility — exact equality.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessibility: Option<Accessibility>,

    /// Filter on Accessibility — minimum level (projection's
    /// accessibility >= this). Used by a11y-floor enforcement
    /// (e.g. `>= Wcag21Aa` is the substrate floor).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessibility_at_least: Option<Accessibility>,

    /// Filter on Sovereignty values (subset semantics).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sovereignty: Vec<Sovereignty>,

    /// Filter on Temporal values (subset semantics).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub temporal: Vec<Temporal>,
}

impl OrientationQuery {
    /// True if the projection satisfies every constraint in this
    /// query (conjunctive AND across axes; subset semantics for
    /// multi-valued filters). Empty query matches every projection.
    #[must_use]
    pub fn matches(&self, p: &OrientationProjection) -> bool {
        // Object substring.
        if let Some(needle) = &self.object_substring {
            match &p.object {
                Some(obj) => {
                    if !obj.contains(needle.as_str()) {
                        return false;
                    }
                }
                None => return false,
            }
        }

        // Objective.
        if let Some(want) = self.objective {
            if p.objective != Some(want) {
                return false;
            }
        }

        // Audience subset.
        if !self.audience.is_empty() {
            let Some(have) = &p.audience else {
                return false;
            };
            if !self.audience.iter().all(|w| have.contains(w)) {
                return false;
            }
        }

        // Domain subset.
        if !self.domain.is_empty() {
            let Some(have) = &p.domain else { return false };
            if !self.domain.iter().all(|w| have.contains(w)) {
                return false;
            }
        }

        // Lifecycle.
        if let Some(want) = self.lifecycle {
            if p.lifecycle != Some(want) {
                return false;
            }
        }

        // Compliance subset.
        if !self.compliance.is_empty() {
            let Some(have) = &p.compliance else {
                return false;
            };
            if !self.compliance.iter().all(|w| have.contains(w)) {
                return false;
            }
        }

        // Risk exact.
        if let Some(want) = self.risk {
            if p.risk != Some(want) {
                return false;
            }
        }

        // Risk >= floor.
        if let Some(floor) = self.risk_at_least {
            match p.risk {
                Some(have) if have >= floor => {}
                _ => return false,
            }
        }

        // Resource subset.
        if !self.resource.is_empty() {
            let Some(have) = &p.resource else {
                return false;
            };
            if !self.resource.iter().all(|w| have.contains(w)) {
                return false;
            }
        }

        // Accessibility exact.
        if let Some(want) = self.accessibility {
            if p.accessibility != Some(want) {
                return false;
            }
        }

        // Accessibility >= floor.
        if let Some(floor) = self.accessibility_at_least {
            match p.accessibility {
                Some(have) if have >= floor => {}
                _ => return false,
            }
        }

        // Sovereignty subset.
        if !self.sovereignty.is_empty() {
            let Some(have) = &p.sovereignty else {
                return false;
            };
            if !self.sovereignty.iter().all(|w| have.contains(w)) {
                return false;
            }
        }

        // Temporal subset.
        if !self.temporal.is_empty() {
            let Some(have) = &p.temporal else {
                return false;
            };
            if !self.temporal.iter().all(|w| have.contains(w)) {
                return false;
            }
        }

        true
    }

    /// Builder: filter on object substring.
    #[must_use]
    pub fn with_object_substring(mut self, needle: impl Into<String>) -> Self {
        self.object_substring = Some(needle.into());
        self
    }

    /// Builder: filter on Objective.
    #[must_use]
    pub fn with_objective(mut self, v: Objective) -> Self {
        self.objective = Some(v);
        self
    }

    /// Builder: require an additional Audience value (subset semantics).
    #[must_use]
    pub fn with_audience(mut self, v: Audience) -> Self {
        self.audience.push(v);
        self
    }

    /// Builder: require an additional Domain value.
    #[must_use]
    pub fn with_domain(mut self, v: Domain) -> Self {
        self.domain.push(v);
        self
    }

    /// Builder: filter on Lifecycle.
    #[must_use]
    pub fn with_lifecycle(mut self, v: Lifecycle) -> Self {
        self.lifecycle = Some(v);
        self
    }

    /// Builder: require an additional Compliance value.
    #[must_use]
    pub fn with_compliance(mut self, v: Compliance) -> Self {
        self.compliance.push(v);
        self
    }

    /// Builder: filter on exact Risk tier.
    #[must_use]
    pub fn with_risk(mut self, v: Risk) -> Self {
        self.risk = Some(v);
        self
    }

    /// Builder: filter on minimum Risk tier (projection's risk >= v).
    #[must_use]
    pub fn with_risk_at_least(mut self, v: Risk) -> Self {
        self.risk_at_least = Some(v);
        self
    }

    /// Builder: require an additional Resource value.
    #[must_use]
    pub fn with_resource(mut self, v: Resource) -> Self {
        self.resource.push(v);
        self
    }

    /// Builder: filter on exact Accessibility level.
    #[must_use]
    pub fn with_accessibility(mut self, v: Accessibility) -> Self {
        self.accessibility = Some(v);
        self
    }

    /// Builder: filter on minimum Accessibility level.
    #[must_use]
    pub fn with_accessibility_at_least(mut self, v: Accessibility) -> Self {
        self.accessibility_at_least = Some(v);
        self
    }

    /// Builder: require an additional Sovereignty value.
    #[must_use]
    pub fn with_sovereignty(mut self, v: Sovereignty) -> Self {
        self.sovereignty.push(v);
        self
    }

    /// Builder: require an additional Temporal value.
    #[must_use]
    pub fn with_temporal(mut self, v: Temporal) -> Self {
        self.temporal.push(v);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A payment-form projection used as the fixture in most tests.
    fn payment_form_projection() -> OrientationProjection {
        OrientationProjection {
            object: Some("Loom.Primitive.PaymentForm".into()),
            objective: Some(Objective::EnablePayment),
            audience: Some(vec![Audience::EndUser]),
            domain: Some(vec![Domain::Ecommerce, Domain::Finance]),
            lifecycle: Some(Lifecycle::Stable),
            compliance: Some(vec![Compliance::PciDss4, Compliance::Gdpr]),
            risk: Some(Risk::Tier6Mutation),
            resource: Some(vec![Resource::CpuBounded, Resource::NetworkFrugal]),
            accessibility: Some(Accessibility::Wcag22Aa),
            sovereignty: Some(vec![Sovereignty::CleartextForbidden, Sovereignty::Private]),
            temporal: Some(vec![Temporal::SessionScoped]),
            ..Default::default()
        }
    }

    #[test]
    fn empty_query_matches_every_projection() {
        let q = OrientationQuery::default();
        assert!(q.matches(&payment_form_projection()));
        assert!(q.matches(&OrientationProjection::default()));
    }

    #[test]
    fn empty_query_serializes_to_empty_object() {
        let q = OrientationQuery::default();
        let json = serde_json::to_string(&q).expect("serialize");
        assert_eq!(json, "{}");
    }

    #[test]
    fn objective_filter_matches_then_misses() {
        let p = payment_form_projection();
        assert!(OrientationQuery::default()
            .with_objective(Objective::EnablePayment)
            .matches(&p));
        assert!(!OrientationQuery::default()
            .with_objective(Objective::DisplayContent)
            .matches(&p));
    }

    #[test]
    fn risk_at_least_floors_the_tier() {
        let p = payment_form_projection(); // Tier6Mutation
        assert!(OrientationQuery::default()
            .with_risk_at_least(Risk::Tier3Functional)
            .matches(&p));
        assert!(OrientationQuery::default()
            .with_risk_at_least(Risk::Tier6Mutation)
            .matches(&p));
        assert!(!OrientationQuery::default()
            .with_risk_at_least(Risk::Tier7Concurrent)
            .matches(&p));
    }

    #[test]
    fn accessibility_at_least_floors_the_level() {
        let p = payment_form_projection(); // Wcag22Aa
        assert!(OrientationQuery::default()
            .with_accessibility_at_least(Accessibility::Wcag21Aa)
            .matches(&p));
        assert!(OrientationQuery::default()
            .with_accessibility_at_least(Accessibility::Wcag22Aa)
            .matches(&p));
        assert!(!OrientationQuery::default()
            .with_accessibility_at_least(Accessibility::Wcag22Aaa)
            .matches(&p));
    }

    #[test]
    fn compliance_subset_match() {
        let p = payment_form_projection(); // PciDss4 + Gdpr
        assert!(OrientationQuery::default()
            .with_compliance(Compliance::Gdpr)
            .matches(&p));
        assert!(OrientationQuery::default()
            .with_compliance(Compliance::Gdpr)
            .with_compliance(Compliance::PciDss4)
            .matches(&p));
        // CCPA not in projection — must fail.
        assert!(!OrientationQuery::default()
            .with_compliance(Compliance::Ccpa)
            .matches(&p));
    }

    #[test]
    fn sovereignty_subset_match() {
        let p = payment_form_projection(); // CleartextForbidden + Private
        assert!(OrientationQuery::default()
            .with_sovereignty(Sovereignty::Private)
            .matches(&p));
        // Anonymous not in projection.
        assert!(!OrientationQuery::default()
            .with_sovereignty(Sovereignty::Anonymous)
            .matches(&p));
    }

    #[test]
    fn conjunctive_combination() {
        let p = payment_form_projection();
        assert!(OrientationQuery::default()
            .with_objective(Objective::EnablePayment)
            .with_domain(Domain::Ecommerce)
            .with_risk_at_least(Risk::Tier6Mutation)
            .with_compliance(Compliance::PciDss4)
            .matches(&p));

        // Same plus an unmet axis.
        assert!(!OrientationQuery::default()
            .with_objective(Objective::EnablePayment)
            .with_sovereignty(Sovereignty::Anonymous)
            .matches(&p));
    }

    #[test]
    fn object_substring_matches() {
        let p = payment_form_projection();
        assert!(OrientationQuery::default()
            .with_object_substring("Loom.Primitive")
            .matches(&p));
        assert!(OrientationQuery::default()
            .with_object_substring("PaymentForm")
            .matches(&p));
        assert!(!OrientationQuery::default()
            .with_object_substring("Crawler")
            .matches(&p));
    }

    #[test]
    fn missing_axis_in_projection_fails_subset_filter() {
        // Projection with no audience declared can't satisfy an
        // audience filter — fail-closed.
        let p = OrientationProjection {
            objective: Some(Objective::DisplayContent),
            ..Default::default()
        };
        assert!(!OrientationQuery::default()
            .with_audience(Audience::EndUser)
            .matches(&p));
    }

    #[test]
    fn query_serializes_only_populated_fields() {
        let q = OrientationQuery::default()
            .with_objective(Objective::EnablePayment)
            .with_risk_at_least(Risk::Tier6Mutation)
            .with_sovereignty(Sovereignty::CleartextForbidden);
        let json = serde_json::to_string(&q).expect("serialize");
        // Populated fields appear.
        assert!(json.contains("\"objective\":\"enable_payment\""));
        assert!(json.contains("\"risk_at_least\":\"tier-6-mutation\""));
        assert!(json.contains("\"sovereignty\":[\"cleartext_forbidden\"]"));
        // Empty/unset fields don't.
        assert!(!json.contains("\"audience\""));
        assert!(!json.contains("\"object_substring\""));
        assert!(!json.contains("\"lifecycle\""));
    }

    #[test]
    fn query_roundtrips_via_json() {
        let q = OrientationQuery::default()
            .with_objective(Objective::EnablePayment)
            .with_compliance(Compliance::Gdpr)
            .with_compliance(Compliance::Hipaa)
            .with_risk_at_least(Risk::Tier6Mutation);
        let json = serde_json::to_string(&q).expect("serialize");
        let back: OrientationQuery = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.objective, Some(Objective::EnablePayment));
        assert_eq!(back.compliance, vec![Compliance::Gdpr, Compliance::Hipaa]);
        assert_eq!(back.risk_at_least, Some(Risk::Tier6Mutation));
    }

    #[test]
    fn query_rejects_unknown_fields() {
        // Future-proofing typo guard.
        let json = r#"{"objective":"display_content","made_up":"x"}"#;
        let r: Result<OrientationQuery, _> = serde_json::from_str(json);
        assert!(r.is_err());
    }
}
