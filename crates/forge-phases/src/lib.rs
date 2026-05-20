//! Forge phases. Each phase implements `forge_core::Phase`.
//! Phases live in modules; the runner instantiates them and
//! drives the build.
//!
//! Port progress (T11): 20 of 22 phases ported. T11 COMPLETE.
//! Retired: viewport_audit, selfaudit.
//! Net: 20 of 20 effective phases now in Rust.
//!
//! T51 (2026-05-06): + theme_consistency (newly added phase that
//! shipped first in bash; supersociety Rust port). T52 will add
//! phase_crawl. After both land, T54 deletes forge.sh.

#![forbid(unsafe_code)]
// T96 cleanup: discipline gate (T92) requires deny-not-warn so a
// missing doc on a new public item fails CI at PR time. Existing
// warn-level violations (26 fields across 4 files) cleaned up in
// the same commit.
#![deny(missing_docs)]

pub mod a11y_landmarks;
pub mod aesthetic_distinctiveness;
pub mod annotation_review;
pub mod density_audit;
pub mod disclosure_audit;
pub mod placeholder_value_audit;
pub mod asset_optimization;
pub mod backend_coverage;
pub mod carbon_budget;
pub mod contrast;
pub mod crawl;
pub mod csp;
pub mod csp_devmode;
pub mod dns_hygiene_lint;
pub mod dual_theme;
pub mod dynamic_runtime;
pub mod external_assets;
pub mod html_semantic;
pub mod html_walk;
pub mod hunted_tier;
pub mod id_strategy;
pub mod iso_8601;
pub mod jurisdiction_compliance;
pub mod label_consistency;
pub mod link_check;
pub mod locale_html_lang;
pub mod loom_lint;
pub mod loom_sync;
pub mod motion;
pub mod motion_respects_reduced;
pub mod noscript_strict;
pub mod network_target_enforcement;
pub mod path_consistency;
pub mod perf_budget;
pub mod phantom_button;
pub mod print_stylesheet;
pub mod reader_safety;
pub mod render;
pub mod required_pages;
pub mod self_check;
pub mod semver_enforcement;
pub mod seo;
pub mod sri;
pub mod structured_data;
pub mod substrate_purity;
pub mod theme_consistency;
pub mod theme_contrast;
pub mod tokens;
pub mod trait_consistency;
pub mod trait_implications;
pub mod unbuilt_route;
pub mod validate_cms;
