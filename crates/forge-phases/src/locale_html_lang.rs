//! `locale_html_lang` — enforce that every page's `<html lang="...">`
//! attribute matches the site's declared locale set.
//!
//! Captures the ISO 639-1 + SITE_OPERATIONS.md i18n doctrine.
//! `<html lang>` is the single most under-set attribute on the
//! modern web — screen readers depend on it (pronunciation +
//! voice selection), browsers depend on it (hyphenation +
//! locale-aware quotation marks), search engines depend on it
//! (regional ranking + indexing). Missing or wrong `lang` is an
//! accessibility regression with measurable user impact.
//!
//! ## Configuration
//!
//! Reads `[locale]` from `forge.toml`:
//!
//! ```toml
//! [locale]
//! # ISO 639-1 (preferred) or BCP 47 default for the site.
//! default = "en"
//!
//! # Optional: the full set of locales the site declares it
//! # supports. Per-page lang values must match one of these. If
//! # omitted, only `default` is accepted.
//! supported = ["en", "es", "de", "fr", "ja", "zh-CN", "ar"]
//!
//! # Optional: skip this check for specific pages (e.g. an
//! # iframe-only embed page that intentionally has no <html>).
//! skip_pages = ["embeds/widget.html"]
//! ```
//!
//! Missing `[locale]` section → silent skip. Sites that haven't
//! declared a locale contract aren't gated.
//!
//! ## Severity
//!
//! - Missing `<html lang>` entirely → **Strict** (WCAG 2.1 SC
//!   3.1.1 "Language of Page" — Level A baseline)
//! - Present but matches no declared locale → **Strict** (the
//!   site shipped a lang the operator didn't sanction; could be
//!   a copy-paste typo or template drift)
//! - Present + matches default but a multi-locale site → silent
//!
//! ## Lookup
//!
//! Per-page check: walks `static/*.html`, looks for
//! `<html lang="..."` (with whitespace tolerance), extracts the
//! value, compares against declared set. Skips the leading
//! `<!DOCTYPE html>` line.

use std::collections::HashSet;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `locale_html_lang` phase.
#[derive(Debug, Default)]
pub struct LocaleHtmlLangPhase;

impl Phase for LocaleHtmlLangPhase {
    fn name(&self) -> &'static str {
        "locale_html_lang"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let Some(cfg) = forge_toml_locale(&ctx.root) else {
            tracing::debug!("locale_html_lang: no [locale] — skip");
            return Ok(vec![]);
        };
        let allowed: HashSet<String> = if cfg.supported.is_empty() {
            std::iter::once(cfg.default.clone()).collect()
        } else {
            cfg.supported.iter().cloned().collect()
        };

        let mut findings = Vec::new();
        for file in walk_html(&ctx.static_dir, self.name())? {
            if cfg.skip_pages.iter().any(|s| s == &file.name) {
                continue;
            }
            match extract_html_lang(&file.body) {
                None => {
                    findings.push(Finding::strict(
                        self.name(),
                        file.name.clone(),
                        format!(
                            "<html lang=\"...\"> missing — WCAG 2.1 SC 3.1.1 Level A; \
                             screen readers cannot select pronunciation, browsers \
                             cannot select hyphenation rules. Declared default: {}",
                            cfg.default,
                        ),
                    ));
                }
                Some(lang) if !allowed.contains(&lang) => {
                    let allowed_list = {
                        let mut v: Vec<String> = allowed.iter().cloned().collect();
                        v.sort();
                        v.join(", ")
                    };
                    findings.push(Finding::strict(
                        self.name(),
                        file.name.clone(),
                        format!(
                            "<html lang=\"{lang}\"> not in declared locale set [{allowed_list}] \
                             — typo, template drift, or unsanctioned locale; \
                             update [locale].supported in forge.toml if intentional"
                        ),
                    ));
                }
                Some(_) => { /* accepted */ }
            }
        }
        Ok(findings)
    }
}

/// Parsed `[locale]` config.
#[derive(Debug, Clone, Default)]
struct LocaleConfig {
    default: String,
    supported: Vec<String>,
    skip_pages: Vec<String>,
}

