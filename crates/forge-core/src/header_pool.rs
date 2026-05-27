//! `header_pool` — hand-curated pool of header-style variants
//! with identity-aware deterministic selection.
//!
//! Per task #392 (default-fragmentation pool 2 of 5). Default-
//! fragmentation pools fight substrate-band collapse: instead of
//! every tenant defaulting to the same header style, the
//! substrate ships ~25 distinct variants and selects per tenant
//! identity. Two tenants with the same theme produce visibly
//! different headers because their identity hashes pull different
//! pool entries.
//!
//! ## Pool design axes
//!
//! Each entry combines orthogonal axes:
//! - layout: where the logo + nav live spatially
//! - accent: visual treatment (rule, gradient, plain)
//! - density: compact / comfortable / loose
//! - mobile_collapse: how it behaves on narrow viewports
//!
//! Hand-curated combinations (not all combinations are useful;
//! the curator's job is to drop the bad pairings + ship only the
//! good ones).
//!
//! ## Selection
//!
//! `select_header(identity_hash, page_kind)` returns one
//! HeaderStyle. Deterministic: same inputs → same output.
//! PageKind constrains the candidate set (brief pages don't get
//! mega-menu styles; civic pages don't get gradient banners).

use serde::Serialize;

/// Header layout axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum HeaderLayout {
    /// Logo left, nav right (most common).
    LogoLeftNavRight,
    /// Logo centered with nav split on either side.
    LogoCenterNavSplit,
    /// Logo only; no nav (brief / portfolio).
    LogoOnly,
    /// Floating capsule pill with logo + nav inside.
    FloatingPill,
    /// Stacked: logo on top row, nav on second row.
    Stacked,
    /// Sidebar: vertical nav rail on left.
    Sidebar,
    /// Minimal: just a single line of text-only nav links.
    Minimal,
}

impl HeaderLayout {
    /// Stable kebab/snake-case slug for this axis variant.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::LogoLeftNavRight => "logo_left_nav_right",
            Self::LogoCenterNavSplit => "logo_center_nav_split",
            Self::LogoOnly => "logo_only",
            Self::FloatingPill => "floating_pill",
            Self::Stacked => "stacked",
            Self::Sidebar => "sidebar",
            Self::Minimal => "minimal",
        }
    }
}

/// Header accent axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum HeaderAccent {
    /// No accent; transparent / inherits page bg.
    None,
    /// Thin bottom rule.
    BottomRule,
    /// Soft drop-shadow on scroll.
    ScrollShadow,
    /// Gradient background.
    GradientBg,
    /// Solid brand-color background.
    SolidBg,
    /// Glass-morphism (blur + transparency).
    Glass,
    /// Top accent bar (1-2px brand color).
    TopBar,
}

impl HeaderAccent {
    /// Stable kebab/snake-case slug for this axis variant.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::BottomRule => "bottom_rule",
            Self::ScrollShadow => "scroll_shadow",
            Self::GradientBg => "gradient_bg",
            Self::SolidBg => "solid_bg",
            Self::Glass => "glass",
            Self::TopBar => "top_bar",
        }
    }
}

/// Header density axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum HeaderDensity {
    /// Tight: 56px / 8px padding.
    Compact,
    /// 72px / 16px padding (default).
    Comfortable,
    /// 96px / 24px padding.
    Loose,
}

impl HeaderDensity {
    /// Stable kebab/snake-case slug for this axis variant.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Comfortable => "comfortable",
            Self::Loose => "loose",
        }
    }
}

/// Mobile collapse strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MobileCollapse {
    /// Hamburger menu.
    Hamburger,
    /// Bottom tab bar on mobile.
    BottomTabs,
    /// Slide-out drawer.
    Drawer,
    /// No mobile-specific behavior (always-visible).
    None,
}

impl MobileCollapse {
    /// Stable kebab/snake-case slug for this axis variant.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Hamburger => "hamburger",
            Self::BottomTabs => "bottom_tabs",
            Self::Drawer => "drawer",
            Self::None => "none",
        }
    }
}

/// One header-style pool entry.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct HeaderStyle {
    /// Stable slug.
    pub slug: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Layout axis.
    pub layout: HeaderLayout,
    /// Accent axis.
    pub accent: HeaderAccent,
    /// Density axis.
    pub density: HeaderDensity,
    /// Mobile-collapse strategy.
    pub mobile_collapse: MobileCollapse,
    /// PageKinds this entry is appropriate for. Empty = any.
    pub page_kind_fit: &'static [&'static str],
}

