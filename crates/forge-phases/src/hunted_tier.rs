//! `hunted_tier` — max-paranoid security profile gate.
//!
//! When `forge.toml [security] tier = "hunted"`, this phase
//! enforces the full set of "as if the world were hunting you
//! down" security properties. Per task #124 + the broader
//! supersociety tech-stack doctrine.
//!
//! The hunted tier is a META-POLICY composed of existing
//! strict modes:
//!
//! 1. `noscript_strict` MUST be on — zero `<script>` tags in
//!    rendered HTML. (Phase: `noscript_strict`.)
//! 2. CSP MUST be the strictest variant — `script-src 'none'`,
//!    `style-src 'self' '<hash>'`, `require-trusted-types-for
//!    'script'; trusted-types 'none'`. Loom's
//!    `LOOM_NOSCRIPT_MODE=1` renders this automatically.
//! 3. No third-party origins anywhere in rendered HTML. Existing
//!    `external_assets` + `network_target_enforcement` phases
//!    cover this.
//! 4. No client-side state APIs in any inline JS — but since
//!    rule #1 forbids inline JS entirely, this is implied. The
//!    audit confirms no `localStorage` / `sessionStorage` /
//!    `document.cookie` / `navigator.*` references in any text
//!    leaking through CMS body content.
//! 5. No fingerprintable canvas / webgl / battery / device
//!    surfaces. Same — rule #1 forbids inline JS so this is
//!    implied; the audit catches accidental data: URIs that
//!    embed canvas-fingerprint payloads.
//!
//! ## Heuristic
//!
//! This phase is the SHAPE-CHECK: it enforces that the
//! prerequisite modes are configured. It does NOT re-run their
//! audits — `noscript_strict` etc. already run separately. If
//! tier = "hunted" but `noscript_strict` is off, this phase
//! fails strict directing the operator to enable it.
//!
//! Plus: a body-text scan for cookie / localStorage references
//! that would imply client-state expectations even if the JS
//! that uses them got stripped.
//!
//! ## Severity
//!
//! Strict — hunted tier means "the build refuses to ship if
//! any guarantee weakens." There's no warn-only mode.

use std::fs;
use std::path::Path;

use forge_core::tenant_corpus::TenantCorpus;
use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// `hunted_tier` phase implementation.
#[derive(Debug, Default)]
pub struct HuntedTierPhase;

impl Phase for HuntedTierPhase {
    fn name(&self) -> &'static str {
        "hunted_tier"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let tier = read_security_tier(&ctx.root);
        if tier.as_deref() != Some("hunted") {
            return Ok(findings);
        }
        // Prerequisite: noscript_strict must also be enabled.
        let noscript_on = read_noscript_strict_enabled(&ctx.root)
            || std::env::var("LOOM_NOSCRIPT_MODE")
                .map(|v| !v.is_empty() && v != "0")
                .unwrap_or(false);
        if !noscript_on {
            findings.push(Finding::strict(
                self.name(),
                "forge.toml".to_owned(),
                "hunted_tier — `[security] tier = \"hunted\"` requires `[noscript_strict] enabled = true` (or LOOM_NOSCRIPT_MODE=1 in env). The hunted tier is a meta-policy; its zero-JS guarantee comes from noscript_strict. Enable it.".to_owned(),
            ));
        }
        // Layer tenant-corpus extra_body_leak_markers on top of
        // the baseline per [[per-tenant-corpora-doctrine]]. Allows
        // operators to extend the client-state-API scan with
        // tenant-specific markers (e.g. IndexedDB.open, FileSystem
        // API, app-specific globals) without forking the substrate.
        let tenant = TenantCorpus::load(&ctx.root);
        let tenant_extra_markers: Vec<&str> = tenant
            .as_ref()
            .map(|t| t.extra_body_leak_markers.iter().map(String::as_str).collect())
            .unwrap_or_default();
        // Body-text scan for client-state API references.
        let static_dir = &ctx.static_dir;
        if static_dir.is_dir() {
            let entries = fs::read_dir(static_dir).map_err(|e| BuildError::Io {
                context: format!("read_dir {}", static_dir.display()),
                source: e,
            })?;
            for entry in entries {
                let entry = entry.map_err(|e| BuildError::Io {
                    context: format!("read_dir entry in {}", static_dir.display()),
                    source: e,
                })?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("html") {
                    continue;
                }
                let raw = fs::read_to_string(&path).map_err(|e| BuildError::Io {
                    context: format!("read {}", path.display()),
                    source: e,
                })?;
                // Baseline markers first.
                for marker in BODY_LEAK_MARKERS {
                    if raw.contains(marker) {
                        findings.push(Finding::strict(
                            self.name(),
                            path.display().to_string(),
                            format!(
                                "hunted_tier — rendered HTML contains the literal `{marker}`, which suggests the page expects client-side state. Under hunted tier the build assumes ZERO client state. If this is innocent body copy (e.g. an article ABOUT localStorage), opt out by setting `[security] tier = \"strict\"` for that build instead."
                            ),
                        ));
                    }
                }
                // Tenant-extra markers (additive). Tagged with
                // (tenant-corpus) in the message so reports
                // distinguish operator-configured from baseline.
                for marker in &tenant_extra_markers {
                    if raw.contains(*marker) {
                        findings.push(Finding::strict(
                            self.name(),
                            path.display().to_string(),
                            format!(
                                "hunted_tier (tenant-corpus) — rendered HTML contains the literal `{marker}`, configured as an extra body-leak marker for this tenant. Same posture as baseline markers: under hunted tier the build assumes ZERO client state."
                            ),
                        ));
                    }
                }
                // Cookie meta hint: a `<meta http-equiv="Set-Cookie">`
                // signal would be ancient + non-standard but worth
                // flagging if it slipped through.
                if raw.to_lowercase().contains("set-cookie") {
                    findings.push(Finding::strict(
                        self.name(),
                        path.display().to_string(),
                        "hunted_tier — rendered HTML mentions `Set-Cookie`. Under hunted tier the build sets no cookies; the literal in HTML implies the server intends to. Remove or move to a non-hunted build profile.".to_owned(),
                    ));
                }
            }
        }
        Ok(findings)
    }
}

