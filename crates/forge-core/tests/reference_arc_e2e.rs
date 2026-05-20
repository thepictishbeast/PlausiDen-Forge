//! End-to-end reference-matching arc test.
//!
//! Exercises the full code path: synthetic extractor inputs →
//! reference_mapping / reference_composition → synthesis →
//! cms/<slug>.json on disk. Failure here is a cross-module
//! integration regression (modules drifted out of sync).

use std::collections::BTreeMap;
use std::fs;

use forge_core::extractors::interactive::{HoverTreatment, HoverTreatmentEntry, InteractiveResult};
use forge_core::extractors::motion::MotionResult;
use forge_core::extractors::palette::{ContrastClass, PaletteEntry};
use forge_core::extractors::sections::PatternClassification;
use forge_core::extractors::spacing::SpacingResult;
use forge_core::extractors::structural::{NavShape, StructuralResult};
use forge_core::extractors::typography::{FontFamilyEntry, TypographyResult};
use forge_core::extractors::voice::VoiceResult;
use forge_core::reference_composition::{compose_multi, WeightedReference};
use forge_core::reference_mapping::{map_to_spec, ExtractedSignals};
use forge_core::synthesis::synthesize;

fn editorial_signals_for_prosperity() -> ExtractedSignals {
    let mut s = ExtractedSignals::default();
    s.palette = vec![
        PaletteEntry::new(
            "#15140f",
            [21, 20, 15],
            200,
            ContrastClass::Dark,
            vec!["color".into()],
        ),
        PaletteEntry::new(
            "#f6f5f0",
            [246, 245, 240],
            180,
            ContrastClass::Light,
            vec!["background-color".into()],
        ),
    ];
    s.typography = TypographyResult::new(
        vec![FontFamilyEntry::new("Iowan Old Style, Georgia, serif", 80)],
        BTreeMap::from([(16u32, 50u32), (32u32, 8u32)]),
        vec![400, 600],
        1.55,
    );
    s.spacing.rhythm_unit_px = 16;
    s.spacing.content_max_width_px = 760;
    s.motion.has_animations = false;
    s.motion.has_gradients = false;
    s.motion.border_radius_mode_px = 2;
    s.motion.distinct_box_shadows = 0;
    s.voice = VoiceResult::new(100, 1800, 9000, 18.0, 16, 30, 0.45, 4, 2200, "editorial");
    s.interactive = InteractiveResult::new(
        vec![HoverTreatmentEntry::new(HoverTreatment::ColorShift, 30)],
        BTreeMap::new(),
        true,
        BTreeMap::new(),
        true,
    );

    s.sections_by_page.insert(
        "index".into(),
        vec![
            PatternClassification::new("hero_editorial", 80, "above-fold + heading + lede"),
            PatternClassification::new("paragraph", 75, "lede"),
            PatternClassification::new("pull_quote", 80, "blockquote"),
            PatternClassification::new("kv_pair", 70, "list"),
            PatternClassification::new("call_to_action", 75, "button"),
        ],
    );
    s
}

