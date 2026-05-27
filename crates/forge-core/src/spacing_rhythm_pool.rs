//! `spacing_rhythm_pool` — hand-curated pool of spacing-rhythm
//! variants for vertical layout cadence.
//!
//! Per task #395 (default-fragmentation pool 5 of 5). Final pool
//! of the 4-pool series (#392/#393/#394/#395 + gradient_pool
//! #352 = 5 total).
//!
//! Spacing rhythm = the vertical-rhythm pattern between sections
//! + prose lines + headings + container padding. Different
//! rhythms produce visibly different page personalities even
//! with identical content. SaaS-modern rhythm is tight + dense;
//! editorial rhythm is loose + breathing.
//!
//! Pool size 12 (smaller than 25 because meaningful rhythm
//! variation is more constrained — 4 base * 3 section-gap levels
//! covers most useful combinations).

use serde::Serialize;

/// Base spacing unit (the multiplier for the 4-px grid).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SpacingBase {
    /// 4px base grid.
    Four,
    /// 8px base grid.
    Eight,
    /// 16px base grid.
    Sixteen,
    /// 24px base grid.
    TwentyFour,
}

impl SpacingBase {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Four => "four",
            Self::Eight => "eight",
            Self::Sixteen => "sixteen",
            Self::TwentyFour => "twenty_four",
        }
    }

    /// The numeric base in pixels.
    #[must_use]
    pub const fn pixels(self) -> u32 {
        match self {
            Self::Four => 4,
            Self::Eight => 8,
            Self::Sixteen => 16,
            Self::TwentyFour => 24,
        }
    }
}

/// Section-to-section gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SectionGap {
    /// Tight: ~32px gap.
    Tight,
    /// Comfortable: ~64px gap.
    Comfortable,
    /// Loose: ~120px gap.
    Loose,
    /// Very loose: ~200px gap.
    VeryLoose,
}

impl SectionGap {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Tight => "tight",
            Self::Comfortable => "comfortable",
            Self::Loose => "loose",
            Self::VeryLoose => "very_loose",
        }
    }
}

/// Prose line-height (leading).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProseLeading {
    /// Tight: 1.3.
    Tight,
    /// Comfortable: 1.55.
    Comfortable,
    /// Loose: 1.75.
    Loose,
    /// Display: 1.9 (editorial / brief register).
    Display,
}

impl ProseLeading {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Tight => "tight",
            Self::Comfortable => "comfortable",
            Self::Loose => "loose",
            Self::Display => "display",
        }
    }
}

/// Container padding (page edge → content distance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ContainerPadding {
    /// Compact: 16px gutter.
    Compact,
    /// Comfortable: 32px gutter.
    Comfortable,
    /// Generous: 80px gutter.
    Generous,
}

impl ContainerPadding {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Comfortable => "comfortable",
            Self::Generous => "generous",
        }
    }
}

/// One spacing-rhythm pool entry.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct SpacingRhythm {
    /// Stable slug.
    pub slug: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Base unit.
    pub base: SpacingBase,
    /// Section-to-section gap.
    pub section_gap: SectionGap,
    /// Prose leading.
    pub prose_leading: ProseLeading,
    /// Container padding.
    pub container_padding: ContainerPadding,
    /// PageKinds this entry suits. Empty = universal.
    pub page_kind_fit: &'static [&'static str],
}

/// Canonical spacing-rhythm pool — 12 hand-curated entries.
pub const SPACING_POOL: &[SpacingRhythm] = &[
    SpacingRhythm {
        slug: "tight-saas",
        name: "Tight — SaaS modern dense",
        base: SpacingBase::Four,
        section_gap: SectionGap::Tight,
        prose_leading: ProseLeading::Comfortable,
        container_padding: ContainerPadding::Comfortable,
        page_kind_fit: &["marketing_landing"],
    },
    SpacingRhythm {
        slug: "comfortable-marketing",
        name: "Comfortable — marketing default",
        base: SpacingBase::Eight,
        section_gap: SectionGap::Comfortable,
        prose_leading: ProseLeading::Comfortable,
        container_padding: ContainerPadding::Comfortable,
        page_kind_fit: &[],
    },
    SpacingRhythm {
        slug: "comfortable-docs",
        name: "Comfortable — docs reading flow",
        base: SpacingBase::Eight,
        section_gap: SectionGap::Comfortable,
        prose_leading: ProseLeading::Loose,
        container_padding: ContainerPadding::Comfortable,
        page_kind_fit: &["documentation"],
    },
    SpacingRhythm {
        slug: "loose-editorial",
        name: "Loose — editorial breathing",
        base: SpacingBase::Sixteen,
        section_gap: SectionGap::Loose,
        prose_leading: ProseLeading::Loose,
        container_padding: ContainerPadding::Generous,
        page_kind_fit: &["editorial"],
    },
    SpacingRhythm {
        slug: "loose-brief",
        name: "Loose — brief / essay reading",
        base: SpacingBase::Sixteen,
        section_gap: SectionGap::Loose,
        prose_leading: ProseLeading::Display,
        container_padding: ContainerPadding::Generous,
        page_kind_fit: &["brief"],
    },
    SpacingRhythm {
        slug: "very-loose-magazine",
        name: "Very loose — magazine register",
        base: SpacingBase::TwentyFour,
        section_gap: SectionGap::VeryLoose,
        prose_leading: ProseLeading::Display,
        container_padding: ContainerPadding::Generous,
        page_kind_fit: &["editorial"],
    },
    SpacingRhythm {
        slug: "compact-civic",
        name: "Compact — civic dense info",
        base: SpacingBase::Four,
        section_gap: SectionGap::Tight,
        prose_leading: ProseLeading::Comfortable,
        container_padding: ContainerPadding::Compact,
        page_kind_fit: &["civic"],
    },
    SpacingRhythm {
        slug: "comfortable-civic",
        name: "Comfortable — civic balanced",
        base: SpacingBase::Eight,
        section_gap: SectionGap::Comfortable,
        prose_leading: ProseLeading::Comfortable,
        container_padding: ContainerPadding::Comfortable,
        page_kind_fit: &["civic"],
    },
    SpacingRhythm {
        slug: "portfolio-airy",
        name: "Portfolio — airy showcase",
        base: SpacingBase::Sixteen,
        section_gap: SectionGap::VeryLoose,
        prose_leading: ProseLeading::Loose,
        container_padding: ContainerPadding::Generous,
        page_kind_fit: &["portfolio"],
    },
    SpacingRhythm {
        slug: "saas-loose",
        name: "SaaS — loose hero-heavy",
        base: SpacingBase::Sixteen,
        section_gap: SectionGap::Loose,
        prose_leading: ProseLeading::Comfortable,
        container_padding: ContainerPadding::Comfortable,
        page_kind_fit: &["marketing_landing"],
    },
    SpacingRhythm {
        slug: "docs-dense",
        name: "Docs — dense reference",
        base: SpacingBase::Four,
        section_gap: SectionGap::Tight,
        prose_leading: ProseLeading::Comfortable,
        container_padding: ContainerPadding::Compact,
        page_kind_fit: &["documentation"],
    },
    SpacingRhythm {
        slug: "minimal-universal",
        name: "Minimal — universal calm",
        base: SpacingBase::Eight,
        section_gap: SectionGap::Loose,
        prose_leading: ProseLeading::Loose,
        container_padding: ContainerPadding::Generous,
        page_kind_fit: &[],
    },
];

