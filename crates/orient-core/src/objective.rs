//! Objective orientation — what an entity is *for*.
//!
//! Per AVP-Doctrine `N_ORIENTATION_SUBSTRATE.md` § Orientation 2:
//! "What the entity is *for*. **Single-valued** in canonical form;
//! aliases allowed."
//!
//! The Objective enum is the highest-leverage non-object orientation
//! axis — it drives objective→primitive mapping tables, lets
//! operators query "which primitive achieves objective X?", and
//! lets Forge phases query by objective ("every entity with
//! objective=`enable_payment` must pass tier-6 risk gate").
//!
//! Per `[[backward-compat-version-discipline]]`: enum is
//! `#[non_exhaustive]` so adding a new objective is an additive
//! (Cat 2) change. Renames are Cat 3 (auto-migration). Removals
//! are Cat 4 (operator-action).
//!
//! Per `[[deterministic-first-lfi-optional]]`: discrete enum, no
//! AI-inferred categories. LFI may *suggest* objective tags during
//! authoring but never *decide* them.

use serde::{Deserialize, Serialize};

/// What an entity is *for*. Canonical closed enum (extensible via
/// doctrine + capability-request).
///
/// Values are snake_case via serde rename_all. Stable across
/// versions per backward-compat doctrine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Objective {
    /// Reduce friction in the user's signup / onboarding path.
    /// Drives auth flows, payment flows, confirmation steps.
    ReduceSignupFriction,

    /// Enable a payment to be initiated, processed, or completed.
    /// Triggers `risk >= tier-6-mutation` per
    /// objective-to-risk mapping table.
    EnablePayment,

    /// Collect explicit consent from the user with a legal basis
    /// declaration. Triggers `compliance >= gdpr` per
    /// objective-to-compliance mapping.
    CollectConsentWithLegalBasis,

    /// Communicate trust / legitimacy / reassurance.
    /// Typical for hero / testimonials / press / accreditations.
    CommunicateTrust,

    /// Surface an alarm / alert / urgent signal to the operator
    /// or end-user. Triggers `accessibility >= wcag-2.1-aa`
    /// minimum + `temporal: ephemeral` for many alarm classes.
    SurfaceAlarm,

    /// Prove provenance of an artifact / event / claim. Drives
    /// audit-chain entries, signature emissions, attestation
    /// flows.
    ProveProvenance,

    /// Enforce an access-control gate. Drives auth middleware,
    /// permission checks, role-based UI gating.
    EnforceAccessControl,

    /// Enable the user to navigate to a different surface.
    /// Typical for nav, breadcrumbs, search.
    EnableNavigation,

    /// Display content to the user. Typical for hero, paragraph,
    /// pull_quote, code, image. The most common objective.
    DisplayContent,

    /// Capture a metric / signal for observability or analytics.
    /// Drives instrumentation, telemetry emissions.
    CaptureMetric,

    /// Produce an audit artifact (report, signed bundle,
    /// compliance export). Triggers attestation phases.
    AuditArtifact,

    /// Publish doctrine / policy / canonical reference for
    /// downstream consumers. Typical for doctrine pages, ADRs,
    /// schema docs.
    PublishDoctrine,

    /// Schedule a recurring action / cron / temporal trigger.
    /// Drives the cron-loop infrastructure + scheduler primitives.
    ScheduleRecurringAction,

    /// Detect a regression — broken text, broken contrast,
    /// broken link, broken layout. Drives Crawler detectors +
    /// audit phases.
    DetectRegression,

    /// Validate typed input at a boundary (form submission, API
    /// call, CMS content parse). Drives form-input primitives +
    /// typed-config gates.
    ValidateTypedInput,

    /// Render typed output (HTML, JSON, RSS, AMP). Drives the
    /// render pipeline + exporters.
    RenderTypedOutput,

    /// Configure the substrate (toggles, feature flags, tenant
    /// settings). Typically operator-audience.
    ConfigureSubstrate,

    /// Declare a capability (manifest entry, MCP tool, schema
    /// extension). Drives capability-request flows + manifest
    /// projection.
    DeclareCapability,

    /// Encode an epistemic belief, claim, or assertion that the
    /// system needs to reason about. Drives LFI knowledge-base
    /// entries (when augmentation enabled per
    /// `[[deterministic-first-lfi-optional]]`).
    EncodeBelief,

    /// Request human review of a decision the substrate cannot
    /// make automatically. Drives review-queue + escalation flows.
    RequestHumanReview,
}

