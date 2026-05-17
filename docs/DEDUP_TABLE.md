# Forge component-variant dedup table

> Canonical artifact. Replaces the per-site `examples/` directory that
> was used between 2026-05-13 and 2026-05-17 as a debug holding pen to
> surface missing `CmsSection` variants.

## What this is

A registry of the typed `CmsSection` variants that **real public sites
need** but the Forge component primitive set has not yet shipped. Each
row carries:

- The proposed Rust shape for the variant.
- Which named-site rebuild first surfaced it.
- The cross-rebuild surface count (X-of-N), which drives priority.
- Implementation status (shipped / queued / deferred).

Per AVP-2 dedup theory: a variant that surfaces in ≥3 cross-vertical
rebuilds is a P1/P2 universal; 1-2 surface counts are deferred until
a second rebuild confirms generality.

## What this is NOT

This is not a list of third-party sites to clone, mimic, or
pixel-recreate. The 9 rebuilds that produced this signal were
single-pass *structural mirrors* used as input, deleted after
extraction. **No further named-site clones will be committed** —
Forge is a general-purpose engine; future component-variant
proposals come from owner-curated novel reference designs or
live customer-site work, not from "let's clone Stripe".

## Methodology (single-pass rebuild → dedup signal)

For each reference site:

1. Take a single-pass IA mirror as a Forge CmsPage JSON. Use ONLY
   currently-shipped `CmsSection` variants. Result: visually distant
   from the original.
2. Document every variant the rebuild **needed but couldn't use** —
   each is a missing primitive.
3. After ≥4 rebuilds across ≥2 verticals, tally cross-rebuild surface
   counts. ≥3 surfaces in one vertical → vertical-specific P-tier.
   ≥3 surfaces across 2+ verticals → universal P-tier.
4. Implement P1/P2 universals first. Vertical-specific universals
   ship next. Single-surface gaps stay in registry until a 2nd
   surface confirms them.

After the variant ships, the rebuild that surfaced it should NOT be
re-built to confirm — the test matrix (`audit:rust:smoke` /
`-mobile` / `-tablet` across light/dark) is the regression net.

## Universal variants (cross-vertical, ≥3 surface count)

| Variant | Stripe | Linear | Vercel | NYT | TheAtlantic | marcodeluca | sphericalwaves | aristidebenoist | lusion | Hits | Status |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `Quote` (testimonial / opinion lede) | ✓ | ✓ | ✓ | ✓ | ✓ | — | — | ✓ | ✓ | **7/9** | **shipped P2 (T660 cycle 3)** |
| `LogoWall` / `loom-brand-icons` | ✓ | ✓ | ✓ | — | — | ✓ | — | — | — | **4/9** | shipped P1 (marketing-vertical) |
| `Pricing` tier-cards | ✓ | ✓ | ✓ | — | — | — | — | — | — | 3/9 | queued P2 (marketing-only) |

## Vertical-specific variants

| Variant | Vertical | Hits | Status |
|---|---|---|---|
| `ArticleCard` (headline + dek + byline + dateline + thumbnail) | news-media | 2/9 (NYT, TheAtlantic) | shipped P2 (T660 cycle 5) — universal across news |
| `Code` / terminal block | dev-tools | 2/9 (Stripe, Vercel) | shipped P3 |

## Single-surface variants (registry only; awaits 2nd surface)

| Variant | First surfaced | Priority | Notes |
|---|---|---|---|
| `Faq` disclosure | Stripe | MED | Needs `<details>`/`<summary>` semantics for a11y |
| `ComparisonTable` (feature × tier matrix) | Stripe | MED | Richer than `KvPair` |
| `FooterSitemap` (multi-col) | Stripe | MED | Page-shell footer currently single `<p>` |
| `CountryPicker` + currency-aware pricing | Stripe | LOW | Needs Forge `Dynamic` mode runtime |
| `KineticTitle` (per-word animation, hover flip) | marcodeluca | LOW | Animation primitive; depends on `TimingCurve` enum |
| `CmsCard.hover_video` | marcodeluca | LOW | T432 `forge-spa-runtime.js` extension |
| `CmsCard.hover_audio` | sphericalwaves | LOW | Mirror of hover_video |
| `ScrollMarquee` (pinned horizontal logo scroll) | marcodeluca | LOW | CSS sticky + scroll-driven animations |
| `Canvas3D` (Three.js/WebGL) | sphericalwaves | NEW | Whole new sandboxed-iframe rendering path |
| Animated avatars (`webp-anim`/`lottie`/`video`) | marcodeluca | LOW | Extends existing avatar enum |
| Live-blog timestamp pattern | NYT | LOW | Additive `style: timeline\|table\|grid` hint on `kv_pair`, not a new variant |

