//! `reference_mapping` — extractor signals → SiteSpec mapper.
//!
//! Task #273 per the reference-matching arc. Consumes outputs
//! from all 9 per-axis extractors (#264-#272) plus the pattern
//! library catalog (#269) and emits a [`crate::synthesis::
//! SiteSpec`] ready for the synthesis backend (#291) to write.
//!
//! Per `docs/REFERENCE_MATCHING.md`: design principle is
//! OPINIONATED translation. SaaS tropes in the reference get
//! mapped to their editorial counterparts. Substrate-native
//! patterns pass through.
//!
//! ## Input
//!
//! [`ExtractedSignals`] — aggregated outputs from each axis
//! extractor at one viewport. The Crawler-side capture pipeline
//! (#263) emits per-axis dumps; the per-axis extractors produce
//! their typed results; this struct collects them.
//!
//! ## Output
//!
//! A [`crate::synthesis::SiteSpec`] — the synthesis backend
//! emits one cms/<slug>.json per declared page.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `#[non_exhaustive]` on every public type.
//! * No unwrap/expect in non-test code.
//! * Pure functions; no filesystem I/O.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::extractors::interactive::InteractiveResult;
use crate::extractors::motion::MotionResult;
use crate::extractors::palette::PaletteEntry;
use crate::extractors::sections::PatternClassification;
use crate::extractors::spacing::SpacingResult;
use crate::extractors::structural::StructuralResult;
use crate::extractors::typography::TypographyResult;
use crate::extractors::voice::VoiceResult;
use crate::synthesis::{SectionSpec, SiteSpec};

/// Aggregated extractor signals for one reference site at one
/// viewport.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExtractedSignals {
    /// Palette extractor output.
    pub palette: Vec<PaletteEntry>,
    /// Typography extractor output.
    pub typography: TypographyResult,
    /// Spacing extractor output.
    pub spacing: SpacingResult,
    /// Motion + decorative extractor output.
    pub motion: MotionResult,
    /// Per-page section classifications, keyed by page slug.
    pub sections_by_page: BTreeMap<String, Vec<PatternClassification>>,
    /// Structural extractor output.
    pub structural: StructuralResult,
    /// Voice extractor output.
    pub voice: VoiceResult,
    /// Interactive extractor output.
    pub interactive: InteractiveResult,
}

/// Map extractor signals → SiteSpec. Pure function.
#[must_use]
pub fn map_to_spec(site_id: &str, tenant_id: &str, signals: &ExtractedSignals) -> SiteSpec {
    let mut spec = SiteSpec::new(site_id, tenant_id)
        .with_voice(signals.voice.suggested_tier.clone())
        .with_mood(suggest_mood(signals))
        .with_density(suggest_density(signals));

    for (page_slug, classifications) in &signals.sections_by_page {
        let sections: Vec<SectionSpec> = classifications
            .iter()
            .map(|c| section_from_classification(c, signals))
            .collect();
        spec = spec.with_page(page_slug.clone(), sections);
    }

    spec
}

fn suggest_mood(signals: &ExtractedSignals) -> String {
    // Mood heuristic from motion + voice + palette signals.
    if signals.motion.has_animations || !signals.motion.transition_curves.is_empty() {
        if signals.motion.distinct_box_shadows >= 3 || signals.motion.has_gradients {
            return "kinetic".to_owned();
        }
    }
    // Heavy decorative treatments → playful or industrial.
    if signals.motion.border_radius_mode_px >= 16 && signals.motion.has_gradients {
        return "playful".to_owned();
    }
    if signals.motion.distinct_box_shadows == 0 && signals.motion.border_radius_mode_px <= 2 {
        // Very flat treatment → severe or minimal.
        if signals.typography.font_families.iter().any(|f| {
            f.stack.to_lowercase().contains("mono") || f.stack.to_lowercase().contains("monospace")
        }) {
            return "industrial".to_owned();
        }
        return "minimal".to_owned();
    }
    // Default for editorial-tier voice: editorial mood.
    match signals.voice.suggested_tier.as_str() {
        "editorial" | "professional" => "editorial".to_owned(),
        "technical" => "industrial".to_owned(),
        "academic" => "archival".to_owned(),
        _ => "editorial".to_owned(),
    }
}

