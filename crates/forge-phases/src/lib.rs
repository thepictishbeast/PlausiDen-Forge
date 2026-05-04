//! Forge phases. Each phase implements `forge_core::Phase`.
//! Phases live in modules; the runner instantiates them and
//! drives the build.
//!
//! Port progress (T11): loom_sync, tokens, html_semantic, csp,
//! seo, perf_budget, sri ported. 15 more queued.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod csp;
pub mod html_semantic;
pub mod html_walk;
pub mod loom_sync;
pub mod perf_budget;
pub mod seo;
pub mod sri;
pub mod tokens;
