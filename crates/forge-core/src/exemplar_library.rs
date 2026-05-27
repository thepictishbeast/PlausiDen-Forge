//! `exemplar_library` — hand-curated libraries of exemplars,
//! anti-exemplars, and contrast pairs.
//!
//! Per task #380. Exemplars are concrete examples of well-done
//! substrate output; anti-exemplars are bad output; contrast
//! pairs put one of each side-by-side along a specific axis so
//! operators see "good vs bad" in the same dimension.
//!
//! These libraries train operators + AI agents on what the
//! substrate considers good vs bad. They're cited by audit
//! findings, surfaced via doc_query, and serve as ground truth
//! for new generation.
//!
//! ## Why hand-curated
//!
//! The substrate ships with a small seed set. Operators register
//! new entries as their tenants ship. The substrate-reframe
//! doctrine is explicit: machine-generated exemplars are
//! suspect; the library prefers human-curated entries reviewed
//! against the reference-site frame.
//!
//! ## Future hook
//!
//! Layer-6 outcome ratings (#379) can promote a tenant to
//! "exemplar candidate" status when it scores high across
//! ship + aesthetic + retention. The library would then accept
//! the promotion via a registry-append pattern. Out of scope
//! for this iteration; the static seed is the starting point.

use serde::Serialize;

/// Category of an exemplar/anti-exemplar entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExemplarCategory {
    /// Marketing-landing-shape site.
    MarketingLanding,
    /// Brief-shape site (paulgraham, single page).
    Brief,
    /// Editorial / magazine register (kinfolk).
    Editorial,
    /// Civic / public-service (gov.uk).
    Civic,
    /// Documentation hub (rust-lang).
    Documentation,
    /// Portfolio.
    Portfolio,
}

impl ExemplarCategory {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::MarketingLanding => "marketing_landing",
            Self::Brief => "brief",
            Self::Editorial => "editorial",
            Self::Civic => "civic",
            Self::Documentation => "documentation",
            Self::Portfolio => "portfolio",
        }
    }
}

/// One positive exemplar entry.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct Exemplar {
    /// Stable kebab-case identifier.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// What page-kind / register this exemplar represents.
    pub category: ExemplarCategory,
    /// One-line summary of what's good about it.
    pub summary: &'static str,
    /// Why it's exemplary (longer rationale).
    pub rationale: &'static str,
    /// Pointer into the substrate (tenant slug, reference URL,
    /// fixture path) for the operator to inspect.
    pub content_ref: &'static str,
}

/// One anti-exemplar entry — known-bad output.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct AntiExemplar {
    /// Stable kebab-case identifier.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Category context.
    pub category: ExemplarCategory,
    /// One-line summary of what's wrong with it.
    pub summary: &'static str,
    /// Why it's bad (longer rationale).
    pub reason_anti: &'static str,
    /// Related anti-pattern dictionary id (if any).
    pub related_pattern: Option<&'static str>,
}

/// A contrast pair: same intent expressed well and badly,
/// along one specific axis.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct ContrastPair {
    /// Stable kebab-case identifier.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Axis the pair contrasts (theme, decoration, density, etc.)
    pub axis: &'static str,
    /// Reference to the good exemplar.
    pub good_exemplar_id: &'static str,
    /// Reference to the bad exemplar.
    pub bad_exemplar_id: &'static str,
    /// What the operator should learn from the contrast.
    pub takeaway: &'static str,
}

/// Canonical exemplar seed set.
pub const EXEMPLARS: &[Exemplar] = &[
    Exemplar {
        id: "paulgraham-essay-shape",
        name: "paulgraham.com essay shape",
        category: ExemplarCategory::Brief,
        summary: "Single vertical-stack column; serif body; no chrome; ISO date stamp.",
        rationale: "Distilled brief register. Density: comfortable. Theme: light + serif. \
                    No nav, no footer, no hero — just the essay. The substrate's brief \
                    PageKind defaults should match this shape closely.",
        content_ref: "https://paulgraham.com (any essay page)",
    },
    Exemplar {
        id: "kinfolk-editorial-shape",
        name: "kinfolk.com editorial shape",
        category: ExemplarCategory::Editorial,
        summary: "Cream canvas + serif display + full-bleed photographic dividers.",
        rationale: "Magazine register: density loose, theme editorial, asymmetric \
                    columns, drop-cap on opening paragraph. Demonstrates the editorial \
                    theme (#358) end-to-end. Compositional Tier-1 gap (mid-flow \
                    full-bleed photo) shows what substrate still needs.",
        content_ref: "https://kinfolk.com",
    },
    Exemplar {
        id: "govuk-task-flow",
        name: "gov.uk task-flow shape",
        category: ExemplarCategory::Civic,
        summary: "Step indicators + form-with-help-panel + service status banner.",
        rationale: "Civic register: density dense, theme light, button variant \
                    secondary (no big primary CTAs), accessibility-default-heavy. \
                    Demonstrates per-PageKind defaulting per #360 + accessibility \
                    workflows per #388-#391.",
        content_ref: "https://gov.uk (any task-flow service page)",
    },
];