## Variant specs (deferred — not yet shipped)

### `Pricing` (queued P2)

```rust
Pricing {
    columns: Vec<CmsPricingColumn>,
}

pub struct CmsPricingColumn {
    pub name: String,
    pub price: String,        // formatted; renderer doesn't compute
    pub unit_suffix: String,  // "per transaction", "/month", etc.
    pub headline: String,
    pub features: Vec<String>,
    pub cta: Option<HeroCta>,
    pub featured: bool,       // for the "popular" callout
}
```

### `Faq` (MED, single-surface)

```rust
Faq {
    items: Vec<FaqItem>,
}

pub struct FaqItem {
    pub question: String,
    pub answer_md: String,    // markdown-rendered answer body
}
```

Renderer must use `<details><summary>` for keyboard + screen-reader
disclosure semantics. WCAG 2.1 AA per
[[feedback_forge_default_themes_a11y]].

### `ComparisonTable` (MED, single-surface)

```rust
ComparisonTable {
    columns: Vec<String>,           // tier names
    rows: Vec<ComparisonRow>,
}

pub struct ComparisonRow {
    pub feature: String,
    pub cells: Vec<ComparisonCell>, // len == columns.len()
}

pub enum ComparisonCell {
    Check,
    Cross,
    Text(String),
    Custom { label: String, tooltip: Option<String> },
}
```

### `FooterSitemap` (MED, single-surface)

```rust
FooterSitemap {
    columns: Vec<FooterColumn>,
    legal: Option<String>,    // bottom-strip text
}

pub struct FooterColumn {
    pub heading: String,
    pub links: Vec<FooterLink>,
}

pub struct FooterLink {
    pub label: String,
    pub href: String,
    pub external: bool,
}
```

### `KineticTitle` (LOW, single-surface)

```rust
KineticTitle {
    words: Vec<KineticWord>,
    /// Easing applied per character on initial reveal.
    easing: TimingCurve,
    /// Hover behaviour. Default: NoOp.
    on_hover: HoverEffect,
}

pub struct KineticWord {
    pub text: String,
    pub color: Option<TokenName>,
    /// Stagger delay multiplier (0..=1).
    pub delay: f32,
}

pub enum TimingCurve {
    Linear,
    EaseOut,
    EaseInOut,
    Spring { stiffness: f32, damping: f32 },
    Cubic { a: f32, b: f32, c: f32, d: f32 },
}

pub enum HoverEffect {
    NoOp,
    FlipColor { from: TokenName, to: TokenName },
    DropShadow { offset_px: u8, blur_px: u8, color: TokenName },
}
```

### `CmsCard.hover_video` (LOW, single-surface)

Extension on existing `CmsCard`:

```rust
pub struct HoverVideoSpec {
    pub src: String,       // /uploads/<stem>.mp4
    pub poster_src: String,
    pub max_bytes: u64,    // budget enforced at build time
    pub preload: VideoPreload,
}

pub enum VideoPreload {
    None,        // load on hover only
    Metadata,    // request headers only on page-load
    Auto,        // full preload (bandwidth-hostile)
}
```

JS-side: `forge-spa-runtime.js` (T432) grows a
`data-loom-hover-video` attribute handler binding `mouseenter` /
`mouseleave` to swap a `<picture>` for `<video autoplay muted playsinline>`.

### `ScrollMarquee` (LOW, single-surface)

