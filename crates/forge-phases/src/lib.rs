//! Forge phases. Each phase implements `forge_core::Phase`.
//! Phases live in modules; the runner instantiates them and
//! drives the build.
//!
//! Port progress (T11): 11 of 22 phases ported.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod csp;
pub mod csp_devmode;
pub mod external_assets;
pub mod html_semantic;
pub mod html_walk;
pub mod loom_sync;
pub mod motion;
pub mod perf_budget;
pub mod phantom_button;
pub mod seo;
pub mod sri;
pub mod tokens;
