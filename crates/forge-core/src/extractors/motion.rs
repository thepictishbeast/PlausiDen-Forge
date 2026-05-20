//! `motion` — motion + decorative-treatment extraction.
//!
//! Task #267 per the reference-matching arc. Reads a
//! [`crate::extractors::ComputedStylesDump`] and emits the
//! motion + decorative signal: transition curves + durations,
//! whether scroll triggers are in play, decorative treatments
//! (border-radius mode, box-shadow count, gradient/filter usage).
//!
//! This axis catches the SaaS-trope decorative shapes the
//! editorial_purity_gate refuses: heavy rounded corners,
//! gradient flooding, drop-shadow card stacks, neon glow filters.
//! The mapping engine (#273) consumes these signals to pick
//! substrate-native alternatives (sharp corners, solid blocks,
//! minimal shadow language).
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

/// Aggregate motion + decorative result.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MotionResult {
    /// Distinct CSS transition-timing-function values observed,
    /// sorted by occurrence descending.
    pub transition_curves: Vec<TransitionCurveEntry>,
    /// Transition duration distribution (ms → count, sorted by
    /// key ascending).
    pub transition_durations_ms: BTreeMap<u32, u32>,
    /// True iff `scroll-behavior: smooth` OR `position: sticky`
    /// observed.
    pub has_scroll_triggers: bool,
    /// True iff `animation-name` other than `none` observed.
    pub has_animations: bool,
    /// Mode of border-radius values (px). 0 = no rounded
    /// corners observed.
    pub border_radius_mode_px: u32,
    /// Count of distinct non-`none` box-shadow values.
    pub distinct_box_shadows: u32,
    /// True iff any background-image value contains a CSS
    /// gradient function.
    pub has_gradients: bool,
    /// True iff any filter value other than `none` observed.
    pub has_filters: bool,
}

/// One transition-curve entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TransitionCurveEntry {
    /// Normalized curve string (e.g. `"cubic-bezier(0.4, 0, 0.2, 1)"`,
    /// `"ease-in-out"`).
    pub curve: String,
    /// Occurrence count.
    pub occurrence_count: u32,
}

/// Extract from a JSON dump file.
pub fn extract_from_path(path: &Path) -> Result<MotionResult, ExtractorError> {
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
pub fn extract(dump: &ComputedStylesDump) -> MotionResult {
    let transition_curves = extract_curves(dump);
    let transition_durations_ms = extract_durations(dump);
    let has_scroll_triggers = detect_scroll_triggers(dump);
    let has_animations = detect_animations(dump);
    let border_radius_mode_px = extract_border_radius_mode(dump);
    let distinct_box_shadows = count_distinct_box_shadows(dump);
    let has_gradients = detect_gradients(dump);
    let has_filters = detect_filters(dump);

    MotionResult {
        transition_curves,
        transition_durations_ms,
        has_scroll_triggers,
        has_animations,
        border_radius_mode_px,
        distinct_box_shadows,
        has_gradients,
        has_filters,
    }
}

fn extract_curves(dump: &ComputedStylesDump) -> Vec<TransitionCurveEntry> {
    let Some(values) = dump.property_values.get("transition-timing-function") else {
        return Vec::new();
    };
    let mut entries: Vec<TransitionCurveEntry> = values
        .iter()
        .filter(|(raw, _)| !raw.trim().is_empty() && raw.trim() != "none")
        .map(|(raw, count)| TransitionCurveEntry {
            curve: normalize_curve(raw),
            occurrence_count: *count,
        })
        .collect();
    entries.sort_by(|a, b| {
        b.occurrence_count
            .cmp(&a.occurrence_count)
            .then_with(|| a.curve.cmp(&b.curve))
    });
    entries
}

fn extract_durations(dump: &ComputedStylesDump) -> BTreeMap<u32, u32> {
    let mut out = BTreeMap::new();
    let Some(values) = dump.property_values.get("transition-duration") else {
        return out;
    };
    for (raw, count) in values {
        if let Some(ms) = parse_duration_ms(raw) {
            if ms > 0 {
                *out.entry(ms).or_insert(0) += *count;
            }
        }
    }
    out
}

fn detect_scroll_triggers(dump: &ComputedStylesDump) -> bool {
    let smooth = dump
        .property_values
        .get("scroll-behavior")
        .and_then(|m| m.keys().find(|k| k.trim() == "smooth"))
        .is_some();
    let sticky = dump
        .property_values
        .get("position")
        .and_then(|m| m.keys().find(|k| k.trim() == "sticky"))
        .is_some();
    smooth || sticky
}

fn detect_animations(dump: &ComputedStylesDump) -> bool {
    let Some(values) = dump.property_values.get("animation-name") else {
        return false;
    };
    values
        .keys()
        .any(|k| !matches!(k.trim(), "none" | "" ))
}

fn extract_border_radius_mode(dump: &ComputedStylesDump) -> u32 {
    let Some(values) = dump.property_values.get("border-radius") else {
        return 0;
    };
    let mut dist: BTreeMap<u32, u32> = BTreeMap::new();
    for (raw, count) in values {
        if let Some(px) = parse_px(raw) {
            if px == 0 {
                continue;
            }
            *dist.entry(px).or_insert(0) += *count;
        }
    }
    dist.iter()
        .max_by(|a, b| {
            a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)) // ties: smaller px
        })
        .map(|(k, _)| *k)
        .unwrap_or(0)
}

