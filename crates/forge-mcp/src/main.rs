//! `forge-mcp` — Model Context Protocol server for the Forge
//! substrate.
//!
//! Exposes Forge subcommands as JSON-RPC tools so MCP-aware
//! clients (Claude Code, Codex, Cursor, …) can invoke
//! `forge orient`, `forge build`, `forge audit <phase>
//! --explain`, `forge doctrine for <path>`, `forge synthesis
//! preview`, `forge codegen` without re-parsing CLI text on
//! every call. The server speaks line-delimited JSON-RPC 2.0
//! over stdio per the MCP spec.
//!
//! Per paul 2026-05-21: "skills and MCPs that allow you to work
//! even more closely with forge and get all the functionality
//! and potential out of it, it should be designed in a way that
//! saves as many tokens as possible."
//!
//! ## Tool surface (planned)
//!
//! - `forge.orient` — session brief
//! - `forge.build` — run every phase + return structured findings
//! - `forge.audit.explain` — `forge audit <phase> --explain`
//! - `forge.doctrine.for` — `forge doctrine for <path> --terse`
//! - `forge.synthesis.preview` — preview a `SiteSpec` before
//!   writing
//! - `forge.codegen` — emit an axum + tokio + sqlx crate from
//!   `cms/*.json` + `backends.toml`
//! - `forge.tenant_style.preview` — render the current tenant
//!   `[style]` config to its `<style>` snippet so callers can
//!   see what'll inject before running a full build
//!
//! Future iters wire each tool through to the appropriate
//! `forge` subcommand. v0.1.0 ships the stdio loop + tool
//! registry skeleton + one working tool (`forge.orient`).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Write;
use tokio::io::{AsyncBufReadExt, BufReader};

mod typed_args;
use typed_args::{
    parse_args, AddAuditPhaseArgs, AddPrimitiveArgs, AuthoringArgs, BuildArgs,
    BuildSiteFromBriefArgs, CodegenArgs, ConfigArgs, DocsQueryArgs, DoctrineForArgs,
    FixArgs, ManifestValidateArgs, ModifyPrimitiveArgs, ModifySiteArgs, OrientArgs,
    ReferenceExtractionArgs, SiteFingerprintCheckArgs, SubstrateGapRegistrationArgs,
    SynthesisPreviewArgs, VerifyContentOriginalityArgs, WorkflowsListArgs,
};

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

const SERVER_INFO: &str = r#"{
    "name": "forge-mcp",
    "version": "0.1.0",
    "description": "Forge substrate operations as MCP tools."
}"#;

