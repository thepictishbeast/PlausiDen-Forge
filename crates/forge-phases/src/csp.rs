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
                findings.push(Finding::strict(
                    self.name(),
                    file.name.clone(),
                    "missing Content-Security-Policy meta",
                ));
            } else if !body.contains("default-src 'self'") {
                findings.push(Finding::strict(
                    self.name(),
                    file.name.clone(),
                    "CSP missing default-src 'self'",
                ));
            }

            if !contains_xcontenttype_nosniff(body) {
                findings.push(Finding::strict(
                    self.name(),
                    file.name.clone(),
                    "missing X-Content-Type-Options nosniff",
                ));
            }

            if !body.contains("frame-ancestors 'none'") {
                findings.push(Finding::strict(
                    self.name(),
                    file.name.clone(),
                    "CSP missing frame-ancestors 'none' (clickjacking)",
                ));
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
