# Decorative primitive coverage audit — 2026-05-27

Per task #356. Walks Forge's existing decorative primitive
surface and compares against what the 5 reference sites
(`docs/SUBSTRATE_REFERENCE_SITES_2026_05_27.md`) actually use,
focusing on the two most decoration-heavy: **kinfolk.com**
(magazine + photographic editorial) and **jvns.ca** (personal
blog + illustrative / comic register).

## Existing decorative surface in Forge

### Polish tokens (loom-tokens::polish::PolishToken — 12 variants)

```
DotGrid         · subtle texture overlay
LinearMesh      · gradient mesh backdrop
Topographic     · contour-line decoration
EditorialRule   · accent rule under headings
InsetFrame      · framed picture treatment
BlueprintCorner · corner crop marks
SoftGlow        · subtle ambient glow
BrandHalo       · brand-tinted aura
AmoledRim       · OLED-true-black edge treatment
SlowReveal      · scroll-triggered fade-in
PageTurn        · page-turn transition
CursorTilt      · pointer-tracking parallax
```

### Hero backgrounds (HeroBackground — 5 variants)

```
GradientMesh    · brand-tinted gradient
Solid           · single brand color
Stripes         · diagonal accent stripes
Dots            · dot pattern overlay
Photo           · photographic backdrop
```

### FeatureSpotlight decoration (3 variants)

```
Decorated       · SaaS-card chrome (rounded, drop shadow, lift on hover)
Editorial       · top accent rule, no card chrome
Minimal         · tight grid, no decoration
```

### Tone variants (per primitive type)

- `AlertTone`: Info / Success / Warning / Danger / Neutral (5)
- `BadgeTone`: Info / Success / Warning / Danger / Neutral (5)
- `ToastTone`: Info / Success / Warning / Error (4)
- `KvPairTone`: Slate / Amoled (2)
- `PullQuoteTone`: Slate / Amoled (2)
- `CodeShellTone`: Slate / Amoled (2)
- `HeroEditorialBackground`: Slate / Plain / Amoled (3)
- `PullQuoteEmphasis`: Inline / Display (2)

### Gradient pool (loom-tokens::gradient_pool — 24 pairs)

Shipped #352. 6 mood categories (Cool / Warm / Monochrome /
Duotone / Neutral / Photographic) + Solid. Identity-aware
deterministic selection.

### Themes (9 shipped)

`light`, `dark`, `auto`, `warm`, `ocean`, `forest`, `violet`,
`rose`, `amoled`. All in the "modern minimal" register.

## What kinfolk.com needs decoratively

### Photographic surface

