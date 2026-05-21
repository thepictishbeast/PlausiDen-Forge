//! `semver_enforcement` — verify every substrate artifact carries a
//! version field, and every version field parses as semver 2.0.0.
//!
//! Per `[[backward-compat-version-discipline]]` doctrine + AVP-
//! Doctrine `VERSION_DISCIPLINE.md`: every artifact (cms/*.json,
//! forge.toml, backends.toml, mcp/manifest.json, mcp/tools/*.json,
//! manifest projections) carries a `version` field (or
//! `schema_version`); upgrades are explicit; renderability is
//! guaranteed.
//!
//! This phase walks the artifact classes that ship today, reads
//! their declared version field, and emits findings when:
//!
//!   * a CMS page lacks a `version` field (warn for now — the rule
//!     lifecycle is `experimental` while existing CMS files
//!     migrate; production deployments can opt-in via strict mode)
//!   * a declared `version` field exists but doesn't parse as
//!     `<major>.<minor>.<patch>` semver (strict; malformed version
//!     is worse than missing because consumers may believe they
//!     have a pinning contract that doesn't hold)
//!
//! Per `[[deterministic-first-lfi-optional]]`: the check is pure
//! string parsing. No AI involvement.
//!
//! Closes task #138 (backcompat-v2).
//!
//! Note on doctrine citations: the rules below cite `build-001`
//! (build-time strict findings) as a temporary anchor. When the
//! `backcompat` doctrine domain lands (per VERSION_DISCIPLINE.md),
//! this phase will recite `backcompat-001` etc.

use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// `semver_enforcement` phase.
#[derive(Debug, Default)]
pub struct SemverEnforcementPhase;

impl Phase for SemverEnforcementPhase {
    fn name(&self) -> &'static str {
        "semver_enforcement"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();

        // 1. forge.toml — must declare `[platform] forge_version`
        //    once the platform layer is wired. Until then, warn only.
        let forge_toml = ctx.root.join("forge.toml");
        if forge_toml.exists() {
            check_forge_toml(&forge_toml, &mut findings);
        }

        // 2. backends.toml — must declare `schema_version`.
        let backends_toml = ctx.root.join("backends.toml");
        if backends_toml.exists() {
            check_backends_toml(&backends_toml, &mut findings);
        }

        // 3. cms/*.json — pages should declare a `version` field
        //    matching the cms-schema.json version contract.
        let cms_dir = ctx.root.join("cms");
        if cms_dir.is_dir() {
            check_cms_dir(&cms_dir, &mut findings);
        }

        // 4. mcp/manifest.json — must declare top-level `version`.
        let mcp_manifest = ctx.root.join("mcp").join("manifest.json");
        if mcp_manifest.exists() {
            check_mcp_manifest(&mcp_manifest, &mut findings);
        }

        Ok(findings)
    }
}

