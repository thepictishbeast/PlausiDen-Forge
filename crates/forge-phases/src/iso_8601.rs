//! `iso_8601` — flag non-ISO 8601 date / time literals in
//! committed sources (`*.md`, `*.toml`, `*.json`, `*.rs`,
//! `*.html`).
//!
//! Owner directive 2026-05-13: "you should also adhere to iso
//! standards." T69 (loom docs/ISO_STANDARDS.md) catalogues which
//! ISO/IEC standards every substrate repo defaults to. This
//! phase mechanically enforces ISO 8601 (`YYYY-MM-DDTHH:MM:SSZ`
//! or the date-only `YYYY-MM-DD` shorthand) for any free-text
//! timestamp.
//!
//! ## What flags as a finding
//!
//! * `5/14/2026` / `5/14/26` (US slash format)
//! * `14/5/2026` / `14/05/2026` (EU slash format)
//! * `14-5-2026` (slash-with-dashes)
//! * `May 14 2026` / `14 May 2026` (English month-name format)
//! * `5.14.2026` (dot-separated)
//!
//! ## What does NOT flag
//!
//! * `2026-05-14` (ISO 8601 date)
//! * `2026-05-14T12:34:56Z` (ISO 8601 datetime)
//! * `2026-05-14T12:34:56+00:00` (ISO 8601 with offset)
//! * Raw 4-digit numbers, version strings, port numbers
//! * Code constants like `Y_2026` or `2026.0`
//!
//! ## Severity
//!
//! `Warn` by default — bare timestamps in prose / commit messages
//! are sometimes legit shorthand. Strict mode (forge.toml
//! `[iso_8601] strict = true`) promotes to `Strict`.
//!
//! AVP-PASS-T69-iso_8601: 2026-05-14.

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use std::path::Path;

/// `iso_8601` phase implementation.
#[derive(Debug, Default)]
pub struct Iso8601Phase;

impl Phase for Iso8601Phase {
    fn name(&self) -> &'static str {
        "iso_8601"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        walk_for_iso(&ctx.root, &mut findings, self.name())?;
        Ok(findings)
    }
}

