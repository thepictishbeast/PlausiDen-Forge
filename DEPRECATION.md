# DEPRECATED FILES — do not extend

**Owner directive 2026-05-06:** Forge IS a Rust application. The bash
script `forge.sh` and any python helpers are parity references during
migration only. **Do not add new phases or logic to bash/python.**

## Live (write here)

| Path | Role |
|------|------|
| `crates/forge-core/`   | `Phase` trait, `Finding`, `Severity`, `BuildCtx`, `BuildReport`, `BuildError` |
| `crates/forge-phases/` | One module per phase (22+ modules) |
| `crates/forge-cli/`    | Binary entry point. **Replaces `forge.sh`.** |
| `crates/forge-serve/`  | Live-reload dev server |
| `crates/forge-replay/` | Replay a prior build report |
| `server-stub/`         | Per-`backends.toml` entry handler scaffolds |

## Deprecated (do not extend)

| Path | Status | Replacement |
|------|--------|-------------|
| `forge.sh`            | **DEPRECATED**   | `cargo run -p forge-cli` |
| `forge_contrast.py`   | **DEPRECATED**   | `crates/forge-phases/src/contrast.rs` |

## Why

- Bash makes refactoring impossible (no types, no compiler).
- The AVP-2 supersociety stack mandates Rust for everything that
  runs in production: deny `unsafe_code`, no `unwrap`/`expect` in
  lib code, property-based testing, fuzz targets, Miri-clean.
- Bash phases cannot be unit-tested or fuzzed in isolation.
- Bash duplicates what `forge-phases/` already implements 20-of-20
  effective phases for.

## Migration policy

A change to a deprecated file is allowed ONLY when:

1. It is a doc/comment fix to clarify the deprecation, OR
2. It is a backport of a Rust phase fix to keep parity until
   T54 (delete `forge.sh`) lands.

Any other contribution to `forge.sh` or `forge_contrast.py` should
be rejected at review and re-implemented in the Rust workspace.

## Active port tasks

- T51 — port `phase_theme_consistency` to Rust (in flight)
- T52 — port `phase_crawl` to Rust
- T54 — delete `forge.sh` once Rust forge has full coverage
