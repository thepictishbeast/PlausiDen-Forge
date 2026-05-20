//! `reference_composition` — multi-reference weighted blend.
//!
//! Task #274 per the reference-matching arc. Where
//! `reference_mapping::map_to_spec` consumes ONE reference site's
//! extractor outputs and emits a SiteSpec, this module composes
//! MULTIPLE references into one SiteSpec via per-axis weighted
//! blending.
//!
//! ## Per-axis blend rules
//!
//! | Axis        | Strategy                                                   |
//! |-------------|------------------------------------------------------------|
//! | Palette     | Weighted-sum occurrence counts; top-N entries by total     |
//! | Typography  | Dominant choice wins (weighted-max occurrence)             |
//! | Spacing     | Median rhythm + weighted-avg max-width                     |
//! | Motion      | Union of treatments; flags = any-set                       |
//! | Sections    | Union per page-slug (with dedup by guessed_kind sequence)  |
//! | Structural  | Sum nav items + page distribution                          |
//! | Voice       | Suggested-tier closest to operator-declared (or first ref) |
//! | Interactive | Union of hover treatments                                  |
//!
//! Each reference carries a weight 0.0–1.0 (default 1.0). Weights
//! that don't sum to 1.0 are normalized.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `#[non_exhaustive]` on every public type.
//! * No unwrap/expect in non-test code.
//! * Pure functions; no I/O.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::extractors::interactive::HoverTreatmentEntry;
use crate::extractors::palette::PaletteEntry;
use crate::extractors::typography::FontFamilyEntry;
use crate::reference_mapping::{map_to_spec, ExtractedSignals};
use crate::synthesis::SiteSpec;

/// One reference + its weight in the composition.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct WeightedReference {
    /// Operator-supplied label (e.g. site slug).
    pub label: String,
    /// Weight 0.0–1.0. Normalized across all references.
    pub weight: f64,
    /// Per-axis extractor outputs.
    pub signals: ExtractedSignals,
}

impl WeightedReference {
    /// Construct with weight 1.0.
    #[must_use]
    pub fn new(label: impl Into<String>, signals: ExtractedSignals) -> Self {
        Self {
            label: label.into(),
            weight: 1.0,
            signals,
        }
    }

    /// Set the weight.
    #[must_use]
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight.max(0.0);
        self
    }
}

/// Aggregate blend result the engine emits before handing to
/// `reference_mapping::map_to_spec`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BlendedSignals {
    /// Blended palette: occurrence_count is the weighted sum.
    pub palette: Vec<PaletteEntry>,
    /// Top-N font families by weighted occurrence.
    pub font_families: Vec<FontFamilyEntry>,
    /// Weighted-median rhythm.
    pub rhythm_unit_px: u32,
    /// Weighted-average max-width.
    pub content_max_width_px: u32,
    /// Any-set motion treatments.
    pub has_animations: bool,
    /// Any-set scroll triggers.
    pub has_scroll_triggers: bool,
    /// Any-set gradient.
    pub has_gradients: bool,
    /// Any-set filter.
    pub has_filters: bool,
    /// Sum of distinct box-shadow counts.
    pub total_distinct_box_shadows: u32,
    /// Union of hover treatments by sum-of-counts.
    pub hover_treatments: Vec<HoverTreatmentEntry>,
    /// Per-page section sequences (union by page slug; later refs
    /// supplement, don't overwrite).
    pub sections_by_page: BTreeMap<String, Vec<crate::extractors::sections::PatternClassification>>,
    /// Tier slug picked from the highest-weight reference's
    /// suggested_tier.
    pub suggested_voice_tier: String,
}

/// Compose multiple references into one SiteSpec.
///
/// Each reference's signals are blended via per-axis rules; the
/// result is then fed through `map_to_spec` to produce the final
/// SiteSpec.
#[must_use]
pub fn compose_multi(site_id: &str, tenant_id: &str, refs: &[WeightedReference]) -> SiteSpec {
    if refs.is_empty() {
        return SiteSpec::new(site_id, tenant_id);
    }

    let blended = blend_signals(refs);

    // Project blended back into ExtractedSignals so we can reuse
    // map_to_spec.
    let mut signals = ExtractedSignals::default();
    signals.palette = blended.palette;
    signals.typography.font_families = blended.font_families;
    signals.spacing.rhythm_unit_px = blended.rhythm_unit_px;
    signals.spacing.content_max_width_px = blended.content_max_width_px;
    signals.motion.has_animations = blended.has_animations;
    signals.motion.has_scroll_triggers = blended.has_scroll_triggers;
    signals.motion.has_gradients = blended.has_gradients;
    signals.motion.has_filters = blended.has_filters;
    signals.motion.distinct_box_shadows = blended.total_distinct_box_shadows;
    signals.interactive.hover_treatments = blended.hover_treatments.clone();
    signals.interactive.has_hover_states = !blended.hover_treatments.is_empty();
    signals.sections_by_page = blended.sections_by_page;
    signals.voice.suggested_tier = blended.suggested_voice_tier;

    map_to_spec(site_id, tenant_id, &signals)
}

