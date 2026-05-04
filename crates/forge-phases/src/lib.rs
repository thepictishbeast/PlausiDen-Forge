//! Forge phases. Each phase implements `forge_core::Phase`.
//! Phases live in modules; the runner instantiates them and
//! drives the build.
//!
//! Port progress (T11): 15 of 22 phases ported.
//! `viewport_audit` retired (Crawler journeys cover that ground).
//! `selfaudit` retired (Rust type system replaces phase-wiring check).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod a11y_landmarks;
pub mod backend_coverage;
pub mod csp;
pub mod csp_devmode;
pub mod external_assets;
pub mod html_semantic;
pub mod html_walk;
pub mod label_consistency;
pub mod loom_sync;
pub mod motion;
pub mod perf_budget;
pub mod phantom_button;
pub mod seo;
pub mod sri;
pub mod tokens;
pub mod unbuilt_route;
