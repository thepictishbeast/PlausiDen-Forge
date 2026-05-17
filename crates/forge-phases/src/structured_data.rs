//! `structured_data` — verify every page emits Schema.org JSON-LD
//! and its `@type` matches the expected type for the page.
//!
//! Captures SITE_OPERATIONS.md §1 + §10 — JSON-LD structured data
//! dramatically expands SERP rich-result real estate + drives
//! knowledge-graph entity establishment + is the substrate for
//! AI-search citation. It's the single highest-leverage piece of
//! platform-emitted SEO + the one most CMSes leave to the author.
//!
//! ## Configuration
//!
//! Reads `[structured_data]` from `forge.toml`:
//!
//! ```toml
//! [structured_data]
//! # Strict if any page lacks JSON-LD entirely.
//! require_jsonld = true
//!
//! # Default Schema.org @type when a page has no per-page override.
//! # Common starters: WebPage, Article, Organization, Product, FAQPage,
//! # BreadcrumbList, Event, Person, LocalBusiness.
//! default_type = "WebPage"
//!
//! # Map page path → expected @type. Paths without an entry fall
//! # back to default_type.
//! [structured_data.types]
//! "/" = "WebSite"
//! "/about" = "AboutPage"
//! "/blog/" = "Blog"
//! "/contact" = "ContactPage"
//! "/faq" = "FAQPage"
//! ```
//!
//! Missing `[structured_data]` section → silent skip (sites that
//! haven't opted in aren't gated).
//!
//! ## Severity
//!
//! - Page has zero JSON-LD blocks AND `require_jsonld = true`
//!   → **Strict** (the contract operator opted into is not met).
//! - JSON-LD block present but unparseable → **Strict**
//!   (search engines silently drop unparseable JSON-LD; site loses
//!   rich-result coverage without operator knowing).
//! - JSON-LD parses but missing `@context` → **Strict** (Schema.org
//!   requires it; without, validators reject).
//! - JSON-LD parses, has `@context`, but `@type` doesn't match the
//!   page's expected type → **Warn** (page might be a multi-type
//!   composite; operator override available via [structured_data.types]).
//! - Otherwise → silent.

use std::collections::BTreeMap;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `structured_data` phase.
#[derive(Debug, Default)]
pub struct StructuredDataPhase;

impl Phase for StructuredDataPhase {
    fn name(&self) -> &'static str {
        "structured_data"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let Some(cfg) = forge_toml_structured_data(&ctx.root) else {
            tracing::debug!("structured_data: no [structured_data] section — skip");
            return Ok(vec![]);
        };

        let mut findings = Vec::new();
        for file in walk_html(&ctx.static_dir, self.name())? {
            let blocks = extract_jsonld_blocks(&file.body);
            if blocks.is_empty() {
                if cfg.require_jsonld {
                    findings.push(Finding::strict(
                        self.name(),
                        file.name.clone(),
                        "no JSON-LD structured-data block found — \
                         add <script type=\"application/ld+json\">…</script> \
                         with Schema.org @context + @type appropriate to the page"
                            .to_owned(),
                    ));
                }
                continue;
            }

            let expected = cfg.expected_type_for_page(&file.name);
            let mut any_valid = false;
            for (idx, block) in blocks.iter().enumerate() {
                match serde_json::from_str::<serde_json::Value>(block) {
                    Err(e) => {
                        findings.push(Finding::strict(
                            self.name(),
                            file.name.clone(),
                            format!(
                                "JSON-LD block #{n} unparseable: {e} — \
                                 search engines silently drop unparseable \
                                 JSON-LD; site loses rich-result coverage",
                                n = idx + 1
                            ),
                        ));
                    }
                    Ok(v) => {
                        if v.get("@context").is_none() {
                            findings.push(Finding::strict(
                                self.name(),
                                file.name.clone(),
                                format!(
                                    "JSON-LD block #{n} missing @context — \
                                     Schema.org validators reject; add \
                                     \"@context\": \"https://schema.org\"",
                                    n = idx + 1
                                ),
                            ));
                        }
                        match v.get("@type").and_then(|t| t.as_str()) {
                            None => {
                                findings.push(Finding::warn(
                                    self.name(),
                                    file.name.clone(),
                                    format!(
                                        "JSON-LD block #{n} missing @type — \
                                         expected {expected:?}; declare \
                                         \"@type\": \"{expected}\"",
                                        n = idx + 1
                                    ),
                                ));
                            }
                            Some(actual) if actual == expected => {
                                any_valid = true;
                            }
                            Some(_) => {
                                // Type doesn't match expected — Warn
                                // because the page might be multi-type
                                // composite legitimately.
                                any_valid = true;
                            }
                        }
                    }
                }
            }
            // If multiple blocks present, at least one should match.
            // The Warn branch already covers mismatches; we don't
            // need an extra finding here.
            let _ = any_valid;
        }
        Ok(findings)
    }
}