fn blend_signals(refs: &[WeightedReference]) -> BlendedSignals {
    let total_weight: f64 = refs.iter().map(|r| r.weight).sum();
    let normalize = if total_weight > 0.0 {
        total_weight
    } else {
        1.0
    };

    let mut palette_counts: BTreeMap<String, (PaletteEntry, f64)> = BTreeMap::new();
    let mut font_counts: BTreeMap<String, f64> = BTreeMap::new();
    let mut rhythm_samples: Vec<(u32, f64)> = Vec::new();
    let mut max_width_weighted: f64 = 0.0;
    let mut max_width_total_weight: f64 = 0.0;
    let mut has_animations = false;
    let mut has_scroll_triggers = false;
    let mut has_gradients = false;
    let mut has_filters = false;
    let mut total_box_shadows: u32 = 0;
    let mut hover_counts: BTreeMap<crate::extractors::interactive::HoverTreatment, u32> =
        BTreeMap::new();
    let mut sections_by_page: BTreeMap<
        String,
        Vec<crate::extractors::sections::PatternClassification>,
    > = BTreeMap::new();

    // Highest-weight ref drives the voice tier.
    let mut top_voice: (f64, &str) = (-1.0, "");

    for r in refs {
        let w = r.weight / normalize;

        for p in &r.signals.palette {
            let entry = palette_counts
                .entry(p.hex.clone())
                .or_insert_with(|| (p.clone(), 0.0));
            entry.1 += f64::from(p.occurrence_count) * w;
        }
        for f in &r.signals.typography.font_families {
            *font_counts.entry(f.stack.clone()).or_insert(0.0) += f64::from(f.occurrence_count) * w;
        }
        if r.signals.spacing.rhythm_unit_px > 0 {
            rhythm_samples.push((r.signals.spacing.rhythm_unit_px, w));
        }
        if r.signals.spacing.content_max_width_px > 0 {
            max_width_weighted += f64::from(r.signals.spacing.content_max_width_px) * w;
            max_width_total_weight += w;
        }
        has_animations |= r.signals.motion.has_animations;
        has_scroll_triggers |= r.signals.motion.has_scroll_triggers;
        has_gradients |= r.signals.motion.has_gradients;
        has_filters |= r.signals.motion.has_filters;
        total_box_shadows = total_box_shadows.saturating_add(r.signals.motion.distinct_box_shadows);
        for entry in &r.signals.interactive.hover_treatments {
            *hover_counts.entry(entry.treatment).or_insert(0) += entry.occurrence_count;
        }
        for (page, classifications) in &r.signals.sections_by_page {
            let slot = sections_by_page.entry(page.clone()).or_default();
            if slot.is_empty() {
                slot.extend(classifications.iter().cloned());
            }
        }
        if r.weight > top_voice.0 && !r.signals.voice.suggested_tier.is_empty() {
            top_voice = (r.weight, r.signals.voice.suggested_tier.as_str());
        }
    }

    let mut palette: Vec<PaletteEntry> = palette_counts
        .into_iter()
        .map(|(_, (mut entry, weighted_count))| {
            entry.occurrence_count = weighted_count.round().max(0.0) as u32;
            entry
        })
        .collect();
    palette.sort_by(|a, b| {
        b.occurrence_count
            .cmp(&a.occurrence_count)
            .then_with(|| a.hex.cmp(&b.hex))
    });

    let mut font_families: Vec<FontFamilyEntry> = font_counts
        .into_iter()
        .map(|(stack, weighted)| FontFamilyEntry {
            stack,
            occurrence_count: weighted.round().max(0.0) as u32,
        })
        .collect();
    font_families.sort_by(|a, b| {
        b.occurrence_count
            .cmp(&a.occurrence_count)
            .then_with(|| a.stack.cmp(&b.stack))
    });

    let rhythm_unit_px = weighted_median(&rhythm_samples);
    let content_max_width_px = if max_width_total_weight > 0.0 {
        (max_width_weighted / max_width_total_weight).round() as u32
    } else {
        0
    };

    let mut hover_treatments: Vec<HoverTreatmentEntry> = hover_counts
        .into_iter()
        .map(|(treatment, occurrence_count)| HoverTreatmentEntry {
            treatment,
            occurrence_count,
        })
        .collect();
    hover_treatments.sort_by(|a, b| b.occurrence_count.cmp(&a.occurrence_count));

    BlendedSignals {
        palette,
        font_families,
        rhythm_unit_px,
        content_max_width_px,
        has_animations,
        has_scroll_triggers,
        has_gradients,
        has_filters,
        total_distinct_box_shadows: total_box_shadows,
        hover_treatments,
        sections_by_page,
        suggested_voice_tier: top_voice.1.to_owned(),
    }
}

