//! Forge phases. Each phase implements `forge_core::Phase`.
//! Phases live in modules; the runner instantiates them and
//! drives the build.
//!
//! Port progress (T11): loom_sync, tokens, html_semantic, csp
//! ported. 18 more queued. See AVP-2 doctrine for invariants.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod csp;
pub mod html_semantic;
pub mod html_walk;
pub mod loom_sync;
pub mod tokens;
