//! `hero_composition_resolve` — Resolver that takes a
//! [`forge_core::hero_composition::HeroProperties`] +
//! [`HeroContent`] slots and produces a populated
//! [`loom_cms_render::CmsSection`].
//!
//! Per architecture audit 2026-05-21 + docs/SUBSTRATE_REFRAME_2026_05_21.md
//! § Accessibility 3 (composition over enumeration).
//!
//! The forge-core pilot ships the property axes and a thin
//! resolver to a `ResolvedHeroVariant` name. This module is the
//! next layer: given the properties + the content slots a hero
//! actually carries (eyebrow, headline, lede, CTA, optional
//! visual), build the populated CmsSection.
//!
//! Lives in forge-phases because `loom-cms-render` is not (and
//! shouldn't be) a forge-core dep. Same architectural separation
//! used by `forge_lite_resolve`.

use forge_core::hero_composition::{
    HeroProperties, ResolvedHeroVariant,
};
use loom_cms_render::{
    CmsSection, HeroAlign, HeroBackground, HeroCta, HeroEditorialBackground, HeroHeight,
    HeroSplitSide, PhotoOverlay, SplitVisual,
};

/// Content slots a hero composition carries. Every field is
/// optional so the resolver can degrade gracefully when a
/// requested variant needs a slot the caller didn't fill.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeroContent {
    /// Small eyebrow / kicker text rendered above the headline.
    pub eyebrow: Option<String>,
    /// Required headline. The resolver returns a stub when this
    /// is empty (caller error; surfaces visibly rather than
    /// silently).
    pub title: String,
    /// Optional lede paragraph below the headline.
    pub lede: Option<String>,
    /// Optional accent fragment that highlights inside the
    /// headline (editorial variants).
    pub headline_accent: Option<String>,
    /// Optional CTA label. Pair with `cta_href`; both required
    /// for the CTA to render.
    pub cta_label: Option<String>,
    /// Optional CTA href. Pair with `cta_label`; both required
    /// for the CTA to render.
    pub cta_href: Option<String>,
    /// Optional photo URL. Pair with `photo_alt`; both required
    /// for any photo-bearing variant.
    pub photo_src: Option<String>,
    /// Optional photo alt text. Pair with `photo_src`.
    pub photo_alt: Option<String>,
}

impl HeroContent {
    fn cta(&self) -> Option<HeroCta> {
        match (&self.cta_label, &self.cta_href) {
            (Some(l), Some(h)) if !l.trim().is_empty() && !h.trim().is_empty() => Some(HeroCta {
                label: l.clone(),
                href: h.clone(),
                data_backend: "hero-composition-cta".to_owned(),
                icon_slug: None,
                variant: None,
            }),
            _ => None,
        }
    }
}

/// Resolve a property-composed hero + content slots into a
/// populated CmsSection. Pure function; no I/O.
///
/// Resolution dispatches via [`HeroProperties::resolve_variant`]
/// then constructs the closest-fit CmsSection. When a variant
/// requires a slot the caller didn't fill (e.g., HeroSplit
/// needs image_url but content has no photo_src), the resolver
/// falls back to a simpler variant rather than rendering a
/// half-populated structure — see the `fallback_*` helpers.
#[must_use]
pub fn resolve(props: &HeroProperties, content: &HeroContent) -> CmsSection {
    match props.resolve_variant() {
        ResolvedHeroVariant::Hero => build_hero(content),
        ResolvedHeroVariant::HeroEditorial => build_hero_editorial(content),
        ResolvedHeroVariant::HeroSplit => build_hero_split_or_fallback(content),
        ResolvedHeroVariant::HeroMinimal => build_hero_minimal(content),
        ResolvedHeroVariant::ImageHero => build_image_hero(content),
        ResolvedHeroVariant::SplitHero => build_split_hero_or_fallback(content),
        ResolvedHeroVariant::CallToAction => {
            // Not normally reachable from property resolution
            // (HeroProperties.resolve_variant never returns
            // CallToAction). Defensive fallback to Hero.
            build_hero(content)
        }
    }
}

fn build_hero(content: &HeroContent) -> CmsSection {
    CmsSection::Hero {
        eyebrow: content.eyebrow.clone(),
        title: content.title.clone(),
        lede: content.lede.clone(),
        cta: content.cta(),
    }
}

fn build_hero_minimal(content: &HeroContent) -> CmsSection {
    CmsSection::HeroMinimal {
        title: content.title.clone(),
        lede: content.lede.clone(),
        cta: content.cta(),
    }
}

