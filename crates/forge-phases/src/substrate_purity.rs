//! `substrate_purity` — enforces the Substrate Discipline rule from
//! `[[substrate-only-path]]`: site repos contain only CMS content +
//! configuration + Forge-emitted build outputs. Hand-authored CSS /
//! JS / HTML / site-specific Rust is forbidden.
//!
//! The phase walks the project's `static/` directory and flags any
//! files that don't match the canonical set of Forge-emitted output
//! patterns. Operators can declare exceptions via the substrate-bypass
//! register (rule build-007) using inline `// SUBSTRATE-BYPASS(...)`
//! tags + `bypass-register.toml`.
//!
//! Citation: rules build-007, prim-006, and the Substrate Discipline
//! doctrine itself.
//!
//! BUG ASSUMPTION: the allowlist is hand-curated; adding a new Loom
//! emission (e.g. `loom-theme-toggle.js`) requires updating both the
//! Loom emitter AND this phase's allowlist. The phase's regression
//! fixture (per rule test-004) covers the existing allowlist; new
//! emissions need a corresponding fixture update.

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use std::collections::HashSet;
use std::path::Path;

/// Forge audit phase enforcing substrate-purity per the
/// `[[substrate-only-path]]` doctrine.
///
/// Detects hand-authored assets in `static/` that don't match the
/// canonical Forge / Loom emission set. Severity: STRICT in
/// production, WARN in poc.
#[derive(Debug, Clone, Copy, Default)]
pub struct SubstratePurityPhase;

impl Phase for SubstratePurityPhase {
    fn name(&self) -> &'static str {
        "substrate_purity"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        if !ctx.static_dir.is_dir() {
            // No static dir — nothing to audit. Not a finding; some
            // sites are content-only and haven't been rendered yet.
            return Ok(findings);
        }
        let allowlist = canonical_emission_allowlist();
        walk_static_for_hand_authored(&ctx.static_dir, &allowlist, &mut findings, self.name());
        Ok(findings)
    }
}

/// The set of filenames Forge / Loom is allowed to emit into the
/// site repo's `static/` directory. Anything else is hand-authored.
///
/// New emitters MUST add their filename here in the same PR per
/// rule docs-007 (AGENTS.md updated same commit).
fn canonical_emission_allowlist() -> HashSet<&'static str> {
    let mut s: HashSet<&'static str> = HashSet::new();
    // Loom-emitted CSS
    s.insert("loom-skin.css");
    s.insert("loom.css");
    s.insert("loom-tokens.css");
    s.insert("loom-critical.css");
    s.insert("loom-fallback.css");
    // Forge-emitted tenant overrides (T-* fixme write_tenant_style_css).
    // Generated from forge.toml [style.*] sections; not hand-authored.
    s.insert("tenant-style.css");
    // Loom-emitted runtime (SPA / hybrid modes)
    s.insert("loom-runtime.js");
    s.insert("loom-runtime.wasm");
    s.insert("loom-runtime.wasm.gz");
    s.insert("loom-runtime.wasm.br");
    // Forge-emitted meta
    s.insert("robots.txt");
    s.insert("sitemap.xml");
    s.insert("favicon.svg");
    s.insert("favicon.ico");
    // Dev-mode error overlay (poc mode only — separate phase gates
    // it out of production builds)
    s.insert("eruda.min.js");
    s
}

