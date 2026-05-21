//! Integration harness for the Forge Lite fixtures.
//!
//! Per `docs/SUBSTRATE_REFRAME_2026_05_21.md` § Forge Lite. The
//! fixtures at `fixtures/forge-lite/` are the comparison-runs
//! base set for the diagnostic: five `ForgeLitePage` JSONs
//! covering distinct identity + theme + primitive combinations
//! from the closed lite vocabulary.
//!
//! This harness:
//!
//! 1. Loads every fixture from `fixtures/forge-lite/*.json`.
//! 2. Resolves each through
//!    [`forge_phases::forge_lite_resolve::resolve`].
//! 3. Confirms every fixture resolves cleanly (no
//!    [`forge_core::forge_lite::LiteValidationError`]).
//! 4. Confirms every resolved CmsPage contains only sections
//!    reachable from the 10 lite primitive kinds (no smuggled
//!    full-substrate primitives).
//! 5. Confirms cross-fixture variance — the resolved CmsPages
//!    span at least three distinct primitive-kind sequences,
//!    proving the lite surface produces visible structural
//!    variation even within its narrow vocabulary.
//!
//! Failures here mean either (a) the fixture set has drifted
//! (a JSON edit broke serde), (b) the resolver has regressed
//! (a kind that used to map now panics or smuggles), or (c) the
//! fixtures have collapsed into homogeneity (the comparison
//! base set no longer exercises variation, defeating the
//! diagnostic).

use std::collections::BTreeSet;
use std::path::PathBuf;

use forge_core::forge_lite::ForgeLitePage;
use forge_phases::forge_lite_resolve::resolve;
use loom_cms_render::CmsSection;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("forge-lite")
}

fn load_all() -> Vec<(String, ForgeLitePage)> {
    let dir = fixtures_dir();
    let mut out = Vec::new();
    let entries = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read fixtures dir {}: {e}", dir.display()));
    for entry in entries {
        let entry = entry.expect("entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let body = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()));
        let page: ForgeLitePage = serde_json::from_str(&body)
            .unwrap_or_else(|e| panic!("parse fixture {}: {e}", path.display()));
        out.push((
            path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_owned(),
            page,
        ));
    }
    // Deterministic ordering for stable test failure messages.
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn section_kind(section: &CmsSection) -> &'static str {
    match section {
        CmsSection::Hero { .. } => "hero",
        CmsSection::Heading { .. } => "heading",
        CmsSection::Paragraph { .. } => "paragraph",
        CmsSection::ImageHero { .. } => "image_hero",
        CmsSection::FeatureSpotlight { .. } => "feature_spotlight",
        CmsSection::PullQuote { .. } => "pull_quote",
        CmsSection::CallToAction { .. } => "call_to_action",
        CmsSection::LogoCloud { .. } => "logo_cloud",
        CmsSection::Compose { .. } => "compose",
        _ => "OUT_OF_BAND",
    }
}

/// The closed allowlist of CmsSection kinds that the lite
/// resolver is permitted to emit. `Compose` is on the allowlist
/// because Divider + Spacer round-trip through it.
const LITE_ALLOWED_RESOLVED_KINDS: &[&str] = &[
    "hero",
    "heading",
    "paragraph",
    "image_hero",
    "feature_spotlight",
    "pull_quote",
    "call_to_action",
    "logo_cloud",
    "compose",
];

#[test]
fn fixtures_dir_has_five_fixtures() {
    let entries = load_all();
    assert_eq!(
        entries.len(),
        5,
        "expected exactly 5 lite fixtures, got {}: {:?}",
        entries.len(),
        entries.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );
}

#[test]
fn every_fixture_resolves_cleanly() {
    let entries = load_all();
    for (name, page) in entries {
        let result = resolve(&page);
        assert!(
            result.is_ok(),
            "fixture {name:?} failed to resolve: {result:?}"
        );
    }
}

#[test]
fn every_resolved_section_is_within_lite_allowlist() {
    let entries = load_all();
    for (name, page) in entries {
        let resolved = resolve(&page).expect("resolves");
        for (idx, section) in resolved.sections.iter().enumerate() {
            let kind = section_kind(section);
            assert!(
                LITE_ALLOWED_RESOLVED_KINDS.contains(&kind),
                "fixture {name:?} section {idx} resolved to out-of-band kind {kind:?} — the lite resolver must only emit kinds reachable from the 10 lite primitives"
            );
        }
    }
}

#[test]
fn fixtures_span_distinct_kind_sequences() {
    let entries = load_all();
    let mut sequences: BTreeSet<Vec<&'static str>> = BTreeSet::new();
    for (_name, page) in entries {
        let resolved = resolve(&page).expect("resolves");
        let seq: Vec<&'static str> = resolved.sections.iter().map(section_kind).collect();
        sequences.insert(seq);
    }
    assert!(
        sequences.len() >= 3,
        "expected ≥3 distinct primitive-kind sequences across the lite fixture set, got {} — fixtures have collapsed into homogeneity and the diagnostic base no longer exercises variation",
        sequences.len()
    );
}

#[test]
fn fixtures_collectively_use_at_least_seven_lite_primitives() {
    // Tests the lite vocabulary breadth: at least 7 of the 10
    // primitives should appear across the fixture set. If
    // fewer, the fixtures aren't exercising the lite surface
    // broadly enough for the diagnostic to be informative.
    let entries = load_all();
    let mut all_kinds: BTreeSet<String> = BTreeSet::new();
    for (_name, page) in entries {
        for section in &page.sections {
            // Reflect the wire-format kind from the JSON
            // serialization — this is the canonical lite kind
            // name, not the resolver's output.
            let kind = serde_json::to_value(section)
                .ok()
                .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_owned))
                .unwrap_or_default();
            if !kind.is_empty() {
                all_kinds.insert(kind);
            }
        }
    }
    assert!(
        all_kinds.len() >= 7,
        "lite fixtures collectively use only {} of 10 primitives: {:?} — broaden coverage to make the diagnostic more informative",
        all_kinds.len(),
        all_kinds
    );
}

#[test]
fn fixtures_span_every_lite_theme() {
    use forge_core::forge_lite::ForgeLiteTheme;
    let entries = load_all();
    let mut themes: BTreeSet<ForgeLiteTheme> = BTreeSet::new();
    for (_name, page) in entries {
        themes.insert(page.theme);
    }
    assert!(
        themes.contains(&ForgeLiteTheme::Light)
            && themes.contains(&ForgeLiteTheme::Dark)
            && themes.contains(&ForgeLiteTheme::Warm),
        "lite fixtures should span all 3 themes; got {themes:?}"
    );
}