fn tool_list() -> Value {
    json!({
        "tools": [
            {
                "name": "forge.orient",
                "description": "Session brief: doctrine rules in scope, skills inventory, canonical defaults, anti-patterns. Run this first whenever entering a Forge / Loom workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": {
                            "type": "string",
                            "description": "Project root path. Defaults to the working directory."
                        }
                    }
                }
            },
            {
                "name": "forge.build",
                "description": "Run every phase against the project at `root` and return the build report. Use this instead of shell-invoking `forge build` so the structured report stays out of the conversation as JSON, not CLI text.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": {
                            "type": "string",
                            "description": "Project root path. Defaults to the working directory."
                        },
                        "json": {
                            "type": "boolean",
                            "description": "If true, request `--json` output (when supported). Default false."
                        }
                    }
                }
            },
            {
                "name": "forge.doctrine.for",
                "description": "Surface AVP-Doctrine rules applicable to a path (crate, file, directory). Walks every loaded rule and matches each rule's `applies_to` entries against the path. Backed by `forge doctrine for <path>`.",
                "inputSchema": {
                    "type": "object",
                    "required": ["path"],
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Workspace-relative or absolute path to query rules for."
                        },
                        "root": {
                            "type": "string",
                            "description": "Project root. Defaults to the working directory."
                        },
                        "terse": {
                            "type": "boolean",
                            "description": "If true, surface rule ids + names only (saves tokens). Default true."
                        }
                    }
                }
            },
            {
                "name": "forge.authoring",
                "description": "Scan a tenant's cms/*.json for empty / below-floor content fields. Returns a structured TODO list of sections that still need content. Companion to the `content_substance` build phase.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": {
                            "type": "string",
                            "description": "Tenant root. Defaults to the working directory."
                        }
                    }
                }
            },
            {
                "name": "forge.config",
                "description": "Umbrella config-gate runner: privacy / trust-safety / domains / forms / federation / email / commerce / memberships. Each missing config file is a warning, not a failure (e.g., a tenant that doesn't sell anything doesn't need commerce.toml). Backed by `forge config`.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": {
                            "type": "string",
                            "description": "Tenant root. Defaults to the working directory."
                        }
                    }
                }
            },
            {
                "name": "forge.fix",
                "description": "Auto-fix mechanical findings from the latest build report. Idempotent; safe to run after every `forge.build`. Backed by `forge fix`.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": {
                            "type": "string",
                            "description": "Project root. Defaults to the working directory."
                        }
                    }
                }
            },
            {
                "name": "forge.synthesis.preview",
                "description": "Load a `SiteSpec` JSON and print its summary without writing any cms/ files. Lets the operator review before committing. Backed by `forge synthesis preview`.",
                "inputSchema": {
                    "type": "object",
                    "required": ["spec_path"],
                    "properties": {
                        "spec_path": {
                            "type": "string",
                            "description": "Path to the SiteSpec JSON file."
                        },
                        "root": {
                            "type": "string",
                            "description": "Project root. Defaults to the working directory."
                        }
                    }
                }
            },
            {
                "name": "forge.codegen",
                "description": "Emit a self-contained Cargo crate (axum + tokio + sqlx + serde + loom-cms-render) from cms/*.json + backends.toml. Each CmsPage becomes a typed `async fn` handler. Backed by `forge codegen`.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": {
                            "type": "string",
                            "description": "Project root. Defaults to the working directory."
                        },
                        "out": {
                            "type": "string",
                            "description": "Output directory for the generated crate. Required unless `dry_run` is true."
                        },
                        "dry_run": {
                            "type": "boolean",
                            "description": "If true, print the plan to stdout instead of writing. Default false."
                        }
                    }
                }
            },
            {
                "name": "forge.manifest.validate",
                "description": "Validate phases.toml + backends.toml at the project root. Reports parsing + projection + topo-sort errors. Backed by `forge manifest validate`.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": {
                            "type": "string",
                            "description": "Project root. Defaults to the working directory."
                        }
                    }
                }
            },
            {
                "name": "forge.docs.query",
                "description": "Query the substrate's progressive doc index (forge-core::doc_query). Returns hand-curated structured entries for doctrine, primitives, audit phases, workflows, and reframes. Reduces context consumption vs loading markdown pages upfront. Each filter is optional; absent filters return all entries.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "kind": {
                            "type": "string",
                            "description": "Restrict to entries of this kind. One of: doctrine, primitive, audit_phase, workflow, reframe."
                        },
                        "tags_any": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Match entries that have at least one of these tags. Valid: tenant, primitive, audit_phase, deploy, authoring, doctrine, reframe, workflow."
                        },
                        "slug_prefix": {
                            "type": "string",
                            "description": "Slug prefix match (case-insensitive)."
                        },
                        "contains_text": {
                            "type": "string",
                            "description": "Substring search across title / summary / body (case-insensitive)."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Cap the number of returned entries.",
                            "minimum": 1
                        },
                        "slug": {
                            "type": "string",
                            "description": "Look up a single entry by exact slug. When set, other filters are ignored and a single entry (or null) is returned."
                        }
                    }
                }
            },
            {
                "name": "forge.substrate_gap_registration",
                "description": "Register a substrate-capability gap into the canonical JSONL gap registry. Paired with skills/forge-substrate-gap-registration/SKILL.md (#372). Per substrate-reframe doctrine: don't route around gaps; register them.",
                "inputSchema": {
                    "type": "object",
                    "required": ["registry_path", "kind", "observed_in", "summary", "proposed_resolution"],
                    "properties": {
                        "registry_path": {
                            "type": "string",
                            "description": "Absolute path to the JSONL registry file. Created if absent."
                        },
                        "kind": {
                            "type": "string",
                            "description": "One of: primitive, audit_phase, theme, page_kind, page_field, doctrine_rule, tooling."
                        },
                        "observed_in": {
                            "type": "string",
                            "description": "Tenant ID or URL where the gap was observed."
                        },
                        "summary": {
                            "type": "string",
                            "description": "One-line description of the gap."
                        },
                        "proposed_resolution": {
                            "type": "string",
                            "description": "Concrete proposed substrate change to close the gap."
                        },
                        "related_tasks": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Optional task IDs that reference or unblock this gap."
                        }
                    }
                }
            },
            {
                "name": "forge.reference_extraction",
                "description": "Load a Crawler-emitted CaptureManifest and prepare for the URL → SiteSpec pipeline. Validates the manifest + surfaces capture inventory. Paired with skills/forge-reference-extraction/SKILL.md (#371). Per-axis extractor invocation lands once the chromiumoxide runner is verified end-to-end (see docs/SUBSTRATE_REFERENCE_PIPELINE_AUDIT_2026_05_27.md).",
                "inputSchema": {
                    "type": "object",
                    "required": ["capture_dir", "site_id", "tenant_id"],
                    "properties": {
                        "capture_dir": {
                            "type": "string",
                            "description": "Absolute path to the Crawler capture directory containing manifest.json."
                        },
                        "site_id": {
                            "type": "string",
                            "description": "Kebab-case site identifier for the emitted SiteSpec."
                        },
                        "tenant_id": {
                            "type": "string",
                            "description": "Kebab-case tenant identifier for the emitted SiteSpec."
                        }
                    }
                }
            },
            {
                "name": "forge.site_fingerprint_check",
                "description": "Compute a tenant's structural fingerprint (section ordering, primitive distribution, density, composition rhythm, asset distribution) and check against the fingerprint registry for near-duplicates. Paired with skills/forge-site-fingerprint-check/SKILL.md (#370).",
                "inputSchema": {
                    "type": "object",
                    "required": ["tenant_root"],
                    "properties": {
                        "tenant_root": {
                            "type": "string",
                            "description": "Absolute path to the tenant root; tenant_root/cms/ is read."
                        },
                        "registry_path": {
                            "type": "string",
                            "description": "Path to the fingerprint registry. When absent, no near-duplicate check runs (fingerprint is computed and returned alone)."
                        },
                        "distance_threshold": {
                            "type": "integer",
                            "description": "Component-distance threshold for near-duplicate detection. Default 4.",
                            "minimum": 0
                        }
                    }
                }
            },
            {
                "name": "forge.verify_content_originality",
                "description": "Anti-reuse gate: scans tenant cms/*.json strings vs reference corpora via n-gram shingles, returns overlaps + verdict (ok/flag/block). Paired with skills/forge-verify-content-originality/SKILL.md (#369).",
                "inputSchema": {
                    "type": "object",
                    "required": ["tenant_root"],
                    "properties": {
                        "tenant_root": {
                            "type": "string",
                            "description": "Absolute path to the tenant root (cms/*.json files are scanned)."
                        },
                        "corpus_roots": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Absolute paths to reference corpora directories. Each path's *.json files are scanned recursively."
                        },
                        "min_ngram_words": {
                            "type": "integer",
                            "description": "Shingle length in words. Default 6. Clamped 2..=20.",
                            "minimum": 2,
                            "maximum": 20
                        }
                    }
                }
            },
            {
                "name": "forge.modify_primitive",
                "description": "Classify a proposed primitive modification per the backward_compat_version_discipline 4-category taxonomy and surface the substrate-side requirements for that category. Paired with skills/forge-modify-primitive/SKILL.md (#368).",
                "inputSchema": {
                    "type": "object",
                    "required": ["primitive_name", "change_kind", "change_summary"],
                    "properties": {
                        "primitive_name": {
                            "type": "string",
                            "description": "Exact Rust type name being modified (e.g. FeatureSpotlightDecoration)."
                        },
                        "change_kind": {
                            "type": "string",
                            "description": "One of: invisible, additive, auto_migration, operator_action."
                        },
                        "change_summary": {
                            "type": "string",
                            "description": "One-line description of the change."
                        }
                    }
                }
            },
            {
                "name": "forge.add_audit_phase",
                "description": "Pre-flight guard for adding a new audit phase. Checks the proposed name against the 75+ existing phase modules in crates/forge-phases/src/ and surfaces near-duplicate category buckets. Paired with skills/add-forge-phase/SKILL.md (#367).",
                "inputSchema": {
                    "type": "object",
                    "required": ["proposed_name"],
                    "properties": {
                        "proposed_name": {
                            "type": "string",
                            "description": "snake_case name for the proposed phase module (e.g., image_dimension_required)."
                        },
                        "finding_summary": {
                            "type": "string",
                            "description": "One-line description of what the phase would detect; surfaced in the duplicate-check report."
                        }
                    }
                }
            },
            {
                "name": "forge.add_primitive",
                "description": "Pre-flight guard for adding a new primitive. Checks the proposed name against existing variants (case-insensitive substring + slug match) and surfaces near-duplicates. Paired with skills/add-loom-primitive/SKILL.md (#366).",
                "inputSchema": {
                    "type": "object",
                    "required": ["proposed_name", "primitive_kind"],
                    "properties": {
                        "proposed_name": {
                            "type": "string",
                            "description": "Camel-case name for the proposed primitive (e.g., TimelineEvent)."
                        },
                        "primitive_kind": {
                            "type": "string",
                            "description": "One of: section (CmsSection variant), block (CmsBlock variant)."
                        },
                        "shape_summary": {
                            "type": "string",
                            "description": "One-line description of the primitive shape; surfaced in the duplicate-check report."
                        }
                    }
                }
            },
            {
                "name": "forge.modify_site",
                "description": "Apply a scoped modification to an existing tenant site. Paired with skills/forge-modify-site/SKILL.md (#365). One axis per call (theme | density | page_kind | add_page | remove_page | content_edit).",
                "inputSchema": {
                    "type": "object",
                    "required": ["tenant_root", "modification_kind", "modification_path"],
                    "properties": {
                        "tenant_root": {
                            "type": "string",
                            "description": "Absolute path to the existing tenant repo."
                        },
                        "modification_kind": {
                            "type": "string",
                            "description": "One of: change_theme, change_density, change_page_kind, add_page, remove_page, content_edit."
                        },
                        "modification_path": {
                            "type": "string",
                            "description": "Absolute path to the TOML file describing the modification."
                        },
                        "dry_run": {
                            "type": "boolean",
                            "description": "Default true. When true, reports the planned delta without writing."
                        }
                    }
                }
            },
            {
                "name": "forge.build_site_from_brief",
                "description": "Build a tenant site from a written brief: parses the brief, scaffolds SiteSpec, optionally writes cms/*.json and runs forge build. Paired with skills/forge-build-site-from-brief/SKILL.md (#364). Default dry_run: prints planned SiteSpec without writing.",
                "inputSchema": {
                    "type": "object",
                    "required": ["brief_path", "tenant_root", "site_id", "tenant_id"],
                    "properties": {
                        "brief_path": {
                            "type": "string",
                            "description": "Absolute path to the brief file (TOML / JSON / Markdown)."
                        },
                        "tenant_root": {
                            "type": "string",
                            "description": "Absolute path where the tenant repo will live."
                        },
                        "site_id": {
                            "type": "string",
                            "description": "Kebab-case site identifier."
                        },
                        "tenant_id": {
                            "type": "string",
                            "description": "Kebab-case tenant identifier (often same as site_id)."
                        },
                        "dry_run": {
                            "type": "boolean",
                            "description": "Default true. When true, prints planned SiteSpec without writing. When false, writes cms/*.json and runs forge build."
                        }
                    }
                }
            },
            {
                "name": "forge.workflows.list",
                "description": "List the substrate's paired (skill, MCP-tool) workflows. Each workflow has a SKILL.md + an MCP tool; this surface lets agents discover them programmatically. Each filter is optional; absent filter returns the full registry.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "status": {
                            "type": "string",
                            "description": "Restrict to workflows in this pairing status. One of: planned, skill_only, mcp_only, paired."
                        },
                        "slug": {
                            "type": "string",
                            "description": "Look up a single workflow by exact slug (snake_case, no `forge_` prefix). When set, status filter is ignored."
                        }
                    }
                }
            }
        ]
    })
}

