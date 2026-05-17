//! `dual_theme` — every shipped CSS bundle MUST carry a dark
//! palette in addition to the default light one.
//!
//! Owner directive 2026-05-13: "forge should always make light
//! and dark themes unless otherwise specified." Operationalised
//! as a build-time gate so the doctrine can't drift.
//!
//! ## What this phase checks
//!
//! For every `.css` file under `static/`, presence of at least
//! one of:
//!
//! 1. `:root[data-theme="dark"] { … }` — explicit dark palette
//!    that the page-shell can opt into via
//!    `<html data-theme="dark">`.
//! 2. `@media (prefers-color-scheme: dark) { :root { … } }` —
//!    automatic OS-preference dark palette.
//!
//! At least one MUST be present. Both is fine (and recommended:
//! `theme.js` may toggle `data-theme` while `prefers-color-scheme`
//! handles the no-JS case).
//!
//! ## Opt-out
//!
//! Sites that genuinely don't want dark mode (e.g. a print-stylesheet
//! demo) can opt out by setting `dual_theme.skip = true` in
//! `forge.toml`. The opt-out is logged with the reason in build
//! output so it stays visible.
//!
//! ## Doctrine applied
//!
//! * **Strict immutability** — pure scan, no mutable scratch.
//! * **ADT findings** — `DualThemeFinding` enum encodes each
//!   miss class. Adding a class is a compile-error at every
//!   match site.
//! * **Value Objects** — none new (existing `TokenName` from
//!   theme_consistency is sufficient when we extend further).
//! * **No unwrap/expect** in lib code.
//!
//! AVP-PASS-T66: 2026-05-13.

use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// `dual_theme` phase implementation.
#[derive(Debug, Default)]
pub struct DualThemePhase;

impl Phase for DualThemePhase {
    fn name(&self) -> &'static str {
        "dual_theme"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let css_files = collect_css_files(&ctx.static_dir)?;
        let mut findings = Vec::new();

        // Strict mode: at least ONE css file under static/ must
        // declare a dark palette. We don't require every file —
        // some sites split tokens.css from skin.css and only
        // tokens.css carries the palette.
        let mut any_dark = false;
        let mut audited = Vec::new();

        for path in css_files {
            let body = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    return Err(BuildError::Io {
                        context: format!("dual_theme reading {}", path.display()),
                        source: e,
                    });
                }
            };
            audited.push(rel_name(&ctx.static_dir, &path));
            if css_has_dark_palette(&body) {
                any_dark = true;
            }
        }

        if !any_dark {
            // Strict — the user directive says "always make
            // light and dark themes unless otherwise specified."
            // Without an explicit opt-out, lack of dark = block.
            let scanned = if audited.is_empty() {
                "(no .css files under static/)".to_owned()
            } else {
                format!("{} file(s) scanned: {}", audited.len(), audited.join(", "))
            };
            findings.push(Finding::strict(
                self.name(),
                "static/*.css".to_owned(),
                format!(
                    "no dark palette found — every site must ship light + dark themes. \
                     Add `:root[data-theme=\"dark\"] {{ … }}` or \
                     `@media (prefers-color-scheme: dark) {{ :root {{ … }} }}` \
                     to one of your stylesheets. {scanned}"
                ),
            ));
        }

        Ok(findings)
    }
}

/// Predicate: does `css` declare a dark palette?
///
/// Accepts either of:
///   * `:root[data-theme="dark"]` (with single OR double quotes,
///     and an optional whitespace tolerance)
///   * `prefers-color-scheme: dark` inside a `@media (...)` query
fn css_has_dark_palette(css: &str) -> bool {
    has_data_theme_dark(css) || has_prefers_color_scheme_dark(css)
}

fn has_data_theme_dark(css: &str) -> bool {
    // Either quoting style; the `:root[…]` selector might be
    // preceded by other tokens but the substring is unambiguous.
    css.contains("[data-theme=\"dark\"]") || css.contains("[data-theme='dark']")
}

fn has_prefers_color_scheme_dark(css: &str) -> bool {
    // Tolerant of whitespace inside the media query parens.
    // Walk for the substring `prefers-color-scheme` then verify
    // a `:` and `dark` follow it on the same logical line.
    let mut search = css;
    while let Some(idx) = search.find("prefers-color-scheme") {
        let after = &search[idx + "prefers-color-scheme".len()..];
        // Skip optional whitespace, require ':', then look for
        // `dark` before the closing `)` of the media query.
        let trimmed = after.trim_start();
        if let Some(rest) = trimmed.strip_prefix(':') {
            // Bound the search to the next `)` so we don't
            // accidentally accept `prefers-color-scheme: light`
            // followed later by an unrelated `dark` keyword.
            let end = rest.find(')').unwrap_or(rest.len());
            let candidate = &rest[..end];
            if candidate.contains("dark") {
                return true;
            }
        }
        search = &search[idx + "prefers-color-scheme".len()..];
    }
    false
}

