//! `substrate-name-audit` — scan substrate repos for hardcoded
//! tenant-specific names that should never appear in generic
//! substrate code.
//!
//! Per paul 2026-05-21 (#328): "Substrate code stays generic. No
//! client/site/operator names in substrate Rust comments,
//! commits, or test fixtures."
//!
//! ## What it scans
//!
//! Walks every `.rs` / `.toml` / `.md` / `.css` / `.html` file
//! under the supplied roots (default: current working directory).
//! Skips `.git`, `target`, `node_modules`, `dist`, `runs`, and
//! anything matching `static/assets/` (tenant content).
//!
//! ## What it catches
//!
//! Three classes of leaks:
//!
//! 1. **Tenant brand names** — the canonical list lives in the
//!    [`BUILTIN_BRAND_PATTERNS`] constant; match is case-
//!    insensitive substring with a platform-self-reference filter
//!    (a brand followed by `-<repo-suffix>` like `-forge`,
//!    `-loom`, `-avp-doctrine` is suppressed; the platform
//!    naming its own subsystems is not a leak).
//!
//! 2. **Real phone numbers** — North American 10-digit shape.
//!    Excludes the FCC-reserved `555-` prefix used in examples
//!    and docs.
//!
//! 3. **Real-looking street addresses** — digit-prefixed
//!    capitalised-word run ending in a street-type suffix
//!    (Street / Avenue / Road / Drive / Lane / Boulevard /
//!    Court / Way / Plaza / Place). Tuned to surface real-world
//!    addresses while leaving common doctrine prose alone.
//!
//! ## Output
//!
//! Per-finding line on stdout (file:line:hit-class).
//! Trailing summary on stderr.
//!
//! ## Exit codes
//!
//! - `0` — no findings
//! - `1` — at least one finding
//! - `2` — fatal error (bad input path, permission denied, etc.)
//!
//! Suitable for `cargo run -p substrate-name-audit -- <roots>` in
//! local dev or as a CI gate.
//!
//! ## Suppression directive
//!
//! Any line containing `audit-allow:` is exempt from the scan.
//! Use this for legitimate platform-name references like the
//! parent doctrine repo (audit-allow: cites the doctrine repo).
//! Convention: `// audit-allow: <reason>` at end of line.

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use walkdir::{DirEntry, WalkDir};

/// One forbidden-name pattern + classification.
struct Pattern {
    /// The literal substring to match (compared case-insensitive).
    needle: &'static str,
    /// Class — surfaced in the report.
    class: &'static str,
}

/// Platform-repo suffixes — hyphenated names that follow a brand
/// when the substrate refers to its OWN repo (e.g.,
/// `PlausiDen-AVP-Doctrine`, `PlausiDen-Forge`, `PlausiDen-Loom`).
/// A brand match immediately followed by one of these suffixes is
/// suppressed — the platform talking about its own repos is fine.
///
/// All entries are lowercase + dash-prefixed to match the post-
/// brand substring exactly.
const PLATFORM_REPO_SUFFIXES: &[&str] = &[
    "-avp-doctrine",
    "-forge",
    "-loom",
    "-crawler",
    "-annotator",
    "-cms",
    "-crm",
    "-lfi",
    "-auth",
    "-manifest",
    "-canon",
    "-mcp",
    "-tools",
    "-site",
    "-bridge",
    "-audits",
];

/// Built-in forbidden brand-name patterns. Authors extending this
/// list should keep entries short + case-insensitive substrings;
/// the matcher lowercases both sides before comparing.
const BUILTIN_BRAND_PATTERNS: &[Pattern] = &[
    Pattern { needle: "plausiden",       class: "brand" }, // audit-allow: pattern def
    Pattern { needle: "prosperityclub",  class: "brand" }, // audit-allow: pattern def
    Pattern { needle: "prosperity club", class: "brand" }, // audit-allow: pattern def
    Pattern { needle: "sacredvote",      class: "brand" }, // audit-allow: pattern def
    Pattern { needle: "sacred-vote",     class: "brand" }, // audit-allow: pattern def
    Pattern { needle: "sacred.vote",     class: "brand" }, // audit-allow: pattern def
    Pattern { needle: "skillshots",      class: "brand" }, // audit-allow: pattern def
    Pattern { needle: "skill-shots",     class: "brand" }, // audit-allow: pattern def
];

/// Extensions we scan. Anything else (binaries, images, lockfiles,
/// JSON tenant content) is skipped entirely.
const SCAN_EXTENSIONS: &[&str] = &["rs", "toml", "md", "css", "html", "yml", "yaml"];

/// Path components that mark a directory as non-substrate (tenant
/// content / build artifacts / vendored deps) and skip its entire
/// subtree.
const SKIP_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "dist",
    "runs",
    "reports",
    "static",
    "cms",
];