/// Markers that, when present in rendered HTML body, suggest the
/// page expects client-state APIs forbidden by the hunted tier.
const BODY_LEAK_MARKERS: &[&str] = &[
    "localStorage.",
    "sessionStorage.",
    "document.cookie",
    "navigator.geolocation",
    "navigator.mediaDevices",
    "navigator.usb",
    "navigator.bluetooth",
    "navigator.serial",
    "canvas.toDataURL",
    "getContext('webgl')",
    "WebGLRenderingContext",
    "navigator.getBattery",
];

fn read_security_tier(root: &Path) -> Option<String> {
    read_toml_string_value(root, "[security]", "tier")
}

fn read_noscript_strict_enabled(root: &Path) -> bool {
    matches!(
        read_toml_bool_value(root, "[noscript_strict]", "enabled"),
        Some(true)
    )
}

fn read_toml_string_value(root: &Path, section: &str, key: &str) -> Option<String> {
    let cfg_path = root.join("forge.toml");
    let body = fs::read_to_string(&cfg_path).ok()?;
    let mut in_section = false;
    for raw in body.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            in_section = line == section;
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some(rest) = line.strip_prefix(key) {
            let v = rest.trim_start().trim_start_matches('=').trim();
            let unquoted = v.trim_matches('"').trim_matches('\'');
            return Some(unquoted.to_owned());
        }
    }
    None
}

