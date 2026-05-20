//! `interactive` — hover / form / transition decision extraction.
//!
//! Task #272 per the reference-matching arc. Reads an
//! [`InteractiveDump`] (Crawler-emitted per-element interactive
//! signal) and emits the interactive signal: hover treatments,
//! form-field kind distribution, focus-style presence,
//! transition-property distribution.
//!
//! Closes the per-axis extractor cascade (#264-#272).
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `#[non_exhaustive]` on every public type.
//! * No unwrap/expect in non-test code.
//! * Pure functions over in-memory dump.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::extractors::ExtractorError;

/// Interactive-dump spec version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InteractiveSpec {
    /// Initial spec.
    #[default]
    V1,
}

impl InteractiveSpec {
    /// Stable kebab-case slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::V1 => "v1",
        }
    }
}

/// Per-element hover pair: the same property's base value vs
/// :hover value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HoverPair {
    /// CSS property name.
    pub property: String,
    /// Computed value at rest.
    pub base_value: String,
    /// Computed value at :hover.
    pub hover_value: String,
}

/// Crawler-emitted interactive dump.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InteractiveDump {
    /// Schema version.
    pub spec: InteractiveSpec,
    /// All hover-pair samples observed across `<button>`,
    /// `<a>`, interactive `[role]` elements.
    pub hover_pairs: Vec<HoverPair>,
    /// Form input type distribution (`text`/`email`/`tel`/
    /// `password`/`number`/`search`/`textarea`/`select`/...).
    pub form_field_kinds: BTreeMap<String, u32>,
    /// True iff any element has `outline:` or `box-shadow` change
    /// on `:focus-visible`.
    pub has_focus_visible_styles: bool,
    /// `transition-property` value distribution observed across
    /// interactive elements.
    pub transition_property_distribution: BTreeMap<String, u32>,
}

/// Aggregate interactive result.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InteractiveResult {
    /// Hover treatment frequencies, sorted by count desc.
    pub hover_treatments: Vec<HoverTreatmentEntry>,
    /// Pass-through of form field kind distribution.
    pub form_field_kinds: BTreeMap<String, u32>,
    /// Pass-through of focus-visible-styles presence.
    pub has_focus_visible_styles: bool,
    /// Pass-through of transition-property distribution.
    pub transition_property_distribution: BTreeMap<String, u32>,
    /// True iff hover_treatments contains any treatment.
    pub has_hover_states: bool,
}

/// One hover treatment classification + count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HoverTreatmentEntry {
    /// Treatment slug.
    pub treatment: HoverTreatment,
    /// Number of HoverPair observations classified as this
    /// treatment.
    pub occurrence_count: u32,
}

/// Categorical hover treatment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum HoverTreatment {
    /// Color value changes on hover.
    ColorShift,
    /// Background-color value changes on hover.
    BackgroundShift,
    /// Underline / text-decoration appears or changes.
    Underline,
    /// Transform applied (scale / rotate / translate).
    Transform,
    /// Box-shadow appears or grows.
    Shadow,
    /// Opacity changes.
    OpacityShift,
    /// No treatment detected (values identical).
    NoChange,
    /// Recognized property change that doesn't fit other slots.
    Other,
}

/// Extract from a JSON dump file.
pub fn extract_from_path(path: &Path) -> Result<InteractiveResult, ExtractorError> {
    let body = std::fs::read_to_string(path)?;
    let dump: InteractiveDump = serde_json::from_str(&body)?;
    Ok(extract(&dump))
}

/// Pure extraction over in-memory dump.
#[must_use]
pub fn extract(dump: &InteractiveDump) -> InteractiveResult {
    let mut counts: BTreeMap<HoverTreatment, u32> = BTreeMap::new();
    for pair in &dump.hover_pairs {
        let treatment = classify(pair);
        *counts.entry(treatment).or_insert(0) += 1;
    }
    let mut hover_treatments: Vec<HoverTreatmentEntry> = counts
        .into_iter()
        .filter(|(t, _)| *t != HoverTreatment::NoChange)
        .map(|(treatment, occurrence_count)| HoverTreatmentEntry {
            treatment,
            occurrence_count,
        })
        .collect();
    hover_treatments.sort_by(|a, b| {
        b.occurrence_count
            .cmp(&a.occurrence_count)
            .then_with(|| (a.treatment as u8).cmp(&(b.treatment as u8)))
    });

    let has_hover_states = !hover_treatments.is_empty();

    InteractiveResult {
        hover_treatments,
        form_field_kinds: dump.form_field_kinds.clone(),
        has_focus_visible_styles: dump.has_focus_visible_styles,
        transition_property_distribution: dump.transition_property_distribution.clone(),
        has_hover_states,
    }
}

fn classify(pair: &HoverPair) -> HoverTreatment {
    if pair.base_value.trim() == pair.hover_value.trim() {
        return HoverTreatment::NoChange;
    }
    let prop = pair.property.trim().to_ascii_lowercase();
    match prop.as_str() {
        "color" => HoverTreatment::ColorShift,
        "background-color" | "background" => HoverTreatment::BackgroundShift,
        "text-decoration" | "text-decoration-line" => HoverTreatment::Underline,
        "transform" => HoverTreatment::Transform,
        "box-shadow" => HoverTreatment::Shadow,
        "opacity" => HoverTreatment::OpacityShift,
        _ => HoverTreatment::Other,
    }
}

// Implement Ord for HoverTreatment so BTreeMap orders entries
// deterministically.
impl PartialOrd for HoverTreatment {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HoverTreatment {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(prop: &str, base: &str, hover: &str) -> HoverPair {
        HoverPair {
            property: prop.to_owned(),
            base_value: base.to_owned(),
            hover_value: hover.to_owned(),
        }
    }

