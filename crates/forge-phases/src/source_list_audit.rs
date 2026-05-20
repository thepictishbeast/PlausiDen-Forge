//! `source_list_audit` — typed `SourceList` quality check.
//!
//! Pairs with the `CmsSection::SourceList` primitive shipped in
//! PlausiDen-Loom commit fd3a173. Enforces bibliography quality
//! AT BUILD TIME: web sources need URLs, malformed URLs fail
//! strict, missing authors warn.
//!
//! Sibling to `disclosure_audit` (which validates `Disclaimer`
//! semantics) — same pattern, different primitive surface.
//!
//! ## What this phase enforces
//!
//! For every `SourceList` section in `cms/*.json`:
//!
//! Per ITEM:
//!
//! * `source-list.web-without-url` strict — item with `kind` of
//!   `web` / `audio` / `video` is missing the `url` field. These
//!   kinds inherently need a URL; the operator forgot one.
//! * `source-list.malformed-url` strict — `url` is present but
//!   doesn't parse as a URL with a scheme. `<a>` would render
//!   as a literal text link that goes nowhere.
//! * `source-list.empty-author` warn — `author` field is empty
//!   or whitespace. The citation has no attribution; reader can't
//!   identify who wrote it.
//! * `source-list.empty-title` strict — `title` field is empty.
//!   A citation with no title is undisclosable; what work?
//!
//! ## Why this is a separate phase
//!
//! The Loom renderer validates SHAPE (Vec<SourceListItem>
//! deserializes); audit validates SEMANTICS (web kind missing
//! URL is well-formed JSON but unusable bibliography).
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * Pure phase walking JSON; no I/O beyond the standard cms
//!   directory read.

use std::fs;

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde_json::Value;

/// Source kinds that REQUIRE a URL — citing a web/audio/video
/// source without naming where to find it is broken.
const URL_REQUIRED_KINDS: &[&str] = &["web", "audio", "video"];

/// `source_list_audit` phase implementation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct SourceListAuditPhase;

impl Phase for SourceListAuditPhase {
    fn name(&self) -> &'static str {
        "source_list_audit"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let cms_dir = ctx.root.join("cms");
        if !cms_dir.is_dir() {
            return Ok(findings);
        }
        let entries = fs::read_dir(&cms_dir).map_err(|e| BuildError::Io {
            context: format!("read_dir {}", cms_dir.display()),
            source: e,
        })?;
        for entry in entries {
            let entry = entry.map_err(|e| BuildError::Io {
                context: format!("read_dir entry in {}", cms_dir.display()),
                source: e,
            })?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let raw = fs::read_to_string(&path).map_err(|e| BuildError::Io {
                context: format!("read {}", path.display()),
                source: e,
            })?;
            let Ok(value) = serde_json::from_str::<Value>(&raw) else {
                continue;
            };
            let path_disp = path.display().to_string();
            check_page(&path_disp, &value, &mut findings, self.name());
        }
        Ok(findings)
    }
}

fn check_page(path: &str, page: &Value, findings: &mut Vec<Finding>, phase: &'static str) {
    let Some(sections) = page.get("sections").and_then(|s| s.as_array()) else {
        return;
    };
    for (sect_idx, section) in sections.iter().enumerate() {
        let Some(tag) = section.get("kind").and_then(|v| v.as_str()) else {
            continue;
        };
        if tag != "source_list" {
            continue;
        }
        let Some(items) = section.get("items").and_then(|v| v.as_array()) else {
            continue;
        };
        for (item_idx, item) in items.iter().enumerate() {
            check_item(path, sect_idx, item_idx, item, findings, phase);
        }
    }
}

