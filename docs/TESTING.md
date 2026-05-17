# Test matrix — 24 combinations per page

Every page rendered by Forge is validated across **24 combinations**:

```
3 themes (light, dark, dark-amoled)
×
2 viewports (desktop 1280×900, mobile 375×667 — tablet 810×1080 is a
              third option but is not part of the strict 24-combo
              floor; opt-in via `audit:rust:tablet*` scripts)
×
2 modes (static, dynamic)
×
2 debug modes (production, debug)
= 24 runs per page
```

Per the design doctrine: dual themes + WCAG 2.1 AA + semantic HTML
are the **default** every site ships ([`feedback_forge_default_themes_a11y`](../README.md#theme--accessibility-defaults)).
The matrix is the regression net that keeps them honest.

## Axis definitions

### Theme axis (3 values)

| Theme | `prefers-color-scheme` | Background token | OLED behavior |
|---|---|---|---|
| `light` | `light` | `#FAFAFD` (off-white) | Pixels lit, normal power |
| `dark` | `dark` | `#0d1117`-ish (muted) | Pixels lit, dim; reduces eyestrain for long reads |
| `dark-amoled` | `dark` | `#000000` (true black) | OLED pixels off; max battery + contrast |

The `dark` vs `dark-amoled` distinction is a SITE-side concern —
Loom ships separate `tokens-dark.json` and (optionally)
`tokens-dark-amoled.json` token sets. The Crawler journey signals
which token set to load via a `?_theme=<value>` URL query param
(`?_theme=dark` or `?_theme=dark-amoled`); sites that don't honor
the param ignore it harmlessly. Browsers report identical
`prefers-color-scheme: dark` for both — the difference is *which
Loom dark variant the site picks*.

### Viewport axis (2 values, +1 opt-in)

| Viewport | Width × Height | Audience |
|---|---|---|
| `desktop` | 1280×900 | Primary; default `audit:rust:smoke*` |
| `mobile` | 375×667 (iPhone SE class) | Tap-target + touch-overflow regressions |
| `tablet` *(opt-in)* | 810×1080 (iPad portrait) | Mid-density grid + keyboard-detach regressions |

Tablet is not in the strict 24-floor because most regressions
surface on desktop XOR mobile; tablet is "would mobile-on-bigger-
viewport break this?" Use `audit:rust:tablet*` when explicitly
testing tablet-specific behavior.

### Mode axis (2 values)

| Mode | Forge invocation | Behavior |
|---|---|---|
| `static` | `forge build --mode static` | Pre-rendered HTML only. Hosted by any static-file server. |
| `dynamic` | `forge build --mode dynamic` | Static HTML + `forge-spa-runtime.js`. Same-origin nav becomes fetch+DOM swap; falls back to full page load on error. |

Both modes must produce **the same visible output** for the same
input. Diff between modes = SPA-runtime bug. The matrix catches this.

### Debug axis (2 values)

| Debug | `Journey.debug` | Browser | Logs | Network |
|---|---|---|---|---|
| `false` (default) | unset / false | Headless | `tracing::warn` + up | Console capture only |
| `true` | `debug: true` | Visible (`--no-headless`), DevTools open | `tracing::debug` everywhere | Full HAR capture |

Debug mode produces materially larger run artifacts (HAR, video,
verbose console). Run it before shipping a release or when hunting
a specific regression, not on every commit.

## Opt-out semantics

Per owner directive 2026-05-17: developers/clients can scope the
matrix down to what their site actually targets. The full 24-combo
sweep is the **recommendation** — every site should aim for it
eventually — but explicit opt-out is supported.

Place a `[test_matrix]` block in your site's `forge.toml`:

```toml
[test_matrix]
# Default if absent: all 24 combinations run.
themes    = ["light", "dark"]        # opt out of dark-amoled testing
viewports = ["desktop"]              # opt out of mobile (NOT recommended)
modes     = ["static", "dynamic"]
debug     = [false]                  # skip debug-mode in CI
```

When the matrix is trimmed, the missing combos are recorded in the
`BuildReport` as `Skipped { axis, value, reason }` findings so the
opt-out is auditable. CI fails if a site claims production-readiness
but opts out of more than one axis.

**Recommended posture**: build for all 24. Most sites that opt out
later regret it the first time a real user hits a configuration the
opt-out skipped.

## Running the matrix

```sh
# Single combo
npm run audit:rust:smoke            # desktop light
npm run audit:rust:smoke-dark       # desktop dark
npm run audit:rust:mobile           # mobile light
npm run audit:rust:mobile-dark      # mobile dark
npm run audit:rust:tablet           # tablet light (opt-in viewport)
npm run audit:rust:tablet-dark      # tablet dark (opt-in viewport)

# 6-journey sweep (theme × viewport, light+dark variants of all 3 viewports)
npm run audit:rust:matrix

# Full 24-combo (manual today — automated runner queued):
#   1. forge build --mode static
#   2. forge build --mode dynamic
#   3. For each: npm run audit:rust:matrix
#   4. For each: npm run audit:rust:matrix -- --debug
#   5. For each dark variant: re-run with ?_theme=dark-amoled URL override
#
# Total = 2 modes × 6 journeys × 2 debug × (1 + AMOLED dark variants) = 24.
```

## Screen recording + UX iteration loop

Per owner directive 2026-05-17 (perfection-loop): every matrix run
in debug mode records:

- **Video** (`video.mp4`) of the entire session — captures animations,
  transitions, scroll behavior.
- **Per-step PNG screenshots** at every labelled checkpoint.
- **Full console capture** (debug-verbosity) including agent-driven
  console.debug from the SPA runtime.
- **HAR network capture** of every request.

After each matrix run, **review the videos + screenshots**, identify
UX/UI gaps (animation jank, layout shift, contrast failures, focus
loss, etc.), fix the underlying Loom primitive or Forge phase, and
re-run. Iterate until pixel-and-behavior perfect across all 24 combos.

Don't commit the per-test recordings — they're transient diagnostic
artifacts. Findings that surface a missing Forge primitive go into
[`DEDUP_TABLE.md`](DEDUP_TABLE.md).

## Acceptance gate

A page is *production-ready* in Forge when:

- All 24 combos return zero `Severity::Error` findings.
- The visual diff between `static` and `dynamic` modes is < 0.1%
  per the `crawl --visual-diff` phase.
- Light-vs-dark contrast (per theme) is ≥ WCAG 2.1 AA in both
  schemes.
- No flash-of-unstyled-content / theme-flash between initial paint
  and SPA hydration.
- Animations honor `prefers-reduced-motion`.

Sites that don't yet meet the gate continue to build in `--mode poc`
where findings are advisory. `--mode production` enforces the gate
strictly.

## ISO/IEC mapping

This matrix advances these ISO/IEC 25010 software-quality
attributes:

- **Functional suitability** — coverage across the full
  configuration space.
- **Performance** — matrix surfaces per-combo render time;
  perf-budget phase fails on regressions.
- **Compatibility** — theme/viewport/mode interop tested
  exhaustively.
- **Usability** — WCAG 2.1 AA (ISO/IEC 40500) enforced in every
  theme.
- **Reliability** — same input + same Forge version = same output
  across all 24 combos.
