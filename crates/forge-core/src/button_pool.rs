//! `button_pool` — hand-curated pool of button-treatment variants
//! with identity-aware deterministic selection.
//!
//! Per task #394 (default-fragmentation pool 4 of 5). Sibling to
//! header_pool (#392) + footer_pool (#393) + gradient_pool (#352).

use serde::Serialize;

/// Button shape axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ButtonShape {
    /// Rectangular with small radius.
    Rectangular,
    /// Pill (fully rounded).
    Pill,
    /// Square (icon-only).
    Square,
    /// Circle (icon-only round).
    Circle,
    /// Rounded (medium radius).
    Rounded,
}

impl ButtonShape {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Rectangular => "rectangular",
            Self::Pill => "pill",
            Self::Square => "square",
            Self::Circle => "circle",
            Self::Rounded => "rounded",
        }
    }
}

/// Button fill / surface treatment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ButtonFill {
    /// Filled with brand color.
    Filled,
    /// Outlined (border only, transparent fill).
    Outlined,
    /// Ghost (no border, no fill, just label).
    Ghost,
    /// Link-style (underlined label only).
    Link,
    /// Gradient background.
    Gradient,
    /// Soft tint (faded brand color bg).
    SoftTint,
}

impl ButtonFill {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Filled => "filled",
            Self::Outlined => "outlined",
            Self::Ghost => "ghost",
            Self::Link => "link",
            Self::Gradient => "gradient",
            Self::SoftTint => "soft_tint",
        }
    }
}

/// Button hover effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ButtonHover {
    /// No motion.
    None,
    /// Lift with shadow on hover.
    Lift,
    /// Scale (105% transform).
    Scale,
    /// Brighten / darken bg color.
    Brighten,
    /// Underline (link-style).
    Underline,
    /// Slide-in icon.
    SlideIcon,
}

impl ButtonHover {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Lift => "lift",
            Self::Scale => "scale",
            Self::Brighten => "brighten",
            Self::Underline => "underline",
            Self::SlideIcon => "slide_icon",
        }
    }
}

/// Icon placement on the button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum IconPlacement {
    /// No icon.
    None,
    /// Icon precedes label.
    Leading,
    /// Icon follows label.
    Trailing,
    /// Icon-only (no label, only icon visible; label is sr-only).
    IconOnly,
}

impl IconPlacement {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Leading => "leading",
            Self::Trailing => "trailing",
            Self::IconOnly => "icon_only",
        }
    }
}

/// One button-treatment pool entry.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct ButtonStyle {
    /// Stable slug.
    pub slug: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Shape.
    pub shape: ButtonShape,
    /// Fill treatment.
    pub fill: ButtonFill,
    /// Hover effect.
    pub hover: ButtonHover,
    /// Icon placement.
    pub icon_placement: IconPlacement,
    /// PageKinds this entry suits. Empty = universal.
    pub page_kind_fit: &'static [&'static str],
}

