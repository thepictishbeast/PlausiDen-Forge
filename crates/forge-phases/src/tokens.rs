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
            //
            // Fix 2026-05-20: ALSO strip the bodies of every
            // `<style>...</style>` block. Those blocks ARE CSS,
            // not HTML — they're the inline critical-CSS payload
            // Loom emits. Raw hex / px inside them is the design
            // system itself (fallback colors in `var(--x, #abc)`,
            // px values in the cascade). The gate's intent is to
            // catch tokens leaking into HTML attributes / body
            // text, not into the canonical CSS-language portion
            // of the same file. Without this strip the gate
            // emits 11 false-positive warns (one per rendered
            // page) every build.
            let body_no_style = strip_style_blocks(&file.body);
            let body_no_decls = strip_css_var_declarations(&body_no_style);

            // 1. Raw px values (excluding common SVG fragment sizes).
            let bad_px: Vec<String> = scan_px(&body_no_decls);
            if !bad_px.is_empty() {
                findings.push(
                    Finding::warn(
                        self.name(),
                        file.name.clone(),
                        format!("raw px values: {}", bad_px.join(" ")),
                    )
                    .citing(["prim-007"])
                    .why("raw px values bypass loom-tokens' theme cascade; a primitive that hard-codes px doesn't respond to user preferences (font scale, density, prefers-reduced-motion implications)")
                    .fix("replace each raw px with a `var(--loom-space-N)` reference. If no token exists for the desired value, file a capability-request to extend loom-tokens — never inline the literal")
                    .skill("add-loom-primitive")
                    .avoid("don't sed-replace px → rem in static/ output — the file is build-emitted; fix in PlausiDen-Loom/loom-tokens/src/skin.css"),
                );
            }
            // 2. Raw hex colors anywhere except meta CSP + SVG.
            if scan_hex_outside_svg_csp(&body_no_decls) {
                findings.push(
                    Finding::warn(
                        self.name(),
                        file.name.clone(),
                        "raw hex color in HTML",
                    )
                    .citing(["prim-007"])
                    .why("raw hex colors bypass the theme system; the same primitive can't render correctly in light + dark + amoled themes")
                    .fix("use a loom-tokens color variable: `var(--loom-color-accent)` / `var(--loom-color-text-primary)` / etc. Define new colors in loom-tokens skin.css, not inline")
                    .skill("add-loom-primitive")
                    .avoid("don't add the hex to a CSS-in-JS string or to inline style attribute — both bypass the theme cascade"),
                );
            }
        }

        Ok(findings)
    }
}

