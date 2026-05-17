# FORGE_ANIMATION_GAPS — marcodeluca.me clone

Task #662 / T74. First animated-reference site rebuilt. Reference:
[marcodeluca.me](https://marcodeluca.me) (designer portfolio with
significant kinetic typography, scroll-coupled animation, and
hover-video case-study cards).

## Phase 1 — Structural mirror (this commit)

Done: `examples/marcodeluca/cms/index.json` reproduces the
landing-page structure (hero / selected work / about / contact)
using only the seven `CmsSection` variants Forge currently ships.
The result is **static text and basic CardFeed** — visually
miles from the original, but the IA matches.

## Phase 2 — Interactivity (queued)

Requires gaps below. Tracked in this file so the rebuild list
accumulates a dedup'd registry of missing variants.

## Phase 3 — Visual diff against original (queued)

Requires the Forge `crawl --capture-baseline` machinery (T660
cycle 2) + the animation primitives below or a deliberate
hold-the-line where we accept the diff and surface the gaps.

## Gaps surfaced by this rebuild

### GAP-T662-1 · `CmsSection::KineticTitle`

The original headline animates per-word with split-letter masks
and a custom rotation easing curve. Hover flips colors of
adjacent words. Proposed shape:

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
    /// Optional explicit color override.
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

### GAP-T662-2 · `CmsCard.hover_video`

Case-study cards swap their cover image for an autoplaying muted
video on hover. Per-card MP4, lazy-loaded, ≤2.4 MB budget.

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

JS-side: the T432 `forge-spa-runtime.js` can grow a
`data-loom-hover-video` attribute handler that binds `mouseenter` /
`mouseleave` events to swap the `<picture>` for a `<video
autoplay muted playsinline>`.

### GAP-T662-3 · `CmsSection::ScrollMarquee`

Below About, the original has a horizontally-scrolling client-logo
marquee pinned to the viewport for ~120vh, with logos sweeping at
parallax-coupled speed.

```rust
ScrollMarquee {
    items: Vec<MarqueeItem>,
    /// How many viewport-heights the marquee pins for.
    pin_height_vh: u16,
    /// Px per scroll-px ratio. 1.0 = direct coupling.
    speed_multiplier: f32,
    /// Direction.
    direction: MarqueeDirection,
}

pub enum MarqueeItem {
    BrandLogo(BrandLogo),  // surfaced as a sibling gap below
    TextChip(String),
    Picture(PictureSpec),
}
```

Implementation: pure CSS via `position: sticky` + a `<div>` chain
with `transform: translateX()` driven by an `IntersectionObserver`
callback in the SPA runtime. Or, with `scroll-driven animations`
(Chrome 115+ / Safari 18.4+), purely declarative. Need a feature-
detect fallback.

### GAP-T662-4 · `CmsSection::LogoWall` / brand logos in loom-icons

Adjacent to GAP-T662-3: the marquee items are brand SVG logos.
`loom-icons` currently ships only abstract UI glyphs (Lucide-
derived). A separate `loom-brand-icons` crate (vetted brand SVG
sources only — never user-uploaded markup) is queued. The
SECURITY: line stays as in `loom-icons`: brand SVG bodies are
trusted content, never user-supplied.

This gap also showed up in T660 Stripe pricing rebuild.
De-duplicated to ONE missing variant.

### GAP-T662-5 · `CmsCard.avatar.kind = "video"` / animated avatars

Smaller gap: some cards on the original use a 3-frame WebP cycle
in the avatar slot. The current `letter` / `picture` avatar kinds
cover the Phase-1 mirror; add `video` / `lottie` / `webp-anim` in
a follow-up when the section gallery genuinely needs them.

## Decision: pixel-faithful vs primitive-driven

Per T74 description: "reproduce so faithfully owner can't tell
the difference." The faithful path requires shipping the 4-5
primitives above, NOT hacking a one-off `<style>` blob into this
page. The DELIVERABLE for cycle 1 is THIS GAP REPORT plus the
structural mirror; cycles 2-3 land the primitives and re-build
this page (+ a second animated site, probably sphericalwaves.com,
to drive the registry to convergence).