fn count_distinct_box_shadows(dump: &ComputedStylesDump) -> u32 {
    let Some(values) = dump.property_values.get("box-shadow") else {
        return 0;
    };
    let distinct = values
        .keys()
        .filter(|k| !matches!(k.trim(), "none" | ""))
        .count();
    u32::try_from(distinct).unwrap_or(u32::MAX)
}

fn detect_gradients(dump: &ComputedStylesDump) -> bool {
    let Some(values) = dump.property_values.get("background-image") else {
        return false;
    };
    values.keys().any(|k| {
        let lower = k.to_lowercase();
        lower.contains("linear-gradient(")
            || lower.contains("radial-gradient(")
            || lower.contains("conic-gradient(")
            || lower.contains("repeating-linear-gradient(")
            || lower.contains("repeating-radial-gradient(")
    })
}

fn detect_filters(dump: &ComputedStylesDump) -> bool {
    let Some(values) = dump.property_values.get("filter") else {
        return false;
    };
    values
        .keys()
        .any(|k| !matches!(k.trim(), "none" | ""))
}

fn normalize_curve(raw: &str) -> String {
    raw.trim()
        .replace(", ", ",")
        .replace(" ,", ",")
        .replace(",", ", ")
}

fn parse_duration_ms(raw: &str) -> Option<u32> {
    let trimmed = raw.trim();
    if let Some(body) = trimmed.strip_suffix("ms") {
        return body.trim().parse::<f64>().ok().map(|f| f.round().max(0.0) as u32);
    }
    if let Some(body) = trimmed.strip_suffix('s') {
        return body
            .trim()
            .parse::<f64>()
            .ok()
            .map(|f| (f * 1000.0).round().max(0.0) as u32);
    }
    None
}