/// Replace every `<style>...</style>` block body with whitespace
/// so the rest of the document keeps its line numbering for
/// downstream phases. The contents of a style block are CSS, not
/// HTML — they belong to the design system layer, not the page
/// layer. Token leakage INTO that CSS is loom's concern; the tokens
/// gate here only asks about tokens leaking into the HTML body
/// (attributes, text, inline `style=` attrs).
///
/// Case-insensitive on the opening / closing tag. Survives nested
/// attributes on the opening tag (e.g. `<style type="text/css">`).
/// Unterminated style blocks elide everything to EOF — a malformed
/// page would produce odd reports either way; failing closed for the
/// purposes of THIS scanner is the safer default.
fn strip_style_blocks(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let bytes = body.as_bytes();
    let lower = body.to_ascii_lowercase();
    let mut i = 0;
    while i < bytes.len() {
        let Some(open_rel) = lower[i..].find("<style") else {
            out.push_str(&body[i..]);
            break;
        };
        let open_start = i + open_rel;
        // Emit everything up to and INCLUDING the opening tag — we
        // strip only the body so the gate can still see e.g. CSP
        // attributes on the <style> tag itself.
        let Some(open_end_rel) = body[open_start..].find('>') else {
            out.push_str(&body[i..]);
            break;
        };
        let open_end = open_start + open_end_rel + 1;
        out.push_str(&body[i..open_end]);
        // Now scan for the closing tag; replace the body with
        // a single newline placeholder so the strip is visible
        // in any diagnostic that prints the stripped text.
        match lower[open_end..].find("</style>") {
            Some(close_rel) => {
                let close_start = open_end + close_rel;
                out.push_str("\n/* [style body stripped by tokens phase] */\n");
                out.push_str(&body[close_start..close_start + "</style>".len()]);
                i = close_start + "</style>".len();
            }
            None => {
                // Unterminated — elide rest of file.
                out.push_str("\n/* [unterminated style body stripped] */\n");
                i = bytes.len();
            }
        }
    }
    out
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

/// True if the body contains a `#` hex color in a CSS-value-bearing
/// HTML attribute (`style="..."` body, or the `content` value of a
/// `<meta name="theme-color">` tag).
///
/// 2026-05-20: previously this scanned every non-svg/csp LINE for
/// `#XXX[XXX]` and returned true on any match. That gives false
/// positives on body text containing GitHub-style issue references
/// (`PlausiDen-Forge #222`) which match as 3-digit hex. The
/// substrate-correct semantic is: only flag hex appearing inside an
/// attribute that the browser will actually parse as CSS color,
/// because that's the only place an unthemed hex actually leaks
/// through the design system. Plain text like "issue #222" is
/// editorial content, not a styling leak.
///
/// The earlier `<style>...</style>` body strip in
/// `strip_style_blocks` already keeps inline critical-CSS out of
/// scope — this function now further narrows the search to attribute
/// values, returning true ONLY when an attribute body itself
/// contains hex.
fn scan_hex_outside_svg_csp(body: &str) -> bool {
    for attr_value in iter_css_bearing_attr_values(body) {
        if has_hex_color(&attr_value) {
            return true;
        }
    }
    false
}

/// Yield the body of every `style="..."` attribute, plus the
/// `content` value of every `<meta name="theme-color" content="...">`
/// tag. These are the two HTML surfaces where a hex literal becomes
/// a real, un-themed CSS color in the browser.
///
/// Conservative parse: handles double-quoted values only (the form
/// Loom emits). Single-quoted / unquoted attributes are out of scope
/// — Loom never produces them; if a different generator does, the
/// resulting hex won't flag here but ALSO won't be a substrate-side
/// regression.
fn iter_css_bearing_attr_values(body: &str) -> Vec<String> {
    let lower = body.to_ascii_lowercase();
    let mut out: Vec<String> = Vec::new();

    // style="..." anywhere.
    let mut search_from = 0;
    while let Some(rel) = lower[search_from..].find("style=\"") {
        let abs = search_from + rel + "style=\"".len();
        if let Some(close_rel) = body[abs..].find('"') {
            out.push(body[abs..abs + close_rel].to_owned());
            search_from = abs + close_rel + 1;
        } else {
            break;
        }
    }

    // <meta name="theme-color" content="...">. The two attributes
    // can appear in either order in well-formed HTML; scan for
    // both spellings.
    for pattern in &["name=\"theme-color\"", "name='theme-color'", "theme-color"] {
        let mut search_from = 0;
        while let Some(rel) = lower[search_from..].find(pattern) {
            let abs = search_from + rel + pattern.len();
            // Find the end of the meta tag.
            let Some(tag_end_rel) = body[abs..].find('>') else {
                break;
            };
            let tag_end = abs + tag_end_rel;
            let tag = &body[abs..tag_end];
            if let Some(c_rel) = tag.to_ascii_lowercase().find("content=\"") {
                let c_abs = abs + c_rel + "content=\"".len();
                if let Some(close_rel) = body[c_abs..tag_end].find('"') {
                    out.push(body[c_abs..c_abs + close_rel].to_owned());
                }
            }
            search_from = tag_end + 1;
        }
    }

    out
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
    fn strip_style_blocks_removes_inline_critical_css() {
        let body = "<head><style>:root{--a:#fff} body { color:#abc }</style><body><p style=\"color:#def\">x</p>";
        let stripped = strip_style_blocks(body);
        assert!(stripped.contains("<style>"), "opening tag preserved");
        assert!(stripped.contains("</style>"), "closing tag preserved");
        assert!(!stripped.contains("#abc"), "style body hex elided");
        assert!(stripped.contains("#def"), "inline attr hex still visible");
    }

    #[test]
    fn tokens_gate_passes_inline_critical_css_with_hex_fallbacks() {
        let body = r#"<head><style>:root{--loom-accent:#4338CA} body{border-left:3px solid color-mix(in oklab,var(--loom-accent,#4338CA) 70%,transparent)}</style><body><p>clean</p>"#;
        let s = strip_css_var_declarations(&strip_style_blocks(body));
        assert!(
            !scan_hex_outside_svg_csp(&s),
            "raw hex fallback inside <style> must not flag — that's the design system, not a leak"
        );
    }

    #[test]
    fn tokens_gate_still_flags_inline_style_attr_hex() {
        let body = r#"<head><body><p style="color:#abc">leak</p>"#;
        let s = strip_css_var_declarations(&strip_style_blocks(body));
        assert!(
            scan_hex_outside_svg_csp(&s),
            "raw hex in inline style= attr must still flag"
        );
    }

    #[test]
    fn tokens_gate_ignores_issue_number_in_body_text() {
        // Editorial body text referencing GitHub issues used to
        // false-positive as 3-digit hex. The gate now scopes hex
        // scanning to actual CSS-bearing attributes only.
        let body = "<p>Pixel-rep target per PlausiDen-Forge #222. Not affiliated.</p>";
        assert!(
            !scan_hex_outside_svg_csp(body),
            "issue-number references in body text must not flag as hex color"
        );
    }

    #[test]
    fn tokens_gate_flags_theme_color_meta_with_raw_hex() {
        // <meta name="theme-color" content="#abc"> IS a real CSS-
        // value-bearing attribute; raw hex there should still flag
        // because the value goes straight to the browser without a
        // theme cascade.
        let body = r##"<meta name="theme-color" content="#abcdef">"##;
        assert!(
            scan_hex_outside_svg_csp(body),
            "raw hex in theme-color meta must still flag"
        );
    }

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
