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

impl Category {
    /// Canonical snake_case slug.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::VisibilityLifecycle => "visibility_lifecycle",
            Self::Interaction => "interaction",
            Self::Accessibility => "accessibility",
            Self::Responsive => "responsive",
            Self::Internationalization => "internationalization",
            Self::Theming => "theming",
            Self::Security => "security",
            Self::Sovereignty => "sovereignty",
            Self::Performance => "performance",
            Self::Reliability => "reliability",
            Self::Discipline => "discipline",
        }
    }

    /// Display-friendly title (Title Case with spacing).
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::VisibilityLifecycle => "Visibility & Lifecycle",
            Self::Interaction => "Interaction",
            Self::Accessibility => "Accessibility (a11y)",
            Self::Responsive => "Responsive",
            Self::Internationalization => "Internationalization (i18n)",
            Self::Theming => "Theming",
            Self::Security => "Security",
            Self::Sovereignty => "Sovereignty (PSA — privacy / security / anonymity)",
            Self::Performance => "Performance",
            Self::Reliability => "Reliability",
            Self::Discipline => "Discipline",
        }
    }

    /// All 11 canonical categories in stable doc-rendering order.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::VisibilityLifecycle,
            Self::Interaction,
            Self::Accessibility,
            Self::Responsive,
            Self::Internationalization,
            Self::Theming,
            Self::Security,
            Self::Sovereignty,
            Self::Performance,
            Self::Reliability,
            Self::Discipline,
        ]
    }
}

/// Render the full 54-trait catalog as a Markdown document
/// suitable for publishing at `docs.plausiden.com/traits` or
/// `PlausiDen-Forge/docs/traits-catalog.md`.
///
/// Pure projection of the typed enum + category metadata — no
/// AI involvement per `[[deterministic-first-lfi-optional]]`.
/// Same input (the enum) → same output (deterministic bytes).
///
/// Closes `#172 [trait-v7]`.
#[must_use]
pub fn render_markdown_catalog() -> String {
    let mut out = String::new();
    out.push_str("# PlausiDen trait catalog\n\n");
    out.push_str(
        "Generated from the `loom-traits` crate per AVP-Doctrine \
         `TRAIT_DAG.md`. **54 traits across 11 categories.**\n\n",
    );
    out.push_str(
        "Every substrate-managed entity (Loom primitive, CMS \
         section, Forge phase, Crawler detector, *-core type) \
         declares which traits it satisfies. Audit phases enforce \
         category-shaped invariants + default-required-trait sets \
         per entity class. See `TRAIT_DAG.md` for the design \
         rationale + entity-class default tables.\n\n",
    );
    out.push_str("---\n\n");

    // Per-category sections in stable order.
    for cat in Category::all() {
        out.push_str(&format!("## {}\n\n", cat.title()));
        let in_cat: Vec<Trait> = Trait::all()
            .iter()
            .copied()
            .filter(|t| t.category() == *cat)
            .collect();
        out.push_str(&format!(
            "_{} trait{}._\n\n",
            in_cat.len(),
            if in_cat.len() == 1 { "" } else { "s" }
        ));
        out.push_str("| Slug | Variant |\n");
        out.push_str("|------|---------|\n");
        for t in in_cat {
            out.push_str(&format!("| `{}` | `{:?}` |\n", t.slug(), t));
        }
        out.push('\n');
    }

    out.push_str("---\n\n");
    out.push_str("## Default-required trait sets\n\n");
    out.push_str(
        "Per AVP-Doctrine rule `prim-001` + `TRAIT_DAG.md` § \
         Default-required traits per entity class:\n\n",
    );

    out.push_str("### Loom primitive (Visible lineage)\n\n");
    for t in Trait::loom_visible_defaults() {
        out.push_str(&format!("- `{}` ({})\n", t.slug(), t.category().title()));
    }
    out.push('\n');

    out.push_str("### Loom primitive (Interactive lineage — additional)\n\n");
    out.push_str(
        "Cascades onto the Visible set above when the primitive declares Interactive.\n\n",
    );
    for t in Trait::loom_interactive_defaults() {
        out.push_str(&format!("- `{}` ({})\n", t.slug(), t.category().title()));
    }
    out.push('\n');

    out.push_str("---\n\n");
    out.push_str("## Cross-references\n\n");
    out.push_str(
        "- [TRAIT_DAG.md](../../PlausiDen-AVP-Doctrine/TRAIT_DAG.md) — design rationale\n",
    );
    out.push_str(
        "- [N_ORIENTATION_SUBSTRATE.md](../../PlausiDen-AVP-Doctrine/N_ORIENTATION_SUBSTRATE.md) — companion orientation system\n",
    );
    out.push_str(
        "- [MAPPING_TABLES.md](../../PlausiDen-AVP-Doctrine/MAPPING_TABLES.md) — \
         cross-orientation mappings that drive default-required selection\n",
    );
    out.push_str(
        "- [VERSION_DISCIPLINE.md](../../PlausiDen-AVP-Doctrine/VERSION_DISCIPLINE.md) — \
         trait lifecycle + additive change classification\n",
    );
    out.push_str(
        "- Source: `crates/loom-traits/src/lib.rs` — typed enum source of truth\n",
    );

    out
}

