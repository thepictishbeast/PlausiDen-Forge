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
            }
        ]
    })
}

async fn handle_request(req: JsonRpcRequest) -> JsonRpcResponse {
    let result = match req.method.as_str() {
        "initialize" => Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": serde_json::from_str::<Value>(SERVER_INFO).unwrap_or(json!({}))
        })),
        "tools/list" => Some(tool_list()),
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