fn walk_for_iso(
    dir: &Path,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) -> Result<(), BuildError> {
    if !dir.is_dir() {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir).map_err(|e| BuildError::Io {
        context: format!("iso_8601 read_dir {}", dir.display()),
        source: e,
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| BuildError::Io {
            context: format!("iso_8601 iter {}", dir.display()),
            source: e,
        })?;
        let path = entry.path();
        // Skip noise directories: target/, node_modules/, .git/,
        // dist/, build/.
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(
                name,
                "target" | "node_modules" | ".git" | "dist" | "build" | "vendor" | "reports"
            ) {
                continue;
            }
            walk_for_iso(&path, findings, phase)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|s| s.to_str());
        if !matches!(
            ext,
            Some("md")
                | Some("toml")
                | Some("json")
                | Some("rs")
                | Some("html")
                | Some("yml")
                | Some("yaml")
        ) {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue, // unreadable / non-UTF8 — skip
        };
        for (lineno, line) in content.lines().enumerate() {
            if let Some(matched) = find_non_iso_date(line) {
                findings.push(Finding::warn(
                    phase,
                    path.display().to_string(),
                    format!(
                        "{lineno}: non-ISO 8601 date `{matched}` — use YYYY-MM-DD or YYYY-MM-DDTHH:MM:SSZ",
                        lineno = lineno + 1
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Scan a single line for the first non-ISO 8601 date literal.
/// Returns the matched substring or None.
///
/// Recognises:
///   * `M/D/YYYY` or `MM/DD/YYYY` (US slashes)
///   * `D/M/YYYY` (EU slashes — same regex matches)
///   * `D-M-YYYY` or `M-D-YYYY` where year is 4 digits (NOT
///     ISO since ISO is YYYY-MM-DD; reverse is the giveaway)
///   * `Mon DD YYYY` or `DD Mon YYYY` (English month names)
///
/// Does NOT match:
///   * `YYYY-MM-DD` (ISO date — passes)
///   * `YYYY-MM-DDTHH:MM:SS[Z|+HH:MM]` (ISO datetime — passes)
fn find_non_iso_date(line: &str) -> Option<String> {
    // Shortcut: lines containing an ISO 8601 date pattern
    // (`\bYYYY-MM-DD\b`) get a fast-path skip when the line is
    // ENTIRELY ISO. We still scan for non-ISO patterns separately
    // — a line could have both.
    //
    // Note: we deliberately avoid pulling in the regex crate to
    // keep this phase a leaf dep. Hand-rolled scanners.

    // Pass 1: M/D/YYYY or MM/DD/YYYY pattern.
    if let Some(m) = scan_slashed_date(line) {
        return Some(m);
    }
    // Pass 2: Mon DD YYYY or DD Mon YYYY (English month names).
    if let Some(m) = scan_english_month_date(line) {
        return Some(m);
    }
    // Pass 3: M-D-YYYY (dashed but year-LAST — ISO is year-FIRST).
    if let Some(m) = scan_year_last_dashed(line) {
        return Some(m);
    }
    None
}

/// Find `M/D/YYYY` or `MM/DD/YYYY` (also matches D/M/YYYY and
/// dotted variants like `M.D.YYYY`).
fn scan_slashed_date(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Look for a digit-run of 1-2 digits followed by a slash
        // or dot, another 1-2 digits, another slash/dot, then 4
        // digits (the year).
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        // Must be word-boundary-like: previous byte not alnum.
        if i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'-') {
            i += 1;
            continue;
        }
        let start = i;
        let n1 = take_digits(bytes, &mut i, 1, 2);
        if n1 == 0 || i >= bytes.len() {
            i = start + 1;
            continue;
        }
        let sep1 = bytes[i];
        if sep1 != b'/' && sep1 != b'.' {
            i = start + 1;
            continue;
        }
        i += 1;
        let n2 = take_digits(bytes, &mut i, 1, 2);
        if n2 == 0 || i >= bytes.len() {
            i = start + 1;
            continue;
        }
        let sep2 = bytes[i];
        if sep2 != sep1 {
            i = start + 1;
            continue;
        }
        i += 1;
        let n3 = take_digits(bytes, &mut i, 4, 4);
        if n3 == 0 {
            i = start + 1;
            continue;
        }
        // Word-boundary on trailing edge: next byte not alnum.
        if i < bytes.len() && bytes[i].is_ascii_alphanumeric() {
            i = start + 1;
            continue;
        }
        return Some(line[start..i].to_owned());
    }
    None
}

/// Find `D-M-YYYY` or `M-D-YYYY` (dashed BUT year-last —
/// distinguishes from ISO `YYYY-MM-DD` which is year-first).
fn scan_year_last_dashed(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        if i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'-') {
            i += 1;
            continue;
        }
        let start = i;
        let n1 = take_digits(bytes, &mut i, 1, 2);
        if n1 == 0 || i >= bytes.len() || bytes[i] != b'-' {
            i = start + 1;
            continue;
        }
        i += 1;
        let n2 = take_digits(bytes, &mut i, 1, 2);
        if n2 == 0 || i >= bytes.len() || bytes[i] != b'-' {
            i = start + 1;
            continue;
        }
        i += 1;
        let n3 = take_digits(bytes, &mut i, 4, 4);
        if n3 == 0 {
            i = start + 1;
            continue;
        }
        if i < bytes.len() && bytes[i].is_ascii_alphanumeric() {
            i = start + 1;
            continue;
        }
        // n1 + n2 are 1-2 digits each — not 4 digits — so this
        // CAN'T be ISO YYYY-MM-DD. Always a finding.
        return Some(line[start..i].to_owned());
    }
    None
}

/// Find `Jan 14 2026` / `14 Jan 2026` style.
fn scan_english_month_date(line: &str) -> Option<String> {
    const MONTHS: &[&str] = &[
        "Jan",
        "Feb",
        "Mar",
        "Apr",
        "May",
        "Jun",
        "Jul",
        "Aug",
        "Sep",
        "Oct",
        "Nov",
        "Dec",
        "January",
        "February",
        "March",
        "April",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    for &month in MONTHS {
        // Pattern A: `Mon DD YYYY` (with optional comma).
        let mut search = line;
        while let Some(idx) = search.find(month) {
            // Word-boundary: previous char not alphanumeric.
            let absolute_start = (line.len() - search.len()) + idx;
            if absolute_start > 0 && line.as_bytes()[absolute_start - 1].is_ascii_alphanumeric() {
                search = &search[idx + month.len()..];
                continue;
            }
            let after = &search[idx + month.len()..];
            if let Some(matched) = match_day_year(after) {
                let total = month.len() + matched;
                return Some(line[absolute_start..absolute_start + total].to_owned());
            }
            // Pattern B: `DD Mon YYYY` — check chars BEFORE month.
            if let Some(matched_before) = match_day_before(line, absolute_start) {
                let after_year = match_year_after(after);
                if let Some(year_len) = after_year {
                    let total = matched_before + month.len() + year_len;
                    let real_start = absolute_start - matched_before;
                    return Some(line[real_start..real_start + total].to_owned());
                }
            }
            search = &search[idx + month.len()..];
        }
    }
    None
}

/// After a month name, see if `,? +DD,? +YYYY` follows.
/// Returns the consumed length (excluding the month itself).
fn match_day_year(after: &str) -> Option<usize> {
    let bytes = after.as_bytes();
    let mut i = 0;
    // Optional comma
    if i < bytes.len() && bytes[i] == b',' {
        i += 1;
    }
    // Required space(s)
    let space_start = i;
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    if i == space_start {
        return None;
    }
    // 1-2 day digits
    let day_start = i;
    let day = take_digits(bytes, &mut i, 1, 2);
    if day == 0 {
        return None;
    }
    let _ = day_start;
    // Optional comma
    if i < bytes.len() && bytes[i] == b',' {
        i += 1;
    }
    // Spaces
    let space_start2 = i;
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    if i == space_start2 {
        return None;
    }
    // 4 year digits
    let year = take_digits(bytes, &mut i, 4, 4);
    if year == 0 {
        return None;
    }
    Some(i)
}

fn match_day_before(line: &str, before_idx: usize) -> Option<usize> {
    // Walk backwards from `before_idx - 1` for: optional space(s),
    // 1-2 digits.
    let bytes = line.as_bytes();
    let mut i = before_idx;
    if i == 0 || bytes[i - 1] != b' ' {
        return None;
    }
    while i > 0 && bytes[i - 1] == b' ' {
        i -= 1;
    }
    let after_digits = i;
    while i > 0 && bytes[i - 1].is_ascii_digit() {
        i -= 1;
    }
    let digit_count = after_digits - i;
    if !(1..=2).contains(&digit_count) {
        return None;
    }
    Some(before_idx - i)
}

fn match_year_after(after: &str) -> Option<usize> {
    let bytes = after.as_bytes();
    let mut i = 0;
    // Optional comma + space(s)
    if i < bytes.len() && bytes[i] == b',' {
        i += 1;
    }
    let space_start = i;
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    if i == space_start {
        return None;
    }
    let year = take_digits(bytes, &mut i, 4, 4);
    if year == 0 {
        return None;
    }
    Some(i)
}

fn take_digits(bytes: &[u8], i: &mut usize, min: usize, max: usize) -> usize {
    let start = *i;
    while *i < bytes.len() && (*i - start) < max && bytes[*i].is_ascii_digit() {
        *i += 1;
    }
    let n = *i - start;
    if n < min {
        0
    } else {
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates_pass_clean() {
        assert!(find_non_iso_date("Released 2026-05-14.").is_none());
        assert!(find_non_iso_date("Built 2026-05-14T12:34:56Z").is_none());
        assert!(find_non_iso_date("AVP-PASS-T69: 2026-05-14.").is_none());
    }

    #[test]
    fn us_slash_format_flags() {
        assert_eq!(
            find_non_iso_date("Released 5/14/2026."),
            Some("5/14/2026".to_owned())
        );
        assert_eq!(
            find_non_iso_date("Released 05/14/2026."),
            Some("05/14/2026".to_owned())
        );
    }

    #[test]
    fn eu_slash_format_flags() {
        assert_eq!(
            find_non_iso_date("Released 14/05/2026."),
            Some("14/05/2026".to_owned())
        );
    }

    #[test]
    fn dot_format_flags() {
        assert_eq!(
            find_non_iso_date("Released 5.14.2026."),
            Some("5.14.2026".to_owned())
        );
    }

    #[test]
    fn dashed_year_last_flags() {
        assert_eq!(
            find_non_iso_date("Released 14-5-2026."),
            Some("14-5-2026".to_owned())
        );
        assert_eq!(
            find_non_iso_date("Released 5-14-2026."),
            Some("5-14-2026".to_owned())
        );
    }

    #[test]
    fn english_month_after_flags() {
        let r = find_non_iso_date("Released May 14 2026.");
        assert_eq!(r, Some("May 14 2026".to_owned()));
    }

    #[test]
    fn english_month_with_comma_flags() {
        let r = find_non_iso_date("Released May 14, 2026.");
        assert_eq!(r, Some("May 14, 2026".to_owned()));
    }

    #[test]
    fn full_month_name_flags() {
        let r = find_non_iso_date("Released January 14, 2026.");
        assert_eq!(r, Some("January 14, 2026".to_owned()));
    }

    #[test]
    fn day_before_month_flags() {
        let r = find_non_iso_date("Released 14 May 2026.");
        assert_eq!(r, Some("14 May 2026".to_owned()));
    }

    #[test]
    fn no_false_positive_on_version_string() {
        assert!(find_non_iso_date("v1.0.0 ships next week").is_none());
        assert!(find_non_iso_date("port 8080").is_none());
        assert!(find_non_iso_date("threshold 4.5/10").is_none());
    }

    #[test]
    fn no_false_positive_on_iso_inside_dashes_text() {
        // "ISO-2026" is not a date.
        assert!(find_non_iso_date("ISO-2026 spec").is_none());
        // "abc-1-2-2026" — the "abc-" prefix kills the boundary.
        assert!(find_non_iso_date("abc-1-2-2026").is_none());
    }

    #[test]
    fn no_false_positive_in_english_prose() {
        assert!(find_non_iso_date("She turned 14 yesterday.").is_none());
        assert!(find_non_iso_date("There were 5 / 14 wins").is_none());
    }
}