```rust
ScrollMarquee {
    items: Vec<MarqueeItem>,
    /// How many viewport-heights the marquee pins for.
    pin_height_vh: u16,
    /// Px per scroll-px ratio. 1.0 = direct coupling.
    speed_multiplier: f32,
    direction: MarqueeDirection,
}

pub enum MarqueeItem {
    BrandLogo(BrandLogo),  // sibling gap; resolved by LogoWall
    TextChip(String),
    Picture(PictureSpec),
}

pub enum MarqueeDirection {
    LeftToRight,
    RightToLeft,
    Vertical,
}
```

Implementation: `position: sticky` + transform chain driven by
`IntersectionObserver` in the SPA runtime, or declaratively via
scroll-driven animations (Chrome 115+, Safari 18.4+) with feature-
detect fallback.

### `Canvas3D` (NEW, single-surface, deferred)

A whole new rendering path: a sandboxed iframe loading a Three.js
or WebGL scene, with a typed `Canvas3DSpec` carrying the scene
definition. Defer until ≥3 sites need it OR until owner explicitly
opens this scope.

## How this drives the backlog

1. **Variants with `Status: shipped`** → done; they ship with the
   current `CmsSection` enum. Don't rebuild them.
2. **`queued P2`** → next implementation cycle. Spec is locked in
   this doc; implementation is a translation exercise.
3. **`MED single-surface`** → awaits a 2nd cross-site surface before
   spec freeze. If a customer site lands needing one, bump straight
   to queued.
4. **`LOW single-surface`** → registry only. No work scheduled.
5. **`NEW`** → would expand Forge's runtime model. Owner must
   re-open scope before implementation begins.

## Coverage by original rebuild (now deleted)

| Rebuild | Vertical | Cycle | Variants shipped | Gaps logged |
|---|---|---|---|---|
| stripe-pricing | dev-tools / SaaS marketing | T660 c1 | banner, group, heading, hero, kv_pair | 7 (Pricing, CountryPicker, Savings, ComparisonTable, LogoWall, Faq, FooterSitemap) |
| linear | dev-tools / SaaS marketing | T660 c2 | + card_feed, letter | + Quote, LogoWall confirmed |
| vercel | dev-tools / SaaS marketing | T660 c3 | + Code | LogoWall dedup table locked at 5 |
| nytimes | news-media | T660 c4 | + Quote | ArticleCard NEW MED, Live-blog NEW LOW |
| theatlantic | news-media | T660 c5 | + ArticleCard | ArticleCard 2-of-2 → LOCKED |
| marcodeluca | designer portfolio (animated) | T662 c1 | (none new) | KineticTitle, hover_video, ScrollMarquee, LogoWall (dup), animated avatars |
| sphericalwaves | 3D portfolio (animated) | T662 c2 | (none new) | hover_audio, Canvas3D NEW |
| aristidebenoist | interaction-design portfolio | T662 c3 | (none new) | (no new gaps; pure dedup confirmation) |
| lusion | animated portfolio | T662 c4 | (none new) | (no new gaps; ScrollRevealText 4-OF-4 LOCKED MAX) |

## Provenance

Surfaced from:

- `examples/stripe-pricing/FORGE_GAPS.md`
- `examples/nytimes/FORGE_GAPS.md`
- `examples/marcodeluca/FORGE_ANIMATION_GAPS.md`
- 9 `examples/*/cms/index.json` structural mirrors
- Tasks #660 (cycles 1-5) and #662 (cycles 1-4)

All examples/ artifacts deleted in the same commit that introduces
this doc. Future variant proposals come from customer-site work or
explicit owner directive, not from speculative third-party clones.

## See also

- `docs/FORGE_VISION.md` — what Forge is and isn't.
- `docs/PERSONAS.md` — who the rebuilds were proxies for.
- `FORGE_ROADMAP.md` — task-level backlog including the T70 (variant
  implementation) and T660/T662 series above.
- `PlausiDen-Loom/CLAUDE.md` — UI doctrine; every shipped variant
  must satisfy it.