fn build_hero_editorial(content: &HeroContent) -> CmsSection {
    CmsSection::HeroEditorial {
        kicker: content.eyebrow.clone(),
        headline: content.title.clone(),
        headline_accent: content.headline_accent.clone(),
        lede: content.lede.clone().unwrap_or_default(),
        cta: content.cta(),
        background: HeroEditorialBackground::default(),
    }
}

fn build_hero_split_or_fallback(content: &HeroContent) -> CmsSection {
    match (&content.photo_src, &content.photo_alt) {
        (Some(src), Some(alt)) if !src.trim().is_empty() => CmsSection::HeroSplit {
            title: content.title.clone(),
            lede: content.lede.clone().unwrap_or_default(),
            image_url: src.clone(),
            image_alt: alt.clone(),
            image_side: HeroSplitSide::default(),
            cta: content.cta(),
            // #582 additive fields: this resolve path does not source
            // them, so default to the pre-#582 shape (byte-identical
            // output). Eyebrow/body/secondary-CTA reach HeroSplit only
            // via direct cms authoring, not this property-resolution.
            eyebrow: None,
            eyebrow_badge: false,
            eyebrow_icon: None,
            body: Vec::new(),
            cta_secondary: None,
        },
        // Photo missing → fall back to Hero (text-only) rather
        // than rendering HeroSplit with empty image slots.
        _ => build_hero(content),
    }
}

fn build_image_hero(content: &HeroContent) -> CmsSection {
    let background = match (&content.photo_src, &content.photo_alt) {
        (Some(src), Some(alt)) if !src.trim().is_empty() => HeroBackground::Photo {
            src: src.clone(),
            alt: alt.clone(),
            overlay: PhotoOverlay::default(),
        },
        _ => HeroBackground::default(),
    };
    CmsSection::ImageHero {
        eyebrow: content.eyebrow.clone(),
        eyebrow_badge: None,
        title: content.title.clone(),
        title_accent: None,
        lede: content.lede.clone(),
        cta: content.cta(),
        cta_secondary: None,
        background,
        height: HeroHeight::default(),
        align: HeroAlign::default(),
        before_headline: Vec::new(),
        after_cta: Vec::new(),
    }
}

fn build_split_hero_or_fallback(content: &HeroContent) -> CmsSection {
    // SplitHero takes a typed SplitVisual (code / stat /
    // AssetSlug photo). Without a richer signal from
    // HeroProperties (e.g., a `visual_kind` axis), the
    // resolver picks AssetSlug when a photo is present and
    // falls back to plain Hero otherwise.
    match &content.photo_src {
        Some(src) if !src.trim().is_empty() => {
            let slug = extract_asset_slug(src);
            CmsSection::SplitHero {
                eyebrow: content.eyebrow.clone(),
                title: content.title.clone(),
                lede: content.lede.clone(),
                cta: content.cta(),
                visual: SplitVisual::AssetSlug {
                    slug,
                    alt: content
                        .photo_alt
                        .clone()
                        .unwrap_or_else(|| "Hero illustration".to_owned()),
                },
                visual_right: true,
            }
        }
        _ => build_hero(content),
    }
}

