//! `session_scope` — the scoped-session pattern for the
//! accessibility axis.
//!
//! Per `docs/SUBSTRATE_REFRAME_2026_05_21.md` § Accessibility 1:
//! each Claude session starts with a *declared scope* — the
//! kind of work it's there to do. The MCP tool surface,
//! documentation context, and visible substrate state are
//! filtered to that scope; tools and docs irrelevant to the
//! current work aren't presented. The substrate decides what's
//! relevant based on declared scope; the consumer (Claude,
//! operator, automation) works within the curated subset.
//!
//! This module ships the typed primitive: a closed enumeration
//! of session scopes plus a filtering function that maps each
//! scope to its in-scope tool set.
//!
//! Closed enum is intentional. Adding a scope is a substrate-
//! doctrine event; widening the enum without curated tool /
//! doc filtering for the new scope produces the same cognitive-
//! overload symptom Forge Lite is designed to test.
//!
//! ## Wiring
//!
//! - `forge-mcp::main`'s `tool_list()` calls
//!   [`tools_in_scope`] to filter what's surfaced to the
//!   client.
//! - `forge-cli`'s `forge orient` subcommand reports the
//!   declared scope plus the tool subset (so operators can see
//!   what's available without invoking the full mcp client).
//! - Documentation surfaces (skill manifests, doctrine
//!   indexes) read [`SessionScope`] to filter their own
//!   payloads.

use serde::{Deserialize, Serialize};

/// Closed enumeration of session scopes. Each variant is one
/// well-defined kind of work an operator might be doing in the
/// Forge workspace.
///
/// JSON wire shape (snake_case kebab):
/// `"build_site"`, `"modify_primitive"`, `"debug_audit"`,
/// `"extend_deploy_target"`, `"author_content"`,
/// `"investigate_substrate"`, `"unscoped"`.
///
/// `Unscoped` is the explicit fallback — the consumer hasn't
/// declared a scope and gets the full tool surface. New
/// sessions should declare a scope explicitly; `Unscoped` is
/// for legacy callers and exploratory work.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum SessionScope {
    /// Building / modifying a tenant site's content + config.
    /// Surfaces: orient, build, authoring, config, doctrine,
    /// fix.
    BuildSite,
    /// Adding or modifying a substrate primitive (Rust code in
    /// loom-* or forge-* crates). Surfaces: doctrine, manifest
    /// validate, codegen, build (for verification), orient.
    ModifyPrimitive,
    /// Investigating a failing build / audit phase. Surfaces:
    /// build (with JSON), doctrine.for, fix, orient.
    DebugAudit,
    /// Extending or hardening a deploy target. Surfaces:
    /// deploy, manifest validate, config, orient.
    ExtendDeployTarget,
    /// Authoring tenant CMS content only — no Rust code.
    /// Narrow surface optimized for the editing loop:
    /// authoring (TODO scan), build, orient.
    AuthorContent,
    /// Exploring the substrate / answering questions / running
    /// audits over registries. Surfaces: orient, doctrine,
    /// fingerprint, identity, synthesis preview.
    InvestigateSubstrate,
    /// No scope declared — full tool surface. Default for
    /// legacy callers; new sessions should declare a scope.
    #[default]
    Unscoped,
}

impl SessionScope {
    /// Stable kebab-case slug for the scope. Used as the
    /// session identity in telemetry, logs, and the
    /// `data-session-scope` attribute the operator UI surfaces.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::BuildSite => "build-site",
            Self::ModifyPrimitive => "modify-primitive",
            Self::DebugAudit => "debug-audit",
            Self::ExtendDeployTarget => "extend-deploy-target",
            Self::AuthorContent => "author-content",
            Self::InvestigateSubstrate => "investigate-substrate",
            Self::Unscoped => "unscoped",
        }
    }

    /// Human-readable label for the scope. Used in operator-
    /// facing surfaces (orient banner, status line) where the
    /// kebab slug reads awkwardly.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::BuildSite => "Build site",
            Self::ModifyPrimitive => "Modify substrate primitive",
            Self::DebugAudit => "Debug failing audit",
            Self::ExtendDeployTarget => "Extend deploy target",
            Self::AuthorContent => "Author content",
            Self::InvestigateSubstrate => "Investigate substrate",
            Self::Unscoped => "(no scope)",
        }
    }

    /// Parse a slug back into the typed scope. Returns `None`
    /// for unknown slugs — callers should treat the absence as
    /// `Unscoped` rather than fatal.
    #[must_use]
    pub fn from_slug(s: &str) -> Option<Self> {
        match s {
            "build-site" => Some(Self::BuildSite),
            "modify-primitive" => Some(Self::ModifyPrimitive),
            "debug-audit" => Some(Self::DebugAudit),
            "extend-deploy-target" => Some(Self::ExtendDeployTarget),
            "author-content" => Some(Self::AuthorContent),
            "investigate-substrate" => Some(Self::InvestigateSubstrate),
            "unscoped" => Some(Self::Unscoped),
            _ => None,
        }
    }
}

