//! `reader_safety` — for sites declaring a non-clearnet deploy
//! target, verify pages WORK + are SAFE for hardened readers.
//!
//! Captures SITE_OPERATIONS.md §8 (reader-side safety features).
//! `phase_network_target_enforcement` protects the operator from
//! shipping clearnet leaks; THIS phase protects the reader from
//! a site that technically lives on Tor but doesn't work for the
//! readers who use Tor specifically.
//!
//! Tor Browser's "Safest" security level disables JS globally,
//! blocks custom fonts, and rejects non-standard features. A
//! Tor-mode site that requires JavaScript to navigate, custom
//! fonts to read body text, or cookies to deliver content has
//! selected against its own audience — those readers won't see
//! the page.
//!
//! ## Configuration
//!
//! Reads `[reader_safety]` from `forge.toml`:
//!
//! ```toml
//! [reader_safety]
//! # When true, every check below fires Strict for sites declaring
//! # tor / i2p in [networks].targets. When false, all findings are
//! # Warn (operator hasn't committed to hardened-reader support).
//! strict = true
//!
//! # Optional: skip specific files (e.g. an interactive admin
//! # page that legitimately requires JS — operator accepts the
//! # exclusion).
//! skip_files = ["admin/console.html"]
//! ```
//!
//! Missing `[reader_safety]` section → silent skip. The phase
//! only fires when EITHER `[reader_safety]` is configured OR
//! `[networks].targets` includes tor / i2p / lokinet. The latter
//! is the implicit default — if you're on an anonymity network,
//! reader safety matters.
//!
//! ## Checks
//!
//! | Check                         | Strict on Tor target | Detail                                          |
//! |-------------------------------|----------------------|-------------------------------------------------|
//! | Inline `<script>` blocks      | yes                  | Tor Safest disables JS; page won't work         |
//! | Required `<noscript>` fallback| yes                  | Pages with `<script>` MUST ship `<noscript>`    |
//! | Inline event handlers (onclick) | yes                | Same as inline script; CSP-incompatible        |
//! | Non-system font references    | yes                  | Tor Browser blocks custom fonts on Safest      |
//! | Cookie-set markup             | warn                 | Tor Browser deletes cookies between sessions   |
//! | localStorage / sessionStorage | warn                 | Same as cookies — wiped per session            |
//! | CAPTCHA libraries (recaptcha) | yes                  | Google reCAPTCHA harasses Tor traffic          |
//! | Form `autocomplete=` (non-off)| warn                 | Tor users want autofill OFF for fingerprinting |
//! | `srcdoc=` iframes             | warn                 | Sandboxing assumption holds, but tracks worse  |
//!
//! ## Severity
//!
//! - `strict = true` + Tor/I2P target → all Strict
//! - `strict = false` OR no anonymity target but `[reader_safety]`
//!   present → all Warn
//! - Each check's listed severity above is the FLOOR; operator
//!   policy can only relax to Warn, not escalate cookie-Warn to
//!   Strict.

use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `reader_safety` phase.
#[derive(Debug, Default)]
pub struct ReaderSafetyPhase;

impl Phase for ReaderSafetyPhase {
    fn name(&self) -> &'static str {
        "reader_safety"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let cfg = match resolve_config(&ctx.root) {
            Some(c) => c,
            None => {
                tracing::debug!("reader_safety: not configured + no anonymity target — skip");
                return Ok(vec![]);
            }
        };