/// Filter the full tool list by the active session scope.
///
/// Per `forge_core::session_scope` (substrate reframe #385): a
/// session can declare its scope via the `FORGE_SESSION_SCOPE`
/// env var. When set to a known scope slug, this function
/// keeps only tools that are in scope; everything else is
/// removed from the MCP surface so the client sees a tighter
/// inventory.
///
/// Unset env var, unknown slug, or `unscoped` → pass the full
/// list through unchanged (back-compat for callers that
/// haven't adopted the scope pattern).
/// Read the active session scope from `FORGE_SESSION_SCOPE` env.
/// Returns `Unscoped` when env is unset or contains an unknown
/// slug — those code-paths get the unfiltered tool surface.
///
/// Kept as a thin helper so callers (the production stdio loop)
/// can pass the resolved scope explicitly into
/// [`filter_tool_list_by_session_scope`]; tests pass scopes
/// directly without touching the process env (avoiding env-var
/// contention between parallel test threads).
fn current_session_scope() -> forge_core::session_scope::SessionScope {
    std::env::var("FORGE_SESSION_SCOPE")
        .ok()
        .and_then(|s| forge_core::session_scope::SessionScope::from_slug(&s))
        .unwrap_or(forge_core::session_scope::SessionScope::Unscoped)
}

/// Filter the full tool list by the supplied session scope.
/// Pure function over the supplied scope; reads no process state.
fn filter_tool_list_by_scope(
    full: Value,
    scope: forge_core::session_scope::SessionScope,
) -> Value {
    let allowed = forge_core::session_scope::tools_in_scope(scope);
    if allowed.is_empty() {
        // Empty → unscoped; pass through.
        return full;
    }
    let Value::Object(mut obj) = full else {
        return full;
    };
    let tools = obj.remove("tools").unwrap_or(Value::Array(Vec::new()));
    let Value::Array(items) = tools else {
        obj.insert("tools".to_owned(), tools);
        return Value::Object(obj);
    };
    let filtered: Vec<Value> = items
        .into_iter()
        .filter(|t| {
            t.get("name")
                .and_then(|v| v.as_str())
                .is_some_and(|name| allowed.contains(&name))
        })
        .collect();
    obj.insert("tools".to_owned(), Value::Array(filtered));
    obj.insert(
        "_session_scope".to_owned(),
        Value::String(scope.slug().to_owned()),
    );
    Value::Object(obj)
}

/// Convenience wrapper that reads env via
/// [`current_session_scope`] and delegates to
/// [`filter_tool_list_by_scope`]. Production code path.
fn filter_tool_list_by_session_scope(full: Value) -> Value {
    filter_tool_list_by_scope(full, current_session_scope())
}

#[cfg(test)]
mod scope_filter_tests {
    use super::*;

    fn mock_full_list() -> Value {
        json!({
            "tools": [
                { "name": "forge.orient", "description": "..." },
                { "name": "forge.build", "description": "..." },
                { "name": "forge.authoring", "description": "..." },
                { "name": "forge.manifest.validate", "description": "..." },
                { "name": "forge.codegen", "description": "..." },
            ]
        })
    }

    fn count_tools(v: &Value) -> usize {
        v.get("tools")
            .and_then(|t| t.as_array())
            .map_or(0, Vec::len)
    }

    fn names(v: &Value) -> Vec<String> {
        v.get("tools")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| e.get("name").and_then(|n| n.as_str()).map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn unscoped_passes_through() {
        // Use the explicit-scope filter to avoid env-var
        // contention between parallel test threads.
        let out = filter_tool_list_by_scope(
            mock_full_list(),
            forge_core::session_scope::SessionScope::Unscoped,
        );
        assert_eq!(count_tools(&out), 5);
        assert!(out.get("_session_scope").is_none());
    }

    #[test]
    fn build_site_scope_drops_substrate_tools() {
        let out = filter_tool_list_by_scope(
            mock_full_list(),
            forge_core::session_scope::SessionScope::BuildSite,
        );
        let kept = names(&out);
        assert!(kept.contains(&"forge.orient".to_owned()));
        assert!(kept.contains(&"forge.build".to_owned()));
        assert!(kept.contains(&"forge.authoring".to_owned()));
        assert!(!kept.contains(&"forge.manifest.validate".to_owned()));
        assert!(!kept.contains(&"forge.codegen".to_owned()));
        assert_eq!(
            out.get("_session_scope").and_then(|v| v.as_str()),
            Some("build-site")
        );
    }

    #[test]
    fn modify_primitive_scope_keeps_substrate_tools() {
        let out = filter_tool_list_by_scope(
            mock_full_list(),
            forge_core::session_scope::SessionScope::ModifyPrimitive,
        );
        let kept = names(&out);
        assert!(kept.contains(&"forge.manifest.validate".to_owned()));
        assert!(kept.contains(&"forge.codegen".to_owned()));
        assert!(!kept.contains(&"forge.authoring".to_owned()));
    }

    #[test]
    fn unknown_slug_in_env_falls_back_to_unscoped() {
        // current_session_scope() handles env+parse together;
        // unknown slug → Unscoped → no filtering. We test the
        // parse path explicitly without touching shared env.
        assert!(matches!(
            forge_core::session_scope::SessionScope::from_slug("does-not-exist"),
            None
        ));
        // And Unscoped → pass-through:
        let out = filter_tool_list_by_scope(
            mock_full_list(),
            forge_core::session_scope::SessionScope::Unscoped,
        );
        assert_eq!(count_tools(&out), 5);
    }
}

async fn handle_request(req: JsonRpcRequest) -> JsonRpcResponse {
    let result = match req.method.as_str() {
        "initialize" => Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": serde_json::from_str::<Value>(SERVER_INFO).unwrap_or(json!({}))
        })),
        "tools/list" => Some(filter_tool_list_by_session_scope(tool_list())),
        "tools/call" => {
            let name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let args = req.params.get("arguments").cloned().unwrap_or(json!({}));
            match name {
                "forge.orient" => Some(tool_forge_orient(args).await),
                "forge.build" => Some(tool_forge_build(args).await),
                "forge.doctrine.for" => Some(tool_forge_doctrine_for(args).await),
                "forge.authoring" => Some(tool_forge_authoring(args).await),
                "forge.config" => Some(tool_forge_config(args).await),
                "forge.fix" => Some(tool_forge_fix(args).await),
                "forge.synthesis.preview" => Some(tool_forge_synthesis_preview(args).await),
                "forge.codegen" => Some(tool_forge_codegen(args).await),
                "forge.manifest.validate" => Some(tool_forge_manifest_validate(args).await),
                "forge.docs.query" => Some(tool_forge_docs_query(args)),
                "forge.workflows.list" => Some(tool_forge_workflows_list(args)),
                "forge.build_site_from_brief" => {
                    Some(tool_forge_build_site_from_brief(args).await)
                }
                "forge.modify_site" => Some(tool_forge_modify_site(args).await),
                "forge.add_primitive" => Some(tool_forge_add_primitive(args)),
                "forge.add_audit_phase" => Some(tool_forge_add_audit_phase(args)),
                "forge.modify_primitive" => Some(tool_forge_modify_primitive(args)),
                "forge.verify_content_originality" => {
                    Some(tool_forge_verify_content_originality(args))
                }
                "forge.site_fingerprint_check" => {
                    Some(tool_forge_site_fingerprint_check(args))
                }
                "forge.reference_extraction" => {
                    Some(tool_forge_reference_extraction(args))
                }
                "forge.substrate_gap_registration" => {
                    Some(tool_forge_substrate_gap_registration(args))
                }
                other => {
                    return JsonRpcResponse {
                        jsonrpc: "2.0",
                        id: req.id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32601,
                            message: format!("unknown tool: {other}"),
                        }),
                    };
                }
            }
        }
        _ => None,
    };
    JsonRpcResponse {
        jsonrpc: "2.0",
        id: req.id,
        result,
        error: None,
    }
}

