//! `seo` — SERP-quality + a11y checks per HTML page.
//!
//! Bash parity: `phase_seo` in forge.sh — meta description,
//! Open Graph, Twitter Card, canonical, single H1, no heading
//! skips, JSON-LD, lang attr, title length, img alt, sitemap.xml,
//! robots.txt.

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `seo` phase implementation.
#[derive(Debug, Default)]
pub struct SeoPhase;

impl Phase for SeoPhase {
    fn name(&self) -> &'static str {
        "seo"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let files = walk_html(&ctx.static_dir, self.name())?;
        let mut findings = Vec::new();

        for file in files {
            let body = file.body.as_str();
            let n = file.name.clone();

            // Meta description.
            if !body.contains(r#"<meta name="description""#) {
                findings.push(Finding::warn(
                    self.name(),
                    n.clone(),
                    r#"missing <meta name="description">"#,
                ));
            }
            // Open Graph (any of the 5 listed).
            if !any_of(
                body,
                &[
                    r#"<meta property="og:title""#,
                    r#"<meta property="og:description""#,
                    r#"<meta property="og:type""#,
                    r#"<meta property="og:url""#,
                    r#"<meta property="og:image""#,
                ],
            ) {
                findings.push(Finding::warn(
                    self.name(),
                    n.clone(),
                    "missing Open Graph tags (og:title/description/type/url/image)",
                ));
            }
            // Twitter Card.
            if !any_of(
                body,
                &[
                    r#"<meta name="twitter:card""#,
                    r#"<meta name="twitter:title""#,
                    r#"<meta name="twitter:description""#,
                ],
            ) {
                findings.push(Finding::warn(
                    self.name(),
                    n.clone(),
                    "missing Twitter Card tags",
                ));
            }
            // Canonical link.
            if !body.contains(r#"<link rel="canonical""#) {
                findings.push(Finding::warn(
                    self.name(),
                    n.clone(),
                    r#"missing <link rel="canonical">"#,
                ));
            }
            // H1 count.
            let h1 = count_open_tag(body, "h1");
            if h1 == 0 {
                findings.push(Finding::strict(self.name(), n.clone(), "no <h1> on page"));
            } else if h1 > 1 {
                findings.push(Finding::warn(
                    self.name(),
                    n.clone(),
                    format!("{h1} <h1> tags (should be exactly 1)"),
                ));
            }
            // Heading skip h1 → h3 without h2.
            if has_open_tag(body, "h3") && !has_open_tag(body, "h2") {
                findings.push(Finding::warn(
                    self.name(),
                    n.clone(),
                    "heading skip: <h3> present without <h2> (breaks reader navigation)",
                ));
            }
            // JSON-LD.
            if !body.contains("application/ld+json") {
                findings.push(Finding::warn(
                    self.name(),
                    n.clone(),
                    "no JSON-LD structured data",
                ));
            }
            // <html lang=...>.
            if !html_has_lang(body) {
                findings.push(Finding::strict(
                    self.name(),
                    n.clone(),
                    "<html> missing lang attribute (also a11y)",
                ));
            }
            // Title length 20-70 chars (warn outside this band).
            if let Some(title) = extract_title(body) {
                let tlen = title.chars().count();
                if tlen < 20 {
                    findings.push(Finding::warn(
                        self.name(),
                        n.clone(),
                        format!("title too short ({tlen} chars; aim 30-60 for SERP)"),
                    ));
                } else if tlen > 70 {
                    findings.push(Finding::warn(
                        self.name(),
                        n.clone(),
                        format!("title too long ({tlen} chars; truncated in SERP at ~60)"),
                    ));
                }
            }
            // Img without alt.
            let no_alt = count_img_without_alt(body);
            if no_alt > 0 {
                findings.push(Finding::strict(
                    self.name(),
                    n.clone(),
                    format!("{no_alt} <img> without alt (a11y + SEO)"),
                ));
            }
        }