fn check_item(
    path: &str,
    sect_idx: usize,
    item_idx: usize,
    item: &Value,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let author = item.get("author").and_then(|v| v.as_str()).unwrap_or("");
    let url = item.get("url").and_then(|v| v.as_str());
    let kind = item.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    let where_at = format!("{path}#section-{sect_idx}-source-{item_idx}");
    // Empty title → strict (citation MUST identify the work).
    if title.trim().is_empty() {
        findings.push(Finding::strict(
            phase,
            where_at.clone(),
            "source_list_audit — SourceListItem.title is empty / whitespace. A citation must identify the work being cited; reader can't follow up otherwise.".to_owned(),
        ));
    }
    // Empty author → warn (sometimes works are anonymous; reader
    // is still missing attribution but the citation is usable).
    if author.trim().is_empty() {
        findings.push(Finding::warn(
            phase,
            where_at.clone(),
            "source_list_audit — SourceListItem.author is empty / whitespace. Anonymous works are legitimate but rare; double-check the citation.".to_owned(),
        ));
    }
    // URL required for web/audio/video kinds.
    if URL_REQUIRED_KINDS.contains(&kind) {
        let url_present = url.map(|u| !u.trim().is_empty()).unwrap_or(false);
        if !url_present {
            findings.push(Finding::strict(
                phase,
                where_at.clone(),
                format!(
                    "source_list_audit — SourceListItem (kind=\"{kind}\") is missing url. Web / audio / video sources inherently need a URL — operator forgot to set one. If the source is legitimately offline / archival, change kind to `report` or `other`."
                ),
            ));
        }
    }
    // Malformed URL (any kind) — strict.
    if let Some(u) = url {
        let trimmed = u.trim();
        if !trimmed.is_empty() && !looks_like_url(trimmed) {
            findings.push(Finding::strict(
                phase,
                where_at,
                format!(
                    "source_list_audit — SourceListItem.url = `{trimmed}` doesn't look like a URL (no scheme://host). Bibliography links should be absolute URLs the reader can click."
                ),
            ));
        }
    }
}