/// Recursively walks `static_dir`, emitting findings for files that
/// look hand-authored.
///
/// Subdirectories containing a `.substrate-widget` marker file are
/// skipped wholesale. That marker declares the directory holds a
/// substrate-built widget pkg (e.g., `wasm-pack build` output for a
/// crate elsewhere in the workspace) that the tenant copies in to
/// serve. The marker contract: ALL files in such a directory are
/// substrate-emitted, not hand-authored, even though they didn't
/// land via Forge/Loom's own emission path. Per task #337.
fn walk_static_for_hand_authored(
    static_dir: &Path,
    allowlist: &HashSet<&'static str>,
    findings: &mut Vec<Finding>,
    phase_name: &str,
) {
    let Ok(entries) = std::fs::read_dir(static_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip directories that explicitly declare themselves
            // as substrate-built widget pkgs.
            if path.join(".substrate-widget").is_file() {
                continue;
            }
            // Recurse into subdirs (e.g. nested route HTML).
            walk_static_for_hand_authored(&path, allowlist, findings, phase_name);
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        match ext {
            // HTML files emitted by Forge render phase from cms/*.json
            // are expected; any HTML in static/ is presumed substrate
            // output, so don't flag.
            "html" => continue,
            // Asset files (images, fonts, video) are content and OK.
            "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "avif" | "ico" | "woff" | "woff2"
            | "ttf" | "otf" | "mp4" | "webm" | "mp3" | "ogg" | "pdf" | "json" | "xml" | "txt"
            | "map" => continue,
            // CSS / JS / WASM go through the allowlist.
            "css" | "js" | "mjs" | "wasm" | "br" | "gz" => {
                if !allowlist.contains(name) {
                    findings.push(
                        Finding::strict(
                            phase_name,
                            path.display().to_string(),
                            format!(
                                "hand-authored asset {name} not in canonical Forge/Loom emission allowlist"
                            ),
                        )
                        .citing(["build-007", "prim-006"])
                        .why("site repos contain only CMS content + Forge-emitted output; this asset is neither")
                        .fix("extend Loom (add a primitive variant + emission), OR move the rule into loom-tokens/src/skin.css if it is a token/theme change, OR — for genuine emergencies — declare the bypass via bypass-register.toml + a `// SUBSTRATE-BYPASS(<issue-id>): <reason>` comment")
                        .skill("add-loom-primitive")
                        .avoid("don't `cp foo.css static/` or `curl ... > static/foo.js` — substrate-only-path is load-bearing per SUBSTRATE_DISCIPLINE.md"),
                    );
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::BuildMode;
    use std::fs;

    fn make_ctx(static_dir: &Path) -> BuildCtx {
        BuildCtx {
            root: static_dir.parent().unwrap_or(static_dir).to_path_buf(),
            static_dir: static_dir.to_path_buf(),
            mode: BuildMode::Poc,
        }
    }

    fn tmp_root() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        std::env::temp_dir().join(format!("substrate-purity-test-{pid}-{n}"))
    }

    #[test]
    fn returns_empty_when_static_dir_missing() {
        let root = tmp_root();
        let phase = SubstratePurityPhase;
        let ctx = make_ctx(&root.join("static"));
        let findings = phase.run(&ctx).expect("runs");
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn allowlist_files_produce_no_findings() {
        let root = tmp_root();
        let static_dir = root.join("static");
        fs::create_dir_all(&static_dir).unwrap();
        for f in ["loom-skin.css", "loom.css", "robots.txt", "favicon.svg"] {
            fs::write(static_dir.join(f), b"x").unwrap();
        }
        let phase = SubstratePurityPhase;
        let ctx = make_ctx(&static_dir);
        let findings = phase.run(&ctx).expect("runs");
        assert!(
            findings.is_empty(),
            "expected no findings, got: {findings:#?}"
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn flags_hand_authored_css() {
        let root = tmp_root();
        let static_dir = root.join("static");
        fs::create_dir_all(&static_dir).unwrap();
        fs::write(static_dir.join("custom.css"), b"body { color: red; }").unwrap();
        let phase = SubstratePurityPhase;
        let ctx = make_ctx(&static_dir);
        let findings = phase.run(&ctx).expect("runs");
        assert_eq!(
            findings.len(),
            1,
            "expected one finding, got: {findings:#?}"
        );
        let f = &findings[0];
        assert_eq!(f.phase, "substrate_purity");
        assert!(f.message.contains("custom.css"));
        assert!(f.enforces_rules.contains(&"build-007".to_string()));
        assert!(f.enforces_rules.contains(&"prim-006".to_string()));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn flags_hand_authored_js() {
        let root = tmp_root();
        let static_dir = root.join("static");
        fs::create_dir_all(&static_dir).unwrap();
        fs::write(static_dir.join("app.js"), b"console.log('hi')").unwrap();
        let phase = SubstratePurityPhase;
        let ctx = make_ctx(&static_dir);
        let findings = phase.run(&ctx).expect("runs");
        assert_eq!(findings.len(), 1);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn html_files_are_not_flagged() {
        // Forge render phase emits HTML — these are substrate output,
        // not hand-authored. Don't flag.
        let root = tmp_root();
        let static_dir = root.join("static");
        fs::create_dir_all(&static_dir).unwrap();
        fs::write(static_dir.join("index.html"), b"<html></html>").unwrap();
        fs::write(static_dir.join("about.html"), b"<html></html>").unwrap();
        let phase = SubstratePurityPhase;
        let ctx = make_ctx(&static_dir);
        let findings = phase.run(&ctx).expect("runs");
        assert_eq!(findings.len(), 0);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn asset_files_are_not_flagged() {
        let root = tmp_root();
        let static_dir = root.join("static");
        fs::create_dir_all(&static_dir).unwrap();
        for f in ["hero.jpg", "logo.svg", "open-sans.woff2", "video.mp4"] {
            fs::write(static_dir.join(f), b"x").unwrap();
        }
        let phase = SubstratePurityPhase;
        let ctx = make_ctx(&static_dir);
        let findings = phase.run(&ctx).expect("runs");
        assert_eq!(findings.len(), 0);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn recurses_into_subdirs() {
        let root = tmp_root();
        let nested = root.join("static").join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("bad.js"), b"x").unwrap();
        let phase = SubstratePurityPhase;
        let ctx = make_ctx(&root.join("static"));
        let findings = phase.run(&ctx).expect("runs");
        assert_eq!(findings.len(), 1);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn substrate_widget_marker_exempts_directory() {
        // A directory carrying `.substrate-widget` skips the
        // hand-authored check entirely — the marker declares the
        // contents are substrate-built (e.g. wasm-pack output).
        let root = tmp_root();
        let widget_dir = root.join("static").join("crucible-widget");
        fs::create_dir_all(&widget_dir).unwrap();
        fs::write(widget_dir.join(".substrate-widget"), b"crucible-widget pkg").unwrap();
        // These would normally trip substrate_purity strict:
        fs::write(widget_dir.join("crucible_widget.js"), b"x").unwrap();
        fs::write(widget_dir.join("crucible_widget_bg.wasm"), b"x").unwrap();
        let phase = SubstratePurityPhase;
        let ctx = make_ctx(&root.join("static"));
        let findings = phase.run(&ctx).expect("runs");
        assert!(
            findings.is_empty(),
            "marker should exempt the whole dir; got {findings:#?}"
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn substrate_widget_marker_required_per_directory() {
        // Marker exempts only THAT directory, not siblings.
        let root = tmp_root();
        let widget_dir = root.join("static").join("ok-widget");
        let sibling_dir = root.join("static").join("not-widget");
        fs::create_dir_all(&widget_dir).unwrap();
        fs::create_dir_all(&sibling_dir).unwrap();
        fs::write(widget_dir.join(".substrate-widget"), b"").unwrap();
        fs::write(widget_dir.join("widget.js"), b"x").unwrap(); // exempt
        fs::write(sibling_dir.join("rogue.js"), b"x").unwrap(); // flagged
        let phase = SubstratePurityPhase;
        let ctx = make_ctx(&root.join("static"));
        let findings = phase.run(&ctx).expect("runs");
        assert_eq!(findings.len(), 1, "sibling without marker should flag");
        assert!(findings[0].message.contains("rogue.js"));
        fs::remove_dir_all(&root).ok();
    }
}
