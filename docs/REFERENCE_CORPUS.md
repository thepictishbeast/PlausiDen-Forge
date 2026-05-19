# Reference Corpus + Density Tiers

**Status:** doctrine + roadmap. Defines the reference corpus the
`aesthetic_distinctiveness` audit phase compares Forge output against,
and the density tiers tenants declare to opt into stricter / looser
content thresholds.

Per `feedback_consumer_shaped_substrate`: the substrate has no
intrinsic pressure toward density or distinctiveness, only toward
correctness. Without a measurable reference, the substrate's
"complete-site" sense drifts toward whatever the first consumers
shipped. A curated corpus + tiered thresholds make density a
first-class concern at audit time.

## Reference corpus — 20 sites worth measuring

Sites picked because their density / typography / composition
discipline rewards study. Not "best-looking SaaS"; **substantively
considered**. Each gets a density-measurement entry tracked over
time. Re-measure annually or on major redesigns.

### Editorial-press tier

| Site                       | What to study                                              |
|----------------------------|------------------------------------------------------------|
| nytimes.com (any article)  | Drop caps, marginalia, lead paragraph, body density        |
| theatlantic.com            | Long-form measure, custom-type-driven hero                 |
| stripe.com/press           | Editorial-on-tech-substrate; gradient discipline           |
| craigmod.com               | Single-author voice; minimal but maximalist typography     |
| matuzo.at                  | Long-form web design analysis; honest density              |
| paulgraham.com             | Density-via-text-only; 0 chrome; reading-first             |

### Technical-product tier

| Site                       | What to study                                              |
|----------------------------|------------------------------------------------------------|
| stripe.com                 | Asymmetric hero, in-line code visuals, gradient discipline |
| linear.app                 | Big-confident type, OKLCH palette, animation restraint     |
| vercel.com                 | Demo-as-hero, dense feature density without overcrowding   |
| anthropic.com              | Editorial typography on a tech site, hero with code mock   |
| render.com                 | Pricing transparency, dense feature pages                  |
| fly.io                     | Density via running prose + code blocks intermixed         |
| supabase.com               | Open-source-honest; dense docs as marketing                |

### Sovereign / privacy-tier

| Site                       | What to study                                              |
|----------------------------|------------------------------------------------------------|
| signal.org                 | Calm density; doesn't rely on movement                     |
| eff.org                    | Editorial-press on an advocacy site; running prose         |
| tailscale.com              | Technical density; refuses marketing-speak                 |
| sourcehut.org              | Anti-spectacle SaaS landing; works without JS              |

### Anti-corpus (slop reference targets)

| Site shape                 | Why it's in here                                           |
|----------------------------|------------------------------------------------------------|
| Generic Webflow agency template | Three centered "we deliver excellence" sections        |
| Most YC company landing v1     | Hero / 3-col features / pricing / testimonial / CTA     |
| Crypto-token launch site       | Neon gradient + animated background + jargon-dense    |
| AI-coding-tool generic landing | Same hero / same value-prop / same "demo" video       |
| Wix-default business site      | Centered hero / generic stock photo / 3-feature grid  |

These four shapes are what the slop dictionary's `centered_single_word_hero`
/ `monotonous_feature_grid` / `numbers_that_compose` / `most_popular_badge`
detectors target.

## Density-measurement schema

For each site in the reference corpus, the corpus entry should
declare:

```json
{
  "site": "stripe.com",
  "url": "https://stripe.com",
  "measured_at": "2026-05-19",
  "tier": "technical-product",
  "viewport": 1280,
  "metrics": {
    "word_count_above_fold": 142,
    "word_count_total": 2840,
    "section_count": 11,
    "distinct_primitive_count": 8,
    "image_count": 6,
    "code_block_count": 3,
    "link_density": 0.07,
    "h1_h2_ratio": 1.0
  },
  "slop_score": 0.04
}
```

`slop_score` is the slop-dictionary match count divided by section
count — low is good. Reference-corpus sites should score < 0.1.

## Density tiers — what tenants opt into

Three named tiers + a custom escape hatch. Declared in
`forge.toml`:

```toml
[aesthetic_distinctiveness]
density_tier = "press"   # editorial / press / commerce / minimal / custom
```

### `press` (highest density)

* Word-count floor: 600 above fold + 2000 total.
* Section count floor: 8.
* Distinct primitive count floor: 5.
* Image desert: strict on any page with < 3 image / icon refs.
* Slop dictionary: strict (every named anti-pattern fails the build).

For editorial-press tier sites (publications, long-form
journalism, manifesto landings).

### `editorial` (default)

* Word-count floor: 400 above fold + 1200 total.
* Section count floor: 6.
* Distinct primitive count floor: 4.
* Image desert: warn.
* Slop dictionary: warn.

For technical-product / sovereign-tier sites that lead with prose.

### `commerce` (loose)

* Word-count floor: 200 above fold.
* Section count floor: 5.
* Distinct primitive count floor: 3.
* Image desert: warn (commerce pages usually have product imagery).
* Slop dictionary: warn-most, strict on green-checkmark + most-popular-
  badge (commerce tropes that hurt conversion in 2026).

For storefront / pricing / signup flows where the reading-and-
considering is downstream of the click.

### `minimal` (lowest density)

* Word-count floor: 60.
* Section count floor: 3.
* Distinct primitive count floor: 2.
* Image desert: silent.
* Slop dictionary: silent (don't apply — single-page apps,
  utilities, /signin / /404 / process-step pages).

### `custom`

Tenant provides explicit thresholds in
`[aesthetic_distinctiveness.thresholds]`. Bypasses the named-tier
defaults.

## How the audit phase consumes this

Right now (commit 2953320) the `aesthetic_distinctiveness` phase
runs a fixed 13-pattern slop dictionary unconditionally. After this
doc lands as doctrine the phase should:

1. Read `[aesthetic_distinctiveness] density_tier` from forge.toml
   (default `editorial`).
2. Apply the tier's thresholds + slop-dictionary severity overrides.
3. Optionally read a per-tenant `corpus.json` declaring 1-N "our
   prior sites" measurements; emit findings when the new build
   regresses from the tenant's own baseline. This is the
   "per-tenant private corpus" from
   `feedback_consumer_shaped_substrate` (anti-similarity to your
   own past output).

## Dogfooding

Per the memory's "dogfooding fix":

> "PlausiDen.com / Sacred.Vote / the platform's own marketing
> site — wherever they're currently hand-built outside Forge —
> get rebuilt **with Forge**, at the density level you want
> commercial customer sites to achieve."

When prosperityclub.com and sacred.vote rebuilds land via Forge
(tasks #112, #118), they MUST opt into the `editorial` or `press`
tier so the corpus measurements are honest. The
`aesthetic_distinctiveness` phase running against the rebuilds
surfaces gaps that get fixed as substrate work, not as one-off
overrides.

## Acceptance test

This doc closes #109 when:

* Reference corpus enumerated (above — 20 sites).
* Density-measurement schema declared.
* Tier system declared with concrete thresholds.
* Roadmap for tenant-corpus + per-tenant density baselines.
* Dogfooding directive recorded.

Future work (file as new tasks if you want them in the backlog):

* corpus.json schema + a `forge corpus measure <url>` CLI that
  walks a live page and emits the JSON.
* `aesthetic_distinctiveness` phase tier-aware threshold logic.
* CI step that re-measures the reference corpus monthly so
  Forge's notion of "what dense looks like in 2026" stays current.
