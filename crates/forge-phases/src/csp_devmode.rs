//! `csp_devmode` — strict-fail any HTML whose meta CSP carries
//! `upgrade-insecure-requests`. That directive rewrites every
//! `http://` subresource URL to `https://` in the browser; on
//! a HTTP dev server it kills every CSS/JS load (page renders
//! unstyled).
//!
//! Production should set CSP via an HTTP header behind TLS, never
//! via the meta tag that ships with the static HTML.
//!
//! Bash parity: `phase_csp_devmode`. False-positive guard: only
//! looks INSIDE the `content="..."` value of the CSP meta tag, not
//! at HTML comments that mention the keyword.

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `csp_devmode` phase.
#[derive(Debug, Default)]
pub struct CspDevmodePhase;

impl Phase for CspDevmodePhase {
    fn name(&self) -> &'static str {
        "csp_devmode"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let files = walk_html(&ctx.static_dir, self.name())?;
        let mut findings = Vec::new();

        for file in files {
            if csp_meta_has_upgrade(&file.body) {
                findings.push(Finding::strict(
                    self.name(),
                    file.name.clone(),
                    "meta CSP includes upgrade-insecure-requests — kills CSS/JS on HTTP dev server. Move CSP to an HTTP header in production.",
                ));
            }
        }

        Ok(findings)
    }
}

/// True if any `<meta http-equiv="Content-Security-Policy"
/// content="...">` value contains `upgrade-insecure-requests`.
fn csp_meta_has_upgrade(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    let mut search = lower.as_str();
    while let Some(idx) = search.find("<meta") {
        let after = &search[idx..];
        let Some(end) = after.find('>') else {
            break;
        };
        let tag = &after[..end];
        if tag.contains(r#"http-equiv="content-security-policy""#) {
            // Find content="..." value (lower-cased view).
            if let Some(c_idx) = tag.find("content=\"") {
                let value_start = c_idx + "content=\"".len();
                let value_rest = &tag[value_start..];
                if let Some(value_end) = value_rest.find('"') {
                    let value = &value_rest[..value_end];
                    if value.contains("upgrade-insecure-requests") {
                        return true;
                    }
                }
            }
        }
        search = &after[end + 1..];
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_upgrade_in_content() {
        let body = r#"<meta http-equiv="Content-Security-Policy" content="default-src 'self'; upgrade-insecure-requests">"#;
        assert!(csp_meta_has_upgrade(body));
    }

    #[test]
    fn does_not_flag_comment_mentioning_upgrade() {
        let body = "<!-- REMOVED upgrade-insecure-requests because it broke dev -->";
        assert!(!csp_meta_has_upgrade(body));
    }

    #[test]
    fn does_not_flag_directive_outside_csp_meta() {
        let body = r#"<meta name="something" content="upgrade-insecure-requests">"#;
        assert!(!csp_meta_has_upgrade(body));
    }

    #[test]
    fn safe_csp_without_upgrade() {
        let body = r#"<meta http-equiv="Content-Security-Policy" content="default-src 'self'; script-src 'self'">"#;
        assert!(!csp_meta_has_upgrade(body));
    }
}