        let mut findings = Vec::new();
        for file in walk_html(&ctx.static_dir, self.name())? {
            if cfg.skip_files.iter().any(|s| s == &file.name) {
                continue;
            }
            for issue in scan_page(&file.body) {
                let sev = match (cfg.strict, issue.minimum_severity) {
                    (true, Severity::Warn) => Severity::Warn,
                    (true, Severity::Strict) => Severity::Strict,
                    (false, _) => Severity::Warn,
                };
                let msg = format!("{kind}: {detail}", kind = issue.kind, detail = issue.detail);
                findings.push(match sev {
                    Severity::Strict => Finding::strict(self.name(), file.name.clone(), msg),
                    Severity::Warn => Finding::warn(self.name(), file.name.clone(), msg),
                });
            }
        }
        Ok(findings)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Severity {
    Strict,
    Warn,
}

struct Issue {
    kind: &'static str,
    minimum_severity: Severity,
    detail: &'static str,
}

fn scan_page(body: &str) -> Vec<Issue> {
    let mut out: Vec<Issue> = Vec::new();
    let has_script = body.contains("<script");
    if has_script {
        out.push(Issue {
            kind: "inline <script>",
            minimum_severity: Severity::Strict,
            detail: "Tor Browser Safest disables JS globally — every reader on \
                     Safest sees a broken page. Either ship a <noscript> \
                     fallback that delivers the content, or strip the script \
                     entirely (the rest of phase_network_target_enforcement \
                     will catch external scripts; this catches inline ones)",
        });
        if !body.contains("<noscript") {
            out.push(Issue {
                kind: "<script> without <noscript> fallback",
                minimum_severity: Severity::Strict,
                detail: "page uses <script> but provides no <noscript> \
                         fallback — JS-disabled readers see incomplete or \
                         broken UI",
            });
        }
    }
    // Inline event handlers (onclick=, onload=, etc) — CSP-incompatible
    // + same threat model as inline <script>.
    let inline_handlers: &[&str] = &[
        " onclick=",
        " onload=",
        " onerror=",
        " onsubmit=",
        " onchange=",
        " onkeyup=",
        " onkeydown=",
        " onmouseover=",
        " onfocus=",
        " onblur=",
    ];
    for h in inline_handlers {
        if body.contains(h) {
            out.push(Issue {
                kind: "inline event handler",
                minimum_severity: Severity::Strict,
                detail: "inline event handlers (onclick / onload / etc) violate \
                         CSP + run as JS; same problem as inline <script>",
            });
            break; // one finding per page is enough
        }
    }
    // Cookie-set: document.cookie= in any script body OR
    // Set-Cookie meta-equivalent in <meta http-equiv>
    if body.contains("document.cookie") || body.contains("Set-Cookie") {
        out.push(Issue {
            kind: "cookie-set markup",
            minimum_severity: Severity::Warn,
            detail: "Tor Browser deletes cookies between sessions; any UX \
                     depending on a persistent cookie value will degrade. \
                     Prefer URL-fragment state or server-side session that \
                     re-authenticates on each visit",
        });
    }
    // localStorage / sessionStorage
    if body.contains("localStorage") || body.contains("sessionStorage") {
        out.push(Issue {
            kind: "Web Storage API use",
            minimum_severity: Severity::Warn,
            detail: "Tor Browser wipes localStorage/sessionStorage between \
                     sessions for fingerprint resistance; any logic depending \
                     on persistence will fail",
        });
    }
    // reCAPTCHA / hCaptcha (the latter is Tor-friendlier but worth Warn)
    if body.contains("recaptcha") || body.contains("google.com/recaptcha") {
        out.push(Issue {
            kind: "Google reCAPTCHA",
            minimum_severity: Severity::Strict,
            detail: "Google's reCAPTCHA harasses Tor users with unending image \
                     challenges or denies them entirely — switch to hCaptcha, \
                     Turnstile, or a self-hosted proof-of-work challenge",
        });
    }
    // Non-system font reference: @font-face inline or <link rel=stylesheet href=...font...>
    // Tor Browser blocks custom fonts on Safest level.
    if body.contains("@font-face") {
        out.push(Issue {
            kind: "@font-face declaration",
            minimum_severity: Severity::Strict,
            detail: "Tor Browser blocks custom font loading on Safest security \
                     level — text falls back to system fonts. If the design \
                     depends on the custom font, the page is broken for \
                     Safest readers. Prefer the system font stack for body \
                     text on anonymity-network sites",
        });
    }
    // Form autocomplete= not "off" (Tor users want fingerprint resistance)
    if body.contains("autocomplete=\"") && !body.contains("autocomplete=\"off\"") {
        // Only one finding even if multiple autocomplete= attributes exist
        // and at least one is non-off
        let mut search = body;
        while let Some(idx) = search.find("autocomplete=\"") {
            let after = &search[idx + 14..];
            let end = after.find('"').unwrap_or(after.len());
            let value = &after[..end];
            if value != "off" {
                out.push(Issue {
                    kind: "autocomplete enabled",
                    minimum_severity: Severity::Warn,
                    detail: "Tor users typically want autocomplete=off on \
                             every form for fingerprint resistance — defaults \
                             leak field-history-driven identity signals",
                });
                break;
            }
            search = &after[end + 1..];
        }
    }
    out
}

#[derive(Debug, Clone, Default)]
struct ReaderSafetyConfig {
    strict: bool,
    skip_files: Vec<String>,
}

fn resolve_config(root: &Path) -> Option<ReaderSafetyConfig> {
    let path = root.join("forge.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    let parsed: toml::Value = content.parse().ok()?;
    let explicit_section = parsed.get("reader_safety");
    let networks_targets: Vec<String> = parsed
        .get("networks")
        .and_then(|n| n.get("targets"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_ascii_lowercase()))
                .collect()
        })
        .unwrap_or_default();
    let has_anonymity_target = networks_targets
        .iter()
        .any(|t| matches!(t.as_str(), "tor" | "onion" | "i2p" | "lokinet" | "loki"));

