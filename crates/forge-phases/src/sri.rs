//! `sri` — every same-origin `<link rel=stylesheet>` + `<script
//! src=>` MUST carry an integrity SHA-384 attribute, and the
//! attribute value MUST match the on-disk asset bytes.
//!
//! Bash parity: `phase_sri` in forge.sh — same scope (same-origin,
//! exclude `forge-findings.js`), same hash algorithm (SHA-384).
//! Replaces `inject_sri.py` for VERIFICATION; the inject side
//! (auto-update marker writing) is queued separately.

use std::fs;
use std::path::Path;

use base64::Engine as _;
use forge_core::{BuildCtx, BuildError, Finding, Phase};
use sha2::{Digest, Sha384};

use crate::html_walk::walk_html;

/// Files we never SRI — they're regenerated every build, so the
/// hash would always mismatch a fraction of a second after build.
const EXCLUDE: &[&str] = &["forge-findings.js"];

/// `sri` phase.
#[derive(Debug, Default)]
pub struct SriPhase;

impl Phase for SriPhase {
    fn name(&self) -> &'static str {
        "sri"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let files = walk_html(&ctx.static_dir, self.name())?;
        let mut findings = Vec::new();

        for file in files {
            for tag in scan_assets(&file.body) {
                if is_external(&tag.href) {
                    continue;
                }
                let basename = strip_path(&tag.href);
                if EXCLUDE.iter().any(|x| basename == *x) {
                    continue;
                }
                let disk_path = ctx.static_dir.join(basename);
                if !disk_path.exists() {
                    findings.push(Finding::warn(
                        self.name(),
                        file.name.clone(),
                        format!("{} → {} on disk missing (broken link)", tag.kind, basename),
                    ));
                    continue;
                }

                match tag.integrity.as_deref() {
                    None => {
                        findings.push(Finding::warn(
                            self.name(),
                            file.name.clone(),
                            format!("{} → {} has no integrity attribute", tag.kind, basename),
                        ));
                    }
                    Some(declared) => {
                        let expected = compute_sri(&disk_path, self.name())?;
                        if declared != expected {
                            findings.push(Finding::strict(
                                self.name(),
                                file.name.clone(),
                                format!(
                                    "{} → {}: integrity mismatch (declared {declared}, expected {expected})",
                                    tag.kind, basename
                                ),
                            ));
                        }
                    }
                }
            }
        }

        Ok(findings)
    }
}

/// One SRI-relevant tag we extracted.
#[derive(Debug)]
struct AssetTag {
    /// "link" or "script".
    kind: &'static str,
    /// `href=` (link) or `src=` (script) value.
    href: String,
    /// Declared `integrity="..."` value, if present.
    integrity: Option<String>,
}

/// Scan body for `<link rel="stylesheet" href=...>` and
/// `<script ... src=...>` tags. Substring-based scan; OK for the
/// PoC. forge-html parser will replace this in T11.X.
fn scan_assets(body: &str) -> Vec<AssetTag> {
    let mut out = Vec::new();
    scan_tag(body, "<link", "href=\"", "link", &mut out);
    scan_tag(body, "<script", "src=\"", "script", &mut out);
    // Keep only stylesheet-link tags (filter rel attribute).
    out.retain(|t| {
        t.kind != "link" || {
            // Find original opening tag by re-scanning; cheap because tag count is small.
            let needle = format!("href=\"{}\"", t.href);
            if let Some(idx) = body.find(&needle) {
                // Look back ~200 chars for `<link` and check for rel="stylesheet".
                let start = idx.saturating_sub(200);
                let window = &body[start..idx];
                window.contains(r#"rel="stylesheet""#)
            } else {
                false
            }
        }
    });
    out
}

fn scan_tag(
    body: &str,
    open_prefix: &str,
    attr_prefix: &str,
    kind: &'static str,
    out: &mut Vec<AssetTag>,
) {
    let mut search = body;
    while let Some(open_idx) = search.find(open_prefix) {
        let after_open = &search[open_idx..];
        let Some(close_idx) = after_open.find('>') else {
            break;
        };
        let tag = &after_open[..close_idx];
        if let Some(href) = extract_attr(tag, attr_prefix) {
            let integrity = extract_attr(tag, "integrity=\"");
            out.push(AssetTag {
                kind,
                href,
                integrity,
            });
        }
        search = &after_open[close_idx + 1..];
    }
}

fn extract_attr(tag: &str, prefix: &str) -> Option<String> {
    let idx = tag.find(prefix)?;
    let after = &tag[idx + prefix.len()..];
    let end = after.find('"')?;
    Some(after[..end].to_owned())
}

fn is_external(href: &str) -> bool {
    href.starts_with("http://") || href.starts_with("https://") || href.starts_with("//")
}

fn strip_path(href: &str) -> &str {
    let s = href.trim_start_matches('/');
    s.split('?').next().unwrap_or(s)
}

/// Compute SRI sha384 of a disk file.
fn compute_sri(path: &Path, phase: &str) -> Result<String, BuildError> {
    let bytes = fs::read(path).map_err(|e| BuildError::Io {
        context: format!("{phase}: read {}", path.display()),
        source: e,
    })?;
    let mut h = Sha384::new();
    h.update(&bytes);
    let digest = h.finalize();
    Ok(format!(
        "sha384-{}",
        base64::engine::general_purpose::STANDARD.encode(digest)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_attr_finds() {
        let tag = r#"<link rel="stylesheet" href="x.css" integrity="sha384-abc">"#;
        assert_eq!(extract_attr(tag, "href=\""), Some("x.css".to_owned()));
        assert_eq!(
            extract_attr(tag, "integrity=\""),
            Some("sha384-abc".to_owned())
        );
    }

    #[test]
    fn extract_attr_missing() {
        let tag = r#"<link rel="stylesheet" href="x.css">"#;
        assert_eq!(extract_attr(tag, "integrity=\""), None);
    }

    #[test]
    fn is_external_http() {
        assert!(is_external("http://cdn.example/x.js"));
        assert!(is_external("https://cdn.example/x.js"));
        assert!(is_external("//cdn.example/x.js"));
        assert!(!is_external("/local.js"));
        assert!(!is_external("local.js"));
    }

    #[test]
    fn strip_path_basics() {
        assert_eq!(strip_path("/x.js"), "x.js");
        assert_eq!(strip_path("x.js"), "x.js");
        assert_eq!(strip_path("/x.js?v=1"), "x.js");
    }

    #[test]
    fn scan_assets_finds_link_and_script() {
        let body = r#"
            <link rel="stylesheet" href="loom-skin.css" integrity="sha384-XYZ">
            <script defer src="theme.js" integrity="sha384-AAA"></script>
            <script defer src="https://cdn/external.js"></script>
        "#;
        let tags = scan_assets(body);
        assert_eq!(tags.len(), 3); // 1 link + 2 scripts; filter step is in the phase
        let links: Vec<_> = tags.iter().filter(|t| t.kind == "link").collect();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].href, "loom-skin.css");
        assert_eq!(links[0].integrity.as_deref(), Some("sha384-XYZ"));
    }

    #[test]
    fn scan_assets_skips_non_stylesheet_link() {
        let body = r#"<link rel="icon" href="favicon.ico">"#;
        let tags = scan_assets(body);
        // Phase filter retains only stylesheet links; this scan
        // returns the raw set, which the phase filters down.
        // The icon link MUST be filtered out by the phase filter.
        let links: Vec<_> = tags.iter().filter(|t| t.kind == "link").collect();
        assert_eq!(links.len(), 0, "icon link should be filtered by rel check");
    }
}
