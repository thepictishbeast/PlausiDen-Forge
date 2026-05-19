# Tor / I2P / Lokinet Site Templates

**Status:** doctrine + recipe. Closes #119. Describes the
configuration shape sites use when they declare any of `tor`,
`i2p`, or `lokinet` as a deploy target.

Forge's existing phases (`network_target_enforcement`,
`reader_safety`) already enforce the substrate-level invariants;
this document is the **author-facing recipe** so a new Tor /
I2P / Lokinet site can be scaffolded by hand (until
`loom site init --target tor` lands) without missing a phase
configuration.

## What "on Tor" means for the substrate

A site on Tor must work for Tor Browser's "Safest" security
level, which disables JS, custom fonts, and most non-baseline
features. A site that requires those to be navigable has
selected against its own audience.

Forge's phases that enforce this:

* **`network_target_enforcement`** — refuses any clearnet host
  reference (script src, link href, img src, fetch URL, etc.)
  in rendered HTML when `[networks].targets` includes Tor / I2P
  / Lokinet.
* **`reader_safety`** — verifies the rendered pages work
  without JS, custom fonts, cookies, or localStorage.
* **`noscript_strict`** (when also enabled) — refuses any
  `<script>` tag in rendered HTML.
* **`hunted_tier`** (when also enabled) — top-tier meta-policy
  that requires noscript_strict + zero-tracker + zero-client-
  state markers.

## forge.toml recipe

The minimum config for a Tor-only site:

```toml
[networks]
targets = ["tor"]
# Or for multi-network publish:
# targets = ["tor", "i2p", "lokinet"]
# Or clearnet + Tor mirror:
# targets = ["clearnet", "tor"]

[reader_safety]
# Strict mode — every reader-safety check that fires becomes
# build-blocking. Default is warn-only; flip to strict once
# the site is genuinely Tor-clean.
strict = true

# Optional: exclude specific files that legitimately need JS
# (e.g. an operator-only admin console accessed at a
# different .onion or behind authentication).
# skip_files = ["admin/console.html"]

[noscript_strict]
# Pair with LOOM_NOSCRIPT_MODE=1 in the build env so Loom's
# page-shell drops its bootstraps (theme-toggle / defer-onload
# / eruda-loader). The audit phase then validates zero <script>
# tags in rendered HTML.
enabled = true

# For maximally paranoid Tor-tier sites:
[security]
tier = "hunted"
```

## Build invocation

```bash
LOOM_NOSCRIPT_MODE=1 forge build
```

The env flag tells Loom's `page_shell_themed` to skip every
inline `<script>`, drop the defer-stylesheet onload swap, and
emit the strictest CSP variant (`script-src 'none'`,
`require-trusted-types-for 'script'`, `trusted-types 'none'`).

After build, the `noscript_strict` Forge phase confirms zero
`<script>` tags in `static/*.html`, and `hunted_tier` confirms
no client-state-API references slipped through body copy.

## cms/index.json recipe

The Tor-friendly defaults flow through normal `CmsPage` JSON.
A few field choices matter:

```json
{
  "$schema": "../cms-schema.json",
  "title": "Site title",
  "description": "...",
  "brand": "Site",
  "chrome": "minimal",
  "theme": "press",
  "content_width": "comfortable",
  "path": "/",
  "nav_links": [...],
  "sections": [...]
}
```

Notes:

* `chrome: "minimal"` — drops the sticky-header bar that uses
  `backdrop-filter`. Tor Browser strict mode may not support
  `backdrop-filter` reliably.
* `theme: "press"` — monochrome editorial palette. Works on
  Tor without any color-mode JS.
* `content_width: "narrow"` (42rem) or `"comfortable"` (64rem)
  — Tor readers tend to want editorial measure, not wide-grid
  app shells.

Avoid:

* `chrome: "floating-pill"` — uses `backdrop-filter: saturate()
  blur()` which Tor Browser Safest disables. Falls back to
  opaque OK but the design intent assumes the blur.
* `dev_devtools: true` — Eruda is a CDN-loaded debug surface;
  contradicts the no-CDN doctrine of an .onion service.
* CmsSection variants that load external resources
  (e.g. third-party video embeds, external font references in
  the theme — all already caught by
  `network_target_enforcement`).

## Multi-network publish

When `[networks].targets` includes both `clearnet` and a non-
clearnet target, Forge builds ONE static output that's valid on
both. The rules:

* No clearnet absolute URLs in rendered HTML. All hrefs are
  same-origin / relative. The same `static/` bundle can be
  served at `https://example.com/` AND `http://example.onion/`.
* The Tor mirror's CSP applies to both — there is no separate
  "loose for clearnet, strict for Tor" build path.
* Forge emits a `static/.well-known/onion-location` declaration
  if the operator includes their .onion address in
  `forge.toml [networks] tor_address = "abc...xyz.onion"`.
  Browsers that support it (Tor Browser, Brave) auto-redirect
  to the onion service when reaching the clearnet origin.

## Operator checklist before first Tor publish

1. `forge build` runs clean with strict findings == 0.
2. Open the rendered `static/index.html` in Tor Browser at
   security level "Safest". Verify every page renders + every
   link works without enabling JS.
3. Run `crawler-runner` with the standard journey at `target =
   tor` and confirm zero strict findings on
   `network_target_enforcement`, `reader_safety`,
   `noscript_strict`, and `hunted_tier` (if configured).
4. Verify the `.onion` service serves the same bytes the
   clearnet origin does (when both are published).

## Out of scope for this template

* The .onion hidden-service setup itself (Tor daemon config,
  hidden_service_dir + private key management) — that's
  ops-side, not build-side.
* I2P eepsite registration + i2pd config.
* Lokinet SNApp registration.
* The `loom site init --target tor` CLI scaffold — queued as
  the next concrete step on this work; this doc is the
  recipe the scaffold will encode.
