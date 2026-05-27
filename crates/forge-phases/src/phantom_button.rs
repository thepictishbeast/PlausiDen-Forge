//! `phantom_button` — every interactive must have a wired backend.
//!
//! Bash parity: walks `backends.toml` for `[backends.NAME]` keys,
//! then per HTML page:
//!   1. Counts `<button>` tags lacking `data-backend=` (and not in
//!      the opt-out allowlist) — emit Warn.
//!   2. Each declared `data-backend="X"` must match a key in
//!      backends.toml — strict on miss.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `phantom_button` phase.
#[derive(Debug, Default)]
pub struct PhantomButtonPhase;

const ALLOW_NO_BACKEND: &[&str] = &[
    "data-backend",
    "data-loom-theme-toggle",
    "data-loom-aesthetic-set",
    "data-no-backend",
    r#"type="submit""#,
];

impl Phase for PhantomButtonPhase {
    fn name(&self) -> &'static str {
        "phantom_button"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let declared = read_declared_backends(&ctx.root, self.name())?;
        let files = walk_html(&ctx.static_dir, self.name())?;
        let mut findings = Vec::new();

        for file in files {
            let body = file.body.as_str();
            let n = file.name.clone();

            let mut unwired = 0usize;
            for tag in scan_button_tags(body) {
                if ALLOW_NO_BACKEND.iter().any(|a| tag.contains(a)) {
                    continue;
                }
                unwired += 1;
            }
            if unwired > 0 {
                findings.push(
                    Finding::warn(
                        self.name(),
                        n.clone(),
                        format!(
                            "{unwired} button(s) with no data-backend (UI not declared in backends.toml)"
                        ),
                    )
                    .citing(["sec-007"])
                    .why("interactive UI elements without data-backend are unaudited; the substrate cannot enforce capability declarations on them")
                    .fix("add `data-backend=\"<slug>\"` to each button + declare the slug in backends.toml in the same commit")
                    .skill("author-cms-content")
                    .avoid("don't `grep -r 'class=\"btn' static/` to triage — use `forge audit phantom_button --explain`"),
                );
            }

            // Every declared data-backend must exist in backends.toml.
            for key in extract_data_backends(body) {
                if !declared.contains(&key) && !is_substrate_internal_backend(&key) {
                    findings.push(
                        Finding::strict(
                            self.name(),
                            n.clone(),
                            format!(
                                "data-backend=\"{key}\" not declared in backends.toml — broken UI"
                            ),
                        )
                        // Per task #177: phantom_button enforces sec-007
                        // (AI-exposed capabilities explicitly declared
                        // in manifest) — the same declarative requirement
                        // applies to UI capabilities exposed via
                        // data-backend slugs.
                        .citing(["sec-007"])
                        .why("rendered HTML references a data-backend slug with no entry in backends.toml; the button will not work in production")
                        .fix(format!(
                            "add `[[backend]]\\nid = \"{key}\"\\nkind = \"<kind>\"\\nendpoint = \"<endpoint>\"` to backends.toml in the same commit, OR remove the data-backend attribute"
                        ))
                        .skill("author-cms-content")
                        .avoid("don't `grep -r 'data-backend' static/` — use `forge audit phantom_button --explain`"),
                    );
                }
            }
        }

        Ok(findings)
    }
}

/// Read `[backends.NAME]` keys from `backends.toml`.
fn read_declared_backends(root: &Path, phase: &str) -> Result<BTreeSet<String>, BuildError> {
    let path = root.join("backends.toml");
    let text = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(e) => {
            return Err(BuildError::Io {
                context: format!("{phase}: read {}", path.display()),
                source: e,
            });
        }
    };
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("[backends.") {
            if let Some(name) = rest.strip_suffix(']') {
                if !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                {
                    out.insert(name.to_owned());
                }
            }
        }
    }
    Ok(out)
}