/// All spacing rhythm entries.
#[must_use]
pub fn all_rhythms() -> &'static [SpacingRhythm] {
    SPACING_POOL
}

/// Entries appropriate for a PageKind.
#[must_use]
pub fn rhythms_for(page_kind: &str) -> Vec<&'static SpacingRhythm> {
    SPACING_POOL
        .iter()
        .filter(|r| r.page_kind_fit.is_empty() || r.page_kind_fit.contains(&page_kind))
        .collect()
}

/// Deterministic identity-aware selection.
#[must_use]
pub fn select_rhythm(
    identity_hash: u64,
    page_kind: &str,
) -> Option<&'static SpacingRhythm> {
    let candidates = rhythms_for(page_kind);
    if candidates.is_empty() {
        return None;
    }
    let idx = (identity_hash as usize) % candidates.len();
    Some(candidates[idx])
}

/// Look up by slug.
#[must_use]
pub fn get_rhythm(slug: &str) -> Option<&'static SpacingRhythm> {
    SPACING_POOL.iter().find(|r| r.slug == slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_size_meets_minimum() {
        // Task spec calls for 10+ entries.
        assert!(SPACING_POOL.len() >= 10);
    }

    #[test]
    fn slugs_unique() {
        let mut slugs: Vec<&str> = SPACING_POOL.iter().map(|r| r.slug).collect();
        slugs.sort_unstable();
        let original = slugs.len();
        slugs.dedup();
        assert_eq!(slugs.len(), original);
    }

    #[test]
    fn rhythms_for_filters() {
        let editorial = rhythms_for("editorial");
        assert!(editorial.iter().any(|r| r.slug == "loose-editorial"));
        assert!(editorial.iter().all(|r| r.slug != "compact-civic"));
    }

    #[test]
    fn universals_appear_for_every_kind() {
        for kind in [
            "marketing_landing",
            "brief",
            "editorial",
            "civic",
            "documentation",
            "portfolio",
        ] {
            let pool = rhythms_for(kind);
            assert!(pool.iter().any(|r| r.slug == "comfortable-marketing"));
        }
    }

    #[test]
    fn select_deterministic() {
        let a = select_rhythm(123, "editorial").unwrap();
        let b = select_rhythm(123, "editorial").unwrap();
        assert_eq!(a.slug, b.slug);
    }

    #[test]
    fn select_disperses() {
        let a = select_rhythm(0, "marketing_landing").unwrap();
        let b = select_rhythm(7, "marketing_landing").unwrap();
        let c = select_rhythm(13, "marketing_landing").unwrap();
        assert!(!(a.slug == b.slug && b.slug == c.slug));
    }

    #[test]
    fn get_rhythm_finds_known() {
        assert!(get_rhythm("loose-editorial").is_some());
        assert!(get_rhythm("nonexistent").is_none());
    }

    #[test]
    fn base_pixels_correct() {
        assert_eq!(SpacingBase::Four.pixels(), 4);
        assert_eq!(SpacingBase::Eight.pixels(), 8);
        assert_eq!(SpacingBase::Sixteen.pixels(), 16);
        assert_eq!(SpacingBase::TwentyFour.pixels(), 24);
    }

    #[test]
    fn axis_slugs_stable() {
        assert_eq!(SpacingBase::Eight.slug(), "eight");
        assert_eq!(SectionGap::Comfortable.slug(), "comfortable");
        assert_eq!(ProseLeading::Display.slug(), "display");
        assert_eq!(ContainerPadding::Generous.slug(), "generous");
    }
}
