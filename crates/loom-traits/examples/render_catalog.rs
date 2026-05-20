//! Render the trait catalog to stdout.
//!
//! Usage:
//! ```
//! cargo run -p loom-traits --example render_catalog > docs/traits-catalog.md
//! ```
//!
//! Output is deterministic per `[[deterministic-first-lfi-optional]]`:
//! same source → same bytes.

fn main() {
    print!("{}", loom_traits::render_markdown_catalog());
}
