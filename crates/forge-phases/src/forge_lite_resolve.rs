//! `forge_lite_resolve` — typed mapping from a
//! [`forge_core::forge_lite::ForgeLitePage`] to a
//! [`loom_cms_render::CmsPage`].
//!
//! Per `docs/SUBSTRATE_REFRAME_2026_05_21.md` § Accessibility
//! axis → Forge Lite diagnostic. The lite surface is the narrow
//! input contract; the render pipeline still consumes
//! `CmsPage`. This module is the seam.
//!
//! Why a separate module + crate (not on `ForgeLitePage`
//! itself): `forge-core` does not depend on
//! `loom-cms-render`. Putting the resolver in `forge-phases`
//! keeps the dependency direction one-way and lets `forge-core`
//! stay render-layer-agnostic.

use forge_core::forge_lite::{
    ForgeLitePage, ForgeLitePrimitive, ForgeLiteTheme, SpacerSize,
};
use loom_cms_render::{
    BlockSpacing, CmsBlock, CmsPage, CmsSection, HeadingLevel, HeroCta, SpotlightItem,
};

/// Resolve a Forge Lite page into a full
/// [`loom_cms_render::CmsPage`] ready for render.
///
/// The lite vocabulary is mapped onto its full-substrate
/// counterpart via the following table:
///
/// | Lite primitive       | CmsSection variant            |
/// |----------------------|-------------------------------|
/// | Hero                 | `Hero`                        |
/// | Heading              | `Heading`                     |
/// | Paragraph            | `Paragraph` (default decor)   |
/// | ImageHero            | `ImageHero` (photo backdrop)  |
/// | FeatureSpotlight     | `FeatureSpotlight`            |
/// | PullQuote            | `PullQuote`                   |
/// | CallToAction         | `CallToAction`                |
/// | LogoCloud            | `LogoCloud`                   |
/// | Divider              | `Compose{Divider}` block      |
/// | Spacer(Small/M/L)    | `Compose{Spacer}` block       |
///
/// Pure function; no I/O; deterministic.
///
/// Validates the lite page first via [`ForgeLitePage::validate`];
/// returns the typed error rather than producing a
/// half-resolved `CmsPage`.
///
/// # Errors
///
/// Returns [`forge_core::forge_lite::LiteValidationError`] if
/// the lite page fails its own validation pass (empty title,
/// bad path, heading level out of range, etc.).
pub fn resolve(
    lite: &ForgeLitePage,
) -> Result<CmsPage, forge_core::forge_lite::LiteValidationError> {
    lite.validate()?;
    let sections: Vec<CmsSection> = lite.sections.iter().map(map_primitive).collect();
    Ok(CmsPage {
        schema: None,
        title: lite.title.clone(),
        description: lite.description.clone(),
        brand: lite.brand.clone(),
        brand_logo: None,
        brand_icon_slug: None,
        brand_icon_boxed: false,
        brand_accent_tail: None,
        utility_strip: None,
        nav_bar_color_role: None,
        nav_border_role: None,
        social_links: vec![],
        lang_selector: None,
        nav_home_icon: false,
        nav_collapse_always: false,
        nav_links_align_end: false,
        nav_toggle_plain: false,
        nav_collapse_sm: false,
        hide_theme_toggle: false,
        path: normalize_path(&lite.path),
        theme: Some(map_theme(lite.theme).to_owned()),
        chrome: None,
        content_width: None,
        density: None,
        breadcrumb: Vec::new(),
        nav_actions: Vec::new(),
        nav_links: Vec::new(),
        sections,
        dev_devtools: false,
        site_origin: None,
        social_image: None,
        footer: None,
    })
}

fn map_theme(theme: ForgeLiteTheme) -> &'static str {
    theme.slug()
}

/// Normalize a lite-page path to satisfy the substrate's
/// path_consistency contract: every CMS path must end with `/`,
/// end with `.html`, or be exactly `/`.
///
/// Per docs/FORGE_LITE_DIAGNOSTIC_2026_05_22.md Category 2: lite
/// fixtures often author bare paths like `/work` or `/brief`
/// because the form is intuitive; path_consistency upstream
/// strict-fails them. The resolver normalizes here at the seam
/// so the lite contract doesn't leak substrate path conventions
/// to the operator.
///
/// Rules:
/// - Empty / missing leading `/` → caller already failed
///   ForgeLitePage::validate; resolver wouldn't reach this point
/// - Exactly `/` → unchanged
/// - Already ends with `/` → unchanged
/// - Already ends with `.html` → unchanged
/// - Otherwise → append `/`
#[must_use]
fn normalize_path(path: &str) -> String {
    if path == "/" || path.ends_with('/') || path.ends_with(".html") {
        path.to_owned()
    } else {
        format!("{path}/")
    }
}

