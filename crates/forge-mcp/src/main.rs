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
fn filter_tool_list_by_session_scope(full: Value) -> Value {
    let scope = std::env::var("FORGE_SESSION_SCOPE")
        .ok()
        .and_then(|s| forge_core::session_scope::SessionScope::from_slug(&s))
        .unwrap_or(forge_core::session_scope::SessionScope::Unscoped);
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
    fn unscoped_env_passes_through() {
        // SAFETY: serial test; cargo test on this fn doesn't
        // run in parallel with other env-touching tests since
        // they're in the same module.
        std::env::remove_var("FORGE_SESSION_SCOPE");
        let out = filter_tool_list_by_session_scope(mock_full_list());
        assert_eq!(count_tools(&out), 5);
        assert!(out.get("_session_scope").is_none());
    }

    #[test]
    fn build_site_scope_drops_substrate_tools() {
        std::env::set_var("FORGE_SESSION_SCOPE", "build-site");
        let out = filter_tool_list_by_session_scope(mock_full_list());
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
        std::env::remove_var("FORGE_SESSION_SCOPE");
    }

    #[test]
    fn modify_primitive_scope_keeps_substrate_tools() {
        std::env::set_var("FORGE_SESSION_SCOPE", "modify-primitive");
        let out = filter_tool_list_by_session_scope(mock_full_list());
        let kept = names(&out);
        assert!(kept.contains(&"forge.manifest.validate".to_owned()));
        assert!(kept.contains(&"forge.codegen".to_owned()));
        assert!(!kept.contains(&"forge.authoring".to_owned()));
        std::env::remove_var("FORGE_SESSION_SCOPE");
    }

    #[test]
    fn unknown_scope_passes_through() {
        std::env::set_var("FORGE_SESSION_SCOPE", "does-not-exist");
        let out = filter_tool_list_by_session_scope(mock_full_list());
        // Unknown slug → SessionScope::Unscoped → empty allow →
        // pass-through.
        assert_eq!(count_tools(&out), 5);
        std::env::remove_var("FORGE_SESSION_SCOPE");
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
    let root = args
        .get("root")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    run_forge("orient", &["orient", "--root", root]).await
}

async fn tool_forge_build(args: Value) -> Value {
    let root = args
        .get("root")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    let json_mode = args.get("json").and_then(|v| v.as_bool()).unwrap_or(false);
    let mut forge_args: Vec<&str> = vec!["build", "--root", root];
    if json_mode {
        forge_args.push("--json");
    }
    run_forge("build", &forge_args).await
}

async fn tool_forge_doctrine_for(args: Value) -> Value {
    let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
        return json!({
            "isError": true,
            "content": [{
                "type": "text",
                "text": "missing required argument: path"
            }]
        });
    };
    let root = args
        .get("root")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    let terse = args.get("terse").and_then(|v| v.as_bool()).unwrap_or(true);
    let mut forge_args: Vec<&str> = vec!["doctrine", "--root", root, "for", path];
    if terse {
        forge_args.push("--terse");
    }
    run_forge("doctrine for", &forge_args).await
}

async fn tool_forge_authoring(args: Value) -> Value {
    let root = args
        .get("root")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    run_forge("authoring", &["authoring", "--root", root]).await
}

async fn tool_forge_config(args: Value) -> Value {
    let root = args
        .get("root")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    run_forge("config", &["config", "--root", root]).await
}

async fn tool_forge_fix(args: Value) -> Value {
    let root = args
        .get("root")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    run_forge("fix", &["fix", "--root", root]).await
}

async fn tool_forge_synthesis_preview(args: Value) -> Value {
    let Some(spec_path) = args.get("spec_path").and_then(|v| v.as_str()) else {
        return json!({
            "isError": true,
            "content": [{
                "type": "text",
                "text": "missing required argument: spec_path"
            }]
        });
    };
    let root = args
        .get("root")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    run_forge(
        "synthesis preview",
        &["synthesis", "--root", root, "preview", spec_path],
    )
    .await
}

async fn tool_forge_codegen(args: Value) -> Value {
    let root = args
        .get("root")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);
    let out = args.get("out").and_then(|v| v.as_str());
    let mut forge_args: Vec<&str> = vec!["codegen", "--root", root];
    if dry_run {
        forge_args.push("--dry-run");
    } else if let Some(o) = out {
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
    let root = args
        .get("root")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
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

    let index = canonical_index();

    // Exact-slug shortcut: when "slug" arg is provided, return
    // that single entry (or null) and ignore other filters.
    if let Some(slug) = args.get("slug").and_then(|v| v.as_str()) {
        return match index.get(slug) {
            Some(entry) => serde_json::to_value(entry).unwrap_or(Value::Null),
            None => Value::Null,
        };
    }

    let kind = args
        .get("kind")
        .and_then(|v| v.as_str())
        .and_then(parse_kind);
    let tags_any: Vec<DocTag> = args
        .get("tags_any")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|t| t.as_str()).filter_map(parse_tag).collect())
        .unwrap_or_default();
    let slug_prefix = args
        .get("slug_prefix")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let contains_text = args
        .get("contains_text")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .and_then(|n| usize::try_from(n).ok());

    let filter = DocQueryFilter {
        kind,
        tags_any,
        slug_prefix,
        contains_text,
        limit,
    };
    let entries = index.query(&filter);
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
