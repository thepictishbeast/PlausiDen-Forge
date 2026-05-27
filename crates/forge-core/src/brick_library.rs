//! `brick_library` — hand-curated multi-primitive compositions
//! that operators reach for instead of composing from scratch.
//!
//! Per task #383. Bricks sit between primitives (single section /
//! block) and templates (whole-site assemblies). Each brick is a
//! named composition of primitives that produces a specific
//! page-section shape — "Editorial Lead", "Feature Trio",
//! "Service Status Banner" — with PageKind affinity.
//!
//! ## Why bricks
//!
//! Without bricks, every operator re-derives the same section-
//! sequence patterns. With bricks, the substrate ships a catalog
//! of known-good compositions cross-referenced to exemplars
//! (#380) and resource budgets (#381).
//!
//! ## Composition shape
//!
//! A brick is an ordered list of section descriptors. Each
//! descriptor names the section kind + optional variant +
//! optional decoration. Operators apply a brick by inserting its
//! section sequence into a page; the bricks's `page_kind_fit`
//! constrains where it's appropriate.

use serde::Serialize;

/// One section descriptor inside a brick.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct BrickSection {
    /// CmsSection kind slug.
    pub kind: &'static str,
    /// Optional variant tag (e.g., "editorial", "minimal").
    pub variant: Option<&'static str>,
    /// One-line hint about role within the brick.
    pub role_hint: &'static str,
}

/// PageKind affinity of a brick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BrickFit {
    /// Universal — fits any PageKind.
    Universal,
    /// Marketing-landing band only.
    MarketingLanding,
    /// Brief band only.
    Brief,
    /// Editorial / magazine band only.
    Editorial,
    /// Civic / public-service band only.
    Civic,
    /// Documentation hub band only.
    Documentation,
    /// Portfolio band only.
    Portfolio,
}

impl BrickFit {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Universal => "universal",
            Self::MarketingLanding => "marketing_landing",
            Self::Brief => "brief",
            Self::Editorial => "editorial",
            Self::Civic => "civic",
            Self::Documentation => "documentation",
            Self::Portfolio => "portfolio",
        }
    }
}

/// One brick entry.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct Brick {
    /// Stable kebab-case identifier.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// One-line description.
    pub description: &'static str,
    /// PageKind this brick fits.
    pub fit: BrickFit,
    /// Ordered section descriptors.
    pub sections: &'static [BrickSection],
    /// Cross-reference to a related exemplar (if any).
    pub related_exemplar: Option<&'static str>,
    /// Cross-reference to a related anti-pattern (avoidance hint).
    pub avoid_pattern: Option<&'static str>,
}

