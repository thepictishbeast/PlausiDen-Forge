//! `structural` — nav + page-type taxonomy + cross-page link
//! extraction.
//!
//! Task #270 per the reference-matching arc. Reads a
//! [`StructuralDump`] emitted by the Crawler (one per reference
//! site) and produces:
//!
//! * `nav_shape` — how the navigation surface is composed
//!   (top-level item count, has-sticky-header, has-footer-nav).
//! * `page_type_distribution` — counts of each URL-pattern
//!   bucket (homepage / article / docs / pricing / about /
//!   contact / other) inferred from URL slugs.
//! * `cross_page_link_density` — average outbound-link count
//!   per captured page (does the site cross-link richly or
//!   stand alone per page?).
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `#[non_exhaustive]` on every public type.
//! * No unwrap/expect in non-test code.
//! * Pure functions over the in-memory dump.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::extractors::ExtractorError;

/// Structural-dump spec version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StructuralSpec {
    /// Initial spec.
    #[default]
    V1,
}

impl StructuralSpec {
    /// Stable kebab-case slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::V1 => "v1",
        }
    }
}

/// One page entry — what the Crawler captured about each
/// distinct URL it walked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PageEntry {
    /// Path component of the URL (no scheme/host; query stripped).
    pub path: String,
    /// Outbound-link count from this page.
    pub outbound_link_count: u32,
}

/// Crawler-emitted structural dump.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StructuralDump {
    /// Schema version.
    pub spec: StructuralSpec,
    /// Top-level nav items found in `<nav>` or `<header>`.
    pub nav_items: Vec<String>,
    /// Whether the site uses a sticky header (any element with
    /// `position: sticky` ancestor of `<nav>`).
    pub has_sticky_header: bool,
    /// Whether `<footer>` contains a `<nav>` block.
    pub has_footer_nav: bool,
    /// Every page captured.
    pub pages: Vec<PageEntry>,
}

impl NavShape {
    /// Construct a NavShape. Public constructor for external
    /// crates.
    #[must_use]
    pub fn new(item_count: u32, has_sticky_header: bool, has_footer_nav: bool) -> Self {
        Self {
            item_count,
            has_sticky_header,
            has_footer_nav,
        }
    }
}

impl StructuralResult {
    /// Construct a StructuralResult. Public constructor for
    /// external crates because the struct is `#[non_exhaustive]`.
    #[must_use]
    pub fn new(
        nav_shape: NavShape,
        page_type_distribution: BTreeMap<String, u32>,
        cross_page_link_density_avg: f64,
    ) -> Self {
        Self {
            nav_shape,
            page_type_distribution,
            cross_page_link_density_avg,
        }
    }
}

/// Aggregate structural result.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StructuralResult {
    /// Nav shape summary.
    pub nav_shape: NavShape,
    /// Distribution of page-type buckets inferred from path
    /// slugs. Key is bucket slug; value is page count.
    pub page_type_distribution: BTreeMap<String, u32>,
    /// Average outbound-link count per captured page. 0.0 when
    /// no pages.
    pub cross_page_link_density_avg: f64,
}

/// Nav-surface summary.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NavShape {
    /// Top-level nav item count.
    pub item_count: u32,
    /// Sticky-header flag.
    pub has_sticky_header: bool,
    /// Footer-nav flag.
    pub has_footer_nav: bool,
}

/// Extract from a JSON dump file.
pub fn extract_from_path(path: &Path) -> Result<StructuralResult, ExtractorError> {
    let body = std::fs::read_to_string(path)?;
    let dump: StructuralDump = serde_json::from_str(&body)?;
    Ok(extract(&dump))
}

