> # ⚠️ DO NOT USE — UNVERIFIED — UNSAFE ⚠️
>
> This software is **unverified and unsafe for any production use**.
> It is published publicly only for transparency, third-party audit,
> and reproducibility. Treat every commit as guilty until proven
> innocent.
>
> By using this code you accept:
> - **No warranty** of any kind, express or implied.
> - **No fitness** for any particular purpose.
> - **No guarantee** of correctness, safety, or freedom from defects.
> - **Zero liability** on the maintainer for any damages — data loss,
>   security compromise, financial loss, or any consequential damages.
>
> The code is under active engineering development per the
> [Adversarial Validation Protocol v2](https://github.com/thepictishbeast/PlausiDen-AVP-Doctrine/blob/main/AVP2_PROTOCOL.md).
> Every commit's default verdict is **STILL BROKEN**. AVP-2 requires
> a minimum of 36 verification passes before a `SHIP-DECISION:`
> annotation may be considered. **No commit in this repository has
> reached `SHIP-DECISION:` status.**

# PlausiDen-Forge

**A typed-CMS-driven, cryptographically-attested build pipeline
for static, hybrid, or full-SPA output — same source, same audit
gates, three deployment shapes.** Forge takes typed CMS pages
(`cms/*.json`), renders them in-process via
[`loom-cms-render`](https://github.com/thepictishbeast/PlausiDen-Loom)
(serde with `deny_unknown_fields` — schema drift fails closed at
the boundary, not at runtime), then runs **27 build phases** that
gate output on accessibility (WCAG 2.1 AA), performance (Core
Web Vitals), security headers (CSP, SRI, HSTS), semantic HTML,
theme + token consistency, multi-network safety (no clearnet
leakage on Tor-mode sites), backend-frontend drift, and runtime
browser audit via the sibling Crawler.

What's actually shipping today:

- **Three output modes** (`static` / `dynamic` / `hybrid`) — the
  `dynamic_runtime` phase emits `forge-spa-runtime.js` (~2.2 KB,
  CSP-safe vanilla JS, no `eval`, handles modifier keys + middle-
  click + target=_blank + download + rel=external correctly,
  falls back to `location.assign` on any error). Same primitives,
  same audit gates across all three modes.
- **Cryptographic build attestation.** T26 Merkle-chained build
  reports (`prev_hash` + `chain_length` on every JSON report).
  T56 Ed25519 signatures on chain roots via
  `forge attest init` + `forge verify --chain --signatures`.
  Externally-publishable public key for auditor trust-pinning.
  Comparable in posture to SLSA / Sigstore / in-toto.
- **`backends.toml` capability manifest.** Every backend endpoint
  declared once with method, path, purpose, `impl_files` array.
  CI surfaces four drift-type findings: `WIRED` (declared + UI +
  impl), `DECLARED` (no UI yet), `PHANTOM` (UI references
  undeclared backend — fatal in production), `PARTIAL` (declared
  but empty `impl_files` — fatal in production). The
  orphan-button / orphan-endpoint drift class is structurally
  prevented.
- **Live dev loop.** `forge watch` (notify-based, 300 ms debounce,
  re-runs full pipeline on save) + `forge serve` (tokio + hyper
  1.x, replaced the Python `serve.py` per the retire-non-Rust
  directive). Edit a CMS JSON → findings stream + browser sees
  rebuilt page.
- **In-process render with atomic writes.** Forge calls
  `loom_cms_render::render_page()` as a Cargo dep — no
  subprocess, no JSON-roundtrip-through-CLI escape risk.
  Temp-file + POSIX-atomic rename.
- **AVP-2 mutation + secrets gates.** `forge audit mutants`
  reads `mutants.out/outcomes.json` and fails on survivor rate
  above threshold (default 5%). `forge audit secrets` scans
  for private keys / certs / dotenv / password stores; with
  no paths, reads `git diff --cached --name-only` so a
  pre-commit hook can refuse on any match.
- **Type-state build pipeline.** `FORGE_PIPELINE=1` routes phases
  through explicit Discover → Parse → Render → Audit stages with
  typed artifacts. Stage ordering enforced by the compiler.
  Currently opt-in for zero-risk prod migration; eventually
  replaces the flat loop.
- **Dual theme + AMOLED variant.** Loom ships `tokens-light.json`,
  `tokens-dark.json`, optional `tokens-dark-amoled.json` (pure
  `#000000` so OLED pixels turn off). `phase_dual_theme` enforces
  parity (every light token defined in dark); `phase_theme_contrast`
  enforces WCAG 2.1 AA in both themes.
- **Multi-network publishing aware.** `[networks]` declares a site
  reachable on clearnet / Tor / I2P / IPFS / Gemini; the primitive
  system enforces no-clearnet-leakage on Tor-mode sites at the
  primitive layer, not as an afterthought. See
  [`docs/SITE_OPERATIONS.md`](docs/SITE_OPERATIONS.md) for the
  threat-model tiers + security rating dashboard.

Forge is **not a generic static-site generator.** It generates
static output today as one of three deployment shapes; the
static-vs-dynamic distinction is a deployment property, not the
product's identity. The product's identity is the **correctness
substrate** that makes AI-built UI reliable — every audit phase
is a check the agent gets BEFORE the page reaches a human
reviewer. The Crawler runtime-audit feedback closes the loop;
the Annotator captures the human-in-the-loop signal when one is
needed.

Forge is **substrate-flexible, product-opinionated.** The same
substrate serves a mainstream marketing site (Stripe + mainstream
cloud + GA opt-in) and a sovereign Tor-only publication (no
clearnet leakage, hardened defaults, plausible-deniability
integration) — with per-dimension values choices, not a forced
binary. The product layer commits to **AI agents building UI** as
the primary audience.

For the architectural depth see the companion documents in
[`docs/`](docs/):

- [`FORGE_VISION.md`](docs/FORGE_VISION.md) — what Forge does, where it's going, personas, capability matrix, acceptance criteria
- [`ARCHITECTURE_PRINCIPLES.md`](docs/ARCHITECTURE_PRINCIPLES.md) — why the substrate is shaped this way (WordPress-inversion, capability manifest, primitives as constraint system, AI as bounded search)
- [`SITE_OPERATIONS.md`](docs/SITE_OPERATIONS.md) — what every published site needs (required pages, jurisdiction-aware compliance, multi-network publishing, site-success operational layer)
- [`ENGINEERING_DISCIPLINES.md`](docs/ENGINEERING_DISCIPLINES.md) — load-bearing engineering practices (caching with surrogate keys, concurrency, time, jobs, webhooks, crypto, secrets, multi-region, incident handling)
- [`COMMERCIALIZATION.md`](docs/COMMERCIALIZATION.md) — tiered commercial framing + what changes when scope expands from operator-toolkit to platform-we-sell

Read them as one connected design at different abstraction
layers, not as separate artifacts.

> ## ⚠ Status: pre-1.0, AVP-2 in flight — NOT production-ready
>
> This codebase is published publicly for transparency, third-party
> audit, and reproducibility — **not** as a shipped product. Per the
> [Adversarial Validation Protocol v2](https://github.com/thepictishbeast/PlausiDen-AVP-Doctrine/blob/main/AVP2_PROTOCOL.md),
> every commit is treated as guilty until proven innocent via a
> minimum of 36 verification passes. The current verdict is **STILL
> BROKEN** — that's the protocol's default and changes only with an
> explicit `SHIP-DECISION:` annotation listing accepted residual risk.
>
> APIs, file layout, CLI flags, and on-disk formats can and will
> change between commits. Tests pass locally; CI may or may not be
> green at any given moment (see Actions tab). Treat this as a
> live engineering tree, not a release.
>
> Licensed under [FSL-1.1-MIT](./LICENSE) — source-available with
> a 2-year competitor-restriction window, after which it converts
> automatically to MIT.

## What this replaces

Forge competes against different tools depending on the
deployment shape — but **across all three modes** the
differentiator is the typed correctness substrate + cryptographic
attestation, not the rendering itself:

- **Astro / Eleventy / Hugo / Jekyll** (the SSG axis) — Forge
  bundles render + audit + attestation + deploy in one CLI with
  typed phases. Audit and attestation are platform features, not
  things you bolt on with separate tools.
- **Next.js / Remix / Vercel** (the hybrid/dynamic axis) — Forge
  emits SPA runtime + hybrid rendering from the same source as
  the static path, with the same audit gates. No hosting
  commitment (deploy anywhere); no per-request server cost for
  static-shaped traffic; same CSP / a11y / token gates regardless
  of mode.
- **Webflow / Framer** (the no-code-CMS axis) — Forge has a typed
  CMS schema, an AI generation path that respects the schema,
  primitive contracts that make broken layouts unreachable, and a
  Tor-mode story. Webflow has none of these. Forge is the
  serious-operator's CMS where Webflow is the casual one.
- **Bespoke build scripts** — every site I've owned had a 500-line
  `make build` that drifted between sites. Forge phases compose
  the same way for every site.
- **Manual accessibility / security review** — every phase is
  doctrine encoded in code; output fails the build on regressions
  instead of degrading silently between releases. The build's
  attestation chain means a customer can hand auditors a
  cryptographic record of every build that produced production
  output.

## Build modes

| Mode | Behavior |
|------|----------|
| `static` (default) | Pre-rendered HTML only. Classic SSG. |
| `dynamic` | Static HTML + SPA runtime (`forge-spa-runtime.js`) injected into every page; same-origin navigation becomes fetch + DOM swap. Falls back to full page load on error. |
| `hybrid` | Same as `dynamic` — pre-rendered AND with the SPA runtime. |
| `poc` | Warns are advisory; useful while iterating. |
| `production` | Warns escalate to strict; nothing ships with known soft regressions. |

Set via `forge build --mode <name>` or `[build] mode = "..."` in
`forge.toml`.

## Phases (current set)

```
validate_cms → loom_sync → render → self_check
  → theme_consistency → theme_contrast → path_consistency
  → tokens → html_semantic → csp → csp_devmode
  → external_assets → a11y_landmarks → id_strategy → seo
  → perf_budget → asset_optimization → sri → phantom_button
  → backend_coverage → unbuilt_route → label_consistency
  → link_check → motion → contrast → dynamic_runtime
  → crawl
```

Each phase implements `forge_core::Phase`. Findings flow up to a
`BuildReport`; exit code = 0 iff every strict finding's severity
clears `Severity::blocks_in(mode)`.

## Quickstart

```sh
cargo install --path crates/forge-cli   # or use the binary
cd <your-site>
forge build                              # default: --mode poc
forge build --mode production            # gate the release

# Local preview (forge-serve)
forge serve                              # http://localhost:8080
forge serve --port 4000 --watch          # rebuild on cms/ change

# Visual regression (delegates to PlausiDen-Crawler)
npm run audit:rust:smoke                 # desktop journey
npm run audit:rust:mobile                # mobile viewport
npm run audit:rust:tablet                # tablet viewport
                                         # × light + dark theme variants
                                         # = 8 combos per page (see test matrix below)
```

`forge.toml` (minimal):
```toml
[build]
mode = "production"

[render]
theme = "loom-default"
```

## Pre-commit secrets gate

`forge audit secrets` refuses to ship filenames that match
dangerous secret-shaped patterns (private keys, certificates,
dotenv, password stores). Install the pre-commit hook in any
git repo with one command:

```sh
forge audit init-hook
# → wrote .githooks/pre-commit (mode 0755)
git config core.hooksPath .githooks   # one-time, per clone
```

The hook runs `forge audit secrets --explain` against the
staged-paths set on every commit. Non-zero exit refuses the
commit. The hook fails-open with a warning if `forge` isn't on
`$PATH` (so cloned-but-not-built repos don't break commits) —
re-install via `forge audit init-hook --force` once you've
rebuilt the binary.

Idempotent: re-running `forge audit init-hook` against an
identical existing hook is a no-op. If the existing hook
differs (operator customized it), the command refuses to
overwrite without `--force`.

Verify the gate works:

```sh
touch test-private-key.pem
git add test-private-key.pem
git commit -m 'should be rejected'
# → 'forge audit secrets: 1 secret-shaped path(s) found — refuse to commit'
git restore --staged test-private-key.pem
rm test-private-key.pem
```

## Test matrix

Every rendered page is validated across **24 combinations**:

```
3 themes (light, dark, dark-amoled)
×
2 viewports (desktop, mobile)         [tablet is opt-in]
×
2 modes (static, dynamic)
×
2 debug modes (production, debug)
= 24 runs per page
```

Driven by [PlausiDen-Crawler](https://github.com/thepictishbeast/PlausiDen-Crawler)
journeys (`journeys/plausiden-smoke{,-mobile,-tablet}{,-dark}.json`).
The Rust crawler (`audit:rust:matrix` npm script) is canonical;
the legacy TS path is being deprecated.

Each combo captures per-step PNGs, full HAR (debug mode), video
of animations + transitions (debug mode), and a typed `Report` of
findings. Reviewing the recordings + screenshots is the
perfection-iteration loop — fix the underlying Loom primitive or
Forge phase, re-run, until pixel-and-behavior perfect.

Themes: `light` / `dark` / `dark-amoled` are SITE-side token sets
(Loom ships `tokens-light.json`, `tokens-dark.json`, optional
`tokens-dark-amoled.json`). Browsers report `prefers-color-scheme:
dark` for both dark variants; the Crawler passes a `?_theme=dark-amoled`
URL param for the AMOLED-specific variant. AMOLED dark uses pure
`#000000` background so OLED pixels turn off — saves battery + max
contrast.

Developers can scope the matrix down via `forge.toml`
`[test_matrix]`, but production-ready sites should target all 24.
See [`docs/TESTING.md`](docs/TESTING.md) for full axis definitions,
opt-out semantics, acceptance gates, and ISO/IEC 25010 mapping.

## Repository layout

```
crates/
  forge-core/      BuildCtx, Phase trait, BuildReport, BuildMode
  forge-phases/    Each phase as its own module
  forge-cli/       `forge` binary
  forge-serve/     Static file server for local preview
  forge-replay/    Re-run a recorded build against a new tree
```

## Ecosystem integration

Forge is one of six PlausiDen tools. The ecosystem is designed for
seamless coupling — each tool's output is a typed input to the next.

```
CMS (cms/*.json)                  ←  content authoring
   │
   ▼
Loom (typed primitives + tokens)  ←  design system, theme tokens,
   │                                 typed component variants
   ▼
Forge (build pipeline)            ←  THIS REPO: renders, audits,
   │                                 emits attested static/dynamic
   ▼                                 bundle
Crawler (Playwright runtime)      ←  drives the rendered output,
   │                                 captures per-step PNGs +
   │                                 typed Report (findings flow
   │                                 back as Forge phase results)
   ▼
Annotator (operator UX capture)   ←  human-in-the-loop session
                                     JSON; consumed by agents +
                                     by future Forge phases as
                                     review-flagged findings
Oxidizer                          ←  (deferred) final ship gate
```

Forge phases consume Crawler reports (`forge-phases::crawl`) and
Annotator sessions (queued: `forge-phases::review_capture`). The
shared schema is `forge-core::Finding` — every tool emits the
same typed shape, every consumer reads it identically.

## Component variants

See [`docs/DEDUP_TABLE.md`](docs/DEDUP_TABLE.md) for the canonical
list of typed `CmsSection` variants — what ships, what's queued, what
single-surface variants are deferred until a second site needs them.
The table is the deliverable; per-site mimic folders are not.

## Theme + accessibility defaults

Every site Forge generates ships **light + dark themes**, **WCAG 2.1
AA accessibility**, and **semantic HTML** by default. Override only
if explicitly asked. Enforced by these phases:

- `phase_theme_consistency` — dual-theme parity (every light token
  is defined in dark).
- `phase_a11y_landmarks` — `<header>`, `<nav>`, `<main>`, `<aside>`,
  `<footer>` present and unique. P0.
- `phase_contrast` — WCAG 2.1 AA contrast in both themes.
- `phase_semantic_html` — no `<div role="banner">`; use semantic
  elements.

Per the design doctrine (see [PlausiDen-Loom/CLAUDE.md](https://github.com/thepictishbeast/PlausiDen-Loom/blob/main/CLAUDE.md)):
no raw class strings outside `loom-components`, no `<div>`-stacks
without `<section>`, every interactive element has `:focus-visible`.

## Standards

PlausiDen software defaults to ISO/IEC standards where one applies:

- **ISO 8601** for all date/time strings (`YYYY-MM-DDTHH:MM:SSZ`)
- **ISO 639-1** for language codes (`<html lang="en">`)
- **ISO 3166-1 alpha-2** for country codes
- **ISO/IEC 25010** software quality model — commit messages note
  which of the eight quality attributes the change advances
- **ISO/IEC 40500:2012 / WCAG 2.1 AA** — accessibility floor
- **ISO/IEC 27001:2022** — infosec management; AVP-2 passes map to
  Annex A controls in audit docs

## Doctrine

- **No raw HTML/CSS.** All output flows through the typed CMS
  + Loom renderer.
- **No `unwrap`/`expect` in library code** without a `SAFETY:` proof.
- **Every public function has a test.** Phase coverage is enforced
  by `forge-runner`'s smoke set.
- **The build is reproducible.** Same input + same forge version
  = bit-identical output. Phases that aren't reproducible (e.g.
  network calls) explicitly opt in via `forge-runner` flags.
- **General-purpose, never site-specific.** No committed clones of
  named third-party sites. The 9 rebuilds that produced
  `docs/DEDUP_TABLE.md` were single-pass inputs and have been deleted.
  Future variant proposals come from customer-site work or owner
  directive, not speculative clones.

See [AVP-Doctrine](https://github.com/thepictishbeast/PlausiDen-AVP-Doctrine)
for the full development methodology.
