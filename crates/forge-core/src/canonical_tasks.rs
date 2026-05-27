//! `canonical_tasks` — opinionated registry mapping common task
//! categories to their single canonical workflow.
//!
//! Per task #389 (AI-DX accessibility #2 of 4). For every common
//! substrate task, the substrate ships ONE canonical workflow.
//! Operators don't choose between 5 ways to do X; there's one
//! way + the substrate enforces that way.
//!
//! This is the doctrine extension of workflow_registry (#363):
//! the registry stores the workflows; this module asserts that
//! every common task category has exactly one workflow mapped.
//!
//! ## Why one canonical
//!
//! Multiple workflows for one task is choice paralysis for AI
//! agents and a source of cohort-level fragmentation: 50 tenants
//! shipping via 5 different workflows produce 5x the variance
//! the substrate could surveil + correct. One canonical workflow
//! per task = one signal to monitor + one decision tree to
//! train against.

use serde::Serialize;

/// Common task categories the substrate covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskCategory {
    /// Build a new tenant site from scratch.
    BuildSiteFromBrief,
    /// Apply a scoped change to existing tenant.
    ModifyExistingSite,
    /// Add a new primitive to the substrate.
    AddPrimitive,
    /// Modify an existing primitive.
    ModifyPrimitive,
    /// Add a new audit phase.
    AddAuditPhase,
    /// Check content originality.
    VerifyContentOriginality,
    /// Check structural fingerprint.
    SiteFingerprintCheck,
    /// Extract a SiteSpec from a captured URL.
    ReferenceExtraction,
    /// Register a substrate-capability gap.
    RegisterSubstrateGap,
    /// Explain a doctrine violation.
    ExplainDoctrineViolation,
    /// Discover which workflow to invoke.
    DiscoverWorkflow,
}

impl TaskCategory {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::BuildSiteFromBrief => "build_site_from_brief",
            Self::ModifyExistingSite => "modify_existing_site",
            Self::AddPrimitive => "add_primitive",
            Self::ModifyPrimitive => "modify_primitive",
            Self::AddAuditPhase => "add_audit_phase",
            Self::VerifyContentOriginality => "verify_content_originality",
            Self::SiteFingerprintCheck => "site_fingerprint_check",
            Self::ReferenceExtraction => "reference_extraction",
            Self::RegisterSubstrateGap => "register_substrate_gap",
            Self::ExplainDoctrineViolation => "explain_doctrine_violation",
            Self::DiscoverWorkflow => "discover_workflow",
        }
    }

    /// The canonical workflow_registry slug for this task.
    /// One-to-one mapping; if you ever feel tempted to return
    /// two, the substrate's opinionated-workflow doctrine is
    /// being broken.
    #[must_use]
    pub const fn canonical_workflow_slug(self) -> &'static str {
        match self {
            Self::BuildSiteFromBrief => "build_site_from_brief",
            Self::ModifyExistingSite => "modify_site",
            Self::AddPrimitive => "add_primitive",
            Self::ModifyPrimitive => "modify_primitive",
            Self::AddAuditPhase => "add_audit_phase",
            Self::VerifyContentOriginality => "verify_content_originality",
            Self::SiteFingerprintCheck => "site_fingerprint_check",
            Self::ReferenceExtraction => "reference_extraction",
            Self::RegisterSubstrateGap => "substrate_gap_registration",
            Self::ExplainDoctrineViolation => "doctrine_violation_explanation",
            Self::DiscoverWorkflow => "skill_invocation_meta",
        }
    }
}

/// All registered task categories (one per workflow).
pub const TASK_CATEGORIES: &[TaskCategory] = &[
    TaskCategory::BuildSiteFromBrief,
    TaskCategory::ModifyExistingSite,
    TaskCategory::AddPrimitive,
    TaskCategory::ModifyPrimitive,
    TaskCategory::AddAuditPhase,
    TaskCategory::VerifyContentOriginality,
    TaskCategory::SiteFingerprintCheck,
    TaskCategory::ReferenceExtraction,
    TaskCategory::RegisterSubstrateGap,
    TaskCategory::ExplainDoctrineViolation,
    TaskCategory::DiscoverWorkflow,
];

/// Look up the canonical workflow slug for a task category.
#[must_use]
pub fn canonical_for(category: TaskCategory) -> &'static str {
    category.canonical_workflow_slug()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow_registry::all_workflows;

    #[test]
    fn every_category_maps_to_a_registered_workflow() {
        let known_slugs: Vec<&str> =
            all_workflows().iter().map(|w| w.slug).collect();
        for category in TASK_CATEGORIES {
            let canonical = category.canonical_workflow_slug();
            assert!(
                known_slugs.contains(&canonical),
                "category {:?} -> canonical workflow '{}' not in workflow_registry",
                category,
                canonical
            );
        }
    }

    #[test]
    fn category_count_matches_workflow_count() {
        // The opinionated-workflow doctrine requires one category
        // per workflow. Counts diverge → either a workflow lacks
        // a category mapping (missing entry here) or a category
        // is unmapped (orphan).
        assert_eq!(TASK_CATEGORIES.len(), all_workflows().len());
    }

    #[test]
    fn category_to_workflow_is_one_to_one() {
        // No two categories should map to the same workflow.
        let mut canonicals: Vec<&str> =
            TASK_CATEGORIES.iter().map(|c| c.canonical_workflow_slug()).collect();
        canonicals.sort_unstable();
        let original = canonicals.len();
        canonicals.dedup();
        assert_eq!(canonicals.len(), original);
    }

    #[test]
    fn slug_stable() {
        assert_eq!(
            TaskCategory::BuildSiteFromBrief.slug(),
            "build_site_from_brief"
        );
        assert_eq!(
            TaskCategory::RegisterSubstrateGap.slug(),
            "register_substrate_gap"
        );
        assert_eq!(
            TaskCategory::DiscoverWorkflow.slug(),
            "discover_workflow"
        );
    }

    #[test]
    fn canonical_for_returns_expected_slug() {
        assert_eq!(
            canonical_for(TaskCategory::BuildSiteFromBrief),
            "build_site_from_brief"
        );
        assert_eq!(
            canonical_for(TaskCategory::DiscoverWorkflow),
            "skill_invocation_meta"
        );
    }
}