fn parse_px(raw: &str) -> Option<u32> {
    let trimmed = raw.trim();
    let body = trimmed.strip_suffix("px")?;
    body.trim().parse::<f64>().ok().map(|f| f.round().max(0.0) as u32)
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
    fn extract_curves_sorts_by_occurrence() {
        let dump = dump_with(&[(
            "transition-timing-function",
            &[("ease-in-out", 5), ("cubic-bezier(0.4, 0, 0.2, 1)", 12), ("linear", 3)],
        )]);
        let r = extract(&dump);
        assert_eq!(r.transition_curves.len(), 3);
        assert_eq!(r.transition_curves[0].occurrence_count, 12);
        assert_eq!(r.transition_curves[1].occurrence_count, 5);
    }

    #[test]
    fn extract_curves_skips_none_and_empty() {
        let dump = dump_with(&[(
            "transition-timing-function",
            &[("ease", 2), ("none", 100), ("", 50)],
        )]);
        let r = extract(&dump);
        assert_eq!(r.transition_curves.len(), 1);
        assert_eq!(r.transition_curves[0].curve, "ease");
    }

    #[test]
    fn extract_durations_handles_s_and_ms_suffixes() {
        let dump = dump_with(&[("transition-duration", &[("200ms", 3), ("0.4s", 5)])]);
        let r = extract(&dump);
        assert_eq!(r.transition_durations_ms.get(&200).copied(), Some(3));
        assert_eq!(r.transition_durations_ms.get(&400).copied(), Some(5));
    }

    #[test]
    fn extract_durations_drops_zero_values() {
        let dump = dump_with(&[("transition-duration", &[("0s", 100), ("150ms", 4)])]);
        let r = extract(&dump);
        assert!(!r.transition_durations_ms.contains_key(&0));
        assert_eq!(r.transition_durations_ms.get(&150).copied(), Some(4));
    }

    #[test]
    fn detect_scroll_triggers_finds_smooth_or_sticky() {
        let dump = dump_with(&[("scroll-behavior", &[("smooth", 1)])]);
        let r = extract(&dump);
        assert!(r.has_scroll_triggers);

        let dump = dump_with(&[("position", &[("sticky", 1)])]);
        let r = extract(&dump);
        assert!(r.has_scroll_triggers);

        let dump = dump_with(&[("position", &[("static", 1)])]);
        let r = extract(&dump);
        assert!(!r.has_scroll_triggers);
    }

    #[test]
    fn detect_animations_finds_non_none_animation_names() {
        let dump = dump_with(&[("animation-name", &[("fadeIn", 1), ("none", 10)])]);
        let r = extract(&dump);
        assert!(r.has_animations);

        let dump = dump_with(&[("animation-name", &[("none", 1)])]);
        let r = extract(&dump);
        assert!(!r.has_animations);
    }

    #[test]
    fn border_radius_mode_returns_most_common_nonzero() {
        let dump = dump_with(&[(
            "border-radius",
            &[("0px", 100), ("4px", 5), ("8px", 30), ("12px", 30)],
        )]);
        let r = extract(&dump);
        // 0px excluded; 8 + 12 tied at 30; tie broken by smaller → 8.
        assert_eq!(r.border_radius_mode_px, 8);
    }

    #[test]
    fn distinct_box_shadows_excludes_none() {
        let dump = dump_with(&[(
            "box-shadow",
            &[("0 1px 2px rgba(0,0,0,0.1)", 5), ("none", 100), ("0 4px 8px rgba(0,0,0,0.2)", 3)],
        )]);
        let r = extract(&dump);
        assert_eq!(r.distinct_box_shadows, 2);
    }

    #[test]
    fn detect_gradients_finds_gradient_functions() {
        let dump = dump_with(&[(
            "background-image",
            &[("linear-gradient(180deg, red, blue)", 1)],
        )]);
        let r = extract(&dump);
        assert!(r.has_gradients);

        let dump = dump_with(&[(
            "background-image",
            &[("radial-gradient(circle, white, black)", 1)],
        )]);
        let r = extract(&dump);
        assert!(r.has_gradients);

        let dump = dump_with(&[("background-image", &[("url(\"image.png\")", 1)])]);
        let r = extract(&dump);
        assert!(!r.has_gradients);
    }

    #[test]
    fn detect_filters_finds_non_none_values() {
        let dump = dump_with(&[("filter", &[("blur(4px)", 1)])]);
        let r = extract(&dump);
        assert!(r.has_filters);

        let dump = dump_with(&[("filter", &[("none", 1)])]);
        let r = extract(&dump);
        assert!(!r.has_filters);
    }

    #[test]
    fn extract_returns_default_on_empty_dump() {
        let dump = dump_with(&[]);
        let r = extract(&dump);
        assert!(r.transition_curves.is_empty());
        assert_eq!(r.border_radius_mode_px, 0);
        assert_eq!(r.distinct_box_shadows, 0);
        assert!(!r.has_gradients);
        assert!(!r.has_filters);
        assert!(!r.has_animations);
        assert!(!r.has_scroll_triggers);
    }

    #[test]
    fn parse_duration_ms_handles_both_units() {
        assert_eq!(parse_duration_ms("200ms"), Some(200));
        assert_eq!(parse_duration_ms("0.4s"), Some(400));
        assert_eq!(parse_duration_ms("1.5s"), Some(1500));
        assert!(parse_duration_ms("auto").is_none());
        assert!(parse_duration_ms("").is_none());
    }

    #[test]
    fn normalize_curve_canonicalizes_spacing() {
        assert_eq!(
            normalize_curve("cubic-bezier(0.4,0,0.2,1)"),
            "cubic-bezier(0.4, 0, 0.2, 1)"
        );
        assert_eq!(normalize_curve("ease-in-out"), "ease-in-out");
    }

    #[test]
    fn extract_from_path_round_trips_dump() {
        let dump = dump_with(&[("transition-duration", &[("200ms", 1)])]);
        let path = std::env::temp_dir().join(format!(
            "forge-motion-{}",
            std::process::id()
        ));
        std::fs::write(&path, serde_json::to_string(&dump).unwrap()).unwrap();
        let r = extract_from_path(&path).unwrap();
        assert_eq!(r.transition_durations_ms.get(&200).copied(), Some(1));
        let _ = std::fs::remove_file(&path);
    }
}
