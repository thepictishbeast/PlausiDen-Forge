//! `workflow_registry` — typed registry of paired (skill, MCP-tool)
//! workflows.
//!
//! Layer-2 substrate-boundary discipline (#363): every workflow
//! exposed to AI agents is a PAIR — a SKILL.md describing what to
//! do + an MCP tool that does the heavy lifting. The skill is read
//! by the agent before it acts; the MCP tool is the action surface.
//!
//! This registry is the source of truth for which pairs exist,
//! their pairing status, and the conventions that bind them.
//! `forge-mcp` surfaces this registry through `forge.workflows.list`
//! so agents can discover the paired-workflow surface programmatically
//! rather than scanning `skills/` + `tool_list()` separately.
//!
//! ## Pairing convention
//!
//! For a workflow named `<verb>_<noun>` (e.g. `build_site_from_brief`):
//!
//! - Skill slug: `forge_<verb>_<noun>` (kebab-snake), file at
//!   `skills/forge-<verb>-<noun>/SKILL.md`
//! - MCP tool: `forge.<verb>_<noun>` (dotted prefix + snake_case)
//!
//! Example:
//!
//! - Workflow: `build_site_from_brief`
//! - Skill: `skills/forge-build-site-from-brief/SKILL.md`
//! - MCP tool: `forge.build_site_from_brief`
//!
//! ## Lifecycle states
//!
//! A workflow can be in one of:
//!
//! - `Planned`: registered here, no skill or MCP tool yet (the
//!   task tracker has the entry but neither side is shipped)
//! - `SkillOnly`: SKILL.md exists; MCP tool not yet wired
//! - `McpOnly`: MCP tool exists; SKILL.md not yet written
//! - `Paired`: both shipped (the only valid steady state for an
//!   agent-facing workflow)
//!
//! CI (#375) will enforce that `Planned` / `SkillOnly` / `McpOnly`
//! are transient — a workflow that hasn't reached `Paired` after a
//! grace window flags a finding.

use serde::Serialize;

/// Lifecycle state of a workflow pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PairingStatus {
    /// Registered as future work; no skill or MCP tool yet.
    Planned,
    /// SKILL.md exists; MCP tool not yet wired.
    SkillOnly,
    /// MCP tool exists; SKILL.md not yet written.
    McpOnly,
    /// Both skill and MCP tool shipped.
    Paired,
}

impl PairingStatus {
    /// Stable kebab-case slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::SkillOnly => "skill_only",
            Self::McpOnly => "mcp_only",
            Self::Paired => "paired",
        }
    }
}

/// One entry in the workflow registry.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct WorkflowEntry {
    /// Workflow slug (snake_case, no `forge_` prefix).
    /// Example: `build_site_from_brief`
    pub slug: &'static str,
    /// One-line summary.
    pub summary: &'static str,
    /// Skill directory name (kebab-case).
    /// Example: `forge-build-site-from-brief`
    pub skill_dir: &'static str,
    /// MCP tool name (dotted prefix + snake_case).
    /// Example: `forge.build_site_from_brief`
    pub mcp_tool: &'static str,
    /// Lifecycle state of the pair.
    pub status: PairingStatus,
    /// Issue/task ID tracking the work (best-effort).
    pub task_ref: &'static str,
}

/// Canonical registry of paired (skill, MCP-tool) workflows.
///
/// The 11 workflows tracked here correspond to substrate-reframe
/// tasks #364-#374. Statuses update as work lands. The registry is
/// compile-time static so `forge-mcp` and CI lints can consume it
/// without I/O.
pub const WORKFLOW_REGISTRY: &[WorkflowEntry] = &[
    WorkflowEntry {
        slug: "build_site_from_brief",
        summary: "Build a complete tenant site from a written brief; \
                  emits SiteSpec, runs forge build, surfaces audit findings.",
        skill_dir: "forge-build-site-from-brief",
        mcp_tool: "forge.build_site_from_brief",
        status: PairingStatus::Paired,
        task_ref: "#364",
    },
    WorkflowEntry {
        slug: "modify_site",
        summary: "Apply a scoped modification to an existing tenant site \
                  (content change, theme swap, primitive substitution).",
        skill_dir: "forge-modify-site",
        mcp_tool: "forge.modify_site",
        status: PairingStatus::Planned,
        task_ref: "#365",
    },
    WorkflowEntry {
        slug: "add_primitive",
        summary: "Add a new primitive to Loom or Forge with required \
                  tests, doc-query entry, and audit-phase coverage.",
        skill_dir: "add-loom-primitive",
        mcp_tool: "forge.add_primitive",
        status: PairingStatus::SkillOnly,
        task_ref: "#366",
    },
    WorkflowEntry {
        slug: "add_audit_phase",
        summary: "Add a new audit phase implementing the Phase trait; \
                  includes manifest entry + projection + tests.",
        skill_dir: "add-forge-phase",
        mcp_tool: "forge.add_audit_phase",
        status: PairingStatus::SkillOnly,
        task_ref: "#367",
    },
    WorkflowEntry {
        slug: "modify_primitive",
        summary: "Modify an existing primitive (new variant, new field, \
                  decoration change) without breaking back-compat.",
        skill_dir: "forge-modify-primitive",
        mcp_tool: "forge.modify_primitive",
        status: PairingStatus::Planned,
        task_ref: "#368",
    },
    WorkflowEntry {
        slug: "verify_content_originality",
        summary: "Check tenant content for verbatim reuse against reference \
                  corpora; anti-reuse gate.",
        skill_dir: "forge-verify-content-originality",
        mcp_tool: "forge.verify_content_originality",
        status: PairingStatus::Planned,
        task_ref: "#369",
    },
    WorkflowEntry {
        slug: "site_fingerprint_check",
        summary: "Compute and check a site's structural / visual / content \
                  fingerprint against the anti-pattern registry.",
        skill_dir: "forge-site-fingerprint-check",
        mcp_tool: "forge.site_fingerprint_check",
        status: PairingStatus::Planned,
        task_ref: "#370",
    },
    WorkflowEntry {
        slug: "reference_extraction",
        summary: "Run the deterministic URL → SiteSpec pipeline against \
                  a captured reference site.",
        skill_dir: "forge-reference-extraction",
        mcp_tool: "forge.reference_extraction",
        status: PairingStatus::Planned,
        task_ref: "#371",
    },
    WorkflowEntry {
        slug: "substrate_gap_registration",
        summary: "Register a gap the substrate doesn't cover (with proposal \
                  for resolution); feeds the gap registry.",
        skill_dir: "forge-substrate-gap-registration",
        mcp_tool: "forge.substrate_gap_registration",
        status: PairingStatus::Planned,
        task_ref: "#372",
    },
    WorkflowEntry {
        slug: "doctrine_violation_explanation",
        summary: "When an audit phase flags a doctrine violation, explain the \
                  rule, rationale, and concrete remediation.",
        skill_dir: "forge-doctrine-violation-explanation",
        mcp_tool: "forge.doctrine_violation_explanation",
        status: PairingStatus::Planned,
        task_ref: "#373",
    },
    WorkflowEntry {
        slug: "skill_invocation_meta",
        summary: "Entry-point meta-skill: which forge-* workflow applies to \
                  the current task? Used as orientation when scope is ambiguous.",
        skill_dir: "forge-skill-invocation-meta",
        mcp_tool: "forge.skill_invocation_meta",
        status: PairingStatus::Planned,
        task_ref: "#374",
    },
];