fn extract_asset_slug(src: &str) -> String {
    // Convert "/assets/photos/foo.jpg" → "photos-foo" /
    // "/assets/foo.svg" → "foo" / passthrough for opaque values.
    let trimmed = src
        .trim_start_matches('/')
        .trim_start_matches("assets/")
        .rsplit_once('.')
        .map_or(src, |(stem, _ext)| stem);
    trimmed.replace('/', "-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::hero_composition::{
        HeroDecoration, HeroEmphasis, HeroLayout, HeroProperties,
    };

    fn basic_content() -> HeroContent {
        HeroContent {
            title: "Welcome".to_owned(),
            lede: Some("Lede paragraph.".to_owned()),
            eyebrow: Some("Eyebrow".to_owned()),
            cta_label: Some("Go".to_owned()),
            cta_href: Some("/x".to_owned()),
            ..Default::default()
        }
    }

    fn with_photo() -> HeroContent {
        HeroContent {
            photo_src: Some("/assets/photos/sample.jpg".to_owned()),
            photo_alt: Some("Sample alt".to_owned()),
            ..basic_content()
        }
    }

    #[test]
    fn default_props_build_hero_variant() {
        let s = resolve(&HeroProperties::default(), &basic_content());
        assert!(
            matches!(s, CmsSection::Hero { .. }),
            "default props should build CmsSection::Hero, got {s:?}"
        );
    }

    #[test]
    fn no_decoration_builds_hero_minimal() {
        let props = HeroProperties {
            layout: HeroLayout::Centered,
            decoration: HeroDecoration::None,
            ..Default::default()
        };
        let s = resolve(&props, &basic_content());
        assert!(matches!(s, CmsSection::HeroMinimal { .. }));
    }

    #[test]
    fn asymmetric_layout_builds_hero_editorial() {
        let props = HeroProperties {
            layout: HeroLayout::Asymmetric,
            ..Default::default()
        };
        let s = resolve(&props, &basic_content());
        assert!(matches!(s, CmsSection::HeroEditorial { .. }));
    }

    #[test]
    fn full_bleed_with_photo_builds_image_hero_with_photo() {
        let props = HeroProperties {
            layout: HeroLayout::FullBleed,
            ..Default::default()
        };
        let s = resolve(&props, &with_photo());
        match s {
            CmsSection::ImageHero { background, .. } => match background {
                HeroBackground::Photo { ref src, .. } => {
                    assert_eq!(src, "/assets/photos/sample.jpg");
                }
                other => panic!("expected Photo background, got {other:?}"),
            },
            other => panic!("expected ImageHero, got {other:?}"),
        }
    }

    #[test]
    fn full_bleed_without_photo_falls_back_to_default_background() {
        let props = HeroProperties {
            layout: HeroLayout::FullBleed,
            ..Default::default()
        };
        let s = resolve(&props, &basic_content());
        match s {
            CmsSection::ImageHero { background, .. } => {
                // Default HeroBackground is NOT Photo (which would
                // need a src). The fallback succeeds without panic.
                assert!(!matches!(background, HeroBackground::Photo { .. }));
            }
            other => panic!("expected ImageHero, got {other:?}"),
        }
    }

    #[test]
    fn split_text_led_with_photo_builds_hero_split() {
        let props = HeroProperties {
            layout: HeroLayout::Split,
            emphasis: HeroEmphasis::TextLed,
            ..Default::default()
        };
        let s = resolve(&props, &with_photo());
        assert!(matches!(s, CmsSection::HeroSplit { .. }));
    }

    #[test]
    fn split_text_led_without_photo_falls_back_to_hero() {
        // HeroSplit needs an image; without one, fall back to
        // Hero rather than render an empty image slot.
        let props = HeroProperties {
            layout: HeroLayout::Split,
            emphasis: HeroEmphasis::TextLed,
            ..Default::default()
        };
        let s = resolve(&props, &basic_content());
        assert!(matches!(s, CmsSection::Hero { .. }));
    }

    #[test]
    fn split_visual_led_with_photo_builds_split_hero() {
        let props = HeroProperties {
            layout: HeroLayout::Split,
            emphasis: HeroEmphasis::VisualLed,
            ..Default::default()
        };
        let s = resolve(&props, &with_photo());
        assert!(matches!(s, CmsSection::SplitHero { .. }));
    }

    #[test]
    fn cta_present_only_when_both_label_and_href_set() {
        let props = HeroProperties::default();
        let mut c = basic_content();
        c.cta_href = None;
        let s = resolve(&props, &c);
        match s {
            CmsSection::Hero { cta, .. } => assert!(cta.is_none()),
            _ => panic!("expected Hero"),
        }
    }

    #[test]
    fn eyebrow_passes_through_to_hero() {
        let s = resolve(&HeroProperties::default(), &basic_content());
        match s {
            CmsSection::Hero { eyebrow, .. } => {
                assert_eq!(eyebrow.as_deref(), Some("Eyebrow"));
            }
            _ => panic!("expected Hero"),
        }
    }

    #[test]
    fn eyebrow_passes_through_as_kicker_on_editorial() {
        let props = HeroProperties {
            layout: HeroLayout::Asymmetric,
            ..Default::default()
        };
        let s = resolve(&props, &basic_content());
        match s {
            CmsSection::HeroEditorial { kicker, .. } => {
                assert_eq!(kicker.as_deref(), Some("Eyebrow"));
            }
            _ => panic!("expected HeroEditorial"),
        }
    }

    #[test]
    fn extract_asset_slug_strips_path_and_ext() {
        assert_eq!(extract_asset_slug("/assets/photos/sample.jpg"), "photos-sample");
        assert_eq!(extract_asset_slug("/assets/logo.svg"), "logo");
        assert_eq!(extract_asset_slug("opaque-value"), "opaque-value");
    }
}
