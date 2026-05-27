# Substrate default-neutralization audit — 2026-05-27

Per task #360. Surveys the substrate's `#[default]` markers and
hard-coded fallback choices to identify where SaaS-marketing-band
calibration is silently baked in, then proposes the per-PageKind
dispatch path forward.

## Methodology

Counted `#[default]` derive markers across the substrate layers:

| Crate | `#[default]` sites |
|---|---|
| `loom-cms-render` | 51 |
| `loom-tokens` | 9 |
| `forge-core` | 13 |
| **Total** | **73 default choices** |

For each, the question is: does omitting the field bias the rendered
output toward one band (SaaS marketing landing) over the others
(brief, editorial, documentation, civic)?

## Findings

### Tier 1 — explicitly SaaS-band defaults (with code-comment evidence)

These are the cases where the substrate code-comments themselves
flag the default as the "legacy" SaaS choice:

1. **`FeatureSpotlightDecoration::Decorated`** (cms-render:5023)
   - Comment: "*The default `Decorated` is the legacy SaaS-card shape
     (rounded chrome + gradient icon tile + hover lift + shadow)*"
   - Impact: every FeatureSpotlight section that omits decoration
     renders as SaaS card chrome regardless of PageKind
   - Reach: 30 across observed content (top 5 most-reached primitive)

2. **`TestimonialDecoration::Decorated`** (cms-render:5054)
   - Comment: "*`Decorated` (default) is the legacy avatar+quote card.
     Back-compat default.*"
   - Impact: testimonials in editorial / brief contexts get card
     chrome
   - Reach: 6 across observed content

3. **`HeroBackground::GradientMesh`** (cms-render:5492)
   - Comment: implicit; `GradientMesh` is the SaaS-modern visual
   - Impact: every Hero without explicit background flashes
     gradient mesh — a strong SaaS-modern marker

### Tier 2 — implicit SaaS-band defaults (no comment, but bias clear)

4. **`ChromeKind::PageShell`** (cms-render:5474) — full sticky-nav
   header chrome. paulgraham + brief pages want `ChromeKind::None`
   or `ChromeKind::Minimal`.

5. **`DensityTier::Comfortable`** (loom-tokens; cms-render multiple
   sites at 1994, 4817, 4845, 4890, 4901, 5559, 5901) — the SaaS
   median spacing. kinfolk-shape editorial wants `Loose`; rust-lang
   docs want `Dense`; gov.uk task-flow wants `Dense`.

6. **`HeroEditorialBackground::Slate`** (cms-render:4873, 4977,
   4918) — Slate is the SaaS-modern dark-card register; editorial
   wants `Plain` (cream / warm-neutral) by default for kinfolk-shape;
   AMOLED wants its own register.

7. **`ButtonVariant::Primary`** (cms-render:5212) — strong filled CTA.
   Gov.uk wants accessible-secondary defaults; editorial wants
   ghost / link-style.

### Tier 3 — band-neutral defaults (no neutralization needed)

These defaults are universal-good choices, not band-biased:

- `MarqueeSpeed::Medium` (1994)
- `StepperState::Upcoming` (2030)
- `StatTrendDirection::Flat` (2092)
- `ToastTone::Info`, `BadgeTone::Neutral`, `AlertTone::Info`
- `ListStyle::Unordered`
- `MfaPromptMethod::Totp`
- `CaptchaDifficulty::Easy`
- `PullQuoteEmphasis::Inline`
- `RevealAnimation::FadeUp`
- `CardCornerRadius::Rounded`
- `IframeSandbox::default()` (the strictest sandbox)
- `PhotoOverlay::None`

### Tier 4 — defaults that should be removed entirely (no Default)

For high-leverage band-defining choices, the substrate should *not*
have a default — operators MUST choose explicitly. These are the
candidates:

- `HeroBackground` (currently `GradientMesh`) → remove `#[default]`
- `FeatureSpotlightDecoration` → remove `#[default]`
- `TestimonialDecoration` → remove `#[default]`
- `ChromeKind` → remove `#[default]`

The cost: every existing tenant `forge.toml` that omits these
fields would start failing build. Migration: emit a deprecation
warning for one cycle (Warn phase finding), then promote to Strict.

## Per-PageKind dispatch table (preferred neutralization)