/// Entity class for the substrate's default-required-trait table
/// per `TRAIT_DAG.md` § Default-required traits per entity class.
///
/// The class determines which trait set the manifest consistency
/// check (`verify_projection`) enforces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EntityClass {
    /// Loom primitive declared as Visible. Requires the 8
    /// default-required traits returned by
    /// `Trait::loom_visible_defaults()`.
    LoomVisiblePrimitive,
    /// Loom primitive declared as both Visible and Interactive.
    /// Requires `loom_visible_defaults()` + the 3 cascade traits
    /// from `loom_interactive_defaults()`.
    LoomInteractivePrimitive,
    /// CMS section. Requires NoSiteSpecific + Manifested + Versioned.
    CmsSection,
    /// Forge audit phase. Requires DoctrineCited + PropertyTested +
    /// FailsClosed.
    ForgePhase,
    /// Crawler runtime detector. Requires Manifested + PropertyTested
    /// + RegressionFixtured.
    CrawlerDetector,
    /// `*-core` typed-surface crate. Requires Manifested + Versioned +
    /// PropertyTested + FailsClosed.
    CoreType,
}

impl EntityClass {
    /// Default-required trait set for this entity class per
    /// `TRAIT_DAG.md`. The manifest consistency check refuses any
    /// projection that omits any of these traits.
    #[must_use]
    pub fn required_traits(self) -> Vec<Trait> {
        match self {
            Self::LoomVisiblePrimitive => Trait::loom_visible_defaults().to_vec(),
            Self::LoomInteractivePrimitive => {
                let mut v = Trait::loom_visible_defaults().to_vec();
                v.extend_from_slice(Trait::loom_interactive_defaults());
                v
            }
            Self::CmsSection => vec![
                Trait::NoSiteSpecific,
                Trait::Manifested,
                Trait::Versioned,
            ],
            Self::ForgePhase => vec![
                Trait::DoctrineCited,
                Trait::PropertyTested,
                Trait::FailsClosed,
            ],
            Self::CrawlerDetector => vec![
                Trait::Manifested,
                Trait::PropertyTested,
                Trait::RegressionFixtured,
            ],
            Self::CoreType => vec![
                Trait::Manifested,
                Trait::Versioned,
                Trait::PropertyTested,
                Trait::FailsClosed,
            ],
        }
    }
}

/// A single entity's trait declaration projected into the manifest.
/// Per `[[manifest-layer-is-the-keystone]]`: traits project through
/// the manifest, not duplicated in Rust source. Closes part of
/// `#171 [trait-v6]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraitProjection {
    /// Stable entity slug (e.g. "Loom.Primitive.Hero" or
    /// "Forge.Phase.Contrast"). Matches the `object` field of an
    /// `OrientationProjection` for the same entity.
    pub entity_id: String,
    /// The entity class — determines which default-required trait
    /// set the consistency check enforces.
    pub entity_class: EntityClass,
    /// Declared traits.
    pub traits: TraitSet,
}

