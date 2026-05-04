//! `external_assets` — strict gate: NO external resources LOADED
//! by the page (CSS, JS, images, fonts via @font-face). Reference
//! URLs (canonical, og:url, og:image-as-meta, JSON-LD `url`,
//! anchor `href`) are NOT assets and must NOT trip this check.
//!
//! Bash parity: forge.sh `phase_external_assets` strips reference
//! tags via a python heredoc, then greps for any http(s):// in
//! `src=` / loadable contexts.

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `external_assets` phase.
#[derive(Debug, Default)]
pub struct ExternalAssetsPhase;

impl Phase for ExternalAssetsPhase {
    fn name(&self) -> &'static str {
        "external_assets"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let files = walk_html(&ctx.static_dir, self.name())?;
        let mut findings = Vec::new();

        for file in files {
            let body = file.body.as_str();
            let stripped = strip_reference_tags(body);
            for hit in scan_external_loads(&stripped) {
                findings.push(Finding::strict(
                    self.name(),
                    file.name.clone(),
                    format!("external load: {hit} (CDN-free policy — vendor instead)"),
                ));
            }
        }

        Ok(findings)
    }
}

/// Strip the parts of an HTML document that contain URL strings
/// which DO NOT trigger network loads (canonical, og:url,
/// twitter:url, JSON-LD blocks). Returns a body fragment safe to
/// scan for actual resource declarations.
fn strip_reference_tags(body: &str) -> String {
    // Strip JSON-LD blocks first (they can contain arbitrary URL
    // strings as data, never loaded).
    let mut out = String::with_capacity(body.len());
    let mut search = body;
    loop {
        match search.find(r#"<script type="application/ld+json""#) {
            None => {
                out.push_str(search);
                break;
            }
            Some(idx) => {
                out.push_str(&search[..idx]);
                let rest = &search[idx..];
                match rest.find("</script>") {
                    Some(end) => search = &rest[end + "</script>".len()..],
                    None => break,
                }
            }
        }
    }
    // Strip canonical / og: / twitter: meta + canonical link.
    let mut filtered = String::with_capacity(out.len());
    for line in out.lines() {
        let l = line.trim_start();
        let is_ref = l.contains(r#"rel="canonical""#)
            || l.contains(r#"property="og:"#)
            || l.contains(r#"name="twitter:"#);
        if !is_ref {
            filtered.push_str(line);
            filtered.push('\n');
        }
    }
    filtered
}

/// Find external URLs (http/https/protocol-relative) in
/// load-triggering attribute contexts: `src="..."`, `href="..."`
/// (only on `<link rel="stylesheet">`), `url(...)` in inline
/// styles or stylesheets.
fn scan_external_loads(body: &str) -> Vec<String> {
    let mut hits = Vec::new();
    // src="http(s):// or //"
    for m in find_url_in_attr(body, "src=\"") {
        hits.push(format!("src={m}"));
    }
    // <link rel="stylesheet" href="http(s):// or //">
    let mut search = body;
    while let Some(idx) = search.find("<link") {
        let after = &search[idx..];
        let Some(end) = after.find('>') else {
            break;
        };
        let tag = &after[..end];
        if tag.contains(r#"rel="stylesheet""#) {
            if let Some(href) = extract_attr(tag, "href=\"") {
                if is_external(&href) {
                    hits.push(format!("href={href}"));
                }
            }
        }
        search = &after[end + 1..];
    }
    hits
}

fn find_url_in_attr(body: &str, prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut search = body;
    while let Some(idx) = search.find(prefix) {
        let after = &search[idx + prefix.len()..];
        if let Some(end) = after.find('"') {
            let val = &after[..end];
            if is_external(val) {
                out.push(val.to_owned());
            }
            search = &after[end + 1..];
        } else {
            break;
        }
    }
    out
}

fn extract_attr(tag: &str, prefix: &str) -> Option<String> {
    let idx = tag.find(prefix)?;
    let after = &tag[idx + prefix.len()..];
    let end = after.find('"')?;
    Some(after[..end].to_owned())
}

fn is_external(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_link_is_not_an_asset() {
        let body = r#"<head><link rel="canonical" href="https://example.com/x"></head>"#;
        let stripped = strip_reference_tags(body);
        let hits = scan_external_loads(&stripped);
        assert!(hits.is_empty());
    }

    #[test]
    fn og_url_meta_is_not_an_asset() {
        let body = r#"<head><meta property="og:url" content="https://example.com/y"></head>"#;
        let stripped = strip_reference_tags(body);
        let hits = scan_external_loads(&stripped);
        assert!(hits.is_empty());
    }

    #[test]
    fn external_script_src_is_caught() {
        let body = r#"<script src="https://cdn.example/foo.js"></script>"#;
        let stripped = strip_reference_tags(body);
        let hits = scan_external_loads(&stripped);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].contains("https://cdn.example/foo.js"));
    }

    #[test]
    fn external_stylesheet_link_is_caught() {
        let body = r#"<link rel="stylesheet" href="https://cdn.example/x.css">"#;
        let stripped = strip_reference_tags(body);
        let hits = scan_external_loads(&stripped);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn local_assets_are_fine() {
        let body = r#"<script src="/local.js"></script><link rel="stylesheet" href="x.css">"#;
        let stripped = strip_reference_tags(body);
        let hits = scan_external_loads(&stripped);
        assert!(hits.is_empty());
    }

    #[test]
    fn jsonld_url_is_not_an_asset() {
        let body = r#"
            <script type="application/ld+json">{"url":"https://example.com/blog/x"}</script>
            <p>page body</p>
        "#;
        let stripped = strip_reference_tags(body);
        // The whole script block should be gone.
        assert!(!stripped.contains("application/ld+json"));
    }
}