/// Canonical anti-exemplar seed set.
pub const ANTI_EXEMPLARS: &[AntiExemplar] = &[
    AntiExemplar {
        id: "saas-yc-launch",
        name: "Generic YC-launch landing",
        category: ExemplarCategory::MarketingLanding,
        summary: "Hero + 3-up feature + testimonial + pricing + CTA. Every YC \
                  Demo Day site in 2020-2024.",
        reason_anti: "Substrate-band collapse. No tenant identity, no information, \
                      no register distinction. The shape so default it has no \
                      content. Anti-pattern dictionary entry: saas-template-collapse.",
        related_pattern: Some("saas-template-collapse"),
    },
    AntiExemplar {
        id: "hero-cta-affiliate",
        name: "Hero + direct CTA (affiliate scam)",
        category: ExemplarCategory::MarketingLanding,
        summary: "Big hero claim + immediate Buy Now CTA. No substance between.",
        reason_anti: "Signals zero content underneath the claim. Anti-pattern \
                      dictionary entry: hero-cta-only (Block severity). Substrate \
                      refuses to ship without explicit content-substance exempt.",
        related_pattern: Some("hero-cta-only"),
    },
    AntiExemplar {
        id: "decorated-everywhere-cms",
        name: "Every section uses Decorated SaaS-card",
        category: ExemplarCategory::Editorial,
        summary: "Editorial PageKind with FeatureSpotlight::Decorated on every section.",
        reason_anti: "Default-band drift: PageKind says editorial but every section \
                      uses the SaaS-default decoration. Per #360 neutralize-defaults \
                      audit, this is the case the dispatch table catches.",
        related_pattern: Some("decorated-everywhere"),
    },
];

/// Canonical contrast-pair seed set.
pub const CONTRAST_PAIRS: &[ContrastPair] = &[
    ContrastPair {
        id: "theme-marketing-vs-editorial",
        name: "Marketing-band theme vs Editorial theme",
        axis: "theme",
        good_exemplar_id: "kinfolk-editorial-shape",
        bad_exemplar_id: "saas-yc-launch",
        takeaway: "Themes are PageKind-driven. An editorial-shape site theming as \
                   SaaS-modern collapses register. Match theme to PageKind.",
    },
    ContrastPair {
        id: "decoration-content-substance",
        name: "Content with substance vs hero-and-CTA-only",
        axis: "content_substance",
        good_exemplar_id: "paulgraham-essay-shape",
        bad_exemplar_id: "hero-cta-affiliate",
        takeaway: "Substance is what brief-shape sites bring. A hero followed \
                   immediately by a CTA signals nothing-to-say. Always insert at \
                   least 2 substantive sections.",
    },
];

/// Return all exemplars.
#[must_use]
pub fn all_exemplars() -> &'static [Exemplar] {
    EXEMPLARS
}

/// Return all anti-exemplars.
#[must_use]
pub fn all_anti_exemplars() -> &'static [AntiExemplar] {
    ANTI_EXEMPLARS
}

/// Return all contrast pairs.
#[must_use]
pub fn all_contrast_pairs() -> &'static [ContrastPair] {
    CONTRAST_PAIRS
}

/// Look up an exemplar by ID.
#[must_use]
pub fn get_exemplar(id: &str) -> Option<&'static Exemplar> {
    EXEMPLARS.iter().find(|e| e.id == id)
}

/// Look up an anti-exemplar by ID.
#[must_use]
pub fn get_anti_exemplar(id: &str) -> Option<&'static AntiExemplar> {
    ANTI_EXEMPLARS.iter().find(|e| e.id == id)
}

/// Filter exemplars by category.
#[must_use]
pub fn exemplars_by_category(category: ExemplarCategory) -> Vec<&'static Exemplar> {
    EXEMPLARS.iter().filter(|e| e.category == category).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_not_empty() {
        assert!(!EXEMPLARS.is_empty());
        assert!(!ANTI_EXEMPLARS.is_empty());
        assert!(!CONTRAST_PAIRS.is_empty());
    }

    #[test]
    fn exemplar_ids_unique() {
        let mut ids: Vec<&str> = EXEMPLARS.iter().map(|e| e.id).collect();
        ids.sort_unstable();
        let original = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), original);
    }

    #[test]
    fn anti_exemplar_ids_unique() {
        let mut ids: Vec<&str> = ANTI_EXEMPLARS.iter().map(|e| e.id).collect();
        ids.sort_unstable();
        let original = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), original);
    }

    #[test]
    fn contrast_pair_references_resolve() {
        for pair in CONTRAST_PAIRS {
            assert!(
                get_exemplar(pair.good_exemplar_id).is_some(),
                "contrast pair {} references unknown good exemplar {}",
                pair.id,
                pair.good_exemplar_id
            );
            assert!(
                get_anti_exemplar(pair.bad_exemplar_id).is_some(),
                "contrast pair {} references unknown bad exemplar {}",
                pair.id,
                pair.bad_exemplar_id
            );
        }
    }

    #[test]
    fn anti_pattern_refs_are_known_or_none() {
        // related_pattern entries should be either None or a non-
        // empty string. (The actual referenced pattern IDs live in
        // forge-core::anti_pattern_dictionary; the cross-crate check
        // could be tightened later.)
        for ae in ANTI_EXEMPLARS {
            if let Some(p) = ae.related_pattern {
                assert!(!p.is_empty());
            }
        }
    }

    #[test]
    fn get_exemplar_finds_known() {
        assert!(get_exemplar("paulgraham-essay-shape").is_some());
        assert!(get_exemplar("nonexistent").is_none());
    }

    #[test]
    fn exemplars_by_category_filters() {
        let editorial = exemplars_by_category(ExemplarCategory::Editorial);
        assert!(editorial.iter().any(|e| e.id == "kinfolk-editorial-shape"));
    }

    #[test]
    fn category_slugs_stable() {
        assert_eq!(ExemplarCategory::MarketingLanding.slug(), "marketing_landing");
        assert_eq!(ExemplarCategory::Brief.slug(), "brief");
        assert_eq!(ExemplarCategory::Editorial.slug(), "editorial");
        assert_eq!(ExemplarCategory::Civic.slug(), "civic");
        assert_eq!(ExemplarCategory::Documentation.slug(), "documentation");
        assert_eq!(ExemplarCategory::Portfolio.slug(), "portfolio");
    }
}