        // Project-wide checks.
        if !ctx.static_dir.join("sitemap.xml").exists() {
            findings.push(Finding::warn(
                self.name(),
                "sitemap.xml",
                "missing sitemap.xml — search-engine crawl coverage suffers",
            ));
        }
        if !ctx.static_dir.join("robots.txt").exists() {
            findings.push(Finding::warn(
                self.name(),
                "robots.txt",
                "missing robots.txt — crawler hint missing",
            ));
        }

        Ok(findings)
    }
}

/// Returns true if `body` contains any of `needles`.
fn any_of(body: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| body.contains(n))
}

/// Count occurrences of `<TAG ` or `<TAG>` (open-tag start with
/// trailing space-or-`>`). Case-insensitive on the tag name.
fn count_open_tag(body: &str, tag: &str) -> usize {
    let lower = body.to_ascii_lowercase();
    let mut count = 0;
    let prefix = format!("<{}", tag.to_ascii_lowercase());
    let mut search = lower.as_str();
    while let Some(idx) = search.find(&prefix) {
        let after = &search[idx + prefix.len()..];
        // BUG ASSUMPTION: tag-name boundary check — next char must
        // be whitespace, `>`, or `/` (for self-closing). Otherwise
        // `<header>` matches as `<h*` for `h1`.
        let next = after.chars().next();
        match next {
            Some(c) if c.is_whitespace() || c == '>' || c == '/' => {
                count += 1;
            }
            _ => {}
        }
        search = after;
    }
    count
}

fn has_open_tag(body: &str, tag: &str) -> bool {
    count_open_tag(body, tag) > 0
}

/// True if `<html ... lang=...>` is present near the start of body.
fn html_has_lang(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    let Some(idx) = lower.find("<html") else {
        return false;
    };
    let after = &lower[idx..];
    let Some(end) = after.find('>') else {
        return false;
    };
    after[..end].contains("lang=")
}

/// Extract the <title> text.
fn extract_title(body: &str) -> Option<String> {
    let lower = body.to_ascii_lowercase();
    let start = lower.find("<title>")?;
    let after = &body[start + "<title>".len()..];
    let lower_after = lower[start + "<title>".len()..].to_owned();
    let end = lower_after.find("</title>")?;
    Some(after[..end].trim().to_owned())
}

/// Count `<img ...>` tags lacking an `alt=` attribute.
fn count_img_without_alt(body: &str) -> usize {
    let lower = body.to_ascii_lowercase();
    let mut count = 0;
    let mut search = lower.as_str();
    while let Some(idx) = search.find("<img") {
        let after = &search[idx..];
        let Some(close) = after.find('>') else {
            break;
        };
        let tag = &after[..close];
        if !tag.contains("alt=") {
            count += 1;
        }
        search = &after[close + 1..];
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_h1_one() {
        let body = "<html><body><h1>Hi</h1></body></html>";
        assert_eq!(count_open_tag(body, "h1"), 1);
    }

    #[test]
    fn count_h1_no_match_for_header() {
        // `<header>` MUST NOT match `<h1` — boundary check.
        let body = "<html><header>x</header><h1>title</h1></html>";
        assert_eq!(count_open_tag(body, "h1"), 1);
    }

    #[test]
    fn html_has_lang_yes() {
        assert!(html_has_lang(r#"<html lang="en"><body></body></html>"#));
        assert!(html_has_lang(r#"<HTML LANG="en">"#));
    }

    #[test]
    fn html_has_lang_no() {
        assert!(!html_has_lang(r#"<html><body></body></html>"#));
    }

    #[test]
    fn extract_title_works() {
        let body = r#"<head><title>  My Page  </title></head>"#;
        assert_eq!(extract_title(body), Some("My Page".to_owned()));
    }

    #[test]
    fn extract_title_missing() {
        assert_eq!(extract_title("<head></head>"), None);
    }

    #[test]
    fn count_img_no_alt() {
        let body = r#"<img src="a.jpg"><img src="b.jpg" alt="ok"><img src="c.jpg">"#;
        assert_eq!(count_img_without_alt(body), 2);
    }
}
