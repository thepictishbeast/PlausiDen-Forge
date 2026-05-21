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
use std::path::Path;

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
        let suppress = forge_toml_suppress_unbuilt_route(&ctx.root);
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
                    let msg = format!("href=\"{href}\" → static/{target} does not exist");
                    let f = if suppress {
                        Finding::warn(self.name(), file.name.clone(), msg)
                    } else {
                        Finding::strict(self.name(), file.name.clone(), msg)
                    };
                    findings.push(
                        f.why(
                            "every internal href in shipped HTML must resolve to a file in \
                             static/. an unresolved href ships to readers as a 404 — silent in \
                             the build, loud in the browser",
                        )
                        .fix(format!(
                            "either: (a) add cms/{}.json so render writes static/{target}, OR \
                             (b) remove the dead link from the rendering source (CMS body / \
                             Loom primitive props)",
                            target.trim_end_matches(".html")
                        ))
                        .skill("author-cms-content")
                        .avoid(
                            "don't manually `touch static/<route>.html` to make the gate pass \
                             — that file is regenerated only from cms/*.json on write_canonical \
                             builds and will be flagged as an orphan",
                        ),
                    );
                }
            }
        }

        Ok(findings)
    }
}

/// Read `[poc] suppress_unbuilt_route = true` from `<root>/forge.toml`.
/// Returns false for any non-true value (missing file, parse error,
/// key absent, anything other than the literal boolean `true`).
///
/// Mirrors the documented poc-mode semantics in forge.toml: in poc
/// mode, unbuilt routes are warnings (surface in the in-page errors
/// overlay) rather than fatal build failures. Production mode flips
/// the suppress flag off and these become strict blockers.
fn forge_toml_suppress_unbuilt_route(root: &Path) -> bool {
    let path = root.join("forge.toml");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return false;
    };
    let Ok(parsed) = content.parse::<toml::Value>() else {
        return false;
    };
    parsed
        .get("poc")
        .and_then(|p| p.get("suppress_unbuilt_route"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
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
            if !has_url_scheme(val) && !val.starts_with('#') && !val.starts_with("//") {
                out.push(val.to_owned());
            }
            search = &after[end + 1..];
        } else {
            break;
        }
    }
    out
}