/// A trait that the entity's class requires but the projection
/// did not declare. Surfaced by `verify_projection`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingTrait {
    /// The required trait that was missing.
    pub required: Trait,
    /// The entity_id (copied for batched audit reports).
    pub entity_id: String,
}

/// Verify a projection against its entity-class default-required
/// trait set. Returns the list of missing traits; empty Vec = ok.
///
/// Closes part of `#171 [trait-v6]`. Used by the Forge
/// `trait_consistency` phase to refuse manifests that don't
/// satisfy the class invariants.
#[must_use]
pub fn verify_projection(p: &TraitProjection) -> Vec<MissingTrait> {
    let required = p.entity_class.required_traits();
    required
        .into_iter()
        .filter(|t| !p.traits.contains(*t))
        .map(|required| MissingTrait {
            required,
            entity_id: p.entity_id.clone(),
        })
        .collect()
}

/// The full trait manifest — every entity's projection.
/// Per `[[manifest-layer-is-the-keystone]]`: this is THE canonical
/// projection for trait declarations across the platform.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraitManifest {
    /// Manifest version for backcompat per VERSION_DISCIPLINE.md.
    /// Defaults to "1.0.0" if absent on read.
    #[serde(default = "default_manifest_version")]
    pub schema_version: String,
    /// All entity projections.
    #[serde(default)]
    pub projections: Vec<TraitProjection>,
}

fn default_manifest_version() -> String {
    "1.0.0".into()
}

/// Audit the whole manifest. Returns the flattened list of missing
/// traits across every projection; empty Vec = clean.
#[must_use]
pub fn verify_manifest(m: &TraitManifest) -> Vec<MissingTrait> {
    let mut missing = Vec::new();
    for p in &m.projections {
        missing.extend(verify_projection(p));
    }
    missing
}

/// A trait implication rule per `TRAIT_DAG.md`: declaring `from`
/// implies declaring `to` (the substrate expands the implication
/// at audit time). Used by the Forge `trait_implications` phase
/// to refuse manifests that declare `from` without `to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Implication {
    /// The trait that triggers the implication when declared.
    pub from: Trait,
    /// The trait the substrate considers implicitly declared.
    pub to: Trait,
}

/// The canonical implication rules per `TRAIT_DAG.md`:
/// "Implication arrows mean: declaring Local automatically declares
/// Private. The manifest expands implications at parse time."
///
/// Closes part of `#169 [trait-v4]`. Used by the
/// `trait_implications` phase to refuse manifests that violate the
/// implication structure.
#[must_use]
pub fn canonical_implications() -> &'static [Implication] {
    &[
        // Sovereignty cluster.
        Implication { from: Trait::Local, to: Trait::Private },
        Implication { from: Trait::EphemeralByDefault, to: Trait::Private },
        // Theming cluster.
        Implication { from: Trait::DarkModeFirst, to: Trait::ThemeAware },
        Implication { from: Trait::AmoledOptimized, to: Trait::DarkModeFirst },
        // (AmoledOptimized → DarkModeFirst → ThemeAware transitively.)
        // Reliability cluster.
        Implication { from: Trait::FuzzTested, to: Trait::PropertyTested },
        // Discipline cluster.
        Implication { from: Trait::NoSiteSpecific, to: Trait::SubstrateNative },
        // Interaction cluster — the cascade enforced by entity-class
        // default sets is also expressible as implications for
        // primitives that opt into Interactive without going through
        // the full LoomInteractivePrimitive entity_class.
        Implication { from: Trait::Interactive, to: Trait::Focusable },
        Implication { from: Trait::Focusable, to: Trait::KeyboardOperable },
        Implication { from: Trait::KeyboardOperable, to: Trait::ScreenReaderAccessible },
    ]
}

/// Mutually-exclusive trait pairs — declaring both is a doctrine
/// violation. Used by `verify_implications` to refuse contradictory
/// manifests.
#[must_use]
pub fn mutual_exclusions() -> &'static [(Trait, Trait)] {
    &[
        // Visibility & Lifecycle: a primitive can be ClientOnly OR
        // ServerOnly, never both.
        (Trait::ClientOnly, Trait::ServerOnly),
    ]
}

