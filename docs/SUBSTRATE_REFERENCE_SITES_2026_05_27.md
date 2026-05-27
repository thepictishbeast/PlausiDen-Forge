# Substrate reference sites — 2026-05-27

Five reference sites picked outside Forge's current SkillShots-shape
consumer band. These drive the substrate vocabulary roadmap per
`docs/SUBSTRATE_REFRAME_2026_05_21.md` § "Pick 5 reference sites."

Each site represents a distinct aesthetic register Forge's
vocabulary must eventually express. The gap analysis (task #348)
catalogs what each site contains that the substrate currently
lacks; the aggregate gap list drives substrate roadmap priority
across #355-#360.

## The five sites

### 1. paulgraham.com — minimal text-only essay

- **URL:** https://www.paulgraham.com/
- **Register:** Editorial / brief / text-only
- **Why this site:** the canonical example of "a site where content IS the design." No images, no decoration, no nav chrome, just a list of essay titles linking to single-column text pages. Tests whether Forge can express minimal text-only sites without forcing in SaaS-shape chrome.
- **Likely substrate gaps:**
  - PageKind::Brief / Editorial (shipped #405)
  - Centered prose layout with reading-measure cap
  - No-decoration single-page composition
  - Per-tenant "no nav chrome" option

### 2. jvns.ca — personal blog with illustrative + comic register

- **URL:** https://jvns.ca/
- **Register:** Editorial blog + illustrative/comic content
- **Why this site:** Julia Evans's site combines text essays with hand-drawn zines, comic-strip explainers, and embedded code. The aesthetic is personality-forward without being marketing-shape. Tests illustrative-content composition + author-voice rendering.
- **Likely substrate gaps:**
  - Embedded illustration / comic-strip primitives (custom layout, not just `picture`)
  - Author-bio + voice-frame primitives
  - Code-block-with-explanation editorial pattern
  - Date-stamped post-list shape
  - Aesthetic register that's personal/zine, not corporate

### 3. rust-lang.org — technical documentation hub

- **URL:** https://www.rust-lang.org/
- **Register:** Technical documentation + community-led
- **Why this site:** structured navigation across docs / blog / governance / sponsors / community. Multi-page with consistent typographic system. Tests documentation-shape primitives + how the substrate handles multi-page typed sites.
- **Likely substrate gaps:**
  - Documentation-style navigation (sidebar TOC, prev/next chapter)
  - Code-block-with-syntax-highlighting + run-in-playground integration
  - Versioned docs (vN switcher)
  - Community / governance / contributor-list primitives
  - Multi-language / multi-locale (Rust docs ship in many languages)

### 4. kinfolk.com — editorial magazine + photographic

- **URL:** https://www.kinfolk.com/
- **Register:** Magazine / photo-led / luxury-editorial
- **Why this site:** large-format photography drives composition, long whitespace, serif display type, generous margins. The aesthetic opposite of dense-information sites — slow, atmospheric, image-led. Tests whether substrate can express photographic-led editorial shape.
- **Likely substrate gaps:**
  - PageKind::Magazine (not yet enumerated)
  - Full-bleed photo-as-section-divider
  - Serif display type as first-class option (currently only system-ui body fonts)
  - Generous-spacing density tier (loose / luxury / editorial)
  - Issue-based content organization
  - Pull-quote + epigraph at editorial scale

### 5. gov.uk — civic information

- **URL:** https://www.gov.uk/
- **Register:** Civic / accessibility-first / functional
- **Why this site:** UK government services site. Famously the gold-standard of "content as service" — strict accessibility, plain-language, no decoration, task-oriented. Tests whether substrate can express civic-functional shape (where decoration is actively undesirable).
- **Likely substrate gaps:**
  - Strict-accessibility profile (currently substrate is generally accessible but doesn't have a "civic-extreme" tier)
  - Task-oriented information architecture primitives
  - Forms-as-first-class (multi-step civic forms with progress + back-button + save-and-resume)
  - Plain-language audit (Flesch-Kincaid scoring, etc.)
  - Translation / multi-lingual content surface (gov.uk supports dozens of languages)
  - Service-status indicators (banners for outages / changes)

## Why these five (vs others)

Picked to maximize aesthetic-register spread while staying real:

- All five are publicly accessible, well-known, observably distinct
- Together they cover: brief (paulgraham) / editorial-illustrative (jvns) / documentation (rust-lang) / magazine-photographic (kinfolk) / civic-functional (gov.uk)
- None of them is SaaS-marketing-shape (Forge's current band)
- Each is mature + stable + likely to be available for repeat inspection

Other candidates considered + skipped:
- **The Verge** — closer to Forge's current band (tech-publication, lots of marketing-style chrome)
- **News.ycombinator.com** — too minimal to test compositional surface
- **Stripe docs** — overlaps rust-lang.org for the documentation niche
- **Drudge Report / brutalist sites** — register is real but ages out fast; not stable for repeat inspection
- **Personal portfolios** — too variable; pick a specific designer's site if a portfolio register is needed later

## Process for using these in audits #355-#360

1. **#348** — catalog gaps per site (what each contains that Forge can't express)
2. **#355** — audit substrate primitive reachability against these sites
3. **#356** — audit decorative primitive coverage
4. **#357** — audit compositional primitive coverage
5. **#358** — grow theme system to cover at least 3 of the 5 register registers
6. **#359** — audit content-model BlockKind coverage
7. **#360** — neutralize default state so new sites can occupy any of these registers

The five-site set is the canonical benchmark. Substrate roadmap progress = how many of these the substrate can express well.

## Refresh policy

Re-inspect every ~12 months. Sites evolve; their register may shift; the substrate's coverage of them is the durable measurement. Add or rotate references only when one becomes unavailable or its register collapses into the SaaS band.
