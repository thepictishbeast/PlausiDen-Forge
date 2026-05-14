//! `render` — Forge generates content directly from CMS sources
//! using `loom-cms-render` IN-PROCESS, not via shelling out to
//! `loom cms-render`.
//!
//! Owner directive 2026-05-13: "plausiden cms should actually
//! generate content and forge uses it so it should actually
//! generate content."
//!
//! Before this phase landed, Forge was an audit pipeline only —
//! it inspected ALREADY-rendered HTML in `static/`. The bash
//! `forge.sh` (now deprecated) shelled out to `loom cms-render`
//! before invoking the audit phases. With T70, that subprocess
//! shell-out becomes a Rust function call.
//!
//! ## What this phase does
//!
//! 1. Walks `<root>/cms/*.json` files (one per page).
//! 2. Parses each as a typed `loom_cms_render::CmsPage` via
//!    serde — `deny_unknown_fields` catches schema drift before
//!    any HTML is emitted.
//! 3. Calls `loom_cms_render::render_page(&page)` to get the
//!    body markup.
//! 4. Wraps it in a minimal HTML5 shell with `<html lang="en">`,
//!    `<meta name="color-scheme" content="light dark">`, and
//!    `<main id="content">` (the user's a11y + dual-theme
//!    defaults from PlausiDen-Loom T48c v1).
//! 5. Writes the output to `<static_dir>/_render/<slug>.html`
//!    via atomic write (temp + rename, POSIX-atomic on the
//!    same filesystem).
//!
//! v1 outputs to `static/_render/` so it doesn't fight whatever
//! is already in `static/`. T70c flips `static/` to be the
//! canonical output once parity is verified.
//!
//! ## Opt-in
//!
//! v1 only runs when a `cms/` directory exists under `ctx.root`.
//! Sites that don't use the typed CMS (legacy hand-written HTML)
//! pass cleanly with zero findings.
//!
//! ## Doctrine applied
//!
//! * **Type-safe end-to-end** — JSON parse → `CmsPage` → markup,
//!   no string-templating step where escaping could be missed.
//! * **Atomic writes** — same primitive Loom's WriteCapability
//!   uses; no torn writes on power loss.
//! * **No `unwrap`/`expect`** in lib code; lint enforces.
//! * **Schema-validated boundary** — `serde_json::from_str`
//!   fails closed with a phase finding, not a panic.
//!
//! AVP-PASS-T70: 2026-05-13.

use std::path::{Path, PathBuf};

use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// `render` phase implementation.
#[derive(Debug, Default)]
pub struct RenderPhase;

impl Phase for RenderPhase {
    fn name(&self) -> &'static str {
        "render"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let cms_dir = ctx.root.join("cms");
        if !cms_dir.is_dir() {
            // Sites without a cms/ tree pass through silently —
            // they're using the legacy hand-written HTML flow.
            return Ok(Vec::new());
        }

        let out_dir = ctx.static_dir.join("_render");
        if let Err(e) = std::fs::create_dir_all(&out_dir) {
            return Err(BuildError::Io {
                context: format!("render mkdir {}", out_dir.display()),
                source: e,
            });
        }

        let json_files = collect_cms_jsons(&cms_dir)?;
        let mut findings = Vec::new();
        let mut rendered_count = 0usize;

        for json_path in json_files {
            let slug = match slug_from_filename(&json_path) {
                Some(s) => s,
                None => {
                    findings.push(Finding::strict(
                        self.name(),
                        json_path.display().to_string(),
                        "filename is not a valid slug — must match [a-z][a-z0-9-]*".to_owned(),
                    ));
                    continue;
                }
            };
            let raw = match std::fs::read_to_string(&json_path) {
                Ok(s) => s,
                Err(e) => {
                    return Err(BuildError::Io {
                        context: format!("render read {}", json_path.display()),
                        source: e,
                    });
                }
            };
            let page: loom_cms_render::CmsPage = match serde_json::from_str(&raw) {
                Ok(p) => p,
                Err(e) => {
                    // Schema drift — fail closed at the boundary.
                    // Strict-severity so the build blocks until
                    // the JSON is corrected.
                    findings.push(Finding::strict(
                        self.name(),
                        json_path.display().to_string(),
                        format!("CmsPage parse failed: {e}"),
                    ));
                    continue;
                }
            };

            // T70b: call Loom's full a11y / dual-theme page-shell
            // directly. Forge inherits the same WCAG-AA contrast
            // tokens, focus-visible outlines, skip-link styling,
            // and `prefers-reduced-motion` honour as Loom-rendered
            // sites — single source of truth in the render layer.
            let body_markup = loom_cms_render::render_page(&page).into_string();
            let html = loom_cms_render::page_shell(
                &page,
                "/loom-skin.css",
                &body_markup,
                None,
            );

            let out_path = out_dir.join(format!("{slug}.html"));
            if let Err(e) = atomic_write(&out_path, html.as_bytes()) {
                return Err(BuildError::Io {
                    context: format!("render write {}", out_path.display()),
                    source: e,
                });
            }
            rendered_count += 1;
        }

