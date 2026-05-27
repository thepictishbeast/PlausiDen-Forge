//! `generation_plan` — pre-generation plan schema + plan-vs-
//! execution audit.
//!
//! Per task #382. Before generating a site, the operator (or AI
//! agent) writes a PLAN describing intent: PageKind, theme,
//! density, target section count + kinds, prose-char target,
//! image count, animation count. After generation, the substrate
//! audits observed output against the plan and flags divergence
//! — places where execution drifted from intent.
//!
//! ## Why mandatory plans
//!
//! Without a plan, generation is unconstrained. Even with
//! resource budgets (#381), an unplanned generation can stay
//! within budget while producing the wrong shape entirely.
//! Plans express SHAPE intent that budgets can't capture: "I
//! want a brief-kind page with 4 sections", not "I want at most
//! 6 primitives per page".
//!
//! ## Audit semantics
//!
//! Divergence is reported but not necessarily blocked. Some
//! divergence is healthy (operator refined the plan as they
//! went). The substrate's job is to SURFACE the divergence so
//! the operator notices when execution drifted unintentionally.

use serde::{Deserialize, Serialize};

/// Pre-generation plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Plan {
    /// Spec version.
    pub spec_version: u32,
    /// Tenant identifier.
    pub tenant_id: String,
    /// Site identifier.
    pub site_id: String,
    /// Operator who authored the plan.
    pub operator_id: String,
    /// PageKind slug.
    pub page_kind: String,
    /// Theme slug.
    pub theme: String,
    /// Density tier slug.
    pub density: String,
    /// Target total sections per page.
    pub target_section_count: u32,
    /// Target section kinds (slugs). Order matters; serves as
    /// the intended page-flow shape.
    pub target_section_kinds: Vec<String>,
    /// Target total prose characters per page.
    pub target_prose_chars: u32,
    /// Target image count per page.
    pub target_image_count: u32,
    /// Target animation section count per page.
    pub target_animation_count: u32,
    /// Operator notes / rationale.
    #[serde(default)]
    pub notes: Option<String>,
}

/// One observed divergence between plan and execution.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct Divergence {
    /// Which field diverged.
    pub field: &'static str,
    /// What the plan declared.
    pub planned: String,
    /// What was observed in execution.
    pub observed: String,
    /// Magnitude of divergence (0 = none; higher = larger).
    pub magnitude: u32,
    /// Severity bucket.
    pub severity: DivergenceSeverity,
}

/// Divergence severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DivergenceSeverity {
    /// Within ~10% — likely intentional refinement.
    Minor,
    /// 10-30% — operator should review.
    Moderate,
    /// 30%+ — drift; flag for explicit re-plan.
    Major,
    /// Field-level mismatch (theme / page_kind / density).
    Discrepancy,
}

/// Observed execution snapshot for plan-vs-execution audit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExecutionSnapshot {
    /// Observed page_kind in built tenant.
    pub page_kind: String,
    /// Observed theme.
    pub theme: String,
    /// Observed density.
    pub density: String,
    /// Observed section count.
    pub section_count: u32,
    /// Observed section kinds (order preserved).
    pub section_kinds: Vec<String>,
    /// Observed prose char count.
    pub prose_chars: u32,
    /// Observed image count.
    pub image_count: u32,
    /// Observed animation count.
    pub animation_count: u32,
}

/// Compare plan to execution; return every divergence found.
#[must_use]
pub fn audit_execution(plan: &Plan, observed: &ExecutionSnapshot) -> Vec<Divergence> {
    let mut out = Vec::new();

    // Field-level discrepancies (categorical fields).
    if plan.page_kind != observed.page_kind {
        out.push(Divergence {
            field: "page_kind",
            planned: plan.page_kind.clone(),
            observed: observed.page_kind.clone(),
            magnitude: 100,
            severity: DivergenceSeverity::Discrepancy,
        });
    }
    if plan.theme != observed.theme {
        out.push(Divergence {
            field: "theme",
            planned: plan.theme.clone(),
            observed: observed.theme.clone(),
            magnitude: 100,
            severity: DivergenceSeverity::Discrepancy,
        });
    }
    if plan.density != observed.density {
        out.push(Divergence {
            field: "density",
            planned: plan.density.clone(),
            observed: observed.density.clone(),
            magnitude: 100,
            severity: DivergenceSeverity::Discrepancy,
        });
    }

    // Numeric-field magnitude divergence.
    add_numeric_divergence(
        &mut out,
        "target_section_count",
        plan.target_section_count,
        observed.section_count,
    );
    add_numeric_divergence(
        &mut out,
        "target_prose_chars",
        plan.target_prose_chars,
        observed.prose_chars,
    );
    add_numeric_divergence(
        &mut out,
        "target_image_count",
        plan.target_image_count,
        observed.image_count,
    );
    add_numeric_divergence(
        &mut out,
        "target_animation_count",
        plan.target_animation_count,
        observed.animation_count,
    );

    // Section-kind sequence divergence: count positions that don't
    // match. (Length differences absorb into magnitude.)
    let len_max = plan.target_section_kinds.len().max(observed.section_kinds.len());
    let len_min = plan.target_section_kinds.len().min(observed.section_kinds.len());
    let mut mismatches = (len_max - len_min) as u32;
    for i in 0..len_min {
        if plan.target_section_kinds[i] != observed.section_kinds[i] {
            mismatches += 1;
        }
    }
    if mismatches > 0 {
        let mag = if len_max == 0 {
            0
        } else {
            (mismatches * 100) / len_max as u32
        };
        out.push(Divergence {
            field: "target_section_kinds",
            planned: plan.target_section_kinds.join(", "),
            observed: observed.section_kinds.join(", "),
            magnitude: mag,
            severity: severity_for_magnitude(mag),
        });
    }

    out
}