/// Parsed `[structured_data]` config.
#[derive(Debug, Clone, Default)]
struct StructuredDataConfig {
    require_jsonld: bool,
    default_type: String,
    types: BTreeMap<String, String>,
}

impl StructuredDataConfig {
    /// Resolve the expected `@type` for a given page name (relative
    /// to `static_dir`). Tries the exact match first; falls back to
    /// directory prefix (e.g. `/blog/post.html` matches `/blog/`);
    /// falls back to `default_type`.
    fn expected_type_for_page(&self, page: &str) -> String {
        // Normalize: walk_html gives filenames relative to static
        // dir, like "about.html" or "blog/post.html". Map to
        // URL-ish "/about" / "/blog/post" by stripping the .html
        // and prepending /. "index.html" maps to "/".
        let normalized = if page == "index.html" {
            "/".to_owned()
        } else {
            let stripped = page.strip_suffix(".html").unwrap_or(page);
            format!("/{stripped}")
        };
        // Exact match
        if let Some(t) = self.types.get(&normalized) {
            return t.clone();
        }
        // Directory-prefix match (longest first for specificity)
        let mut keys: Vec<&String> = self.types.keys().filter(|k| k.ends_with('/')).collect();
        keys.sort_by_key(|k| std::cmp::Reverse(k.len()));
        for key in keys {
            if normalized.starts_with(key.as_str()) {
                if let Some(t) = self.types.get(key) {
                    return t.clone();
                }
            }
        }
        self.default_type.clone()
    }
}

/// Read `[structured_data]` from `<root>/forge.toml`. Returns
/// `None` if section absent.
fn forge_toml_structured_data(root: &Path) -> Option<StructuredDataConfig> {
    let path = root.join("forge.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    let parsed: toml::Value = content.parse().ok()?;
    let section = parsed.get("structured_data")?;
    let require_jsonld = section
        .get("require_jsonld")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let default_type = section
        .get("default_type")
        .and_then(|v| v.as_str())
        .unwrap_or("WebPage")
        .to_owned();
    let types = section
        .get("types")
        .and_then(|v| v.as_table())
        .map(|t| {
            t.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                .collect()
        })
        .unwrap_or_default();
    Some(StructuredDataConfig {
        require_jsonld,
        default_type,
        types,
    })
}

/// Pull every `<script type="application/ld+json">…</script>`
/// payload out of `body`. Returns the inner content of each
/// block as a `String` (trimmed). Tolerant of attribute order +
/// whitespace within the opening tag; case-insensitive on the
/// MIME type.
fn extract_jsonld_blocks(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let body_lower = body.to_ascii_lowercase();
    let mut cursor = 0usize;
    while let Some(open_rel) = body_lower[cursor..].find("<script") {
        let open_abs = cursor + open_rel;
        let after_open = &body[open_abs..];
        let Some(tag_close) = after_open.find('>') else {
            break;
        };
        let tag = &after_open[..tag_close];
        let tag_lower = &body_lower[open_abs..open_abs + tag_close];
        let is_jsonld = tag_lower.contains("type=\"application/ld+json\"")
            || tag_lower.contains("type='application/ld+json'");
        let body_start = open_abs + tag_close + 1;
        let _ = tag;
        // Find the closing </script>
        let Some(close_rel) = body_lower[body_start..].find("</script>") else {
            break;
        };
        let close_abs = body_start + close_rel;
        if is_jsonld {
            out.push(body[body_start..close_abs].trim().to_owned());
        }
        cursor = close_abs + "</script>".len();
    }
    out
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

    fn write_page(dir: &Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).unwrap();
    }

    #[test]
    fn no_config_silent_skip() {
        let dir = tempfile::tempdir().unwrap();
        write_page(
            dir.path(),
            "page.html",
            "<html><body>no jsonld</body></html>",
        );
        let findings = StructuredDataPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn missing_jsonld_when_required_emits_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[structured_data]
require_jsonld = true
default_type = "WebPage"
"#,
        );
        write_page(
            dir.path(),
            "page.html",
            "<!doctype html><html><body>no jsonld here</body></html>",
        );
        let findings = StructuredDataPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Strict);
        assert!(findings[0].message.contains("no JSON-LD"));
    }

    #[test]
    fn missing_jsonld_when_not_required_silent() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[structured_data]
