//! server-stub — handler scaffolds for every entry in backends.toml.
//!
//! Each module under `handlers/` corresponds to one
//! `[backends.X]` section; the file path is
//! `src/handlers/<key_with_dashes_to_underscores>.rs`.
//!
//! Bodies are placeholders that return `Ok(Response { ok: true })`.
//! Replace with real implementations as each backend wires up.
//! Add new modules to `handlers/mod.rs` when `loom backend-stub`
//! scaffolds a new file.

pub mod handlers;
