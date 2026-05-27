//! Type-enforced argument shapes for every `forge.*` MCP tool.
//!
//! Layer-1 substrate-boundary discipline (#362): every MCP tool
//! parses its arguments through a typed struct with
//! `#[serde(deny_unknown_fields)]` so:
//!
//! 1. Typo'd argument names (`roots` instead of `root`) fail at
//!    parse time, not silently default.
//! 2. Type mismatches (e.g. boolean passed where string expected)
//!    surface as structured errors, not panics or silent
//!    fallbacks.
//! 3. Required vs optional fields are encoded in the type system
//!    (`Option<T>` vs `T`).
//! 4. Future schema-generation (e.g. via schemars) can derive
//!    `inputSchema` from these structs instead of hand-maintained
//!    JSON, closing the doc/code drift gap.
//!
//! Pattern: each tool function takes `Value` from the JSON-RPC
//! layer and immediately calls `parse_args::<ToolNameArgs>(value)`.
//! Parse failures return an `isError` MCP response carrying the
//! serde error message. The actual tool body sees a typed struct.

use serde::Deserialize;
use serde_json::{json, Value};

/// MCP error response carrying a typed parse-error message.
fn arg_parse_error(tool: &str, err: serde_json::Error) -> Value {
    json!({
        "isError": true,
        "content": [{
            "type": "text",
            "text": format!(
                "forge.{tool}: argument parse error — {err}\n\
                 hint: this MCP server runs deny_unknown_fields; \
                 verify field names match the schema in tools/list."
            )
        }]
    })
}

/// Parse a serde_json::Value into the typed argument struct for a
/// tool. On failure, returns the structured MCP error response.
/// On success, returns the typed struct.
pub(crate) fn parse_args<T: for<'de> Deserialize<'de>>(
    tool: &str,
    args: Value,
) -> Result<T, Value> {
    serde_json::from_value::<T>(args).map_err(|e| arg_parse_error(tool, e))
}

/// `forge.orient` — { root?: String }
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrientArgs {
    #[serde(default)]
    pub root: Option<String>,
}

/// `forge.build` — { root?: String, json?: bool }
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct BuildArgs {
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub json: bool,
}

/// `forge.doctrine.for` — { root?, path: String, terse?: bool }
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DoctrineForArgs {
    #[serde(default)]
    pub root: Option<String>,
    pub path: String,
    #[serde(default = "DoctrineForArgs::default_terse")]
    pub terse: bool,
}

impl DoctrineForArgs {
    const fn default_terse() -> bool {
        true
    }
}

/// `forge.authoring` — { root?: String }
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct AuthoringArgs {
    #[serde(default)]
    pub root: Option<String>,
}

/// `forge.config` — { root?: String }
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConfigArgs {
    #[serde(default)]
    pub root: Option<String>,
}

/// `forge.fix` — { root?: String }
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct FixArgs {
    #[serde(default)]
    pub root: Option<String>,
}

/// `forge.synthesis.preview` — { root?: String, spec_path: String }
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SynthesisPreviewArgs {
    #[serde(default)]
    pub root: Option<String>,
    pub spec_path: String,
}

/// `forge.codegen` — { root?, out?: String, dry_run?: bool }
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct CodegenArgs {
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub out: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
}

/// `forge.manifest.validate` — { root?: String }
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManifestValidateArgs {
    #[serde(default)]
    pub root: Option<String>,
}

/// `forge.build_site_from_brief` — Workflow #1 paired skill+MCP.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BuildSiteFromBriefArgs {
    pub brief_path: String,
    pub tenant_root: String,
    pub site_id: String,
    pub tenant_id: String,
    #[serde(default = "BuildSiteFromBriefArgs::default_dry_run")]
    pub dry_run: bool,
}

impl BuildSiteFromBriefArgs {
    const fn default_dry_run() -> bool {
        true
    }
}

/// `forge.bricks` — Brick-library query.
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct BricksArgs {
    #[serde(default)]
    pub fit: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
}

/// `forge.audit_plan_execution` — plan-vs-execution audit.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AuditPlanExecutionArgs {
    pub plan_json: String,
    pub observed_json: String,
}

/// `forge.budgets` — Resource-budget query for a PageKind.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BudgetsArgs {
    pub page_kind: String,
}

/// `forge.exemplars` — Exemplar / anti-exemplar / contrast-pair query.
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExemplarsArgs {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
}

/// `forge.record_outcome` — Layer-6 outcome tracking.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecordOutcomeArgs {
    pub outcomes_path: String,
    pub tenant_id: String,
    pub rater_id: String,
    pub kind: String,
    pub score: u32,
    #[serde(default)]
    pub notes: Option<String>,
}

