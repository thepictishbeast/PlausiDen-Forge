//! `hero_composition` — orthogonal-property composition pilot
//! for the Hero primitive family.
//!
//! Per architecture audit 2026-05-21 + docs/SUBSTRATE_REFRAME_2026_05_21.md
//! § Accessibility 3 (composition over enumeration).
//!
//! The Hero family currently exists as 7 enumerated CmsSection
//! variants: Hero, HeroEditorial, HeroSplit, HeroMinimal,
//! ImageHero, SplitHero, CallToAction. Each is a slightly
//! different shape with overlapping content slots (eyebrow +
//! title + lede + CTA + optional visual). Choosing the right
//! variant requires holding all 7 in working context.
//!
//! This module pilots the composition shape: instead of picking
//! one of 7 enumerated variants, callers specify a small set of
//! orthogonal properties:
//!
//! - `layout`    — Centered / Split / Asymmetric / FullBleed / Stacked
//! - `emphasis`  — TextLed / VisualLed / Balanced
//! - `density`   — Tight / Normal / Loose
//! - `decoration` — None / Subtle / Prominent / Atmospheric
//! - `motion`    — Still / Subtle / Expressive
//!
//! 5 axes × 5 / 3 / 3 / 4 / 3 values ≈ 900 combinations
//! representable from a working surface of 5 enums. Smaller
//! cognitive surface; richer expressive space.
//!
//! ## Why a pilot, not a sweep
//!
//! Refactoring 163 enumerated CmsSection variants in one go is
//! a substrate-migration event and a risk concentration. The
//! pilot lets us:
//!
//! 1. Validate the property axes are the right axes (do five
//!    properties + their values actually cover what the seven
//!    enumerated variants needed?).
//! 2. Ship a non-breaking primitive (`HeroComposed` is additive;
//!    existing CmsSection::Hero variants stay untouched).
//! 3. Measure adoption: do tenants reach for `HeroComposed`
//!    over the enumerated variants once it's available? If yes,
//!    the composition shape is winning and the pattern extends
//!    to the rest of the substrate. If no, the property axes
//!    need rework before extending.
//!
//! ## Resolution
//!
//! [`HeroProperties::resolve_variant`] returns the closest-fit
//! enumerated variant for a given property combination — the
//! pilot's first step is a thin resolver layer over the existing
//! shapes. Future iterations can short-circuit straight to
//! rendered output without going through a named variant.

use serde::{Deserialize, Serialize};

/// Layout axis — how the hero's content arranges spatially.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum HeroLayout {
    /// Single-column centered. The SaaS-canonical shape.
    #[default]
    Centered,
    /// Two-column with text on one side, visual on the other.
    Split,
    /// Two-column with deliberately unbalanced widths
    /// (editorial publication shape).
    Asymmetric,
    /// Visual spans the full viewport width; text overlays.
    FullBleed,
    /// Text + visual stacked vertically (mobile-first /
    /// portrait composition).
    Stacked,
}

/// Emphasis axis — which content carries the visual weight.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum HeroEmphasis {
    /// Headline + lede are the focal point; visual is subdued.
    #[default]
    TextLed,
    /// Visual is the focal point; text is captioning.
    VisualLed,
    /// Text and visual share the focal weight equally.
    Balanced,
}

/// Density axis — vertical / horizontal whitespace allocation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum HeroDensity {
    /// Tight — minimal padding; brutalist / technical
    /// aesthetic.
    Tight,
    /// Normal — the SaaS-baseline spacing.
    #[default]
    Normal,
    /// Loose — generous padding; editorial / luxury
    /// aesthetic.
    Loose,
}

/// Decoration axis — atmospheric / ornamental treatment.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum HeroDecoration {
    /// No decoration; raw composition.
    None,
    /// Subtle accent rule, eyebrow chip, or minor color
    /// underlay.
    #[default]
    Subtle,
    /// Prominent gradient, pattern backdrop, or accent shape.
    Prominent,
    /// Atmospheric depth (gradient mesh, ambient gradient,
    /// photographic backdrop). The most visually ambitious.
    Atmospheric,
}

/// Motion axis — how content animates on view.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum HeroMotion {
    /// No animation; static.
    #[default]
    Still,
    /// Subtle fade-in or slide-in on first paint.
    Subtle,
    /// Multi-element staggered animation; carousel-style
    /// expressive entrance.
    Expressive,
}

/// Composed hero specification.
///
/// Five orthogonal properties replace the prior seven enumerated
/// variants. The total combinatorial space (5 × 3 × 3 × 4 × 3 =
/// 540 combinations) covers a much larger expressive surface
/// than the seven variants did, while the working surface a
/// caller must hold is exactly five enums.
#[derive(
    Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub struct HeroProperties {
    /// Layout axis.
    #[serde(default)]
    pub layout: HeroLayout,
    /// Emphasis axis.
    #[serde(default)]
    pub emphasis: HeroEmphasis,
    /// Density axis.
    #[serde(default)]
    pub density: HeroDensity,
    /// Decoration axis.
    #[serde(default)]
    pub decoration: HeroDecoration,
    /// Motion axis.
    #[serde(default)]
    pub motion: HeroMotion,
}