| Need | Coverage |
|---|---|
| Full-bleed hero photo | ✓ `HeroBackground::Photo` |
| Photo as mid-flow section divider | ✗ ImageHero is hero-only; mid-flow full-bleed photo isn't a CmsSection |
| Two-up photo composition (side-by-side) | ✗ No primitive; would have to compose manually |
| Photo-with-caption mid-flow | partial — `picture` primitive exists but never reached (audit #355); no caption emphasis |
| Photo grid (4-8 images, no headings) | partial — `image_grid` exists but never reached |

### Typographic surface

| Need | Coverage |
|---|---|
| Serif display type as theme baseline | ✗ All 9 themes use system-ui; serif requires per-tenant override |
| Drop cap on opening paragraph | ✗ `drop_cap` primitive exists but never reached |
| Generous line-height / leading | partial — `DensityTier::Loose` exists but isn't threaded through `Paragraph` |
| Pull quote at editorial scale (full-column, serif, ornamental marks) | partial — `PullQuoteEmphasis::Display` is the closest; no ornamental-marks variant |

### Atmosphere

| Need | Coverage |
|---|---|
| Subtle warm-tinted backdrop | ✓ `warm` theme + `PolishToken::SoftGlow` |
| Slow page-load reveal (atmospheric in/out) | ✓ `PolishToken::SlowReveal` |
| Quiet, no-chrome between sections | ✗ No "section-separator: silent" — divider primitive draws a rule |

## What jvns.ca needs decoratively

### Hand-drawn / illustrative register

| Need | Coverage |
|---|---|
| Hand-drawn / sketch border on figures | ✗ No primitive; only `InsetFrame` (clean rectangle frame) |
| Marker / highlighter accent text | ✗ No primitive |
| Doodle-style arrows / annotations | ✗ No primitive |
| Comic-strip panels (multi-panel illustration with captions) | ✗ No primitive — major gap |
| Personality / zine aesthetic theme | ✗ No theme — all 9 themes are "modern minimal" |

### Code + explanation editorial pattern

| Need | Coverage |
|---|---|
| Code block with side-explanation | partial — `Compose` + manual layout works; no first-class primitive |
| Inline annotation on specific code line | ✗ No primitive |
| Code + diagram side-by-side | ✗ No primitive (`diagram` exists but never reached) |

### Author voice frame

| Need | Coverage |
|---|---|
| Sitewide author identity (name + photo + bio + links) | ✗ No structured primitive (CmsPage.brand is single string) |
| Per-post date stamp | ✗ No CmsPage.published_at field |
| Author voice profile (informal / personal / first-person) | partial — VoiceProfile exists but not surfaced visually |

## Aggregate gap list

Decorative work mapped from "would advance reference-site coverage":

### Tier 1 — covers both kinfolk and jvns or one heavily

1. **Mid-flow full-bleed photo section** (kinfolk primary; jvns secondary)
   New CmsSection variant: `PhotoBleed { src, alt, height_ramp }`. Spans full viewport width; renders between body sections as a visual breath. Bounded — single new variant + skin styling.

2. **Editorial / magazine theme** (kinfolk primary)
   New theme entry. Serif display + body fonts shipped as theme defaults; loose density; generous margins; warm neutral palette. Design-led work — 1-2 weeks. Lands in loom-tokens::style_packs + skin.css.

3. **Drop-cap render-arm for opening paragraph** (kinfolk + paulgraham)
   `Paragraph` primitive gains optional `drop_cap: bool` field; render emits the ornamental drop-cap when true. Also: existing `drop_cap` CmsSection variant gets a doc-query entry so operators find it. Bounded.

### Tier 2 — niche but high-value for one site

4. **Comic-strip primitive** (jvns)
   New CmsSection variant: `ComicStrip { panels: Vec<Panel> }` where `Panel { image, caption, position }`. Substantial — requires illustration asset pipeline + responsive panel layout. Design + engineering.

5. **Zine aesthetic theme** (jvns)
   New theme entry. Hand-drawn-style display fonts, marker-color accents, sketch borders as default polish. Design-led. Optional asset pack (sketch-border SVGs).

6. **Author identity surface** (jvns)
   New struct on CmsPage: `author: Option<Author>` where `Author { name, avatar, bio, links }`. Author-bio frame primitive that renders the structured author. Bounded — single new field + new variant.

7. **Date-stamped page surface** (jvns + paulgraham)
   `CmsPage.published_at: Option<IsoDate>` field; page-shell renders it under the title. Bounded.

### Tier 3 — surfacing existing primitives

8. **`drop_cap` / `picture` / `image_grid` / `figure` / `figure_group` doc-query entries** (per #355)
   These are shipped but never reached. Surfacing them via doc-query means kinfolk-shape content can find them. Documentation work, not new substrate.

9. **Polish token reach push**
   `EditorialRule`, `InsetFrame`, `LinearMesh` are existing polish tokens that suit editorial registers. Add doc-query entries + example uses so they get reached.

## Honest scope note

This audit doesn't address marketing landing or gov.uk decorative
needs because:
- Marketing landing IS the substrate's current band; decorative
  coverage is already strong
- gov.uk's register is anti-decorative on principle; the gap is
  in restraint primitives (no-decoration tiers), not new
  decorative shapes

The decorative gap is concentrated in editorial-magazine (kinfolk)
and personal-zine (jvns) registers. Tier 1 + Tier 2 cover ~80%
of the observed decorative needs across these two sites.

## Tier-1 effort estimate

- Mid-flow full-bleed photo: 1 day of substrate work + skin styling
- Editorial theme: 1-2 weeks of design-led work
- Drop-cap surfacing: half a day

Tier 1 ships in ~2 weeks of focused work. Tier 2 is another 4-6
weeks (comic strip primitive is the heaviest item). Tier 3 is
documentation that ships as the doc-query index grows.

## Mapping to existing tasks

- **#358** (theme system growth) — owns Tier 1 #2 + Tier 2 #5
  (editorial + zine themes)
- **#359** (BlockKind coverage) — owns Tier 1 #1 (PhotoBleed) +
  Tier 2 #4 (ComicStrip) + Tier 2 #6 (Author identity)
- **#398** doc-query expansion — owns Tier 3 (surface existing
  primitives in the canonical_index)