fn weighted_median(samples: &[(u32, f64)]) -> u32 {
    if samples.is_empty() {
        return 0;
    }
    let mut sorted: Vec<(u32, f64)> = samples.iter().copied().collect();
    sorted.sort_by_key(|(v, _)| *v);
    let total_weight: f64 = sorted.iter().map(|(_, w)| *w).sum();
    let target = total_weight / 2.0;
    let mut cumulative = 0.0;
    for (v, w) in &sorted {
        cumulative += w;
        if cumulative >= target {
            return *v;
        }
    }
    sorted.last().map(|(v, _)| *v).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractors::interactive::{HoverTreatment, HoverTreatmentEntry, InteractiveResult};
    use crate::extractors::sections::PatternClassification;
    use crate::extractors::typography::FontFamilyEntry;

    fn signals_with_voice(tier: &str) -> ExtractedSignals {
        let mut s = ExtractedSignals::default();
        s.voice.suggested_tier = tier.to_owned();
        s
    }

    #[test]
    fn compose_empty_refs_returns_empty_spec() {
        let spec = compose_multi("s", "t", &[]);
        assert_eq!(spec.site_id, "s");
        assert!(spec.pages.is_empty());
    }

    #[test]
    fn compose_single_ref_passes_through_voice() {
        let r = WeightedReference::new("a", signals_with_voice("editorial"));
        let spec = compose_multi("s", "t", &[r]);
        assert_eq!(spec.voice, "editorial");
    }

    #[test]
    fn compose_uses_highest_weight_voice_tier() {
        let r1 = WeightedReference::new("a", signals_with_voice("plain")).with_weight(0.2);
        let r2 = WeightedReference::new("b", signals_with_voice("editorial")).with_weight(0.8);
        let spec = compose_multi("s", "t", &[r1, r2]);
        assert_eq!(spec.voice, "editorial");
    }

    #[test]
    fn compose_blends_palette_via_weighted_occurrence() {
        let mut s1 = ExtractedSignals::default();
        s1.palette.push(PaletteEntry {
            hex: "#000000".into(),
            rgb: [0, 0, 0],
            occurrence_count: 10,
            contrast_class: crate::extractors::palette::ContrastClass::Dark,
            source_properties: vec!["color".into()],
        });
        let mut s2 = ExtractedSignals::default();
        s2.palette.push(PaletteEntry {
            hex: "#000000".into(),
            rgb: [0, 0, 0],
            occurrence_count: 20,
            contrast_class: crate::extractors::palette::ContrastClass::Dark,
            source_properties: vec!["color".into()],
        });
        let refs = vec![
            WeightedReference::new("a", s1).with_weight(0.5),
            WeightedReference::new("b", s2).with_weight(0.5),
        ];
        // Run blend directly to inspect.
        let blended = blend_signals(&refs);
        assert_eq!(blended.palette.len(), 1);
        // 10*0.5 + 20*0.5 = 15
        assert_eq!(blended.palette[0].occurrence_count, 15);
    }

    #[test]
    fn compose_blends_fonts_picks_top_by_occurrence() {
        let mut s1 = ExtractedSignals::default();
        s1.typography.font_families.push(FontFamilyEntry {
            stack: "Inter".into(),
            occurrence_count: 50,
        });
        let mut s2 = ExtractedSignals::default();
        s2.typography.font_families.push(FontFamilyEntry {
            stack: "Iowan Old Style".into(),
            occurrence_count: 80,
        });
        let refs = vec![
            WeightedReference::new("a", s1).with_weight(0.5),
            WeightedReference::new("b", s2).with_weight(0.5),
        ];
        let blended = blend_signals(&refs);
        // 80*0.5 > 50*0.5; Iowan comes first.
        assert_eq!(blended.font_families[0].stack, "Iowan Old Style");
    }

    #[test]
    fn compose_rhythm_uses_weighted_median() {
        let mut s1 = ExtractedSignals::default();
        s1.spacing.rhythm_unit_px = 8;
        let mut s2 = ExtractedSignals::default();
        s2.spacing.rhythm_unit_px = 16;
        let refs = vec![
            WeightedReference::new("a", s1).with_weight(0.3),
            WeightedReference::new("b", s2).with_weight(0.7),
        ];
        let blended = blend_signals(&refs);
        // 0.3 + 0.7 = 1.0, median at 0.5 weight; cumulative 0.3
        // (8) doesn't hit, 0.3+0.7=1.0 (16) hits. Median = 16.
        assert_eq!(blended.rhythm_unit_px, 16);
    }

    #[test]
    fn compose_motion_flags_are_any_set() {
        let mut s1 = ExtractedSignals::default();
        s1.motion.has_animations = false;
        s1.motion.has_gradients = true;
        let mut s2 = ExtractedSignals::default();
        s2.motion.has_animations = true;
        s2.motion.has_gradients = false;
        let refs = vec![
            WeightedReference::new("a", s1),
            WeightedReference::new("b", s2),
        ];
        let b = blend_signals(&refs);
        assert!(b.has_animations);
        assert!(b.has_gradients);
    }

    #[test]
    fn compose_box_shadows_sum_across_refs() {
        let mut s1 = ExtractedSignals::default();
        s1.motion.distinct_box_shadows = 3;
        let mut s2 = ExtractedSignals::default();
        s2.motion.distinct_box_shadows = 5;
        let refs = vec![
            WeightedReference::new("a", s1),
            WeightedReference::new("b", s2),
        ];
        let b = blend_signals(&refs);
        assert_eq!(b.total_distinct_box_shadows, 8);
    }

    #[test]
    fn compose_hover_treatments_union_with_summed_counts() {
        let mut s1 = ExtractedSignals::default();
        s1.interactive.hover_treatments.push(HoverTreatmentEntry {
            treatment: HoverTreatment::ColorShift,
            occurrence_count: 5,
        });
        let mut s2 = ExtractedSignals::default();
        s2.interactive.hover_treatments.push(HoverTreatmentEntry {
            treatment: HoverTreatment::ColorShift,
            occurrence_count: 7,
        });
        s2.interactive.hover_treatments.push(HoverTreatmentEntry {
            treatment: HoverTreatment::Transform,
            occurrence_count: 2,
        });
        let refs = vec![
            WeightedReference::new("a", s1),
            WeightedReference::new("b", s2),
        ];
        let b = blend_signals(&refs);
        assert_eq!(b.hover_treatments.len(), 2);
        let color = b
            .hover_treatments
            .iter()
            .find(|e| e.treatment == HoverTreatment::ColorShift)
            .unwrap();
        assert_eq!(color.occurrence_count, 12);
    }

    #[test]
    fn compose_sections_first_ref_wins_per_page() {
        let mut s1 = ExtractedSignals::default();
        s1.sections_by_page.insert(
            "index".to_owned(),
            vec![PatternClassification {
                guessed_kind: "hero_editorial".to_owned(),
                confidence: 80,
                feature_signature: String::new(),
            }],
        );
        let mut s2 = ExtractedSignals::default();
        s2.sections_by_page.insert(
            "index".to_owned(),
            vec![PatternClassification {
                guessed_kind: "split_hero".to_owned(),
                confidence: 80,
                feature_signature: String::new(),
            }],
        );
        let refs = vec![
            WeightedReference::new("a", s1),
            WeightedReference::new("b", s2),
        ];
        let b = blend_signals(&refs);
        // First ref's sections take the slot; later refs don't
        // overwrite (intentional — operator picks the primary).
        assert_eq!(
            b.sections_by_page.get("index").unwrap()[0].guessed_kind,
            "hero_editorial"
        );
    }

    #[test]
    fn weighted_median_handles_empty() {
        assert_eq!(weighted_median(&[]), 0);
    }

    #[test]
    fn weighted_median_picks_single() {
        assert_eq!(weighted_median(&[(8, 1.0)]), 8);
    }

    #[test]
    fn weight_normalization_handles_unnormalized_inputs() {
        let mut s1 = ExtractedSignals::default();
        s1.spacing.rhythm_unit_px = 8;
        let mut s2 = ExtractedSignals::default();
        s2.spacing.rhythm_unit_px = 16;
        let mut s3 = ExtractedSignals::default();
        s3.spacing.rhythm_unit_px = 24;
        // Weights sum to 100, not 1.0 — should still work.
        // weighted_median with 33/33/33 → cumulative reaches
        // 50 at the 16 sample → median 16.
        let refs = vec![
            WeightedReference::new("a", s1).with_weight(33.0),
            WeightedReference::new("b", s2).with_weight(33.0),
            WeightedReference::new("c", s3).with_weight(33.0),
        ];
        let b = blend_signals(&refs);
        assert_eq!(b.rhythm_unit_px, 16);
    }
}