        tracing::info!(
            target: "forge_phases::render",
            rendered = rendered_count,
            "phase_render generated {rendered_count} HTML page(s) from cms/*.json"
        );

        Ok(findings)
    }
}

/// Walk `cms/` for `*.json` files (one level only — sub-pages
/// are not supported in v1; pages map 1:1 to top-level JSON
/// files).
fn collect_cms_jsons(cms_dir: &Path) -> Result<Vec<PathBuf>, BuildError> {
    let entries = match std::fs::read_dir(cms_dir) {
        Ok(e) => e,
        Err(e) => {
            return Err(BuildError::Io {
                context: format!("render read_dir {}", cms_dir.display()),
                source: e,
            });
        }
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                return Err(BuildError::Io {
                    context: format!("render iter {}", cms_dir.display()),
                    source: e,
                });
            }
        };
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
            // Skip the `$schema` companion doc if present.
            if path.file_name().and_then(|s| s.to_str())
                == Some("cms-schema.json")
            {
                continue;
            }
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

/// Extract a slug from a filename like `home.json` → `Some("home")`.
/// Returns None if the stem doesn't match `[a-z][a-z0-9-]*`.
fn slug_from_filename(path: &Path) -> Option<String> {
    let stem = path.file_stem().and_then(|s| s.to_str())?;
    let mut chars = stem.chars();
    let first = chars.next()?;
    if !first.is_ascii_lowercase() {
        return None;
    }
    if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return None;
    }
    if stem.len() > 80 {
        return None;
    }
    Some(stem.to_owned())
}

// T70b: `wrap_html_shell` and `escape_text` removed — replaced
// by direct calls into `loom_cms_render::page_shell` and
// `loom_cms_render::escape_html_text`. Keeps the a11y / dual-
// theme contract co-located with the renderer that owns it.

