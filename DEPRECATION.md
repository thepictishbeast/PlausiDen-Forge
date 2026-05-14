# DEPRECATED FILES — historical record

**Owner directive 2026-05-06:** Forge IS a Rust application. Bash and
Python remnants are gone as of 2026-05-14 (T54).

## Live (write here)

| Path | Role |
|------|------|
| `crates/forge-core/`   | `Phase` trait, `Finding`, `Severity`, `BuildCtx`, `BuildReport`, `BuildError` |
| `crates/forge-phases/` | One module per phase (22+ modules including the in-process renderer T70) |
| `crates/forge-cli/`    | Binary entry point. **Replaces `forge.sh`.** |
| `crates/forge-serve/`  | Live-reload dev server. **Replaces `serve.py`.** |
| `crates/forge-replay/` | Replay a prior build report |
| `server-stub/`         | Per-`backends.toml` entry handler scaffolds |

## Removed 2026-05-14 (T54)

| Path | Replacement | Last commit before removal |
|------|-------------|----------------------------|
| `forge.sh`            | `cargo run -p forge-cli`                  | 1a83f09 |
| `forge_contrast.py`   | `crates/forge-phases/src/contrast.rs`     | 1a83f09 |
| `inject_backends.py`  | `crates/forge-phases/src/backend_coverage.rs` | 1a83f09 |
| `inject_seo.py`       | `crates/forge-phases/src/seo.rs`          | 1a83f09 |
| `inject_sri.py`       | `crates/forge-phases/src/sri.rs`          | 1a83f09 |
| `serve.py`            | `cargo run -p forge-serve`                | 1a83f09 |

If you need the historical bash/python source for parity reference,
`git show 1a83f09:forge.sh` etc. retrieves it. The Rust replacements
are tested, fuzzed, and AVP-2-audited; bash/python equivalents are
not and never will be.

## Why removal

- Bash and Python make refactoring impossible (no types, no compiler).
- The AVP-2 supersociety stack mandates Rust for everything that
  runs in production: deny `unsafe_code`, no `unwrap`/`expect` in
  lib code, property-based testing, fuzz targets, Miri-clean.
- Bash and Python phases cannot be unit-tested or fuzzed in isolation.
- The Rust port has been at full parity (20 of 20 effective phases)
  since 2026-05-04. The bash code has had no contributions since
  then; keeping it lying around invites accidental re-use.

## Active port tasks (closed 2026-05-14)

- ~T51~ — port `phase_theme_consistency` to Rust → DONE
- ~T52~ — port `phase_crawl` to Rust → DONE
- ~T54~ — delete `forge.sh` and Python helpers → DONE (this commit)
- T70b — moved `page_shell` from `loom-cli` into `loom-cms-render`
  so Forge inherits the same WCAG-AA + dual-theme defaults via
  the public render API. → DONE 2026-05-14.

## Open follow-ups

- T70c — flip `static/` to canonical `phase_render` output (currently
  emits to `static/_render/` to avoid clobbering hand-curated demo
  files; needs an audit + opt-in flag in `forge.toml` first).
