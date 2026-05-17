//! `manifest-codegen-macros` — function-like proc-macros that
//! project a `PlatformManifest` into Rust at compile time.
//!
//! Companion to the `build.rs` path in
//! [`manifest_codegen::generate_from_file`]. Use the proc-macro
//! path when you want one-line ergonomics and don't want to
//! maintain a `build.rs` for each consumer crate. Use the
//! `build.rs` path when you want zero proc-macro deps in your
//! consumer graph.
//!
//! Both paths produce equivalent generated source — the projector
//! logic in `manifest-codegen` is shared.
//!
//! # Example
//!
//! ```ignore
//! manifest_codegen_macros::include_manifest!("manifest.toml");
//!
//! // Now in scope: ALL_CAPABILITIES, ALL_PHASES, ALL_BACKENDS,
//! //               MANIFEST_PLATFORM, MANIFEST_SCHEMA.
//! for cap in ALL_CAPABILITIES {
//!     println!("{}: {}", cap.id, cap.summary);
//! }
//! ```
//!
//! ### Path resolution
//!
//! The path is resolved relative to `CARGO_MANIFEST_DIR` (the
//! consumer crate's `Cargo.toml` directory), matching the
//! convention used by `include_bytes!` + `include_str!`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::path::PathBuf;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, LitStr};

/// Read a `PlatformManifest` from `path` (relative to
/// `CARGO_MANIFEST_DIR`), project it to Rust, and emit the
/// generated tokens at the macro call site.
///
/// Emits the same constants the `build.rs` path emits:
/// `MANIFEST_SCHEMA`, `MANIFEST_PLATFORM`, `ALL_CAPABILITIES`,
/// `ALL_PHASES`, `ALL_BACKENDS`, and their record types.
#[proc_macro]
pub fn include_manifest(input: TokenStream) -> TokenStream {
    let path_lit = parse_macro_input!(input as LitStr);
    let rel = path_lit.value();
    let root = match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(d) => PathBuf::from(d),
        Err(_) => {
            return syn::Error::new_spanned(
                &path_lit,
                "CARGO_MANIFEST_DIR not set; include_manifest! must be invoked from a Cargo build",
            )
            .to_compile_error()
            .into();
        }
    };
    let full = root.join(&rel);

    let src = match std::fs::read_to_string(&full) {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("could not read manifest {}: {e}", full.display());
            return syn::Error::new_spanned(&path_lit, msg)
                .to_compile_error()
                .into();
        }
    };
    let manifest = match manifest_core::PlatformManifest::from_toml(&src) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("manifest parse failed: {e}");
            return syn::Error::new_spanned(&path_lit, msg)
                .to_compile_error()
                .into();
        }
    };

    let generated = manifest_codegen::project_to_rust(&manifest);
    let tokens: proc_macro2::TokenStream = match generated.parse() {
        Ok(t) => t,
        Err(e) => {
            let msg = format!("internal codegen tokenisation failed: {e}");
            return syn::Error::new_spanned(&path_lit, msg)
                .to_compile_error()
                .into();
        }
    };

    let path_str = full.to_string_lossy();
    let expanded = quote! {
        // Hint to rust-analyzer + cargo that this file participates
        // in the dependency graph. The build.rs path uses
        // `cargo:rerun-if-changed`; the proc-macro path uses an
        // include_bytes! at the call site so edits trigger
        // recompilation of the consumer.
        const _: &[u8] = include_bytes!(#path_str);

        #tokens
    };

    expanded.into()
}