/// Pure extraction over an in-memory dump.
#[must_use]
pub fn extract(dump: &StructuralDump) -> StructuralResult {
    let nav_shape = NavShape {
        item_count: u32::try_from(dump.nav_items.len()).unwrap_or(u32::MAX),
        has_sticky_header: dump.has_sticky_header,
        has_footer_nav: dump.has_footer_nav,
    };

    let mut page_type_distribution: BTreeMap<String, u32> = BTreeMap::new();
    let mut total_links: u64 = 0;
    for entry in &dump.pages {
        let bucket = classify_path(&entry.path);
        *page_type_distribution.entry(bucket.to_owned()).or_insert(0) += 1;
        total_links += u64::from(entry.outbound_link_count);
    }

    let cross_page_link_density_avg = if dump.pages.is_empty() {
        0.0
    } else {
        total_links as f64 / dump.pages.len() as f64
    };

    StructuralResult {
        nav_shape,
        page_type_distribution,
        cross_page_link_density_avg,
    }
}

fn classify_path(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    let trimmed = lower.trim_matches('/');
    if trimmed.is_empty() || trimmed == "index" || trimmed == "home" {
        return "homepage";
    }
    let first_seg = trimmed.split('/').next().unwrap_or("");
    let last_seg = trimmed.rsplit('/').next().unwrap_or("");

    // Article/blog patterns.
    if first_seg == "blog"
        || first_seg == "posts"
        || first_seg == "articles"
        || first_seg == "field-notes"
    {
        return "article";
    }
    // Docs / reference.
    if first_seg == "docs"
        || first_seg == "documentation"
        || first_seg == "reference"
        || first_seg == "guide"
        || first_seg == "guides"
        || first_seg == "manual"
    {
        return "docs";
    }
    // Pricing.
    if first_seg == "pricing"
        || first_seg == "plans"
        || last_seg == "pricing"
        || last_seg == "plans"
    {
        return "pricing";
    }
    // Legal (must come before about so /about/privacy-policy
    // routes correctly).
    if first_seg == "legal"
        || first_seg == "privacy"
        || first_seg == "terms"
        || last_seg == "privacy-policy"
        || last_seg == "terms-of-service"
    {
        return "legal";
    }
    // About / company.
    if first_seg == "about" || first_seg == "company" || first_seg == "team" || last_seg == "about"
    {
        return "about";
    }
    // Contact / support.
    if first_seg == "contact"
        || first_seg == "support"
        || first_seg == "help"
        || last_seg == "contact"
    {
        return "contact";
    }
    // Changelog / releases.
    if first_seg == "changelog" || first_seg == "releases" || first_seg == "release-notes" {
        return "changelog";
    }
    // Case studies / customers.
    if first_seg == "customers" || first_seg == "case-studies" || first_seg == "stories" {
        return "case_study";
    }
    // Search.
    if first_seg == "search" {
        return "search";
    }
    "other"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(path: &str, links: u32) -> PageEntry {
        PageEntry {
            path: path.to_owned(),
            outbound_link_count: links,
        }
    }

    #[test]
    fn extract_nav_shape_reflects_dump() {
        let dump = StructuralDump {
            spec: StructuralSpec::V1,
            nav_items: vec!["Home".into(), "Docs".into(), "Pricing".into()],
            has_sticky_header: true,
            has_footer_nav: true,
            pages: vec![],
        };
        let r = extract(&dump);
        assert_eq!(r.nav_shape.item_count, 3);
        assert!(r.nav_shape.has_sticky_header);
        assert!(r.nav_shape.has_footer_nav);
    }

    #[test]
    fn classify_path_homepage() {
        assert_eq!(classify_path("/"), "homepage");
        assert_eq!(classify_path(""), "homepage");
        assert_eq!(classify_path("/index"), "homepage");
        assert_eq!(classify_path("/home"), "homepage");
    }

    #[test]
    fn classify_path_articles_and_docs() {
        assert_eq!(classify_path("/blog/my-post"), "article");
        assert_eq!(classify_path("/posts/2026-05/welcome"), "article");
        assert_eq!(classify_path("/articles/intro"), "article");
        assert_eq!(classify_path("/field-notes/2026-05-20"), "article");
        assert_eq!(classify_path("/docs/api"), "docs");
        assert_eq!(classify_path("/documentation/v2"), "docs");
        assert_eq!(classify_path("/guide/getting-started"), "docs");
    }

    #[test]
    fn classify_path_pricing_and_about() {
        assert_eq!(classify_path("/pricing"), "pricing");
        assert_eq!(classify_path("/plans"), "pricing");
        assert_eq!(classify_path("/about"), "about");
        assert_eq!(classify_path("/company/team"), "about");
        assert_eq!(classify_path("/team"), "about");
    }

    #[test]
    fn classify_path_contact_changelog_case_study() {
        assert_eq!(classify_path("/contact"), "contact");
        assert_eq!(classify_path("/support"), "contact");
        assert_eq!(classify_path("/changelog"), "changelog");
        assert_eq!(classify_path("/releases"), "changelog");
        assert_eq!(classify_path("/customers/acme"), "case_study");
        assert_eq!(classify_path("/case-studies/stripe"), "case_study");
    }

    #[test]
    fn classify_path_legal_search_other() {
        assert_eq!(classify_path("/legal/privacy"), "legal");
        assert_eq!(classify_path("/privacy"), "legal");
        assert_eq!(classify_path("/terms"), "legal");
        assert_eq!(classify_path("/about/privacy-policy"), "legal");
        assert_eq!(classify_path("/search"), "search");
        assert_eq!(classify_path("/weird-path"), "other");
        assert_eq!(classify_path("/programs/x"), "other");
    }

    #[test]
    fn extract_page_distribution_counts_buckets() {
        let dump = StructuralDump {
            spec: StructuralSpec::V1,
            nav_items: vec![],
            has_sticky_header: false,
            has_footer_nav: false,
            pages: vec![
                page("/", 5),
                page("/blog/a", 3),
                page("/blog/b", 2),
                page("/pricing", 4),
                page("/about", 6),
                page("/weird", 1),
            ],
        };
        let r = extract(&dump);
        assert_eq!(r.page_type_distribution.get("homepage").copied(), Some(1));
        assert_eq!(r.page_type_distribution.get("article").copied(), Some(2));
        assert_eq!(r.page_type_distribution.get("pricing").copied(), Some(1));
        assert_eq!(r.page_type_distribution.get("about").copied(), Some(1));
        assert_eq!(r.page_type_distribution.get("other").copied(), Some(1));
    }

    #[test]
    fn extract_cross_page_link_density_average() {
        let dump = StructuralDump {
            spec: StructuralSpec::V1,
            nav_items: vec![],
            has_sticky_header: false,
            has_footer_nav: false,
            pages: vec![page("/", 10), page("/about", 4), page("/contact", 1)],
        };
        let r = extract(&dump);
        assert!((r.cross_page_link_density_avg - (15.0 / 3.0)).abs() < 1e-9);
    }

    #[test]
    fn extract_returns_zero_density_for_no_pages() {
        let dump = StructuralDump {
            spec: StructuralSpec::V1,
            nav_items: vec![],
            has_sticky_header: false,
            has_footer_nav: false,
            pages: vec![],
        };
        let r = extract(&dump);
        assert_eq!(r.cross_page_link_density_avg, 0.0);
        assert!(r.page_type_distribution.is_empty());
    }

    #[test]
    fn extract_from_path_round_trips_dump() {
        let dump = StructuralDump {
            spec: StructuralSpec::V1,
            nav_items: vec!["A".into(), "B".into()],
            has_sticky_header: true,
            has_footer_nav: false,
            pages: vec![page("/", 3)],
        };
        let path = std::env::temp_dir().join(format!("forge-structural-{}", std::process::id()));
        std::fs::write(&path, serde_json::to_string(&dump).unwrap()).unwrap();
        let r = extract_from_path(&path).unwrap();
        assert_eq!(r.nav_shape.item_count, 2);
        assert_eq!(r.page_type_distribution.get("homepage").copied(), Some(1));
        let _ = std::fs::remove_file(&path);
    }
}