/// Closest-fit enumerated variant for a property combination.
/// Lets the pilot ship as a thin resolver over the existing
/// Hero / HeroEditorial / HeroSplit / HeroMinimal / ImageHero /
/// SplitHero / CallToAction shapes without changing render.
///
/// The mapping is intentionally lossy — the property surface is
/// larger than the variant surface. Multiple property
/// combinations resolve to the same variant. This is fine for
/// the pilot; a future iteration can short-circuit straight to
/// render without going through a variant.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum ResolvedHeroVariant {
    /// CmsSection::Hero — the SaaS-baseline centered hero.
    Hero,
    /// CmsSection::HeroEditorial — editorial asymmetric.
    HeroEditorial,
    /// CmsSection::HeroSplit — text-and-visual split.
    HeroSplit,
    /// CmsSection::HeroMinimal — minimal-decoration centered.
    HeroMinimal,
    /// CmsSection::ImageHero — photo or gradient-mesh backdrop.
    ImageHero,
    /// CmsSection::SplitHero — typed-visual split (code /
    /// stat / asset-slug).
    SplitHero,
    /// CmsSection::CallToAction — bottom-of-page CTA band.
    CallToAction,
}

impl HeroProperties {
    /// Resolve to the closest-fit enumerated variant.
    ///
    /// Resolution rules (most-specific first):
    ///
    /// - FullBleed layout → ImageHero (full-bleed backdrop is
    ///   the canonical ImageHero shape).
    /// - Split layout + VisualLed emphasis → SplitHero (typed
    ///   visual on one side).
    /// - Split layout (other emphases) → HeroSplit.
    /// - Asymmetric layout → HeroEditorial.
    /// - Centered + Atmospheric/Prominent decoration → ImageHero.
    /// - Centered + None decoration → HeroMinimal.
    /// - Centered + other → Hero.
    /// - Stacked → Hero (treated as a centered variant).
    ///
    /// CallToAction is NOT reachable via property resolution —
    /// it's semantically distinct (bottom-of-page CTA band, not
    /// a hero). CallToAction stays as its own primitive; the
    /// HeroProperties surface is only for hero-position content.
    #[must_use]
    pub fn resolve_variant(&self) -> ResolvedHeroVariant {
        match self.layout {
            HeroLayout::FullBleed => ResolvedHeroVariant::ImageHero,
            HeroLayout::Split => match self.emphasis {
                HeroEmphasis::VisualLed => ResolvedHeroVariant::SplitHero,
                _ => ResolvedHeroVariant::HeroSplit,
            },
            HeroLayout::Asymmetric => ResolvedHeroVariant::HeroEditorial,
            HeroLayout::Centered | HeroLayout::Stacked => match self.decoration {
                HeroDecoration::Atmospheric | HeroDecoration::Prominent => {
                    ResolvedHeroVariant::ImageHero
                }
                HeroDecoration::None => ResolvedHeroVariant::HeroMinimal,
                HeroDecoration::Subtle => ResolvedHeroVariant::Hero,
            },
        }
    }