/// An implication-rule violation surfaced by `verify_implications`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImplicationViolation {
    /// The entity declares `trigger` but is missing `implied`.
    /// The substrate refuses to expand the implication
    /// automatically — declaration is positive per
    /// `[[deterministic-first-lfi-optional]]`.
    MissingImpliedTrait {
        /// Entity slug (e.g. "Loom.Primitive.Hero").
        entity_id: String,
        /// The trait that triggered the implication.
        trigger: Trait,
        /// The trait that should have been declared but wasn't.
        implied: Trait,
    },
    /// The entity declares both members of a mutually-exclusive
    /// pair. One must be removed.
    MutuallyExclusiveBoth {
        /// Entity slug.
        entity_id: String,
        /// First trait in the conflicting pair.
        first: Trait,
        /// Second trait in the conflicting pair.
        second: Trait,
    },
}

impl ImplicationViolation {
    /// Entity_id for batched audit grouping.
    #[must_use]
    pub fn entity_id(&self) -> &str {
        match self {
            Self::MissingImpliedTrait { entity_id, .. } => entity_id,
            Self::MutuallyExclusiveBoth { entity_id, .. } => entity_id,
        }
    }
}

/// Verify a single projection against the canonical implication
/// rules + mutual-exclusion pairs. Returns the violation list;
/// empty Vec = clean.
///
/// Closes part of `#169 [trait-v4]`.
#[must_use]
pub fn verify_implications(p: &TraitProjection) -> Vec<ImplicationViolation> {
    let mut violations = Vec::new();
    // Forward implication check.
    for imp in canonical_implications() {
        if p.traits.contains(imp.from) && !p.traits.contains(imp.to) {
            violations.push(ImplicationViolation::MissingImpliedTrait {
                entity_id: p.entity_id.clone(),
                trigger: imp.from,
                implied: imp.to,
            });
        }
    }
    // Mutual-exclusion check.
    for (a, b) in mutual_exclusions() {
        if p.traits.contains(*a) && p.traits.contains(*b) {
            violations.push(ImplicationViolation::MutuallyExclusiveBoth {
                entity_id: p.entity_id.clone(),
                first: *a,
                second: *b,
            });
        }
    }
    violations
}

