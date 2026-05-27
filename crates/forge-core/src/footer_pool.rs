//! `footer_pool` — hand-curated pool of footer-style variants
//! with identity-aware deterministic selection.
//!
//! Per task #393 (default-fragmentation pool 3 of 5). Sibling to
//! `header_pool` (#392) + `gradient_pool` (#352). Per-tenant
//! identity selects deterministically from 25 hand-curated
//! variants; two tenants with the same theme produce visibly
//! different footers.

use serde::Serialize;

/// Footer layout axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FooterLayout {
    /// Multi-column (3-5 columns of grouped links).
    MultiColumn,
    /// Single row of links + copyright.
    SingleRow,
    /// Minimal: only copyright line.
    Minimal,
    /// Sitemap grid (large, every page linked).
    SitemapGrid,
    /// Signature-only: brand name + copyright + maybe one link.
    SignatureOnly,
    /// Stacked: heading + columns below, vertical layout.
    Stacked,
    /// Newsletter-led: signup form is the main element.
    NewsletterLed,
}

impl FooterLayout {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::MultiColumn => "multi_column",
            Self::SingleRow => "single_row",
            Self::Minimal => "minimal",
            Self::SitemapGrid => "sitemap_grid",
            Self::SignatureOnly => "signature_only",
            Self::Stacked => "stacked",
            Self::NewsletterLed => "newsletter_led",
        }
    }
}

/// Footer content axis — what kind of content the footer surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FooterContent {
    /// Links only.
    LinksOnly,
    /// Contact information only.
    ContactOnly,
    /// Legal links only (privacy / terms).
    LegalOnly,
    /// Newsletter signup focus.
    Newsletter,
    /// Full: links + contact + legal + social.
    Full,
    /// Social links only.
    SocialOnly,
}

impl FooterContent {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::LinksOnly => "links_only",
            Self::ContactOnly => "contact_only",
            Self::LegalOnly => "legal_only",
            Self::Newsletter => "newsletter",
            Self::Full => "full",
            Self::SocialOnly => "social_only",
        }
    }
}

/// Footer density axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FooterDensity {
    /// Compact: 32px / 8px padding.
    Compact,
    /// Comfortable: 64px / 16px padding.
    Comfortable,
    /// Loose: 96px / 32px padding.
    Loose,
}

impl FooterDensity {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Comfortable => "comfortable",
            Self::Loose => "loose",
        }
    }
}

/// Footer accent axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FooterAccent {
    /// No accent.
    None,
    /// Thin top rule.
    TopRule,
    /// Gradient divider above.
    GradientDivider,
    /// Solid background tone (slate / cream / brand).
    SolidBg,
    /// Inverted: dark footer on light page.
    Inverted,
}

impl FooterAccent {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::TopRule => "top_rule",
            Self::GradientDivider => "gradient_divider",
            Self::SolidBg => "solid_bg",
            Self::Inverted => "inverted",
        }
    }
}

/// One footer-style pool entry.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct FooterStyle {
    /// Stable slug.
    pub slug: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Layout axis.
    pub layout: FooterLayout,
    /// Content axis.
    pub content: FooterContent,
    /// Density axis.
    pub density: FooterDensity,
    /// Accent axis.
    pub accent: FooterAccent,
    /// PageKinds this entry suits. Empty = universal.
    pub page_kind_fit: &'static [&'static str],
}

