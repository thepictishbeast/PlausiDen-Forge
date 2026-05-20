//! `csp` — every shipped HTML page must carry strict
//! Content-Security-Policy + X-Content-Type-Options + frame-
//! ancestors directives.
//!
//! Bash parity: `phase_csp` in forge.sh — same four checks per
//! page (CSP meta, default-src, nosniff, frame-ancestors).

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

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

            if !body.contains("frame-ancestors 'none'") {
                findings.push(
                    Finding::strict(
                        self.name(),
                        file.name.clone(),
                        "CSP missing frame-ancestors 'none' (clickjacking)",
                    )
                    .citing(["sec-005"])
                    .why("without frame-ancestors 'none', the page can be iframed by hostile origins for clickjacking attacks")
                    .fix("add `frame-ancestors 'none'` to the CSP directive set in the Loom page-shell template")
                    .skill("add-loom-primitive"),
                );
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
}