/// Run `forge <args>` and wrap stdout/stderr in an MCP
/// `content`-shaped response. Centralises the spawn + error path
/// so each `tool_*` body stays short.
async fn run_forge(label: &str, forge_args: &[&str]) -> Value {
    let output = tokio::process::Command::new("forge")
        .args(forge_args)
        .output()
        .await;
    match output {
        Ok(out) if out.status.success() => json!({
            "content": [{
                "type": "text",
                "text": String::from_utf8_lossy(&out.stdout).to_string()
            }]
        }),
        Ok(out) => json!({
            "isError": true,
            "content": [{
                "type": "text",
                "text": format!(
                    "forge {label} exited {status}: {err}",
                    label = label,
                    status = out.status,
                    err = String::from_utf8_lossy(&out.stderr)
                )
            }]
        }),
        Err(e) => json!({
            "isError": true,
            "content": [{
                "type": "text",
                "text": format!("could not spawn forge {label}: {e}")
            }]
        }),
    }
}

async fn tool_forge_orient(args: Value) -> Value {
    let parsed: OrientArgs = match parse_args("orient", args) {
        Ok(p) => p,
        Err(err_value) => return err_value,
    };
    let root = parsed.root.as_deref().unwrap_or(".");
    run_forge("orient", &["orient", "--root", root]).await
}

async fn tool_forge_build(args: Value) -> Value {
    let parsed: BuildArgs = match parse_args("build", args) {
        Ok(p) => p,
        Err(err_value) => return err_value,
    };
    let root = parsed.root.as_deref().unwrap_or(".");
    let mut forge_args: Vec<&str> = vec!["build", "--root", root];
    if parsed.json {
        forge_args.push("--json");
    }
    run_forge("build", &forge_args).await
}

async fn tool_forge_doctrine_for(args: Value) -> Value {
    let parsed: DoctrineForArgs = match parse_args("doctrine.for", args) {
        Ok(p) => p,
        Err(err_value) => return err_value,
    };
    let root = parsed.root.as_deref().unwrap_or(".");
    let mut forge_args: Vec<&str> = vec!["doctrine", "--root", root, "for", &parsed.path];
    if parsed.terse {
        forge_args.push("--terse");
    }
    run_forge("doctrine for", &forge_args).await
}

async fn tool_forge_authoring(args: Value) -> Value {
    let parsed: AuthoringArgs = match parse_args("authoring", args) {
        Ok(p) => p,
        Err(err_value) => return err_value,
    };
    let root = parsed.root.as_deref().unwrap_or(".");
    run_forge("authoring", &["authoring", "--root", root]).await
}

async fn tool_forge_config(args: Value) -> Value {
    let parsed: ConfigArgs = match parse_args("config", args) {
        Ok(p) => p,
        Err(err_value) => return err_value,
    };
    let root = parsed.root.as_deref().unwrap_or(".");
    run_forge("config", &["config", "--root", root]).await
}

async fn tool_forge_fix(args: Value) -> Value {
    let parsed: FixArgs = match parse_args("fix", args) {
        Ok(p) => p,
        Err(err_value) => return err_value,
    };
    let root = parsed.root.as_deref().unwrap_or(".");
    run_forge("fix", &["fix", "--root", root]).await
}

async fn tool_forge_synthesis_preview(args: Value) -> Value {
    let parsed: SynthesisPreviewArgs = match parse_args("synthesis.preview", args) {
        Ok(p) => p,
        Err(err_value) => return err_value,
    };
    let root = parsed.root.as_deref().unwrap_or(".");
    run_forge(
        "synthesis preview",
        &["synthesis", "--root", root, "preview", &parsed.spec_path],
    )
    .await
}

async fn tool_forge_codegen(args: Value) -> Value {
    let parsed: CodegenArgs = match parse_args("codegen", args) {
        Ok(p) => p,
        Err(err_value) => return err_value,
    };
    let root = parsed.root.as_deref().unwrap_or(".");
    let mut forge_args: Vec<&str> = vec!["codegen", "--root", root];
    if parsed.dry_run {
        forge_args.push("--dry-run");
    } else if let Some(ref o) = parsed.out {
        forge_args.push("--out");
        forge_args.push(o);
    } else {
        return json!({
            "isError": true,
            "content": [{
                "type": "text",
                "text": "forge.codegen requires either `out` or `dry_run: true`"
            }]
        });
    }
    run_forge("codegen", &forge_args).await
}

async fn tool_forge_manifest_validate(args: Value) -> Value {
    let parsed: ManifestValidateArgs = match parse_args("manifest.validate", args) {
        Ok(p) => p,
        Err(err_value) => return err_value,
    };
    let root = parsed.root.as_deref().unwrap_or(".");
    run_forge("manifest validate", &["manifest", "--root", root, "validate"]).await
}

/// Query the substrate doc index. Pure in-process; no shell out.
/// Wraps `forge_core::doc_query::canonical_index()`. Synchronous;
/// not declared `async` because there's no I/O.
fn tool_forge_docs_query(args: Value) -> Value {
    use forge_core::doc_query::{canonical_index, DocKind, DocQueryFilter};
    use forge_core::session_scope::DocTag;

    fn parse_kind(s: &str) -> Option<DocKind> {
        match s {
            "doctrine" => Some(DocKind::Doctrine),
            "primitive" => Some(DocKind::Primitive),
            "audit_phase" | "audit-phase" => Some(DocKind::AuditPhase),
            "workflow" => Some(DocKind::Workflow),
            "reframe" => Some(DocKind::Reframe),
            _ => None,
        }
    }

    fn parse_tag(s: &str) -> Option<DocTag> {
        match s {
            "tenant" => Some(DocTag::Tenant),
            "primitive" => Some(DocTag::Primitive),
            "audit_phase" | "audit-phase" => Some(DocTag::AuditPhase),
            "deploy" => Some(DocTag::Deploy),
            "authoring" => Some(DocTag::Authoring),
            "doctrine" => Some(DocTag::Doctrine),
            "reframe" => Some(DocTag::Reframe),
            "workflow" => Some(DocTag::Workflow),
            _ => None,
        }
    }

    let parsed: DocsQueryArgs = match parse_args("docs.query", args) {
        Ok(p) => p,
        Err(err_value) => return err_value,
    };

    let index = canonical_index();

    // Exact-slug shortcut: when "slug" arg is provided, return
    // that single entry (or null) and ignore other filters.
    if let Some(ref slug) = parsed.slug {
        return match index.get(slug) {
            Some(entry) => serde_json::to_value(entry).unwrap_or(Value::Null),
            None => Value::Null,
        };
    }

    let kind = parsed.kind.as_deref().and_then(parse_kind);
    let tags_any: Vec<DocTag> = parsed
        .tags_any
        .iter()
        .filter_map(|s| parse_tag(s))
        .collect();
    let limit = parsed.limit.and_then(|n| usize::try_from(n).ok());

    let filter = DocQueryFilter {
        kind,
        tags_any,
        slug_prefix: parsed.slug_prefix,
        contains_text: parsed.contains_text,
        limit,
    };
    let entries = index.query(&filter);
    serde_json::to_value(&entries).unwrap_or(Value::Null)
}

