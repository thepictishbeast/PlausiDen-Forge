# Substrate component templates

Cargo scaffolds for new substrate components with the canonical defaults pre-loaded per the [Substrate Discipline doctrine](https://github.com/thepictishbeast/PlausiDen-AVP-Doctrine/blob/main/SUBSTRATE_DISCIPLINE.md) (rule `build-004`, `prim-006`, et al).

## Why

Per [[substrate-only-path]] + [[tool-starvation-anti-pattern]]: the substrate's canonical stack choices are made. New components start from these templates so the "should I use Axum or Actix" question never arises — the answer is baked into the scaffold's `Cargo.toml`.

Reaching for a non-canonical lib in new substrate work requires (a) editing one of these templates with a rationale, (b) writing the ADR, (c) updating the doctrine. The friction is the discipline.

## Templates

| Template | When to use | Canonical deps pre-loaded |
|----------|-------------|---------------------------|
| `http-service/` | New HTTP service (admin API, webhook receiver, federation endpoint) | `axum` + `tokio` + `tower` + `serde` + `tracing` + `tracing-subscriber` + `anyhow` |
| `cli-tool/` | New CLI binary (operator tool, validator, generator) | `clap` (derive) + `anyhow` + `tracing` + `tracing-subscriber` + `serde` + `serde_json` |
| `library-crate/` | New `*-core` typed-surface crate | `serde` + `thiserror` + `proptest` (dev) — no I/O, no async |
| `forge-phase/` | New Forge audit phase | `forge-core` + `tracing` + `serde` + `proptest` (dev) — implements `Phase` trait |
| `core-types/` | New typed-surface crate for a new domain (commerce, federation, …) | `serde` + `thiserror` + `proptest` (dev) — same shape as the existing `*-core` crates |

## How to use

1. Copy the relevant template to `crates/<your-crate>/`.
2. Edit `Cargo.toml`:
   - Set `name = "your-crate"`
   - Update `description` to the one-sentence purpose.
3. Add the new crate to the workspace root `Cargo.toml` `[workspace.members]` list with a comment explaining what it's for.
4. Rename the placeholder identifiers in `src/lib.rs` or `src/main.rs`.
5. Run `cargo build -p your-crate` to verify it compiles clean.
6. Run `cargo test -p your-crate` to verify the proptest harness runs.

## What's baked in

Every template:
- `[lints]` block deferring to workspace-level lint configuration (set `missing_docs = "deny"` + `unwrap_used = "deny"` + `unsafe_code = "forbid"` per `build-003` / `build-004` / `docs-006`).
- `[package].publish = false` — workspace-internal crates never publish to crates.io.
- `edition.workspace = true` + `rust-version.workspace = true` for consistent toolchain.
- Workspace deps via `dep-name.workspace = true` — versions pinned at workspace root.
- AVP-2 invariant scaffolding: a `BUG ASSUMPTION` comment in src/lib.rs naming the main risk class, a stub property test, `#![forbid(unsafe_code)]`.

## What's NOT baked in

These templates are scaffolds, not opinions on:
- Crate naming (depends on domain — `*-core` for typed surfaces, `*-phase` for phases, etc.)
- Persistence layer (when needed, add `sqlx` workspace dep via the storage-service variant — pending)
- Cryptography (when needed, depend on `crucible-crypto` or `ed25519-dalek` workspace deps — never roll your own per rule `sec-002`)
- Property test arbitrary impls (write them per the crate's actual types)

## Verification cadence

Every template's `Cargo.toml` is verified to compile clean periodically:
```
for t in templates/*/; do
    name=$(basename "$t")
    test -f "$t/Cargo.toml" && cargo build --manifest-path "$t/Cargo.toml" 2>&1 || echo "❌ $name"
done
```

If a template breaks (workspace dep version bump, etc.), update the template's deps in the same PR. Templates that lag the workspace are misleading.
