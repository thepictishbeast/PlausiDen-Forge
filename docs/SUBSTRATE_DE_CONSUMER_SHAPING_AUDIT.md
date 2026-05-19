# Substrate-vs-Theme Audit (Forge + Loom)

**Status:** structural audit. Categorizes every Loom primitive +
Forge phase as one of:

* **substrate** — would exist in the same shape regardless of the
  first consumers. Keep as-is.
* **consumer-shaped tuning** — exists everywhere, but with
  thresholds / defaults / specific check logic calibrated against
  PlausiDen's first consumers. Tune dial, don't restructure.
* **consumer-shaped** — only exists in this shape because of
  consumers. Needs a substrate-vs-theme split.
* **aesthetic / theme** — visual decoration. Caller-replaceable.

Per `feedback_consumer_shaped_substrate`: the substrate grew up
alongside SkillShots / plausiden.com / Sacred.Vote and silently
encoded their aesthetic assumptions. This audit identifies the
consumer-shaped items so iteration N+1 can split each one.

## Diagnostic question

For each item, ask:

> Would this exist, in this shape, if the first consumer had been
> a completely different kind of site — editorial magazine,
> government portal, photography portfolio, academic publication,
> e-commerce shop, SaaS dashboard?

* "Yes, mostly the same" → **substrate**.
* "Yes, but the thresholds would differ" → **tuning**.
* "No, this is the X-shaped version of a more general concept" →
  **consumer-shaped**.
* "No, this is purely the visual layer" → **aesthetic / theme**.

## Loom primitives (CmsSection variants, 132 today)

### Structural — substrate

These are layout / container primitives that exist in every
substrate. Keep.

| Primitive       | Why substrate                                        |
|-----------------|------------------------------------------------------|
| Group           | Generic grouping wrapper                             |
| Sidebar         | Generic side-panel container                         |
| Container       | Bounded-width wrapper                                |
| Divider         | Section separator                                    |
| Spacer          | Vertical breathing room                              |
| Banner          | Top-of-page notice                                   |
| Picture         | Generic image                                        |
| Paragraph       | Body text (now with editorial decoration variants)   |
| Heading         | Section heading                                      |
| KvPair          | Definition list                                      |
| Code            | Fenced code block                                    |
| Marginalia      | Editorial sidenote                                   |
| PullQuote       | Editorial quote (distinct from testimonial)          |
| LogoWall        | Brand-mention row                                    |
| Faq             | Question/answer accordion                            |
| Steps           | Numbered process                                     |
| Comparison      | Tabular comparison                                   |
| Marquee         | Kinetic text band                                    |
| Timeline        | Date-keyed event list                                |
| Roadmap         | Now/Next/Later commitment list                       |

### Consumer-shaped — needs substrate/theme split

These exist because PlausiDen consumers are SaaS/marketing
landings. Each needs the variant-explosion + slot composition
treatment from `feedback_consumer_shaped_substrate`.

| Primitive             | What's consumer-shaped                        | Substrate move                                                          |
|-----------------------|-----------------------------------------------|-------------------------------------------------------------------------|
| Hero                  | Single-CTA, optional eyebrow + lede           | Add slots: `before_headline`, `after_cta`, `decorative_background`     |
| ImageHero             | Gradient-mesh default + centered text         | Already de-troped via Photo bg + ParagraphDecoration on lede           |
| SplitHero             | text + visual side-by-side, fixed swap        | Add asymmetric variants: offset, layered, diagonal                    |
| FeatureSpotlight      | 3-column grid of icon+title+body cards        | Already partially de-troped; add list / detailed / 2-up variants      |
| StatBand              | Big-numbers row, default gradient values      | Already de-troped (default flat, opt-in gradient/carded)              |
| Pricing               | 3-tier card row + highlighted middle          | Already de-troped (flat default, opt-in carded/centered)              |
| CallToAction          | Single eyebrow + title + lede + CTA           | Add slot: optional secondary action / disclaimer                      |
| Quote                 | Testimonial card with attribution             | Split into Quote (substrate) + TestimonialCard (theme)                |
| AuthCard              | Bottom-of-form footer + dividers              | Substrate; tuning per tenant — keep                                   |
| CrucibleWidget        | Image-classify / similarity / arithmetic etc. | Substrate (challenge runtime), tuning per tenant                      |

### Aesthetic / theme — caller-replaceable

These ARE the theme. Tenants override entirely via `data-theme`,
or via dedicated `theme=X` override blocks in skin.css. Already
the 14 named themes shipped (light / dark / dark-amoled / auto /
warm / ocean / forest / violet / rose / sepia / press / hc-dark /
hc-light + per-tenant).

| Token group                           | Theme axis                       |
|---------------------------------------|----------------------------------|
| `--loom-color-bg-canvas`              | bg                               |
| `--loom-color-ink` / `--loom-color-ink-muted` | text                       |
| `--loom-color-primary` / `--loom-color-accent` | brand                     |
| `--loom-radius-*` / `--loom-shadow-*` | chrome                           |
| `--loom-font-display` / `--loom-font-body` / `--loom-font-mono` | typeface |
| `--loom-motion-*` / `--loom-ease-*`   | motion                           |

### Consumer-shaped — page-shell defaults

Sneakier than primitive-shape: defaults built into the page-
shell renderer.

