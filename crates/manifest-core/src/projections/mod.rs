//! Bridges from pre-existing PlausiDen file formats into
//! manifest-core types.
//!
//! Each submodule reads a domain-specific TOML/JSON file that
//! predates the manifest layer and projects its entries into typed
//! [`crate`] surface (capabilities, phases, backends). The original
//! file format stays untouched — old parsers keep working — and
//! new consumers go through the typed projection.
//!
//! Currently shipping:
//!   * [`backends_toml`] — app-level `backends.toml` (T31)
//!   * [`phases_toml`]   — workspace-level `phases.toml` (T32)

pub mod backends_toml;
pub mod phases_toml;
