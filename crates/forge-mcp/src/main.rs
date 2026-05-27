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
    parse_args, AuthoringArgs, BuildArgs, BuildSiteFromBriefArgs, CodegenArgs, ConfigArgs,
    DocsQueryArgs, DoctrineForArgs, FixArgs, ManifestValidateArgs, ModifySiteArgs,
    OrientArgs, SynthesisPreviewArgs, WorkflowsListArgs,
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
