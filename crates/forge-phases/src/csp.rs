//! `csp` — every shipped HTML page must carry a strict
//! Content-Security-Policy and X-Content-Type-Options, the policy must
//! actually cover the page's own inline styles and scripts, and it must not
//! carry directives the browser ignores in a `<meta>` element.
//!
//! ## Why the inline-coverage check exists
//!
//! Forge post-processes the HTML that Loom renders — it injects a stylesheet
//! link, an SRI attribute, and once injected a gradient `<style>` block. Loom
//! computes the CSP hashes at render time, so anything Forge added afterwards
//! was outside the policy it had already written. That is exactly what
//! happened: an injected `<style>` block was CSP-blocked on every page of every
//! Forge-built site, and nothing failed, because no phase compared the policy
//! against the document it describes.
//!
//! A CSP is a claim about a document. This phase verifies the claim instead of
//! trusting the pipeline that produced it, so any future post-render injection
//! fails the build rather than the browser.

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// Directives a user agent MUST ignore when the policy is delivered in a
/// `<meta http-equiv>` element rather than a response header (CSP Level 3,
/// "Delivery"). Shipping one here is worse than omitting it: it reads as
/// protection in review, provides none, and logs a console error on every page.
const META_INERT_DIRECTIVES: [&str; 4] =
    ["frame-ancestors", "report-uri", "report-to", "sandbox"];

/// Base64 sha256 in the form CSP expects (`sha256-…`).
fn csp_sha256(bytes: &[u8]) -> String {
    use base64::Engine as _;
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    format!(
        "sha256-{}",
        base64::engine::general_purpose::STANDARD.encode(hasher.finalize())
    )
}

/// The `content="…"` of the CSP `<meta>`, if present.
fn meta_csp(body: &str) -> Option<&str> {
    let at = body.find(r#"http-equiv="Content-Security-Policy""#)?;
    let rest = &body[at..];
    let end_tag = rest.find('>')?;
    let tag = &rest[..end_tag];
    let c = tag.find(r#"content=""#)? + r#"content=""#.len();
    let close = tag[c..].find('"')?;
    Some(&tag[c..c + close])
}

/// The token list of one CSP directive, e.g. `style-src` → `'self' 'sha256-…'`.
fn directive<'a>(csp: &'a str, name: &str) -> Option<&'a str> {
    csp.split(';').map(str::trim).find_map(|d| {
        let rest = d.strip_prefix(name)?;
        // Guard against `script-src` matching `script-src-elem`.
        match rest.chars().next() {
            None => Some(""),
            Some(c) if c.is_whitespace() => Some(rest.trim()),
            Some(_) => None,
        }
    })
}

/// Inline `<style>` / `<script>` bodies that the CSP is required to cover.
///
/// Skips elements with a `src` (governed by the URL allow-list, not a hash) and
/// scripts whose `type` is a data block such as `application/ld+json` — those
/// are never executed, so no user agent applies `script-src` to them.
fn inline_blocks<'a>(body: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while let Some(rel) = body[cursor..].find(&open) {
        let start = cursor + rel;
        let Some(gt) = body[start..].find('>') else { break };
        let attrs = &body[start + open.len()..start + gt];
        let content_start = start + gt + 1;
        let Some(end_rel) = body[content_start..].find(&close) else { break };
        let content = &body[content_start..content_start + end_rel];
        cursor = content_start + end_rel + close.len();

        if attrs.contains("src=") {
            continue;
        }
        if tag == "script" && !is_executable_script(attrs) {
            continue;
        }
        out.push(content);
    }
    out
}

/// A `<script>` runs only when its `type` is absent, empty, a JavaScript MIME
/// type, or `module`. Anything else is an inert data block.
fn is_executable_script(attrs: &str) -> bool {
    let Some(at) = attrs.find("type=\"") else {
        return true;
    };
    let rest = &attrs[at + "type=\"".len()..];
    let Some(end) = rest.find('"') else {
        return true;
    };
    let ty = rest[..end].trim().to_ascii_lowercase();
    ty.is_empty()
        || ty == "module"
        || ty == "text/javascript"
        || ty == "application/javascript"
        || ty == "text/ecmascript"
        || ty == "application/ecmascript"
}

/// `csp` phase implementation.
#[derive(Debug, Default)]
pub struct CspPhase;

impl Phase for CspPhase {
    fn name(&self) -> &'static str {
        "csp"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let files = walk_html(&ctx.static_dir, self.name())?;
        let mut findings = Vec::new();