/// Canonical header pool. 25 hand-curated entries.
pub const HEADER_POOL: &[HeaderStyle] = &[
    HeaderStyle {
        slug: "classic-left-nav",
        name: "Classic — logo left, nav right",
        layout: HeaderLayout::LogoLeftNavRight,
        accent: HeaderAccent::BottomRule,
        density: HeaderDensity::Comfortable,
        mobile_collapse: MobileCollapse::Hamburger,
        page_kind_fit: &[],
    },
    HeaderStyle {
        slug: "classic-no-rule",
        name: "Classic — no rule, scroll shadow",
        layout: HeaderLayout::LogoLeftNavRight,
        accent: HeaderAccent::ScrollShadow,
        density: HeaderDensity::Comfortable,
        mobile_collapse: MobileCollapse::Hamburger,
        page_kind_fit: &[],
    },
    HeaderStyle {
        slug: "centered-nav-split",
        name: "Centered logo — nav split",
        layout: HeaderLayout::LogoCenterNavSplit,
        accent: HeaderAccent::BottomRule,
        density: HeaderDensity::Comfortable,
        mobile_collapse: MobileCollapse::Hamburger,
        page_kind_fit: &["editorial", "portfolio"],
    },
    HeaderStyle {
        slug: "centered-loose",
        name: "Centered logo — loose density",
        layout: HeaderLayout::LogoCenterNavSplit,
        accent: HeaderAccent::None,
        density: HeaderDensity::Loose,
        mobile_collapse: MobileCollapse::Drawer,
        page_kind_fit: &["editorial"],
    },
    HeaderStyle {
        slug: "logo-only-brief",
        name: "Logo only — brief register",
        layout: HeaderLayout::LogoOnly,
        accent: HeaderAccent::None,
        density: HeaderDensity::Compact,
        mobile_collapse: MobileCollapse::None,
        page_kind_fit: &["brief"],
    },
    HeaderStyle {
        slug: "logo-only-portfolio",
        name: "Logo only — portfolio register",
        layout: HeaderLayout::LogoOnly,
        accent: HeaderAccent::BottomRule,
        density: HeaderDensity::Comfortable,
        mobile_collapse: MobileCollapse::None,
        page_kind_fit: &["portfolio"],
    },
    HeaderStyle {
        slug: "floating-pill-saas",
        name: "Floating pill — SaaS modern",
        layout: HeaderLayout::FloatingPill,
        accent: HeaderAccent::Glass,
        density: HeaderDensity::Compact,
        mobile_collapse: MobileCollapse::Drawer,
        page_kind_fit: &["marketing_landing"],
    },
    HeaderStyle {
        slug: "floating-pill-comfy",
        name: "Floating pill — comfortable density",
        layout: HeaderLayout::FloatingPill,
        accent: HeaderAccent::Glass,
        density: HeaderDensity::Comfortable,
        mobile_collapse: MobileCollapse::Hamburger,
        page_kind_fit: &["marketing_landing", "portfolio"],
    },
    HeaderStyle {
        slug: "stacked-publication",
        name: "Stacked — publication register",
        layout: HeaderLayout::Stacked,
        accent: HeaderAccent::BottomRule,
        density: HeaderDensity::Loose,
        mobile_collapse: MobileCollapse::Hamburger,
        page_kind_fit: &["editorial"],
    },
    HeaderStyle {
        slug: "stacked-docs",
        name: "Stacked — docs nav rows",
        layout: HeaderLayout::Stacked,
        accent: HeaderAccent::ScrollShadow,
        density: HeaderDensity::Compact,
        mobile_collapse: MobileCollapse::Drawer,
        page_kind_fit: &["documentation"],
    },
    HeaderStyle {
        slug: "sidebar-docs",
        name: "Sidebar nav — docs",
        layout: HeaderLayout::Sidebar,
        accent: HeaderAccent::None,
        density: HeaderDensity::Compact,
        mobile_collapse: MobileCollapse::Drawer,
        page_kind_fit: &["documentation"],
    },
    HeaderStyle {
        slug: "sidebar-app",
        name: "Sidebar nav — app shell",
        layout: HeaderLayout::Sidebar,
        accent: HeaderAccent::BottomRule,
        density: HeaderDensity::Comfortable,
        mobile_collapse: MobileCollapse::BottomTabs,
        page_kind_fit: &[],
    },
    HeaderStyle {
        slug: "minimal-text",
        name: "Minimal text-only links",
        layout: HeaderLayout::Minimal,
        accent: HeaderAccent::None,
        density: HeaderDensity::Compact,
        mobile_collapse: MobileCollapse::None,
        page_kind_fit: &["brief", "editorial"],
    },
    HeaderStyle {
        slug: "minimal-compact",
        name: "Minimal compact + bottom rule",
        layout: HeaderLayout::Minimal,
        accent: HeaderAccent::BottomRule,
        density: HeaderDensity::Compact,
        mobile_collapse: MobileCollapse::None,
        page_kind_fit: &["brief"],
    },
    HeaderStyle {
        slug: "civic-gov-top-bar",
        name: "Civic — top accent bar",
        layout: HeaderLayout::LogoLeftNavRight,
        accent: HeaderAccent::TopBar,
        density: HeaderDensity::Comfortable,
        mobile_collapse: MobileCollapse::Hamburger,
        page_kind_fit: &["civic"],
    },
    HeaderStyle {
        slug: "civic-solid-bg",
        name: "Civic — solid brand-color bg",
        layout: HeaderLayout::LogoLeftNavRight,
        accent: HeaderAccent::SolidBg,
        density: HeaderDensity::Comfortable,
        mobile_collapse: MobileCollapse::Hamburger,
        page_kind_fit: &["civic"],
    },
    HeaderStyle {
        slug: "saas-gradient",
        name: "SaaS — gradient bg",
        layout: HeaderLayout::LogoLeftNavRight,
        accent: HeaderAccent::GradientBg,
        density: HeaderDensity::Compact,
        mobile_collapse: MobileCollapse::Hamburger,
        page_kind_fit: &["marketing_landing"],
    },
    HeaderStyle {
        slug: "saas-glass",
        name: "SaaS — glass-morphism sticky",
        layout: HeaderLayout::LogoLeftNavRight,
        accent: HeaderAccent::Glass,
        density: HeaderDensity::Compact,
        mobile_collapse: MobileCollapse::Drawer,
        page_kind_fit: &["marketing_landing"],
    },
    HeaderStyle {
        slug: "magazine-loose",
        name: "Magazine — loose layout, no accent",
        layout: HeaderLayout::LogoCenterNavSplit,
        accent: HeaderAccent::None,
        density: HeaderDensity::Loose,
        mobile_collapse: MobileCollapse::Hamburger,
        page_kind_fit: &["editorial"],
    },
    HeaderStyle {
        slug: "ecommerce-classic",
        name: "Commerce — logo + nav + cart",
        layout: HeaderLayout::LogoLeftNavRight,
        accent: HeaderAccent::BottomRule,
        density: HeaderDensity::Comfortable,
        mobile_collapse: MobileCollapse::BottomTabs,
        page_kind_fit: &[],
    },
    HeaderStyle {
        slug: "portfolio-only-name",
        name: "Portfolio — name only",
        layout: HeaderLayout::Minimal,
        accent: HeaderAccent::None,
        density: HeaderDensity::Loose,
        mobile_collapse: MobileCollapse::None,
        page_kind_fit: &["portfolio"],
    },
    HeaderStyle {
        slug: "brief-borderless",
        name: "Brief — borderless minimal",
        layout: HeaderLayout::LogoOnly,
        accent: HeaderAccent::None,
        density: HeaderDensity::Compact,
        mobile_collapse: MobileCollapse::None,
        page_kind_fit: &["brief"],
    },
    HeaderStyle {
        slug: "docs-three-tier",
        name: "Docs — 3-tier nav stack",
        layout: HeaderLayout::Stacked,
        accent: HeaderAccent::BottomRule,
        density: HeaderDensity::Compact,
        mobile_collapse: MobileCollapse::Drawer,
        page_kind_fit: &["documentation"],
    },
    HeaderStyle {
        slug: "magazine-stacked",
        name: "Magazine — stacked logo + section",
        layout: HeaderLayout::Stacked,
        accent: HeaderAccent::None,
        density: HeaderDensity::Loose,
        mobile_collapse: MobileCollapse::Drawer,
        page_kind_fit: &["editorial"],
    },
    HeaderStyle {
        slug: "civic-minimal",
        name: "Civic — minimal text-only",
        layout: HeaderLayout::Minimal,
        accent: HeaderAccent::TopBar,
        density: HeaderDensity::Compact,
        mobile_collapse: MobileCollapse::Hamburger,
        page_kind_fit: &["civic"],
    },
];