/// One leak finding.
#[derive(Debug)]
struct Finding {
    path: PathBuf,
    line_number: usize,
    matched: String,
    class: &'static str,
    line: String,
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
            let mut by_class: BTreeMap<&'static str, usize> = BTreeMap::new();
            for f in &findings {
                let line_prefix = display_path(&f.path);
                let snippet = f.line.trim();
                println!(
                    "{}:{}: hit {:?} — class={} | {}",
                    line_prefix, f.line_number, f.matched, f.class, snippet
                );
                *by_class.entry(f.class).or_default() += 1;
            }
            eprintln!();
            if findings.is_empty() {
                eprintln!("substrate-name-audit: clean (0 findings)");
                ExitCode::SUCCESS
            } else {
                eprintln!("substrate-name-audit: {} finding(s):", findings.len());
                for (class, count) in by_class {
                    eprintln!("  {class}: {count}");
                }
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("substrate-name-audit: {e:#}");
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
            if !has_scan_extension(entry.path()) {
                continue;
            }
            scan_file(entry.path(), &mut findings)?;
        }
    }
    Ok(findings)
}

fn is_not_skipped(entry: &DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    if entry.file_type().is_dir() {
        // SKIP_DIRS match if the directory name itself is in the
        // list — exact match, no substring.
        !SKIP_DIRS.iter().any(|d| name == *d)
    } else {
        true
    }
}

fn has_scan_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SCAN_EXTENSIONS.iter().any(|allowed| e.eq_ignore_ascii_case(allowed)))
        .unwrap_or(false)
}

fn scan_file(path: &Path, findings: &mut Vec<Finding>) -> Result<()> {
    let body = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return Ok(()), // non-UTF8 / unreadable — skip silently
    };
    for (line_idx, line) in body.lines().enumerate() {
        // Inline-directive escape hatch: a line containing
        // `audit-allow:` is intentionally exempt (typically because
        // it cites the platform repo itself — e.g.,
        // PlausiDen-AVP-Doctrine — or because the binary's own
        // documentation enumerates the patterns it scans for).
        // Convention: `// audit-allow: <reason>` at end of line.
        if line.contains("audit-allow:") {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        // 1. Brand patterns — find each occurrence and skip the
        //    ones that are platform self-references (e.g.,
        //    "PlausiDen-AVP-Doctrine", "PlausiDen-Forge"). A line
        //    with one platform-self-reference + one tenant leak
        //    must still flag the tenant leak, so we iterate every
        //    match position.
        'pattern_loop: for pat in BUILTIN_BRAND_PATTERNS {
            for (idx, _) in lower.match_indices(pat.needle) {
                let after = idx + pat.needle.len();
                let tail = &lower[after..];
                let is_platform_self = PLATFORM_REPO_SUFFIXES
                    .iter()
                    .any(|suf| tail.starts_with(suf));
                if is_platform_self {
                    continue;
                }
                findings.push(Finding {
                    path: path.to_path_buf(),
                    line_number: line_idx + 1,
                    matched: pat.needle.to_owned(),
                    class: pat.class,
                    line: line.to_owned(),
                });
                // One finding per (pattern, line) is enough; move
                // on to the next pattern.
                continue 'pattern_loop;
            }
        }
        // 2. Phone numbers (NNN-NNN-NNNN form, excluding 555-)
        if let Some(matched) = detect_phone(line) {
            findings.push(Finding {
                path: path.to_path_buf(),
                line_number: line_idx + 1,
                matched,
                class: "phone",
                line: line.to_owned(),
            });
        }
        // 3. Street addresses
        if let Some(matched) = detect_address(line) {
            findings.push(Finding {
                path: path.to_path_buf(),
                line_number: line_idx + 1,
                matched,
                class: "address",
                line: line.to_owned(),
            });
        }
    }
    Ok(())
}

/// Detect a North-American 10-digit phone in common formats.
/// Returns the matched substring on hit. Excludes 555-prefix
/// (reserved for fictional use per the FCC).
fn detect_phone(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i + 12 <= bytes.len() {
        // NNN-NNN-NNNN
        if bytes[i].is_ascii_digit()
            && bytes[i + 1].is_ascii_digit()
            && bytes[i + 2].is_ascii_digit()
            && bytes[i + 3] == b'-'
            && bytes[i + 4].is_ascii_digit()
            && bytes[i + 5].is_ascii_digit()
            && bytes[i + 6].is_ascii_digit()
            && bytes[i + 7] == b'-'
            && bytes[i + 8].is_ascii_digit()
            && bytes[i + 9].is_ascii_digit()
            && bytes[i + 10].is_ascii_digit()
            && bytes[i + 11].is_ascii_digit()
        {
            let match_str = &line[i..i + 12];
            if !match_str.starts_with("555-") && &match_str[4..7] != "555" {
                return Some(match_str.to_owned());
            }
        }
        i += 1;
    }
    None
}