/// Canonical button pool — 25 hand-curated entries.
pub const BUTTON_POOL: &[ButtonStyle] = &[
    ButtonStyle {
        slug: "classic-filled-rect",
        name: "Classic — filled rectangular",
        shape: ButtonShape::Rectangular,
        fill: ButtonFill::Filled,
        hover: ButtonHover::Brighten,
        icon_placement: IconPlacement::None,
        page_kind_fit: &[],
    },
    ButtonStyle {
        slug: "classic-rounded-filled",
        name: "Classic — rounded filled",
        shape: ButtonShape::Rounded,
        fill: ButtonFill::Filled,
        hover: ButtonHover::Brighten,
        icon_placement: IconPlacement::None,
        page_kind_fit: &[],
    },
    ButtonStyle {
        slug: "outlined-rect",
        name: "Outlined rectangular",
        shape: ButtonShape::Rectangular,
        fill: ButtonFill::Outlined,
        hover: ButtonHover::Brighten,
        icon_placement: IconPlacement::None,
        page_kind_fit: &[],
    },
    ButtonStyle {
        slug: "outlined-rounded",
        name: "Outlined rounded",
        shape: ButtonShape::Rounded,
        fill: ButtonFill::Outlined,
        hover: ButtonHover::Brighten,
        icon_placement: IconPlacement::None,
        page_kind_fit: &[],
    },
    ButtonStyle {
        slug: "ghost-link",
        name: "Ghost — link style",
        shape: ButtonShape::Rectangular,
        fill: ButtonFill::Ghost,
        hover: ButtonHover::Underline,
        icon_placement: IconPlacement::None,
        page_kind_fit: &["brief", "editorial"],
    },
    ButtonStyle {
        slug: "link-underlined",
        name: "Link — underlined inline",
        shape: ButtonShape::Rectangular,
        fill: ButtonFill::Link,
        hover: ButtonHover::Underline,
        icon_placement: IconPlacement::None,
        page_kind_fit: &["brief", "editorial", "documentation"],
    },
    ButtonStyle {
        slug: "link-trailing-arrow",
        name: "Link — trailing arrow",
        shape: ButtonShape::Rectangular,
        fill: ButtonFill::Link,
        hover: ButtonHover::SlideIcon,
        icon_placement: IconPlacement::Trailing,
        page_kind_fit: &["editorial"],
    },
    ButtonStyle {
        slug: "pill-filled",
        name: "Pill — filled",
        shape: ButtonShape::Pill,
        fill: ButtonFill::Filled,
        hover: ButtonHover::Lift,
        icon_placement: IconPlacement::None,
        page_kind_fit: &["marketing_landing"],
    },
    ButtonStyle {
        slug: "pill-outlined",
        name: "Pill — outlined",
        shape: ButtonShape::Pill,
        fill: ButtonFill::Outlined,
        hover: ButtonHover::Brighten,
        icon_placement: IconPlacement::None,
        page_kind_fit: &["marketing_landing", "portfolio"],
    },
    ButtonStyle {
        slug: "pill-soft-tint",
        name: "Pill — soft-tint bg",
        shape: ButtonShape::Pill,
        fill: ButtonFill::SoftTint,
        hover: ButtonHover::Brighten,
        icon_placement: IconPlacement::None,
        page_kind_fit: &[],
    },
    ButtonStyle {
        slug: "gradient-saas",
        name: "Gradient — SaaS modern",
        shape: ButtonShape::Rounded,
        fill: ButtonFill::Gradient,
        hover: ButtonHover::Lift,
        icon_placement: IconPlacement::None,
        page_kind_fit: &["marketing_landing"],
    },
    ButtonStyle {
        slug: "gradient-pill",
        name: "Gradient pill — playful",
        shape: ButtonShape::Pill,
        fill: ButtonFill::Gradient,
        hover: ButtonHover::Scale,
        icon_placement: IconPlacement::None,
        page_kind_fit: &["marketing_landing"],
    },
    ButtonStyle {
        slug: "filled-with-arrow",
        name: "Filled + trailing arrow icon",
        shape: ButtonShape::Rounded,
        fill: ButtonFill::Filled,
        hover: ButtonHover::SlideIcon,
        icon_placement: IconPlacement::Trailing,
        page_kind_fit: &[],
    },
    ButtonStyle {
        slug: "filled-with-leading-icon",
        name: "Filled + leading icon",
        shape: ButtonShape::Rectangular,
        fill: ButtonFill::Filled,
        hover: ButtonHover::Brighten,
        icon_placement: IconPlacement::Leading,
        page_kind_fit: &[],
    },
    ButtonStyle {
        slug: "icon-only-square",
        name: "Icon-only square",
        shape: ButtonShape::Square,
        fill: ButtonFill::Ghost,
        hover: ButtonHover::Brighten,
        icon_placement: IconPlacement::IconOnly,
        page_kind_fit: &[],
    },
    ButtonStyle {
        slug: "icon-only-circle",
        name: "Icon-only circle",
        shape: ButtonShape::Circle,
        fill: ButtonFill::Filled,
        hover: ButtonHover::Lift,
        icon_placement: IconPlacement::IconOnly,
        page_kind_fit: &[],
    },
    ButtonStyle {
        slug: "civic-filled-no-radius",
        name: "Civic — filled, no radius",
        shape: ButtonShape::Rectangular,
        fill: ButtonFill::Filled,
        hover: ButtonHover::Brighten,
        icon_placement: IconPlacement::None,
        page_kind_fit: &["civic"],
    },
    ButtonStyle {
        slug: "civic-secondary-outlined",
        name: "Civic — secondary outlined",
        shape: ButtonShape::Rectangular,
        fill: ButtonFill::Outlined,
        hover: ButtonHover::Brighten,
        icon_placement: IconPlacement::None,
        page_kind_fit: &["civic"],
    },
    ButtonStyle {
        slug: "editorial-text-arrow",
        name: "Editorial — text + arrow link",
        shape: ButtonShape::Rectangular,
        fill: ButtonFill::Link,
        hover: ButtonHover::SlideIcon,
        icon_placement: IconPlacement::Trailing,
        page_kind_fit: &["editorial"],
    },
    ButtonStyle {
        slug: "editorial-pill-soft",
        name: "Editorial — pill soft-tint",
        shape: ButtonShape::Pill,
        fill: ButtonFill::SoftTint,
        hover: ButtonHover::Brighten,
        icon_placement: IconPlacement::None,
        page_kind_fit: &["editorial"],
    },
    ButtonStyle {
        slug: "portfolio-ghost-large",
        name: "Portfolio — large ghost CTA",
        shape: ButtonShape::Rectangular,
        fill: ButtonFill::Ghost,
        hover: ButtonHover::Brighten,
        icon_placement: IconPlacement::Trailing,
        page_kind_fit: &["portfolio"],
    },
    ButtonStyle {
        slug: "docs-secondary-link",
        name: "Docs — secondary link",
        shape: ButtonShape::Rectangular,
        fill: ButtonFill::Link,
        hover: ButtonHover::Underline,
        icon_placement: IconPlacement::Trailing,
        page_kind_fit: &["documentation"],
    },
    ButtonStyle {
        slug: "docs-outlined-secondary",
        name: "Docs — outlined secondary",
        shape: ButtonShape::Rounded,
        fill: ButtonFill::Outlined,
        hover: ButtonHover::Brighten,
        icon_placement: IconPlacement::None,
        page_kind_fit: &["documentation"],
    },
    ButtonStyle {
        slug: "brief-text-link",
        name: "Brief — text link only",
        shape: ButtonShape::Rectangular,
        fill: ButtonFill::Link,
        hover: ButtonHover::Underline,
        icon_placement: IconPlacement::None,
        page_kind_fit: &["brief"],
    },
    ButtonStyle {
        slug: "minimal-ghost",
        name: "Minimal — ghost no-icon",
        shape: ButtonShape::Rectangular,
        fill: ButtonFill::Ghost,
        hover: ButtonHover::Brighten,
        icon_placement: IconPlacement::None,
        page_kind_fit: &["brief", "editorial"],
    },
];

