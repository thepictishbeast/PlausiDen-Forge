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

A static-site generator + audit pipeline for the PlausiDen ecosystem.
Reads typed CMS pages (`cms/*.json`), renders them through
[`loom-cms-render`](https://github.com/thepictishbeast/PlausiDen-Loom),
then runs 25+ build phases that gate the output on accessibility,
performance, security headers, semantic HTML, token consistency, and
runtime browser audit.

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

- **Hugo / Jekyll / Eleventy** — when "render templates" is one of
  twenty things you need a static site to do, a generic SSG is
  the wrong axis. Forge bundles render + audit + deploy in one
  CLI with typed phases.
- **Bespoke build scripts** — every site I've owned had a 500-line
  `make build` that drifted between sites. Forge phases compose
  the same way for every site.
- **Manual accessibility / security review** — every phase is a
  doctrine encoded in code; output fails the build on regressions
  instead of degrading silently between releases.

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
```

`forge.toml` (minimal):
```toml
[build]
mode = "production"

[render]
theme = "loom-default"
```

## Repository layout

```
crates/
  forge-core/      BuildCtx, Phase trait, BuildReport, BuildMode
  forge-phases/    Each phase as its own module
  forge-cli/       `forge` binary
  forge-serve/     Static file server for local preview
  forge-replay/    Re-run a recorded build against a new tree
```

## Doctrine

- **No raw HTML/CSS.** All output flows through the typed CMS
  + Loom renderer.
- **No `unwrap`/`expect` in library code** without a `SAFETY:` proof.
- **Every public function has a test.** Phase coverage is enforced
  by `forge-runner`'s smoke set.
- **The build is reproducible.** Same input + same forge version
  = bit-identical output. Phases that aren't reproducible (e.g.
  network calls) explicitly opt in via `forge-runner` flags.

See [AVP-Doctrine](https://github.com/thepictishbeast/PlausiDen-AVP-Doctrine)
for the full development methodology.