fn add_numeric_divergence(
    out: &mut Vec<Divergence>,
    field: &'static str,
    planned: u32,
    observed: u32,
) {
    if planned == observed {
        return;
    }
    let diff = planned.abs_diff(observed);
    let base = planned.max(observed).max(1);
    let mag = (diff * 100) / base;
    if mag == 0 {
        return;
    }
    out.push(Divergence {
        field,
        planned: planned.to_string(),
        observed: observed.to_string(),
        magnitude: mag,
        severity: severity_for_magnitude(mag),
    });
}

fn severity_for_magnitude(mag: u32) -> DivergenceSeverity {
    match mag {
        0..=10 => DivergenceSeverity::Minor,
        11..=30 => DivergenceSeverity::Moderate,
        _ => DivergenceSeverity::Major,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_plan() -> Plan {
        Plan {
            spec_version: 1,
            tenant_id: "alpha".to_owned(),
            site_id: "alpha".to_owned(),
            operator_id: "paul".to_owned(),
            page_kind: "brief".to_owned(),
            theme: "light".to_owned(),
            density: "comfortable".to_owned(),
            target_section_count: 4,
            target_section_kinds: vec![
                "hero".to_owned(),
                "paragraph".to_owned(),
                "pull_quote".to_owned(),
                "call_to_action".to_owned(),
            ],
            target_prose_chars: 1000,
            target_image_count: 0,
            target_animation_count: 0,
            notes: None,
        }
    }

    fn matching_snapshot() -> ExecutionSnapshot {
        ExecutionSnapshot {
            page_kind: "brief".to_owned(),
            theme: "light".to_owned(),
            density: "comfortable".to_owned(),
            section_count: 4,
            section_kinds: vec![
                "hero".to_owned(),
                "paragraph".to_owned(),
                "pull_quote".to_owned(),
                "call_to_action".to_owned(),
            ],
            prose_chars: 1000,
            image_count: 0,
            animation_count: 0,
        }
    }

    #[test]
    fn no_divergence_when_perfect_match() {
        let divs = audit_execution(&sample_plan(), &matching_snapshot());
        assert!(divs.is_empty());
    }

    #[test]
    fn page_kind_discrepancy_caught() {
        let mut snap = matching_snapshot();
        snap.page_kind = "marketing_landing".to_owned();
        let divs = audit_execution(&sample_plan(), &snap);
        assert!(divs.iter().any(|d| d.field == "page_kind"
            && d.severity == DivergenceSeverity::Discrepancy));
    }

    #[test]
    fn minor_numeric_drift_is_minor() {
        let mut snap = matching_snapshot();
        snap.prose_chars = 1050; // 5% drift
        let divs = audit_execution(&sample_plan(), &snap);
        let prose = divs.iter().find(|d| d.field == "target_prose_chars").unwrap();
        assert_eq!(prose.severity, DivergenceSeverity::Minor);
    }

    #[test]
    fn major_numeric_drift_is_major() {
        let mut snap = matching_snapshot();
        snap.prose_chars = 2500; // 60% drift
        let divs = audit_execution(&sample_plan(), &snap);
        let prose = divs.iter().find(|d| d.field == "target_prose_chars").unwrap();
        assert_eq!(prose.severity, DivergenceSeverity::Major);
    }

    #[test]
    fn section_kind_sequence_mismatch_caught() {
        let mut snap = matching_snapshot();
        // Reorder middle two sections.
        snap.section_kinds = vec![
            "hero".to_owned(),
            "pull_quote".to_owned(),
            "paragraph".to_owned(),
            "call_to_action".to_owned(),
        ];
        let divs = audit_execution(&sample_plan(), &snap);
        assert!(divs.iter().any(|d| d.field == "target_section_kinds"));
    }

    #[test]
    fn extra_section_inflates_magnitude() {
        let mut snap = matching_snapshot();
        snap.section_kinds.push("testimonial".to_owned());
        snap.section_count = 5;
        let divs = audit_execution(&sample_plan(), &snap);
        // Both target_section_count + target_section_kinds should
        // fire.
        assert!(divs.iter().any(|d| d.field == "target_section_count"));
        assert!(divs.iter().any(|d| d.field == "target_section_kinds"));
    }

    #[test]
    fn plan_serializes_to_json() {
        let plan = sample_plan();
        let json = serde_json::to_string(&plan).unwrap();
        let roundtrip: Plan = serde_json::from_str(&json).unwrap();
        assert_eq!(plan, roundtrip);
    }

    #[test]
    fn divergence_severity_buckets() {
        assert_eq!(severity_for_magnitude(0), DivergenceSeverity::Minor);
        assert_eq!(severity_for_magnitude(10), DivergenceSeverity::Minor);
        assert_eq!(severity_for_magnitude(11), DivergenceSeverity::Moderate);
        assert_eq!(severity_for_magnitude(30), DivergenceSeverity::Moderate);
        assert_eq!(severity_for_magnitude(31), DivergenceSeverity::Major);
        assert_eq!(severity_for_magnitude(100), DivergenceSeverity::Major);
    }
}