fn read_toml_bool_value(root: &Path, section: &str, key: &str) -> Option<bool> {
    let raw = read_toml_string_value(root, section, key)?;
    Some(matches!(raw.to_lowercase().as_str(), "true" | "1"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let p = env::temp_dir().join(format!("forge-hunted-test-{name}-{}", std::process::id()));
        let _ = fs::create_dir_all(&p);
        p
    }

    #[test]
    fn no_tier_silent_skip() {
        let dir = temp_dir("nosec");
        let _ = fs::write(dir.join("forge.toml"), "[other]\nfoo = true\n");
        assert_eq!(read_security_tier(&dir), None);
        let _ = fs::remove_file(dir.join("forge.toml"));
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn tier_hunted_recognized() {
        let dir = temp_dir("hunted");
        let _ = fs::write(dir.join("forge.toml"), "[security]\ntier = \"hunted\"\n");
        assert_eq!(read_security_tier(&dir).as_deref(), Some("hunted"));
        let _ = fs::remove_file(dir.join("forge.toml"));
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn noscript_strict_bool_reader() {
        let dir = temp_dir("nostrict");
        let _ = fs::write(dir.join("forge.toml"), "[noscript_strict]\nenabled = true\n");
        assert!(read_noscript_strict_enabled(&dir));
        let _ = fs::remove_file(dir.join("forge.toml"));
        let _ = fs::remove_dir(&dir);
    }

    fn run_phase_in(dir: &Path) -> Vec<Finding> {
        let ctx = BuildCtx {
            root: dir.to_path_buf(),
            static_dir: dir.join("static"),
            mode: forge_core::BuildMode::Static,
        };
        HuntedTierPhase
            .run(&ctx)
            .expect("hunted_tier run should not error in test fixture")
    }

    fn write_fixture(dir: &Path, forge_toml: &str, static_html: &[(&str, &str)]) {
        let _ = fs::create_dir_all(dir);
        let _ = fs::create_dir_all(dir.join("static"));
        let _ = fs::write(dir.join("forge.toml"), forge_toml);
        for (name, body) in static_html {
            let _ = fs::write(dir.join("static").join(name), body);
        }
    }

    #[test]
    fn tenant_extra_body_leak_marker_fires_strict() {
        // forge.toml declares hunted tier + noscript_strict + a
        // tenant-extra marker. A static/*.html that contains the
        // tenant-extra string fires a strict finding tagged with
        // (tenant-corpus).
        let dir = temp_dir("tenant-extra-marker");
        write_fixture(
            &dir,
            r#"[security]
tier = "hunted"

[noscript_strict]
enabled = true

[tenant_corpus]
extra_body_leak_markers = ["indexedDB.open"]
"#,
            &[("foo.html", "<html><body>indexedDB.open(...)</body></html>")],
        );
        let findings = run_phase_in(&dir);
        assert!(
            findings.iter().any(|f| f.message.contains("(tenant-corpus)")
                && f.message.contains("indexedDB.open")),
            "expected tenant-corpus finding for indexedDB.open: {:?}",
            findings.iter().map(|f| &f.message).collect::<Vec<_>>()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn baseline_and_tenant_extras_are_additive() {
        // Static HTML contains BOTH a baseline marker (localStorage.)
        // AND a tenant-extra marker (indexedDB.open). Both fire.
        let dir = temp_dir("additive");
        write_fixture(
            &dir,
            r#"[security]
tier = "hunted"

[noscript_strict]
enabled = true

[tenant_corpus]
extra_body_leak_markers = ["indexedDB.open"]
"#,
            &[(
                "foo.html",
                "<p>localStorage.foo</p><p>indexedDB.open()</p>",
            )],
        );
        let findings = run_phase_in(&dir);
        // 1 baseline (localStorage.) + 1 tenant (indexedDB.open) = 2
        let baseline_hits = findings
            .iter()
            .filter(|f| f.message.contains("localStorage.") && !f.message.contains("(tenant-corpus)"))
            .count();
        let tenant_hits = findings
            .iter()
            .filter(|f| f.message.contains("(tenant-corpus)") && f.message.contains("indexedDB.open"))
            .count();
        assert_eq!(baseline_hits, 1, "baseline localStorage hit expected once");
        assert_eq!(tenant_hits, 1, "tenant indexedDB.open hit expected once");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_tenant_corpus_falls_through_to_baseline_only() {
        // No [tenant_corpus] section → tenant_extra_markers empty
        // → only baseline scan runs. Adding a string that ONLY a
        // hypothetical tenant marker would match should NOT fire.
        let dir = temp_dir("no-tenant");
        write_fixture(
            &dir,
            r#"[security]
tier = "hunted"

[noscript_strict]
enabled = true
"#,
            &[("foo.html", "<p>indexedDB.open(...)</p>")],
        );
        let findings = run_phase_in(&dir);
        assert!(
            !findings.iter().any(|f| f.message.contains("indexedDB.open")),
            "indexedDB.open isn't in baseline markers; without tenant extension, no finding"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn body_leak_markers_cover_known_apis() {
        // Spec invariant: each known client-state API must be in
        // the BODY_LEAK_MARKERS list. Drift-guards the constant.
        for needle in ["localStorage.", "document.cookie", "canvas.toDataURL"] {
            assert!(
                BODY_LEAK_MARKERS.contains(&needle),
                "BODY_LEAK_MARKERS missing {needle}"
            );
        }
    }
}
