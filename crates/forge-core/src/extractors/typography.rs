//! `typography` — font family + size distribution extraction.
//!
//! Task #265 per the reference-matching arc. Reads a
//! [`crate::extractors::ComputedStylesDump`] and emits the
//! typography signal: which font families are used and how
//! often, the font-size distribution, the weight set, and the
//! leading (line-height vs font-size) ratio.
//!
//! ## Output
//!
//! ```jsonc
//! {
//!   "font_families": [
//!     { "stack": "Iowan Old Style, Georgia, serif", "occurrence_count": 42 },
//!     { "stack": "JetBrains Mono, monospace", "occurrence_count": 8 }
//!   ],
//!   "size_distribution_px": { "14": 30, "16": 50, "20": 12, "32": 4 },
//!   "weight_set": [400, 600, 700],
//!   "leading_ratio_avg": 1.55
//! }
//! ```
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `#[non_exhaustive]` on every public type.
//! * No unwrap/expect in non-test code.
//! * Pure functions over the in-memory dump.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::extractors::{ComputedStylesDump, ExtractorError, StylesSpec};

impl FontFamilyEntry {
    /// Construct a font-family entry. Public constructor because
    /// the struct is `#[non_exhaustive]`.
    #[must_use]
    pub fn new(stack: impl Into<String>, occurrence_count: u32) -> Self {
        Self {
            stack: stack.into(),
            occurrence_count,
        }
    }
}

impl TypographyResult {
    /// Construct a TypographyResult.
    #[must_use]
    pub fn new(
        font_families: Vec<FontFamilyEntry>,
        size_distribution_px: std::collections::BTreeMap<u32, u32>,
        weight_set: Vec<u32>,
        leading_ratio_avg: f64,
    ) -> Self {
        Self {
            font_families,
            size_distribution_px,
            weight_set,
            leading_ratio_avg,
        }
    }
}

/// One font-family entry in the ranked output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FontFamilyEntry {
    /// Normalized font stack (trimmed, single-spaced).
    pub stack: String,
    /// Number of computed-style observations using this stack.
    pub occurrence_count: u32,
}

/// Aggregate typography result.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TypographyResult {
    /// Font families sorted descending by occurrence_count.
    pub font_families: Vec<FontFamilyEntry>,
    /// Font-size distribution in CSS pixels (rounded to nearest
    /// integer). Sorted ascending by key.
    pub size_distribution_px: BTreeMap<u32, u32>,
    /// Distinct font weights observed, sorted ascending.
    pub weight_set: Vec<u32>,
    /// Average leading ratio (line-height / font-size) across
    /// observations where both values are resolvable as pixels.
    /// 0.0 when no resolvable observations.
    pub leading_ratio_avg: f64,
}

