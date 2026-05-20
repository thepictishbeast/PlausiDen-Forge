# Pixel-Reproduction Rotation (#124)

**Status:** rotation tracker for the loop's PRIORITY 3 — pixel-by-
pixel real-site reproductions via Forge.

Closes task #230 / preamble #124.

This document is the **state register** for the rotation. When the
operator runs a pixel-reproduction pass, they:

1. Read the "Next up" row.
2. Run the per-site recipe (Crawler screenshot → Forge CMS build
   → deploy to dev.plausiden.com → Firefox-headless screenshot
   → visual diff → add missing Loom primitives generic-only →
   iterate to match).
3. Update this doc with the result + advance the rotation pointer.

The rotation order, per the loop preamble, alternates between:

* **In-ecosystem sites** — sites paul builds for production
  (prosperityclub.com, plausiden.com, sacred.vote-static)
* **Reference sites** — best-of-class external sites used as
  corpus for primitive-explosion targets (Stripe, Linear,
  Vercel, GitHub, Notion, Anthropic, Render, Fly)

Alternating keeps the substrate work grounded in real production
needs AND broad reference-corpus inputs (per memory
[[forge-substrate-flexible-product-opinionated]] +
[[reference_corpus_and_density_tiers]]).

---

## Rotation register

| Slot | Site | Task ID | Status | Last pass | Notes |
|---|---|---|---|---|---|
| 1 | prosperityclub.com | #218 | not-started | — | In-ecosystem. Existing CMS at `cms/prosperityclub.json` (per memory [[session-2026-05-20-checkpoint]] — dev.plausiden.com already serves a Forge build of the homepage) |
| 2 | Stripe (stripe.com) | #221 | not-started | — | Reference. Premium pricing-page + hero composition. Used as primitive-density baseline. |
| 3 | plausiden.com | #219 | not-started | — | In-ecosystem. Forge-static must match the prod Rust app's homepage output. |
| 4 | Linear (linear.app) | #222 | not-started | — | Reference. Editorial-tech composition; gradient text + monospace accents. |
| 5 | sacred.vote / sacredvote.org | #220 | not-started | — | In-ecosystem (Forge-static approximation; never touch sacred.vote source per scope constraint) |
| 6 | Vercel (vercel.com) | #222 | not-started | — | Reference. Black-on-black aesthetic; type-driven; no decorative chrome. |
| 7 | (cycle back to slot 1) | — | — | — | — |

After slot 7 wraps back to slot 1, the rotation continues. Per
loop preamble, additional reference sites (GitHub, Notion,
Anthropic, Render, Fly) can be inserted as alternate slots if
the operator wants more reference-corpus density before re-
hitting an in-ecosystem site.

## Per-site recipe

For each site, the operator runs (per the loop preamble PRIORITY 3):

```bash
# 1. Live-site screenshot via Crawler runner at 3 viewports
cd /home/paul/projects/PlausiDen-Crawler
sudo -u paul ./target/release/crawler-runner journey-screenshot \
  --url https://<target> \
  --viewport 390 \
  --output /tmp/reference-390.png
# Repeat at 768 and 1280.

# 2. Build / update the Forge CMS targeting the same composition
cd /home/paul/projects/PlausiDen-Forge
# Edit cms/<site-slug>.json — compose CmsSection variants that
# match the visual structure. Per substrate doctrine ([[substrate-only-path]]):
# if a primitive doesn't exist, ADD it to loom-components / loom-cms-render —
# never hand-author HTML/CSS/JS.
sudo -u paul ./target/release/forge build

# 3. Deploy to dev.plausiden.com
sudo rsync -a --delete /home/paul/projects/PlausiDen-Forge/static/ \
  /var/www/dev.plausiden.com/
sudo chown -R caddy:caddy /var/www/dev.plausiden.com/

# 4. Forge-output screenshot at the same viewports
firefox-esr --headless --window-size=390,800 \
  --screenshot=/tmp/forge-390.png https://dev.plausiden.com/
# Repeat at 768x1024 and 1280x800.

# 5. Visual diff
# Use any image-diff tool. ImageMagick:
compare -metric AE -fuzz 5% /tmp/reference-390.png /tmp/forge-390.png /tmp/diff-390.png
# AE = absolute-error pixel count. Lower = closer match.

# 6. Iterate: identify missing primitives/themes/colors/variants.
# ADD them generic-only to loom-components / loom-tokens / loom-cms-render.
# NEVER add site-specific shape ([[crawler-stays-general-purpose]] +
# [[forge-default-themes-a11y]]).
```

## Status conventions for the table

* **not-started** — slot in rotation but no pass run yet
* **in-progress** — operator currently iterating; partial diffs landed
* **passing** — visual diff at all three viewports is within tolerance
  (Stripe-tier sites: ~5% AE; in-ecosystem sites: ~2% AE)
* **blocked-on-primitive** — pass paused because a missing Loom
  primitive is needed; track which one in the Notes column
* **complete-for-this-rotation** — slot passed; next rotation
  through will re-validate against fresh live-site screenshots
  (the live target may have changed)

When a slot moves to `passing` or `complete-for-this-rotation`,
update `Last pass` with the ISO-8601 UTC timestamp.

## Companion deliverables

Each completed pass should produce:

1. **Updated `cms/<site-slug>.json`** in PlausiDen-Forge.
2. **Any new Loom primitives** committed as separate, named
   commits in PlausiDen-Loom (per [[crawler-stays-general-
   purpose]] + [[substrate-only-path]] — primitives are
   generic, the site CMS instantiates them with content).
3. **Pixel-diff report** committed under
   `PlausiDen-Forge/reports/pixel-diff/<site>/<ISO timestamp>/`
   with the 3-viewport reference + forge + diff images.
4. **Detector finding deltas** — if the pass exposes a new
   Crawler axis gap, file under #117 (rolling). My
   `meta_refresh` detector (commit c5df9e6) is an example —
   surfaced during the noscript-audit dimension of this work.

## Why this tracker exists

Without a rotation register, every pixel-reproduction pass risks
re-hitting the same site (most-recently-visible in tmp) or
forgetting which references have been covered. The register
makes the rotation explicit + auditable + recoverable across
sessions.

Per memory [[pixel-reproduction-needs-live-infra]] — each pass
is a multi-hour live-infra job; the routine /loop substrate
work feeds primitives INTO this dimension but doesn't replace
the live pass itself.

## Future work

* **Automate the diff loop** — a `forge audit pixel-diff
  <site-slug>` subcommand that runs Crawler + Forge + Firefox
  headless + ImageMagick compare in one invocation, dropping
  the report under `reports/pixel-diff/`. Currently each step
  is manual.
* **Pin reference-site snapshots** — capture the live target's
  HTML+CSS+image bytes at the moment of a passing pass, so
  subsequent rotations can verify "we still match the snapshot
  we matched last time" without re-hitting the live target
  (and without depending on whether the live target has
  changed). Caveat: image rights / licensing for cached
  reference snapshots.
* **Per-viewport tolerance config** — mobile (390) tolerates
  more variation than desktop (1280) because mobile text-wrap
  is more font-metric-dependent; tunable per-site overrides
  needed for sites with heavy text density.