        for file in files {
            let body = file.body.as_str();

            if !body.contains(r#"http-equiv="Content-Security-Policy""#) {
                findings.push(
                    Finding::strict(
                        self.name(),
                        file.name.clone(),
                        "missing Content-Security-Policy meta",
                    )
                    .citing(["sec-005"])
                    .why("rendered page ships no CSP — XSS payloads from any origin can execute against the page")
                    .fix("the page shell template should emit a `<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'self'; frame-ancestors 'none'; ...\">` — fix in the Loom page-shell primitive that emitted this HTML, not in static/")
                    .skill("add-loom-primitive")
                    .avoid("don't hand-add a <meta> tag to the rendered HTML — it's a build artifact"),
                );
            } else if !body.contains("default-src 'self'") {
                findings.push(
                    Finding::strict(
                        self.name(),
                        file.name.clone(),
                        "CSP missing default-src 'self'",
                    )
                    .citing(["sec-005"])
                    .why("CSP without default-src 'self' leaves the fall-through wide open for any directive not explicitly set")
                    .fix("update the page-shell CSP emission in PlausiDen-Loom to start every CSP with `default-src 'self'; ...`")
                    .skill("add-loom-primitive"),
                );
            }

            if !contains_xcontenttype_nosniff(body) {
                findings.push(
                    Finding::strict(
                        self.name(),
                        file.name.clone(),
                        "missing X-Content-Type-Options nosniff",
                    )
                    .citing(["sec-005"])
                    .why("without nosniff, browsers may MIME-sniff a response and interpret data as HTML/JS; an attacker-controlled upload can execute in the browser")
                    .fix("emit `<meta http-equiv=\"X-Content-Type-Options\" content=\"nosniff\">` in the Loom page-shell template")
                    .skill("add-loom-primitive"),
                );
            }

            let Some(csp) = meta_csp(body) else { continue };

            for inert in META_INERT_DIRECTIVES {
                if directive(csp, inert).is_some() {
                    findings.push(
                        Finding::strict(
                            self.name(),
                            file.name.clone(),
                            format!("meta CSP carries `{inert}`, which browsers ignore there"),
                        )
                        .citing(["sec-005"])
                        .why("CSP Level 3 requires user agents to ignore frame-ancestors, report-uri, report-to and sandbox when the policy arrives in a <meta> element — the directive provides no protection, reads as protection in review, and logs a console error on every page")
                        .fix("remove it from the meta CSP in the Loom page-shell template; framing protection is a response-header concern — serve the headers named in the generated deploy/headers.caddy")
                        .skill("add-loom-primitive"),
                    );
                }
            }

            // The policy must cover the document it ships with. Anything
            // injected after the hashes were computed lands here.
            for (tag, dir_name) in [("style", "style-src"), ("script", "script-src")] {
                let Some(tokens) = directive(csp, dir_name) else { continue };
                if tokens.contains("'unsafe-inline'") {
                    continue;
                }
                for content in inline_blocks(body, tag) {
                    let hash = csp_sha256(content.as_bytes());
                    if tokens.contains(&hash) {
                        continue;
                    }
                    let preview: String = content.chars().take(60).collect();
                    findings.push(
                        Finding::strict(
                            self.name(),
                            file.name.clone(),
                            format!("inline <{tag}> is not covered by {dir_name} ({hash})"),
                        )
                        .citing(["sec-005"])
                        .why(format!("the browser will refuse to apply this block — it ships in the page but never takes effect, silently, on every visit. Starts: {preview:?}"))
                        .fix(format!("either add {hash} to {dir_name} where the policy is built, or stop injecting the block after the CSP has been computed — a post-render injector cannot amend a policy that was already written"))
                        .skill("add-loom-primitive"),
                    );
                }
            }
        }

        Ok(findings)
    }
}