/// Canonical footer pool — 25 hand-curated entries.
pub const FOOTER_POOL: &[FooterStyle] = &[
    FooterStyle {
        slug: "classic-4col",
        name: "Classic — 4 columns + legal row",
        layout: FooterLayout::MultiColumn,
        content: FooterContent::Full,
        density: FooterDensity::Comfortable,
        accent: FooterAccent::TopRule,
        page_kind_fit: &[],
    },
    FooterStyle {
        slug: "classic-3col-inverted",
        name: "Classic 3-col — inverted register",
        layout: FooterLayout::MultiColumn,
        content: FooterContent::Full,
        density: FooterDensity::Comfortable,
        accent: FooterAccent::Inverted,
        page_kind_fit: &["marketing_landing"],
    },
    FooterStyle {
        slug: "single-row-links",
        name: "Single-row links + copyright",
        layout: FooterLayout::SingleRow,
        content: FooterContent::LinksOnly,
        density: FooterDensity::Compact,
        accent: FooterAccent::TopRule,
        page_kind_fit: &[],
    },
    FooterStyle {
        slug: "single-row-social",
        name: "Single-row social icons",
        layout: FooterLayout::SingleRow,
        content: FooterContent::SocialOnly,
        density: FooterDensity::Compact,
        accent: FooterAccent::None,
        page_kind_fit: &["portfolio", "editorial"],
    },
    FooterStyle {
        slug: "minimal-copyright",
        name: "Minimal — copyright line only",
        layout: FooterLayout::Minimal,
        content: FooterContent::LegalOnly,
        density: FooterDensity::Compact,
        accent: FooterAccent::None,
        page_kind_fit: &["brief", "portfolio"],
    },
    FooterStyle {
        slug: "minimal-legal-only",
        name: "Minimal — legal links only",
        layout: FooterLayout::Minimal,
        content: FooterContent::LegalOnly,
        density: FooterDensity::Compact,
        accent: FooterAccent::TopRule,
        page_kind_fit: &["brief", "civic"],
    },
    FooterStyle {
        slug: "sitemap-grid-large",
        name: "Sitemap grid — every-page linked",
        layout: FooterLayout::SitemapGrid,
        content: FooterContent::LinksOnly,
        density: FooterDensity::Loose,
        accent: FooterAccent::SolidBg,
        page_kind_fit: &["documentation"],
    },
    FooterStyle {
        slug: "sitemap-grid-compact",
        name: "Sitemap grid — compact density",
        layout: FooterLayout::SitemapGrid,
        content: FooterContent::LinksOnly,
        density: FooterDensity::Compact,
        accent: FooterAccent::TopRule,
        page_kind_fit: &["documentation"],
    },
    FooterStyle {
        slug: "signature-only-portfolio",
        name: "Signature — name + year, portfolio",
        layout: FooterLayout::SignatureOnly,
        content: FooterContent::ContactOnly,
        density: FooterDensity::Comfortable,
        accent: FooterAccent::None,
        page_kind_fit: &["portfolio"],
    },
    FooterStyle {
        slug: "signature-editorial",
        name: "Signature — publication colophon",
        layout: FooterLayout::SignatureOnly,
        content: FooterContent::Full,
        density: FooterDensity::Loose,
        accent: FooterAccent::TopRule,
        page_kind_fit: &["editorial"],
    },
    FooterStyle {
        slug: "stacked-brand-focus",
        name: "Stacked — brand heading + columns",
        layout: FooterLayout::Stacked,
        content: FooterContent::Full,
        density: FooterDensity::Loose,
        accent: FooterAccent::GradientDivider,
        page_kind_fit: &["marketing_landing"],
    },
    FooterStyle {
        slug: "stacked-cream",
        name: "Stacked — cream warm bg",
        layout: FooterLayout::Stacked,
        content: FooterContent::Full,
        density: FooterDensity::Comfortable,
        accent: FooterAccent::SolidBg,
        page_kind_fit: &["editorial"],
    },
    FooterStyle {
        slug: "newsletter-led-marketing",
        name: "Newsletter signup — marketing focus",
        layout: FooterLayout::NewsletterLed,
        content: FooterContent::Newsletter,
        density: FooterDensity::Loose,
        accent: FooterAccent::Inverted,
        page_kind_fit: &["marketing_landing"],
    },
    FooterStyle {
        slug: "newsletter-led-editorial",
        name: "Newsletter signup — editorial focus",
        layout: FooterLayout::NewsletterLed,
        content: FooterContent::Newsletter,
        density: FooterDensity::Comfortable,
        accent: FooterAccent::TopRule,
        page_kind_fit: &["editorial"],
    },
    FooterStyle {
        slug: "contact-only-portfolio",
        name: "Contact-only — portfolio register",
        layout: FooterLayout::SingleRow,
        content: FooterContent::ContactOnly,
        density: FooterDensity::Loose,
        accent: FooterAccent::None,
        page_kind_fit: &["portfolio"],
    },
    FooterStyle {
        slug: "social-grid",
        name: "Social grid",
        layout: FooterLayout::SingleRow,
        content: FooterContent::SocialOnly,
        density: FooterDensity::Comfortable,
        accent: FooterAccent::TopRule,
        page_kind_fit: &[],
    },
    FooterStyle {
        slug: "civic-govuk",
        name: "Civic — govuk-style footer",
        layout: FooterLayout::MultiColumn,
        content: FooterContent::Full,
        density: FooterDensity::Comfortable,
        accent: FooterAccent::SolidBg,
        page_kind_fit: &["civic"],
    },
    FooterStyle {
        slug: "civic-minimal",
        name: "Civic — minimal legal-only",
        layout: FooterLayout::Minimal,
        content: FooterContent::LegalOnly,
        density: FooterDensity::Compact,
        accent: FooterAccent::TopRule,
        page_kind_fit: &["civic"],
    },
    FooterStyle {
        slug: "saas-modern-3col",
        name: "SaaS — modern 3-col + gradient divider",
        layout: FooterLayout::MultiColumn,
        content: FooterContent::Full,
        density: FooterDensity::Loose,
        accent: FooterAccent::GradientDivider,
        page_kind_fit: &["marketing_landing"],
    },
    FooterStyle {
        slug: "saas-dark",
        name: "SaaS — dark inverted, 4-col",
        layout: FooterLayout::MultiColumn,
        content: FooterContent::Full,
        density: FooterDensity::Comfortable,
        accent: FooterAccent::Inverted,
        page_kind_fit: &["marketing_landing"],
    },
    FooterStyle {
        slug: "docs-nav-mirror",
        name: "Docs — mirror of nav structure",
        layout: FooterLayout::SitemapGrid,
        content: FooterContent::Full,
        density: FooterDensity::Compact,
        accent: FooterAccent::TopRule,
        page_kind_fit: &["documentation"],
    },
    FooterStyle {
        slug: "essay-borderless",
        name: "Essay footer — borderless minimal",
        layout: FooterLayout::Minimal,
        content: FooterContent::LegalOnly,
        density: FooterDensity::Compact,
        accent: FooterAccent::None,
        page_kind_fit: &["brief"],
    },
    FooterStyle {
        slug: "magazine-colophon",
        name: "Magazine — colophon + masthead",
        layout: FooterLayout::Stacked,
        content: FooterContent::Full,
        density: FooterDensity::Loose,
        accent: FooterAccent::TopRule,
        page_kind_fit: &["editorial"],
    },
    FooterStyle {
        slug: "magazine-minimal-cream",
        name: "Magazine — minimal cream signature",
        layout: FooterLayout::SignatureOnly,
        content: FooterContent::LegalOnly,
        density: FooterDensity::Loose,
        accent: FooterAccent::SolidBg,
        page_kind_fit: &["editorial"],
    },
    FooterStyle {
        slug: "portfolio-name-year",
        name: "Portfolio — name + year, no links",
        layout: FooterLayout::SignatureOnly,
        content: FooterContent::LegalOnly,
        density: FooterDensity::Loose,
        accent: FooterAccent::None,
        page_kind_fit: &["portfolio"],
    },
];

