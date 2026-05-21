//! `substrate-docs-audit` — scans substrate `.rs` files for
//! public-API items that lack a `///` doc comment.
//!
//! Per paul 2026-05-21 (#329): every public-API item across
//! substrate repos must have a useful doc comment. This binary
//! reports any `pub fn` / `pub struct` / `pub enum` / `pub trait`
//! / `pub const` / `pub static` / `pub type` / `pub mod`
//! declaration that is NOT preceded by a `///` doc line.
//!
//! ## What it scans
//!
//! Walks every `.rs` file under the supplied roots (default:
//! current working directory). Skips `.git`, `target`,
//! `node_modules`, `dist`, `runs`.
//!
//! ## Heuristics
//!
//! A public item is "documented" when the line immediately
//! before its declaration (skipping back over `#[…]` attribute
//! lines + `pub(crate)`/`pub(super)` continuations) starts with
//! `///` or `//!`. Tightly heuristic — false-positives possible
//! on multi-attribute decoration. Authors suppress with
//! `// docs-audit-allow: <reason>` on the declaration line.
//!
//! ## Output
//!
//! Per-finding line on stdout:
//!
//! ```text
//! crates/forge-core/src/lib.rs:42: missing doc — `pub fn render_…`
//! ```
//!
//! Trailing summary on stderr.
//!
//! ## Exit codes
//!
//! - `0` — no findings
//! - `1` — at least one finding
//! - `2` — fatal error
//!
//! Suitable for `cargo run -p substrate-docs-audit -- <roots>`
//! locally or as a CI gate.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use walkdir::{DirEntry, WalkDir};

/// Top-level `pub` keywords whose declaration warrants a doc
/// comment. `pub use` is intentionally absent — re-exports
/// inherit their target's docs. `pub mod` is also absent — the
/// module file's `//!` module-level doc is the canonical
/// documentation; flagging the parent's `pub mod foo;` line
/// would be noise.
const PUB_ITEM_KEYWORDS: &[&str] = &[
    "fn ", "struct ", "enum ", "trait ", "const ", "static ",
    "type ",
];

/// Directories to skip entirely (build artifacts / vendored).
const SKIP_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "dist",
    "runs",
    "reports",
];

