//! Integration test: skill ↔ MCP-tool pairing invariants.
//!
//! Per task #375: every workflow registered as `Paired` in
//! `forge_core::workflow_registry::WORKFLOW_REGISTRY` MUST have:
//!
//! 1. A SKILL.md file on disk at `skills/<skill_dir>/SKILL.md`
//!    relative to the workspace root
//! 2. (Future: when forge-mcp exposes a public tool_list helper)
//!    the `mcp_tool` name registered in `forge-mcp::tool_list()`
//!
//! This is a CI-gating test: if a developer flips a workflow's
//! status to `Paired` without shipping the SKILL.md, this test
//! fails and the PR is blocked.
//!
//! Compile-time invariants (registry size, unique slugs/MCP names,
//! naming convention) live in `workflow_registry::tests` —
//! filesystem checks belong here because they require I/O.

use forge_core::workflow_registry::{all_workflows, PairingStatus};
use std::path::PathBuf;

/// Resolve the workspace root by walking up from the forge-core
/// CARGO_MANIFEST_DIR. forge-core lives at workspace/crates/forge-core,
/// so the workspace root is two parents up.
fn workspace_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .expect("crates dir parent")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn every_paired_workflow_has_skill_md_on_disk() {
    let root = workspace_root();
    let mut missing: Vec<String> = Vec::new();

    for entry in all_workflows() {
        if entry.status != PairingStatus::Paired {
            continue;
        }
        let skill_path = root
            .join("skills")
            .join(entry.skill_dir)
            .join("SKILL.md");
        if !skill_path.is_file() {
            missing.push(format!(
                "workflow '{}' (status: Paired) — expected {}",
                entry.slug,
                skill_path.display()
            ));
        }
    }

    if !missing.is_empty() {
        panic!(
            "Paired workflows missing SKILL.md on disk:\n{}",
            missing.join("\n")
        );
    }
}

#[test]
fn every_skill_only_workflow_has_skill_md_on_disk() {
    let root = workspace_root();
    let mut missing: Vec<String> = Vec::new();

    for entry in all_workflows() {
        if entry.status != PairingStatus::SkillOnly {
            continue;
        }
        let skill_path = root
            .join("skills")
            .join(entry.skill_dir)
            .join("SKILL.md");
        if !skill_path.is_file() {
            missing.push(format!(
                "workflow '{}' (status: SkillOnly) — expected {}",
                entry.slug,
                skill_path.display()
            ));
        }
    }

    if !missing.is_empty() {
        panic!(
            "SkillOnly workflows missing SKILL.md on disk:\n{}",
            missing.join("\n")
        );
    }
}

#[test]
fn skill_dir_naming_convention_matches_slug_or_pre_existing() {
    // Two skill dirs predate the workflow registry naming convention
    // (add-loom-primitive, add-forge-phase). The convention is
    // forge-<slug-with-dashes>; the two legacy names are tracked in
    // the registry with their historical paths. This test pins both
    // accepted forms.
    let legacy_acceptable: &[&str] =
        &["add-loom-primitive", "add-forge-phase"];

    for entry in all_workflows() {
        let conventional = format!("forge-{}", entry.slug.replace('_', "-"));
        let accepted = entry.skill_dir == conventional
            || legacy_acceptable.contains(&entry.skill_dir);
        assert!(
            accepted,
            "skill_dir '{}' for workflow '{}' doesn't match either:\n\
             - convention 'forge-<slug-with-dashes>' (would be '{}')\n\
             - legacy accepted set: {:?}",
            entry.skill_dir, entry.slug, conventional, legacy_acceptable
        );
    }
}
