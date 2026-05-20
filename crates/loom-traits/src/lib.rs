//! `loom-traits` — typed projection of the PlausiDen trait DAG.
//!
//! Per AVP-Doctrine `TRAIT_DAG.md`: every substrate-managed entity
//! (Loom primitive, CMS section, Forge phase, Crawler detector,
//! *-core type) declares which traits it satisfies. Traits are
//! orthogonal to Rust type identity — they live in the manifest
//! projection, audited at build time.
//!
//! 54 traits across 11 categories:
//!
//! 1. **Visibility & Lifecycle** (6) — Renderable, Visible,
//!    ClientOnly, ServerOnly, Cacheable, Streamable
//! 2. **Interaction** (5) — Interactive, Focusable,
//!    KeyboardOperable, MouseOperable, TouchOperable
//! 3. **Accessibility** (5) — ScreenReaderAccessible,
//!    ReducedMotionAware, HighContrastSupported, ColorBlindSafe,
//!    LowVisionSupported
//! 4. **Responsive** (5) — MobileFriendly, TabletFriendly,
//!    DesktopFriendly, ContainerQueryAware, OrientationAware
//! 5. **Internationalization** (4) — RTLAware, LocaleAware,
//!    NumberFormatAware, DateFormatAware
//! 6. **Theming** (4) — ThemeAware, ColorSchemePicked,
//!    DarkModeFirst, AmoledOptimized
//! 7. **Security** (5) — CspCompatible, SriVerified, NonceAware,
//!    OriginIsolated, NoEval
//! 8. **Sovereignty (PSA)** (6) — Anonymous, Private, Local,
//!    EphemeralByDefault, TorCompatible, OfflineCapable
//! 9. **Performance** (5) — CarbonBudgeted, LcpSafe, ClsStable,
//!    BundleSizeBounded, LazyLoadable
//! 10. **Reliability** (4) — PropertyTested, FuzzTested,
//!     RegressionFixtured, FailsClosed
//! 11. **Discipline** (5) — DoctrineCited, SubstrateNative,
//!     NoSiteSpecific, Manifested, Versioned
//!
//! Companion to `orient-core`: traits encode entity *capabilities*
//! ("this primitive is keyboard operable"); orientations classify
//! entities along *axes of meaning* ("this primitive's audience is
//! the end user"). Both project through the manifest.
//!
//! Per `[[substrate-traits-and-doctrine]]`: traits are typed
//! inheritance + composition; audit-enforced via Forge phases.
//!
//! Per `[[backward-compat-version-discipline]]`: enum is
//! `#[non_exhaustive]` so new traits land additively (Cat 2).
//!
//! Closes `#167 [trait-v2]`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// One of the 11 trait categories. Used by the substrate's audit
/// phases to enforce category-shaped invariants (e.g. "every Visible
/// primitive must declare at least one Responsive trait").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Category {
    /// Visibility & Lifecycle — Renderable / Visible / ClientOnly /
    /// ServerOnly / Cacheable / Streamable.
    VisibilityLifecycle,
    /// Interaction — Interactive / Focusable / KeyboardOperable /
    /// MouseOperable / TouchOperable.
    Interaction,
    /// Accessibility (a11y) — ScreenReaderAccessible /
    /// ReducedMotionAware / HighContrastSupported / ColorBlindSafe /
    /// LowVisionSupported.
    Accessibility,
    /// Responsive — MobileFriendly / TabletFriendly / DesktopFriendly /
    /// ContainerQueryAware / OrientationAware.
    Responsive,
    /// Internationalization — RTLAware / LocaleAware /
    /// NumberFormatAware / DateFormatAware.
    Internationalization,
    /// Theming — ThemeAware / ColorSchemePicked / DarkModeFirst /
    /// AmoledOptimized.
    Theming,
    /// Security — CspCompatible / SriVerified / NonceAware /
    /// OriginIsolated / NoEval.
    Security,
    /// Sovereignty (PSA — privacy / security / anonymity) —
    /// Anonymous / Private / Local / EphemeralByDefault /
    /// TorCompatible / OfflineCapable.
    Sovereignty,
    /// Performance — CarbonBudgeted / LcpSafe / ClsStable /
    /// BundleSizeBounded / LazyLoadable.
    Performance,
    /// Reliability — PropertyTested / FuzzTested /
    /// RegressionFixtured / FailsClosed.
    Reliability,
    /// Discipline — DoctrineCited / SubstrateNative / NoSiteSpecific /
    /// Manifested / Versioned.
    Discipline,
}