/// Extract typography from a JSON dump file.
pub fn extract_from_path(path: &Path) -> Result<TypographyResult, ExtractorError> {
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
pub fn extract(dump: &ComputedStylesDump) -> TypographyResult {
    let font_families = extract_font_families(dump);
    let size_distribution_px = extract_size_distribution(dump);
    let weight_set = extract_weights(dump);
    let leading_ratio_avg = extract_leading_ratio(dump);

    TypographyResult {
        font_families,
        size_distribution_px,
        weight_set,
        leading_ratio_avg,
    }
}

fn extract_font_families(dump: &ComputedStylesDump) -> Vec<FontFamilyEntry> {
    let Some(values) = dump.property_values.get("font-family") else {
        return Vec::new();
    };
    let mut entries: Vec<FontFamilyEntry> = values
        .iter()
        .filter(|(raw, _)| !raw.trim().is_empty())
        .map(|(raw, count)| FontFamilyEntry {
            stack: normalize_font_stack(raw),
            occurrence_count: *count,
        })
        .collect();
    entries.sort_by(|a, b| {
        b.occurrence_count
            .cmp(&a.occurrence_count)
            .then_with(|| a.stack.cmp(&b.stack))
    });
    entries
}

fn extract_size_distribution(dump: &ComputedStylesDump) -> BTreeMap<u32, u32> {
    let mut out = BTreeMap::new();
    let Some(values) = dump.property_values.get("font-size") else {
        return out;
    };
    for (raw, count) in values {
        if let Some(px) = parse_px(raw) {
            *out.entry(px).or_insert(0) += *count;
        }
    }
    out
}

fn extract_weights(dump: &ComputedStylesDump) -> Vec<u32> {
    let Some(values) = dump.property_values.get("font-weight") else {
        return Vec::new();
    };
    let mut set: Vec<u32> = values.keys().filter_map(|raw| parse_weight(raw)).collect();
    set.sort_unstable();
    set.dedup();
    set
}

fn extract_leading_ratio(dump: &ComputedStylesDump) -> f64 {
    let line_heights = match dump.property_values.get("line-height") {
        Some(v) => v,
        None => return 0.0,
    };
    let font_sizes = match dump.property_values.get("font-size") {
        Some(v) => v,
        None => return 0.0,
    };

    // Build the resolved size distribution as a histogram of
    // (font_size_px, occurrence_count). Average the font-size in
    // pixels across the distribution as the denominator.
    let mut total_fs: f64 = 0.0;
    let mut total_fs_count: u64 = 0;
    for (raw, count) in font_sizes {
        if let Some(px) = parse_px(raw) {
            total_fs += f64::from(px) * f64::from(*count);
            total_fs_count += u64::from(*count);
        }
    }
    if total_fs_count == 0 {
        return 0.0;
    }
    let avg_fs = total_fs / total_fs_count as f64;
    if avg_fs <= 0.0 {
        return 0.0;
    }

    // Average line-height in pixels across the distribution.
    let mut total_lh: f64 = 0.0;
    let mut total_lh_count: u64 = 0;
    for (raw, count) in line_heights {
        if let Some(px) = parse_px(raw) {
            total_lh += f64::from(px) * f64::from(*count);
            total_lh_count += u64::from(*count);
        } else if let Some(ratio) = parse_unitless_ratio(raw) {
            // unitless line-height: ratio * font-size; multiply
            // through the avg_fs since we don't have per-element
            // correlation.
            total_lh += ratio * avg_fs * f64::from(*count);
            total_lh_count += u64::from(*count);
        }
    }
    if total_lh_count == 0 {
        return 0.0;
    }
    let avg_lh = total_lh / total_lh_count as f64;
    avg_lh / avg_fs
}

fn normalize_font_stack(raw: &str) -> String {
    raw.split(',')
        .map(|p| p.trim().trim_matches('"').trim_matches('\''))
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_px(raw: &str) -> Option<u32> {
    let trimmed = raw.trim();
    // Require explicit "px" suffix — unitless values get
    // routed to parse_unitless_ratio. Computed-style dumps from
    // Crawler always include the unit on font-size + line-height.
    let body = trimmed.strip_suffix("px")?;
    body.trim()
        .parse::<f64>()
        .ok()
        .map(|f| f.round().max(0.0) as u32)
}

fn parse_unitless_ratio(raw: &str) -> Option<f64> {
    let trimmed = raw.trim();
    // Reject if it has a unit.
    if trimmed.ends_with("px")
        || trimmed.ends_with('%')
        || trimmed.ends_with("em")
        || trimmed.ends_with("rem")
    {
        return None;
    }
    trimmed.parse::<f64>().ok()
}

fn parse_weight(raw: &str) -> Option<u32> {
    let trimmed = raw.trim().to_lowercase();
    match trimmed.as_str() {
        "normal" => Some(400),
        "bold" => Some(700),
        "lighter" | "bolder" => None, // relative; can't resolve
        _ => trimmed.parse::<u32>().ok(),
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
    fn extract_font_families_sorts_by_occurrence_desc() {
        let dump = dump_with(&[(
            "font-family",
            &[
                ("Iowan Old Style, Georgia, serif", 42),
                ("JetBrains Mono, monospace", 8),
                ("Inter, sans-serif", 30),
            ],
        )]);
        let r = extract(&dump);
        assert_eq!(r.font_families.len(), 3);
        assert_eq!(r.font_families[0].occurrence_count, 42);
        assert_eq!(r.font_families[1].occurrence_count, 30);
        assert_eq!(r.font_families[2].occurrence_count, 8);
    }

    #[test]
    fn extract_normalizes_font_stack_quotes_and_spacing() {
        let dump = dump_with(&[(
            "font-family",
            &[("  \"Iowan Old Style\",   'Georgia',   serif  ", 1)],
        )]);
        let r = extract(&dump);
        assert_eq!(r.font_families[0].stack, "Iowan Old Style, Georgia, serif");
    }

    #[test]
    fn extract_size_distribution_aggregates_by_px() {
        let dump = dump_with(&[(
            "font-size",
            &[("14px", 10), ("16px", 20), ("16.4px", 5), ("32px", 2)],
        )]);
        let r = extract(&dump);
        // 16px + 16.4px → rounds to 16px; combined count 25.
        assert_eq!(r.size_distribution_px.get(&14).copied(), Some(10));
        assert_eq!(r.size_distribution_px.get(&16).copied(), Some(25));
        assert_eq!(r.size_distribution_px.get(&32).copied(), Some(2));
    }

    #[test]
    fn extract_weights_dedupes_and_sorts() {
        let dump = dump_with(&[(
            "font-weight",
            &[
                ("400", 50),
                ("normal", 10),
                ("700", 5),
                ("600", 3),
                ("bold", 2),
            ],
        )]);
        let r = extract(&dump);
        // normal=400, bold=700; expected unique sorted [400, 600, 700].
        assert_eq!(r.weight_set, vec![400, 600, 700]);
    }

    #[test]
    fn extract_leading_ratio_handles_px_line_height() {
        let dump = dump_with(&[
            ("font-size", &[("16px", 1)]),
            ("line-height", &[("24px", 1)]),
        ]);
        let r = extract(&dump);
        // 24 / 16 = 1.5
        assert!((r.leading_ratio_avg - 1.5).abs() < 1e-9);
    }

    #[test]
    fn extract_leading_ratio_handles_unitless_line_height() {
        let dump = dump_with(&[
            ("font-size", &[("16px", 1)]),
            ("line-height", &[("1.5", 1)]),
        ]);
        let r = extract(&dump);
        // Unitless 1.5 * avg_fs 16 = 24; 24/16 = 1.5
        assert!((r.leading_ratio_avg - 1.5).abs() < 1e-9);
    }

    #[test]
    fn extract_leading_ratio_zero_when_no_data() {
        let dump = dump_with(&[]);
        let r = extract(&dump);
        assert_eq!(r.leading_ratio_avg, 0.0);
    }

    #[test]
    fn extract_empty_dump_returns_empty_result() {
        let dump = dump_with(&[]);
        let r = extract(&dump);
        assert!(r.font_families.is_empty());
        assert!(r.size_distribution_px.is_empty());
        assert!(r.weight_set.is_empty());
        assert_eq!(r.leading_ratio_avg, 0.0);
    }

    #[test]
    fn extract_from_path_round_trips_dump() {
        let dump = dump_with(&[("font-family", &[("Inter", 3)])]);
        let path = std::env::temp_dir().join(format!("forge-typography-{}", std::process::id()));
        std::fs::write(&path, serde_json::to_string(&dump).unwrap()).unwrap();
        let r = extract_from_path(&path).unwrap();
        assert_eq!(r.font_families.len(), 1);
        assert_eq!(r.font_families[0].stack, "Inter");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn parse_weight_handles_named_and_numeric() {
        assert_eq!(parse_weight("normal"), Some(400));
        assert_eq!(parse_weight("bold"), Some(700));
        assert_eq!(parse_weight("500"), Some(500));
        assert_eq!(parse_weight("BOLD"), Some(700));
        assert!(parse_weight("lighter").is_none());
        assert!(parse_weight("bolder").is_none());
        assert!(parse_weight("garbage").is_none());
    }

    #[test]
    fn parse_px_requires_px_suffix() {
        assert_eq!(parse_px("16px"), Some(16));
        assert_eq!(parse_px("15.6px"), Some(16));
        // No suffix → reject; routed to parse_unitless_ratio.
        assert!(parse_px("16").is_none());
        assert!(parse_px("garbage").is_none());
    }

    #[test]
    fn parse_unitless_ratio_rejects_unit_values() {
        assert_eq!(parse_unitless_ratio("1.5"), Some(1.5));
        assert!(parse_unitless_ratio("1.5em").is_none());
        assert!(parse_unitless_ratio("24px").is_none());
        assert!(parse_unitless_ratio("150%").is_none());
    }
}
