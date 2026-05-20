# Pixel reproduction — github.com

Forge #218 rotation step. Live state as of 2026-05-20.

Captured via `make pixel-rep SLUG=github
SITE_URL=https://github.com/ FORGE_PATH=/github.html`. Cycle
landed in ~12 seconds end-to-end.

## Manifest-level deltas

| Axis | Live github.com | Forge mirror | Substrate verdict |
|---|---|---|---|
| Live PNG (390 / 768 / 1280) | 12525 / 10102 / 11108 px tall | 3899 / 2932 / 2893 | Forge ~3-4× shorter |
| Live HTML | ~150 KB | ~22 KB | Forge **7×** lighter |
| Image count | (live likely 10+) | 1 | Forge sparse by design |
| Script count | (live many) | 2 | Forge **N× fewer** |
| 3rd-party origins | several (CDN + analytics) | 0 | Forge zero tracking |

## Visual pixel-diff

`make pixel-rep-visual-diff SLUG=github` reports:

  390 px  diff = 4.82M px (98.7% of live area)
  768 px  diff = 7.53M px (97.0%)
  1280 px diff = 13.5M px (94.9%)

**~95-99% of pixels differ.** Same magnitude as anthropic.com —
the live github.com landing page is a full marketing
experience (hero illustration + product pillars + customer
logos + testimonial carousel + footer city), the Forge mirror
is an editorial gloss with 3-4× less vertical content.

## Gaps

1. **Page length** — live is ~11k px tall (full marketing
   funnel); Forge is ~3k px (editorial summary). Closing this
   gap means authoring 8-10 more CmsSections.
2. **Hero illustration** — live opens with a full-bleed
   custom-illustrated hero; Forge has the text-only image_hero.
3. **Customer-logo wall** — live has a "trusted by" logo grid;
   Forge has none. Loom has `logo_wall` primitive available;
   paul-call whether to deploy it on the mirror.
4. **Product pillars** — live has 6+ feature-card sections
   with custom illustrations; Forge has kv_pair text-only.
5. **Editorial-vs-marketing decision** — same paul-call as
   anthropic.com (see PIXEL_REP_ROTATION.md).

## Reproducing

```sh
cd PlausiDen-Forge
make pixel-rep              SLUG=github SITE_URL=https://github.com/ FORGE_PATH=/github.html
make pixel-rep-visual-diff  SLUG=github
```

Output at `PlausiDen-Crawler/runs/github/`,
`PlausiDen-Crawler/runs/github-forge/`,
`PlausiDen-Crawler/runs/github-diff/`.