/// A trait a substrate entity may declare. Closed enum (54
/// variants) extensible via doctrine + capability-request per
/// `[[backward-compat-version-discipline]]`.
///
/// Stable kebab-case slugs via serde rename. Reads identically
/// across Claude / Gemini / other agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Trait {
    // ── Visibility & Lifecycle ─────────────────────────────────
    /// The entity can be rendered (any output produced).
    Renderable,
    /// Renderable + has a visual surface (HTML emission, image, etc.).
    Visible,
    /// Renders only on the client (JS-required, dynamic).
    ClientOnly,
    /// Renders only on the server (no JS expected).
    ServerOnly,
    /// Output can be cached safely (idempotent given inputs).
    Cacheable,
    /// Output can be streamed (chunked emission, no fixed total length).
    Streamable,

    // ── Interaction ─────────────────────────────────────────────
    /// User can interact with the entity (forms, buttons, links).
    Interactive,
    /// Can receive focus (tab-stop in keyboard navigation).
    Focusable,
    /// Fully usable from keyboard alone (every action has a key binding).
    KeyboardOperable,
    /// Has mouse / pointer affordances (hover, click).
    MouseOperable,
    /// Has touch affordances (tap, swipe, pinch).
    TouchOperable,

    // ── Accessibility (a11y) ───────────────────────────────────
    /// Announced correctly by screen readers (ARIA roles + labels).
    ScreenReaderAccessible,
    /// Respects prefers-reduced-motion (no auto-animation when set).
    ReducedMotionAware,
    /// Supports prefers-contrast / high-contrast color schemes.
    HighContrastSupported,
    /// Distinguishable to colorblind users (not color-only signaling).
    ColorBlindSafe,
    /// Sized + spaced for low-vision users (200%+ zoom, large hit areas).
    LowVisionSupported,

    // ── Responsive ──────────────────────────────────────────────
    /// Renders correctly at 390px viewport without horizontal overflow.
    MobileFriendly,
    /// Renders correctly at 768px viewport.
    TabletFriendly,
    /// Renders correctly at 1280px viewport.
    DesktopFriendly,
    /// Uses @container queries for primitive-internal responsiveness.
    ContainerQueryAware,
    /// Adapts to portrait / landscape orientation changes.
    OrientationAware,

    // ── Internationalization ───────────────────────────────────
    /// Correct in RTL languages (logical properties only, no left/right).
    RtlAware,
    /// Respects the active locale (date / number formatting, copy).
    LocaleAware,
    /// Formats numbers per locale (1,000.00 vs 1.000,00 vs 1 000,00).
    NumberFormatAware,
    /// Formats dates per locale (ISO 8601 + locale display variations).
    DateFormatAware,

    // ── Theming ─────────────────────────────────────────────────
    /// Adapts to declared themes (light / dark / brand packs).
    ThemeAware,
    /// Respects the user's prefers-color-scheme (light / dark).
    ColorSchemePicked,
    /// Designed dark-first (no light-mode-only assumptions).
    DarkModeFirst,
    /// AMOLED-optimized (true #000000 backgrounds when dark).
    AmoledOptimized,

    // ── Security ────────────────────────────────────────────────
    /// Compatible with strict CSP (no inline scripts / eval).
    CspCompatible,
    /// Ships with subresource integrity hashes verified.
    SriVerified,
    /// Uses CSP nonces correctly for any required inline content.
    NonceAware,
    /// Renders in an isolated origin (no third-party fetches).
    OriginIsolated,
    /// Does not use eval / new Function / unsafe-eval.
    NoEval,

    // ── Sovereignty (PSA) ──────────────────────────────────────
    /// No identifier links the entity to a person.
    Anonymous,
    /// Data never leaves the substrate without explicit consent.
    Private,
    /// Data never persists to disk (in-memory only).
    Local,
    /// Data expires per declared TTL (default ephemeral posture).
    EphemeralByDefault,
    /// Reachable over `.onion`; no clearnet linkage required.
    TorCompatible,
    /// Functions without network (Service Worker / local-first).
    OfflineCapable,

    // ── Performance ─────────────────────────────────────────────
    /// Declares a CO2e budget per invocation (per rule perf-006).
    CarbonBudgeted,
    /// Designed for fast LCP (no synchronous heavy work in critical path).
    LcpSafe,
    /// Designed for low CLS (reserved layout space; no shifting).
    ClsStable,
    /// Declares + respects an asset / bundle size budget.
    BundleSizeBounded,
    /// Loaded lazily (loading="lazy" / IntersectionObserver / etc.).
    LazyLoadable,

    // ── Reliability ─────────────────────────────────────────────
    /// Covered by proptest at every input boundary.
    PropertyTested,
    /// Covered by cargo-fuzz / afl on parsing paths.
    FuzzTested,
    /// Carries a regression-fixture corpus for the bug class it addresses.
    RegressionFixtured,
    /// Fails closed on error (deterministic baseline when augmentation
    /// fails per `[[deterministic-first-lfi-optional]]`).
    FailsClosed,

    // ── Discipline ──────────────────────────────────────────────
    /// Findings cite their AVP-Doctrine rule ids.
    DoctrineCited,
    /// Built through substrate (no hand-coded CSS/HTML/JS).
    SubstrateNative,
    /// Generic across sites (not bound to one tenant per rule prim-012).
    NoSiteSpecific,
    /// Declared in the canonical manifest projection.
    Manifested,
    /// Carries an explicit version field per VERSION_DISCIPLINE.md.
    Versioned,
}