/// All button entries.
#[must_use]
pub fn all_buttons() -> &'static [ButtonStyle] {
    BUTTON_POOL
}

/// Entries appropriate for a PageKind.
#[must_use]
pub fn buttons_for(page_kind: &str) -> Vec<&'static ButtonStyle> {
    BUTTON_POOL
        .iter()
        .filter(|b| b.page_kind_fit.is_empty() || b.page_kind_fit.contains(&page_kind))
        .collect()
}

/// Deterministic identity-aware selection.
#[must_use]
pub fn select_button(
    identity_hash: u64,
    page_kind: &str,
) -> Option<&'static ButtonStyle> {
    let candidates = buttons_for(page_kind);
    if candidates.is_empty() {
        return None;
    }
    let idx = (identity_hash as usize) % candidates.len();
    Some(candidates[idx])
}

/// Look up by slug.
#[must_use]
pub fn get_button(slug: &str) -> Option<&'static ButtonStyle> {
    BUTTON_POOL.iter().find(|b| b.slug == slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_size_in_target_range() {
        let n = BUTTON_POOL.len();
        assert!(n >= 20 && n <= 30, "pool size {} out of 20..=30", n);
    }

    #[test]
    fn slugs_unique() {
        let mut slugs: Vec<&str> = BUTTON_POOL.iter().map(|b| b.slug).collect();
        slugs.sort_unstable();
        let original = slugs.len();
        slugs.dedup();
        assert_eq!(slugs.len(), original);
    }

    #[test]
    fn buttons_for_brief_excludes_saas() {
        let brief = buttons_for("brief");
        assert!(brief.iter().all(|b| b.slug != "gradient-saas"));
        assert!(brief.iter().any(|b| b.slug == "brief-text-link"));
    }

    #[test]
    fn universals_in_every_kind() {
        for kind in [
            "marketing_landing",
            "brief",
            "editorial",
            "civic",
            "documentation",
            "portfolio",
        ] {
            let pool = buttons_for(kind);
            assert!(pool.iter().any(|b| b.slug == "classic-filled-rect"));
        }
    }

    #[test]
    fn select_deterministic() {
        let a = select_button(42, "marketing_landing").unwrap();
        let b = select_button(42, "marketing_landing").unwrap();
        assert_eq!(a.slug, b.slug);
    }

    #[test]
    fn select_disperses() {
        let a = select_button(0, "marketing_landing").unwrap();
        let b = select_button(7, "marketing_landing").unwrap();
        let c = select_button(13, "marketing_landing").unwrap();
        assert!(!(a.slug == b.slug && b.slug == c.slug));
    }

    #[test]
    fn get_button_finds_known() {
        assert!(get_button("classic-filled-rect").is_some());
        assert!(get_button("nonexistent").is_none());
    }

    #[test]
    fn axis_slugs_stable() {
        assert_eq!(ButtonShape::Pill.slug(), "pill");
        assert_eq!(ButtonFill::Outlined.slug(), "outlined");
        assert_eq!(ButtonHover::Lift.slug(), "lift");
        assert_eq!(IconPlacement::Leading.slug(), "leading");
    }
}