/// Audit the whole manifest against implication rules + mutual
/// exclusions. Returns the flat list of violations.
#[must_use]
pub fn verify_manifest_implications(m: &TraitManifest) -> Vec<ImplicationViolation> {
    let mut out = Vec::new();
    for p in &m.projections {
        out.extend(verify_implications(p));
    }
    out
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

    // -----------------------------------------------------------------
    // Markdown catalog rendering tests — task #172
    // -----------------------------------------------------------------

    #[test]
    fn category_metadata_complete() {
        // Every category has non-empty title + slug + appears in all().
        for c in Category::all() {
            assert!(!c.title().is_empty(), "{c:?} empty title");
            assert!(!c.slug().is_empty(), "{c:?} empty slug");
        }
        assert_eq!(Category::all().len(), 11);
    }

    #[test]
    fn category_slugs_unique() {
        let mut seen = std::collections::HashSet::new();
        for c in Category::all() {
            assert!(seen.insert(c.slug()), "duplicate category slug: {:?}", c.slug());
        }
    }

    #[test]
    fn rendered_catalog_contains_every_trait() {
        let md = render_markdown_catalog();
        // Every trait slug appears at least once in the rendered doc.
        for t in Trait::all() {
            assert!(
                md.contains(t.slug()),
                "rendered catalog missing trait {:?} ({})",
                t,
                t.slug()
            );
        }
    }

    #[test]
    fn rendered_catalog_contains_every_category_title() {
        let md = render_markdown_catalog();
        for c in Category::all() {
            assert!(
                md.contains(c.title()),
                "rendered catalog missing category title {:?}",
                c.title()
            );
        }
    }

    #[test]
    fn rendered_catalog_has_top_level_heading() {
        let md = render_markdown_catalog();
        assert!(md.starts_with("# PlausiDen trait catalog"));
    }

    #[test]
    fn rendered_catalog_documents_default_sets() {
        let md = render_markdown_catalog();
        // Visible defaults section header + all 8 trait slugs appear.
        assert!(md.contains("Loom primitive (Visible lineage)"));
        for t in Trait::loom_visible_defaults() {
            assert!(md.contains(t.slug()));
        }
        // Interactive cascade section + 3 trait slugs.
        assert!(md.contains("Loom primitive (Interactive lineage"));
        for t in Trait::loom_interactive_defaults() {
            assert!(md.contains(t.slug()));
        }
    }

    #[test]
    fn rendered_catalog_is_deterministic() {
        // Pure projection: same call → identical bytes.
        let a = render_markdown_catalog();
        let b = render_markdown_catalog();
        assert_eq!(a, b);
    }

    // -----------------------------------------------------------------
    // Manifest projection + consistency check tests — task #171
    // -----------------------------------------------------------------

    #[test]
    fn entity_class_loom_visible_returns_eight_defaults() {
        let req = EntityClass::LoomVisiblePrimitive.required_traits();
        assert_eq!(req.len(), 8);
        for t in Trait::loom_visible_defaults() {
            assert!(req.contains(t));
        }
    }

    #[test]
    fn entity_class_loom_interactive_returns_eleven() {
        // Cascade: 8 Visible defaults + 3 Interactive cascade = 11.
        let req = EntityClass::LoomInteractivePrimitive.required_traits();
        assert_eq!(req.len(), 11);
        assert!(req.contains(&Trait::Focusable));
        assert!(req.contains(&Trait::KeyboardOperable));
        assert!(req.contains(&Trait::ScreenReaderAccessible));
        // Visible defaults still present.
        assert!(req.contains(&Trait::MobileFriendly));
    }

    #[test]
    fn verify_projection_clean_when_all_required_declared() {
        let p = TraitProjection {
            entity_id: "Loom.Primitive.Hero".into(),
            entity_class: EntityClass::LoomVisiblePrimitive,
            traits: TraitSet::from_slice(Trait::loom_visible_defaults()),
        };
        let missing = verify_projection(&p);
        assert!(
            missing.is_empty(),
            "expected clean, got missing: {missing:?}"
        );
    }

    #[test]
    fn verify_projection_reports_missing_required_traits() {
        let p = TraitProjection {
            entity_id: "Loom.Primitive.SloppyHero".into(),
            entity_class: EntityClass::LoomVisiblePrimitive,
            traits: TraitSet::from_slice(&[
                Trait::MobileFriendly,
                Trait::RtlAware,
                // missing six others including ReducedMotionAware
            ]),
        };
        let missing = verify_projection(&p);
        // Six required traits missing.
        assert_eq!(missing.len(), 6);
        let missing_traits: Vec<Trait> = missing.iter().map(|m| m.required).collect();
        assert!(missing_traits.contains(&Trait::ReducedMotionAware));
        assert!(missing_traits.contains(&Trait::ThemeAware));
        assert!(missing_traits.contains(&Trait::DoctrineCited));
        // Each MissingTrait carries the entity_id for batched reports.
        for m in &missing {
            assert_eq!(m.entity_id, "Loom.Primitive.SloppyHero");
        }
    }

    #[test]
    fn verify_projection_handles_each_entity_class() {
        // Forge phase needs 3 traits.
        let phase = TraitProjection {
            entity_id: "Forge.Phase.Contrast".into(),
            entity_class: EntityClass::ForgePhase,
            traits: TraitSet::from_slice(&[
                Trait::DoctrineCited,
                Trait::PropertyTested,
                Trait::FailsClosed,
            ]),
        };
        assert!(verify_projection(&phase).is_empty());

        // CmsSection needs 3 traits.
        let section = TraitProjection {
            entity_id: "Cms.Section.PullQuote".into(),
            entity_class: EntityClass::CmsSection,
            traits: TraitSet::from_slice(&[
                Trait::NoSiteSpecific,
                Trait::Manifested,
                Trait::Versioned,
            ]),
        };
        assert!(verify_projection(&section).is_empty());

        // CrawlerDetector missing one.
        let detector = TraitProjection {
            entity_id: "Crawler.Detector.Contrast".into(),
            entity_class: EntityClass::CrawlerDetector,
            traits: TraitSet::from_slice(&[Trait::Manifested, Trait::PropertyTested]),
        };
        let missing = verify_projection(&detector);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].required, Trait::RegressionFixtured);
    }

    #[test]
    fn verify_manifest_aggregates_missing_across_entities() {
        let m = TraitManifest {
            schema_version: "1.0.0".into(),
            projections: vec![
                TraitProjection {
                    entity_id: "Loom.Primitive.Clean".into(),
                    entity_class: EntityClass::LoomVisiblePrimitive,
                    traits: TraitSet::from_slice(Trait::loom_visible_defaults()),
                },
                TraitProjection {
                    entity_id: "Loom.Primitive.Sloppy".into(),
                    entity_class: EntityClass::LoomVisiblePrimitive,
                    traits: TraitSet::from_slice(&[Trait::MobileFriendly]),
                },
            ],
        };
        let missing = verify_manifest(&m);
        // Sloppy is missing 7; Clean missing 0. So 7 total.
        assert_eq!(missing.len(), 7);
        for m in &missing {
            assert_eq!(m.entity_id, "Loom.Primitive.Sloppy");
        }
    }

    #[test]
    fn manifest_serde_roundtrips_with_version_and_projections() {
        let m = TraitManifest {
            schema_version: "1.0.0".into(),
            projections: vec![TraitProjection {
                entity_id: "Loom.Primitive.Hero".into(),
                entity_class: EntityClass::LoomVisiblePrimitive,
                traits: TraitSet::from_slice(&[Trait::MobileFriendly, Trait::Versioned]),
            }],
        };
        let json = serde_json::to_string(&m).expect("serialize");
        let back: TraitManifest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.schema_version, "1.0.0");
        assert_eq!(back.projections.len(), 1);
        assert_eq!(back.projections[0].entity_id, "Loom.Primitive.Hero");
    }

    #[test]
    fn manifest_deserialize_supplies_default_version() {
        // Per VERSION_DISCIPLINE.md additive change: omitting
        // schema_version reads as 1.0.0 (the only released schema).
        let json = r#"{"projections":[]}"#;
        let m: TraitManifest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(m.schema_version, "1.0.0");
        assert!(m.projections.is_empty());
    }

    #[test]
    fn manifest_rejects_unknown_field() {
        let json = r#"{"schema_version":"1.0.0","made_up":"x"}"#;
        let r: Result<TraitManifest, _> = serde_json::from_str(json);
        assert!(r.is_err());
    }

    // -----------------------------------------------------------------
    // Implication rules + mutual exclusions — task #169
    // -----------------------------------------------------------------

    #[test]
    fn canonical_implications_no_self_arrows() {
        // Implications should be A → B with A ≠ B.
        for imp in canonical_implications() {
            assert_ne!(imp.from, imp.to, "self-implication: {imp:?}");
        }
    }

    #[test]
    fn canonical_implications_unique() {
        let mut seen = std::collections::HashSet::new();
        for imp in canonical_implications() {
            assert!(seen.insert((imp.from, imp.to)), "duplicate implication: {imp:?}");
        }
    }

    #[test]
    fn mutual_exclusions_no_self_pairs() {
        for (a, b) in mutual_exclusions() {
            assert_ne!(a, b);
        }
    }

    #[test]
    fn verify_implications_clean_when_no_triggers_declared() {
        let p = TraitProjection {
            entity_id: "Loom.Primitive.Hero".into(),
            entity_class: EntityClass::LoomVisiblePrimitive,
            traits: TraitSet::from_slice(&[
                Trait::MobileFriendly,
                Trait::RtlAware,
                Trait::ReducedMotionAware,
                Trait::ThemeAware,
                Trait::NoSiteSpecific,
                Trait::SubstrateNative,  // satisfies the NoSiteSpecific implication
                Trait::Manifested,
                Trait::Versioned,
                Trait::DoctrineCited,
            ]),
        };
        let v = verify_implications(&p);
        assert!(v.is_empty(), "expected clean, got: {v:?}");
    }

    #[test]
    fn verify_implications_catches_local_without_private() {
        let p = TraitProjection {
            entity_id: "Loom.Primitive.LocalCache".into(),
            entity_class: EntityClass::LoomVisiblePrimitive,
            traits: TraitSet::from_slice(&[Trait::Local]),
        };
        let v = verify_implications(&p);
        assert!(v.iter().any(|x| matches!(
            x,
            ImplicationViolation::MissingImpliedTrait {
                trigger: Trait::Local,
                implied: Trait::Private,
                ..
            }
        )));
    }

    #[test]
    fn verify_implications_catches_amoled_without_dark_mode_first() {
        let p = TraitProjection {
            entity_id: "Loom.Primitive.PicoStyled".into(),
            entity_class: EntityClass::LoomVisiblePrimitive,
            traits: TraitSet::from_slice(&[Trait::AmoledOptimized]),
        };
        let v = verify_implications(&p);
        assert!(v.iter().any(|x| matches!(
            x,
            ImplicationViolation::MissingImpliedTrait {
                trigger: Trait::AmoledOptimized,
                implied: Trait::DarkModeFirst,
                ..
            }
        )));
    }

    #[test]
    fn verify_implications_catches_fuzz_without_property() {
        let p = TraitProjection {
            entity_id: "Forge.Phase.Test".into(),
            entity_class: EntityClass::ForgePhase,
            traits: TraitSet::from_slice(&[Trait::FuzzTested]),
        };
        let v = verify_implications(&p);
        assert!(v.iter().any(|x| matches!(
            x,
            ImplicationViolation::MissingImpliedTrait {
                trigger: Trait::FuzzTested,
                implied: Trait::PropertyTested,
                ..
            }
        )));
    }

    #[test]
    fn verify_implications_catches_interactive_cascade() {
        // Interactive → Focusable → KeyboardOperable → SR-accessible
        // chain; if Interactive is declared without Focusable, we
        // flag the first missing link.
        let p = TraitProjection {
            entity_id: "Loom.Primitive.SloppyForm".into(),
            entity_class: EntityClass::LoomInteractivePrimitive,
            traits: TraitSet::from_slice(&[Trait::Interactive]),
        };
        let v = verify_implications(&p);
        assert!(v.iter().any(|x| matches!(
            x,
            ImplicationViolation::MissingImpliedTrait {
                trigger: Trait::Interactive,
                implied: Trait::Focusable,
                ..
            }
        )));
    }

    #[test]
    fn verify_implications_catches_mutually_exclusive_client_and_server() {
        let p = TraitProjection {
            entity_id: "Loom.Primitive.Contradictory".into(),
            entity_class: EntityClass::LoomVisiblePrimitive,
            traits: TraitSet::from_slice(&[Trait::ClientOnly, Trait::ServerOnly]),
        };
        let v = verify_implications(&p);
        assert!(v.iter().any(|x| matches!(
            x,
            ImplicationViolation::MutuallyExclusiveBoth {
                first: Trait::ClientOnly,
                second: Trait::ServerOnly,
                ..
            }
        )));
    }

    #[test]
    fn verify_manifest_implications_aggregates() {
        let m = TraitManifest {
            schema_version: "1.0.0".into(),
            projections: vec![
                TraitProjection {
                    entity_id: "Clean".into(),
                    entity_class: EntityClass::CmsSection,
                    traits: TraitSet::from_slice(&[
                        Trait::NoSiteSpecific,
                        Trait::SubstrateNative,
                        Trait::Manifested,
                        Trait::Versioned,
                    ]),
                },
                TraitProjection {
                    entity_id: "Sloppy".into(),
                    entity_class: EntityClass::CmsSection,
                    // NoSiteSpecific but missing SubstrateNative.
                    traits: TraitSet::from_slice(&[
                        Trait::NoSiteSpecific,
                        Trait::Manifested,
                        Trait::Versioned,
                    ]),
                },
            ],
        };
        let v = verify_manifest_implications(&m);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].entity_id(), "Sloppy");
    }

    #[test]
    fn implication_violation_serializes() {
        let v = ImplicationViolation::MissingImpliedTrait {
            entity_id: "X".into(),
            trigger: Trait::Local,
            implied: Trait::Private,
        };
        let json = serde_json::to_string(&v).expect("serialize");
        assert!(json.contains("MissingImpliedTrait"));
        assert!(json.contains("local"));
        assert!(json.contains("private"));
    }
}