impl Trait {
    /// Canonical kebab-case slug. Stable across versions.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            // Visibility & Lifecycle
            Self::Renderable => "renderable",
            Self::Visible => "visible",
            Self::ClientOnly => "client-only",
            Self::ServerOnly => "server-only",
            Self::Cacheable => "cacheable",
            Self::Streamable => "streamable",
            // Interaction
            Self::Interactive => "interactive",
            Self::Focusable => "focusable",
            Self::KeyboardOperable => "keyboard-operable",
            Self::MouseOperable => "mouse-operable",
            Self::TouchOperable => "touch-operable",
            // Accessibility
            Self::ScreenReaderAccessible => "screen-reader-accessible",
            Self::ReducedMotionAware => "reduced-motion-aware",
            Self::HighContrastSupported => "high-contrast-supported",
            Self::ColorBlindSafe => "color-blind-safe",
            Self::LowVisionSupported => "low-vision-supported",
            // Responsive
            Self::MobileFriendly => "mobile-friendly",
            Self::TabletFriendly => "tablet-friendly",
            Self::DesktopFriendly => "desktop-friendly",
            Self::ContainerQueryAware => "container-query-aware",
            Self::OrientationAware => "orientation-aware",
            // i18n
            Self::RtlAware => "rtl-aware",
            Self::LocaleAware => "locale-aware",
            Self::NumberFormatAware => "number-format-aware",
            Self::DateFormatAware => "date-format-aware",
            // Theming
            Self::ThemeAware => "theme-aware",
            Self::ColorSchemePicked => "color-scheme-picked",
            Self::DarkModeFirst => "dark-mode-first",
            Self::AmoledOptimized => "amoled-optimized",
            // Security
            Self::CspCompatible => "csp-compatible",
            Self::SriVerified => "sri-verified",
            Self::NonceAware => "nonce-aware",
            Self::OriginIsolated => "origin-isolated",
            Self::NoEval => "no-eval",
            // Sovereignty
            Self::Anonymous => "anonymous",
            Self::Private => "private",
            Self::Local => "local",
            Self::EphemeralByDefault => "ephemeral-by-default",
            Self::TorCompatible => "tor-compatible",
            Self::OfflineCapable => "offline-capable",
            // Performance
            Self::CarbonBudgeted => "carbon-budgeted",
            Self::LcpSafe => "lcp-safe",
            Self::ClsStable => "cls-stable",
            Self::BundleSizeBounded => "bundle-size-bounded",
            Self::LazyLoadable => "lazy-loadable",
            // Reliability
            Self::PropertyTested => "property-tested",
            Self::FuzzTested => "fuzz-tested",
            Self::RegressionFixtured => "regression-fixtured",
            Self::FailsClosed => "fails-closed",
            // Discipline
            Self::DoctrineCited => "doctrine-cited",
            Self::SubstrateNative => "substrate-native",
            Self::NoSiteSpecific => "no-site-specific",
            Self::Manifested => "manifested",
            Self::Versioned => "versioned",
        }
    }

    /// Category this trait belongs to. Used by Forge audit phases
    /// to enforce category-shaped invariants.
    #[must_use]
    pub fn category(self) -> Category {
        match self {
            Self::Renderable | Self::Visible | Self::ClientOnly | Self::ServerOnly
                | Self::Cacheable | Self::Streamable => Category::VisibilityLifecycle,
            Self::Interactive | Self::Focusable | Self::KeyboardOperable
                | Self::MouseOperable | Self::TouchOperable => Category::Interaction,
            Self::ScreenReaderAccessible | Self::ReducedMotionAware
                | Self::HighContrastSupported | Self::ColorBlindSafe
                | Self::LowVisionSupported => Category::Accessibility,
            Self::MobileFriendly | Self::TabletFriendly | Self::DesktopFriendly
                | Self::ContainerQueryAware | Self::OrientationAware => Category::Responsive,
            Self::RtlAware | Self::LocaleAware | Self::NumberFormatAware
                | Self::DateFormatAware => Category::Internationalization,
            Self::ThemeAware | Self::ColorSchemePicked | Self::DarkModeFirst
                | Self::AmoledOptimized => Category::Theming,
            Self::CspCompatible | Self::SriVerified | Self::NonceAware
                | Self::OriginIsolated | Self::NoEval => Category::Security,
            Self::Anonymous | Self::Private | Self::Local | Self::EphemeralByDefault
                | Self::TorCompatible | Self::OfflineCapable => Category::Sovereignty,
            Self::CarbonBudgeted | Self::LcpSafe | Self::ClsStable
                | Self::BundleSizeBounded | Self::LazyLoadable => Category::Performance,
            Self::PropertyTested | Self::FuzzTested | Self::RegressionFixtured
                | Self::FailsClosed => Category::Reliability,
            Self::DoctrineCited | Self::SubstrateNative | Self::NoSiteSpecific
                | Self::Manifested | Self::Versioned => Category::Discipline,
        }
    }

    /// All 54 canonical traits in stable iteration order
    /// (visibility first → discipline last). Used by audit phases
    /// to enumerate the trait surface without instantiating each.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            // Visibility & Lifecycle
            Self::Renderable, Self::Visible, Self::ClientOnly, Self::ServerOnly,
            Self::Cacheable, Self::Streamable,
            // Interaction
            Self::Interactive, Self::Focusable, Self::KeyboardOperable,
            Self::MouseOperable, Self::TouchOperable,
            // Accessibility
            Self::ScreenReaderAccessible, Self::ReducedMotionAware,
            Self::HighContrastSupported, Self::ColorBlindSafe, Self::LowVisionSupported,
            // Responsive
            Self::MobileFriendly, Self::TabletFriendly, Self::DesktopFriendly,
            Self::ContainerQueryAware, Self::OrientationAware,
            // i18n
            Self::RtlAware, Self::LocaleAware, Self::NumberFormatAware,
            Self::DateFormatAware,
            // Theming
            Self::ThemeAware, Self::ColorSchemePicked, Self::DarkModeFirst,
            Self::AmoledOptimized,
            // Security
            Self::CspCompatible, Self::SriVerified, Self::NonceAware,
            Self::OriginIsolated, Self::NoEval,
            // Sovereignty
            Self::Anonymous, Self::Private, Self::Local, Self::EphemeralByDefault,
            Self::TorCompatible, Self::OfflineCapable,
            // Performance
            Self::CarbonBudgeted, Self::LcpSafe, Self::ClsStable,
            Self::BundleSizeBounded, Self::LazyLoadable,
            // Reliability
            Self::PropertyTested, Self::FuzzTested, Self::RegressionFixtured,
            Self::FailsClosed,
            // Discipline
            Self::DoctrineCited, Self::SubstrateNative, Self::NoSiteSpecific,
            Self::Manifested, Self::Versioned,
        ]
    }

    /// Parse a trait from its canonical slug. Returns `None` for
    /// unknown / mistyped slugs — callers fail-closed.
    #[must_use]
    pub fn from_slug(s: &str) -> Option<Self> {
        Self::all().iter().copied().find(|t| t.slug() == s)
    }

    /// The 8 default-required traits for every Loom Visible
    /// primitive per AVP-Doctrine rule prim-001 + TRAIT_DAG.md
    /// § Default-required traits table.
    ///
    /// Audit phases use this set to refuse to render a primitive
    /// that omits any of these.
    #[must_use]
    pub fn loom_visible_defaults() -> &'static [Self] {
        &[
            Self::MobileFriendly,
            Self::RtlAware,
            Self::ReducedMotionAware,
            Self::ThemeAware,
            Self::NoSiteSpecific,
            Self::Manifested,
            Self::Versioned,
            Self::DoctrineCited,
        ]
    }

    /// Additional defaults for Interactive primitives (cascade
    /// adds Focusable + KeyboardOperable + ScreenReaderAccessible
    /// to the Visible defaults).
    #[must_use]
    pub fn loom_interactive_defaults() -> &'static [Self] {
        &[
            Self::Focusable,
            Self::KeyboardOperable,
            Self::ScreenReaderAccessible,
        ]
    }
}