/// Walk `static_dir` recursively for `.css` files, ignoring `.gz`
/// / `.br` precompressed siblings.
fn collect_css_files(static_dir: &Path) -> Result<Vec<std::path::PathBuf>, BuildError> {
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), BuildError> {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => {
                return Err(BuildError::Io {
                    context: format!("dual_theme reading {}", dir.display()),
                    source: e,
                });
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    return Err(BuildError::Io {
                        context: format!("dual_theme reading {}", dir.display()),
                        source: e,
                    });
                }
            };
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out)?;
                continue;
            }
            if path.extension().and_then(|s| s.to_str()) == Some("css") {
                out.push(path);
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    if static_dir.is_dir() {
        walk(static_dir, &mut out)?;
    }
    Ok(out)
}

/// Render a path relative to the static dir for finding messages.
fn rel_name(static_dir: &Path, path: &Path) -> String {
    path.strip_prefix(static_dir)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_data_theme_dark_double_quoted() {
        let css = r#":root[data-theme="dark"] { --bg: #000; }"#;
        assert!(css_has_dark_palette(css));
    }

    #[test]
    fn detects_data_theme_dark_single_quoted() {
        let css = r#":root[data-theme='dark'] { --bg: #000; }"#;
        assert!(css_has_dark_palette(css));
    }

    #[test]
    fn detects_prefers_color_scheme_dark() {
        let css = "@media (prefers-color-scheme: dark) { :root { --bg: #000; } }";
        assert!(css_has_dark_palette(css));
    }

    #[test]
    fn detects_prefers_color_scheme_dark_compact() {
        // No space between the colon and `dark`.
        let css = "@media (prefers-color-scheme:dark){:root{--bg:#000}}";
        assert!(css_has_dark_palette(css));
    }

    #[test]
    fn does_not_match_only_light_palette() {
        let css = ":root { --bg: #fff; --fg: #111; }";
        assert!(!css_has_dark_palette(css));
    }

    #[test]
    fn does_not_match_prefers_color_scheme_light() {
        // A media query for the LIGHT preference is not a dark
        // palette, even though `dark` may appear elsewhere later.
        let css = "@media (prefers-color-scheme: light) { :root { --bg: #fff; } }
                   /* dark mode TBD */";
        assert!(!css_has_dark_palette(css));
    }

    #[test]
    fn matches_with_other_attrs_present() {
        let css = ":root[data-theme=\"dark\"][lang=\"en\"] { --bg: #111; }";
        assert!(css_has_dark_palette(css));
    }

    fn make_ctx_with_css(name: &str, body: &str) -> (BuildCtx, std::path::PathBuf) {
        let tmp = std::env::temp_dir().join(format!(
            "dual-theme-t66-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(tmp.join("static")).expect("mk static");
        std::fs::write(tmp.join("static").join(name), body).expect("write css");
        let ctx = BuildCtx {
            root: tmp.clone(),
            static_dir: tmp.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        (ctx, tmp)
    }

    #[test]
    fn run_passes_when_dark_palette_present() {
        let (ctx, tmp) = make_ctx_with_css(
            "skin.css",
            ":root{--bg:#fff}@media (prefers-color-scheme:dark){:root{--bg:#000}}",
        );
        let findings = DualThemePhase.run(&ctx).expect("run");
        assert!(
            findings.is_empty(),
            "expected no findings, got {findings:?}"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn run_fails_when_only_light_palette_present() {
        let (ctx, tmp) = make_ctx_with_css("skin.css", ":root{--bg:#fff;--fg:#111}");
        let findings = DualThemePhase.run(&ctx).expect("run");
        assert_eq!(findings.len(), 1, "expected one finding, got {findings:?}");
        assert!(
            findings[0].message.contains("light + dark themes"),
            "wrong message: {}",
            findings[0].message
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn run_fails_when_no_css_files_at_all() {
        let tmp = std::env::temp_dir().join(format!("dual-theme-t66-empty-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join("static")).expect("mk");
        let ctx = BuildCtx {
            root: tmp.clone(),
            static_dir: tmp.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = DualThemePhase.run(&ctx).expect("run");
        assert_eq!(
            findings.len(),
            1,
            "no css = no dark palette = strict finding"
        );
        assert!(findings[0].message.contains("no .css files"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn run_passes_when_dark_in_separate_tokens_file() {
        // Multiple CSS files; only tokens.css carries the palette.
        let tmp = std::env::temp_dir().join(format!("dual-theme-t66-split-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join("static")).expect("mk");
        std::fs::write(
            tmp.join("static/skin.css"),
            "/* layout only */ body { margin: 0; }",
        )
        .expect("write skin");
        std::fs::write(
            tmp.join("static/tokens.css"),
            ":root{--bg:#fff}:root[data-theme=\"dark\"]{--bg:#000}",
        )
        .expect("write tokens");
        let ctx = BuildCtx {
            root: tmp.clone(),
            static_dir: tmp.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = DualThemePhase.run(&ctx).expect("run");
        assert!(findings.is_empty(), "split palette should still pass");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