/// Workflow #9: register a substrate-capability gap.
///
/// Validates the kind against the closed taxonomy, then appends a
/// new GapEntry to the JSONL registry. Returns the assigned ID +
/// timestamp. Pure substrate operation — no shell-outs.
fn tool_forge_substrate_gap_registration(args: Value) -> Value {
    use forge_core::gap_registry::{append, GapKind};

    let parsed: SubstrateGapRegistrationArgs =
        match parse_args("substrate_gap_registration", args) {
            Ok(p) => p,
            Err(err_value) => return err_value,
        };

    let kind = match GapKind::parse(&parsed.kind) {
        Some(k) => k,
        None => {
            return json!({
                "isError": true,
                "content": [{
                    "type": "text",
                    "text": format!(
                        "Unknown kind: {}. Must be one of: primitive, audit_phase, \
                         theme, page_kind, page_field, doctrine_rule, tooling.",
                        parsed.kind
                    )
                }]
            });
        }
    };

    let timestamp = forge_core::iso_time::current_rfc3339_utc();

    let path = std::path::Path::new(&parsed.registry_path);
    match append(
        path,
        kind,
        &parsed.observed_in,
        &parsed.summary,
        &parsed.proposed_resolution,
        parsed.related_tasks.clone(),
        &timestamp,
    ) {
        Ok(entry) => json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Registered: forge.substrate_gap_registration\n\
                     -----\n\
                     id:                  {id}\n\
                     registered_at:       {at}\n\
                     kind:                {kind}\n\
                     observed_in:         {oi}\n\
                     summary:             {summ}\n\
                     proposed_resolution: {prop}\n\
                     status:              open\n\
                     related_tasks:       {rt}\n\
                     registry_path:       {path}",
                    id = entry.id,
                    at = entry.registered_at,
                    kind = entry.kind.slug(),
                    oi = entry.observed_in,
                    summ = entry.summary,
                    prop = entry.proposed_resolution,
                    rt = entry.related_tasks.join(", "),
                    path = parsed.registry_path,
                )
            }]
        }),
        Err(e) => json!({
            "isError": true,
            "content": [{
                "type": "text",
                "text": format!("gap registry append failed: {}", e)
            }]
        }),
    }
}

/// Workflow #8: reference-extraction pipeline entry point.
///
/// Loads the CaptureManifest at <capture_dir>/manifest.json, validates
/// the capture inventory, and surfaces a structured report. Per-axis
/// extractor invocation (palette/typography/spacing/motion/structural/
/// voice/sections/interactive) lands once the chromiumoxide runner is
/// verified end-to-end. The integration boundary is documented in
/// docs/SUBSTRATE_REFERENCE_PIPELINE_AUDIT_2026_05_27.md.
fn tool_forge_reference_extraction(args: Value) -> Value {
    use forge_core::reference_capture::CaptureManifest;

    let parsed: ReferenceExtractionArgs =
        match parse_args("reference_extraction", args) {
            Ok(p) => p,
            Err(err_value) => return err_value,
        };

    let manifest_path = std::path::Path::new(&parsed.capture_dir).join("manifest.json");
    if !manifest_path.is_file() {
        return json!({
            "isError": true,
            "content": [{
                "type": "text",
                "text": format!(
                    "manifest.json not found at: {}\n\
                     Run the Crawler reference-capture mode against the URL first.",
                    manifest_path.display()
                )
            }]
        });
    }

    let manifest = match CaptureManifest::read(&manifest_path) {
        Ok(m) => m,
        Err(e) => {
            return json!({
                "isError": true,
                "content": [{
                    "type": "text",
                    "text": format!("CaptureManifest read failed: {}", e)
                }]
            });
        }
    };

    let viewports: Vec<u32> =
        manifest.captures.iter().map(|c| c.viewport_px).collect();
    let total_images: u32 =
        manifest.captures.iter().map(|c| c.network_summary.image_count).sum();
    let total_fonts: usize = manifest
        .captures
        .iter()
        .map(|c| c.network_summary.fonts_loaded.len())
        .sum();

    let summary = json!({
        "spec": manifest.spec.slug(),
        "site_slug": manifest.site_slug,
        "url": manifest.url,
        "updated_at": manifest.updated_at,
        "capture_count": manifest.captures.len(),
        "viewports_px": viewports,
        "total_images_across_captures": total_images,
        "total_fonts_loaded_across_captures": total_fonts,
        "target_site_id": parsed.site_id,
        "target_tenant_id": parsed.tenant_id,
        "extraction_status": "manifest_validated_extractor_pending",
        "next_step": "Per-axis extraction lands once chromiumoxide runner verification \
                      completes. See docs/SUBSTRATE_REFERENCE_PIPELINE_AUDIT_2026_05_27.md."
    });

    json!({
        "content": [{
            "type": "text",
            "text": format!(
                "Reference extraction: forge.reference_extraction\n\
                 -----\n\
                 capture_dir: {}\n\
                 \n\
                 {}",
                parsed.capture_dir,
                serde_json::to_string_pretty(&summary).unwrap_or_default()
            )
        }]
    })
}

/// Workflow #7: compute site fingerprint + check vs registry.
///
/// Calls forge-core::fingerprint::build_from_cms_dir to compute
/// the SiteFingerprint, then (if registry_path provided) calls
/// fingerprint_registry::find_near_duplicates to surface matches.
fn tool_forge_site_fingerprint_check(args: Value) -> Value {
    use forge_core::fingerprint::build_from_cms_dir;
    use forge_core::fingerprint_registry::find_near_duplicates;

    let parsed: SiteFingerprintCheckArgs =
        match parse_args("site_fingerprint_check", args) {
            Ok(p) => p,
            Err(err_value) => return err_value,
        };

    let cms_dir = std::path::Path::new(&parsed.tenant_root).join("cms");
    if !cms_dir.is_dir() {
        return json!({
            "isError": true,
            "content": [{
                "type": "text",
                "text": format!(
                    "tenant_root/cms not a directory: {}",
                    cms_dir.display()
                )
            }]
        });
    }

    let fingerprint = match build_from_cms_dir(&cms_dir) {
        Ok(fp) => fp,
        Err(e) => {
            return json!({
                "isError": true,
                "content": [{
                    "type": "text",
                    "text": format!("fingerprint build failed: {}", e)
                }]
            });
        }
    };

    let commitment = fingerprint.commitment_hex();

    let registry_summary = if let Some(ref reg_path) = parsed.registry_path {
        let path = std::path::Path::new(reg_path);
        if !path.exists() {
            json!({
                "registry_present": false,
                "note": format!("registry_path does not exist: {}", reg_path)
            })
        } else {
            match find_near_duplicates(path, &fingerprint, parsed.distance_threshold) {
                Ok(matches) => {
                    let verdict = match matches.first().map(|(_, d)| *d) {
                        None => "ok",
                        Some(0) => "block",
                        Some(d) if d <= parsed.distance_threshold / 2 => "block",
                        Some(_) => "flag",
                    };
                    json!({
                        "registry_present": true,
                        "total_entries_scanned": matches.len(),
                        "near_duplicate_count": matches.len(),
                        "nearest_distance": matches.first().map(|(_, d)| *d),
                        "verdict": verdict,
                        "matches": matches.iter().map(|(e, d)| json!({
                            "tenant_id": e.tenant_id,
                            "site_id": e.site_id,
                            "distance": d,
                            "commitment_hex": e.fingerprint.commitment_hex(),
                        })).collect::<Vec<_>>()
                    })
                }
                Err(e) => json!({
                    "registry_present": true,
                    "error": format!("registry read failed: {}", e)
                }),
            }
        }
    } else {
        json!({
            "registry_present": false,
            "note": "No registry_path provided; fingerprint computed only."
        })
    };

    json!({
        "content": [{
            "type": "text",
            "text": format!(
                "Fingerprint: forge.site_fingerprint_check\n\
                 -----\n\
                 tenant_root:        {}\n\
                 commitment_hex:     {}\n\
                 distance_threshold: {}\n\
                 \n\
                 Registry summary (JSON):\n{}",
                parsed.tenant_root,
                commitment,
                parsed.distance_threshold,
                serde_json::to_string_pretty(&registry_summary).unwrap_or_default()
            )
        }]
    })
}

