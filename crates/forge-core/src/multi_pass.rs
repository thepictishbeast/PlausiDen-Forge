//! `multi_pass` — generate multiple candidate variants of a
//! generation decision so the operator picks instead of accepting
//! whatever came first.
//!
//! Layer-4 substrate-reframe doctrine (#377): single-shot
//! generation produces template collapse. Multi-pass generation
//! produces N candidates that vary along orthogonal axes
//! (decoration, theme, density, register), surfaces them with
//! per-candidate rationale, and forces explicit operator
//! selection.
//!
//! ## Scope
//!
//! This module operates at the substrate-decision layer, not the
//! content-generation layer. It produces alternative *structural*
//! variants of a given seed input (theme name, decoration enum,
//! density tier, primitive choice) — not alternative prose.
//! Content generation belongs to LFI / LLM pipelines downstream.
//!
//! ## Use cases
//!
//! - `theme_alternatives(seed_theme)` — surface 3 nearby themes
//!   for an operator considering theme=light to also evaluate
//!   theme=warm and theme=editorial
//! - `decoration_alternatives(seed_decoration)` — surface the
//!   N-1 sibling decoration variants alongside the operator's
//!   chosen one
//! - `density_alternatives(seed_density)` — surface adjacent
//!   density tiers
//! - `compose_alternatives(brief_axis, seed)` — generic dispatch

use serde::Serialize;

/// Axes along which the substrate can produce alternatives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AlternativeAxis {
    /// Theme name (light/dark/warm/editorial/etc.)
    Theme,
    /// FeatureSpotlight / Testimonial decoration variant.
    Decoration,
    /// DensityTier (sparse/comfortable/dense/extreme).
    Density,
    /// PageKind (marketing_landing/brief/editorial/etc.)
    PageKind,
    /// HeroBackground variant.
    HeroBackground,
}

impl AlternativeAxis {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::Decoration => "decoration",
            Self::Density => "density",
            Self::PageKind => "page_kind",
            Self::HeroBackground => "hero_background",
        }
    }
}

/// One generation-candidate variant.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct GenerationPass {
    /// Axis this candidate varies on.
    pub axis: AlternativeAxis,
    /// The variant value (e.g., "warm", "editorial", "decorated").
    pub value: String,
    /// Why this candidate is worth considering. Hand-curated.
    pub rationale: String,
    /// Distance from the seed (0 = same; higher = more divergent).
    pub divergence: u32,
}

/// Multi-pass alternatives report for one (axis, seed) input.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct AlternativesReport {
    /// Axis explored.
    pub axis: AlternativeAxis,
    /// The seed value the operator supplied (or substrate inferred).
    pub seed: String,
    /// The candidate alternatives, sorted ascending by divergence.
    pub passes: Vec<GenerationPass>,
    /// Diversity score across passes (0..=100; higher = more orthogonal).
    pub diversity_score: u32,
}

/// Produce alternative theme candidates for a seed theme.
///
/// Hand-curated map: each theme has a small set of "nearby but
/// distinct" alternatives. The substrate's job is not to invent
/// themes — it's to surface the alternatives the operator might
/// otherwise miss.
#[must_use]
pub fn theme_alternatives(seed: &str) -> AlternativesReport {
    let passes = match seed {
        "light" => vec![
            ("warm", "Warm-neutral palette; same register as light but \
                     less clinical. Try for content-led sites.", 1),
            ("editorial", "Magazine register: cream canvas + serif display + \
                          publication-red accent. Try for kinfolk-shape \
                          editorial.", 3),
            ("ocean", "Cool-blue palette; calmer than light, still bright.", 2),
        ],
        "dark" => vec![
            ("amoled", "True-black bg #000000; OLED-optimized + battery-friendly. \
                       Same register but stricter contrast.", 1),
            ("editorial", "Some operators reaching for 'dark' actually want \
                          editorial-cream (dark theme is SaaS-default).", 4),
            ("violet", "Cool dark with violet accent; less SaaS-modern.", 2),
        ],
        "warm" => vec![
            ("rose", "Warm + slightly redder; sibling palette.", 1),
            ("editorial", "Warm extended to full magazine register.", 2),
            ("light", "Cooler counterpart if warmth feels too heavy.", 1),
        ],
        "editorial" => vec![
            ("warm", "Drops the publication-red accent; keeps cream canvas.", 1),
            ("rose", "Editorial-leaning with rose accent for softer brand.", 2),
            ("light", "Strips serif display; falls back to system-ui.", 3),
        ],
        _ => vec![
            ("light", "Universal safe choice; works for most bands.", 4),
            ("warm", "Content-led sites; less clinical than light.", 4),
            ("editorial", "Magazine / publication register.", 5),
        ],
    };

    let passes: Vec<GenerationPass> = passes
        .into_iter()
        .map(|(value, rationale, divergence)| GenerationPass {
            axis: AlternativeAxis::Theme,
            value: value.to_owned(),
            rationale: rationale.to_owned(),
            divergence,
        })
        .collect();

    let diversity = compute_diversity(&passes);

    AlternativesReport {
        axis: AlternativeAxis::Theme,
        seed: seed.to_owned(),
        passes,
        diversity_score: diversity,
    }
}

