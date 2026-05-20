# Editorial Purity — radical anti-SaaS-trope doctrine

**Status:** doctrine. Per paul 2026-05-20 directive: "you need to
radically improve forge so [SkillShots-shape sites] stop
happening. every line. every file, every inch of forge." The
substrate had 160 typed primitives but operators were still
reaching for SaaS-trope variants (`Hero`, `StatBand`, `Pricing`
with `highlighted: true`, `Testimonial` with avatar circles) by
default. This doctrine + the `editorial_purity_gate` phase
shift the substrate from "primitives are available; operators
choose" to "primitives are REQUIRED; SaaS-trope variants REFUSE
to build."

---

## What the gate refuses

When `forge.toml [editorial_purity] enforce = true`, the build
emits strict findings for every flagged trope:

| Trope kind                                  | What's banned                                | Use instead                                  |
|---------------------------------------------|----------------------------------------------|----------------------------------------------|
| `editorial-purity.saas-hero`                | `CmsSection::Hero` (centered SaaS default)   | `CmsSection::HeroEditorial`                  |
| `editorial-purity.centered-single-line-hero`| Hero title < 30 chars + no lede + no eyebrow | Multi-clause title + lede OR different primitive |
| `editorial-purity.feature-spotlight-grid`   | `FeatureSpotlight` with 3+ columns/items     | `KvPairCard` dense info panels               |
| `editorial-purity.stat-band`                | `StatBand` variant — "Numbers that compose"  | `Sparkline` / `Histogram` / per-metric editorial |
| `editorial-purity.pricing-most-popular`     | `Pricing` with any tier `highlighted: true`  | Drop the highlight                           |
| `editorial-purity.testimonial-card-avatar`  | `Testimonial` with `avatar_slug` set         | `PullQuote` (left-border, no avatar)         |
| `editorial-purity.cookie-notice-cta`        | Empty/`No`/`Dismiss` reject_label            | Full reject label as prominent as accept     |

Every banned shape has an EXISTING substrate-shipped editorial
counterpart. Operators that hit a trope finding can ALWAYS
migrate.

## How to enable

```toml
[editorial_purity]
enforce = true

# Optional exemption list. Use sparingly — defeats the gate.
# exempt = ["editorial-purity.saas-hero"]
```

Without the `[editorial_purity]` section the phase is silent —
back-compat for sites that haven't migrated.

## How to migrate a site

1. `forge build` with `enforce = true` to enumerate tropes.
2. For each finding:
   - **`saas-hero`** → rewrite `"kind": "hero"` → `"kind":
     "hero_editorial"`. Add monospace kicker, editorial background.
   - **`centered-single-line-hero`** → expand title to 30+ chars
     + add lede OR eyebrow.
   - **`feature-spotlight-grid`** → rewrite to `kv_pair` with 5+
     label/value entries. Target Comfortable–Dense per `DensityTier`.
   - **`stat-band`** → rewrite to `sparkline` / `histogram` with
     real data, OR `pull_stat` if only one metric.
   - **`pricing-most-popular`** → every tier `highlighted: false`.
   - **`testimonial-card-avatar`** → rewrite to `pull_quote` with
     body + plain attribution. Drop avatar.
   - **`cookie-notice-cta`** → expand `reject_label` to full phrase
     ("Decline non-essential cookies").
3. Re-run `forge build`. Iterate until findings == 0.
4. Commit migrated `cms/*.json`.

## What this doctrine does NOT do

* Does NOT remove the SaaS-trope variants from the substrate —
  they still exist for non-editorial sites, legacy data, A/B
  testing. The gate REFUSES them only for sites that opt in.
* Does NOT auto-rewrite CMS JSON. Future `forge editorial-rewrite`
  CLI subcommand can land separately; today the operator migrates
  by hand.
* Does NOT cover EVERY possible SaaS trope. v1 ships the 7
  most-common shapes. Future axis adds: gradient-text-clip,
  marquee-logo-wall, numbers-that-compose-heading, centered-CTA-
  band-with-gradient. Per-tenant corpora can extend.

## Why strict-by-flag (not strict-by-default)

1. Existing sites don't break on substrate update.
2. Sites that explicitly want editorial discipline opt in.
3. The opt-in IS the doctrine commitment — "this site stays
   editorial; the build refuses to drift toward SaaS-trope
   patterns over time."

## Future axis adds (queued for v2)

* **Gradient-clipped marketing text** — Crawler-side runtime
  detection of `background-clip: text` + gradient.
* **Marquee logo wall** — `LogoWall` with > N entries +
  scroll-animation = "Trusted by these companies" trope.
* **"Numbers that compose" heading** — `Heading` text matching
  JARGON_PHRASES patterns.
* **Centered CTA band with gradient + 2 buttons** — the pre-
  footer "Get started" SaaS trope.
* **`forge editorial-rewrite` CLI subcommand** — auto-migration:
  Hero → HeroEditorial, FeatureSpotlight 3-col → KvPair,
  StatBand → Sparkline, etc.

This doctrine is the substrate's strongest forcing function for
the "radical improvement" directive. v1 ships the gate; v2
expands the trope dictionary; v3 ships the auto-rewriter.