/// `forge.cohort_summary` — Layer-6 cohort aggregation.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CohortSummaryArgs {
    pub outcomes_path: String,
    pub kind: String,
    pub group_by: String,
}

/// `forge.operator_profile` — Layer-6 operator-rating profile.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OperatorProfileArgs {
    pub outcomes_path: String,
    pub operator_id: String,
}

/// `forge.record_correction` — Layer-5 inline operator override.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecordCorrectionArgs {
    pub corrections_path: String,
    pub tenant_id: String,
    pub operator_id: String,
    pub axis: String,
    pub original_value: String,
    pub corrected_value: String,
    #[serde(default)]
    pub reason: Option<String>,
}

/// `forge.operator_preferences` — Layer-5 operator-pattern lookup.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OperatorPreferencesArgs {
    pub corrections_path: String,
    pub operator_id: String,
}

/// `forge.alternatives` — Layer-4 multi-pass alternatives surfacing.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AlternativesArgs {
    pub axis: String,
    pub seed: String,
}

/// `forge.skill_invocation_meta` — Workflow #11 paired skill+MCP.
///
/// Entry-point router: given a task description, surfaces the
/// matching workflow candidates from `workflow_registry`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SkillInvocationMetaArgs {
    pub task_description: String,
    #[serde(default = "SkillInvocationMetaArgs::default_max_candidates")]
    pub max_candidates: u32,
}

impl SkillInvocationMetaArgs {
    const fn default_max_candidates() -> u32 {
        5
    }
}

/// `forge.doctrine_violation_explanation` — Workflow #10 paired skill+MCP.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DoctrineViolationExplanationArgs {
    pub rule_id: String,
    #[serde(default)]
    pub violating_path: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
}

/// `forge.substrate_gap_registration` — Workflow #9 paired skill+MCP.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SubstrateGapRegistrationArgs {
    pub registry_path: String,
    pub kind: String,
    pub observed_in: String,
    pub summary: String,
    pub proposed_resolution: String,
    #[serde(default)]
    pub related_tasks: Vec<String>,
}

/// `forge.reference_extraction` — Workflow #8 paired skill+MCP.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReferenceExtractionArgs {
    pub capture_dir: String,
    pub site_id: String,
    pub tenant_id: String,
}

/// `forge.site_fingerprint_check` — Workflow #7 paired skill+MCP.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SiteFingerprintCheckArgs {
    pub tenant_root: String,
    #[serde(default)]
    pub registry_path: Option<String>,
    #[serde(default = "SiteFingerprintCheckArgs::default_threshold")]
    pub distance_threshold: u32,
}

impl SiteFingerprintCheckArgs {
    const fn default_threshold() -> u32 {
        4
    }
}

/// `forge.verify_content_originality` — Workflow #6 paired skill+MCP.
///
/// Anti-reuse gate: scans tenant strings vs corpus strings via
/// n-gram shingles, surfaces overlaps with verdict (ok/flag/block).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VerifyContentOriginalityArgs {
    pub tenant_root: String,
    #[serde(default)]
    pub corpus_roots: Vec<String>,
    #[serde(default = "VerifyContentOriginalityArgs::default_min_ngram_words")]
    pub min_ngram_words: u32,
}

impl VerifyContentOriginalityArgs {
    const fn default_min_ngram_words() -> u32 {
        6
    }
}

/// `forge.modify_primitive` — Workflow #5 paired skill+MCP.
///
/// Classifies a proposed primitive modification per the
/// backward_compat_version_discipline 4-category taxonomy.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModifyPrimitiveArgs {
    pub primitive_name: String,
    pub change_kind: String,
    pub change_summary: String,
}

/// `forge.add_audit_phase` — Workflow #4 paired skill+MCP.
///
/// Pre-flight guard for adding a new audit phase: checks the
/// proposed name against existing phase modules + buckets so the
/// developer doesn't ship a near-duplicate.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AddAuditPhaseArgs {
    pub proposed_name: String,
    #[serde(default)]
    pub finding_summary: Option<String>,
}

/// `forge.add_primitive` — Workflow #3 paired skill+MCP.
///
/// Pre-flight guard for adding a new primitive: checks the
/// proposed name against existing variants + reach data so the
/// developer doesn't ship a near-duplicate.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AddPrimitiveArgs {
    pub proposed_name: String,
    pub primitive_kind: String,
    #[serde(default)]
    pub shape_summary: Option<String>,
}