impl Objective {
    /// Return the canonical snake_case slug. Stable across
    /// versions; consumers may match on this string.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::ReduceSignupFriction => "reduce_signup_friction",
            Self::EnablePayment => "enable_payment",
            Self::CollectConsentWithLegalBasis => "collect_consent_with_legal_basis",
            Self::CommunicateTrust => "communicate_trust",
            Self::SurfaceAlarm => "surface_alarm",
            Self::ProveProvenance => "prove_provenance",
            Self::EnforceAccessControl => "enforce_access_control",
            Self::EnableNavigation => "enable_navigation",
            Self::DisplayContent => "display_content",
            Self::CaptureMetric => "capture_metric",
            Self::AuditArtifact => "audit_artifact",
            Self::PublishDoctrine => "publish_doctrine",
            Self::ScheduleRecurringAction => "schedule_recurring_action",
            Self::DetectRegression => "detect_regression",
            Self::ValidateTypedInput => "validate_typed_input",
            Self::RenderTypedOutput => "render_typed_output",
            Self::ConfigureSubstrate => "configure_substrate",
            Self::DeclareCapability => "declare_capability",
            Self::EncodeBelief => "encode_belief",
            Self::RequestHumanReview => "request_human_review",
        }
    }

    /// All canonical Objective values in stable iteration order.
    /// Used by `forge orient --objectives` to enumerate the surface
    /// without instantiating each variant.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::ReduceSignupFriction,
            Self::EnablePayment,
            Self::CollectConsentWithLegalBasis,
            Self::CommunicateTrust,
            Self::SurfaceAlarm,
            Self::ProveProvenance,
            Self::EnforceAccessControl,
            Self::EnableNavigation,
            Self::DisplayContent,
            Self::CaptureMetric,
            Self::AuditArtifact,
            Self::PublishDoctrine,
            Self::ScheduleRecurringAction,
            Self::DetectRegression,
            Self::ValidateTypedInput,
            Self::RenderTypedOutput,
            Self::ConfigureSubstrate,
            Self::DeclareCapability,
            Self::EncodeBelief,
            Self::RequestHumanReview,
        ]
    }

    /// Parse an Objective from its canonical snake_case slug.
    /// Returns `None` for unknown / mistyped slugs — callers
    /// fail-closed per `[[deterministic-first-lfi-optional]]`.
    #[must_use]
    pub fn from_slug(s: &str) -> Option<Self> {
        Self::all().iter().copied().find(|o| o.slug() == s)
    }
}

impl std::fmt::Display for Objective {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_snake_case() {
        for o in Objective::all() {
            let slug = o.slug();
            assert!(!slug.is_empty(), "{o:?} has empty slug");
            assert!(
                slug.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "{o:?} slug {slug:?} is not snake_case"
            );
        }
    }

    #[test]
    fn all_objectives_unique_slugs() {
        let mut seen = std::collections::HashSet::new();
        for o in Objective::all() {
            assert!(seen.insert(o.slug()), "duplicate slug: {:?}", o.slug());
        }
        assert_eq!(seen.len(), 20, "expected 20 canonical objectives");
    }

    #[test]
    fn from_slug_roundtrip() {
        for o in Objective::all() {
            let s = o.slug();
            let back = Objective::from_slug(s).expect("known slug");
            assert_eq!(back, *o, "slug {s} did not roundtrip");
        }
    }

    #[test]
    fn from_slug_rejects_unknown() {
        assert!(Objective::from_slug("").is_none());
        assert!(Objective::from_slug("Garbage").is_none());
        assert!(Objective::from_slug("display content").is_none()); // space, not underscore
        assert!(Objective::from_slug("DISPLAY_CONTENT").is_none()); // uppercase rejected
    }

    #[test]
    fn serde_roundtrip_canonical() {
        for o in Objective::all() {
            let json = serde_json::to_string(o).expect("serialize");
            let back: Objective = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, *o);
        }
    }

    #[test]
    fn serde_uses_snake_case_slug() {
        let json = serde_json::to_string(&Objective::EnablePayment).expect("serialize");
        assert_eq!(json, "\"enable_payment\"");

        let json =
            serde_json::to_string(&Objective::CollectConsentWithLegalBasis).expect("serialize");
        assert_eq!(json, "\"collect_consent_with_legal_basis\"");
    }

    #[test]
    fn display_matches_slug() {
        assert_eq!(format!("{}", Objective::DisplayContent), "display_content");
    }

    #[test]
    fn deserialize_rejects_unknown_variant() {
        let r: Result<Objective, _> = serde_json::from_str("\"made_up_objective\"");
        assert!(r.is_err());
    }
}
