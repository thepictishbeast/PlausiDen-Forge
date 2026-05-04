//! `phantom_button` — every interactive must have a wired backend.
//!
//! Bash parity: walks `backends.toml` for `[backends.NAME]` keys,
//! then per HTML page:
//!   1. Counts <button> tags lacking `data-backend=` (and not in
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
                findings.push(Finding::warn(
                    self.name(),
                    n.clone(),
                    format!(
                        "{unwired} button(s) with no data-backend (UI not declared in backends.toml)"
                    ),
                ));
            }

            // Every declared data-backend must exist in backends.toml.
            for key in extract_data_backends(body) {
                if !declared.contains(&key) {
                    findings.push(Finding::strict(
                        self.name(),
                        n.clone(),
                        format!(
                            "data-backend=\"{key}\" not declared in backends.toml — broken UI"
                        ),
                    ));
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
                    && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
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
                && val.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
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
        let body = r#"<button data-backend="sign-in">x</button> <a data-backend="post-skill">y</a>"#;
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
}
