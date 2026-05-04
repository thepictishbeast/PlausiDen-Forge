//! `unbuilt_route` — every internal `href` in shipped HTML must
//! resolve to a file in `static/`. Catches dead navigation links
//! before the crawler (or a user) hits a 404.
//!
//! Bash parity: `phase_unbuilt_route`. Path normalization:
//!   `/`               → `index.html`
//!   `/foo.html`       → `foo.html`
//!   `./bar.html`      → `bar.html`
//!   `bar.html#frag`   → `bar.html` (fragment stripped)
//! Skips: absolute URLs (canonical refs), `mailto:`, `tel:`,
//! pure fragments (`#section`).

use std::collections::BTreeSet;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `unbuilt_route` phase.
#[derive(Debug, Default)]
pub struct UnbuiltRoutePhase;

impl Phase for UnbuiltRoutePhase {
    fn name(&self) -> &'static str {
        "unbuilt_route"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let files = walk_html(&ctx.static_dir, self.name())?;
        let mut findings = Vec::new();

        for file in &files {
            let mut seen: BTreeSet<String> = BTreeSet::new();
            for href in extract_internal_hrefs(&file.body) {
                if !seen.insert(href.clone()) {
                    continue;
                }
                let Some(target) = normalize_href(&href) else {
                    continue;
                };
                let path = ctx.static_dir.join(&target);
                if !path.exists() {
                    findings.push(Finding::strict(
                        self.name(),
                        file.name.clone(),
                        format!("href=\"{href}\" → static/{target} does not exist"),
                    ));
                }
            }
        }

        Ok(findings)
    }
}

/// Pull every `href="..."` where the value is not absolute.
fn extract_internal_hrefs(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let needle = "href=\"";
    let mut search = body;
    while let Some(idx) = search.find(needle) {
        let after = &search[idx + needle.len()..];
        if let Some(end) = after.find('"') {
            let val = &after[..end];
            if !is_absolute(val) && !is_special_scheme(val) {
                out.push(val.to_owned());
            }
            search = &after[end + 1..];
        } else {
            break;
        }
    }
    out
}

fn is_absolute(href: &str) -> bool {
    href.starts_with("http://") || href.starts_with("https://") || href.starts_with("//")
}

fn is_special_scheme(href: &str) -> bool {
    href.starts_with("mailto:") || href.starts_with("tel:") || href.starts_with('#')
}

/// Normalize an internal href to a relative-from-static path.
/// Returns None for hrefs that don't reference a buildable file
/// (pure fragments after stripping a leading slash, etc.).
fn normalize_href(href: &str) -> Option<String> {
    // Drop fragment.
    let without_frag = match href.find('#') {
        Some(i) => &href[..i],
        None => href,
    };
    // Drop query string.
    let path = match without_frag.find('?') {
        Some(i) => &without_frag[..i],
        None => without_frag,
    };
    if path.is_empty() {
        return None;
    }
    // Map `/` → `index.html`.
    if path == "/" {
        return Some("index.html".to_owned());
    }
    let stripped = path.trim_start_matches("./").trim_start_matches('/');
    if stripped.is_empty() {
        return None;
    }
    Some(stripped.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_root() {
        assert_eq!(normalize_href("/"), Some("index.html".to_owned()));
    }

    #[test]
    fn normalize_strips_fragment() {
        assert_eq!(
            normalize_href("/page.html#section"),
            Some("page.html".to_owned())
        );
    }

    #[test]
    fn normalize_strips_query() {
        assert_eq!(
            normalize_href("/page.html?ref=nav"),
            Some("page.html".to_owned())
        );
    }

    #[test]
    fn normalize_relative() {
        assert_eq!(
            normalize_href("./contact.html"),
            Some("contact.html".to_owned())
        );
    }

    #[test]
    fn normalize_fragment_only_returns_none() {
        assert_eq!(normalize_href("#"), None);
    }

    #[test]
    fn extract_skips_external() {
        // BUG ASSUMPTION: this raw string uses r##"..."## (double
        // hash) because the body contains the literal sequence
        // `"#section"` which would prematurely close a single-hash
        // raw string. Caught at compile time first attempt — cargo
        // diagnostics pointed straight at the offending line.
        let body = r##"
            <a href="https://example.com">x</a>
            <a href="/local.html">y</a>
            <a href="mailto:x@y">m</a>
            <a href="#section">f</a>
        "##;
        let hrefs = extract_internal_hrefs(body);
        assert_eq!(hrefs, vec!["/local.html"]);
    }
}