fn suggest_density(signals: &ExtractedSignals) -> String {
    // Density from spacing rhythm.
    let rhythm = signals.spacing.rhythm_unit_px;
    let total_sections: usize = signals.sections_by_page.values().map(Vec::len).sum();

    match (rhythm, total_sections) {
        (0..=8, _) if total_sections >= 8 => "extreme".to_owned(),
        (0..=8, _) => "dense".to_owned(),
        (9..=16, n) if n >= 6 => "dense".to_owned(),
        (9..=16, _) => "comfortable".to_owned(),
        (17..=32, _) => "comfortable".to_owned(),
        _ => "sparse".to_owned(),
    }
}

fn section_from_classification(
    c: &PatternClassification,
    signals: &ExtractedSignals,
) -> SectionSpec {
    // Translate trope guesses into substrate-native counterparts
    // per docs/REFERENCE_MATCHING.md.
    let substrate_kind = translate_to_substrate(&c.guessed_kind);
    let variant = variant_hint(&substrate_kind, signals);
    let mut sec = SectionSpec::new(substrate_kind);
    if !variant.is_empty() {
        sec = sec.with_variant(variant);
    }
    sec
}

fn translate_to_substrate(guessed_kind: &str) -> String {
    // Per the pattern library catalog (#269) opinionated
    // translation table.
    match guessed_kind {
        // SaaS tropes → editorial counterparts.
        "feature_spotlight" => "kv_pair".to_owned(),
        "stat_band" => "sparkline".to_owned(),
        "testimonial" => "pull_quote".to_owned(),
        "marquee" => "kv_pair".to_owned(),
        "hero" => "hero_editorial".to_owned(),
        // Substrate-native: accept as-is.
        _ => guessed_kind.to_owned(),
    }
}