/// One leak finding.
#[derive(Debug)]
struct Finding {
    path: PathBuf,
    line_number: usize,
    declaration: String,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let roots: Vec<PathBuf> = if args.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        args.iter().map(PathBuf::from).collect()
    };
    match scan_all(&roots) {
        Ok(findings) => {
            for f in &findings {
                let line_prefix = display_path(&f.path);
                println!(
                    "{}:{}: missing doc — `{}`",
                    line_prefix,
                    f.line_number,
                    f.declaration.trim()
                );
            }
            eprintln!();
            if findings.is_empty() {
                eprintln!("substrate-docs-audit: clean (0 findings)");
                ExitCode::SUCCESS
            } else {
                eprintln!("substrate-docs-audit: {} undocumented public item(s)", findings.len());
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("substrate-docs-audit: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn scan_all(roots: &[PathBuf]) -> Result<Vec<Finding>> {
    let mut findings = Vec::new();
    for root in roots {
        let abs = root
            .canonicalize()
            .with_context(|| format!("canonicalize {}", root.display()))?;
        for entry in WalkDir::new(&abs).into_iter().filter_entry(is_not_skipped) {
            let entry = entry.context("walk")?;
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            scan_file(path, &mut findings)?;
        }
    }
    Ok(findings)
}

fn is_not_skipped(entry: &DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    if entry.file_type().is_dir() {
        !SKIP_DIRS.iter().any(|d| name == *d)
    } else {
        true
    }
}

fn scan_file(path: &Path, findings: &mut Vec<Finding>) -> Result<()> {
    let body = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };
    let lines: Vec<&str> = body.lines().collect();
    // Track multi-line raw-string-literal state — `r#"..."#`
    // blocks can contain Rust-looking text (`pub fn …`) inside
    // test fixtures, and those false-positives should be
    // skipped. Heuristic: count occurrences of `r#"` and `"#`
    // per line; if currently inside a raw string at the start
    // of a line, skip scanning that line.
    let mut in_raw_string = false;
    let mut in_continued_string = false;
    for (idx, line) in lines.iter().enumerate() {
        let line_in_raw_string_at_start = in_raw_string;
        let line_in_continued_string_at_start = in_continued_string;
        // Track raw-string-literal state. Count `r#"` opens
        // and `"#` closes; the opens are stripped before
        // counting closes so `r#"#[…]` doesn't double-count
        // (the `"#` overlap with the opener would otherwise be
        // miscounted as a closer).
        let opens = line.matches("r##\"").count() + line.matches("r#\"").count();
        let stripped = line.replace("r##\"", "").replace("r#\"", "");
        let closes = stripped.matches("\"#").count();
        if opens > closes {
            in_raw_string = true;
        } else if closes > opens {
            in_raw_string = false;
        }
        // Track line-continued cooked-string state. A "..." \
        // line ends in `\` and the next line is still inside
        // the same string. End the run when a line WITHOUT a
        // trailing `\` appears.
        let trimmed_end = line.trim_end();
        if line_in_continued_string_at_start {
            if !trimmed_end.ends_with('\\') {
                in_continued_string = false;
            }
        } else if trimmed_end.ends_with("\\n\\")
            || trimmed_end.ends_with("\"\\")
            || (trimmed_end.ends_with('\\')
                && line.matches('"').count() % 2 == 1)
        {
            in_continued_string = true;
        }
        if line_in_raw_string_at_start || line_in_continued_string_at_start {
            continue;
        }
        if !is_pub_item_line(line) {
            continue;
        }
        if line.contains("docs-audit-allow:") {
            continue;
        }
        if is_documented(&lines, idx) {
            continue;
        }
        findings.push(Finding {
            path: path.to_path_buf(),
            line_number: idx + 1,
            declaration: extract_decl(line),
        });
    }
    Ok(())
}

/// True when the line opens with `pub <keyword>` for one of the
/// documented item categories. Strips leading whitespace; permits
/// `pub(crate)` / `pub(super)` modifiers — both still count as
/// public API surface to a caller who's outside the crate or
/// module hierarchy.
fn is_pub_item_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let after_pub = if let Some(rest) = trimmed.strip_prefix("pub ") {
        rest
    } else if let Some(rest) = trimmed
        .strip_prefix("pub(crate) ")
        .or_else(|| trimmed.strip_prefix("pub(super) "))
    {
        rest
    } else {
        return false;
    };
    let after_pub = after_pub.trim_start_matches("async ").trim_start_matches("unsafe ");
    PUB_ITEM_KEYWORDS.iter().any(|kw| after_pub.starts_with(kw))
}

/// Walk backward from the declaration line and skip blank lines
/// + attribute lines (`#[…]`). The first non-skipped line must
/// be a doc comment (`///` or `//!`) or the item is considered
/// undocumented.
///
/// Multi-line attributes (e.g., `#[derive(\n  Debug,\n  …\n)]`)
/// are skipped by also treating attribute-continuation lines —
/// any line whose `trim()` starts with `[A-Za-z]` or `)]` —
/// as attribute body until a real opener `#[` is reached.
fn is_documented(lines: &[&str], idx: usize) -> bool {
    let mut i = idx;
    let mut inside_multi_line_attr = false;
    while i > 0 {
        i -= 1;
        let prev = lines[i].trim_start();
        if prev.is_empty() {
            continue;
        }
        // Multi-line attribute body — keep skipping until we
        // see the opening `#[`.
        if inside_multi_line_attr {
            if prev.starts_with("#[") {
                inside_multi_line_attr = false;
            }
            continue;
        }
        if prev.starts_with("#[") || prev.starts_with("#!") {
            continue;
        }
        // Closing of a multi-line attribute (`)]`, `}]`, `,` at
        // end of a continuation, or a token-only line that's
        // clearly inside `#[…]` body). Switch state and keep
        // walking back.
        if prev == ")]" || prev == "}]" || prev.ends_with(",") || prev.ends_with("=") {
            inside_multi_line_attr = true;
            continue;
        }
        if prev.starts_with("//!") || prev.starts_with("///") {
            return true;
        }
        // Allow a `// docs-audit-allow:` directive on the line
        // immediately above the decl as an alternative.
        if prev.contains("docs-audit-allow:") {
            return true;
        }
        // First substantive preceding line is not a doc — fail.
        return false;
    }
    // Walked off the top of the file without finding a doc.
    false
}

/// Pull a short "what's missing" label out of the declaration
/// line so the report is readable. Returns the first ~80 chars
/// of the line trimmed.
fn extract_decl(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.len() <= 80 {
        trimmed.to_owned()
    } else {
        format!("{}…", &trimmed[..80])
    }
}

fn display_path(path: &Path) -> String {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    path.strip_prefix(&cwd)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pub_fn_with_doc_is_documented() {
        let lines = vec!["/// Adds two numbers.", "pub fn add(a: i32, b: i32) -> i32 {"];
        assert!(is_pub_item_line(lines[1]));
        assert!(is_documented(&lines, 1));
    }

    #[test]
    fn pub_fn_without_doc_is_undocumented() {
        let lines = vec!["", "pub fn naked() {}"];
        assert!(is_pub_item_line(lines[1]));
        assert!(!is_documented(&lines, 1));
    }

    #[test]
    fn pub_fn_with_attribute_between_doc_and_decl_documented() {
        let lines = vec![
            "/// The answer.",
            "#[must_use]",
            "pub fn answer() -> i32 { 42 }",
        ];
        assert!(is_documented(&lines, 2));
    }

    #[test]
    fn pub_struct_recognised() {
        assert!(is_pub_item_line("pub struct Foo {"));
        assert!(is_pub_item_line("pub(crate) struct Bar;"));
        assert!(is_pub_item_line("pub(super) enum Baz {}"));
    }

    #[test]
    fn pub_use_not_audited() {
        // Re-exports inherit doc from target; ignore.
        assert!(!is_pub_item_line("pub use foo::Bar;"));
    }

    #[test]
    fn pub_async_fn_recognised() {
        assert!(is_pub_item_line("pub async fn handler() {}"));
    }

    #[test]
    fn inline_docs_audit_allow_suppresses() {
        let mut findings = Vec::new();
        let tmp = std::env::temp_dir().join("docs-audit-allow.rs");
        std::fs::write(
            &tmp,
            "pub fn shadowed() {} // docs-audit-allow: test exemption\n",
        )
        .unwrap();
        scan_file(&tmp, &mut findings).unwrap();
        assert!(findings.is_empty());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn module_doc_above_decl_suffices() {
        let lines = vec!["//! Module-level docs.", "", "pub fn alpha() {}"];
        assert!(is_documented(&lines, 2));
    }

    #[test]
    fn nested_indented_pub_fn_recognised() {
        // Methods inside impl blocks — indented `pub fn`.
        assert!(is_pub_item_line("    pub fn method(&self) {}"));
    }
}
