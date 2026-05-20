# Pixel reproduction — anthropic.com

Forge #218 rotation step. Live state as of 2026-05-20.

Captured via `make pixel-rep SLUG=anthropic
SITE_URL=https://www.anthropic.com/ FORGE_PATH=/anthropic.html` —
the substrate machinery shipped earlier this session.

## Manifest-level deltas

| Axis | Live anthropic.com | Forge mirror | Substrate verdict |
|---|---|---|---|
| HTML size | ~120 KB | ~21 KB | Forge **5.7×** lighter |
| Live PNG (390 / 768 / 1280) | 6116 / 5083 / 3138 px tall | 4227 / 3128 / 3012 | Forge significantly shorter |
| Fonts loaded | 2 | 3 | comparable |
| Image count | 1 | 1 | match |
| Script count | 25 | 2 | Forge **12.5×** fewer |
| 3rd-party origins | 7 | 0 | Forge zero tracking |

## Visual pixel-diff

`make pixel-rep-visual-diff SLUG=anthropic` reports:

  390 px  diff = 2.36M px (99.0% of live area)
  768 px  diff = 3.86M px (98.8% of live area)
  1280 px diff = 3.92M px (97.6% of live area)

**~98% of pixels differ.** This is MUCH higher than the
prosperityclub mirror (~45%) — the live anthropic.com uses
dark mode + a sparse modernist layout that the Forge warm-theme
mirror doesn't approximate at all.

## Gaps (all paul-call decisions)

1. **Color scheme** — Live anthropic.com is dark with orange
   accents; Forge mirror is warm/cream. Loom theme PR needed
   for the "anthropic-dark" variant (or use the existing dark
   theme).
2. **Typography** — Live uses Anthropic's custom sans (Styrene
   or similar); Forge uses Inter. Self-hosted variable font
   would close the gap.
3. **Hero composition** — Live opens with a full-bleed marketing
   illustration; Forge opens with a text-only image_hero.
4. **Section density** — Forge mirror has 6 sections; live has
   ~12-15 product/research blocks.
5. **Editorial-vs-marketing decision** — substrate-correct: do
   we WANT to copy anthropic.com's marketing-page shape, or
   keep Forge as an editorial gloss of the same content? paul-
   call.

## Reproducing

```sh
cd PlausiDen-Forge
make pixel-rep              SLUG=anthropic SITE_URL=https://www.anthropic.com/ FORGE_PATH=/anthropic.html
make pixel-rep-diff         SLUG=anthropic
make pixel-rep-visual-diff  SLUG=anthropic
```

Output lives at `PlausiDen-Crawler/runs/anthropic/`,
`PlausiDen-Crawler/runs/anthropic-forge/`,
`PlausiDen-Crawler/runs/anthropic-diff/`.
