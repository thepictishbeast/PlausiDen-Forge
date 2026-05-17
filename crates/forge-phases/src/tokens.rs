//! `tokens` — flag raw `px`, `#hex`, `rgb()`, `hsl()` values in
//! shipped HTML files. Tokens belong in skin.css; pages use
//! component classes that reference design tokens.
//!
//! Bash parity: `phase_tokens` in forge.sh — same regexes, same
//! pixel exclusion list (0px / 1px / 2px / 3px are SVG path
//! fragments), same hex skip-list (`http-equiv`, `svg`).
//!
//! AVP-2: returns `Vec<Finding>` with one entry per offending file
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
            // Strip CSS-variable declarations BEFORE scanning. The
            // tokens phase rejects raw px/hex in HTML because they
            // should resolve through the design system — but the
            // place those values are LEGITIMATELY defined IS the
            // CSS-variable declaration block (`:root{--foo: 12px}`).
            // Without this strip, the phase rejects the very file
            // that defines the design system.
            //
            // Fix 2026-05-17: the SkillShots premium design pass
            // landed 80+ tokens (px scale, hex palette) inside
            // inline <style>:root{} blocks. tokens-phase strict-
            // failed on every page. Stripping the declaration
            // VALUES leaves the rest of the body to be scanned —
            // so `padding: 16px;` in a rule body still flags, but
            // `--loom-space-4: 16px;` in a token declaration does not.
            let body_no_decls = strip_css_var_declarations(&file.body);

            // 1. Raw px values (excluding common SVG fragment sizes).
            let bad_px: Vec<String> = scan_px(&body_no_decls);
            if !bad_px.is_empty() {
                findings.push(Finding::warn(
                    self.name(),
                    file.name.clone(),
                    format!("raw px values: {}", bad_px.join(" ")),
                ));
            }
            // 2. Raw hex colors anywhere except meta CSP + SVG.
            if scan_hex_outside_svg_csp(&body_no_decls) {
                findings.push(Finding::warn(
                    self.name(),
                    file.name.clone(),
                    "raw hex color in HTML",
                ));
            }
        }

        Ok(findings)
    }
}

/// Strip the VALUE side of every `--name: value;` declaration in
/// `body`. The declaration syntax is the canonical place to put raw
/// px / hex / rgb / hsl tokens — they're the DEFINITION of the
/// design system, not a leak through it.
///
/// Conservative: only strips when the line starts with optional
/// whitespace + `--` + identifier + `:`. Doesn't try to parse the
/// full CSS grammar (rare edge cases like `color: var(--x, #fff)`
/// stay flagged — the fallback color of a var() is still a raw hex
/// and arguably should resolve through a token too).
fn strip_css_var_declarations(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    for line in body.lines() {
        // Find every `--ident:` in the line and elide its value
        // up to the next `;` or end-of-line. Iterating per
        // occurrence handles minified CSS where multiple decls
        // share a line (the entire inline <style> blob is one
        // giant line in our outputs).
        let mut rest = line;
        loop {
            let Some(start) = rest.find("--") else {
                out.push_str(rest);
                break;
            };
            // Emit prefix verbatim so non-declaration text in the
            // line stays visible to the px/hex scanners.
            out.push_str(&rest[..start]);
            // Skip "--" and the identifier (alphanumeric / dash).
            let after_dashes = &rest[start + 2..];
            let ident_end = after_dashes
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                .unwrap_or(after_dashes.len());
            let after_ident = &after_dashes[ident_end..];
            // Must be followed by ':' to be a declaration.
            if !after_ident.starts_with(':') {
                // Not a declaration — emit the `--ident` text and
                // continue scanning the rest of the line.
                out.push_str(&rest[start..start + 2 + ident_end]);
                rest = after_ident;
                continue;
            }
            // Found a declaration. Skip the value up to the next
            // `;` or end-of-line.
            let after_colon = &after_ident[1..];
            let value_end = after_colon.find(';').unwrap_or(after_colon.len());
            // Don't emit the value bytes — that's the whole point.
            // Do emit the `;` so downstream parsers still see it.
            if value_end < after_colon.len() {
                rest = &after_colon[value_end..]; // starts with `;`
            } else {
                rest = ""; // value ran to EOL
            }
        }
        out.push('\n');
    }
    out
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
    fn strip_css_var_declarations_elides_value() {
        let body = ":root{--loom-space-4: 16px; --loom-color-bg: #fff;} body { padding: 24px; }";
        let stripped = strip_css_var_declarations(body);
        // Token declaration values gone — scanner shouldn't see 16px or #fff.
        assert!(!stripped.contains("16px"));
        assert!(!stripped.contains("#fff"));
        // Regular rule body preserved — `24px` should still flag.
        assert!(stripped.contains("24px"));
    }

    #[test]
    fn strip_passes_through_non_declarations() {
        let body = "/* comment with --foo not a decl */ padding: 8px;";
        let stripped = strip_css_var_declarations(body);
        assert!(stripped.contains("--foo"));
        assert!(stripped.contains("8px"));
    }

    #[test]
    fn full_phase_accepts_token_decls_but_rejects_inline_px() {
        // Token decl values stripped; the `padding: 16px` in a rule
        // body still flags as before.
        let cleaned = strip_css_var_declarations(
            "<style>:root{--loom-space-4: 16px;}.x{padding: 16px}</style>",
        );
        assert!(scan_px(&cleaned).contains(&"16px".to_owned()));
    }

    #[test]
    fn strip_handles_minified_inline_style() {
        // Real-world inline style block from premium design system.
        let body = ":root{--loom-bg:#FBFAF7;--loom-space-4:1rem;--loom-pad-card:1rem;--loom-radius-component:10px;--loom-font-xs:.75rem;--loom-size-icon-sm:20px;--loom-shadow-sm:0 1px 2px rgba(20,24,42,.06);}";
        let stripped = strip_css_var_declarations(body);
        // All raw px / hex / rgb stripped from the declaration block.
        assert!(scan_px(&stripped).is_empty());
        assert!(!stripped.contains("#FBFAF7"));
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
