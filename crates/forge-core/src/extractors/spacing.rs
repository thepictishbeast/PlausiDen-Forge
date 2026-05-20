//! `spacing` — rhythm + density extraction from reference
//! captures.
//!
//! Task #266 per the reference-matching arc. Reads a
//! [`crate::extractors::ComputedStylesDump`] and emits the
//! spacing signal:
//!
//! * `rhythm_unit_px` — the most-common gap value across
//!   `margin-*`, `padding-*`, `gap`, and `row-gap` properties.
//!   The mode is the substrate's "this site lives on an N-px
//!   rhythm" answer.
//! * `section_gap_p95` — 95th percentile of vertical-gap values
//!   (margin-top + margin-bottom + padding-top + padding-bottom
//!   distribution). Catches the long-tail "headline before this
//!   section is 96px above the content" pattern.
//! * `content_max_width_px` — most-common `max-width` value.
//!   Indicates the operator's measure-line decision.
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

impl SpacingResult {
    /// Construct a SpacingResult. Public constructor needed for
    /// external crates because the struct is `#[non_exhaustive]`.
    #[must_use]
    pub fn new(
        rhythm_unit_px: u32,
        section_gap_p95_px: u32,
        content_max_width_px: u32,
        gap_distribution_px: BTreeMap<u32, u32>,
    ) -> Self {
        Self {
            rhythm_unit_px,
            section_gap_p95_px,
            content_max_width_px,
            gap_distribution_px,
        }
    }
}

/// Aggregate spacing result.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SpacingResult {
    /// Most-common gap value across margin/padding/gap
    /// properties, in CSS pixels (rounded to nearest integer).
    /// 0 when no resolvable observations.
    pub rhythm_unit_px: u32,
    /// 95th percentile of vertical-gap values (px). 0 when no
    /// resolvable observations.
    pub section_gap_p95_px: u32,
    /// Most-common `max-width` value, in px. 0 when no
    /// resolvable observations.
    pub content_max_width_px: u32,
    /// Full distribution of all gap-bearing property values
    /// (px → count, sorted by key). Available for downstream
    /// percentile calculations beyond p95.
    pub gap_distribution_px: BTreeMap<u32, u32>,
}

const GAP_PROPERTIES: &[&str] = &[
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "gap",
    "row-gap",
    "column-gap",
];

const VERTICAL_GAP_PROPERTIES: &[&str] = &[
    "margin-top",
    "margin-bottom",
    "padding-top",
    "padding-bottom",
    "row-gap",
];

/// Extract from a JSON dump file.
pub fn extract_from_path(path: &Path) -> Result<SpacingResult, ExtractorError> {
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
pub fn extract(dump: &ComputedStylesDump) -> SpacingResult {
    // Build the gap distribution across all gap-bearing
    // properties. Skip 0-px entries (no gap to model).
    let mut gap_dist: BTreeMap<u32, u32> = BTreeMap::new();
    for prop in GAP_PROPERTIES {
        let Some(values) = dump.property_values.get(*prop) else {
            continue;
        };
        for (raw, count) in values {
            let Some(px) = parse_px(raw) else {
                continue;
            };
            if px == 0 {
                continue;
            }
            *gap_dist.entry(px).or_insert(0) += *count;
        }
    }

    let rhythm_unit_px = mode_value(&gap_dist);

    // Vertical-gap-only distribution for p95.
    let mut vertical_dist: BTreeMap<u32, u32> = BTreeMap::new();
    for prop in VERTICAL_GAP_PROPERTIES {
        let Some(values) = dump.property_values.get(*prop) else {
            continue;
        };
        for (raw, count) in values {
            let Some(px) = parse_px(raw) else { continue };
            if px == 0 {
                continue;
            }
            *vertical_dist.entry(px).or_insert(0) += *count;
        }
    }
    let section_gap_p95_px = percentile_from_distribution(&vertical_dist, 0.95);

    let mut max_width_dist: BTreeMap<u32, u32> = BTreeMap::new();
    if let Some(values) = dump.property_values.get("max-width") {
        for (raw, count) in values {
            if let Some(px) = parse_px(raw) {
                if px > 0 {
                    *max_width_dist.entry(px).or_insert(0) += *count;
                }
            }
        }
    }
    let content_max_width_px = mode_value(&max_width_dist);

    SpacingResult {
        rhythm_unit_px,
        section_gap_p95_px,
        content_max_width_px,
        gap_distribution_px: gap_dist,
    }
}

fn mode_value(dist: &BTreeMap<u32, u32>) -> u32 {
    dist.iter()
        .max_by(|a, b| {
            a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)) // ties broken by smaller px
        })
        .map(|(k, _)| *k)
        .unwrap_or(0)
}

fn percentile_from_distribution(dist: &BTreeMap<u32, u32>, p: f64) -> u32 {
    let total: u64 = dist.values().map(|v| u64::from(*v)).sum();
    if total == 0 {
        return 0;
    }
    let target = ((total as f64) * p).ceil() as u64;
    let mut cumulative: u64 = 0;
    for (k, count) in dist {
        cumulative += u64::from(*count);
        if cumulative >= target {
            return *k;
        }
    }
    // Fallback: largest key.
    dist.keys().last().copied().unwrap_or(0)
}