/// Read `[locale]` from `<root>/forge.toml`. Returns `None` if
/// file missing, parse error, section absent, or `default`
/// unspecified.
fn forge_toml_locale(root: &Path) -> Option<LocaleConfig> {
    let path = root.join("forge.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    let parsed: toml::Value = content.parse().ok()?;
    let section = parsed.get("locale")?;
    let default = section.get("default").and_then(|v| v.as_str())?.to_owned();
    let supported = section
        .get("supported")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let skip_pages = section
        .get("skip_pages")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    Some(LocaleConfig {
        default,
        supported,
        skip_pages,
    })
}

/// Extract the `lang` attribute value from the page's `<html>`
/// opening tag. Tolerates whitespace + attribute ordering;
/// case-insensitive on the tag name + attribute name; preserves
/// case in the returned value (BCP 47 cares about it for
/// regional subtags — `zh-CN` ≠ `zh-cn` semantically though most
/// browsers normalize).
///
/// Returns `None` if the page has no `<html>` tag or its
/// `<html>` has no `lang` attribute.
fn extract_html_lang(body: &str) -> Option<String> {
    // Find `<html` case-insensitively. Skip leading <!DOCTYPE...>.
    let body_lower = body.to_ascii_lowercase();
    let html_open_idx = body_lower.find("<html")?;
    // Find the closing `>` of the opening tag.
    let after_html = &body[html_open_idx..];
    let close_idx = after_html.find('>')?;
    let tag = &after_html[..close_idx];
    let tag_lower = &body_lower[html_open_idx..html_open_idx + close_idx];

    // Look for ` lang=` (preceded by whitespace) or `\tlang=` etc.
    // We don't allow `xlang=` to false-match `lang=`.
    let lang_marker = " lang=";
    let lang_idx = tag_lower.find(lang_marker)?;
    let after_eq = &tag[lang_idx + lang_marker.len()..];

    // Value can be quoted with `"` or `'`, or bare (rare in HTML5
    // but legal). Handle both quoted styles + bare-until-ws-or-gt.
    let value_start = after_eq.chars().next()?;
    if value_start == '"' || value_start == '\'' {
        let rest = &after_eq[value_start.len_utf8()..];
        let end = rest.find(value_start)?;
        Some(rest[..end].to_owned())
    } else {
        let end = after_eq
            .find(|c: char| c.is_whitespace() || c == '>')
            .unwrap_or(after_eq.len());
        Some(after_eq[..end].to_owned())
    }
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

    #[test]
    fn no_locale_section_silent_skip() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("page.html"),
            "<!doctype html><html><body>x</body></html>",
        )
        .unwrap();
        let findings = LocaleHtmlLangPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn missing_html_lang_emits_strict() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("forge.toml"),
            "[locale]\ndefault = \"en\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("page.html"),
            "<!doctype html><html><body>x</body></html>",
        )
        .unwrap();
        let findings = LocaleHtmlLangPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Strict);
        assert!(findings[0].message.contains("WCAG 2.1 SC 3.1.1"));
    }

    #[test]
    fn matching_lang_accepted() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("forge.toml"),
            "[locale]\ndefault = \"en\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("page.html"),
            r#"<!doctype html><html lang="en"><body>x</body></html>"#,
        )
        .unwrap();
        let findings = LocaleHtmlLangPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn lang_outside_supported_set_emits_strict() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("forge.toml"),
            "[locale]\ndefault = \"en\"\nsupported = [\"en\", \"es\"]\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("page.html"),
            r#"<!doctype html><html lang="de"><body>x</body></html>"#,
        )
        .unwrap();
        let findings = LocaleHtmlLangPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("not in declared locale set"));
        assert!(findings[0].message.contains("[en, es]"));
    }

    #[test]
    fn bcp47_regional_subtag_zh_cn_accepted() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("forge.toml"),
            r#"[locale]
default = "en"
supported = ["en", "zh-CN", "ja", "ko", "ar", "he", "fa", "ru", "de", "fr"]
"#,
        )
        .unwrap();
        for (path, lang) in [
            ("zh.html", "zh-CN"),
            ("ja.html", "ja"),
            ("ko.html", "ko"),
            ("ar.html", "ar"),
            ("he.html", "he"),
            ("fa.html", "fa"),
            ("ru.html", "ru"),
            ("de.html", "de"),
            ("fr.html", "fr"),
        ] {
            std::fs::write(
                dir.path().join(path),
                format!(r#"<!doctype html><html lang="{lang}"><body>x</body></html>"#),
            )
            .unwrap();
        }
        let findings = LocaleHtmlLangPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(
            findings.len(),
            0,
            "every supported lang should be accepted: {findings:?}"
        );
    }

    #[test]
    fn skip_pages_silences_specific_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("forge.toml"),
            "[locale]\ndefault = \"en\"\nskip_pages = [\"widget.html\"]\n",
        )
        .unwrap();
        // widget.html has no <html lang> but is skipped
        std::fs::write(dir.path().join("widget.html"), "<div>embed</div>").unwrap();
        // normal page has it → clean
        std::fs::write(
            dir.path().join("page.html"),
            r#"<!doctype html><html lang="en"><body>x</body></html>"#,
        )
        .unwrap();
        let findings = LocaleHtmlLangPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn single_quoted_lang_attribute_parsed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("forge.toml"),
            "[locale]\ndefault = \"en\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("page.html"),
            "<!doctype html><html lang='en'><body>x</body></html>",
        )
        .unwrap();
        let findings = LocaleHtmlLangPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn extract_html_lang_double_quoted() {
        assert_eq!(
            extract_html_lang(r#"<!doctype html><html lang="en-US" dir="ltr">"#),
            Some("en-US".to_owned())
        );
    }

    #[test]
    fn extract_html_lang_attribute_order_independent() {
        assert_eq!(
            extract_html_lang(r#"<html dir="ltr" lang="ja">"#),
            Some("ja".to_owned())
        );
    }

    #[test]
    fn extract_html_lang_absent_returns_none() {
        assert_eq!(extract_html_lang("<!doctype html><html><body>"), None);
    }

    #[test]
    fn extract_html_lang_no_html_tag_returns_none() {
        assert_eq!(extract_html_lang("<div>fragment</div>"), None);
    }

    #[test]
    fn xlang_does_not_false_match() {
        // Bug-guard: a hypothetical xlang attribute must not
        // be picked up as `lang`. The marker " lang=" requires
        // leading whitespace.
        let extracted = extract_html_lang(r#"<html xlang="en">"#);
        assert_eq!(extracted, None, "must not false-match xlang= as lang=");
    }

    #[test]
    fn case_insensitive_tag_name() {
        // Old HTML sometimes uses <HTML> uppercase. The walker
        // lowercases for the search step but preserves the
        // original case of the returned value.
        assert_eq!(
            extract_html_lang(r#"<HTML lang="en">"#),
            Some("en".to_owned())
        );
    }
}