/// All footer entries.
#[must_use]
pub fn all_footers() -> &'static [FooterStyle] {
    FOOTER_POOL
}

/// Entries appropriate for a PageKind (universal entries always
/// included).
#[must_use]
pub fn footers_for(page_kind: &str) -> Vec<&'static FooterStyle> {
    FOOTER_POOL
        .iter()
        .filter(|f| f.page_kind_fit.is_empty() || f.page_kind_fit.contains(&page_kind))
        .collect()
}

/// Deterministic identity-aware selection.
#[must_use]
pub fn select_footer(
    identity_hash: u64,
    page_kind: &str,
) -> Option<&'static FooterStyle> {
    let candidates = footers_for(page_kind);
    if candidates.is_empty() {
        return None;
    }
    let idx = (identity_hash as usize) % candidates.len();
    Some(candidates[idx])
}

/// Look up by slug.
#[must_use]
pub fn get_footer(slug: &str) -> Option<&'static FooterStyle> {
    FOOTER_POOL.iter().find(|f| f.slug == slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_size_in_target_range() {
        let n = FOOTER_POOL.len();
        assert!(n >= 20 && n <= 30, "pool size {} out of 20..=30", n);
    }

    #[test]
    fn slugs_unique() {
        let mut slugs: Vec<&str> = FOOTER_POOL.iter().map(|f| f.slug).collect();
        slugs.sort_unstable();
        let original = slugs.len();
        slugs.dedup();
        assert_eq!(slugs.len(), original);
    }

    #[test]
    fn footers_for_brief_filters() {
        let brief = footers_for("brief");
        // Should NOT include sitemap-grid-large (docs-only).
        assert!(brief.iter().all(|f| f.slug != "sitemap-grid-large"));
        assert!(brief.iter().any(|f| f.slug == "minimal-copyright"));
    }

    #[test]
    fn universal_entries_appear_for_every_kind() {
        for kind in [
            "marketing_landing",
            "brief",
            "editorial",
            "civic",
            "documentation",
            "portfolio",
        ] {
            let pool = footers_for(kind);
            assert!(
                pool.iter().any(|f| f.slug == "classic-4col"),
                "universal 'classic-4col' missing for kind {}",
                kind
            );
        }
    }

    #[test]
    fn select_footer_deterministic() {
        let a = select_footer(0xDEAD_BEEF, "marketing_landing").unwrap();
        let b = select_footer(0xDEAD_BEEF, "marketing_landing").unwrap();
        assert_eq!(a.slug, b.slug);
    }

    #[test]
    fn select_footer_differs_per_identity() {
        let a = select_footer(0, "marketing_landing").unwrap();
        let b = select_footer(7, "marketing_landing").unwrap();
        let c = select_footer(13, "marketing_landing").unwrap();
        let all_same = a.slug == b.slug && b.slug == c.slug;
        assert!(!all_same, "identity dispersion failed");
    }

    #[test]
    fn get_footer_finds_known() {
        assert!(get_footer("classic-4col").is_some());
        assert!(get_footer("nonexistent").is_none());
    }

    #[test]
    fn axis_slugs_stable() {
        assert_eq!(FooterLayout::MultiColumn.slug(), "multi_column");
        assert_eq!(FooterContent::LinksOnly.slug(), "links_only");
        assert_eq!(FooterDensity::Comfortable.slug(), "comfortable");
        assert_eq!(FooterAccent::TopRule.slug(), "top_rule");
    }
}