/// Canonical brick library seed set.
pub const BRICKS: &[Brick] = &[
    Brick {
        id: "editorial-lead",
        name: "Editorial lead",
        description: "Hero (editorial) + opening paragraph + pull-quote + \
                      second paragraph. Magazine-shape page opener.",
        fit: BrickFit::Editorial,
        sections: &[
            BrickSection {
                kind: "hero_editorial",
                variant: Some("plain"),
                role_hint: "Establishes mood + headline; serif display.",
            },
            BrickSection {
                kind: "paragraph",
                variant: None,
                role_hint: "Opening paragraph with optional drop-cap.",
            },
            BrickSection {
                kind: "pull_quote",
                variant: Some("display"),
                role_hint: "Editorial pull quote breaks the prose rhythm.",
            },
            BrickSection {
                kind: "paragraph",
                variant: None,
                role_hint: "Continuation paragraph.",
            },
        ],
        related_exemplar: Some("kinfolk-editorial-shape"),
        avoid_pattern: None,
    },
    Brick {
        id: "brief-essay",
        name: "Brief essay",
        description: "Heading + 3-5 paragraphs. Single vertical column, no \
                      hero. Brief-shape default.",
        fit: BrickFit::Brief,
        sections: &[
            BrickSection {
                kind: "heading",
                variant: Some("level=1"),
                role_hint: "Essay title.",
            },
            BrickSection {
                kind: "paragraph",
                variant: None,
                role_hint: "Opening paragraph.",
            },
            BrickSection {
                kind: "paragraph",
                variant: None,
                role_hint: "Middle paragraph(s).",
            },
            BrickSection {
                kind: "paragraph",
                variant: None,
                role_hint: "Closing paragraph.",
            },
        ],
        related_exemplar: Some("paulgraham-essay-shape"),
        avoid_pattern: None,
    },
    Brick {
        id: "feature-trio-editorial",
        name: "Feature trio (editorial decoration)",
        description: "3-up FeatureSpotlight with Editorial decoration. \
                      Marketing-landing band; uses content-led decoration \
                      vs SaaS-card chrome.",
        fit: BrickFit::MarketingLanding,
        sections: &[
            BrickSection {
                kind: "feature_spotlight",
                variant: Some("decoration=editorial;columns=3"),
                role_hint: "3-up feature row with top accent rules.",
            },
        ],
        related_exemplar: None,
        avoid_pattern: Some("decorated-everywhere"),
    },
    Brick {
        id: "service-status-banner",
        name: "Service status banner",
        description: "Alert (info tone) + paragraph explanation. Civic-band \
                      service-status pattern.",
        fit: BrickFit::Civic,
        sections: &[
            BrickSection {
                kind: "alert",
                variant: Some("tone=info"),
                role_hint: "Service status announcement.",
            },
            BrickSection {
                kind: "paragraph",
                variant: None,
                role_hint: "Explanation of current state.",
            },
        ],
        related_exemplar: Some("govuk-task-flow"),
        avoid_pattern: None,
    },
    Brick {
        id: "split-hero-about",
        name: "Split hero — about page",
        description: "SplitHero with photo left + text right. About / \
                      profile / portfolio opener.",
        fit: BrickFit::Portfolio,
        sections: &[
            BrickSection {
                kind: "hero_split",
                variant: Some("side=right"),
                role_hint: "Image left, narrative right.",
            },
            BrickSection {
                kind: "paragraph",
                variant: None,
                role_hint: "Below-hero supporting prose.",
            },
        ],
        related_exemplar: None,
        avoid_pattern: None,
    },
    Brick {
        id: "doc-section-with-toc",
        name: "Documentation section",
        description: "Heading + paragraph + code + paragraph + heading + \
                      paragraph. Standard docs section flow.",
        fit: BrickFit::Documentation,
        sections: &[
            BrickSection {
                kind: "heading",
                variant: Some("level=2"),
                role_hint: "Section anchor.",
            },
            BrickSection {
                kind: "paragraph",
                variant: None,
                role_hint: "Introduce the concept.",
            },
            BrickSection {
                kind: "code",
                variant: None,
                role_hint: "Worked example.",
            },
            BrickSection {
                kind: "paragraph",
                variant: None,
                role_hint: "Explain the example.",
            },
            BrickSection {
                kind: "heading",
                variant: Some("level=3"),
                role_hint: "Sub-section anchor.",
            },
            BrickSection {
                kind: "paragraph",
                variant: None,
                role_hint: "Detail.",
            },
        ],
        related_exemplar: None,
        avoid_pattern: None,
    },
];

/// Return all bricks.
#[must_use]
pub fn all_bricks() -> &'static [Brick] {
    BRICKS
}

/// Look up a brick by ID.
#[must_use]
pub fn get_brick(id: &str) -> Option<&'static Brick> {
    BRICKS.iter().find(|b| b.id == id)
}

/// Filter bricks by PageKind fit (Universal bricks always
/// included).
#[must_use]
pub fn bricks_for_fit(fit: BrickFit) -> Vec<&'static Brick> {
    BRICKS
        .iter()
        .filter(|b| b.fit == fit || b.fit == BrickFit::Universal)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_not_empty() {
        assert!(!BRICKS.is_empty());
    }

    #[test]
    fn brick_ids_unique() {
        let mut ids: Vec<&str> = BRICKS.iter().map(|b| b.id).collect();
        ids.sort_unstable();
        let original = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), original);
    }

    #[test]
    fn every_brick_has_sections() {
        for b in BRICKS {
            assert!(!b.sections.is_empty(), "brick {} has no sections", b.id);
        }
    }

    #[test]
    fn bricks_for_fit_filters() {
        let editorial = bricks_for_fit(BrickFit::Editorial);
        assert!(editorial.iter().any(|b| b.id == "editorial-lead"));
        // Should NOT include brief-essay.
        assert!(editorial.iter().all(|b| b.id != "brief-essay"));
    }

    #[test]
    fn fit_slug_stable() {
        assert_eq!(BrickFit::Universal.slug(), "universal");
        assert_eq!(BrickFit::MarketingLanding.slug(), "marketing_landing");
        assert_eq!(BrickFit::Brief.slug(), "brief");
        assert_eq!(BrickFit::Editorial.slug(), "editorial");
        assert_eq!(BrickFit::Civic.slug(), "civic");
        assert_eq!(BrickFit::Documentation.slug(), "documentation");
        assert_eq!(BrickFit::Portfolio.slug(), "portfolio");
    }

    #[test]
    fn get_brick_finds_known() {
        assert!(get_brick("editorial-lead").is_some());
        assert!(get_brick("nonexistent").is_none());
    }

    #[test]
    fn related_exemplar_refs_are_non_empty_when_set() {
        for b in BRICKS {
            if let Some(ex) = b.related_exemplar {
                assert!(!ex.is_empty());
            }
        }
    }
}
