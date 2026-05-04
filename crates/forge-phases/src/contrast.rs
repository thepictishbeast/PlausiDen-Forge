//! `contrast` — WCAG 2.1 contrast on Loom token pairs.
//!
//! Bash parity: `phase_contrast` + `forge_contrast.py`. Reads
//! `static/loom-tokens.css`, extracts every `--loom-color-*`
//! token (across light + every named data-theme block), and
//! computes WCAG relative-luminance contrast ratio for the
//! curated PAIRS list. Strict-fails any pair below 4.5:1 (AA
//! normal text). Warn for < 3.0:1 (UI element / AA-large floor).
//!
//! Pure math in Rust — no Python dep. The HSL conversion and
//! relative luminance formulas come straight from WCAG 2.1 § 1.4.3.

use std::collections::BTreeMap;
use std::fs;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// `contrast` phase.
#[derive(Debug, Default)]
pub struct ContrastPhase;

/// (fg_token, bg_token, label, min_ratio).
const PAIRS: &[(&str, &str, &str, f64)] = &[
    ("ink", "bg-canvas", "body text on canvas", 4.5),
    ("ink", "surface", "body text on card surface", 4.5),
    ("ink", "surface-muted", "body text on muted surface", 4.5),
    ("ink-muted", "bg-canvas", "muted text on canvas", 4.5),
    ("ink-muted", "surface", "muted text on card surface", 4.5),
    (
        "ink-muted",
        "surface-muted",
        "muted text on muted surface",
        4.5,
    ),
    ("primary-fg", "primary", "button text on primary bg", 4.5),
    ("ink", "warn-bg", "ink on warn callout bg", 4.5),
    ("ink", "bg-overlay", "ink on overlay surface", 4.5),
    (
        "ink-muted",
        "bg-overlay",
        "muted ink on overlay surface",
        4.5,
    ),
    ("border", "bg-canvas", "border on canvas (UI element)", 3.0),
    ("border-strong", "bg-canvas", "strong border on canvas", 3.0),
    ("danger", "bg-canvas", "danger color on canvas", 3.0),
    ("success", "bg-canvas", "success color on canvas", 3.0),
    ("warn", "bg-canvas", "warn color on canvas", 3.0),
];

/// RGB color in the [0.0, 1.0] range.
type Rgb = (f64, f64, f64);

impl Phase for ContrastPhase {
    fn name(&self) -> &'static str {
        "contrast"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let tokens_path = ctx.static_dir.join("loom-tokens.css");
        if !tokens_path.exists() {
            return Ok(vec![Finding::warn(
                self.name(),
                "loom-tokens.css",
                "tokens.css missing — skipping contrast phase",
            )]);
        }
        let css = fs::read_to_string(&tokens_path).map_err(|e| BuildError::Io {
            context: format!("{}: read {}", self.name(), tokens_path.display()),
            source: e,
        })?;

        let themes = parse_tokens(&css);
        if themes.is_empty() {
            return Ok(vec![Finding::warn(
                self.name(),
                "loom-tokens.css",
                "no token blocks parsed — file may be malformed",
            )]);
        }

        let mut findings = Vec::new();
        for theme in themes.keys() {
            let tokens = &themes[theme];
            for (fg_name, bg_name, label, min_ratio) in PAIRS {
                let Some(fg) = tokens.get(*fg_name) else {
                    continue;
                };
                let Some(bg) = tokens.get(*bg_name) else {
                    continue;
                };
                let ratio = contrast_ratio(*fg, *bg);
                if ratio < *min_ratio {
                    let msg = format!(
                        "{theme} theme: {ratio:.2}:1 (need {min_ratio}) — {fg_name} on {bg_name} ({label})"
                    );
                    if *min_ratio >= 4.5 {
                        findings.push(Finding::strict(self.name(), "loom-tokens.css", msg));
                    } else {
                        findings.push(Finding::warn(self.name(), "loom-tokens.css", msg));
                    }
                }
            }
        }
        Ok(findings)
    }
}

/// Parse `:root` + `:root[data-theme="X"]` blocks, returning
/// `{ theme: { token_name: rgb } }`.
///
/// BUG ASSUMPTION: this is a brace-balanced text scan, not a CSS
/// AST parse. Comments containing literal `{` or `}` would skew
/// the depth counter — the PoC's tokens.css doesn't have those
/// in comments, but a real CSS parser is queued (forge-css crate).
fn parse_tokens(css: &str) -> BTreeMap<String, BTreeMap<String, Rgb>> {
    let mut out: BTreeMap<String, BTreeMap<String, Rgb>> = BTreeMap::new();
    let mut cursor = 0usize;
    while cursor < css.len() {
        let Some(rel_idx) = css[cursor..].find(":root") else {
            break;
        };
        let abs_idx = cursor + rel_idx;
        let after_root = &css[abs_idx + ":root".len()..];

        // Optional `[data-theme="X"]` (case-insensitive, attribute
        // spacing ignored — match anything between `:root` and `{`).
        let theme = if after_root.starts_with('[') {
            extract_theme_attr(after_root)
        } else {
            Some("light".to_owned())
        };

        // Find the opening brace.
        let Some(brace_offset) = after_root.find('{') else {
            break;
        };
        let block_start = abs_idx + ":root".len() + brace_offset + 1;
        let block_end = match find_matching_brace(&css[block_start..]) {
            Some(end) => block_start + end,
            None => break,
        };
        cursor = block_end + 1;

        let theme = theme.unwrap_or_else(|| "light".to_owned());
        let block = &css[block_start..block_end];
        let entry = out.entry(theme).or_default();
        for (name, rgb) in parse_color_decls(block) {
            entry.insert(name, rgb);
        }
    }
    out
}