| Default                                          | Status            | Fix                                                                                  |
|--------------------------------------------------|-------------------|--------------------------------------------------------------------------------------|
| `<main id="content">` landmark                   | substrate         | Keep — WCAG-correct                                                                  |
| `max-width: 64rem` on main#content               | consumer-shaped   | Move to a typed `Container.max_width` slot on CmsPage. Default 64rem, tenant override. |
| Sticky `<header>` with backdrop-filter blur      | consumer-shaped   | Already de-troped via ChromeKind enum (PageShell / FloatingPill / Minimal)            |
| Body backdrop = 3 radial gradients               | consumer-shaped   | Already de-troped via FloatingPill chrome dropping the radial gradients               |
| 30-line inline theme-toggle JS                   | consumer-shaped   | Pending #102 (CSS-only `:has()` or WASM port)                                         |
| Default 13 themes                                | tuning            | Number is arbitrary; structure (per-theme `:root[data-theme=X]`) is substrate          |

## Forge phases (43 today)

### Substrate

Phases that check universal correctness — would exist regardless
of consumer.

| Phase                | Why substrate                                        |
|----------------------|------------------------------------------------------|
| validate_cms         | Typed CMS input gate                                 |
| render               | CMS → HTML pipeline                                  |
| html_walk            | DOM walker shared infra                              |
| self_check           | Build-output sanity                                  |
| csp                  | Security baseline                                    |
| csp_devmode          | Dev-mode CSP variant                                 |
| contrast             | WCAG-AA contrast gate                                |
| html_semantic        | Semantic markup audit                                |
| a11y_landmarks       | Landmark presence                                    |
| id_strategy          | DOM id uniqueness                                    |
| label_consistency    | Same-href consistent-label                           |
| link_check           | Internal link integrity                              |
| sri                  | Subresource integrity                                |
| seo                  | OG / meta-description / canonical                    |
| iso_8601             | Standards conformance                                |
| theme_consistency    | Tokens-defined-per-theme                             |
| theme_contrast       | Contrast ratio per theme                             |
| path_consistency     | cms.path ↔ static/file consistency                   |
| dynamic_runtime      | Runtime JS surface audit                             |
| structured_data      | Schema.org JSON-LD                                   |
| motion               | Animation declaration audit                          |
| motion_respects_reduced | prefers-reduced-motion honor                      |
| print_stylesheet     | Print CSS presence                                   |
| network_target_enforcement | Tor/I2P/Lokinet purity                         |
| reader_safety        | Tor-mode reader checks                               |
| dns_hygiene_lint     | DNS-related hygiene                                  |
| jurisdiction_compliance | Per-juris compliance markers                      |
| locale_html_lang     | `<html lang>` validity                               |
| unbuilt_route        | Route referenced but not built                       |
| required_pages       | Site-type required pages                             |

### Consumer-shaped tuning

| Phase                | Why tuned                                            | Substrate move                                              |
|----------------------|------------------------------------------------------|-------------------------------------------------------------|
| tokens               | Hard-coded raw-px allowlist                          | Move allowlist to `[tokens] allow_raw_px = [...]` config    |
| asset_optimization   | Image-size thresholds tuned to current consumers      | Move thresholds to config; per-tenant override               |
| carbon_budget        | kb-per-page budget set against current consumer sites | Move to per-tier config (matches density tiers in REFERENCE_CORPUS.md) |
| perf_budget          | Same                                                  | Same — move to per-tier config                              |
| backend_coverage     | Specific backends.toml shape                          | Substrate, but the shape's a tenant-private detail           |
| phantom_button       | Same                                                  | Same                                                         |
| external_assets      | Allowlist tuned to current consumers                  | Per-tenant allowlist override                                |
| crawl                | Specific dev-server port 8123 default                 | Move port to config                                          |
| annotation_review    | PlausiDen-Annotator coupling                          | Substrate trait, current impl is consumer-coupled            |
| loom_lint            | Loom-specific                                         | Substrate (Loom is the typed UI substrate)                   |
| loom_sync            | Same                                                  | Same                                                         |

### Aesthetic / new

| Phase                          | Class                                                                  |
|--------------------------------|------------------------------------------------------------------------|
| aesthetic_distinctiveness      | Substrate (slop dictionary is data, not code; theme-replaceable)        |
| dual_theme                     | Substrate                                                              |

## Priority moves (ordered)

The audit identifies these as the highest-leverage substrate-vs-
theme splits to land:

1. **`tokens` phase raw-px allowlist → config** — currently hard-
   coded in Rust; should be `forge.toml [tokens] allow_raw_px =
   ["14px", "44px", ...]`. Lets tenants extend without forking
   Forge.

2. **`perf_budget` / `carbon_budget` thresholds → per-tier
   config** — pair with `REFERENCE_CORPUS.md` density tiers so
   `density_tier = "press"` automatically applies stricter
   carbon-budget cuts.

3. **Hero / SplitHero slot composition** — the highest-leverage
   primitive variant work per
   `feedback_consumer_shaped_substrate`. Add typed slots:
   `before_headline`, `after_cta`, `decorative_background`,
   `interactive_demo`, `below_fold`.

4. **Quote → split into Quote + TestimonialCard** — the
   editorial-vs-marketing distinction. PullQuote already exists
   for editorial; Quote stays as the testimonial-attribution
   shape (separate from Marginalia for the same reason).

5. **Theme-toggle JS → CSS-only `:has(:checked)` OR WASM** —
   already filed as #102. The 30-line JS bootstrap is the last
   ship-by-default JS surface.

6. **`max-width: 64rem` on main#content → typed slot** —
   `CmsPage.content_max_width = 64rem` with tenant override.
   Tiny change; today the value is hard-coded in the page-shell
   inline-base CSS.

## How this doc closes #103

The task is the structural audit, not the implementation. Each
"priority move" above is its own follow-up work (filed
individually if backlog space allows, OR carried in this doc as
the canonical roadmap). The audit is the deliverable.

Maintenance: re-audit when any of the priority moves lands, OR
when a major new primitive batch arrives (annotate the new
primitive's class inline as it ships, not post-hoc).