/// Detect a real-looking street address. Matches `<digits>
/// <CapitalisedWord(s)> <StreetSuffix>`. Suffix list intentionally
/// limited to common ones to keep false-positive rate low.
fn detect_address(line: &str) -> Option<String> {
    let suffixes = [
        " Street", " St.", " St ", " Avenue", " Ave.", " Ave ", " Road", " Rd.",
        " Rd ", " Drive", " Dr.", " Dr ", " Lane", " Ln.", " Ln ", " Boulevard",
        " Blvd.", " Blvd ", " Court", " Ct.", " Plaza", " Place", " Pl ",
    ];
    for suffix in suffixes {
        if let Some(suffix_idx) = line.find(suffix) {
            // Walk backward from suffix_idx to find a digit sequence
            // followed by at least one capitalised word.
            let before = &line[..suffix_idx];
            if let Some(start) = scan_address_start(before) {
                let address = &line[start..suffix_idx + suffix.len()];
                return Some(address.trim().to_owned());
            }
        }
    }
    None
}

/// Find the start index of a number-prefixed street-address candidate
/// in `before`. Heuristic: rightmost run of `\d+\s+<Cap word>(\s+<word>)*`
/// ending at the end of `before`.
fn scan_address_start(before: &str) -> Option<usize> {
    let bytes = before.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let mut i = bytes.len();
    // Walk back past words until we hit a digit run
    while i > 0 {
        // skip whitespace
        while i > 0 && bytes[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        // walk back through one token
        let token_end = i;
        while i > 0 && !bytes[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        if i == token_end {
            return None;
        }
        let token = &before[i..token_end];
        if token.chars().all(|c| c.is_ascii_digit()) {
            return Some(i);
        }
        let first_char = token.chars().next()?;
        if !first_char.is_ascii_uppercase() {
            return None;
        }
    }
    None
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
    fn detects_brand_in_rust_test_fixture() {
        let mut findings = Vec::new();
        let tmp = std::env::temp_dir().join("audit-brand.rs");
        // audit-allow: test fixture exercising the scanner
        std::fs::write(
            &tmp,
            "fn x() { let brand = \"PlausiDen\"; }\nfn y() {}\n", // audit-allow: test fixture
        )
        .unwrap();
        scan_file(&tmp, &mut findings).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].matched, "plausiden"); // audit-allow: scanner pattern slug
        assert_eq!(findings[0].class, "brand");
        assert_eq!(findings[0].line_number, 1);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn detects_phone_in_text() {
        assert_eq!(
            detect_phone("call 978-351-6495 today"), // audit-allow: test phone fixture
            Some("978-351-6495".to_owned())          // audit-allow: test phone fixture
        );
    }

    #[test]
    fn ignores_555_phone_prefix() {
        assert!(detect_phone("call 555-1234").is_none());
        assert!(detect_phone("978-555-0100").is_none());
    }

    #[test]
    fn detects_address() {
        assert!(detect_address("123 Main Street, Cityville").is_some()); // audit-allow: test address
        assert!(detect_address("45 Park Avenue").is_some());             // audit-allow: test address
    }

    #[test]
    fn ignores_address_prose_with_no_number() {
        assert!(detect_address("walking down Main Street today").is_none());
    }

    #[test]
    fn case_insensitive_brand_match() {
        let mut findings = Vec::new();
        let tmp = std::env::temp_dir().join("audit-case.rs");
        // Mixed-case + UPPERCASE on the same line should still flag.
        std::fs::write(&tmp, "// SacredVote and SACREDVOTE\n").unwrap(); // audit-allow: test fixture
        scan_file(&tmp, &mut findings).unwrap();
        // The matcher reports one finding per (pattern, line) pair,
        // not per occurrence, so this produces exactly one hit for
        // the canonical brand pattern on line 1. (audit-allow: doc)
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].matched, "sacredvote"); // audit-allow: scanner pattern slug
        assert_eq!(findings[0].line_number, 1);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn skip_dir_filter_excludes_target_and_git() {
        // construct a fake DirEntry-like check via path-name only
        for name in [".git", "target", "node_modules", "static", "cms"] {
            let path = std::env::temp_dir().join(name);
            std::fs::create_dir_all(&path).ok();
            let entry = WalkDir::new(std::env::temp_dir())
                .into_iter()
                .filter_map(|e| e.ok())
                .find(|e| e.file_name() == name)
                .expect("entry");
            assert!(!is_not_skipped(&entry), "expected to skip {name}");
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}
