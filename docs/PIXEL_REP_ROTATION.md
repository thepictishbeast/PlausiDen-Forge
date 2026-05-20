# Pixel reproduction — rotation summary

Forge #218 / #112-#124. Index of all captured sites + a one-row
summary per site so the operator sees the substrate-vs-live gap
across the whole rotation at a glance.

Per-site detail lives in `PIXEL_REP_<SLUG>.md` (one file per
captured slug). Run new captures via:

```sh
make pixel-rep              SLUG=<slug> SITE_URL=<url> FORGE_PATH=/<slug>.html
make pixel-rep-visual-diff  SLUG=<slug>
```

## Captured sites

Full rotation captured 2026-05-20. Diff averaged across the 390 /
768 / 1280 viewport set:

| Slug             | Live URL                       | Forge mirror path   | Pixel diff (avg) | Notes                                                       |
|------------------|--------------------------------|---------------------|------------------|-------------------------------------------------------------|
| render           | https://render.com/            | `/render.html`      | **~27%**         | Editorial-shape live; Forge approximates well               |
| notion           | https://www.notion.so/         | `/notion.html`      | **~30%**         | Long-form docs+marketing hybrid                             |
| sacred-vote      | https://sacred.vote/           | `/sacred-vote.html` | ~43%             | Tiny live page (887-2560px tall); little to compare         |
| stripe           | https://stripe.com/            | `/stripe.html`      | ~44%             | Marketing-heavy live (14-20k tall) vs 4k Forge — diff capped by whitespace |
| prosperityclub   | https://prosperityclub.com/    | `/` (index)         | ~45%             | Closest editorial mirror; warm theme matches                |
| vercel           | https://vercel.com/            | `/vercel.html`      | ~48%             | Marketing-shape; some structural match                      |
| fly              | https://fly.io/                | `/fly.html`         | ~67%             | Heavy hero illustration + dark accents                      |
| plausiden        | https://plausiden.com/         | `/plausiden.html`   | ~67%             | Live is the Rust app; mirror is the Forge static rebuild    |
| linear           | https://linear.app/            | `/linear.html`      | ~90%             | Heavily-designed marketing — biggest structural mismatch    |
| github           | https://github.com/            | `/github.html`      | ~97%             | Full marketing funnel (12k px tall) vs 3k editorial gloss   |
| anthropic        | https://www.anthropic.com/     | `/anthropic.html`   | ~98%             | Dark mode + custom illustration heroes                      |

## What the rotation tells us

After all 11 captures, three patterns hold:

1. **Forge wins on weight + tracking**: every mirror ships
   7-15× lighter HTML, 12-25× fewer scripts, ZERO third-party
   tracking origins. The substrate's editorial discipline is
   empirically validated on every site.

2. **Pixel diff correlates with live-site marketing density**:
   the deeper the live marketing funnel (hero illustration →
   product pillars → customer logos → testimonials → footer
   city), the larger the diff. Editorial-shape live sites
   (render.com, notion.so, prosperityclub) land at 27-45%.
   Marketing-shape live sites (linear, github, anthropic) land
   at 90-98%.

3. **Sacred.vote is a control case**: live page is only
   887-2560 px tall (minimal landing). Forge mirror is 3-4×
   TALLER than live, inverting the usual relationship. Mirror
   has more substance than the source on that one — useful
   reminder that "more pixels different" doesn't always mean
   "worse mirror".

## Substrate-vs-product decision

The rotation surfaces a question paul hasn't yet answered:

**Should Forge mirrors aspire to pixel-perfect reproduction of
upstream marketing pages, or stay as editorial-substrate
glosses of the same content?**

Two paths:

- **Pixel-fidelity path**: Loom adds per-site themes (dark,
  amber, anthropic-style, github-style), custom-sans support,
  full-bleed marketing-illustration primitives. Mirrors become
  near-twins. Substrate inherits consumer-shaped patterns by
  design.

- **Editorial-substrate path**: Forge mirrors stay deliberately
  text-rich, dark-pattern-free, lightweight. Mirrors don't
  match — they IMPROVE. The pixel-diff is the FEATURE, not the
  bug. The substrate refuses to reproduce SaaS-trope shapes
  even when asked.

Paul-call. The substrate has been on the editorial-substrate
path so far (per `aesthetic_distinctiveness`,
`editorial_purity_gate`, `slop_dictionary`, and the broader
"refuse consumer-shape" doctrine in CLAUDE.md memories). The
pixel-diff numbers reflect THAT choice — they don't refute it.

## Provenance

All captures live under `PlausiDen-Crawler/runs/<slug>/`
(live) and `<slug>-forge/` (mirror). Diff overlays under
`<slug>-diff/`. The capture command, host config, and chromium
version are recorded in each `manifest.json`. Diffs are
reproducible from those artifacts via:

```sh
make pixel-rep-visual-diff SLUG=<slug>
```

The make target also accepts `PIXEL_REP_FUZZ` (default `5%`) to
tune the per-pixel color tolerance.