/// Strict semver 2.0.0 parser. Accepts `MAJOR.MINOR.PATCH` plus
/// optional pre-release / build metadata suffixes. The check is
/// intentionally narrow — we want it to refuse `1.0`, `latest`,
/// or non-numeric tokens that downstream consumers would
/// misinterpret as pin contracts.
fn parse_semver(s: &str) -> bool {
    // Split off any pre-release `-X` suffix.
    let core = s.split('-').next().unwrap_or(s);
    // Then split off any build-metadata `+X` suffix.
    let core = core.split('+').next().unwrap_or(core);
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Check `forge.toml` for the `[platform]` version block.
fn check_forge_toml(path: &Path, out: &mut Vec<Finding>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(value): Result<toml::Value, _> = text.parse() else {
        return;
    };
    let display = path.display().to_string();

    let Some(platform) = value.get("platform") else {
        out.push(
            Finding::warn(
                "semver_enforcement",
                display.clone(),
                "forge.toml missing [platform] block",
            )
            .citing(["build-001"])
            .why("per VERSION_DISCIPLINE.md, every site declares its pinned forge_version + loom_version + crawler_version so upgrades are explicit, not silent")
            .fix("add `[platform]\\nforge_version = \"<exact>\"\\nloom_version = \"<exact>\"\\ncrawler_version = \"<exact>\"` to forge.toml")
            .skill("author-cms-content"),
        );
        return;
    };

    for field in ["forge_version", "loom_version", "crawler_version"] {
        let Some(v) = platform.get(field) else {
            continue;
        };
        let Some(s) = v.as_str() else {
            out.push(
                Finding::strict(
                    "semver_enforcement",
                    display.clone(),
                    format!("[platform].{field} is not a string"),
                )
                .citing(["build-001"])
                .why("version field must be a string in semver 2.0.0 form, not a TOML integer or table")
                .fix(format!("set `{field} = \"<major>.<minor>.<patch>\"` in forge.toml")),
            );
            continue;
        };
        // Pinned versions may be exact or a range — accept either.
        // Reject obvious anti-patterns: "latest", empty, raw integer.
        if s.is_empty() || s.eq_ignore_ascii_case("latest") {
            out.push(
                Finding::strict(
                    "semver_enforcement",
                    display.clone(),
                    format!("[platform].{field} = \"{s}\" — floating pointer forbidden"),
                )
                .citing(["build-001"])
                .why("VERSION_DISCIPLINE.md § Pin-by-default: 'latest' / empty pins defeat the upgrade-is-explicit guarantee — every build silently picks up incompatible changes")
                .fix(format!("pin to a specific version: `{field} = \"<major>.<minor>.<patch>\"`. For ranges, use `\">=X.Y.Z,<A.B.C\"` form"))
                .avoid("'latest' / 'master' / 'main' branch pointers are unstable; never use them as version pins"),
            );
        }
    }
}

/// Check `backends.toml` for `schema_version`.
fn check_backends_toml(path: &Path, out: &mut Vec<Finding>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(value): Result<toml::Value, _> = text.parse() else {
        return;
    };
    let display = path.display().to_string();

    let Some(v) = value.get("schema_version") else {
        out.push(
            Finding::warn(
                "semver_enforcement",
                display.clone(),
                "backends.toml missing schema_version",
            )
            .citing(["build-001"])
            .why("backends.toml carries a typed schema that evolves; an unversioned file can't be safely auto-migrated when the schema changes")
            .fix("add `schema_version = 1` at the top of backends.toml; bump on every schema change per VERSION_DISCIPLINE.md taxonomy"),
        );
        return;
    };

    // Accept integer (legacy `schema_version = 1`) OR string (newer
    // semver pattern `schema_version = "1.0.0"`). Reject anything else.
    if v.as_integer().is_none() && v.as_str().is_none() {
        out.push(
            Finding::strict(
                "semver_enforcement",
                display,
                "backends.toml schema_version must be an integer or semver string",
            )
            .citing(["build-001"])
            .fix("set `schema_version = 1` (integer) or `schema_version = \"1.0.0\"` (string)"),
        );
    }
}

/// Walk `cms/*.json` and verify each page declares a `version` field.
fn check_cms_dir(dir: &Path, out: &mut Vec<Finding>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        check_cms_page(&path, out);
    }
}

fn check_cms_page(path: &Path, out: &mut Vec<Finding>) {
    let display = path.display().to_string();
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(value): Result<serde_json::Value, _> = serde_json::from_str(&text) else {
        // Malformed JSON is the content_validate phase's problem,
        // not ours.
        return;
    };

    let Some(version) = value.get("version") else {
        out.push(
            Finding::warn(
                "semver_enforcement",
                display.clone(),
                "cms page missing `version` field",
            )
            .citing(["build-001"])
            .why("VERSION_DISCIPLINE.md § Per-artifact-class versioning: every CMS page carries a `version` field; absence prevents auto-migration when the page schema evolves")
            .fix("add `\"version\": \"1.0.0\"` to the page's top-level JSON object")
            .skill("author-cms-content"),
        );
        return;
    };

    let Some(version_str) = version.as_str() else {
        out.push(
            Finding::strict(
                "semver_enforcement",
                display.clone(),
                "cms page version field is not a string",
            )
            .citing(["build-001"])
            .fix("set `\"version\": \"<major>.<minor>.<patch>\"` (string form, semver 2.0.0)"),
        );
        return;
    };

    if !parse_semver(version_str) {
        out.push(
            Finding::strict(
                "semver_enforcement",
                display,
                format!("cms page version `{version_str}` does not parse as semver 2.0.0"),
            )
            .citing(["build-001"])
            .why("a malformed version string is worse than no version: downstream consumers may believe they have a pinning contract that doesn't hold")
            .fix("rewrite as `<major>.<minor>.<patch>` (e.g. `1.0.0`, `2.3.1`). Optional pre-release suffix per https://semver.org/")
            .avoid("don't use `1.0`, `v1`, `latest`, or non-numeric tokens — they're not semver"),
        );
    }
}