impl std::fmt::Display for Trait {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

/// A set of traits an entity declares. Wraps `Vec<Trait>` with
/// helpers for subset / superset queries used by Forge audit
/// phases ("does this primitive satisfy the Loom Visible defaults?").
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TraitSet(pub Vec<Trait>);

impl TraitSet {
    /// Construct from a slice.
    #[must_use]
    pub fn from_slice(traits: &[Trait]) -> Self {
        Self(traits.to_vec())
    }

    /// True if every trait in `required` is declared in self.
    /// Used by audit phases to check default-required-trait
    /// satisfaction.
    #[must_use]
    pub fn contains_all(&self, required: &[Trait]) -> bool {
        required.iter().all(|t| self.0.contains(t))
    }

    /// True if self contains the trait.
    #[must_use]
    pub fn contains(&self, t: Trait) -> bool {
        self.0.contains(&t)
    }

    /// Return the traits in self that belong to `category`. Used
    /// by audit phases like "every Visible primitive declares at
    /// least one Responsive trait."
    #[must_use]
    pub fn in_category(&self, category: Category) -> Vec<Trait> {
        self.0.iter().copied().filter(|t| t.category() == category).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifty_four_canonical_traits() {
        assert_eq!(Trait::all().len(), 54);
    }

    #[test]
    fn all_unique_slugs() {
        let mut seen = std::collections::HashSet::new();
        for t in Trait::all() {
            assert!(seen.insert(t.slug()), "duplicate slug: {:?}", t.slug());
        }
        assert_eq!(seen.len(), 54);
    }

    #[test]
    fn slugs_are_kebab_case() {
        for t in Trait::all() {
            let slug = t.slug();
            assert!(!slug.is_empty());
            assert!(
                slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{t:?} slug {slug:?} not kebab-case"
            );
        }
    }

    #[test]
    fn from_slug_roundtrip() {
        for t in Trait::all() {
            assert_eq!(Trait::from_slug(t.slug()), Some(*t));
        }
    }

    #[test]
    fn from_slug_rejects_unknown() {
        assert!(Trait::from_slug("").is_none());
        assert!(Trait::from_slug("Mobile").is_none()); // uppercase
        assert!(Trait::from_slug("mobile_friendly").is_none()); // underscore
        assert!(Trait::from_slug("made-up-trait").is_none());
    }

    #[test]
    fn category_assignment_complete() {
        // Every trait belongs to exactly one category.
        for t in Trait::all() {
            let _cat = t.category();
        }
    }

    #[test]
    fn category_counts_match_design() {
        // Per TRAIT_DAG.md the 11 categories carry these counts:
        let expected = [
            (Category::VisibilityLifecycle, 6),
            (Category::Interaction, 5),
            (Category::Accessibility, 5),
            (Category::Responsive, 5),
            (Category::Internationalization, 4),
            (Category::Theming, 4),
            (Category::Security, 5),
            (Category::Sovereignty, 6),
            (Category::Performance, 5),
            (Category::Reliability, 4),
            (Category::Discipline, 5),
        ];
        let total: usize = expected.iter().map(|(_, n)| n).sum();
        assert_eq!(total, 54, "expected total category count");
        for (cat, expected_n) in expected.iter() {
            let actual = Trait::all().iter().filter(|t| t.category() == *cat).count();
            assert_eq!(actual, *expected_n, "category {cat:?} count");
        }
    }

    #[test]
    fn serde_roundtrip_canonical() {
        for t in Trait::all() {
            let json = serde_json::to_string(t).expect("serialize");
            let back: Trait = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, *t);
        }
    }