/// Return all workflow entries.
#[must_use]
pub fn all_workflows() -> &'static [WorkflowEntry] {
    WORKFLOW_REGISTRY
}

/// Look up a workflow entry by slug (snake_case, no `forge_`
/// prefix).
#[must_use]
pub fn get_workflow(slug: &str) -> Option<&'static WorkflowEntry> {
    WORKFLOW_REGISTRY.iter().find(|w| w.slug == slug)
}

/// Filter workflows by pairing status.
#[must_use]
pub fn workflows_with_status(status: PairingStatus) -> Vec<&'static WorkflowEntry> {
    WORKFLOW_REGISTRY
        .iter()
        .filter(|w| w.status == status)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_eleven_workflows() {
        assert_eq!(WORKFLOW_REGISTRY.len(), 11);
    }

    #[test]
    fn every_workflow_has_unique_slug() {
        let mut slugs: Vec<&str> = WORKFLOW_REGISTRY.iter().map(|w| w.slug).collect();
        slugs.sort_unstable();
        let original_len = slugs.len();
        slugs.dedup();
        assert_eq!(slugs.len(), original_len, "duplicate slug in registry");
    }

    #[test]
    fn every_workflow_has_unique_mcp_tool() {
        let mut tools: Vec<&str> =
            WORKFLOW_REGISTRY.iter().map(|w| w.mcp_tool).collect();
        tools.sort_unstable();
        let original_len = tools.len();
        tools.dedup();
        assert_eq!(tools.len(), original_len, "duplicate MCP tool name");
    }

    #[test]
    fn mcp_tool_names_follow_convention() {
        for entry in WORKFLOW_REGISTRY {
            assert!(
                entry.mcp_tool.starts_with("forge."),
                "mcp_tool must start with forge. — got {}",
                entry.mcp_tool
            );
            let suffix = &entry.mcp_tool["forge.".len()..];
            assert_eq!(
                suffix, entry.slug,
                "mcp_tool suffix must match workflow slug \
                 (forge.<slug>); got {}",
                entry.mcp_tool
            );
        }
    }

    #[test]
    fn get_workflow_finds_known_slug() {
        let entry = get_workflow("build_site_from_brief").expect("known slug");
        assert_eq!(entry.task_ref, "#364");
    }

    #[test]
    fn get_workflow_misses_unknown_slug() {
        assert!(get_workflow("does_not_exist").is_none());
    }

    #[test]
    fn workflows_with_status_filters() {
        let planned = workflows_with_status(PairingStatus::Planned);
        let skill_only = workflows_with_status(PairingStatus::SkillOnly);
        let paired = workflows_with_status(PairingStatus::Paired);
        // After #364 shipped: 1 Paired (build_site_from_brief),
        // 2 SkillOnly (add-loom-primitive, add-forge-phase),
        // 8 Planned. Sums to 11.
        assert_eq!(paired.len(), 1);
        assert_eq!(skill_only.len(), 2);
        assert_eq!(planned.len(), 8);
        assert_eq!(paired.len() + skill_only.len() + planned.len(), 11);
    }

    #[test]
    fn pairing_status_slugs_stable() {
        assert_eq!(PairingStatus::Planned.slug(), "planned");
        assert_eq!(PairingStatus::SkillOnly.slug(), "skill_only");
        assert_eq!(PairingStatus::McpOnly.slug(), "mcp_only");
        assert_eq!(PairingStatus::Paired.slug(), "paired");
    }
}