/// MCP tool name allowlist for a given scope. The list is
/// hand-curated to expose only tools relevant to the declared
/// work. Returning a closed set lets `forge-mcp`'s `tool_list`
/// filter deterministically.
///
/// `Unscoped` returns an empty slice meaning "no filtering" —
/// the caller surfaces the full tool list. (Returning every
/// tool name here would couple this module to forge-mcp's
/// inventory; the empty-slice convention preserves the seam.)
#[must_use]
pub fn tools_in_scope(scope: SessionScope) -> &'static [&'static str] {
    match scope {
        SessionScope::BuildSite => &[
            "forge.orient",
            "forge.build",
            "forge.authoring",
            "forge.config",
            "forge.doctrine.for",
            "forge.fix",
        ],
        SessionScope::ModifyPrimitive => &[
            "forge.orient",
            "forge.doctrine.for",
            "forge.manifest.validate",
            "forge.codegen",
            "forge.build",
        ],
        SessionScope::DebugAudit => &[
            "forge.orient",
            "forge.build",
            "forge.doctrine.for",
            "forge.fix",
        ],
        SessionScope::ExtendDeployTarget => &[
            "forge.orient",
            "forge.manifest.validate",
            "forge.config",
        ],
        SessionScope::AuthorContent => &[
            "forge.orient",
            "forge.authoring",
            "forge.build",
        ],
        SessionScope::InvestigateSubstrate => &[
            "forge.orient",
            "forge.doctrine.for",
            "forge.synthesis.preview",
        ],
        SessionScope::Unscoped => &[],
    }
}

/// Documentation surface tags in scope for the declared
/// session. The substrate's docs are tagged with one or more
/// of these tags; the caller filters by intersection. Adding
/// a new doc tag is a substrate-doctrine event (matches the
/// closed-enum discipline applied to the scope itself).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum DocTag {
    Tenant,
    Primitive,
    AuditPhase,
    Deploy,
    Authoring,
    Doctrine,
    Reframe,
    Workflow,
}