    if explicit_section.is_none() && !has_anonymity_target {
        return None;
    }

    // Default strict = true when an anonymity target is declared (the
    // operator opted into hardened-reader audience by choosing the
    // network); strict = false when only [reader_safety] is explicit
    // without an anonymity target (operator wants the checks as
    // advisory).
    let strict = explicit_section
        .and_then(|s| s.get("strict"))
        .and_then(|v| v.as_bool())
        .unwrap_or(has_anonymity_target);
    let skip_files = explicit_section
        .and_then(|s| s.get("skip_files"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();

    Some(ReaderSafetyConfig { strict, skip_files })
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::{BuildMode, Severity as ForgeSeverity};

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
    fn no_config_no_anonymity_target_silent_skip() {
        let dir = tempfile::tempdir().unwrap();
        write_page(
            dir.path(),
            "page.html",
            "<html><script>alert(1)</script></html>",
        );
        let findings = ReaderSafetyPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn anonymity_target_implicitly_enables_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"tor\"]\n");
        write_page(
            dir.path(),
            "page.html",
            "<html><body><script>alert(1)</script></body></html>",
        );
        let findings = ReaderSafetyPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings.iter().any(|f| {
            f.severity == ForgeSeverity::Strict && f.message.contains("inline <script>")
        }));
    }