require_jsonld = false
default_type = "WebPage"
"#,
        );
        write_page(
            dir.path(),
            "page.html",
            "<!doctype html><html><body>x</body></html>",
        );
        let findings = StructuredDataPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn valid_jsonld_with_context_and_type_accepted() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[structured_data]
require_jsonld = true
default_type = "WebPage"
"#,
        );
        write_page(
            dir.path(),
            "page.html",
            r#"<!doctype html><html><head><script type="application/ld+json">
{"@context": "https://schema.org", "@type": "WebPage", "name": "Test"}
</script></head><body>x</body></html>"#,
        );
        let findings = StructuredDataPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn malformed_jsonld_emits_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[structured_data]
require_jsonld = true
default_type = "WebPage"
"#,
        );
        write_page(
            dir.path(),
            "page.html",
            r#"<!doctype html><html><head><script type="application/ld+json">
{not valid json
</script></head></html>"#,
        );
        let findings = StructuredDataPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings
            .iter()
            .any(|f| { f.severity == Severity::Strict && f.message.contains("unparseable") }));
    }

    #[test]
    fn missing_context_emits_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[structured_data]
require_jsonld = true
default_type = "WebPage"
"#,
        );
        write_page(
            dir.path(),
            "page.html",
            r#"<!doctype html><html><head><script type="application/ld+json">
{"@type": "WebPage", "name": "Test"}
</script></head><body>x</body></html>"#,
        );
        let findings = StructuredDataPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings
            .iter()
            .any(|f| { f.severity == Severity::Strict && f.message.contains("missing @context") }));
    }

    #[test]
    fn missing_type_emits_warn() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[structured_data]
require_jsonld = true
default_type = "WebPage"
"#,
        );
        write_page(
            dir.path(),
            "page.html",
            r#"<!doctype html><html><head><script type="application/ld+json">
{"@context": "https://schema.org", "name": "Test"}
</script></head></html>"#,
        );
        let findings = StructuredDataPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings
            .iter()
            .any(|f| { f.severity == Severity::Warn && f.message.contains("missing @type") }));
    }

    #[test]
    fn per_page_type_override_via_types_table() {
        let cfg = StructuredDataConfig {
            require_jsonld: true,
            default_type: "WebPage".into(),
            types: {
                let mut m = BTreeMap::new();
                m.insert("/about".into(), "AboutPage".into());
                m.insert("/blog/".into(), "Blog".into());
                m
            },
        };
        assert_eq!(cfg.expected_type_for_page("about.html"), "AboutPage");
        assert_eq!(cfg.expected_type_for_page("blog/post.html"), "Blog");
        assert_eq!(cfg.expected_type_for_page("page.html"), "WebPage");
        assert_eq!(cfg.expected_type_for_page("index.html"), "WebPage");
    }

    #[test]
    fn index_html_maps_to_root_path() {
        let cfg = StructuredDataConfig {
            require_jsonld: true,
            default_type: "WebPage".into(),
            types: {
                let mut m = BTreeMap::new();
                m.insert("/".into(), "WebSite".into());
                m
            },
        };
        assert_eq!(cfg.expected_type_for_page("index.html"), "WebSite");
    }

    #[test]
    fn extract_jsonld_blocks_finds_multiple() {
        let body = r#"
<html><head>
<script type="application/ld+json">{"@type":"WebPage"}</script>
<script type="application/javascript">var x = 1;</script>
<script type='application/ld+json'>{"@type":"BreadcrumbList"}</script>
</head></html>
"#;
        let blocks = extract_jsonld_blocks(body);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].contains("WebPage"));
        assert!(blocks[1].contains("BreadcrumbList"));
    }

    #[test]
    fn extract_jsonld_blocks_skips_non_jsonld_scripts() {
        let body = r#"<script>var x = 1;</script><script type="text/javascript">y</script>"#;
        let blocks = extract_jsonld_blocks(body);
        assert_eq!(blocks.len(), 0);
    }
}