/// `forge.modify_site` — Workflow #2 paired skill+MCP.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModifySiteArgs {
    pub tenant_root: String,
    pub modification_kind: String,
    pub modification_path: String,
    #[serde(default = "ModifySiteArgs::default_dry_run")]
    pub dry_run: bool,
}

impl ModifySiteArgs {
    const fn default_dry_run() -> bool {
        true
    }
}

/// `forge.workflows.list` — { status?: String, slug?: String }
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkflowsListArgs {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
}

/// `forge.docs.query` — multi-field filter.
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct DocsQueryArgs {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub tags_any: Vec<String>,
    #[serde(default)]
    pub slug_prefix: Option<String>,
    #[serde(default)]
    pub contains_text: Option<String>,
    #[serde(default)]
    pub limit: Option<u64>,
    #[serde(default)]
    pub slug: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_orient_empty_ok() {
        let args = parse_args::<OrientArgs>("orient", json!({})).unwrap();
        assert!(args.root.is_none());
    }

    #[test]
    fn parse_orient_with_root_ok() {
        let args =
            parse_args::<OrientArgs>("orient", json!({"root": "/tmp/site"})).unwrap();
        assert_eq!(args.root.as_deref(), Some("/tmp/site"));
    }

    #[test]
    fn parse_orient_unknown_field_rejected() {
        let result = parse_args::<OrientArgs>("orient", json!({"roots": "/typo"}));
        assert!(result.is_err());
        // The error Value should be an MCP isError response.
        let err_value = result.unwrap_err();
        assert_eq!(err_value.get("isError"), Some(&json!(true)));
    }

    #[test]
    fn parse_build_defaults_json_false() {
        let args = parse_args::<BuildArgs>("build", json!({})).unwrap();
        assert!(!args.json);
    }

    #[test]
    fn parse_build_wrong_type_rejected() {
        let result = parse_args::<BuildArgs>("build", json!({"json": "true"}));
        assert!(result.is_err());
    }

    #[test]
    fn parse_doctrine_for_requires_path() {
        let result = parse_args::<DoctrineForArgs>("doctrine.for", json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn parse_doctrine_for_terse_defaults_true() {
        let args = parse_args::<DoctrineForArgs>(
            "doctrine.for",
            json!({"path": "crates/forge-core/src"}),
        )
        .unwrap();
        assert!(args.terse);
    }

    #[test]
    fn parse_synthesis_preview_requires_spec_path() {
        let result = parse_args::<SynthesisPreviewArgs>(
            "synthesis.preview",
            json!({"root": "/tmp/site"}),
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_codegen_all_optional() {
        let args = parse_args::<CodegenArgs>("codegen", json!({})).unwrap();
        assert!(args.out.is_none());
        assert!(!args.dry_run);
    }

    #[test]
    fn parse_build_site_from_brief_requires_all_four_fields() {
        let result = parse_args::<BuildSiteFromBriefArgs>(
            "build_site_from_brief",
            json!({"brief_path": "/tmp/brief.toml"}),
        );
        assert!(result.is_err(), "missing tenant_root/site_id/tenant_id should fail");
    }

    #[test]
    fn parse_build_site_from_brief_dry_run_defaults_true() {
        let args = parse_args::<BuildSiteFromBriefArgs>(
            "build_site_from_brief",
            json!({
                "brief_path": "/tmp/brief.toml",
                "tenant_root": "/tmp/tenant",
                "site_id": "test",
                "tenant_id": "test"
            }),
        )
        .unwrap();
        assert!(args.dry_run);
    }

    #[test]
    fn parse_build_site_from_brief_rejects_unknown_field() {
        let result = parse_args::<BuildSiteFromBriefArgs>(
            "build_site_from_brief",
            json!({
                "brief_path": "/tmp/brief.toml",
                "tenant_root": "/tmp/tenant",
                "site_id": "test",
                "tenant_id": "test",
                "extra_field": "oops"
            }),
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_bricks_empty_ok() {
        let args = parse_args::<BricksArgs>("bricks", json!({})).unwrap();
        assert!(args.fit.is_none() && args.id.is_none());
    }

    #[test]
    fn parse_bricks_rejects_unknown() {
        let result = parse_args::<BricksArgs>("bricks", json!({"bogus": 1}));
        assert!(result.is_err());
    }

    #[test]
    fn parse_audit_plan_execution_requires_both() {
        let result = parse_args::<AuditPlanExecutionArgs>(
            "audit_plan_execution",
            json!({"plan_json": "{}"}),
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_budgets_requires_page_kind() {
        let result = parse_args::<BudgetsArgs>("budgets", json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn parse_budgets_ok() {
        let args = parse_args::<BudgetsArgs>(
            "budgets",
            json!({"page_kind": "brief"}),
        )
        .unwrap();
        assert_eq!(args.page_kind, "brief");
    }

    #[test]
    fn parse_exemplars_empty_ok() {
        let args = parse_args::<ExemplarsArgs>("exemplars", json!({})).unwrap();
        assert!(args.kind.is_none());
    }

    #[test]
    fn parse_exemplars_unknown_field_rejected() {
        let result = parse_args::<ExemplarsArgs>("exemplars", json!({"bogus": true}));
        assert!(result.is_err());
    }

    #[test]
    fn parse_record_outcome_requires_required_fields() {
        let result = parse_args::<RecordOutcomeArgs>(
            "record_outcome",
            json!({"outcomes_path": "/tmp/o.jsonl"}),
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_record_outcome_full_ok() {
        let args = parse_args::<RecordOutcomeArgs>(
            "record_outcome",
            json!({
                "outcomes_path": "/tmp/o.jsonl",
                "tenant_id": "alpha",
                "rater_id": "paul",
                "kind": "ship",
                "score": 85
            }),
        )
        .unwrap();
        assert_eq!(args.score, 85);
    }

    #[test]
    fn parse_cohort_summary_requires_group_by() {
        let result = parse_args::<CohortSummaryArgs>(
            "cohort_summary",
            json!({"outcomes_path": "/tmp/o.jsonl", "kind": "ship"}),
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_record_correction_requires_all_required_fields() {
        let result = parse_args::<RecordCorrectionArgs>(
            "record_correction",
            json!({"corrections_path": "/tmp/c.jsonl", "tenant_id": "alpha"}),
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_record_correction_reason_optional() {
        let args = parse_args::<RecordCorrectionArgs>(
            "record_correction",
            json!({
                "corrections_path": "/tmp/c.jsonl",
                "tenant_id": "alpha",
                "operator_id": "paul",
                "axis": "theme",
                "original_value": "light",
                "corrected_value": "editorial"
            }),
        )
        .unwrap();
        assert!(args.reason.is_none());
    }

    #[test]
    fn parse_operator_preferences_requires_both_fields() {
        let result = parse_args::<OperatorPreferencesArgs>(
            "operator_preferences",
            json!({"operator_id": "paul"}),
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_alternatives_requires_axis_and_seed() {
        let result = parse_args::<AlternativesArgs>("alternatives", json!({"axis": "theme"}));
        assert!(result.is_err());
    }

    #[test]
    fn parse_alternatives_full_ok() {
        let args = parse_args::<AlternativesArgs>(
            "alternatives",
            json!({"axis": "theme", "seed": "light"}),
        )
        .unwrap();
        assert_eq!(args.axis, "theme");
        assert_eq!(args.seed, "light");
    }

    #[test]
    fn parse_skill_invocation_meta_requires_description() {
        let result = parse_args::<SkillInvocationMetaArgs>(
            "skill_invocation_meta",
            json!({}),
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_skill_invocation_meta_defaults_max_candidates() {
        let args = parse_args::<SkillInvocationMetaArgs>(
            "skill_invocation_meta",
            json!({"task_description": "swap theme to editorial"}),
        )
        .unwrap();
        assert_eq!(args.max_candidates, 5);
    }

    #[test]
    fn parse_doctrine_violation_requires_rule_id() {
        let result = parse_args::<DoctrineViolationExplanationArgs>(
            "doctrine_violation_explanation",
            json!({}),
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_doctrine_violation_optional_path() {
        let args = parse_args::<DoctrineViolationExplanationArgs>(
            "doctrine_violation_explanation",
            json!({"rule_id": "prim-012"}),
        )
        .unwrap();
        assert_eq!(args.rule_id, "prim-012");
        assert!(args.violating_path.is_none());
    }

    #[test]
    fn parse_gap_registration_requires_all_fields() {
        let result = parse_args::<SubstrateGapRegistrationArgs>(
            "substrate_gap_registration",
            json!({"registry_path": "/tmp/gaps.jsonl"}),
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_gap_registration_full_ok() {
        let args = parse_args::<SubstrateGapRegistrationArgs>(
            "substrate_gap_registration",
            json!({
                "registry_path": "/tmp/gaps.jsonl",
                "kind": "primitive",
                "observed_in": "tenant-alpha",
                "summary": "Need ComicStrip",
                "proposed_resolution": "Add CmsSection::ComicStrip",
                "related_tasks": ["#359"]
            }),
        )
        .unwrap();
        assert_eq!(args.related_tasks.len(), 1);
    }

    #[test]
    fn parse_reference_extraction_requires_three_fields() {
        let result = parse_args::<ReferenceExtractionArgs>(
            "reference_extraction",
            json!({"capture_dir": "/tmp/cap"}),
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_fingerprint_check_requires_tenant_root() {
        let result = parse_args::<SiteFingerprintCheckArgs>("site_fingerprint_check", json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn parse_fingerprint_check_defaults_threshold() {
        let args = parse_args::<SiteFingerprintCheckArgs>(
            "site_fingerprint_check",
            json!({"tenant_root": "/tmp/tenant"}),
        )
        .unwrap();
        assert_eq!(args.distance_threshold, 4);
        assert!(args.registry_path.is_none());
    }

    #[test]
    fn parse_verify_originality_requires_tenant_root() {
        let result = parse_args::<VerifyContentOriginalityArgs>(
            "verify_content_originality",
            json!({}),
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_verify_originality_defaults_min_ngram() {
        let args = parse_args::<VerifyContentOriginalityArgs>(
            "verify_content_originality",
            json!({"tenant_root": "/tmp/tenant"}),
        )
        .unwrap();
        assert_eq!(args.min_ngram_words, 6);
        assert!(args.corpus_roots.is_empty());
    }

    #[test]
    fn parse_modify_primitive_requires_three_fields() {
        let result = parse_args::<ModifyPrimitiveArgs>(
            "modify_primitive",
            json!({"primitive_name": "FeatureSpotlight"}),
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_modify_primitive_full_ok() {
        let args = parse_args::<ModifyPrimitiveArgs>(
            "modify_primitive",
            json!({
                "primitive_name": "FeatureSpotlightDecoration",
                "change_kind": "additive",
                "change_summary": "Add Brutalist variant"
            }),
        )
        .unwrap();
        assert_eq!(args.change_kind, "additive");
    }

    #[test]
    fn parse_add_audit_phase_requires_name() {
        let result = parse_args::<AddAuditPhaseArgs>("add_audit_phase", json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn parse_add_audit_phase_optional_summary() {
        let args = parse_args::<AddAuditPhaseArgs>(
            "add_audit_phase",
            json!({"proposed_name": "image_dimension_required"}),
        )
        .unwrap();
        assert_eq!(args.proposed_name, "image_dimension_required");
        assert!(args.finding_summary.is_none());
    }

    #[test]
    fn parse_add_primitive_requires_two_fields() {
        let result = parse_args::<AddPrimitiveArgs>(
            "add_primitive",
            json!({"proposed_name": "TimelineEvent"}),
        );
        assert!(result.is_err(), "missing primitive_kind should fail");
    }

    #[test]
    fn parse_add_primitive_accepts_optional_summary() {
        let args = parse_args::<AddPrimitiveArgs>(
            "add_primitive",
            json!({
                "proposed_name": "TimelineEvent",
                "primitive_kind": "section",
                "shape_summary": "Date + title + summary, vertically stacked"
            }),
        )
        .unwrap();
        assert!(args.shape_summary.is_some());
    }

    #[test]
    fn parse_modify_site_requires_three_fields() {
        let result = parse_args::<ModifySiteArgs>(
            "modify_site",
            json!({"tenant_root": "/tmp/tenant"}),
        );
        assert!(result.is_err(), "missing modification_kind/path should fail");
    }

    #[test]
    fn parse_modify_site_dry_run_defaults_true() {
        let args = parse_args::<ModifySiteArgs>(
            "modify_site",
            json!({
                "tenant_root": "/tmp/tenant",
                "modification_kind": "change_theme",
                "modification_path": "/tmp/mod.toml"
            }),
        )
        .unwrap();
        assert!(args.dry_run);
    }

    #[test]
    fn parse_docs_query_empty_ok() {
        let args = parse_args::<DocsQueryArgs>("docs.query", json!({})).unwrap();
        assert!(args.kind.is_none());
        assert!(args.tags_any.is_empty());
    }

    #[test]
    fn parse_docs_query_full_ok() {
        let args = parse_args::<DocsQueryArgs>(
            "docs.query",
            json!({
                "kind": "doctrine",
                "tags_any": ["tenant", "primitive"],
                "slug_prefix": "manifest",
                "limit": 5
            }),
        )
        .unwrap();
        assert_eq!(args.kind.as_deref(), Some("doctrine"));
        assert_eq!(args.tags_any.len(), 2);
        assert_eq!(args.limit, Some(5));
    }
}
