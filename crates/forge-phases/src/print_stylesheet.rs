//! `print_stylesheet` — verify a print stylesheet exists + meets
//! minimum quality.
//!
//! Per SITE_OPERATIONS.md §10 (things-most-site-owners-forget).
//! Most sites print terribly because no one writes the print CSS
//! and authors only test the screen viewport. Fixing this is one
//! tiny stylesheet, but verifying it exists is a build-time gate
//! that prevents the regression class entirely.
//!
//! A good print stylesheet:
//! - Drops navigation chrome (header / nav / footer / sidebar)
//! - Expands link URLs as text (`a[href]::after { content: " (" attr(href) ")" }`)
//! - Removes background fills + ensures black-on-white text
//! - Avoids page-breaks inside critical blocks
//! - Uses serif typography for body text (typographic convention
//!   for print + better readability at print resolutions)
//!
//! This phase checks the FIRST property — that a print stylesheet
//! exists in some form. Operators can opt into stricter quality
//! gates via `[print_stylesheet] check_quality = true` which
//! verifies the rest.
//!
//! ## Configuration
//!
//! Reads `[print_stylesheet]` from `forge.toml`:
//!
//! ```toml
//! [print_stylesheet]
//! # Severity for missing print CSS:
//! # - "strict" → required (e.g. for content sites where readers print)
//! # - "warn"   → recommended (the default for most sites)
//! severity = "warn"
//!
//! # Optional: turn on quality checks beyond presence.
//! # When true, also verifies @media print includes:
//! #   - link-URL expansion
//! #   - background-color removal
//! # Each missing quality check adds a separate Warn finding.
//! check_quality = false
//!
//! # Optional: skip the check for sites that legitimately don't
//! # need print styles (e.g. interactive apps with no readable
//! # content).
//! skip = false
//! ```
//!
//! Missing `[print_stylesheet]` section → silent skip.
//!
//! ## What counts as "a print stylesheet exists"
//!
//! - `@media print { ... }` block anywhere in any `*.css` under
//!   `static_dir`
//! - `<link rel="stylesheet" media="print" href="...">` in any
//!   shipped HTML page
//!
//! Either form satisfies the presence check.

use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `print_stylesheet` phase.
#[derive(Debug, Default)]
pub struct PrintStylesheetPhase;

impl Phase for PrintStylesheetPhase {
    fn name(&self) -> &'static str {
        "print_stylesheet"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let Some(cfg) = forge_toml_print_stylesheet(&ctx.root) else {
            tracing::debug!("print_stylesheet: no [print_stylesheet] — skip");
            return Ok(vec![]);
        };
        if cfg.skip {
            tracing::debug!("print_stylesheet: skip = true — skip");
            return Ok(vec![]);
        }

        let mut findings = Vec::new();
        let (has_print_block, print_css_body) = scan_for_print_stylesheet(&ctx.static_dir)?;

        if !has_print_block {
            let msg = "no print stylesheet found — pages will print with screen \
                       styles (navigation chrome visible, background fills wasting \
                       ink, link URLs hidden). Add @media print {...} to your \
                       main CSS or <link rel=\"stylesheet\" media=\"print\" href=\"...\"> \
                       to a page."
                .to_owned();
            findings.push(match cfg.severity {
                SeverityPolicy::Strict => Finding::strict(self.name(), "static/", msg),
                SeverityPolicy::Warn => Finding::warn(self.name(), "static/", msg),
            });
            // Quality checks irrelevant if there's no print CSS at all
            return Ok(findings);
        }

        if cfg.check_quality {
            if !print_css_body.contains("attr(href)") {
                findings.push(Finding::warn(
                    self.name(),
                    "static/",
                    "print stylesheet present but doesn't expand link URLs — \
                     add `a[href]::after { content: \" (\" attr(href) \")\"; }` \
                     inside @media print so readers can find linked references \
                     when reading printed output"
                        .to_owned(),
                ));
            }
            // Check that some explicit "remove backgrounds" pattern
            // appears. Either `background: none` / `background-color:
            // transparent` / `background: white` / `print-color-adjust:
            // economy` are acceptable signals.
            let has_bg_normalize = print_css_body.contains("background: none")
                || print_css_body.contains("background-color: transparent")
                || print_css_body.contains("background-color: white")
                || print_css_body.contains("background: white")
                || print_css_body.contains("print-color-adjust: economy");
            if !has_bg_normalize {
                findings.push(Finding::warn(
                    self.name(),
                    "static/",
                    "print stylesheet present but doesn't normalize backgrounds — \
                     add `background: none` or `print-color-adjust: economy` to \
                     `* { ... }` inside @media print so readers don't waste ink \
                     on screen-mode color fills"
                        .to_owned(),
                ));
            }
        }
        Ok(findings)
    }
}

