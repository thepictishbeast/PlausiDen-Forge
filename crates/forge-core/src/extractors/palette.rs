//! `palette` — color palette extraction from reference captures.
//!
//! Task #264 per the reference-matching arc. Reads a
//! [`crate::extractors::ComputedStylesDump`] and emits the
//! ranked palette: which colors appear, how often, and at
//! what contrast classification.
//!
//! ## What it captures
//!
//! Walks the color-bearing CSS properties (`color`,
//! `background-color`, `border-color`, `box-shadow`,
//! `fill`, `stroke`) and tallies each distinct value's
//! occurrence count. Each entry is classified into a
//! contrast class so the mapping engine can split the
//! palette into light-text + dark-text + accent buckets
//! for token assignment.
//!
//! ## Wire shape
//!
//! ```jsonc
//! [
//!   { "hex": "#000000", "rgb": [0,0,0], "occurrence_count": 42,
//!     "contrast_class": "dark", "source_properties": ["color"] },
//!   { "hex": "#5b3e9b", "rgb": [91,62,155], "occurrence_count": 14,
//!     "contrast_class": "accent", "source_properties":
//!       ["color", "background-color"] }
//! ]
//! ```
//!
//! Entries are sorted descending by occurrence_count.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `#[non_exhaustive]` on every public type.
//! * No unwrap/expect in non-test code.
//! * Pure functions; filesystem I/O bounded to the explicit
//!   dump path argument.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::extractors::{ComputedStylesDump, ExtractorError, StylesSpec};

/// Color-bearing CSS properties this extractor scans.
const COLOR_PROPERTIES: &[&str] = &[
    "color",
    "background-color",
    "border-color",
    "border-top-color",
    "border-right-color",
    "border-bottom-color",
    "border-left-color",
    "outline-color",
    "fill",
    "stroke",
    "caret-color",
    "text-decoration-color",
];

impl PaletteEntry {
    /// Construct a palette entry. Public constructor needed for
    /// external crates because the struct is `#[non_exhaustive]`.
    #[must_use]
    pub fn new(
        hex: impl Into<String>,
        rgb: [u8; 3],
        occurrence_count: u32,
        contrast_class: ContrastClass,
        source_properties: Vec<String>,
    ) -> Self {
        Self {
            hex: hex.into(),
            rgb,
            occurrence_count,
            contrast_class,
            source_properties,
        }
    }
}

/// One palette entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PaletteEntry {
    /// Normalized hex (`#rrggbb` lowercase).
    pub hex: String,
    /// RGB tuple (0-255).
    pub rgb: [u8; 3],
    /// Total occurrence count across all source properties.
    pub occurrence_count: u32,
    /// Contrast classification — see [`ContrastClass`].
    pub contrast_class: ContrastClass,
    /// CSS properties this color appeared under, sorted.
    pub source_properties: Vec<String>,
}

/// Contrast bucket for the palette entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ContrastClass {
    /// Very dark — safe text on light backgrounds.
    Dark,
    /// Very light — safe text on dark backgrounds OR background
    /// color for dark text.
    Light,
    /// Mid-luminance — accent / decorative.
    Accent,
    /// Near-grey — neutral / rule lines.
    Neutral,
}

/// Extract the ranked palette from a computed-styles dump
/// loaded from JSON.
pub fn extract_from_path(path: &Path) -> Result<Vec<PaletteEntry>, ExtractorError> {
    let body = std::fs::read_to_string(path)?;
    let dump: ComputedStylesDump = serde_json::from_str(&body)?;
    if dump.spec != StylesSpec::V1 {
        return Err(ExtractorError::SpecMismatch {
            expected: StylesSpec::V1,
            actual: dump.spec,
        });
    }
    Ok(extract(&dump))
}