/// Return all header pool entries.
#[must_use]
pub fn all_headers() -> &'static [HeaderStyle] {
    HEADER_POOL
}

/// Filter pool entries appropriate for a PageKind. Entries with
/// empty page_kind_fit (universal) are always included.
#[must_use]
pub fn headers_for(page_kind: &str) -> Vec<&'static HeaderStyle> {
    HEADER_POOL
        .iter()
        .filter(|h| h.page_kind_fit.is_empty() || h.page_kind_fit.contains(&page_kind))
        .collect()
}

/// Deterministic identity-aware selection. Same inputs → same
/// output. Hashes a simple combination of identity_hash + page_kind
/// to pick an index into the page-kind-filtered candidate list.
#[must_use]
pub fn select_header(
    identity_hash: u64,
    page_kind: &str,
) -> Option<&'static HeaderStyle> {
    let candidates = headers_for(page_kind);
    if candidates.is_empty() {
        return None;
    }
    let idx = (identity_hash as usize) % candidates.len();
    Some(candidates[idx])
}

/// Look up by slug.
#[must_use]
pub fn get_header(slug: &str) -> Option<&'static HeaderStyle> {
    HEADER_POOL.iter().find(|h| h.slug == slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_size_in_target_range() {
        // Task spec calls for 20-30 hand-curated entries.
        let n = HEADER_POOL.len();
        assert!(n >= 20 && n <= 30, "pool size {} out of 20..=30", n);
    }

    #[test]
    fn slugs_unique() {
        let mut slugs: Vec<&str> = HEADER_POOL.iter().map(|h| h.slug).collect();
        slugs.sort_unstable();
        let original = slugs.len();
        slugs.dedup();
        assert_eq!(slugs.len(), original);
    }

    #[test]
    fn headers_for_brief_filters() {
        let brief = headers_for("brief");
        assert!(brief.iter().any(|h| h.slug == "logo-only-brief"));
        // Should NOT include saas-gradient.
        assert!(brief.iter().all(|h| h.slug != "saas-gradient"));
    }

    #[test]
    fn headers_for_universal_included() {
        // classic-left-nav has page_kind_fit=[]; should appear
        // for every PageKind.
        for kind in [
            "marketing_landing",
            "brief",
            "editorial",
            "civic",
            "documentation",
            "portfolio",
        ] {
            let pool = headers_for(kind);
            assert!(
                pool.iter().any(|h| h.slug == "classic-left-nav"),
                "universal 'classic-left-nav' missing for kind {}",
                kind
            );
        }
    }

    #[test]
    fn select_header_deterministic() {
        let a = select_header(12345, "marketing_landing").unwrap();
        let b = select_header(12345, "marketing_landing").unwrap();
        assert_eq!(a.slug, b.slug);
    }

    #[test]
    fn select_header_differs_per_identity() {
        // Two different identity hashes shouldn't reliably collide,
        // even if a small pool size means some collisions exist.
        let a = select_header(0, "marketing_landing").unwrap();
        let b = select_header(7, "marketing_landing").unwrap();
        // At least one of these N pairs should differ — pool is
        // ≥3 candidates for marketing.
        let c = select_header(13, "marketing_landing").unwrap();
        let all_same = a.slug == b.slug && b.slug == c.slug;
        assert!(!all_same, "identity dispersion failed");
    }

    #[test]
    fn get_header_finds_known() {
        assert!(get_header("classic-left-nav").is_some());
        assert!(get_header("nonexistent").is_none());
    }

    #[test]
    fn axis_slugs_stable() {
        assert_eq!(HeaderLayout::LogoLeftNavRight.slug(), "logo_left_nav_right");
        assert_eq!(HeaderAccent::BottomRule.slug(), "bottom_rule");
        assert_eq!(HeaderDensity::Comfortable.slug(), "comfortable");
        assert_eq!(MobileCollapse::Hamburger.slug(), "hamburger");
    }
}
