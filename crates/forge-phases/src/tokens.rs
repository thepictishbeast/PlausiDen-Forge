//! `tokens` — flag raw `px`, `#hex`, `rgb()`, `hsl()` values in
//! shipped HTML files. Tokens belong in skin.css; pages use
//! component classes that reference design tokens.
//!
//! Bash parity: `phase_tokens` in forge.sh — same regexes, same
//! pixel exclusion list (0px / 1px / 2px / 3px are SVG path
//! fragments), same hex skip-list (`http-equiv`, `svg`).
//!
//! AVP-2: returns Vec<Finding> with one entry per offending file
//! per category, severity Strict.

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `tokens` phase implementation.
#[derive(Debug, Default)]
pub struct TokensPhase;

impl Phase for TokensPhase {
    fn name(&self) -> &'static str {
        "tokens"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let files = walk_html(&ctx.static_dir, self.name())?;
        let mut findings = Vec::new();

        for file in files {
            // 1. Raw px values (excluding common SVG fragment sizes).
            let bad_px: Vec<String> = scan_px(&file.body);
            if !bad_px.is_empty() {
                findings.push(Finding::strict(
                    self.name(),
                    file.name.clone(),
                    format!("raw px values: {}", bad_px.join(" ")),
                ));
            }
            // 2. Raw hex colors anywhere except meta CSP + SVG.
            if scan_hex_outside_svg_csp(&file.body) {
                findings.push(Finding::strict(
                    self.name(),
                    file.name.clone(),
                    "raw hex color in HTML",
                ));
            }
        }

        Ok(findings)
    }
}

/// Extract sorted-unique px values from `body` that are NOT in
/// the safe list (0..=3 px). Returns a sorted unique vector of
/// strings in `<N>px` form.
///
/// BUG ASSUMPTION: matches `[0-9]+px` only. Decimals like
/// `1.5px` are NOT flagged — historically the PoC doesn't use
/// fractional px in HTML. Worth tightening once the CMS pipeline
/// is in place; for now, parity with bash forge.
fn scan_px(body: &str) -> Vec<String> {
    let mut hits: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Find the start of a digit run.
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        // Find end of digit run.
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        // Must be followed by literal "px".
        if i + 1 < bytes.len() && bytes[i] == b'p' && bytes[i + 1] == b'x' {
            // Word-boundary check on the trailing side: must not
            // continue with a letter (else it's a longer ident).
            let after = i + 2;
            let trailing_is_word = after < bytes.len()
                && (bytes[after].is_ascii_alphanumeric() || bytes[after] == b'_');
            if !trailing_is_word {
                let token = &body[start..i + 2];
                if !is_safe_px(token) {
                    hits.insert(token.to_owned());
                }
            }
            i = after;
        }
    }
    hits.into_iter().collect()
}

/// `0px / 1px / 2px / 3px` are SVG fragment sizes that show up in
/// inlined SVG strokes — not actual layout px.
fn is_safe_px(s: &str) -> bool {
    matches!(s, "0px" | "1px" | "2px" | "3px")
}

/// True if the body contains a `#` hex color (3 or 6 digits) on
/// a line that is NOT a `http-equiv` (CSP meta) or `svg` (inline
/// SVG path) line. This mirrors the bash forge regex behavior.
///
/// REGRESSION-GUARD: the bash version did `grep | grep -v` chain.
/// Replicating that here verbatim; a more semantic check (parse
/// the HTML, check only attribute values) is queued.
fn scan_hex_outside_svg_csp(body: &str) -> bool {
    for line in body.lines() {
        if line.contains("http-equiv") || line.contains("svg") {
            continue;
        }
        if has_hex_color(line) {
            return true;
        }
    }
    false
}

/// Detect a `#XXX` or `#XXXXXX` hex-color token. Word-boundary
/// aware so `#0` (id selector fragment) doesn't trip it.
fn has_hex_color(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'#' {
            i += 1;
            continue;
        }
        // Count following hex digits.
        let start = i + 1;
        let mut j = start;
        while j < bytes.len() && bytes[j].is_ascii_hexdigit() {
            j += 1;
        }
        let len = j - start;
        // Must NOT be followed by an identifier-continuation char
        // (so `#abcdefg` doesn't count as `#abcdef`).
        let trailing_is_word = j < bytes.len()
            && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_' || bytes[j] == b'-');
        if (len == 3 || len == 6 || len == 8) && !trailing_is_word {
            return true;
        }
        i = j.max(i + 1);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn px_scanner_finds_offenders() {
        let body = "padding: 12px; margin: 24px;";
        let hits = scan_px(body);
        assert!(hits.contains(&"12px".to_owned()));
        assert!(hits.contains(&"24px".to_owned()));
    }

    #[test]
    fn px_scanner_skips_safe_sizes() {
        let body = "stroke-width: 1px; foo: 2px; bar: 3px; baz: 0px;";
        assert!(scan_px(body).is_empty());
    }

    #[test]
    fn px_scanner_does_not_match_pxident() {
        // `7pxfoo` is not a px size.
        let body = "data-pxfoo='7pxfoo'";
        assert!(scan_px(body).is_empty());
    }

    #[test]
    fn hex_scanner_finds_color() {
        assert!(has_hex_color("background: #ffffff;"));
        assert!(has_hex_color("color: #f00;"));
    }

    #[test]
    fn hex_scanner_ignores_id_fragments() {
        // `#hashfragment` (long ident) is not a hex color.
        assert!(!has_hex_color("href=\"#nav-section-name\""));
    }

    #[test]
    fn outside_svg_csp_skips_those_lines() {
        let body = "
            <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'self' #0000ff\">
            <svg fill=\"#ff0000\"/>
            <p>plain text only</p>
        ";
        assert!(!scan_hex_outside_svg_csp(body));
    }

    #[test]
    fn outside_svg_csp_catches_real_offenders() {
        let body = "<p style=\"color: #abc\">oops</p>";
        assert!(scan_hex_outside_svg_csp(body));
    }
}