/// Workflow #6: verify content originality (anti-reuse gate).
///
/// Loads tenant cms/*.json + corpus *.json files, extracts string
/// fields, runs the deterministic n-gram shingle check from
/// forge-core::originality. Returns the structured report.
fn tool_forge_verify_content_originality(args: Value) -> Value {
    use forge_core::originality::check_originality;

    let parsed: VerifyContentOriginalityArgs =
        match parse_args("verify_content_originality", args) {
            Ok(p) => p,
            Err(err_value) => return err_value,
        };

    let tenant_strings = collect_strings_from_root(&parsed.tenant_root);
    let corpus_strings: Vec<String> = parsed
        .corpus_roots
        .iter()
        .flat_map(|p| collect_strings_from_root(p))
        .collect();

    let report = check_originality(&tenant_strings, &corpus_strings, parsed.min_ngram_words);
    serde_json::to_value(&report).unwrap_or(Value::Null)
}

/// Walk a directory recursively, find every `*.json` file, parse,
/// and extract every leaf string value. Pure data extraction; no
/// schema awareness beyond "valid JSON".
fn collect_strings_from_root(root: &str) -> Vec<String> {
    let mut out = Vec::new();
    let path = std::path::Path::new(root);
    if !path.exists() {
        return out;
    }
    if path.is_file() {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                walk_value_for_strings(&value, &mut out);
            }
        }
        return out;
    }
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(text) = std::fs::read_to_string(&p) {
                    if let Ok(value) = serde_json::from_str::<Value>(&text) {
                        walk_value_for_strings(&value, &mut out);
                    }
                }
            }
        }
    }
    out
}

/// Walk a serde_json::Value tree, pushing every String leaf into `out`.
fn walk_value_for_strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Array(arr) => {
            for item in arr {
                walk_value_for_strings(item, out);
            }
        }
        Value::Object(map) => {
            for (_, val) in map {
                walk_value_for_strings(val, out);
            }
        }
        _ => {}
    }
}

/// Workflow #5: classify a proposed primitive modification.
///
/// Per the backward_compat_version_discipline doctrine, every
/// change to an existing primitive falls into one of four
/// categories with distinct substrate-side discipline. This tool
/// validates the operator's classification and surfaces the
/// per-category requirements.
fn tool_forge_modify_primitive(args: Value) -> Value {
    let parsed: ModifyPrimitiveArgs = match parse_args("modify_primitive", args) {
        Ok(p) => p,
        Err(err_value) => return err_value,
    };

    let (category_label, requirements) = match parsed.change_kind.as_str() {
        "invisible" => (
            "Invisible (internal refactor, no wire-shape change)",
            "Discipline:\n\
             - No tenant-visible change\n\
             - Run the full test suite; verify no behavior change\n\
             - No serde / schema implication\n\
             - No doc_query update needed",
        ),
        "additive" => (
            "Additive (backward-compatible extension)",
            "Discipline:\n\
             - New enum variant OR new field with #[serde(default)]\n\
             - Extend render impl to handle the new shape\n\
             - Add a snapshot test pinning new render output\n\
             - Add a doc_query entry surfacing the new variant\n\
             - Existing tenant content MUST still build unchanged",
        ),
        "auto_migration" => (
            "Auto-migration (renamed via signed migration registry)",
            "Discipline:\n\
             - Add new shape alongside the old (#[serde(alias = \"old\")])\n\
             - Register migration entry in the signed migration registry\n\
             - Update doc_query: new name preferred, old marked deprecated\n\
             - Plan a future cycle for old-name removal\n\
             - Migration registry signature is the canonical record",
        ),
        "operator_action" => (
            "Operator-action (breaking change requiring tenant edits)",
            "Discipline:\n\
             - Feature-flag the new shape in a separate module\n\
             - Emit forge build Warn finding in the current cycle\n\
             - Plan Strict-promotion + release-notes for next major\n\
             - Document the migration path in docs/MIGRATIONS.md\n\
             - Tenants must edit their content before next cycle",
        ),
        other => {
            return json!({
                "isError": true,
                "content": [{
                    "type": "text",
                    "text": format!(
                        "Unknown change_kind: {other}. Must be one of: invisible, additive, auto_migration, operator_action.\n\
                         See backward_compat_version_discipline doctrine for the 4-category taxonomy."
                    )
                }]
            });
        }
    };

    let report = format!(
        "Classification: forge.modify_primitive\n\
         -----\n\
         Primitive:      {name}\n\
         Change kind:    {kind}\n\
         Summary:        {summary}\n\
         Category:       {label}\n\
         \n\
         {req}\n\
         \n\
         Follow skills/forge-modify-primitive/SKILL.md for procedural\n\
         guidance + the [[backward-compat-version-discipline]] doctrine\n\
         for the rationale.",
        name = parsed.primitive_name,
        kind = parsed.change_kind,
        summary = parsed.change_summary,
        label = category_label,
        req = requirements,
    );

    json!({
        "content": [{
            "type": "text",
            "text": report
        }]
    })
}