    #[test]
    fn classify_no_change_when_values_equal() {
        let p = pair("color", "rgb(0, 0, 0)", "rgb(0, 0, 0)");
        assert_eq!(classify(&p), HoverTreatment::NoChange);
    }

    #[test]
    fn classify_color_shift() {
        let p = pair("color", "rgb(0, 0, 0)", "rgb(91, 62, 155)");
        assert_eq!(classify(&p), HoverTreatment::ColorShift);
    }

    #[test]
    fn classify_background_shift() {
        let p = pair("background-color", "rgba(0,0,0,0)", "rgb(245,245,245)");
        assert_eq!(classify(&p), HoverTreatment::BackgroundShift);
    }

    #[test]
    fn classify_underline() {
        let p = pair("text-decoration", "none", "underline");
        assert_eq!(classify(&p), HoverTreatment::Underline);
    }

    #[test]
    fn classify_transform() {
        let p = pair("transform", "none", "scale(1.05)");
        assert_eq!(classify(&p), HoverTreatment::Transform);
    }

    #[test]
    fn classify_shadow() {
        let p = pair("box-shadow", "none", "0 4px 8px rgba(0,0,0,0.1)");
        assert_eq!(classify(&p), HoverTreatment::Shadow);
    }

    #[test]
    fn classify_opacity_shift() {
        let p = pair("opacity", "1", "0.85");
        assert_eq!(classify(&p), HoverTreatment::OpacityShift);
    }

    #[test]
    fn classify_other_for_unknown_property() {
        let p = pair("border-radius", "0px", "4px");
        assert_eq!(classify(&p), HoverTreatment::Other);
    }

    #[test]
    fn extract_drops_no_change_entries() {
        let dump = InteractiveDump {
            spec: InteractiveSpec::V1,
            hover_pairs: vec![
                pair("color", "rgb(0,0,0)", "rgb(0,0,0)"),
                pair("color", "rgb(0,0,0)", "rgb(255,0,0)"),
            ],
            form_field_kinds: BTreeMap::new(),
            has_focus_visible_styles: false,
            transition_property_distribution: BTreeMap::new(),
        };
        let r = extract(&dump);
        assert_eq!(r.hover_treatments.len(), 1);
        assert_eq!(r.hover_treatments[0].treatment, HoverTreatment::ColorShift);
        assert_eq!(r.hover_treatments[0].occurrence_count, 1);
        assert!(r.has_hover_states);
    }

    #[test]
    fn extract_sorts_hover_treatments_by_occurrence_desc() {
        let mut dump = InteractiveDump::default();
        dump.spec = InteractiveSpec::V1;
        for _ in 0..5 {
            dump.hover_pairs
                .push(pair("background-color", "white", "gray"));
        }
        for _ in 0..2 {
            dump.hover_pairs
                .push(pair("transform", "none", "scale(1.05)"));
        }
        for _ in 0..8 {
            dump.hover_pairs.push(pair("color", "black", "purple"));
        }
        let r = extract(&dump);
        assert_eq!(r.hover_treatments.len(), 3);
        assert_eq!(r.hover_treatments[0].treatment, HoverTreatment::ColorShift);
        assert_eq!(r.hover_treatments[0].occurrence_count, 8);
        assert_eq!(r.hover_treatments[1].treatment, HoverTreatment::BackgroundShift);
        assert_eq!(r.hover_treatments[1].occurrence_count, 5);
        assert_eq!(r.hover_treatments[2].treatment, HoverTreatment::Transform);
        assert_eq!(r.hover_treatments[2].occurrence_count, 2);
    }

    #[test]
    fn extract_returns_no_hover_states_when_all_unchanged() {
        let mut dump = InteractiveDump::default();
        dump.spec = InteractiveSpec::V1;
        dump.hover_pairs
            .push(pair("color", "rgb(0,0,0)", "rgb(0,0,0)"));
        let r = extract(&dump);
        assert!(!r.has_hover_states);
        assert!(r.hover_treatments.is_empty());
    }

    #[test]
    fn extract_passes_through_form_field_kinds_and_focus_flag() {
        let mut dump = InteractiveDump::default();
        dump.spec = InteractiveSpec::V1;
        dump.form_field_kinds.insert("text".to_owned(), 3);
        dump.form_field_kinds.insert("email".to_owned(), 1);
        dump.has_focus_visible_styles = true;
        let r = extract(&dump);
        assert_eq!(r.form_field_kinds.get("text").copied(), Some(3));
        assert_eq!(r.form_field_kinds.get("email").copied(), Some(1));
        assert!(r.has_focus_visible_styles);
    }

    #[test]
    fn extract_passes_through_transition_property_distribution() {
        let mut dump = InteractiveDump::default();
        dump.spec = InteractiveSpec::V1;
        dump.transition_property_distribution
            .insert("color".to_owned(), 5);
        dump.transition_property_distribution
            .insert("background-color".to_owned(), 3);
        let r = extract(&dump);
        assert_eq!(r.transition_property_distribution.get("color").copied(), Some(5));
        assert_eq!(
            r.transition_property_distribution
                .get("background-color")
                .copied(),
            Some(3)
        );
    }

    #[test]
    fn extract_from_path_round_trips_dump() {
        let mut dump = InteractiveDump::default();
        dump.spec = InteractiveSpec::V1;
        dump.hover_pairs
            .push(pair("color", "black", "purple"));
        let path = std::env::temp_dir().join(format!(
            "forge-interactive-{}",
            std::process::id()
        ));
        std::fs::write(&path, serde_json::to_string(&dump).unwrap()).unwrap();
        let r = extract_from_path(&path).unwrap();
        assert_eq!(r.hover_treatments.len(), 1);
        let _ = std::fs::remove_file(&path);
    }
}
