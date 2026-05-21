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

async fn tool_forge_orient(args: Value) -> Value {
    let root = args
        .get("root")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    // Shell out to the installed `forge` binary. Future iter:
    // call into forge-core directly to skip the subprocess
    // round-trip.
    let output = tokio::process::Command::new("forge")
        .args(["orient", "--root", root])
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
                    "forge orient exited {}: {}",
                    out.status,
                    String::from_utf8_lossy(&out.stderr)
                )
            }]
        }),
        Err(e) => json!({
            "isError": true,
            "content": [{
                "type": "text",
                "text": format!("could not spawn forge: {e}")
            }]
        }),
    }
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
