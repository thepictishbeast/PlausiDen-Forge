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