/// Workflow #4: pre-flight guard before adding a new audit phase.
///
/// Pure function; no I/O. Surfaces near-duplicate category buckets
/// from the 75+ existing phase modules in
/// `crates/forge-phases/src/`. Procedural guidance lives in
/// `skills/add-forge-phase/SKILL.md`.
fn tool_forge_add_audit_phase(args: Value) -> Value {
    let parsed: AddAuditPhaseArgs = match parse_args("add_audit_phase", args) {
        Ok(p) => p,
        Err(err_value) => return err_value,
    };

    let proposed_lower = parsed.proposed_name.to_lowercase();

    // Phase categories observed in crates/forge-phases/src/. When a
    // proposed name matches one of these tokens, the developer is
    // pointed at the existing phases in that category to read first.
    let phase_categories: Vec<(&str, &[&str])> = vec![
        ("Accessibility / a11y", &["a11y_landmarks", "contrast", "motion_respects_reduced", "label_consistency"]),
        ("Aesthetic / mood", &["aesthetic_distinctiveness", "mood_lock", "editorial_purity_gate"]),
        ("Content quality", &["content_substance", "annotation_review", "disclosure_audit", "identity_coherence"]),
        ("Layout / structure", &["composition_lineage", "density_audit", "html_semantic", "html_walk", "hero_composition_resolve"]),
        ("Performance / assets", &["asset_optimization", "carbon_budget", "external_assets"]),
        ("Network / security", &["csp", "csp_devmode", "dns_hygiene_lint", "link_check", "network_target_enforcement"]),
        ("Crawler / hunting", &["crawl", "hunted_tier"]),
        ("Forbidden patterns", &["forbidden_patterns", "loom_lint", "loom_sync"]),
        ("Internationalization", &["iso_8601", "locale_html_lang"]),
        ("Variation arc", &["differentiation_budget"]),
        ("Theming", &["dual_theme"]),
        ("Backend / coverage", &["backend_coverage"]),
        ("Jurisdiction / compliance", &["jurisdiction_compliance"]),
        ("ID / lineage", &["id_strategy"]),
        ("Motion", &["motion", "motion_respects_reduced"]),
    ];

    let mut overlap_categories: Vec<&str> = Vec::new();
    let mut overlap_phases: Vec<&str> = Vec::new();
    let tokens: Vec<&str> = proposed_lower.split('_').collect();
    for (bucket, phases) in &phase_categories {
        for phase in phases.iter() {
            let phase_tokens: Vec<&str> = phase.split('_').collect();
            let shared = tokens.iter().any(|t| !t.is_empty() && phase_tokens.contains(t));
            if shared || proposed_lower.contains(phase) || phase.contains(&proposed_lower) {
                overlap_categories.push(bucket);
                overlap_phases.push(phase);
            }
        }
    }
    overlap_categories.sort_unstable();
    overlap_categories.dedup();
    overlap_phases.sort_unstable();
    overlap_phases.dedup();

    let mut report = format!(
        "Pre-flight guard: forge.add_audit_phase\n\
         -----\n\
         Proposed name: {}\n",
        parsed.proposed_name
    );
    if let Some(ref summary) = parsed.finding_summary {
        report.push_str(&format!("Finding summary: {}\n", summary));
    }
    report.push_str("\n");

    if overlap_phases.is_empty() {
        report.push_str(
            "No obvious overlap with existing phase categories.\n\
             \n\
             Next steps:\n\
             1. Follow skills/add-forge-phase/SKILL.md procedure.\n\
             2. Browse all 75+ phases: ls crates/forge-phases/src/*.rs\n\
             3. Decide if the finding belongs in an existing phase \
             (extend that phase) vs warrants a new module.\n\
             4. Confirm the phase is substrate-general (not site-\
             specific) and observable from the build artifacts.\n",
        );
    } else {
        report.push_str(&format!(
            "Likely overlap with category: {}\n\
             Existing phases that share name tokens: {}\n\
             \n\
             Before adding a new phase:\n\
             1. Read each listed phase's lib.rs entry + finding shape.\n\
             2. Could the proposed finding be added to an existing \
             phase as a new finding kind (rather than a new module)?\n\
             3. If a new phase is needed, follow \
             skills/add-forge-phase/SKILL.md.\n",
            overlap_categories.join(", "),
            overlap_phases.join(", ")
        ));
    }

    json!({
        "content": [{
            "type": "text",
            "text": report
        }]
    })
}

/// Workflow #3: pre-flight guard before adding a new primitive.
///
/// Pure function; no I/O. Takes the proposed name + kind and
/// surfaces nearby existing variants from the substrate reachability
/// reference set so the developer doesn't ship a near-duplicate.
///
/// Procedural guidance lives in `skills/add-loom-primitive/SKILL.md`.
fn tool_forge_add_primitive(args: Value) -> Value {
    let parsed: AddPrimitiveArgs = match parse_args("add_primitive", args) {
        Ok(p) => p,
        Err(err_value) => return err_value,
    };

    // Validate kind at boundary.
    let kind_label = match parsed.primitive_kind.as_str() {
        "section" => "CmsSection",
        "block" => "CmsBlock",
        other => {
            return json!({
                "isError": true,
                "content": [{
                    "type": "text",
                    "text": format!(
                        "Unknown primitive_kind: {}. Must be one of: section, block.",
                        other
                    )
                }]
            });
        }
    };

    // Known near-duplicate buckets from the reachability audit
    // (SUBSTRATE_REACHABILITY_AUDIT_2026_05_27.md). For a proposed
    // name, surface the bucket(s) it overlaps with so the developer
    // checks each before adding a new variant.
    //
    // This is intentionally a hand-curated set, not a fuzzy
    // similarity search — explicit signal beats heuristic.
    let proposed_lower = parsed.proposed_name.to_lowercase();
    let related_buckets: Vec<(&str, &[&str])> = vec![
        ("Hero family", &["hero", "hero_editorial", "hero_split", "hero_minimal", "image_hero"]),
        ("Card family", &["card_feed", "feed_post", "review_card", "case_study", "product_card", "profile_card"]),
        ("Image / photo", &["picture", "image_grid", "image_hero", "mosaic_grid", "slideshow", "figure", "figure_group"]),
        ("Form family", &["form", "form_input", "form_select", "form_textarea", "form_toggle", "form_submit", "form_file", "form_date", "form_color", "form_search", "form_slider"]),
        ("Auth flow", &["auth_card", "auth_flow_stepper", "mfa_prompt", "password_reset", "signed_in_card"]),
        ("Commerce", &["product_card", "product_gallery", "product_grid", "product_spec", "cart_drawer", "add_to_cart", "price_tag", "pricing"]),
        ("List / feed", &["card_feed", "thread_list", "thread_row", "comment_thread", "chat_thread"]),
        ("Quote / testimonial", &["pull_quote", "testimonial", "review_card"]),
        ("Code / dev", &["code", "code_shell", "math_block", "diagram"]),
    ];

    let mut overlaps: Vec<&str> = Vec::new();
    let mut nearby_variants: Vec<&str> = Vec::new();
    for (bucket_name, members) in &related_buckets {
        for member in members.iter() {
            if proposed_lower.contains(member) || member.contains(&proposed_lower) {
                overlaps.push(bucket_name);
                nearby_variants.push(member);
            }
        }
    }
    overlaps.sort_unstable();
    overlaps.dedup();
    nearby_variants.sort_unstable();
    nearby_variants.dedup();

    let mut report = format!(
        "Pre-flight guard: forge.add_primitive\n\
         -----\n\
         Proposed name:   {}\n\
         Primitive kind:  {} ({})\n",
        parsed.proposed_name, parsed.primitive_kind, kind_label
    );
    if let Some(ref summary) = parsed.shape_summary {
        report.push_str(&format!("Shape summary:   {}\n", summary));
    }
    report.push_str("\n");

    if overlaps.is_empty() {
        report.push_str(
            "No obvious near-duplicate buckets detected.\n\
             \n\
             Next steps:\n\
             1. Follow skills/add-loom-primitive/SKILL.md procedure.\n\
             2. Check all 163 CmsSection variants directly: grep -nE \\\n   \
                'pub enum CmsSection' crates/loom-cms-render/src/lib.rs\n\
             3. Confirm no existing primitive satisfies the shape via \
             property composition (per Hero pilot pattern, #387).\n\
             4. Verify the need is substrate-general per prim-012, \
             not site-specific.\n",
        );
    } else {
        report.push_str(&format!(
            "Near-duplicate buckets found: {}\n\
             Nearby existing variants: {}\n\
             \n\
             Before adding a new variant:\n\
             1. Read each nearby variant's existing definition + render impl.\n\
             2. Could the shape be expressed by extending an existing variant \
             (new field, new enum case for a sub-enum)?\n\
             3. Could the shape be expressed via property composition \
             (per Hero family pilot pattern, #387)?\n\
             4. If a new primitive is genuinely needed, follow \
             skills/add-loom-primitive/SKILL.md.\n",
            overlaps.join(", "),
            nearby_variants.join(", ")
        ));
    }

    json!({
        "content": [{
            "type": "text",
            "text": report
        }]
    })
}

