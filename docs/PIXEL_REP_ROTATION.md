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

| Slug             | Live URL                       | Forge mirror path  | Pixel diff (avg) | Status                              |
|------------------|--------------------------------|---------------------|------------------|-------------------------------------|
| prosperityclub   | https://prosperityclub.com/    | `/` (index)         | ~45%             | Closest mirror; warm theme matches  |
| anthropic        | https://www.anthropic.com/     | `/anthropic.html`   | ~98%             | Dark vs warm; full mismatch         |
| github           | https://github.com/            | `/github.html`      | ~97%             | Forge 4× shorter; sparse editorial  |

(Sites in paul's directive that don't yet have docs: plausiden,
sacred.vote, Stripe, Linear, Vercel, Notion, Render, Fly.)

## What the rotation tells us

After three captures, the pattern is clear:

1. **Forge wins on weight + tracking**: every mirror ships
   7-15× lighter HTML, 12-25× fewer scripts, ZERO third-party
   tracking origins. The substrate's editorial discipline is
   empirically validated.

2. **Forge loses on visual fidelity by default**: 95-99% pixel
   diff for marketing-heavy sites (anthropic, github). The
   editorial warm-theme mirror doesn't approximate dark-mode +
   custom-sans + illustration-hero marketing pages.

3. **Forge approaches visual fidelity when the live site IS
   editorial**: prosperityclub ~45% diff — still substantial
   but cuts in half. The closer the live site is to an
   editorial-substrate shape, the closer the mirror reads.

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