/// Produce alternative decoration candidates.
#[must_use]
pub fn decoration_alternatives(seed: &str) -> AlternativesReport {
    let passes = match seed {
        "decorated" => vec![
            ("editorial", "Strip SaaS-card chrome; top accent rule per item.", 1),
            ("minimal", "Tight grid, no decoration. Best for dense content.", 2),
        ],
        "editorial" => vec![
            ("decorated", "Back-compat SaaS-card chrome (default).", 1),
            ("minimal", "Strip decoration entirely; pure typography.", 1),
        ],
        "minimal" => vec![
            ("editorial", "Add top accent rule for editorial register.", 1),
            ("decorated", "Add SaaS-card chrome if minimal is too austere.", 2),
        ],
        _ => vec![
            ("decorated", "Default SaaS-card shape.", 0),
            ("editorial", "Editorial accent-rule shape.", 1),
            ("minimal", "No-decoration shape.", 2),
        ],
    };

    let passes: Vec<GenerationPass> = passes
        .into_iter()
        .map(|(value, rationale, divergence)| GenerationPass {
            axis: AlternativeAxis::Decoration,
            value: value.to_owned(),
            rationale: rationale.to_owned(),
            divergence,
        })
        .collect();
    let diversity = compute_diversity(&passes);

    AlternativesReport {
        axis: AlternativeAxis::Decoration,
        seed: seed.to_owned(),
        passes,
        diversity_score: diversity,
    }
}

/// Produce alternative density-tier candidates.
#[must_use]
pub fn density_alternatives(seed: &str) -> AlternativesReport {
    let passes = match seed {
        "sparse" => vec![
            ("comfortable", "One step denser; more substance per scroll.", 1),
            ("dense", "Documentation / civic register; favors information \
                      density.", 2),
        ],
        "comfortable" => vec![
            ("sparse", "Brief / editorial register; favors breathing room.", 1),
            ("dense", "Documentation register.", 1),
        ],
        "dense" => vec![
            ("comfortable", "Marketing register; one tier looser.", 1),
            ("extreme", "Reference-style density; only when justified.", 1),
        ],
        "extreme" => vec![
            ("dense", "Step looser; recommended for most use cases.", 1),
        ],
        _ => vec![
            ("sparse", "Brief register; lots of whitespace.", 0),
            ("comfortable", "Marketing register; SaaS default.", 1),
            ("dense", "Documentation register.", 2),
        ],
    };

    let passes: Vec<GenerationPass> = passes
        .into_iter()
        .map(|(value, rationale, divergence)| GenerationPass {
            axis: AlternativeAxis::Density,
            value: value.to_owned(),
            rationale: rationale.to_owned(),
            divergence,
        })
        .collect();
    let diversity = compute_diversity(&passes);

    AlternativesReport {
        axis: AlternativeAxis::Density,
        seed: seed.to_owned(),
        passes,
        diversity_score: diversity,
    }
}

/// Generic dispatch: route to the right axis-specific function.
#[must_use]
pub fn compose_alternatives(axis: AlternativeAxis, seed: &str) -> AlternativesReport {
    match axis {
        AlternativeAxis::Theme => theme_alternatives(seed),
        AlternativeAxis::Decoration => decoration_alternatives(seed),
        AlternativeAxis::Density => density_alternatives(seed),
        // Future axes return empty for now; the dispatch is open
        // for extension.
        AlternativeAxis::PageKind | AlternativeAxis::HeroBackground => {
            AlternativesReport {
                axis,
                seed: seed.to_owned(),
                passes: Vec::new(),
                diversity_score: 0,
            }
        }
    }
}

/// Compute a diversity score from a set of passes.
/// 100 = max divergence sum; lower = more clustered.
fn compute_diversity(passes: &[GenerationPass]) -> u32 {
    if passes.is_empty() {
        return 0;
    }
    let total: u32 = passes.iter().map(|p| p.divergence).sum();
    let max_possible = passes.len() as u32 * 5;
    if max_possible == 0 {
        return 0;
    }
    ((total * 100) / max_possible).min(100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_alternatives_for_known_seed() {
        let report = theme_alternatives("light");
        assert_eq!(report.axis, AlternativeAxis::Theme);
        assert_eq!(report.seed, "light");
        assert_eq!(report.passes.len(), 3);
        assert!(report.passes.iter().any(|p| p.value == "warm"));
        assert!(report.passes.iter().any(|p| p.value == "editorial"));
    }

    #[test]
    fn theme_alternatives_for_unknown_seed() {
        let report = theme_alternatives("doesnotexist");
        assert!(!report.passes.is_empty());
    }

    #[test]
    fn decoration_alternatives_decorated() {
        let report = decoration_alternatives("decorated");
        assert!(report.passes.iter().any(|p| p.value == "editorial"));
        assert!(report.passes.iter().any(|p| p.value == "minimal"));
    }

    #[test]
    fn density_alternatives_comfortable() {
        let report = density_alternatives("comfortable");
        let values: Vec<&str> = report.passes.iter().map(|p| p.value.as_str()).collect();
        assert!(values.contains(&"sparse"));
        assert!(values.contains(&"dense"));
    }

    #[test]
    fn diversity_score_in_range() {
        let report = theme_alternatives("light");
        assert!(report.diversity_score <= 100);
    }

    #[test]
    fn compose_alternatives_dispatch() {
        let report = compose_alternatives(AlternativeAxis::Theme, "warm");
        assert_eq!(report.axis, AlternativeAxis::Theme);
        assert!(!report.passes.is_empty());
    }

    #[test]
    fn page_kind_axis_returns_empty_for_now() {
        let report = compose_alternatives(AlternativeAxis::PageKind, "marketing_landing");
        assert!(report.passes.is_empty());
        assert_eq!(report.diversity_score, 0);
    }

    #[test]
    fn axis_slugs_stable() {
        assert_eq!(AlternativeAxis::Theme.slug(), "theme");
        assert_eq!(AlternativeAxis::Decoration.slug(), "decoration");
        assert_eq!(AlternativeAxis::Density.slug(), "density");
        assert_eq!(AlternativeAxis::PageKind.slug(), "page_kind");
        assert_eq!(AlternativeAxis::HeroBackground.slug(), "hero_background");
    }
}