#[test]
fn arc_single_reference_round_trip_emits_clean_cms() {
    let signals = editorial_signals_for_prosperity();
    let spec = map_to_spec("prosperityclub-test", "plausiden", &signals);

    assert_eq!(spec.site_id, "prosperityclub-test");
    assert_eq!(spec.voice, "editorial");
    assert!(matches!(spec.mood.as_str(), "minimal" | "editorial"));
    assert_eq!(spec.density, "comfortable");
    assert!(spec.pages.contains_key("index"));
    assert_eq!(spec.pages["index"].len(), 5);
    assert_eq!(spec.pages["index"][0].kind, "hero_editorial");
    assert_eq!(spec.pages["index"][1].kind, "paragraph");
    assert_eq!(spec.pages["index"][2].kind, "pull_quote");
    assert_eq!(spec.pages["index"][3].kind, "kv_pair");
    assert_eq!(spec.pages["index"][4].kind, "call_to_action");

    let dir = std::env::temp_dir().join(format!("forge-arc-e2e-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let written = synthesize(&spec, &dir).unwrap();
    assert_eq!(written.len(), 1);
    let body = fs::read_to_string(&written[0]).unwrap();
    assert!(body.contains("\"voice_tier\": \"editorial\""));
    assert!(body.contains("\"kind\": \"hero_editorial\""));
    assert!(body.contains("\"kind\": \"pull_quote\""));
    assert!(body.contains("\"kind\": \"kv_pair\""));
    assert!(body.contains("\"kind\": \"call_to_action\""));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn arc_translates_saas_trope_classifications_to_editorial_counterparts() {
    let mut signals = editorial_signals_for_prosperity();
    signals.sections_by_page.insert(
        "index".into(),
        vec![
            PatternClassification::new("hero", 70, ""),
            PatternClassification::new("feature_spotlight", 70, ""),
            PatternClassification::new("stat_band", 70, ""),
            PatternClassification::new("testimonial", 70, ""),
            PatternClassification::new("marquee", 70, ""),
        ],
    );
    let spec = map_to_spec("test", "", &signals);
    let sections = &spec.pages["index"];
    assert_eq!(sections[0].kind, "hero_editorial");
    assert_eq!(sections[1].kind, "kv_pair");
    assert_eq!(sections[2].kind, "sparkline");
    assert_eq!(sections[3].kind, "pull_quote");
    assert_eq!(sections[4].kind, "kv_pair");
}

#[test]
fn arc_multi_reference_composition_round_trip() {
    let editorial = editorial_signals_for_prosperity();

    let mut technical = ExtractedSignals::default();
    technical.voice = VoiceResult::new(0, 0, 0, 0.0, 0, 0, 0.0, 0, 0, "technical");
    technical.spacing.rhythm_unit_px = 8;
    technical.motion.has_animations = true;
    technical.motion.distinct_box_shadows = 4;
    technical.typography = TypographyResult::new(
        vec![FontFamilyEntry::new("JetBrains Mono, monospace", 40)],
        BTreeMap::new(),
        vec![400],
        1.4,
    );
    technical.sections_by_page.insert(
        "docs".into(),
        vec![PatternClassification::new("code", 80, "code block")],
    );

    let refs = vec![
        WeightedReference::new("editorial", editorial).with_weight(0.7),
        WeightedReference::new("technical", technical).with_weight(0.3),
    ];

    let spec = compose_multi("blended-site", "tenant", &refs);
    assert_eq!(spec.voice, "editorial");
    assert!(spec.pages.contains_key("index"));
    assert!(spec.pages.contains_key("docs"));

    let dir = std::env::temp_dir().join(format!("forge-arc-multi-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let written = synthesize(&spec, &dir).unwrap();
    assert_eq!(written.len(), 2);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn arc_constructors_cover_every_extractor_result_type() {
    // Exercise every extractor's pub fn new() to verify the
    // constructors stay in sync with the structs they build.
    // If any field is added to a #[non_exhaustive] struct, this
    // test breaks at the call site — forcing a sync update.
    let _palette = PaletteEntry::new(
        "#000000",
        [0, 0, 0],
        10,
        ContrastClass::Dark,
        vec!["color".into()],
    );
    let _typography = TypographyResult::new(
        vec![FontFamilyEntry::new("Inter", 5)],
        BTreeMap::from([(16u32, 1u32)]),
        vec![400],
        1.4,
    );
    let _spacing = SpacingResult::new(16, 32, 760, BTreeMap::new());
    let _motion = MotionResult::new(vec![], BTreeMap::new(), false, false, 4, 1, false, false);
    let _structural = StructuralResult::new(NavShape::new(3, true, false), BTreeMap::new(), 2.5);
    let _voice = VoiceResult::new(10, 100, 500, 10.0, 8, 18, 0.4, 1, 10000, "casual");
    let _interactive = InteractiveResult::new(
        vec![HoverTreatmentEntry::new(HoverTreatment::ColorShift, 1)],
        BTreeMap::new(),
        true,
        BTreeMap::new(),
        true,
    );
    let _section = PatternClassification::new("paragraph", 60, "");
    // If this test compiles + runs, every constructor matches
    // its struct shape.
}

#[test]
fn arc_emits_zero_sections_for_empty_signals_passes_through() {
    let signals = ExtractedSignals::default();
    let spec = map_to_spec("empty", "", &signals);
    assert!(spec.pages.is_empty());

    let dir = std::env::temp_dir().join(format!("forge-arc-empty-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let written = synthesize(&spec, &dir).unwrap();
    assert!(written.is_empty());
    let _ = fs::remove_dir_all(&dir);
}