/// Mirror bash regex `X-Content-Type-Options.*nosniff`. Substring
/// checks suffice because the directive is space-separated and the
/// only legitimate value is `nosniff`.
fn contains_xcontenttype_nosniff(body: &str) -> bool {
    if let Some(idx) = body.find("X-Content-Type-Options") {
        body[idx..].contains("nosniff")
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nosniff_helper_finds_separated() {
        let body = r#"<meta http-equiv="X-Content-Type-Options" content="nosniff">"#;
        assert!(contains_xcontenttype_nosniff(body));
    }

    #[test]
    fn nosniff_helper_misses_when_absent() {
        let body = "<title>page</title>";
        assert!(!contains_xcontenttype_nosniff(body));
    }

    #[test]
    fn nosniff_helper_misses_directive_without_value() {
        let body = r#"<meta http-equiv="X-Content-Type-Options" content="">"#;
        assert!(!contains_xcontenttype_nosniff(body));
    }

    #[test]
    fn csp_sha256_matches_an_independently_computed_digest() {
        // A hash this phase computes itself proves nothing against itself, so
        // the expected value comes from outside the crate. Reproduce with:
        //   printf 'body{color:red}' | openssl dgst -sha256 -binary | openssl base64
        // (cross-checked against Python's hashlib; both agree).
        assert_eq!(
            csp_sha256(b"body{color:red}"),
            "sha256-FcQqt3aNlV7AZnGV4zkQRVeCeJOxbMPnQSx258L803E="
        );
    }

    #[test]
    fn meta_csp_extracts_the_policy() {
        let body = r#"<meta http-equiv="Content-Security-Policy" content="default-src 'self'; style-src 'self'">"#;
        assert_eq!(meta_csp(body), Some("default-src 'self'; style-src 'self'"));
        assert_eq!(meta_csp("<title>no csp</title>"), None);
    }

    #[test]
    fn directive_does_not_match_a_longer_directive_name() {
        let csp = "default-src 'self'; script-src-elem 'self' 'sha256-x'";
        assert_eq!(directive(csp, "script-src"), None, "must not match script-src-elem");
        assert_eq!(directive(csp, "script-src-elem"), Some("'self' 'sha256-x'"));
    }

    #[test]
    fn inline_blocks_skips_external_and_data_blocks() {
        let body = r#"
            <style>a{}</style>
            <style data-x data-y="z">b{}</style>
            <script src="/app.js"></script>
            <script type="application/ld+json">{"@type":"X"}</script>
            <script>real()</script>
        "#;
        assert_eq!(inline_blocks(body, "style"), vec!["a{}", "b{}"]);
        assert_eq!(
            inline_blocks(body, "script"),
            vec!["real()"],
            "external scripts use the URL allow-list; ld+json is never executed"
        );
    }

    /// Reproduces the shipped defect: a `<style>` block injected after the CSP
    /// was computed, so its hash is absent and the browser drops it.
    #[test]
    fn uncovered_inline_style_is_reported() {
        let covered = csp_sha256(b"a{}");
        let body = format!(
            r#"<meta http-equiv="Content-Security-Policy" content="default-src 'self'; style-src 'self' '{covered}'">
               <meta http-equiv="X-Content-Type-Options" content="nosniff">
               <style>a{{}}</style>
               <style data-loom-default-gradient>:root{{--x:1}}</style>"#
        );
        let findings = check_body(&body);
        let uncovered: Vec<_> = findings
            .iter()
            .filter(|m| m.contains("not covered by style-src"))
            .collect();
        assert_eq!(uncovered.len(), 1, "only the injected block is uncovered: {findings:?}");
        assert!(uncovered[0].contains(&csp_sha256(b":root{--x:1}")));
    }

    #[test]
    fn fully_covered_page_is_clean() {
        let h1 = csp_sha256(b"a{}");
        let h2 = csp_sha256(b"run()");
        let body = format!(
            r#"<meta http-equiv="Content-Security-Policy" content="default-src 'self'; style-src 'self' '{h1}'; script-src 'self' '{h2}'">
               <meta http-equiv="X-Content-Type-Options" content="nosniff">
               <style>a{{}}</style><script>run()</script>"#
        );
        assert!(check_body(&body).is_empty(), "{:?}", check_body(&body));
    }

    #[test]
    fn unsafe_inline_waives_the_hash_requirement() {
        let body = r#"<meta http-equiv="Content-Security-Policy" content="default-src 'self'; style-src 'self' 'unsafe-inline'">
                      <meta http-equiv="X-Content-Type-Options" content="nosniff">
                      <style>anything{}</style>"#;
        assert!(check_body(body).iter().all(|m| !m.contains("not covered")));
    }

    #[test]
    fn meta_inert_directives_are_reported() {
        let body = r#"<meta http-equiv="Content-Security-Policy" content="default-src 'self'; frame-ancestors 'none'; report-to default">
                      <meta http-equiv="X-Content-Type-Options" content="nosniff">"#;
        let findings = check_body(body);
        assert!(findings.iter().any(|m| m.contains("`frame-ancestors`")), "{findings:?}");
        assert!(findings.iter().any(|m| m.contains("`report-to`")), "{findings:?}");
    }

    /// Drives the phase over one in-memory document and returns its messages.
    fn check_body(body: &str) -> Vec<String> {
        let dir = std::env::temp_dir().join(format!(
            "forge-csp-{}-{:p}",
            std::process::id(),
            body as *const str
        ));
        let static_dir = dir.join("static");
        std::fs::create_dir_all(&static_dir).unwrap();
        std::fs::write(static_dir.join("index.html"), body).unwrap();
        let ctx = BuildCtx {
            root: dir.clone(),
            static_dir,
            mode: forge_core::BuildMode::Poc,
        };
        let out = CspPhase
            .run(&ctx)
            .unwrap()
            .into_iter()
            .map(|f| f.message)
            .collect();
        let _ = std::fs::remove_dir_all(&dir);
        out
    }
}