/// Given `[data-theme="X"]<rest>`, extract X. Returns None on
/// malformed attribute (caller falls back to "light").
fn extract_theme_attr(s: &str) -> Option<String> {
    // Find closing `]`.
    let close = s.find(']')?;
    let attr = &s[..=close];
    // Pull `data-theme="X"` or `data-theme=X` value.
    let lower = attr.to_ascii_lowercase();
    let key_idx = lower.find("data-theme")?;
    let after_key = &attr[key_idx + "data-theme".len()..];
    // Skip operator chars (=, ~=, ^=, |=, *=, $=).
    let trimmed = after_key.trim_start_matches(|c: char| {
        c == '=' || c == '~' || c == '^' || c == '|' || c == '*' || c == '$' || c.is_whitespace()
    });
    let val = trimmed.trim_start_matches('"');
    let end = val
        .find(|c: char| c == '"' || c == ']' || c.is_whitespace())
        .unwrap_or(val.len());
    Some(val[..end].to_owned())
}

/// Find offset of the matching `}` for a block whose opening `{`
/// is BEFORE the slice's first char (caller passes the body, not
/// the brace itself). Returns offset of closing brace.
fn find_matching_brace(body: &str) -> Option<usize> {
    let bytes = body.as_bytes();
    let mut depth: i32 = 1;
    for (i, &c) in bytes.iter().enumerate() {
        match c {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Walk a `:root { ... }` block and pull every
/// `--loom-color-NAME: VALUE;` declaration.
fn parse_color_decls(block: &str) -> Vec<(String, Rgb)> {
    let mut out = Vec::new();
    let prefix = "--loom-color-";
    let mut search = block;
    while let Some(idx) = search.find(prefix) {
        let after = &search[idx + prefix.len()..];
        // Name runs to first `:`.
        let Some(colon) = after.find(':') else {
            break;
        };
        let name = after[..colon].trim().to_owned();
        let value_rest = &after[colon + 1..];
        // Value runs to first `;`.
        let Some(semi) = value_rest.find(';') else {
            break;
        };
        let raw_value = value_rest[..semi].trim();
        if let Some(rgb) = parse_color_value(raw_value) {
            // Validate name shape.
            if !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                out.push((name, rgb));
            }
        }
        search = &value_rest[semi + 1..];
    }
    out
}

/// Parse a CSS color: `hsl(H S% L%)`, `hsl(H S% L% / A)`, or `#rgb`/`#rrggbb`.
fn parse_color_value(value: &str) -> Option<Rgb> {
    let v = value.trim();
    if let Some(stripped) = v.strip_prefix('#') {
        return hex_to_rgb(stripped);
    }
    let lower = v.to_ascii_lowercase();
    if let Some(args) = lower
        .strip_prefix("hsl(")
        .or_else(|| lower.strip_prefix("hsla("))
    {
        let body = args.trim_end_matches(')').trim();
        return parse_hsl_args(body);
    }
    None
}

/// Hex like `fff` or `ffffff` → RGB.
fn hex_to_rgb(hex: &str) -> Option<Rgb> {
    let h = hex.trim();
    let chars: String = if h.len() == 3 {
        h.chars().flat_map(|c| [c, c]).collect()
    } else if h.len() == 6 || h.len() == 8 {
        h[..6].to_owned()
    } else {
        return None;
    };
    let r = u8::from_str_radix(&chars[0..2], 16).ok()? as f64 / 255.0;
    let g = u8::from_str_radix(&chars[2..4], 16).ok()? as f64 / 255.0;
    let b = u8::from_str_radix(&chars[4..6], 16).ok()? as f64 / 255.0;
    Some((r, g, b))
}

/// Parse `H S% L%[ / A]` (post-css color-3 notation, comma OR
/// space separated). Whitespace tolerant.
fn parse_hsl_args(s: &str) -> Option<Rgb> {
    // Strip any `/ A` alpha portion.
    let without_alpha = match s.find('/') {
        Some(i) => &s[..i],
        None => s,
    };
    // Replace commas with spaces, then split.
    let cleaned = without_alpha.replace(',', " ");
    let parts: Vec<&str> = cleaned.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }
    let h: f64 = parts[0].parse().ok()?;
    let s_val: f64 = parts[1].trim_end_matches('%').parse::<f64>().ok()? / 100.0;
    let l_val: f64 = parts[2].trim_end_matches('%').parse::<f64>().ok()? / 100.0;
    Some(hsl_to_rgb(h, s_val, l_val))
}

/// HSL → RGB, all params normalized.
fn hsl_to_rgb(h: f64, s: f64, l: f64) -> Rgb {
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - (((h / 60.0) % 2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r1, g1, b1) = if (0.0..60.0).contains(&h) {
        (c, x, 0.0)
    } else if (60.0..120.0).contains(&h) {
        (x, c, 0.0)
    } else if (120.0..180.0).contains(&h) {
        (0.0, c, x)
    } else if (180.0..240.0).contains(&h) {
        (0.0, x, c)
    } else if (240.0..300.0).contains(&h) {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (r1 + m, g1 + m, b1 + m)
}

/// WCAG 2.1 relative luminance.
fn relative_luminance(rgb: Rgb) -> f64 {
    fn chan(c: f64) -> f64 {
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    let (r, g, b) = rgb;
    0.2126 * chan(r) + 0.7152 * chan(g) + 0.0722 * chan(b)
}

/// WCAG contrast ratio.
pub fn contrast_ratio(fg: Rgb, bg: Rgb) -> f64 {
    let l1 = relative_luminance(fg);
    let l2 = relative_luminance(bg);
    let (hi, lo) = if l1 < l2 { (l2, l1) } else { (l1, l2) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn hex_to_rgb_basic() {
        let (r, g, b) = hex_to_rgb("ffffff").unwrap();
        assert!(approx_eq(r, 1.0, 1e-9) && approx_eq(g, 1.0, 1e-9) && approx_eq(b, 1.0, 1e-9));
        let (r, g, b) = hex_to_rgb("000000").unwrap();
        assert!(approx_eq(r, 0.0, 1e-9) && approx_eq(g, 0.0, 1e-9) && approx_eq(b, 0.0, 1e-9));
    }

    #[test]
    fn hex_to_rgb_short_form() {
        let (r, g, b) = hex_to_rgb("f00").unwrap();
        assert!(approx_eq(r, 1.0, 1e-9));
        assert!(approx_eq(g, 0.0, 1e-9));
        assert!(approx_eq(b, 0.0, 1e-9));
    }

    #[test]
    fn hsl_white_and_black() {
        let (r, g, b) = hsl_to_rgb(0.0, 0.0, 1.0); // L=1 → white
        assert!(approx_eq(r, 1.0, 1e-9));
        assert!(approx_eq(g, 1.0, 1e-9));
        assert!(approx_eq(b, 1.0, 1e-9));
        let (r, g, b) = hsl_to_rgb(0.0, 0.0, 0.0); // L=0 → black
        assert!(approx_eq(r, 0.0, 1e-9));
        assert!(approx_eq(g, 0.0, 1e-9));
        assert!(approx_eq(b, 0.0, 1e-9));
    }

    #[test]
    fn hsl_red() {
        let (r, g, b) = hsl_to_rgb(0.0, 1.0, 0.5);
        assert!(approx_eq(r, 1.0, 1e-9));
        assert!(approx_eq(g, 0.0, 1e-9));
        assert!(approx_eq(b, 0.0, 1e-9));
    }

    #[test]
    fn relative_luminance_extremes() {
        // Pure white = 1.0; pure black = 0.0.
        assert!(approx_eq(relative_luminance((1.0, 1.0, 1.0)), 1.0, 1e-6));
        assert!(approx_eq(relative_luminance((0.0, 0.0, 0.0)), 0.0, 1e-9));
    }

    #[test]
    fn contrast_white_on_black_is_21() {
        let r = contrast_ratio((1.0, 1.0, 1.0), (0.0, 0.0, 0.0));
        assert!(approx_eq(r, 21.0, 1e-6));
    }

    #[test]
    fn contrast_same_color_is_1() {
        let r = contrast_ratio((0.5, 0.5, 0.5), (0.5, 0.5, 0.5));
        assert!(approx_eq(r, 1.0, 1e-9));
    }

    #[test]
    fn parse_hsl_arg_three_space() {
        let rgb = parse_hsl_args("220 90% 28%").unwrap();
        // hsl(220 90% 28%) — dark blue. Loose check: blue dominant.
        assert!(rgb.2 > rgb.0);
    }

    #[test]
    fn parse_color_value_supports_slash_alpha() {
        // `/ alpha` should be ignored for opaque conversion.
        let rgb = parse_color_value("hsl(0 100% 50% / 0.5)").unwrap();
        assert!(approx_eq(rgb.0, 1.0, 1e-9));
    }

    #[test]
    fn parse_tokens_extracts_simple_block() {
        let css = r#"
            :root {
                --loom-color-bg-canvas: #ffffff;
                --loom-color-ink: hsl(0 0% 0%);
            }
        "#;
        let themes = parse_tokens(css);
        let light = themes.get("light").unwrap();
        assert!(light.contains_key("bg-canvas"));
        assert!(light.contains_key("ink"));
    }

    #[test]
    fn parse_tokens_handles_data_theme() {
        let css = r#"
            :root[data-theme="dark"] {
                --loom-color-ink: #ffffff;
            }
        "#;
        let themes = parse_tokens(css);
        assert!(themes.contains_key("dark"));
        assert!(themes["dark"].contains_key("ink"));
    }
}
