//! `backend_coverage` — every declared backend in `backends.toml`
//! must be referenced by at least one UI element; every backend
//! flagged `impl_files = []` (stub) surfaces as a partial-impl
//! warning.
//!
//! Bash parity: `phase_backend_coverage` in forge.sh.

use std::collections::BTreeSet;
use std::fs;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;
use crate::phantom_button::PhantomButtonPhase;

/// `backend_coverage` phase.
#[derive(Debug, Default)]
pub struct BackendCoveragePhase;

impl Phase for BackendCoveragePhase {
    fn name(&self) -> &'static str {
        "backend_coverage"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let backends_path = ctx.root.join("backends.toml");
        let mut findings = Vec::new();
        if !backends_path.exists() {
            findings.push(
                Finding::warn(
                    self.name(),
                    "*",
                    "no backends.toml — UI ↔ backend mapping unverified",
                )
                .citing(["sec-007"])
                .why("without backends.toml the phantom_button + backend_coverage gates can't enforce capability declarations; UI buttons without backing data-backend slugs ship silently broken")
                .fix("create `backends.toml` at the project root with one `[[backend]] id = \"...\" kind = \"...\" endpoint = \"...\"` entry per UI affordance that needs server wiring")
                .skill("author-cms-content"),
            );
            return Ok(findings);
        }
        let toml_text = fs::read_to_string(&backends_path).map_err(|e| BuildError::Io {
            context: format!("{}: read {}", self.name(), backends_path.display()),
            source: e,
        })?;
        let declared = parse_declared_backends(&toml_text);
        let stubs = parse_stub_backends(&toml_text);

        // Collect all data-backend refs from every page.
        let files = walk_html(&ctx.static_dir, self.name())?;
        let mut all_refs: BTreeSet<String> = BTreeSet::new();
        for file in &files {
            for r in PhantomButtonPhase::extract_data_backends_pub(&file.body) {
                all_refs.insert(r);
            }
        }

        for name in &declared {
            if !all_refs.contains(name) {
                findings.push(
                    Finding::warn(
                        self.name(),
                        "backends.toml",
                        format!("[{name}] declared but no UI references it (dead spec)"),
                    )
                    .citing(["sec-007"])
                    .why("a backend declared but unused signals either a deleted UI affordance that left the spec behind, or a planned UI that was never wired. Either way: dead spec drift")
                    .fix(format!(
                        "either: (a) remove [[backend]] id = \"{name}\" from backends.toml if the UI affordance is gone, OR (b) wire a UI element with `data-backend=\"{name}\"` if the spec is still planned"
                    ))
                    .skill("author-cms-content"),
                );
            }
            if stubs.contains(name) {
                findings.push(
                    Finding::warn(
                        self.name(),
                        "backends.toml",
                        format!("[{name}] declared but impl_files is empty (PARTIAL — stub)"),
                    )
                    .citing(["sec-007"])
                    .why("a backend with empty impl_files is a partial declaration; phantom_button passes (the slug exists) but at runtime the backend has nowhere to dispatch to")
                    .fix(format!(
                        "populate impl_files for [{name}] with the Rust module path(s) that handle the backend's verbs — OR remove the declaration if the implementation isn't planned"
                    ))
                    .skill("author-cms-content"),
                );
            }
        }
        Ok(findings)
    }
}

/// Parse `[backends.NAME]` keys from raw TOML text. Hand-rolled
/// scan — avoids pulling toml dep into forge-phases for a single
/// pattern. Mirrors `phantom_button::read_declared_backends`.
fn parse_declared_backends(toml_text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in toml_text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("[backends.") {
            if let Some(name) = rest.strip_suffix(']') {
                if !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                {
                    out.push(name.to_owned());
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Find backend keys whose section contains `impl_files = []` on
/// the next 2-3 lines. Mirrors bash `grep -A2 ... | grep -qE
/// 'impl_files\s*=\s*\[\s*\]'`.
fn parse_stub_backends(toml_text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let lines: Vec<&str> = toml_text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("[backends.") else {
            continue;
        };
        let Some(name) = rest.strip_suffix(']') else {
            continue;
        };
        // Look ahead a few lines for impl_files = [ ].
        let end = (i + 6).min(lines.len());
        for ahead in &lines[i + 1..end] {
            let a = ahead.trim();
            if a.starts_with('[') {
                break; // next section
            }
            if let Some(eq_value) = a.strip_prefix("impl_files") {
                let v = eq_value.trim_start_matches(|c: char| c == '=' || c.is_whitespace());
                let v = v.trim();
                if v == "[]" || v == "[ ]" {
                    out.insert(name.to_owned());
                    break;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_declared_basic() {
        let t = "
            [backends.sign-in]
            method = \"POST\"

            [backends.post-skill]
            method = \"POST\"
        ";
        let out = parse_declared_backends(t);
        assert_eq!(out, vec!["post-skill", "sign-in"]);
    }

    #[test]
    fn parse_stubs_basic() {
        let t = r#"
            [backends.foo]
            impl_files = []

            [backends.bar]
            impl_files = ["src/main.rs"]
        "#;
        let stubs = parse_stub_backends(t);
        assert!(stubs.contains("foo"));
        assert!(!stubs.contains("bar"));
    }

    #[test]
    fn parse_stubs_handles_section_boundary() {
        // impl_files in a different section must not bleed.
        let t = r#"
            [backends.foo]
            method = "GET"

            [backends.bar]
            impl_files = []
        "#;
        let stubs = parse_stub_backends(t);
        assert!(!stubs.contains("foo"));
        assert!(stubs.contains("bar"));
    }
}