/// Yield each `<button ...>` open tag (the literal substring) in
/// source order. Caller decides what attribute checks matter.
fn scan_button_tags(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut search = body;
    while let Some(idx) = search.find("<button") {
        let after = &search[idx..];
        let Some(end) = after.find('>') else {
            break;
        };
        out.push(after[..=end].to_owned());
        search = &after[end + 1..];
    }
    out
}

impl PhantomButtonPhase {
    /// Public alias for `extract_data_backends` so sibling phases
    /// (notably `backend_coverage`) can scan UI references without
    /// duplicating the parser. Pure function — no shared state.
    #[must_use]
    pub fn extract_data_backends_pub(body: &str) -> BTreeSet<String> {
        extract_data_backends(body)
    }
}

/// Substrate-internal data-backend slugs that the phantom_button
/// phase auto-allows without requiring an explicit backends.toml
/// entry. Used by closed-surface modes (Forge Lite, etc.) where
/// the resolver hardcodes specific slugs that the operator has no
/// way to declare from the lite contract.
///
/// Per architecture audit 2026-05-21 + docs/FORGE_LITE_DIAGNOSTIC_2026_05_22.md
/// (Category 2 lite-surface leak): the lite resolver emits
/// `data-backend="lite-cta"` and similar `lite-*` slugs from
/// fixed templates; requiring tenants to author backends.toml
/// entries for these defeats the "narrow surface" promise. The
/// fix is to auto-allow the `lite-*` prefix.
const SUBSTRATE_INTERNAL_BACKEND_PREFIXES: &[&str] = &["lite-"];

/// True when `key` matches a substrate-internal prefix and
/// should be auto-allowed without an explicit backends.toml
/// declaration.
#[must_use]
pub fn is_substrate_internal_backend(key: &str) -> bool {
    SUBSTRATE_INTERNAL_BACKEND_PREFIXES
        .iter()
        .any(|prefix| key.starts_with(prefix))
}

/// Pull every `data-backend="X"` value out of `body`.
fn extract_data_backends(body: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let needle = "data-backend=\"";
    let mut search = body;
    while let Some(idx) = search.find(needle) {
        let after = &search[idx + needle.len()..];
        if let Some(end) = after.find('"') {
            let val = &after[..end];
            if !val.is_empty()
                && val
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                out.insert(val.to_owned());
            }
            search = &after[end + 1..];
        } else {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_backends_basic() {
        let body =
            r#"<button data-backend="sign-in">x</button> <a data-backend="post-skill">y</a>"#;
        let s = extract_data_backends(body);
        assert!(s.contains("sign-in"));
        assert!(s.contains("post-skill"));
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn scan_button_tags_returns_open_tags() {
        let body = r#"<button data-backend="a">A</button><button>B</button>"#;
        let tags = scan_button_tags(body);
        assert_eq!(tags.len(), 2);
        assert!(tags[0].contains("data-backend=\"a\""));
    }

    #[test]
    fn substrate_internal_lite_prefix_is_auto_allowed() {
        assert!(is_substrate_internal_backend("lite-cta"));
        assert!(is_substrate_internal_backend("lite-nav-link"));
        assert!(is_substrate_internal_backend("lite-footer-1"));
    }

    #[test]
    fn substrate_internal_does_not_match_tenant_slugs() {
        // Tenant-authored slugs that happen to LOOK similar must
        // still require backends.toml declaration.
        assert!(!is_substrate_internal_backend("sign-in"));
        assert!(!is_substrate_internal_backend("post-skill"));
        assert!(!is_substrate_internal_backend("footer-contact"));
        assert!(!is_substrate_internal_backend("contact-lite"));
        assert!(!is_substrate_internal_backend("limited-cta"));
        assert!(!is_substrate_internal_backend(""));
    }

    #[test]
    fn substrate_internal_prefix_must_be_at_start() {
        // "lite-" anywhere but the start doesn't count.
        assert!(!is_substrate_internal_backend("my-lite-thing"));
        assert!(!is_substrate_internal_backend("xlite-cta"));
    }
}