/// Walk static_dir for any @media print block in CSS files OR
/// any `<link rel="stylesheet" media="print">` in HTML pages.
/// Returns (any_present, accumulated_print_css_body).
fn scan_for_print_stylesheet(static_dir: &Path) -> Result<(bool, String), BuildError> {
    let mut any = false;
    let mut accumulated = String::new();

    // CSS files: walk one level (matches walk_html semantics — the
    // current dogfood + new sites keep flat static/ trees).
    let entries = std::fs::read_dir(static_dir).map_err(|source| BuildError::Io {
        context: format!("print_stylesheet: read_dir {}", static_dir.display()),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| BuildError::Io {
            context: format!("print_stylesheet: iterate {}", static_dir.display()),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("css") {
            continue;
        }
        let body = std::fs::read_to_string(&path).map_err(|source| BuildError::Io {
            context: format!("print_stylesheet: read {}", path.display()),
            source,
        })?;
        if let Some(block) = extract_print_media_block(&body) {
            any = true;
            accumulated.push_str(&block);
            accumulated.push('\n');
        }
    }

    // HTML pages: any <link rel="stylesheet" media="print">
    // counts as presence (we don't read the linked file's content
    // here — operators using this pattern already have the print
    // file out-of-tree or in a CDN-served bundle).
    let html_files = walk_html(static_dir, "print_stylesheet")?;
    for file in html_files {
        if file.body.contains("media=\"print\"") || file.body.contains("media='print'") {
            any = true;
        }
    }

    Ok((any, accumulated))
}

/// Return the BODY of the first `@media print { ... }` block in
/// the CSS. Tracks brace depth to find the matching closing `}`.
fn extract_print_media_block(css: &str) -> Option<String> {
    let lower = css.to_ascii_lowercase();
    let mut search_start = 0;
    while let Some(idx) = lower[search_start..].find("@media") {
        let abs = search_start + idx;
        let after = &lower[abs..];
        let Some(brace_rel) = after.find('{') else {
            return None;
        };
        let header = &after[..brace_rel];
        if header.contains("print") && !header.contains("not print") {
            // Found a print media block. Track brace depth.
            let body_start_abs = abs + brace_rel + 1;
            let mut depth: i32 = 1;
            let bytes = lower.as_bytes();
            let mut i = body_start_abs;
            while i < bytes.len() && depth > 0 {
                match bytes[i] {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                }
                i += 1;
            }
            // i is just past the closing brace
            let body_end = i.saturating_sub(1);
            return Some(css[body_start_abs..body_end].to_owned());
        }
        search_start = abs + brace_rel + 1;
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SeverityPolicy {
    Strict,
    #[default]
    Warn,
}

#[derive(Debug, Clone, Default)]
struct PrintConfig {
    severity: SeverityPolicy,
    check_quality: bool,
    skip: bool,
}

fn forge_toml_print_stylesheet(root: &Path) -> Option<PrintConfig> {
    let path = root.join("forge.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    let parsed: toml::Value = content.parse().ok()?;
    let section = parsed.get("print_stylesheet")?;
    let severity = match section.get("severity").and_then(|v| v.as_str()) {
        Some("strict") => SeverityPolicy::Strict,
        _ => SeverityPolicy::Warn,
    };
    let check_quality = section
        .get("check_quality")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let skip = section
        .get("skip")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    Some(PrintConfig {
        severity,
        check_quality,
        skip,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::{BuildMode, Severity};

    fn ctx_in(dir: &Path) -> BuildCtx {
        BuildCtx {
            root: dir.to_path_buf(),
            static_dir: dir.to_path_buf(),
            mode: BuildMode::Poc,
        }
    }

    fn write_forge_toml(dir: &Path, body: &str) {
        std::fs::write(dir.join("forge.toml"), body).unwrap();
    }

    #[test]
    fn no_config_silent_skip() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("x.css"), "body { color: red; }").unwrap();
        let findings = PrintStylesheetPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn missing_print_stylesheet_emits_warn_default() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[print_stylesheet]\n");
        std::fs::write(dir.path().join("x.css"), "body { color: red; }").unwrap();
        let findings = PrintStylesheetPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warn);
        assert!(findings[0].message.contains("no print stylesheet"));
    }

    #[test]
    fn missing_print_stylesheet_emits_strict_when_strict_severity() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[print_stylesheet]\nseverity = \"strict\"\n");
        std::fs::write(dir.path().join("x.css"), "body { color: red; }").unwrap();
        let findings = PrintStylesheetPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings[0].severity, Severity::Strict);
    }

    #[test]
    fn print_media_block_in_css_accepted() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[print_stylesheet]\n");
        std::fs::write(
            dir.path().join("x.css"),
            "body { color: red; }\n@media print { nav { display: none; } }",
        )
        .unwrap();
        let findings = PrintStylesheetPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn print_link_in_html_accepted() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[print_stylesheet]\n");
        std::fs::write(
            dir.path().join("page.html"),
            r#"<html><head><link rel="stylesheet" media="print" href="/print.css"></head></html>"#,
        )
        .unwrap();
        let findings = PrintStylesheetPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn skip_true_silences_phase() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            "[print_stylesheet]\nseverity = \"strict\"\nskip = true\n",
        );
        std::fs::write(dir.path().join("x.css"), "body { color: red; }").unwrap();
        let findings = PrintStylesheetPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn check_quality_warns_on_missing_url_expansion() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[print_stylesheet]\ncheck_quality = true\n");
        std::fs::write(
            dir.path().join("x.css"),
            "@media print { nav { display: none; } * { background: none; } }",
        )
        .unwrap();
        let findings = PrintStylesheetPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings
            .iter()
            .any(|f| f.message.contains("expand link URLs")));
    }

    #[test]
    fn check_quality_warns_on_missing_background_normalize() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[print_stylesheet]\ncheck_quality = true\n");
        std::fs::write(
            dir.path().join("x.css"),
            r#"@media print { a[href]::after { content: " (" attr(href) ")"; } }"#,
        )
        .unwrap();
        let findings = PrintStylesheetPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings
            .iter()
            .any(|f| f.message.contains("normalize backgrounds")));
    }

    #[test]
    fn check_quality_clean_when_both_satisfied() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[print_stylesheet]\ncheck_quality = true\n");
        std::fs::write(
            dir.path().join("x.css"),
            r#"@media print {
                * { background: none; }
                a[href]::after { content: " (" attr(href) ")"; }
                nav, footer { display: none; }
            }"#,
        )
        .unwrap();
        let findings = PrintStylesheetPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn print_color_adjust_economy_satisfies_normalize() {
        // print-color-adjust: economy is the modern equivalent of
        // background: none — both signal "save ink"
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[print_stylesheet]\ncheck_quality = true\n");
        std::fs::write(
            dir.path().join("x.css"),
            r#"@media print {
                * { print-color-adjust: economy; }
                a[href]::after { content: " (" attr(href) ")"; }
            }"#,
        )
        .unwrap();
        let findings = PrintStylesheetPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(!findings
            .iter()
            .any(|f| f.message.contains("normalize backgrounds")));
    }

    #[test]
    fn extract_print_media_block_finds_body_with_nested_braces() {
        let css = "body { color: red; }
@media print {
    @supports (display: grid) {
        .grid { display: block; }
    }
    nav { display: none; }
}
.after { color: blue; }";
        let body = extract_print_media_block(css).expect("found");
        assert!(body.contains("nav { display: none; }"));
        assert!(body.contains("@supports"));
        assert!(!body.contains(".after"));
    }

    #[test]
    fn not_print_media_query_skipped() {
        let css = "@media not print { body { color: red; } }";
        assert!(extract_print_media_block(css).is_none());
    }
}