/// Check `mcp/manifest.json` for a top-level `version` field that
/// parses as semver.
fn check_mcp_manifest(path: &Path, out: &mut Vec<Finding>) {
    let display = path.display().to_string();
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(value): Result<serde_json::Value, _> = serde_json::from_str(&text) else {
        return;
    };

    let Some(version) = value.get("version").and_then(|v| v.as_str()) else {
        out.push(
            Finding::warn(
                "semver_enforcement",
                display.clone(),
                "mcp/manifest.json missing top-level `version` field",
            )
            .citing(["build-001"])
            .why("MCP clients consume the manifest as the canonical surface; without a version they can't reason about schema migrations")
            .fix("add `\"version\": \"1.0.0\"` to mcp/manifest.json at the top level"),
        );
        return;
    };

    if !parse_semver(version) {
        out.push(
            Finding::strict(
                "semver_enforcement",
                display,
                format!("mcp/manifest.json version `{version}` does not parse as semver"),
            )
            .citing(["build-001"])
            .fix("rewrite as semver 2.0.0 form `<major>.<minor>.<patch>`"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_semver_accepts_canonical_forms() {
        assert!(parse_semver("0.0.0"));
        assert!(parse_semver("1.0.0"));
        assert!(parse_semver("2.3.1"));
        assert!(parse_semver("10.20.30"));
        assert!(parse_semver("1.0.0-alpha"));
        assert!(parse_semver("1.0.0-beta.2"));
        assert!(parse_semver("1.0.0+build.42"));
        assert!(parse_semver("1.0.0-alpha+build.42"));
    }

    #[test]
    fn parse_semver_rejects_non_canonical() {
        assert!(!parse_semver(""));
        assert!(!parse_semver("1"));
        assert!(!parse_semver("1.0"));
        assert!(!parse_semver("v1.0.0"));
        assert!(!parse_semver("latest"));
        assert!(!parse_semver("1.0.0.0"));
        assert!(!parse_semver("1.0.x"));
        assert!(!parse_semver("a.b.c"));
        assert!(!parse_semver(".0.0"));
        assert!(!parse_semver("1..0"));
    }

    #[test]
    fn check_forge_toml_warns_on_missing_platform_block() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("forge.toml");
        std::fs::write(&p, "[forge]\nmode = \"poc\"\n").unwrap();
        let mut findings = Vec::new();
        check_forge_toml(&p, &mut findings);
        assert!(findings
            .iter()
            .any(|f| f.message.contains("missing [platform] block")));
    }

    #[test]
    fn check_forge_toml_strict_on_floating_pointer() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("forge.toml");
        std::fs::write(
            &p,
            r#"[platform]
forge_version = "latest"
"#,
        )
        .unwrap();
        let mut findings = Vec::new();
        check_forge_toml(&p, &mut findings);
        assert!(findings.iter().any(
            |f| f.message.contains("\"latest\"") && f.severity == forge_core::Severity::Strict
        ));
    }

    #[test]
    fn check_cms_page_warns_on_missing_version() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("index.json");
        std::fs::write(&p, r#"{"title":"x"}"#).unwrap();
        let mut findings = Vec::new();
        check_cms_page(&p, &mut findings);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("missing `version`"));
    }

    #[test]
    fn check_cms_page_strict_on_malformed_version() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("about.json");
        std::fs::write(&p, r#"{"title":"x","version":"latest"}"#).unwrap();
        let mut findings = Vec::new();
        check_cms_page(&p, &mut findings);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, forge_core::Severity::Strict);
        assert!(findings[0].message.contains("does not parse"));
    }

    #[test]
    fn check_cms_page_clean_on_canonical_version() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("contact.json");
        std::fs::write(&p, r#"{"title":"x","version":"1.0.0"}"#).unwrap();
        let mut findings = Vec::new();
        check_cms_page(&p, &mut findings);
        assert!(findings.is_empty());
    }

    #[test]
    fn check_mcp_manifest_warns_on_missing_version() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("manifest.json");
        std::fs::write(&p, r#"{"name":"x","tools":[]}"#).unwrap();
        let mut findings = Vec::new();
        check_mcp_manifest(&p, &mut findings);
        assert!(findings.iter().any(|f| f.message.contains("missing")));
    }

    #[test]
    fn check_mcp_manifest_clean_on_canonical() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("manifest.json");
        std::fs::write(&p, r#"{"name":"x","version":"0.1.0","tools":[]}"#).unwrap();
        let mut findings = Vec::new();
        check_mcp_manifest(&p, &mut findings);
        assert!(findings.is_empty());
    }

    #[test]
    fn check_backends_toml_warns_on_missing_schema_version() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("backends.toml");
        std::fs::write(&p, "[[backend]]\nid = \"foo\"\n").unwrap();
        let mut findings = Vec::new();
        check_backends_toml(&p, &mut findings);
        assert!(findings
            .iter()
            .any(|f| f.message.contains("missing schema_version")));
    }

    #[test]
    fn full_phase_run_against_existing_workspace_layout() {
        let tmp = tempfile::tempdir().unwrap();
        // Minimal site: forge.toml + backends.toml + cms/index.json.
        std::fs::write(
            tmp.path().join("forge.toml"),
            r#"[platform]
forge_version = "0.1.0"
"#,
        )
        .unwrap();
        std::fs::write(tmp.path().join("backends.toml"), "schema_version = 1\n").unwrap();
        std::fs::create_dir_all(tmp.path().join("cms")).unwrap();
        std::fs::write(
            tmp.path().join("cms/index.json"),
            r#"{"title":"x","version":"1.0.0"}"#,
        )
        .unwrap();
        let ctx = BuildCtx {
            root: tmp.path().to_path_buf(),
            static_dir: tmp.path().join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = SemverEnforcementPhase.run(&ctx).expect("run");
        assert!(
            findings.is_empty(),
            "expected clean run, got: {:?}",
            findings.iter().map(|f| &f.message).collect::<Vec<_>>()
        );
    }
}