    #[test]
    fn serde_specific_slugs() {
        assert_eq!(
            serde_json::to_string(&Trait::MobileFriendly).unwrap(),
            "\"mobile-friendly\""
        );
        assert_eq!(
            serde_json::to_string(&Trait::ScreenReaderAccessible).unwrap(),
            "\"screen-reader-accessible\""
        );
        assert_eq!(
            serde_json::to_string(&Trait::AmoledOptimized).unwrap(),
            "\"amoled-optimized\""
        );
    }

    #[test]
    fn loom_visible_defaults_are_eight() {
        assert_eq!(Trait::loom_visible_defaults().len(), 8);
        for t in Trait::loom_visible_defaults() {
            // Each default must be a real trait.
            assert!(Trait::all().contains(t));
        }
    }

    #[test]
    fn loom_visible_defaults_per_design() {
        // Per TRAIT_DAG.md § Default-required traits per entity class:
        // Loom primitive (Visible) requires these eight.
        let want: std::collections::HashSet<_> = [
            Trait::MobileFriendly,
            Trait::RtlAware,
            Trait::ReducedMotionAware,
            Trait::ThemeAware,
            Trait::NoSiteSpecific,
            Trait::Manifested,
            Trait::Versioned,
            Trait::DoctrineCited,
        ].into_iter().collect();
        let have: std::collections::HashSet<_> =
            Trait::loom_visible_defaults().iter().copied().collect();
        assert_eq!(want, have);
    }