/// Atomic write: tmp file + rename. POSIX guarantees rename is
/// atomic on the same filesystem.
fn atomic_write(dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = dest.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "dest has no parent")
    })?;
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_name = format!(
        ".{}.tmp.{pid}.{nanos}",
        dest.file_name().and_then(|s| s.to_str()).unwrap_or("out")
    );
    let tmp = parent.join(tmp_name);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx_with_cms(pages: &[(&str, &str)]) -> (BuildCtx, std::path::PathBuf) {
        let tmp = std::env::temp_dir().join(format!(
            "render-t70-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(tmp.join("cms")).expect("mk cms");
        std::fs::create_dir_all(tmp.join("static")).expect("mk static");
        for (name, body) in pages {
            std::fs::write(tmp.join("cms").join(format!("{name}.json")), body)
                .expect("write json");
        }
        let ctx = BuildCtx {
            root: tmp.clone(),
            static_dir: tmp.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        (ctx, tmp)
    }

    fn sample_page(title: &str) -> String {
        serde_json::to_string(&serde_json::json!({
            "title": title,
            "description": "test",
            "path": "/",
            "sections": [
                {"kind": "heading", "level": 2, "text": "Hello"},
                {"kind": "paragraph", "text": "World."}
            ]
        }))
        .expect("serialise sample")
    }

    #[test]
    fn render_passes_through_when_no_cms_dir() {
        let tmp = std::env::temp_dir().join(format!(
            "render-t70-empty-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(tmp.join("static")).expect("mk");
        let ctx = BuildCtx {
            root: tmp.clone(),
            static_dir: tmp.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = RenderPhase.run(&ctx).expect("run");
        assert!(findings.is_empty(), "no cms dir = no findings");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn render_emits_html_for_each_cms_json() {
        let (ctx, tmp) = make_ctx_with_cms(&[
            ("home", &sample_page("Home")),
            ("about", &sample_page("About")),
        ]);
        let findings = RenderPhase.run(&ctx).expect("run");
        assert!(findings.is_empty(), "expected clean run, got {findings:?}");
        let home = std::fs::read_to_string(tmp.join("static/_render/home.html"))
            .expect("home.html");
        let about = std::fs::read_to_string(tmp.join("static/_render/about.html"))
            .expect("about.html");
        // Title from CMS landed in <title>.
        assert!(home.contains("<title>Home</title>"));
        assert!(about.contains("<title>About</title>"));
        // The actual rendered body markup from loom-cms-render
        // includes the heading text and paragraph.
        assert!(home.contains("Hello"), "missing heading: {home}");
        assert!(home.contains("World."), "missing paragraph");
        // a11y defaults from loom_cms_render::page_shell (T70b).
        assert!(home.contains("<main id=\"content\">"));
        assert!(home.contains("color-scheme"));
        assert!(home.contains("lang=\"en\""));
        // Dual-theme + reduced-motion + skip-link styling
        // inherited from loom-cms-render's BASE_THEME_CSS:
        assert!(
            home.contains("prefers-color-scheme:dark"),
            "T70b: phase_render must inherit Loom's dark-mode CSS"
        );
        assert!(
            home.contains("prefers-reduced-motion:reduce"),
            "T70b: phase_render must honour reduced-motion"
        );
        assert!(
            home.contains(".loom-skip:focus"),
            "T70b: phase_render must surface the skip link on focus"
        );
        // CSP inline-style hash present (no unsafe-inline).
        assert!(home.contains("sha256-"), "base-theme must be CSP-pinned");
        assert!(
            !home.contains("'unsafe-inline'"),
            "page_shell must never grant unsafe-inline"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn render_emits_strict_finding_on_schema_drift() {
        let (ctx, tmp) = make_ctx_with_cms(&[
            ("broken", r#"{"title":"X","description":"D","path":"/","sections":[{"kind":"paragraph","body":"WRONG FIELD NAME"}]}"#),
        ]);
        let findings = RenderPhase.run(&ctx).expect("run");
        assert_eq!(findings.len(), 1);
        assert!(
            findings[0].message.contains("CmsPage parse failed"),
            "wrong message: {}",
            findings[0].message
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn render_skips_invalid_slug_filenames() {
        // Filename has uppercase / dot — not a valid slug.
        let (ctx, tmp) = make_ctx_with_cms(&[
            ("good", &sample_page("OK")),
        ]);
        std::fs::write(
            ctx.root.join("cms/Bad.Name.json"),
            sample_page("Bad"),
        )
        .expect("write bad");
        let findings = RenderPhase.run(&ctx).expect("run");
        assert_eq!(findings.len(), 1, "one finding for the bad filename");
        assert!(
            findings[0].message.contains("not a valid slug"),
            "wrong message: {}",
            findings[0].message
        );
        // The good slug still rendered.
        assert!(tmp.join("static/_render/good.html").is_file());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn render_writes_atomically_and_overwrites_existing() {
        let (ctx, tmp) = make_ctx_with_cms(&[
            ("home", &sample_page("v1")),
        ]);
        // First run writes v1.
        let _ = RenderPhase.run(&ctx).expect("run v1");
        let v1 = std::fs::read_to_string(tmp.join("static/_render/home.html"))
            .expect("v1 file");
        assert!(v1.contains("<title>v1</title>"));
        // Update CMS, second run overwrites.
        std::fs::write(tmp.join("cms/home.json"), sample_page("v2")).expect("update");
        let _ = RenderPhase.run(&ctx).expect("run v2");
        let v2 = std::fs::read_to_string(tmp.join("static/_render/home.html"))
            .expect("v2 file");
        assert!(v2.contains("<title>v2</title>"));
        assert!(!v2.contains("<title>v1</title>"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn slug_from_filename_validates() {
        assert_eq!(slug_from_filename(Path::new("home.json")), Some("home".to_owned()));
        assert_eq!(slug_from_filename(Path::new("about-us.json")), Some("about-us".to_owned()));
        assert_eq!(slug_from_filename(Path::new("page-1.json")), Some("page-1".to_owned()));
        assert_eq!(slug_from_filename(Path::new("Home.json")), None);
        assert_eq!(slug_from_filename(Path::new("1home.json")), None);
        assert_eq!(slug_from_filename(Path::new("home page.json")), None);
        // file_stem() drops the directory prefix; the caller
        // (collect_cms_jsons) doesn't recurse, so a sub-dir
        // entry never reaches this validator.
        assert_eq!(slug_from_filename(Path::new(".hidden.json")), None);
    }

    #[test]
    fn collect_cms_jsons_skips_schema_companion() {
        let (ctx, tmp) = make_ctx_with_cms(&[
            ("home", &sample_page("Home")),
        ]);
        std::fs::write(
            ctx.root.join("cms/cms-schema.json"),
            r#"{"$schema": "http://json-schema.org/draft-07/schema#"}"#,
        )
        .expect("write schema");
        let jsons = collect_cms_jsons(&ctx.root.join("cms")).expect("collect");
        let names: Vec<_> = jsons
            .iter()
            .filter_map(|p| p.file_name().and_then(|s| s.to_str()).map(str::to_owned))
            .collect();
        assert!(names.contains(&"home.json".to_owned()));
        assert!(!names.contains(&"cms-schema.json".to_owned()));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
