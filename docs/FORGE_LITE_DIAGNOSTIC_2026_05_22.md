# Forge Lite diagnostic verdict — 2026-05-22

Result of running the 5 Forge Lite fixtures
(`fixtures/forge-lite/*.json`) through `forge lite resolve` →
`forge build` with `[strict] aesthetic_distinctiveness = true`
and `[strict] content_substance = true` enabled per tenant.

This is the diagnostic the architecture audit
(2026-05-21) called for: test what's already shipped against
the diagnostic surface before adding more primitives.

## Verdict (one line)

**Mixed.** All 5 fixtures FAILED strict, but mostly for reasons
that are EITHER (a) lite-surface leaks (missing slots /
auto-declaration) or (b) variation-arc gates calibrated for
SaaS shapes mis-firing on intentionally narrow content. NOT
primarily for convergence between lite outputs.

## Per-fixture results

| Fixture | strict findings | Root cause |
|---|---|---|
| alpha-saas       | 3 | loom_bin (test env), phantom_button (lite-cta), monotonous_feature_grid |
| beta-editorial   | 2 | loom_bin (test env), path_consistency (`/notes/bounded-interfaces`) |
| gamma-portfolio  | 3 | loom_bin (test env), path_consistency (`/work`), sparse_page (3 sections) |
| delta-brief      | 4 | loom_bin (test env), path_consistency (`/brief`), sparse_page, image_desert |
| epsilon-cta-heavy| 4 | loom_bin (test env), path_consistency (`/sign-up`), phantom_button, monotonous_feature_grid |

## Classification of findings

### Category 1 — Test-environment noise (5/16 findings)

`validate_cms: loom binary not found` fired on every fixture
because the LOOM_BIN env var wasn't set when running the
diagnostic. Not a real substrate issue; just diagnostic
harness setup. Not informative.

### Category 2 — Lite-surface leaks (4/16 findings)

The lite contract is supposed to be a closed narrow surface.
These findings prove the contract has open edges that leak
substrate details upward:

1. **`phantom_button: data-backend="lite-cta" not declared`**
   — The lite resolver hardcodes `"lite-cta"` as the CTA's
   data-backend slug. No `backends.toml` is generated.
   Phantom_button strict-fails. **The operator has no path
   to fix this from the lite contract** — the lite surface
   doesn't expose data-backend authoring. Fix: either the
   `forge lite resolve` subcommand auto-writes a minimal
   `backends.toml` covering `lite-*` slugs, OR the phantom_
   button phase special-cases `lite-*` prefix and treats it
   as substrate-internal.

2. **`path_consistency: path=/x must end in / or .html`** —
   4 of 5 fixtures use paths like `/work` / `/brief` /
   `/sign-up` without trailing slash. Path-consistency
   upstream requires `/` / `.html` / exactly `/`. The lite
   fixtures themselves are bugged; the lite validator
   should auto-normalize at the resolver boundary OR catch
   the issue at validation time with a clear remediation.

### Category 3 — Real lite-surface gaps (1/16 findings)

3. **`monotonous_feature_grid: 1 unique icon across 3 items`**
   — `FeatureSpotlight` items in 2 fixtures (alpha, epsilon)
   have NO icons because the lite `FeatureItem` struct never
   exposed an `icon_slug` field. Even if operators wanted to
   vary iconography, they COULDN'T through the lite contract.
   **FIXED this iteration**: added `icon_slug: Option<String>`
   to `forge_core::forge_lite::FeatureItem`; resolver maps it
   to `loom_cms_render::SpotlightItem.icon_slug`.

### Category 4 — SaaS-shape calibration of variation-arc gates (3/16 findings)

4. **`sparse_page: only 3-4 sections; marketing landings need ≥5`**
   — fires on gamma-portfolio (3 sections) and delta-brief
   (4 sections). The gate treats every page as a marketing
   landing. Brief / portfolio / editorial shapes are
   legitimately sparser. **Real substrate gap, not a
   diagnostic failure**: the gate needs context-awareness
   (page-type signal from `[site_identity]`) or a per-tenant
   exemption surface.

5. **`image_desert: page has N sections and zero images`** —
   fires on delta-brief (which is intentionally text-only).
   Same overbroad assumption: the gate treats text-only as a
   defect regardless of intent. **Real substrate gap**.

## What the diagnostic proved

- **`strict_promotions` works end-to-end across multiple
  tenant identities.** Wired correctly; promoted 1–2
  findings per fixture in line with the [strict] config.
  The audit hypothesis ("enforcement is in place but
  ignored at Warn severity") is validated against >1
  tenant.

- **Forge Lite's CLI subcommand works end-to-end.** All 5
  fixtures resolve cleanly via
  `forge lite resolve <input.json> --output <cms.json>`.
  Cost: ~milliseconds per fixture.

- **The narrow surface is not narrow enough yet.** Three
  surface-level leaks (CTA backend auto-declaration, path
  normalization, missing icon_slot slot) and two
  calibration-level gaps (sparse_page and image_desert
  fire too broadly) prevent the lite outputs from passing
  strict on their own merit. The diagnostic surface
  doesn't yet test what it's intended to test.

## What was shipped this iteration

- `forge_core::forge_lite::FeatureItem.icon_slug` field
  added; resolver wires through; existing tests adjusted.
- This memo.

## What is NOT shipped (next iteration)

- **Lite CTA backend auto-declaration** — biggest unblock
  for fixture builds. `forge lite resolve` should produce
  a minimal `backends.toml` alongside `cms/index.json`
  containing every `lite-*` data-backend the resolved
  CmsPage references.

- **Path normalization** — `forge_core::forge_lite`
  validation OR the resolver should accept `/work` and
  normalize to `/work/` at the cms boundary.

- **Context-aware `sparse_page` + `image_desert`** —
  the variation-arc phases need a page-type signal from
  `[site_identity].kind` (marketing-landing / brief /
  portfolio / editorial) and adjust thresholds per kind.
  OR a `[strict.exempt]` table that suppresses specific
  findings per tenant with rationale required.

- **Lite fixtures' own path bugs** — beta/gamma/delta/
  epsilon paths should be fixed in
  `fixtures/forge-lite/*.json`. Independent of substrate
  changes.

## Diagnostic re-run plan

Once the four pending fixes land, re-run this exact
diagnostic. Expected outcome:

- Category 1 (loom_bin) — gone (set LOOM_BIN in run command).
- Category 2 (phantom_button + path_consistency) — gone
  (lite handles both internally).
- Category 3 (monotonous_feature_grid) — gone OR remains
  with operator-actionable remediation (operators can now
  set `icon_slug`).
- Category 4 (sparse_page + image_desert) — context-aware
  thresholds let portfolio + brief shapes pass cleanly.

If categories 1–3 disappear and category 4 still fires
broadly: the gates are mis-calibrated, fix is calibration.
If categories 1–3 disappear and category 4 fires
selectively: the substrate is correctly enforcing
shape-appropriate quality.

## Audit follow-through

This memo closes #396 (Forge Lite diagnostic verdict). It
opens four follow-up tasks (#401–#404 captured in
TaskList) for the unwired edges + the calibration gaps.

The advisor's central caution from this iteration applies:
**don't ship more primitives until the diagnostic surface
actually tests what it's intended to test.** The four
follow-ups are wiring work, not new infrastructure.