The cleaner path than removing defaults is **PageKind-driven
defaulting**. `SiteIdentity.kind` already exists (added in #405).
Extend the default resolution: when a struct field is absent, the
renderer looks up the per-PageKind default before falling back
to the type-level default.

Proposed dispatch table:

| Field | `MarketingLanding` | `Brief` | `Portfolio` | `Editorial` | `Documentation` | `Civic` | `Commerce` |
|---|---|---|---|---|---|---|---|
| `ChromeKind` | `PageShell` | `None` | `Minimal` | `Minimal` | `PageShell` | `PageShell` | `PageShell` |
| `HeroBackground` | `GradientMesh` | (no hero) | `Solid` | `Photo` | `Solid` | `Solid` | `GradientMesh` |
| `FeatureSpotlightDecoration` | `Decorated` | `Minimal` | `Editorial` | `Editorial` | `Minimal` | `Minimal` | `Decorated` |
| `TestimonialDecoration` | `Decorated` | (no testimonials) | `Editorial` | `Editorial` | (no testimonials) | (no testimonials) | `Decorated` |
| `DensityTier` | `Comfortable` | `Dense` | `Comfortable` | `Loose` | `Dense` | `Dense` | `Comfortable` |
| `HeroEditorialBackground` | `Slate` | `Plain` | `Plain` | `Plain` | `Plain` | `Plain` | `Slate` |
| `ButtonVariant` default | `Primary` | `Ghost` | `Ghost` | `Ghost` | `Primary` | `Secondary` | `Primary` |
| `Theme` candidate pool | warm, light, dark, ocean | light, dark, amoled | warm, rose, light, dark | editorial, warm, rose | light, dark | light, dark | warm, light, dark, ocean |

## Implementation surface

Bounded substrate work to land per-PageKind defaulting:

1. **New module: `forge-core::page_kind_defaults`** — one function per
   defaultable type returning the per-PageKind value. Reads
   `SiteIdentity.kind` and returns the appropriate default.

2. **Render integration** — when a `CmsSection` field is `Option<T>`
   and `None`, the renderer calls into the dispatch table before
   falling back to `T::default()`. This is the largest change since
   every section that uses defaults needs the threading.

3. **Audit phase: `default_band_drift`** — new phase that warns when
   a tenant's PageKind doesn't match the inferred band of their
   default values. Example: `kind = "editorial"` but every
   FeatureSpotlight uses `Decorated` — flag as Warn finding.

4. **Doctrine doc** — register the per-PageKind dispatch as a
   doctrine rule with rationale: "*defaults bias output toward
   one band; PageKind must drive defaulting to avoid silent
   over-fit to consumer SaaS marketing landing.*"

## Effort estimate

- Module + dispatch table + tests: 1-2 days
- Render integration (per section): 1 day per ~5 sections
- Audit phase: half-day
- Doctrine doc: 1 hour

Total: ~1-2 weeks of focused work to fully neutralize Tier 1 + Tier 2.

## Mapping to existing tasks

- **#405** (PageKind in SiteIdentity) — shipped; this audit builds on it
- **#358** (theme system growth) — editorial theme shipped; per-band
  theme dispatch is part of this work
- **#347** (kinfolk-shape investigation) — the audit's central finding
  validates the "consumer-band calibration" diagnosis from #347
- **#392-#395** (default-fragmentation pools) — per-band defaulting
  works orthogonally with fragmentation pools (per-tenant identity
  picks from per-band pool)

## Honest scope note

This audit produces the survey + dispatch table. It does NOT ship the
code change. The dispatch table is design-led — the per-band values
need design review before they bake into the substrate. The audit
output is the *roadmap* for the eventual code change, not the change
itself.

The audit also doesn't address **render-time band-drift detection**
beyond the proposed audit phase. A more aggressive approach would be
to refuse build entirely when the PageKind / defaults mismatch is
severe — that's a doctrine call, not an audit finding.

## Conclusion

The substrate has 73 `#[default]` markers across 3 crates. Of those:

- **3 are explicitly SaaS-band** (FeatureSpotlight, Testimonial,
  HeroBackground) per code-comment evidence
- **4 are implicitly SaaS-band** (ChromeKind, DensityTier,
  HeroEditorialBackground, ButtonVariant)
- **~66 are band-neutral** and stay as-is

The remediation is per-PageKind defaulting in `forge-core`, render
threading, an audit phase, and a doctrine doc. Effort: 1-2 weeks.
Outcome: substrate output stops silently favoring SaaS-marketing
band when PageKind says otherwise.