/// Doc tags in scope for the declared session. Mirrors
/// [`tools_in_scope`].
#[must_use]
pub fn docs_in_scope(scope: SessionScope) -> &'static [DocTag] {
    match scope {
        SessionScope::BuildSite => &[
            DocTag::Tenant,
            DocTag::Authoring,
            DocTag::Workflow,
            DocTag::Doctrine,
        ],
        SessionScope::ModifyPrimitive => &[
            DocTag::Primitive,
            DocTag::Doctrine,
            DocTag::Workflow,
        ],
        SessionScope::DebugAudit => &[
            DocTag::AuditPhase,
            DocTag::Doctrine,
            DocTag::Workflow,
        ],
        SessionScope::ExtendDeployTarget => &[DocTag::Deploy, DocTag::Doctrine],
        SessionScope::AuthorContent => &[DocTag::Authoring, DocTag::Tenant],
        SessionScope::InvestigateSubstrate => &[
            DocTag::Reframe,
            DocTag::Doctrine,
            DocTag::Primitive,
            DocTag::AuditPhase,
        ],
        SessionScope::Unscoped => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn slug_round_trips_every_scope() {
        for s in [
            SessionScope::BuildSite,
            SessionScope::ModifyPrimitive,
            SessionScope::DebugAudit,
            SessionScope::ExtendDeployTarget,
            SessionScope::AuthorContent,
            SessionScope::InvestigateSubstrate,
            SessionScope::Unscoped,
        ] {
            assert_eq!(SessionScope::from_slug(s.slug()), Some(s));
        }
    }

    #[test]
    fn from_slug_returns_none_for_unknown() {
        assert!(SessionScope::from_slug("does-not-exist").is_none());
    }

    #[test]
    fn unscoped_returns_empty_tool_filter() {
        assert!(tools_in_scope(SessionScope::Unscoped).is_empty());
    }

    #[test]
    fn every_scope_includes_forge_orient() {
        // Orient is the universal entry point; it should be in
        // scope for every non-unscoped session.
        for s in [
            SessionScope::BuildSite,
            SessionScope::ModifyPrimitive,
            SessionScope::DebugAudit,
            SessionScope::ExtendDeployTarget,
            SessionScope::AuthorContent,
            SessionScope::InvestigateSubstrate,
        ] {
            let tools = tools_in_scope(s);
            assert!(
                tools.contains(&"forge.orient"),
                "scope {:?} missing forge.orient",
                s.slug()
            );
        }
    }

    #[test]
    fn build_site_does_not_surface_substrate_only_tools() {
        let tools: BTreeSet<&&str> = tools_in_scope(SessionScope::BuildSite).iter().collect();
        // Manifest validate is a substrate-modify operation,
        // not a tenant-build operation. The point of the scope
        // is to NOT surface it during build-site work.
        assert!(
            !tools.contains(&"forge.manifest.validate"),
            "build-site scope should not surface forge.manifest.validate"
        );
    }

    #[test]
    fn modify_primitive_does_not_surface_authoring() {
        let tools: BTreeSet<&&str> =
            tools_in_scope(SessionScope::ModifyPrimitive).iter().collect();
        assert!(
            !tools.contains(&"forge.authoring"),
            "modify-primitive scope should not surface forge.authoring (tenant-content tool)"
        );
    }

    #[test]
    fn tool_lists_are_unique_per_scope() {
        let lists: Vec<BTreeSet<&str>> = [
            SessionScope::BuildSite,
            SessionScope::ModifyPrimitive,
            SessionScope::DebugAudit,
            SessionScope::ExtendDeployTarget,
            SessionScope::AuthorContent,
            SessionScope::InvestigateSubstrate,
        ]
        .iter()
        .map(|s| tools_in_scope(*s).iter().copied().collect())
        .collect();
        for i in 0..lists.len() {
            for j in (i + 1)..lists.len() {
                assert_ne!(
                    lists[i], lists[j],
                    "scope tool surfaces must be distinct; scopes at indices {i} and {j} match"
                );
            }
        }
    }

    #[test]
    fn docs_in_scope_round_trips() {
        // Every scope must surface AT LEAST one doc tag
        // (excluding Unscoped which is full surface).
        for s in [
            SessionScope::BuildSite,
            SessionScope::ModifyPrimitive,
            SessionScope::DebugAudit,
            SessionScope::ExtendDeployTarget,
            SessionScope::AuthorContent,
            SessionScope::InvestigateSubstrate,
        ] {
            assert!(
                !docs_in_scope(s).is_empty(),
                "scope {:?} has empty doc filter — orient surface will starve",
                s.slug()
            );
        }
        assert!(docs_in_scope(SessionScope::Unscoped).is_empty());
    }

    #[test]
    fn json_wire_format_is_kebab_via_snake_case() {
        // serde(rename_all = "snake_case") on the enum +
        // closed match on the slug must agree so external
        // consumers (forge-mcp JSON-RPC, dashboards) can rely
        // on a single canonical wire form.
        for s in [
            SessionScope::BuildSite,
            SessionScope::ModifyPrimitive,
            SessionScope::DebugAudit,
            SessionScope::ExtendDeployTarget,
            SessionScope::AuthorContent,
            SessionScope::InvestigateSubstrate,
            SessionScope::Unscoped,
        ] {
            let wire = serde_json::to_string(&s).expect("serializes");
            // strip surrounding quotes
            let bare = wire.trim_matches('"');
            // serde snake_case → e.g. "build_site"; slug() →
            // "build-site". Both forms must round-trip.
            assert_eq!(bare.replace('_', "-"), s.slug());
        }
    }
}