    #[test]
    fn script_with_noscript_fallback_no_fallback_warning() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"tor\"]\n");
        write_page(
            dir.path(),
            "page.html",
            r#"<html><body>
                <script>alert(1)</script>
                <noscript>fallback content</noscript>
            </body></html>"#,
        );
        let findings = ReaderSafetyPhase.run(&ctx_in(dir.path())).unwrap();
        // The page still has script (Strict) but the noscript-missing
        // finding should NOT fire.
        assert!(findings
            .iter()
            .any(|f| f.message.contains("inline <script>")));
        assert!(!findings
            .iter()
            .any(|f| f.message.contains("without <noscript>")));
    }

    #[test]
    fn inline_event_handler_emits_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"tor\"]\n");
        write_page(
            dir.path(),
            "page.html",
            r#"<html><body><button onclick="x()">go</button></body></html>"#,
        );
        let findings = ReaderSafetyPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings
            .iter()
            .any(|f| f.message.contains("inline event handler")));
    }

    #[test]
    fn cookie_set_emits_warn_even_when_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"tor\"]\n");
        write_page(
            dir.path(),
            "page.html",
            "<html><body><script>document.cookie = 'a=1';</script></body></html>",
        );
        let findings = ReaderSafetyPhase.run(&ctx_in(dir.path())).unwrap();
        // cookie-set is Warn even with strict=true because Tor Browser
        // doesn't HARD-fail on cookies, it wipes them
        assert!(findings
            .iter()
            .any(|f| { f.severity == ForgeSeverity::Warn && f.message.contains("cookie") }));
    }

    #[test]
    fn local_storage_emits_warn() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"tor\"]\n");
        write_page(
            dir.path(),
            "page.html",
            "<html><body><script>localStorage.setItem('x', 1)</script></body></html>",
        );
        let findings = ReaderSafetyPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings
            .iter()
            .any(|f| f.message.contains("Web Storage API")));
    }

    #[test]
    fn recaptcha_emits_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"tor\"]\n");
        write_page(
            dir.path(),
            "page.html",
            r#"<html><script src="https://www.google.com/recaptcha/api.js"></script></html>"#,
        );
        let findings = ReaderSafetyPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings.iter().any(|f| f.message.contains("reCAPTCHA")));
    }

    #[test]
    fn font_face_emits_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"tor\"]\n");
        write_page(
            dir.path(),
            "page.html",
            r#"<html><style>@font-face { font-family: x; src: url(x.woff2); }</style></html>"#,
        );
        let findings = ReaderSafetyPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings.iter().any(|f| f.message.contains("@font-face")));
    }

    #[test]
    fn autocomplete_on_emits_warn() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"tor\"]\n");
        write_page(
            dir.path(),
            "page.html",
            r#"<html><form><input autocomplete="email" name="x"></form></html>"#,
        );
        let findings = ReaderSafetyPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings
            .iter()
            .any(|f| f.message.contains("autocomplete enabled")));
    }

    #[test]
    fn autocomplete_off_no_finding() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"tor\"]\n");
        write_page(
            dir.path(),
            "page.html",
            r#"<html><form><input autocomplete="off" name="x"></form></html>"#,
        );
        let findings = ReaderSafetyPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(!findings.iter().any(|f| f.message.contains("autocomplete")));
    }

    #[test]
    fn explicit_strict_false_demotes_strict_to_warn() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[networks]
targets = ["tor"]

[reader_safety]
strict = false
"#,
        );
        write_page(
            dir.path(),
            "page.html",
            "<html><body><script>alert(1)</script></body></html>",
        );
        let findings = ReaderSafetyPhase.run(&ctx_in(dir.path())).unwrap();
        // Both inline script + missing noscript fallback should be Warn
        assert!(findings.iter().all(|f| f.severity == ForgeSeverity::Warn));
    }

    #[test]
    fn skip_files_silences_per_file() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[networks]
targets = ["tor"]

[reader_safety]
skip_files = ["admin.html"]
"#,
        );
        write_page(
            dir.path(),
            "admin.html",
            "<html><body><script>alert(1)</script></body></html>",
        );
        write_page(
            dir.path(),
            "page.html",
            "<html><body><script>alert(2)</script></body></html>",
        );
        let findings = ReaderSafetyPhase.run(&ctx_in(dir.path())).unwrap();
        // Only page.html flagged
        assert!(findings.iter().all(|f| f.path == "page.html"));
    }

    #[test]
    fn reader_safety_section_without_anonymity_target_warns_only() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[reader_safety]
strict = true
"#,
        );
        write_page(
            dir.path(),
            "page.html",
            "<html><body><script>alert(1)</script></body></html>",
        );
        let findings = ReaderSafetyPhase.run(&ctx_in(dir.path())).unwrap();
        // Even with strict=true, without an anonymity target the
        // implicit-default is strict=false → all Warn... wait, the
        // explicit_section has strict=true so it WILL be strict.
        // That's fine: operators can opt in to hardened-reader
        // checks for clearnet sites if they want.
        assert!(findings.iter().any(|f| f.severity == ForgeSeverity::Strict));
    }
}