fn parse_px(raw: &str) -> Option<u32> {
    let trimmed = raw.trim();
    let body = trimmed.strip_suffix("px")?;
    body.trim()
        .parse::<f64>()
        .ok()
        .map(|f| f.round().max(0.0) as u32)
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
    fn extract_rhythm_unit_picks_mode_across_gap_properties() {
        let dump = dump_with(&[
            ("margin-top", &[("8px", 5), ("16px", 50)]),
            ("padding-left", &[("16px", 30), ("24px", 10)]),
            ("gap", &[("32px", 2)]),
        ]);
        let r = extract(&dump);
        // 16px is the mode (50 + 30 = 80 vs 8px=5, 24px=10, 32px=2).
        assert_eq!(r.rhythm_unit_px, 16);
    }

    #[test]
    fn extract_section_gap_p95_uses_vertical_only_distribution() {
        let dump = dump_with(&[
            // Vertical: many small + one huge.
            ("margin-top", &[("8px", 10), ("16px", 10), ("96px", 1)]),
            // Horizontal-only — should not affect p95.
            ("margin-left", &[("256px", 100)]),
        ]);
        let r = extract(&dump);
        // 21 vertical observations. p95 idx = ceil(21*0.95) = 20.
        // Sorted distribution ascending [8,8,8,8,8,8,8,8,8,8,16,16,...,96].
        // Cumulative: 8→10, 16→20, 96→21. target=20 reached at 16.
        assert_eq!(r.section_gap_p95_px, 16);
    }

    #[test]
    fn extract_section_gap_p95_catches_long_tail_outlier() {
        let mut counts: Vec<(&str, u32)> = (0..20).map(|_| ("8px", 1)).collect();
        counts.push(("96px", 1));
        // Above doesn't work in const; rebuild via Vec.
        let mut inner = BTreeMap::new();
        inner.insert("8px".to_owned(), 20u32);
        inner.insert("96px".to_owned(), 1u32);
        let mut property_values = BTreeMap::new();
        property_values.insert("margin-top".to_owned(), inner);
        let dump = ComputedStylesDump {
            spec: StylesSpec::V1,
            property_values,
        };
        let r = extract(&dump);
        // 21 observations. p95 idx = ceil(21*0.95)=20. Cumulative
        // hits 20 at "8px" (count 20). So p95 = 8.
        assert_eq!(r.section_gap_p95_px, 8);
    }

    #[test]
    fn extract_content_max_width_picks_mode() {
        let dump = dump_with(&[("max-width", &[("768px", 3), ("1280px", 10), ("960px", 5)])]);
        let r = extract(&dump);
        assert_eq!(r.content_max_width_px, 1280);
    }

    #[test]
    fn extract_zero_px_entries_are_ignored() {
        let dump = dump_with(&[
            ("margin-top", &[("0px", 100), ("16px", 5)]),
            ("padding-left", &[("0px", 50)]),
        ]);
        let r = extract(&dump);
        // 0px excluded; only 16px observed → mode 16.
        assert_eq!(r.rhythm_unit_px, 16);
    }

    #[test]
    fn extract_returns_zero_when_no_resolvable_observations() {
        let dump = dump_with(&[("margin-top", &[("auto", 5)])]);
        let r = extract(&dump);
        assert_eq!(r.rhythm_unit_px, 0);
        assert_eq!(r.section_gap_p95_px, 0);
        assert_eq!(r.content_max_width_px, 0);
    }

    #[test]
    fn extract_gap_distribution_includes_all_gap_properties() {
        let dump = dump_with(&[
            ("margin-top", &[("8px", 1)]),
            ("padding-bottom", &[("16px", 1)]),
            ("gap", &[("24px", 1)]),
            ("column-gap", &[("32px", 1)]),
        ]);
        let r = extract(&dump);
        assert!(r.gap_distribution_px.contains_key(&8));
        assert!(r.gap_distribution_px.contains_key(&16));
        assert!(r.gap_distribution_px.contains_key(&24));
        assert!(r.gap_distribution_px.contains_key(&32));
    }

    #[test]
    fn percentile_from_distribution_basic() {
        let mut d = BTreeMap::new();
        d.insert(10u32, 90);
        d.insert(100u32, 10);
        // Total 100, p95 → idx 95. Cumulative 10→90, 100→100. So
        // p95 = 100.
        assert_eq!(percentile_from_distribution(&d, 0.95), 100);
        // p50 → idx 50. Cumulative 10→90 covers it. So p50 = 10.
        assert_eq!(percentile_from_distribution(&d, 0.50), 10);
    }

    #[test]
    fn mode_value_breaks_ties_with_smaller_px() {
        let mut d = BTreeMap::new();
        d.insert(8u32, 10);
        d.insert(16u32, 10);
        d.insert(24u32, 5);
        // Tie between 8 + 16; smaller wins → 8.
        assert_eq!(mode_value(&d), 8);
    }

    #[test]
    fn parse_px_requires_px_suffix() {
        assert_eq!(parse_px("16px"), Some(16));
        assert_eq!(parse_px("15.6px"), Some(16));
        assert!(parse_px("auto").is_none());
        assert!(parse_px("16").is_none());
    }

    #[test]
    fn extract_from_path_round_trips_dump() {
        let dump = dump_with(&[("margin-top", &[("16px", 5)])]);
        let path = std::env::temp_dir().join(format!("forge-spacing-{}", std::process::id()));
        std::fs::write(&path, serde_json::to_string(&dump).unwrap()).unwrap();
        let r = extract_from_path(&path).unwrap();
        assert_eq!(r.rhythm_unit_px, 16);
        let _ = std::fs::remove_file(&path);
    }
}
