# Substrate gap catalog — 2026-05-27

Per-site enumeration of what each of the 5 reference sites
(`docs/SUBSTRATE_REFERENCE_SITES_2026_05_27.md`) contains that
Forge's substrate cannot currently express well, plus the
aggregate gap list that drives substrate roadmap priority across
#355-#360.

Substrate baseline as of 2026-05-27: 163 CmsSection variants +
62 CmsBlock primitives + 9 themes + 61 audit phases + page-kind
threading (#405).

## Per-site gap inventory

### paulgraham.com — minimal text-only essay

**Existing substrate covers:**
- PageKind::Brief / Editorial (shipped #405)
- Paragraph + Heading primitives
- Brief-kind sparse_page + image_desert suppression
- Per-tenant [style.fonts] for serif body

**Missing:**
- **No-chrome page-shell variant** — current `ChromeKind` is PageShell / FloatingPill / Minimal; "Minimal" still emits brand + nav. Need `ChromeKind::None` (nothing above content).
- **Essay-list primitive** — paulgraham.com's index is just a `<ul>` of essay titles + dates. Existing closest is `FeatureSpotlight` which forces a 2-4 column grid. Need a plain text-list section with title + date + linked entry.
- **Reading-measure typography preset** — body width should cap at ~70ch for readability. Currently `content_width: Comfortable` is wider than ideal for prose; needs `ContentWidth::Prose` (~50-65ch).
- **Date-stamped per-page surface** — there's no canonical "published-on" field on CmsPage. Editorial sites need it.

### jvns.ca — personal blog + illustrative/comic register

**Existing substrate covers:**
- Paragraph + Heading + Code blocks (via CmsBlock)
- AssetSlug for images
- Per-tenant typography via [style.fonts]
- Post-list shape (via FeatureSpotlight or LogoCloud approximation)

**Missing:**
- **Embedded comic / illustration primitive** — jvns ships hand-drawn zines as multi-panel comic strips with captions. No CmsSection or CmsBlock currently models a comic strip; closest is `picture` which is single-image with caption.
- **Author voice frame** — site-wide author identity (name + avatar + bio + links) declared once, surfaced consistently. Currently substrate has `CmsPage.brand` (single string); no structured author voice.
- **Code-with-explanation editorial pattern** — code block followed by side-explanation, or explanation followed by code, with consistent typography. Currently Compose blocks support this loosely but no first-class primitive.
- **Personality / zine aesthetic theme** — current themes (light/dark/warm/ocean/forest/violet/rose/amoled/auto) are all "modern minimal." Zine aesthetic (hand-drawn type, marker accents, sketch borders) is its own register, not covered.

### rust-lang.org — technical documentation hub

**Existing substrate covers:**
- Multi-page sites (Loom CMS supports multiple pages)
- Code blocks via CmsBlock
- Heading hierarchy
- LogoCloud for sponsors
- Tabs / split layouts loosely via Compose

**Missing:**
- **Documentation-style sidebar TOC** — left-rail navigation tracking the article's heading hierarchy, prev/next chapter buttons. Not in current page-shell options.
- **Versioned content surface** — vN switcher in nav, content tagged with version applicability. No substrate model for versioned docs.
- **Multi-language content surface** — content keyed by locale, language switcher in chrome. Substrate has no locale primitive.
- **Code playground integration** — "Run this in playground" link from a code block. CmsBlock::Code doesn't carry a playground endpoint.
- **Community / governance / contributor surfaces** — RFC list, team-member grid (with role + photo + bio), sponsor levels — currently approximate with FeatureSpotlight but lose the typed shape.

### kinfolk.com — magazine + photographic editorial

**Existing substrate covers:**
- ImageHero with photo background
- Picture / AssetSlug for inline images
- Per-tenant [style.palette] for warm tones
- PullQuote + Epigraph
- Serif via [style.fonts]

**Missing:**
- **Full-bleed photographic section** — image spans 100% viewport width, no chrome, between body sections. Current ImageHero is hero-only; can't be used mid-flow as a divider.
- **PageKind::Magazine** (not yet enumerated; current PageKind enum has 7 variants — none is magazine-specific)
- **Generous-spacing density tier** — `DensityTier::Loose` exists in loom-tokens but not threaded through every primitive
- **Serif display type as theme-level option** — current substrate themes ship system-ui defaults; serif requires per-tenant [style.fonts] override. Need a "magazine" / "editorial" theme that ships serif display by default.
- **Issue-based content organization** — magazines have issues (Winter 2025, Spring 2026); substrate has pages but no issue surface.
- **Pull-quote at editorial scale** — current PullQuote sizes are inline/display; magazine pull-quotes are often a full column with marked-up serif drop-cap. Aesthetic gap, not contract gap.

### gov.uk — civic / accessibility-first / functional

**Existing substrate covers:**
- PageKind::Civic (shipped #405)
- Sparse-page threshold relaxed for Civic kind
- A11y phases (a11y_landmarks, contrast, etc.)
- Strict CSP + tokens-only mode
- AnnouncementBar for service alerts

**Missing:**
- **Strict-accessibility profile** — gov.uk's bar is well above WCAG 2.1 AA (font sizes, contrast ratios, focus rings, keyboard nav, screen-reader-tested copy). Substrate doesn't expose a "civic-extreme" a11y tier that elevates floors beyond standard.
- **Forms-as-first-class primitive** — gov.uk's core surface is multi-step task-oriented forms (apply for X, register Y). Substrate has `CmsSection::Form` but it's basic; no multi-step / save-and-resume / back-button-navigation / progress-indicator support.
- **Plain-language audit phase** — Flesch-Kincaid / readability scoring as an audit phase. Doesn't exist.
- **Translation + locale surface** — same gap as rust-lang.org for multi-language content.
- **Service-status banner primitive** — typed "this service is currently degraded / under maintenance" with structured remediation. Approximate via AnnouncementBar but loses the typed shape.
- **Task-oriented information architecture** — "Start now" button leading into a flow; flow's pages tagged as steps in a typed task. No substrate primitive for "task = sequence of pages with progress state."

## Aggregate substrate roadmap

Each gap mapped to a substrate work item, prioritized by how many of the 5 reference sites it serves:

### Tier 1 — affects 3+ sites (high priority)

1. **Multi-language / locale surface** (rust-lang, gov.uk, partially kinfolk for international issues) — new CmsPage.locale field + per-page i18n bundle. Substantial substrate work.
2. **Generous-spacing density tier + editorial / magazine theme** (kinfolk, paulgraham, jvns) — extend existing theme system to include serif-default + loose-spacing register. Bounded design work.
3. **Reading-measure / Prose ContentWidth variant** (paulgraham, jvns, kinfolk for body text) — add `ContentWidth::Prose` (~50-65ch). Single substrate change.
4. **No-chrome page-shell variant** (paulgraham, jvns minimal posts, gov.uk task-flow steps) — add `ChromeKind::None`. Single substrate change.

### Tier 2 — affects 2 sites (medium priority)

5. **Author voice / identity surface** (jvns, kinfolk) — typed Author struct with name + bio + avatar + links + voice profile. New primitive.
6. **Date-stamped editorial surface** (paulgraham, jvns) — CmsPage.published_at field, date-rendered consistently. Single substrate change.
7. **Documentation sidebar-TOC + prev/next** (rust-lang, partially kinfolk for in-issue navigation) — new page-shell variant + nav surface. Substantial design.
8. **Versioned content** (rust-lang docs, gov.uk service-version updates) — new CmsPage.version field + version-switcher primitive.

### Tier 3 — affects 1 site (specific gaps)

9. **Embedded comic / illustration primitive** (jvns) — new CmsSection variant for multi-panel illustration. Design-led.
10. **PageKind::Magazine** (kinfolk) — additional PageKind variant + magazine-specific thresholds.
11. **Full-bleed photographic divider** (kinfolk) — new CmsSection variant or extend ImageHero to mid-flow use.
12. **Multi-step forms primitive** (gov.uk) — substantial new substrate (form-flow state machine).
13. **Plain-language audit phase** (gov.uk) — new audit phase, Flesch-Kincaid + style guide enforcement.
14. **Strict-accessibility profile** (gov.uk) — extend a11y phases with civic-extreme tier.
15. **Service-status banner** (gov.uk) — typed primitive distinct from AnnouncementBar.
16. **Code playground integration** (rust-lang) — extend CmsBlock::Code with optional playground_url.
17. **Issue-based content organization** (kinfolk) — substantial new content model.
18. **Personality / zine aesthetic theme** (jvns) — new theme entry with hand-drawn / marker / sketch tokens. Design-led.

## Aggregate primitive surface implications

If the substrate ships Tier 1 + Tier 2 (8 items, ~6 substrate changes), it covers most observable gaps across all 5 reference sites. Tier 3 is site-specific niche work; not all of it ships before "substrate covers our reference set."

## Mapping to #355-#360 audit tasks

- **#355 audit primitive reachability** — uses this catalog to identify which primitives ARE in the substrate but not being reached for in tenant content; complements gap-list with reach-list
- **#356 audit decorative primitive coverage** — Tier 1 items #2 + #3 + Tier 3 #18 are the design tasks here
- **#357 audit compositional primitive coverage** — Tier 1 #4 (no-chrome) + Tier 2 #7 (sidebar-TOC) are the comp gaps
- **#358 grow theme system** — Tier 1 #2 (editorial/magazine theme) + Tier 3 #18 (zine) are the theme work
- **#359 audit content-model BlockKind coverage** — Tier 2 #5 #6 + Tier 3 #9 #10 #11 #12 #15 #16 #17 are the BlockKind work
- **#360 neutralize defaults** — implicit; the new themes + ContentWidth variant + page-shell options remove the SaaS-default-everywhere problem

## Honest scope notes

This catalog assumes the substrate is the rendering layer for these sites. In practice, paul has already decided (per `feedback_dont_pixel_reproduce_outside_band`) that out-of-band sites should NOT be reproduced on Forge. So this catalog isn't a "build these sites" plan — it's a substrate vocabulary roadmap. The substrate grows toward expressing these registers over months/years; individual sites in the registers get built off-Forge in the meantime.

Tier 1 is months of work. Tier 2 is another quarter. Tier 3 is open-ended. Don't optimize for "ship all of this fast" — optimize for one register at a time, each delivered well, with the discipline the substrate doctrine requires.
