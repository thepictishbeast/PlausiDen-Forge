# Pixel reproduction — prosperityclub.com

Forge #218 / #112 working notes. Live state as of 2026-05-20.

## Capture commands

```sh
# Live site baseline (Crawler captures with chromium-shell):
cd PlausiDen-Crawler
./target/release/crawler --capture-reference https://prosperityclub.com/ \
                        --site-slug prosperityclub

# Local Forge mirror (same harness, served via static file server):
cd PlausiDen-Forge/static && ruby -run -ehttpd . -p 8125 &
cd PlausiDen-Crawler
./target/release/crawler --capture-reference http://127.0.0.1:8125/ \
                        --site-slug prosperityclub-forge
```

Outputs land in `PlausiDen-Crawler/runs/<site-slug>/<viewport>.{png,html,styles.json}`
plus a `manifest.json` per slug.

## Manifest-level deltas (live vs Forge mirror)

| Axis | Live prosperityclub.com | Forge mirror | Substrate verdict |
|---|---|---|---|
| HTML size | 180 KB | 25 KB | Forge **7×** lighter (good) |
| 1280.png | 1.25 MB | 1.16 MB | comparable |
| 768.png  | 1.17 MB | 0.96 MB | comparable |
| 390.png  | 913 KB  | 790 KB  | comparable |
| Fonts loaded | 8 (PT Sans / FontAwesome / Source Sans Pro / Montserrat / Lucida / Calibri / Arial) | 3 (Inter / Outfit / monospace fallback) | Forge **fewer** (good) |
| Image count | 34 | 1 | live carries lots of stock photos (bad for them, gap for us) |
| Script count | 31 | 2 | Forge **15× fewer** scripts (massive win) |
| 3rd-party origins | 8 (fonts.googleapis.com / fonts.gstatic.com / i0.wp.com / pagead2.googlesyndication.com / pixel.wp.com / region1.google-analytics.com / stats.wp.com / googletagmanager.com) | 0 | Forge has **zero tracking** (substrate win) |
| Body height @ 390 | 6562 px | 7433 px | Forge slightly taller — more vertical content density |

## Visual deltas (390 px viewport)

Side-by-side at <https://> hosts the live; Forge mirror at dev.plausiden.com.

### Missing on Forge mirror (substrate gaps)

1. **Brand wordmark** — live has a large red "PROSPERITY CLUB" logo at top;
   Forge has only the text nav. Loom-side: needs a header brand-image slot
   (`page_shell` accepts no logo today).
2. **Header utility bar** — live shows phone + email + language picker above
   the main nav. Forge omits this. The `announcement_bar` section provides
   it inline but doesn't pin to the top.
3. **Hero photo not visible in rendered screenshot** — despite
   `cms/index.json` declaring `image_hero.background.{kind: photo, src:
   "/assets/dreamstime/man-at-computer.webp"}`, and the asset existing on
   disk (306 KB webp), and the HTML correctly emitting
   `<img class="loom-image-hero__photo ov-dark" src="...">`, and the CSS
   correctly positioning `.loom-image-hero__photo { position: absolute;
   inset: 0; object-fit: cover; }` over a parent with `position: relative;
   overflow: hidden;` — the chromium-shell screenshot does not show the
   image. Theory: screenshot fires before image decode completes (webp
   async decode); needs explicit wait or different capture mode.
4. **Aggressive brand-color palette** — live is heavy red + orange marketing
   palette; Forge mirror is the `warm` theme (cream + ochre + burnt-orange).
   The warm theme is editorially correct for the substrate but doesn't
   match the live brand. Loom-side: a per-tenant theme variant or a
   `prosperityclub` theme registered on a Loom branch.
5. **Image-rich layout** — live alternates text + photos every 2-3
   sections; Forge mirror is text-only with one hero photo. The substrate
   choice is intentional (editorial discipline > stock-photo SaaS shape)
   so this gap is **deliberate**, not a defect.

### Working correctly on Forge mirror (no gap)

* Editorial section ordering (hero → paragraph → kv_pair → heading
  → paragraph → pull_quote → CTA-band).
* Warm-theme color cascade applied consistently.
* CTA buttons render with the canonical `loom-btn--primary` shape.
* Footer with four columns + legal links + contact block.
* Theme toggle button (top-right) in place.

## Next iteration's actionable list

The substrate-correct gap closures (drop the consumer-shaped ones):

1. **Investigate hero-photo timing in capture-reference.**
   Either bump the screenshot wait or use `--virtual-time-budget` to flush
   decoded resources before the screenshot fires. Owner: PlausiDen-Crawler.
2. **Loom: add `header.brand_image` slot to page_shell.** Per-PR work;
   queue currently 12 deep on PlausiDen-Loom. (#213 vehicle.)
3. **Loom: register a `prosperity` named theme** with the live brand
   palette extracted from `runs/prosperityclub/390.styles.json`. Mirror
   the `warm` / `dark` / `ocean` registration pattern.
4. **Forge: editorial-positioned decision** — does the mirror match live
   exactly (consumer-shape inheritance) or stay editorial (intentional
   improvement)? paul-call.

## Provenance

Capture command, host config, run timestamps, and chromium-shell version
are recorded in `runs/<slug>/manifest.json`. The `390.styles.json`
file carries computed-style fingerprints for cross-run regression
detection — diffing two runs of the same slug surfaces real changes
from the noise of resampled wall-clock state.