    /// Number of distinct property combinations the surface can
    /// represent. Used in tests + the audit memo to quantify
    /// the cognitive-surface reduction vs the enumerated
    /// variant count.
    #[must_use]
    pub const fn combinatorial_space() -> usize {
        // layout × emphasis × density × decoration × motion
        5 * 3 * 3 * 4 * 3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_properties_resolve_to_hero() {
        let p = HeroProperties::default();
        assert_eq!(p.resolve_variant(), ResolvedHeroVariant::Hero);
    }

    #[test]
    fn full_bleed_resolves_to_image_hero() {
        let p = HeroProperties {
            layout: HeroLayout::FullBleed,
            ..Default::default()
        };
        assert_eq!(p.resolve_variant(), ResolvedHeroVariant::ImageHero);
    }

    #[test]
    fn split_visual_led_resolves_to_split_hero() {
        let p = HeroProperties {
            layout: HeroLayout::Split,
            emphasis: HeroEmphasis::VisualLed,
            ..Default::default()
        };
        assert_eq!(p.resolve_variant(), ResolvedHeroVariant::SplitHero);
    }

    #[test]
    fn split_text_led_resolves_to_hero_split() {
        let p = HeroProperties {
            layout: HeroLayout::Split,
            emphasis: HeroEmphasis::TextLed,
            ..Default::default()
        };
        assert_eq!(p.resolve_variant(), ResolvedHeroVariant::HeroSplit);
    }

    #[test]
    fn asymmetric_resolves_to_hero_editorial() {
        let p = HeroProperties {
            layout: HeroLayout::Asymmetric,
            ..Default::default()
        };
        assert_eq!(p.resolve_variant(), ResolvedHeroVariant::HeroEditorial);
    }

    #[test]
    fn centered_no_decoration_resolves_to_hero_minimal() {
        let p = HeroProperties {
            layout: HeroLayout::Centered,
            decoration: HeroDecoration::None,
            ..Default::default()
        };
        assert_eq!(p.resolve_variant(), ResolvedHeroVariant::HeroMinimal);
    }

    #[test]
    fn centered_atmospheric_resolves_to_image_hero() {
        let p = HeroProperties {
            layout: HeroLayout::Centered,
            decoration: HeroDecoration::Atmospheric,
            ..Default::default()
        };
        assert_eq!(p.resolve_variant(), ResolvedHeroVariant::ImageHero);
    }

    #[test]
    fn centered_prominent_resolves_to_image_hero() {
        let p = HeroProperties {
            layout: HeroLayout::Centered,
            decoration: HeroDecoration::Prominent,
            ..Default::default()
        };
        assert_eq!(p.resolve_variant(), ResolvedHeroVariant::ImageHero);
    }

    #[test]
    fn stacked_layout_treated_as_centered_family() {
        // Stacked + Subtle should resolve the same as Centered
        // + Subtle (both are vertical-stack compositions
        // sharing the Hero base shape).
        let p = HeroProperties {
            layout: HeroLayout::Stacked,
            decoration: HeroDecoration::Subtle,
            ..Default::default()
        };
        assert_eq!(p.resolve_variant(), ResolvedHeroVariant::Hero);
    }

    #[test]
    fn combinatorial_space_dwarfs_variant_count() {
        // The cognitive-surface argument: 5 enumerated property
        // axes (5 + 3 + 3 + 4 + 3 = 18 enum values to hold)
        // produce ≥50× more reachable hero compositions than
        // the 7 enumerated variants. Actual: 540 / 7 ≈ 77×.
        let variants_enumerated = 7;
        assert!(
            HeroProperties::combinatorial_space() >= variants_enumerated * 50,
            "combinatorial space ({}) should be ≥50× the 7 enumerated variants",
            HeroProperties::combinatorial_space()
        );
    }

    #[test]
    fn every_variant_is_reachable_from_some_property_combination() {
        // Diagnostic invariant: each enumerated variant (except
        // CallToAction, which is semantically distinct per the
        // module docs) must be reachable from at least one
        // property combination. If a variant becomes
        // unreachable, the resolution rules need adjustment.
        use std::collections::BTreeSet;
        let mut reached: BTreeSet<ResolvedHeroVariant> = BTreeSet::new();
        for &layout in &[
            HeroLayout::Centered,
            HeroLayout::Split,
            HeroLayout::Asymmetric,
            HeroLayout::FullBleed,
            HeroLayout::Stacked,
        ] {
            for &emphasis in &[
                HeroEmphasis::TextLed,
                HeroEmphasis::VisualLed,
                HeroEmphasis::Balanced,
            ] {
                for &decoration in &[
                    HeroDecoration::None,
                    HeroDecoration::Subtle,
                    HeroDecoration::Prominent,
                    HeroDecoration::Atmospheric,
                ] {
                    let p = HeroProperties {
                        layout,
                        emphasis,
                        decoration,
                        ..Default::default()
                    };
                    reached.insert(p.resolve_variant());
                }
            }
        }
        let required: BTreeSet<ResolvedHeroVariant> = [
            ResolvedHeroVariant::Hero,
            ResolvedHeroVariant::HeroEditorial,
            ResolvedHeroVariant::HeroSplit,
            ResolvedHeroVariant::HeroMinimal,
            ResolvedHeroVariant::ImageHero,
            ResolvedHeroVariant::SplitHero,
        ]
        .into_iter()
        .collect();
        assert!(
            required.is_subset(&reached),
            "required variants {required:?} not all reachable from property combinations; reached: {reached:?}"
        );
    }

    #[test]
    fn json_wire_round_trips() {
        let p = HeroProperties {
            layout: HeroLayout::Asymmetric,
            emphasis: HeroEmphasis::Balanced,
            density: HeroDensity::Loose,
            decoration: HeroDecoration::Atmospheric,
            motion: HeroMotion::Subtle,
        };
        let json = serde_json::to_string(&p).expect("serializes");
        let back: HeroProperties = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(p, back);
    }

    #[test]
    fn empty_json_uses_all_defaults() {
        let back: HeroProperties = serde_json::from_str("{}").expect("empty object parses");
        assert_eq!(back, HeroProperties::default());
        assert_eq!(back.resolve_variant(), ResolvedHeroVariant::Hero);
    }
}