/// Pure extraction over an in-memory dump.
#[must_use]
pub fn extract(dump: &ComputedStylesDump) -> Vec<PaletteEntry> {
    // Aggregate counts + source properties per normalized hex.
    let mut by_hex: BTreeMap<String, (u32, [u8; 3], Vec<String>)> = BTreeMap::new();
    for prop in COLOR_PROPERTIES {
        let Some(values) = dump.property_values.get(*prop) else {
            continue;
        };
        for (raw_value, count) in values {
            let Some((hex, rgb)) = parse_color(raw_value) else {
                continue;
            };
            let entry = by_hex.entry(hex).or_insert_with(|| (0u32, rgb, Vec::new()));
            entry.0 = entry.0.saturating_add(*count);
            if !entry.2.iter().any(|p| p == prop) {
                entry.2.push((*prop).to_owned());
            }
        }
    }

    let mut out: Vec<PaletteEntry> = by_hex
        .into_iter()
        .map(|(hex, (count, rgb, mut props))| {
            props.sort();
            PaletteEntry {
                hex,
                rgb,
                occurrence_count: count,
                contrast_class: classify(rgb),
                source_properties: props,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        b.occurrence_count
            .cmp(&a.occurrence_count)
            .then_with(|| a.hex.cmp(&b.hex))
    });
    out
}

/// Parse a CSS color value into normalized hex + RGB tuple.
/// Accepts `rgb(r, g, b)`, `rgb(r g b)`, `rgba(...)` (alpha
/// dropped), `#rrggbb`, `#rgb`. Returns None for unrecognized
/// or transparent colors.
fn parse_color(raw: &str) -> Option<(String, [u8; 3])> {
    let trimmed = raw.trim().to_lowercase();
    if trimmed.is_empty()
        || trimmed == "transparent"
        || trimmed == "none"
        || trimmed.starts_with("currentcolor")
    {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix('#') {
        return parse_hex(rest);
    }
    if let Some(inner) = trimmed
        .strip_prefix("rgb(")
        .or_else(|| trimmed.strip_prefix("rgba("))
    {
        let inner = inner.strip_suffix(')')?;
        let parts: Vec<&str> = inner
            .split(|c: char| c == ',' || c.is_whitespace() || c == '/')
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() < 3 {
            return None;
        }
        let r = parse_channel(parts[0])?;
        let g = parse_channel(parts[1])?;
        let b = parse_channel(parts[2])?;
        return Some((format!("#{r:02x}{g:02x}{b:02x}"), [r, g, b]));
    }
    None
}

fn parse_hex(rest: &str) -> Option<(String, [u8; 3])> {
    let normalized = if rest.len() == 3 {
        rest.chars()
            .flat_map(|c| std::iter::repeat(c).take(2))
            .collect::<String>()
    } else if rest.len() == 6 {
        rest.to_owned()
    } else if rest.len() == 8 {
        // #rrggbbaa — drop alpha.
        rest[..6].to_owned()
    } else {
        return None;
    };
    let bytes: Vec<u8> = (0..3)
        .map(|i| u8::from_str_radix(&normalized[i * 2..i * 2 + 2], 16).ok())
        .collect::<Option<Vec<_>>>()?;
    Some((format!("#{normalized}"), [bytes[0], bytes[1], bytes[2]]))
}

fn parse_channel(s: &str) -> Option<u8> {
    if let Some(pct) = s.strip_suffix('%') {
        let f: f64 = pct.parse().ok()?;
        return Some((f / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8);
    }
    let f: f64 = s.parse().ok()?;
    Some(f.round().clamp(0.0, 255.0) as u8)
}

fn classify(rgb: [u8; 3]) -> ContrastClass {
    // Relative luminance per WCAG 2.x (sRGB).
    let [r, g, b] = rgb.map(|c| {
        let f = f64::from(c) / 255.0;
        if f <= 0.03928 {
            f / 12.92
        } else {
            ((f + 0.055) / 1.055).powf(2.4)
        }
    });
    let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let max_chan = rgb.iter().copied().max().unwrap_or(0);
    let min_chan = rgb.iter().copied().min().unwrap_or(0);
    let saturation_spread = max_chan.saturating_sub(min_chan);

    if luminance < 0.08 {
        ContrastClass::Dark
    } else if luminance > 0.7 {
        ContrastClass::Light
    } else if saturation_spread < 20 {
        ContrastClass::Neutral
    } else {
        ContrastClass::Accent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn dump_with(props: &[(&str, &[(&str, u32)])]) -> ComputedStylesDump {
        let mut property_values = BTreeMap::new();
        for (prop, values) in props {
            let mut inner = BTreeMap::new();
            for (val, count) in *values {
                inner.insert((*val).to_owned(), *count);
            }
            property_values.insert((*prop).to_owned(), inner);
        }
        ComputedStylesDump {
            spec: StylesSpec::V1,
            property_values,
        }
    }

    #[test]
    fn extract_single_color_returns_one_entry() {
        let dump = dump_with(&[("color", &[("rgb(0, 0, 0)", 5)])]);
        let palette = extract(&dump);
        assert_eq!(palette.len(), 1);
        assert_eq!(palette[0].hex, "#000000");
        assert_eq!(palette[0].rgb, [0, 0, 0]);
        assert_eq!(palette[0].occurrence_count, 5);
        assert_eq!(palette[0].contrast_class, ContrastClass::Dark);
    }

    #[test]
    fn extract_merges_same_color_across_properties() {
        let dump = dump_with(&[
            ("color", &[("rgb(91, 62, 155)", 3)]),
            ("background-color", &[("#5b3e9b", 4)]),
        ]);
        let palette = extract(&dump);
        assert_eq!(palette.len(), 1);
        assert_eq!(palette[0].occurrence_count, 7);
        assert_eq!(palette[0].source_properties.len(), 2);
        assert!(palette[0].source_properties.contains(&"color".to_owned()));
        assert!(palette[0]
            .source_properties
            .contains(&"background-color".to_owned()));
    }

    #[test]
    fn extract_sorts_by_occurrence_descending() {
        let dump = dump_with(&[(
            "color",
            &[
                ("rgb(0,0,0)", 5),
                ("rgb(255,255,255)", 100),
                ("rgb(100,100,100)", 30),
            ],
        )]);
        let palette = extract(&dump);
        assert_eq!(palette.len(), 3);
        assert_eq!(palette[0].occurrence_count, 100);
        assert_eq!(palette[1].occurrence_count, 30);
        assert_eq!(palette[2].occurrence_count, 5);
    }

    #[test]
    fn classify_assigns_dark_for_near_black() {
        assert_eq!(classify([0, 0, 0]), ContrastClass::Dark);
        assert_eq!(classify([15, 15, 15]), ContrastClass::Dark);
    }

    #[test]
    fn classify_assigns_light_for_near_white() {
        assert_eq!(classify([255, 255, 255]), ContrastClass::Light);
        assert_eq!(classify([240, 240, 240]), ContrastClass::Light);
    }

    #[test]
    fn classify_assigns_neutral_for_mid_grey() {
        assert_eq!(classify([128, 128, 128]), ContrastClass::Neutral);
    }

    #[test]
    fn classify_assigns_accent_for_saturated_mid_luminance() {
        assert_eq!(classify([91, 62, 155]), ContrastClass::Accent);
        assert_eq!(classify([180, 50, 50]), ContrastClass::Accent);
    }

    #[test]
    fn parse_color_handles_hex_3_6_8_digit() {
        assert_eq!(parse_color("#fff").unwrap().0, "#ffffff");
        assert_eq!(parse_color("#ABCDEF").unwrap().0, "#abcdef");
        assert_eq!(parse_color("#aabbccdd").unwrap().0, "#aabbcc");
    }

    #[test]
    fn parse_color_handles_rgb_and_rgba() {
        assert_eq!(parse_color("rgb(255, 0, 0)").unwrap().0, "#ff0000");
        assert_eq!(parse_color("rgba(0, 128, 0, 0.5)").unwrap().0, "#008000");
    }

    #[test]
    fn parse_color_handles_percentages() {
        let (hex, rgb) = parse_color("rgb(50%, 50%, 50%)").unwrap();
        assert_eq!(hex, "#808080");
        assert_eq!(rgb, [128, 128, 128]);
    }

    #[test]
    fn parse_color_rejects_transparent_and_unknown() {
        assert!(parse_color("transparent").is_none());
        assert!(parse_color("none").is_none());
        assert!(parse_color("currentcolor").is_none());
        assert!(parse_color("not-a-color").is_none());
        assert!(parse_color("").is_none());
    }

    #[test]
    fn extract_from_path_round_trips_dump() {
        let dump = dump_with(&[("color", &[("rgb(0,0,0)", 3)])]);
        let path = std::env::temp_dir().join(format!("forge-palette-{}", std::process::id()));
        std::fs::write(&path, serde_json::to_string(&dump).unwrap()).unwrap();
        let palette = extract_from_path(&path).unwrap();
        assert_eq!(palette.len(), 1);
        assert_eq!(palette[0].hex, "#000000");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn extract_ignores_non_color_properties() {
        let dump = dump_with(&[("font-family", &[("Iowan Old Style", 10)])]);
        let palette = extract(&dump);
        assert!(palette.is_empty());
    }

    #[test]
    fn extract_includes_border_and_fill_properties() {
        let dump = dump_with(&[
            ("border-color", &[("rgb(255, 0, 0)", 1)]),
            ("fill", &[("rgb(0, 255, 0)", 1)]),
            ("stroke", &[("rgb(0, 0, 255)", 1)]),
        ]);
        let palette = extract(&dump);
        assert_eq!(palette.len(), 3);
    }
}