fn cta_or_none(label: &Option<String>, href: &Option<String>) -> Option<HeroCta> {
    match (label, href) {
        (Some(l), Some(h)) if !l.trim().is_empty() && !h.trim().is_empty() => Some(HeroCta {
            label: l.clone(),
            href: h.clone(),
            data_backend: "lite-cta".to_owned(),
            icon_slug: None,
            variant: None,
        }),
        _ => None,
    }
}

fn map_heading_level(level: u8) -> HeadingLevel {
    // Validation guarantees level is 2 or 3. Fall back to 2 if
    // somehow not — defensive, never reached after
    // ForgeLitePage::validate().
    match level {
        3 => HeadingLevel::H3,
        _ => HeadingLevel::H2,
    }
}

fn map_spacer(size: SpacerSize) -> BlockSpacing {
    // Map the lite closed enum to the substrate's BlockSpacing
    // step enum. Lite caps to three stable values so the rhythm
    // scale stays predictable across Forge Lite sites.
    match size {
        SpacerSize::Small => BlockSpacing::Sm,
        SpacerSize::Medium => BlockSpacing::Md,
        SpacerSize::Large => BlockSpacing::Lg,
    }
}

fn map_primitive(p: &ForgeLitePrimitive) -> CmsSection {
    match p {
        ForgeLitePrimitive::Hero {
            eyebrow,
            title,
            lede,
            cta_label,
            cta_href,
        } => CmsSection::Hero {
            eyebrow: eyebrow.clone(),
            title: title.clone(),
            lede: lede.clone(),
            cta: cta_or_none(cta_label, cta_href),
        },
        ForgeLitePrimitive::Heading { level, text } => CmsSection::Heading {
            text: text.clone(),
            level: map_heading_level(*level),
            id: None,
            polish: Vec::new(),
        },
        ForgeLitePrimitive::Paragraph { text } => CmsSection::Paragraph {
            text: text.clone(),
            decoration: loom_cms_render::ParagraphDecoration::default(),
        },
        ForgeLitePrimitive::ImageHero {
            eyebrow,
            title,
            lede,
            photo_src,
            photo_alt,
            cta_label,
            cta_href,
        } => CmsSection::ImageHero {
            eyebrow: eyebrow.clone(),
            eyebrow_badge: None,
            title: title.clone(),
            title_accent: None,
            lede: lede.clone(),
            cta: cta_or_none(cta_label, cta_href),
            cta_secondary: None,
            background: loom_cms_render::HeroBackground::Photo {
                src: photo_src.clone(),
                alt: photo_alt.clone(),
                overlay: loom_cms_render::PhotoOverlay::default(),
            },
            height: loom_cms_render::HeroHeight::default(),
            align: loom_cms_render::HeroAlign::default(),
            before_headline: Vec::new(),
            after_cta: Vec::new(),
        },
        ForgeLitePrimitive::FeatureSpotlight {
            heading,
            columns,
            items,
        } => CmsSection::FeatureSpotlight {
            heading: Some(heading.clone()),
            lede: None,
            items: items
                .iter()
                .map(|it| SpotlightItem {
                    icon_slug: it.icon_slug.clone(),
                    eyebrow: None,
                    image_portrait: false,
                    image: None,
                    title: it.title.clone(),
                    body: it.body.clone(),
                    href: None,
                    data_backend: None,
                    body_link: None,
                    segments: None,
                    meta: None,
                })
                .collect(),
            columns: *columns,
            decoration: loom_cms_render::FeatureSpotlightDecoration::default(),
            heading_color: loom_cms_render::SpotlightHeadingColor::default(),
            border_color: loom_cms_render::SpotlightBorderColor::default(),
            border_style: loom_cms_render::SpotlightBorderStyle::default(),
            centered: false,
            numbered: false,
        },
        ForgeLitePrimitive::PullQuote {
            text,
            attribution,
        } => CmsSection::PullQuote {
            body: text.clone(),
            attribution: attribution.clone(),
            cite_url: None,
            emphasis: loom_cms_render::PullQuoteEmphasis::default(),
            tone: loom_cms_render::PullQuoteTone::default(),
        },
        ForgeLitePrimitive::CallToAction {
            title,
            lede,
            cta_label,
            cta_href,
        } => CmsSection::CallToAction {
            eyebrow: None,
            title: title.clone(),
            lede: lede.clone(),
            cta: HeroCta {
                label: cta_label.clone(),
                href: cta_href.clone(),
                data_backend: "lite-cta".to_owned(),
                icon_slug: None,
                variant: None,
            },
            background: loom_cms_render::HeroBackground::default(),
            align: loom_cms_render::HeroAlign::default(),
        },
        ForgeLitePrimitive::LogoCloud { heading, logos } => CmsSection::LogoCloud {
            heading: heading.clone(),
            items: logos.iter().map(|l| l.alt.clone()).collect(),
        },
        ForgeLitePrimitive::Divider => CmsSection::Compose {
            heading: None,
            blocks: vec![CmsBlock::Divider],
        },
        ForgeLitePrimitive::Spacer { size } => CmsSection::Compose {
            heading: None,
            blocks: vec![CmsBlock::Spacer {
                size: map_spacer(*size),
            }],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::forge_lite::{FeatureItem, LogoItem};

    #[test]
    fn resolve_empty_page_produces_empty_sections() {
        let lite = ForgeLitePage {
            title: "T".to_owned(),
            description: "D".to_owned(),
            path: "/".to_owned(),
            theme: ForgeLiteTheme::Light,
            brand: None,
            sections: Vec::new(),
        };
        let page = resolve(&lite).expect("valid empty page");
        assert!(page.sections.is_empty());
        assert_eq!(page.theme.as_deref(), Some("light"));
    }

    #[test]
    fn resolve_validation_error_propagates() {
        let lite = ForgeLitePage {
            title: "T".to_owned(),
            description: "D".to_owned(),
            path: "no-leading-slash".to_owned(),
            theme: ForgeLiteTheme::Light,
            brand: None,
            sections: Vec::new(),
        };
        assert!(resolve(&lite).is_err());
    }

    #[test]
    fn hero_round_trips_with_cta() {
        let lite = ForgeLitePage {
            title: "T".to_owned(),
            description: "D".to_owned(),
            path: "/".to_owned(),
            theme: ForgeLiteTheme::Dark,
            brand: None,
            sections: vec![ForgeLitePrimitive::Hero {
                eyebrow: Some("EB".to_owned()),
                title: "Welcome".to_owned(),
                lede: Some("Sub".to_owned()),
                cta_label: Some("Go".to_owned()),
                cta_href: Some("/x".to_owned()),
            }],
        };
        let page = resolve(&lite).expect("ok");
        match &page.sections[0] {
            CmsSection::Hero {
                eyebrow,
                title,
                lede,
                cta,
            } => {
                assert_eq!(eyebrow.as_deref(), Some("EB"));
                assert_eq!(title, "Welcome");
                assert_eq!(lede.as_deref(), Some("Sub"));
                let cta = cta.as_ref().expect("cta present");
                assert_eq!(cta.label, "Go");
                assert_eq!(cta.href, "/x");
            }
            other => panic!("expected Hero, got {other:?}"),
        }
        assert_eq!(page.theme.as_deref(), Some("dark"));
    }

    #[test]
    fn heading_levels_map() {
        for (input, expected) in [(2u8, HeadingLevel::H2), (3u8, HeadingLevel::H3)] {
            let lite = ForgeLitePage {
                title: "T".to_owned(),
                description: "D".to_owned(),
                path: "/".to_owned(),
                theme: ForgeLiteTheme::Light,
                brand: None,
                sections: vec![ForgeLitePrimitive::Heading {
                    level: input,
                    text: "x".to_owned(),
                }],
            };
            let page = resolve(&lite).expect("ok");
            match &page.sections[0] {
                CmsSection::Heading { level, .. } => assert_eq!(*level, expected),
                _ => panic!("expected Heading"),
            }
        }
    }

    #[test]
    fn feature_spotlight_items_round_trip() {
        let lite = ForgeLitePage {
            title: "T".to_owned(),
            description: "D".to_owned(),
            path: "/".to_owned(),
            theme: ForgeLiteTheme::Light,
            brand: None,
            sections: vec![ForgeLitePrimitive::FeatureSpotlight {
                heading: "Features".to_owned(),
                columns: 3,
                items: vec![
                    FeatureItem {
                        title: "A".to_owned(),
                        body: "alpha".to_owned(),
                        icon_slug: None,
                    },
                    FeatureItem {
                        title: "B".to_owned(),
                        body: "beta".to_owned(),
                        icon_slug: None,
                    },
                ],
            }],
        };
        let page = resolve(&lite).expect("ok");
        match &page.sections[0] {
            CmsSection::FeatureSpotlight {
                heading,
                items,
                columns,
                ..
            } => {
                assert_eq!(heading.as_deref(), Some("Features"));
                assert_eq!(*columns, 3);
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].title, "A");
            }
            _ => panic!("expected FeatureSpotlight"),
        }
    }

    #[test]
    fn logo_cloud_extracts_alt_text() {
        let lite = ForgeLitePage {
            title: "T".to_owned(),
            description: "D".to_owned(),
            path: "/".to_owned(),
            theme: ForgeLiteTheme::Light,
            brand: None,
            sections: vec![ForgeLitePrimitive::LogoCloud {
                heading: Some("Trusted by".to_owned()),
                logos: vec![
                    LogoItem {
                        src: "/a.svg".to_owned(),
                        alt: "Acme".to_owned(),
                    },
                    LogoItem {
                        src: "/b.svg".to_owned(),
                        alt: "Brand X".to_owned(),
                    },
                ],
            }],
        };
        let page = resolve(&lite).expect("ok");
        match &page.sections[0] {
            CmsSection::LogoCloud { heading, items } => {
                assert_eq!(heading.as_deref(), Some("Trusted by"));
                assert_eq!(items, &vec!["Acme".to_owned(), "Brand X".to_owned()]);
            }
            _ => panic!("expected LogoCloud"),
        }
    }

    #[test]
    fn spacer_size_maps_to_step() {
        let lite = ForgeLitePage {
            title: "T".to_owned(),
            description: "D".to_owned(),
            path: "/".to_owned(),
            theme: ForgeLiteTheme::Light,
            brand: None,
            sections: vec![
                ForgeLitePrimitive::Spacer { size: SpacerSize::Small },
                ForgeLitePrimitive::Spacer { size: SpacerSize::Medium },
                ForgeLitePrimitive::Spacer { size: SpacerSize::Large },
            ],
        };
        let page = resolve(&lite).expect("ok");
        let extract = |idx: usize| match &page.sections[idx] {
            CmsSection::Compose { blocks, .. } => match &blocks[0] {
                CmsBlock::Spacer { size } => Some(*size),
                _ => None,
            },
            _ => None,
        };
        assert_eq!(extract(0), Some(BlockSpacing::Sm));
        assert_eq!(extract(1), Some(BlockSpacing::Md));
        assert_eq!(extract(2), Some(BlockSpacing::Lg));
    }

    #[test]
    fn cta_omitted_when_label_or_href_missing() {
        let lite = ForgeLitePage {
            title: "T".to_owned(),
            description: "D".to_owned(),
            path: "/".to_owned(),
            theme: ForgeLiteTheme::Light,
            brand: None,
            sections: vec![ForgeLitePrimitive::Hero {
                eyebrow: None,
                title: "x".to_owned(),
                lede: None,
                cta_label: Some("Go".to_owned()),
                cta_href: None,
            }],
        };
        let page = resolve(&lite).expect("ok");
        match &page.sections[0] {
            CmsSection::Hero { cta, .. } => assert!(cta.is_none()),
            _ => panic!("expected Hero"),
        }
    }

    #[test]
    fn normalize_path_appends_trailing_slash() {
        assert_eq!(normalize_path("/work"), "/work/");
        assert_eq!(normalize_path("/brief"), "/brief/");
        assert_eq!(normalize_path("/notes/bounded-interfaces"), "/notes/bounded-interfaces/");
    }

    #[test]
    fn normalize_path_preserves_root() {
        assert_eq!(normalize_path("/"), "/");
    }

    #[test]
    fn normalize_path_preserves_trailing_slash() {
        assert_eq!(normalize_path("/work/"), "/work/");
        assert_eq!(normalize_path("/notes/x/"), "/notes/x/");
    }

    #[test]
    fn normalize_path_preserves_html_suffix() {
        assert_eq!(normalize_path("/work.html"), "/work.html");
        assert_eq!(normalize_path("/brief/index.html"), "/brief/index.html");
    }

    #[test]
    fn resolve_normalizes_path() {
        let lite = ForgeLitePage {
            title: "T".to_owned(),
            description: "D".to_owned(),
            path: "/work".to_owned(),
            theme: ForgeLiteTheme::Light,
            brand: None,
            sections: Vec::new(),
        };
        let page = resolve(&lite).expect("ok");
        assert_eq!(page.path, "/work/");
    }
}