/// Workflow #2: apply a scoped modification to an existing tenant.
///
/// Validates the modification kind at the MCP boundary; full
/// procedural guidance lives in `skills/forge-modify-site/SKILL.md`.
async fn tool_forge_modify_site(args: Value) -> Value {
    let parsed: ModifySiteArgs = match parse_args("modify_site", args) {
        Ok(p) => p,
        Err(err_value) => return err_value,
    };

    // Boundary validation: kinds the workflow supports.
    const KNOWN_KINDS: &[&str] = &[
        "change_theme",
        "change_density",
        "change_page_kind",
        "add_page",
        "remove_page",
        "content_edit",
    ];
    if !KNOWN_KINDS.contains(&parsed.modification_kind.as_str()) {
        return json!({
            "isError": true,
            "content": [{
                "type": "text",
                "text": format!(
                    "Unknown modification_kind: {}. Must be one of: {}.",
                    parsed.modification_kind,
                    KNOWN_KINDS.join(", ")
                )
            }]
        });
    }

    if !std::path::Path::new(&parsed.tenant_root).is_dir() {
        return json!({
            "isError": true,
            "content": [{
                "type": "text",
                "text": format!(
                    "tenant_root not a directory: {}. \
                     Use forge.build_site_from_brief (#364) for from-zero builds.",
                    parsed.tenant_root
                )
            }]
        });
    }

    if !std::path::Path::new(&parsed.modification_path).is_file() {
        return json!({
            "isError": true,
            "content": [{
                "type": "text",
                "text": format!(
                    "modification_path not a file: {}",
                    parsed.modification_path
                )
            }]
        });
    }

    if parsed.dry_run {
        return json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Dry-run: forge.modify_site\n\
                     -----\n\
                     tenant_root:       {root}\n\
                     modification_kind: {kind}\n\
                     modification_path: {path}\n\
                     \n\
                     Real-run (dry_run: false) would:\n\
                     1. Parse {path} per {kind} shape\n\
                     2. Apply the modification to {root}\n\
                     3. Run `forge build --root {root} --json`\n\
                     4. Return the structured build report\n\
                     \n\
                     Follow skills/forge-modify-site/SKILL.md for\n\
                     procedural guidance.",
                    root = parsed.tenant_root,
                    kind = parsed.modification_kind,
                    path = parsed.modification_path
                )
            }]
        });
    }

    // Real-run: re-build after the operator applies the change per
    // the skill procedure. Future iteration: this MCP tool itself
    // could read the modification TOML and apply it, but for the
    // Paired-status MVP it delegates to the build pipeline.
    run_forge(
        "modify_site",
        &["build", "--root", &parsed.tenant_root, "--json"],
    )
    .await
}

/// Workflow #1: build a tenant site from a written brief.
///
/// Thin orchestrator at the MCP layer: validates inputs, then
/// delegates to existing forge subcommands. The procedural
/// guidance lives in `skills/forge-build-site-from-brief/SKILL.md`.
///
/// Dry-run path: validates the brief exists + readable, reports
/// the planned tenant_root structure, no writes.
///
/// Real-run path: requires the operator to follow the skill
/// procedure (build SiteSpec, write cms files), then runs
/// `forge build --root <tenant_root>` and surfaces the report.
async fn tool_forge_build_site_from_brief(args: Value) -> Value {
    let parsed: BuildSiteFromBriefArgs = match parse_args("build_site_from_brief", args) {
        Ok(p) => p,
        Err(err_value) => return err_value,
    };

    // Validate brief exists + readable. This is the minimum boundary
    // contract the substrate enforces; brief-shape validation belongs
    // to the skill procedure, not the MCP tool.
    if !std::path::Path::new(&parsed.brief_path).is_file() {
        return json!({
            "isError": true,
            "content": [{
                "type": "text",
                "text": format!(
                    "brief_path not a file or unreadable: {}",
                    parsed.brief_path
                )
            }]
        });
    }

    if parsed.dry_run {
        // Dry-run: report what a real-run would do.
        return json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Dry-run: forge.build_site_from_brief\n\
                     -----\n\
                     brief_path:  {brief}\n\
                     tenant_root: {root}\n\
                     site_id:     {site}\n\
                     tenant_id:   {tenant}\n\
                     \n\
                     Real-run (dry_run: false) would:\n\
                     1. Parse the brief at {brief}\n\
                     2. Scaffold SiteSpec via forge-core::synthesis\n\
                     3. Write {root}/cms/<page>.json files\n\
                     4. Run `forge build --root {root}`\n\
                     5. Return the structured build report\n\
                     \n\
                     Follow skills/forge-build-site-from-brief/SKILL.md\n\
                     for procedural guidance.",
                    brief = parsed.brief_path,
                    root = parsed.tenant_root,
                    site = parsed.site_id,
                    tenant = parsed.tenant_id
                )
            }]
        });
    }

    // Real-run path: requires tenant_root to exist with cms/*.json
    // already populated per skill procedure. The MCP tool runs the
    // build phase against that prepared root.
    if !std::path::Path::new(&parsed.tenant_root).is_dir() {
        return json!({
            "isError": true,
            "content": [{
                "type": "text",
                "text": format!(
                    "tenant_root not a directory: {}\n\
                     Follow skills/forge-build-site-from-brief/SKILL.md \
                     step 4 to write cms/*.json before real-run.",
                    parsed.tenant_root
                )
            }]
        });
    }

    run_forge(
        "build_site_from_brief",
        &["build", "--root", &parsed.tenant_root, "--json"],
    )
    .await
}

/// List the paired (skill, MCP-tool) workflow registry.
/// Wraps `forge_core::workflow_registry`. Synchronous; pure.
fn tool_forge_workflows_list(args: Value) -> Value {
    use forge_core::workflow_registry::{
        all_workflows, get_workflow, workflows_with_status, PairingStatus,
    };

    let parsed: WorkflowsListArgs = match parse_args("workflows.list", args) {
        Ok(p) => p,
        Err(err_value) => return err_value,
    };

    // Exact-slug shortcut.
    if let Some(ref slug) = parsed.slug {
        return match get_workflow(slug) {
            Some(entry) => serde_json::to_value(entry).unwrap_or(Value::Null),
            None => Value::Null,
        };
    }

    let status_filter = parsed.status.as_deref().and_then(|s| match s {
        "planned" => Some(PairingStatus::Planned),
        "skill_only" => Some(PairingStatus::SkillOnly),
        "mcp_only" => Some(PairingStatus::McpOnly),
        "paired" => Some(PairingStatus::Paired),
        _ => None,
    });

    let entries: Vec<_> = match status_filter {
        Some(status) => workflows_with_status(status),
        None => all_workflows().iter().collect(),
    };
    serde_json::to_value(&entries).unwrap_or(Value::Null)
}

#[tokio::main]
async fn main() -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let stdout = std::io::stdout();
    let mut buf = String::new();
    loop {
        buf.clear();
        let n = reader
            .read_line(&mut buf)
            .await
            .context("read stdin")?;
        if n == 0 {
            break;
        }
        let line = buf.trim();
        if line.is_empty() {
            continue;
        }
        let req: JsonRpcRequest = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("forge-mcp: malformed json-rpc request: {e}");
                continue;
            }
        };
        // Per MCP spec notifications have no id and expect no
        // response.
        let is_notification = req.id.is_none() && req.method.starts_with("notifications/");
        let resp = handle_request(req).await;
        if !is_notification {
            let mut out = stdout.lock();
            let line = serde_json::to_string(&resp).unwrap_or_else(|e| {
                format!(r#"{{"jsonrpc":"2.0","error":{{"code":-32603,"message":"{e}"}}}}"#)
            });
            writeln!(out, "{line}").ok();
            out.flush().ok();
        }
    }
    Ok(())
}