    #[test]
    fn trait_set_contains_all_matches_required_set() {
        let primitive_declares = TraitSet::from_slice(&[
            Trait::Renderable,
            Trait::Visible,
            Trait::MobileFriendly,
            Trait::RtlAware,
            Trait::ReducedMotionAware,
            Trait::ThemeAware,
            Trait::NoSiteSpecific,
            Trait::Manifested,
            Trait::Versioned,
            Trait::DoctrineCited,
        ]);
        assert!(primitive_declares.contains_all(Trait::loom_visible_defaults()));

        let incomplete = TraitSet::from_slice(&[
            Trait::MobileFriendly,
            Trait::RtlAware,
            // missing ReducedMotionAware
        ]);
        assert!(!incomplete.contains_all(Trait::loom_visible_defaults()));
    }

    #[test]
    fn trait_set_in_category_filters_correctly() {
        let set = TraitSet::from_slice(&[
            Trait::MobileFriendly,    // Responsive
            Trait::RtlAware,           // i18n
            Trait::Anonymous,          // Sovereignty
            Trait::Private,            // Sovereignty
            Trait::ThemeAware,         // Theming
        ]);
        let sov = set.in_category(Category::Sovereignty);
        assert_eq!(sov.len(), 2);
        assert!(sov.contains(&Trait::Anonymous));
        assert!(sov.contains(&Trait::Private));
    }

    #[test]
    fn trait_set_serializes_transparently() {
        // serde(transparent) means TraitSet serializes as the inner Vec.
        let set = TraitSet::from_slice(&[Trait::MobileFriendly, Trait::Versioned]);
        let json = serde_json::to_string(&set).expect("serialize");
        assert_eq!(json, "[\"mobile-friendly\",\"versioned\"]");
        let back: TraitSet = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.0, set.0);
    }
}