/// Loose URL check — has a scheme:// followed by a host fragment.
/// Doesn't actually parse; just verifies operator wrote a real
/// URL not a path or descriptive string.
#[must_use]
fn looks_like_url(s: &str) -> bool {
    // Standard absolute URL shape: scheme://host(/path)?(?...)?(#...)?
    if let Some(scheme_end) = s.find("://") {
        let scheme = &s[..scheme_end];
        let rest = &s[scheme_end + 3..];
        return !scheme.is_empty()
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
            && !rest.is_empty();
    }
    // mailto:foo@bar.example is a legit URL despite no `://`.
    if s.starts_with("mailto:") && s.contains('@') {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run_check(page: Value) -> Vec<Finding> {
        let mut findings = Vec::new();
        check_page("/cms/test.json", &page, &mut findings, "source_list_audit");
        findings
    }

    #[test]
    fn page_without_source_list_emits_no_findings() {
        let page = json!({
            "sections": [
                { "kind": "paragraph", "text": "Body" }
            ]
        });
        assert!(run_check(page).is_empty());
    }

    #[test]
    fn well_formed_source_list_emits_no_findings() {
        let page = json!({
            "sections": [{
                "kind": "source_list",
                "heading": "Sources",
                "style": "numbered",
                "items": [{
                    "author": "Smith, J.",
                    "title": "On Substrate Doctrine",
                    "url": "https://example.com/substrate",
                    "date_published": "2024-03-15",
                    "kind": "web"
                }]
            }]
        });
        assert!(run_check(page).is_empty());
    }

    #[test]
    fn empty_title_is_strict() {
        let page = json!({
            "sections": [{
                "kind": "source_list",
                "heading": "Sources",
                "style": "numbered",
                "items": [{
                    "author": "Smith, J.",
                    "title": "",
                    "url": "https://example.com",
                    "date_published": null,
                    "kind": "web"
                }]
            }]
        });
        let findings = run_check(page);
        assert!(findings
            .iter()
            .any(|f| f.severity == forge_core::Severity::Strict
                && f.message.contains("title is empty")));
    }

    #[test]
    fn empty_author_is_warn() {
        let page = json!({
            "sections": [{
                "kind": "source_list",
                "heading": "Sources",
                "style": "numbered",
                "items": [{
                    "author": "",
                    "title": "On Anonymity",
                    "url": "https://example.com",
                    "date_published": null,
                    "kind": "web"
                }]
            }]
        });
        let findings = run_check(page);
        assert!(findings
            .iter()
            .any(|f| f.severity == forge_core::Severity::Warn && f.message.contains("author")));
    }

    #[test]
    fn web_without_url_is_strict() {
        let page = json!({
            "sections": [{
                "kind": "source_list",
                "heading": "Sources",
                "style": "numbered",
                "items": [{
                    "author": "Smith, J.",
                    "title": "Web Article With No URL",
                    "url": null,
                    "date_published": null,
                    "kind": "web"
                }]
            }]
        });
        let findings = run_check(page);
        assert!(findings
            .iter()
            .any(|f| f.severity == forge_core::Severity::Strict
                && f.message.contains("missing url")
                && f.message.contains("kind=\"web\"")));
    }

    #[test]
    fn audio_video_without_url_also_strict() {
        for kind in ["audio", "video"] {
            let page = json!({
                "sections": [{
                    "kind": "source_list",
                    "heading": "S",
                    "style": "numbered",
                    "items": [{
                        "author": "A",
                        "title": "T",
                        "url": null,
                        "date_published": null,
                        "kind": kind
                    }]
                }]
            });
            let findings = run_check(page);
            assert!(
                findings
                    .iter()
                    .any(|f| f.severity == forge_core::Severity::Strict
                        && f.message.contains("missing url")),
                "kind {kind} should require URL"
            );
        }
    }

    #[test]
    fn book_article_report_without_url_silent() {
        // These kinds legitimately exist without a URL (printed
        // book, paywalled journal article, offline gov report).
        for kind in ["book", "article", "report", "other"] {
            let page = json!({
                "sections": [{
                    "kind": "source_list",
                    "heading": "S",
                    "style": "numbered",
                    "items": [{
                        "author": "A",
                        "title": "T",
                        "url": null,
                        "date_published": null,
                        "kind": kind
                    }]
                }]
            });
            let findings = run_check(page);
            assert!(
                !findings.iter().any(|f| f.message.contains("missing url")),
                "kind {kind} should not require URL"
            );
        }
    }

    #[test]
    fn malformed_url_is_strict() {
        let page = json!({
            "sections": [{
                "kind": "source_list",
                "heading": "S",
                "style": "numbered",
                "items": [{
                    "author": "A",
                    "title": "T",
                    "url": "not a URL just text",
                    "date_published": null,
                    "kind": "web"
                }]
            }]
        });
        let findings = run_check(page);
        assert!(findings
            .iter()
            .any(|f| f.severity == forge_core::Severity::Strict
                && f.message.contains("doesn't look like a URL")));
    }

    #[test]
    fn looks_like_url_basic_cases() {
        // Positive cases
        assert!(looks_like_url("https://example.com"));
        assert!(looks_like_url("http://example.com/path?q=1"));
        assert!(looks_like_url("https://example.com:8080/x"));
        assert!(looks_like_url("ftp://files.example.com/x"));
        assert!(looks_like_url("mailto:contact@example.com"));
        // Negative cases
        assert!(!looks_like_url("just plain text"));
        assert!(!looks_like_url("example.com")); // no scheme
        assert!(!looks_like_url("/relative/path"));
        assert!(!looks_like_url(""));
        assert!(!looks_like_url("mailto:")); // no @
        assert!(!looks_like_url("://nohost")); // empty scheme
    }

    #[test]
    fn whitespace_url_for_required_kind_is_strict() {
        let page = json!({
            "sections": [{
                "kind": "source_list",
                "heading": "S",
                "style": "numbered",
                "items": [{
                    "author": "A",
                    "title": "T",
                    "url": "   ",
                    "date_published": null,
                    "kind": "web"
                }]
            }]
        });
        let findings = run_check(page);
        assert!(findings.iter().any(|f| f.message.contains("missing url")));
    }

    #[test]
    fn multiple_items_emit_independent_findings() {
        let page = json!({
            "sections": [{
                "kind": "source_list",
                "heading": "S",
                "style": "numbered",
                "items": [
                    {
                        "author": "Smith",
                        "title": "Good citation",
                        "url": "https://example.com/good",
                        "date_published": null,
                        "kind": "web"
                    },
                    {
                        "author": "",
                        "title": "",
                        "url": null,
                        "date_published": null,
                        "kind": "web"
                    }
                ]
            }]
        });
        let findings = run_check(page);
        // Second item has 3 issues (empty title, empty author, no URL).
        // First item is fine.
        assert!(findings.len() >= 3);
        // All findings should reference section-0-source-1 (the second item)
        assert!(findings.iter().all(|f| f.path.contains("source-1")));
    }

    #[test]
    fn non_source_list_sections_silently_skipped() {
        let page = json!({
            "sections": [
                { "kind": "paragraph", "text": "hi" },
                { "kind": "heading", "text": "Title" }
            ]
        });
        assert!(run_check(page).is_empty());
    }

    #[test]
    fn missing_sections_array_silently_skipped() {
        let page = json!({ "title": "no sections" });
        assert!(run_check(page).is_empty());
    }

    #[test]
    fn missing_items_array_silently_skipped() {
        let page = json!({
            "sections": [{
                "kind": "source_list",
                "heading": "S",
                "style": "numbered"
            }]
        });
        assert!(run_check(page).is_empty());
    }
}