/// True for any href that begins with a URL scheme per RFC 3986 §3.1
/// (`ALPHA *( ALPHA / DIGIT / "+" / "-" / "." ) ":"`). Catches every
/// schemed URL — `http:`, `https:`, `mailto:`, `tel:`, `data:`,
/// `javascript:`, `blob:`, `file:`, `ws:`, `wss:`, `chrome:`,
/// `view-source:`, `about:`, `ssh:`, custom schemes — without
/// maintaining an explicit allowlist. Internal-route validation
/// applies only to relative paths; everything with a scheme is
/// considered external + skipped.
///
/// BUG ASSUMPTION: an href like `path:foo` (relative path that
/// happens to contain a colon) would be misclassified as schemed.
/// In practice URLs of that shape don't appear in substrate-built
/// rendered HTML — content paths never contain bare colons. If a
/// real case surfaces, switch to checking that the colon appears
/// before the first `/` (which IS how RFC 3986 actually disambiguates).
fn has_url_scheme(href: &str) -> bool {
    let mut chars = href.chars();
    let first = match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => c,
        _ => return false,
    };
    let _ = first;
    for c in chars {
        if c == ':' {
            return true;
        }
        if !(c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
            return false;
        }
    }
    false
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
    fn has_url_scheme_recognizes_known_schemes() {
        // Every scheme that should be SKIPPED — they're URLs to external
        // resources, not relative routes for filesystem lookup.
        for href in [
            "http://example.com/",
            "https://example.com/",
            "mailto:hi@example.com",
            "tel:+15551234567",
            "data:image/svg+xml,%3Csvg%3E%3C/svg%3E",
            "data:text/plain;base64,SGVsbG8=",
            "javascript:void(0)",
            "blob:https://example.com/abc-123",
            "file:///etc/hosts",
            "about:blank",
            "view-source:https://example.com/",
            "chrome://flags",
            "ws://localhost:8080/",
            "wss://example.com/socket",
            "ssh://user@host",
            "ftp://files.example.com/",
        ] {
            assert!(has_url_scheme(href), "should detect scheme on: {href}");
        }
    }

    #[test]
    fn has_url_scheme_rejects_relative_paths() {
        for href in [
            "/",
            "/page.html",
            "/page.html#frag",
            "./contact.html",
            "../parent.html",
            "contact.html",
            "#fragment-only",
        ] {
            assert!(!has_url_scheme(href), "should NOT detect scheme on: {href}");
        }
    }

    #[test]
    fn extract_skips_data_uris() {
        // Regression: pre-fix, data:image/svg+xml,... was treated as a
        // relative route and reported as "static/data:image/...
        // does not exist", producing dozens of false-positive blockers
        // on any page with inline SVG references.
        let body = r##"
            <link rel="icon" href="data:image/svg+xml,%3Csvg%3E%3C/svg%3E">
            <a href="/real-route">y</a>
            <img src="data:image/png;base64,iVBOR..." href="data:image/png;base64,iVBOR...">
        "##;
        let hrefs = extract_internal_hrefs(body);
        assert_eq!(hrefs, vec!["/real-route".to_owned()]);
    }

    #[test]
    fn extract_skips_javascript_uris() {
        let body = r##"
            <a href="javascript:void(0)">noop</a>
            <a href="/real">real</a>
        "##;
        let hrefs = extract_internal_hrefs(body);
        assert_eq!(hrefs, vec!["/real".to_owned()]);
    }

    #[test]
    fn extract_skips_protocol_relative() {
        // `//cdn.example.com/x.js` is protocol-relative; treat as
        // external (not in our static/ tree).
        let body = r##"
            <a href="//cdn.example.com/script.js">x</a>
            <a href="/internal">i</a>
        "##;
        let hrefs = extract_internal_hrefs(body);
        assert_eq!(hrefs, vec!["/internal".to_owned()]);
    }

    #[test]
    fn suppress_flag_reads_true() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("forge.toml"),
            "[forge]\nmode = \"poc\"\n\n[poc]\nsuppress_unbuilt_route = true\n",
        )
        .unwrap();
        assert!(forge_toml_suppress_unbuilt_route(dir.path()));
    }

    #[test]
    fn suppress_flag_reads_false_when_explicit_false() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("forge.toml"),
            "[poc]\nsuppress_unbuilt_route = false\n",
        )
        .unwrap();
        assert!(!forge_toml_suppress_unbuilt_route(dir.path()));
    }

    #[test]
    fn suppress_flag_defaults_false_when_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        // no forge.toml at all
        assert!(!forge_toml_suppress_unbuilt_route(dir.path()));
        // forge.toml exists but no [poc] section
        std::fs::write(dir.path().join("forge.toml"), "[forge]\nmode = \"poc\"\n").unwrap();
        assert!(!forge_toml_suppress_unbuilt_route(dir.path()));
    }

    #[test]
    fn unbuilt_route_findings_carry_advocacy() {
        use std::fs;
        let tmp =
            std::env::temp_dir().join(format!("forge-unbuilt-advocacy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("static")).unwrap();
        // Page with one dead internal href.
        fs::write(
            tmp.join("static/index.html"),
            r#"<!doctype html><html><body><a href="/learn/">learn</a></body></html>"#,
        )
        .unwrap();
        let ctx = BuildCtx {
            root: tmp.clone(),
            static_dir: tmp.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = UnbuiltRoutePhase.run(&ctx).expect("run");
        assert_eq!(findings.len(), 1, "expected one unbuilt-route finding");
        let adv = &findings[0].advocacy;
        assert!(!adv.why.is_empty(), "must carry .why()");
        assert!(
            adv.substrate_fix.contains("cms/")
                || adv.substrate_fix.contains("remove the dead link"),
            ".fix() must name the substrate-correct action: {:?}",
            adv.substrate_fix
        );
        assert_eq!(adv.skill.as_deref(), Some("author-cms-content"));
        assert!(adv.anti_pattern.is_some(), "must carry .avoid()");
        let _ = fs::remove_dir_all(&tmp);
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