fn variant_hint(kind: &str, signals: &ExtractedSignals) -> String {
    // Suggest a variant based on signals. Today: kinetic mood
    // implies motion variants on certain primitives.
    let suggested_mood = suggest_mood(signals);
    match (kind, suggested_mood.as_str()) {
        ("hero_editorial", "editorial") => String::new(),
        ("hero_editorial", "industrial") => "visual=code".to_owned(),
        ("kv_pair", "kinetic") => "density=dense".to_owned(),
        ("kv_pair", _) => String::new(),
        ("call_to_action", "editorial" | "minimal" | "severe") => "editorial_solid".to_owned(),
        ("call_to_action", _) => String::new(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractors::sections::PatternClassification;

    fn classification(kind: &str) -> PatternClassification {
        PatternClassification {
            guessed_kind: kind.to_owned(),
            confidence: 80,
            feature_signature: String::new(),
        }
    }

    fn signals_with_voice(tier: &str) -> ExtractedSignals {
        let mut s = ExtractedSignals::default();
        s.voice.suggested_tier = tier.to_owned();
        s
    }

    #[test]
    fn map_carries_voice_tier_through_to_spec() {
        let signals = signals_with_voice("editorial");
        let spec = map_to_spec("test", "tenant", &signals);
        assert_eq!(spec.site_id, "test");
        assert_eq!(spec.tenant_id, "tenant");
        assert_eq!(spec.voice, "editorial");
    }

    #[test]
    fn map_emits_one_page_per_classification_group() {
        let mut signals = signals_with_voice("editorial");
        signals
            .sections_by_page
            .insert("index".to_owned(), vec![classification("hero_editorial")]);
        signals
            .sections_by_page
            .insert("about".to_owned(), vec![classification("paragraph")]);
        let spec = map_to_spec("s", "", &signals);
        assert_eq!(spec.pages.len(), 2);
        assert!(spec.pages.contains_key("index"));
        assert!(spec.pages.contains_key("about"));
    }

    #[test]
    fn translate_to_substrate_swaps_saas_tropes() {
        assert_eq!(translate_to_substrate("feature_spotlight"), "kv_pair");
        assert_eq!(translate_to_substrate("stat_band"), "sparkline");
        assert_eq!(translate_to_substrate("testimonial"), "pull_quote");
        assert_eq!(translate_to_substrate("marquee"), "kv_pair");
        assert_eq!(translate_to_substrate("hero"), "hero_editorial");
    }

    #[test]
    fn translate_to_substrate_passes_through_native_kinds() {
        for native in [
            "hero_editorial",
            "paragraph",
            "kv_pair",
            "pull_quote",
            "code",
            "image_hero",
            "section_heading",
            "call_to_action",
        ] {
            assert_eq!(translate_to_substrate(native), native);
        }
    }

    #[test]
    fn map_translates_trope_in_section_emission() {
        let mut signals = signals_with_voice("editorial");
        signals.sections_by_page.insert(
            "p".to_owned(),
            vec![
                classification("feature_spotlight"),
                classification("stat_band"),
                classification("testimonial"),
            ],
        );
        let spec = map_to_spec("s", "", &signals);
        let sections = spec.pages.get("p").unwrap();
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].kind, "kv_pair");
        assert_eq!(sections[1].kind, "sparkline");
        assert_eq!(sections[2].kind, "pull_quote");
    }

    #[test]
    fn suggest_density_dense_for_tight_rhythm() {
        let mut s = signals_with_voice("technical");
        s.spacing.rhythm_unit_px = 8;
        let spec = map_to_spec("s", "", &s);
        assert_eq!(spec.density, "dense");
    }

    #[test]
    fn suggest_density_comfortable_for_mid_rhythm() {
        let mut s = signals_with_voice("editorial");
        s.spacing.rhythm_unit_px = 16;
        let spec = map_to_spec("s", "", &s);
        assert_eq!(spec.density, "comfortable");
    }

    #[test]
    fn suggest_density_sparse_for_large_rhythm() {
        let mut s = signals_with_voice("editorial");
        s.spacing.rhythm_unit_px = 64;
        let spec = map_to_spec("s", "", &s);
        assert_eq!(spec.density, "sparse");
    }

    #[test]
    fn suggest_mood_industrial_when_monospace_present() {
        let mut s = signals_with_voice("technical");
        s.typography
            .font_families
            .push(crate::extractors::typography::FontFamilyEntry {
                stack: "JetBrains Mono, monospace".to_owned(),
                occurrence_count: 20,
            });
        // Flat treatment (no shadows, no border-radius).
        let spec = map_to_spec("s", "", &s);
        assert_eq!(spec.mood, "industrial");
    }

    #[test]
    fn suggest_mood_minimal_when_flat_treatment_no_mono() {
        let mut s = signals_with_voice("editorial");
        s.typography
            .font_families
            .push(crate::extractors::typography::FontFamilyEntry {
                stack: "Iowan Old Style, Georgia, serif".to_owned(),
                occurrence_count: 30,
            });
        let spec = map_to_spec("s", "", &s);
        assert_eq!(spec.mood, "minimal");
    }

    #[test]
    fn suggest_mood_kinetic_when_animations_plus_shadows() {
        let mut s = signals_with_voice("casual");
        s.motion.has_animations = true;
        s.motion.distinct_box_shadows = 5;
        let spec = map_to_spec("s", "", &s);
        assert_eq!(spec.mood, "kinetic");
    }

    #[test]
    fn map_returns_empty_pages_when_no_classifications() {
        let signals = ExtractedSignals::default();
        let spec = map_to_spec("s", "", &signals);
        assert!(spec.pages.is_empty());
    }

    #[test]
    fn variant_hint_kinetic_kv_pair_dense() {
        let mut s = ExtractedSignals::default();
        s.motion.has_animations = true;
        s.motion.distinct_box_shadows = 5;
        assert_eq!(variant_hint("kv_pair", &s), "density=dense");
    }
}
