//! `jurisdiction_compliance` — verify per-jurisdiction compliance
//! markers (cookie banner, CCPA "Do Not Sell" link, LGPD,
//! age verification) actually appear in the rendered HTML when the
//! site declares operating in those jurisdictions.
//!
//! Captures SITE_OPERATIONS.md §3 jurisdiction-aware behavior.
//! "Cookie Policy page exists" is checked by `phase_required_pages`;
//! THIS phase checks that the *runtime markers* (banner widget,
//! footer link, age gate) actually appear in the rendered HTML.
//! A site can legally have a Privacy Policy and STILL ship illegal
//! cookie-banner behavior — this phase closes that gap.
//!
//! ## Configuration
//!
//! Reads `[jurisdiction_compliance]` from `forge.toml`:
//!
//! ```toml
//! [jurisdiction_compliance]
//! # Lowercased declared jurisdictions. Drives which markers are
//! # required. Shares vocabulary with [required_pages].jurisdictions.
//! jurisdictions = ["eu", "us-ca", "br"]
//!
//! # Optional: content categories with extra jurisdiction overlays.
//! # "alcohol" or "gambling" + appropriate jurisdiction triggers
//! # age-verification flow requirement.
//! content_categories = ["alcohol"]
//!
//! # Optional: skip checks for specific pages (e.g. iframe embeds
//! # that legitimately have no chrome).
//! skip_pages = ["embeds/widget.html"]
//! ```
//!
//! Missing `[jurisdiction_compliance]` section → silent skip.
//!
//! ## Marker contract
//!
//! Each compliance marker is a substring that platform-emitted
//! markup is expected to include. Operators using bespoke markup
//! can supply custom needles via the optional `[jurisdiction_compliance.markers]`
//! sub-table.
//!
//! Default markers (drawn from Loom's emitted compliance widgets):
//!
//! - `data-loom-cookie-banner` → EU cookie banner widget
//! - `data-loom-ccpa-do-not-sell` → CCPA opt-out link
//! - `data-loom-lgpd-consent` → LGPD consent widget
//! - `data-loom-age-gate` → age verification flow
//!
//! ## Severity
//!
//! - EU jurisdiction declared but no cookie banner marker → **Strict**
//!   (illegal under GDPR; pre-ticked cookie banners are the most
//!   common compliance failure)
//! - California (`us-ca`) declared but no CCPA "Do Not Sell" link →
//!   **Strict**
//! - Brazil (`br`) declared but no LGPD consent widget → **Strict**
//! - Alcohol/gambling content category + applicable jurisdiction but
//!   no age-gate marker → **Strict**

use std::collections::HashSet;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `jurisdiction_compliance` phase.
#[derive(Debug, Default)]
pub struct JurisdictionCompliancePhase;

impl Phase for JurisdictionCompliancePhase {
    fn name(&self) -> &'static str {
        "jurisdiction_compliance"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let Some(cfg) = forge_toml_jurisdiction_compliance(&ctx.root) else {
            tracing::debug!("jurisdiction_compliance: no [jurisdiction_compliance] — skip");
            return Ok(vec![]);
        };
        let markers = required_markers_for(&cfg);
        if markers.is_empty() {
            return Ok(vec![]);
        }

        let mut findings = Vec::new();
        let files = walk_html(&ctx.static_dir, self.name())?;
        for marker in &markers {
            let mut found_anywhere = false;
            for file in &files {
                if cfg.skip_pages.iter().any(|s| s == &file.name) {
                    continue;
                }
                if file.body.contains(&marker.needle) {
                    found_anywhere = true;
                    break;
                }
            }
            if !found_anywhere {
                findings.push(Finding::strict(
                    self.name(),
                    marker.id.clone(),
                    format!(
                        "{description} — declared jurisdiction(s) {jur} require this marker, \
                         but no rendered page contains `{needle}`. Either add the \
                         Loom-emitted widget, or supply a custom marker via \
                         [jurisdiction_compliance.markers].{id} in forge.toml.",
                        description = marker.description,
                        jur = marker.triggering_jurisdictions.join(","),
                        needle = marker.needle,
                        id = marker.id,
                    ),
                ));
            }
        }
        Ok(findings)
    }
}

#[derive(Debug, Clone, Default)]
struct JurisdictionConfig {
    jurisdictions: HashSet<String>,
    content_categories: HashSet<String>,
    skip_pages: Vec<String>,
    custom_markers: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
struct RequiredMarker {
    id: String,
    description: &'static str,
    needle: String,
    triggering_jurisdictions: Vec<String>,
}

fn required_markers_for(cfg: &JurisdictionConfig) -> Vec<RequiredMarker> {
    let eu_codes: &[&str] = &[
        "eu", "de", "fr", "es", "it", "nl", "pl", "pt", "se", "ie", "at", "be", "fi", "dk", "cy",
        "cz", "gr", "hr", "hu", "lt", "lu", "lv", "mt", "ro", "si", "sk", "bg", "ee",
    ];
    let mut out: Vec<RequiredMarker> = Vec::new();
    let custom = |id: &str, default: &str| -> String {
        cfg.custom_markers
            .get(id)
            .cloned()
            .unwrap_or_else(|| default.to_owned())
    };

    // EU → cookie banner
    let eu_triggers: Vec<String> = cfg
        .jurisdictions
        .iter()
        .filter(|j| eu_codes.contains(&j.as_str()))
        .cloned()
        .collect();
    if !eu_triggers.is_empty() {
        out.push(RequiredMarker {
            id: "cookie_banner".to_owned(),
            description: "GDPR cookie banner required for EU traffic (must be \
                          opt-in not opt-out; pre-ticked boxes illegal)",
            needle: custom("cookie_banner", "data-loom-cookie-banner"),
            triggering_jurisdictions: eu_triggers,
        });
    }

    // California → CCPA "Do Not Sell" link
    if cfg.jurisdictions.contains("us-ca") {
        out.push(RequiredMarker {
            id: "ccpa_do_not_sell".to_owned(),
            description: "CCPA \"Do Not Sell My Personal Information\" link \
                          required in footer for California residents",
            needle: custom("ccpa_do_not_sell", "data-loom-ccpa-do-not-sell"),
            triggering_jurisdictions: vec!["us-ca".to_owned()],
        });
    }

    // Brazil → LGPD
    if cfg.jurisdictions.contains("br") {
        out.push(RequiredMarker {
            id: "lgpd_consent".to_owned(),
            description: "LGPD consent widget required for Brazilian traffic",
            needle: custom("lgpd_consent", "data-loom-lgpd-consent"),
            triggering_jurisdictions: vec!["br".to_owned()],
        });
    }

    // Age-restricted content + applicable jurisdiction → age gate
    let needs_age_gate = (cfg.content_categories.contains("alcohol")
        || cfg.content_categories.contains("gambling"))
        && (cfg.jurisdictions.contains("us") || !eu_codes.is_empty());
    if needs_age_gate {
        let cats: Vec<String> = cfg
            .content_categories
            .iter()
            .filter(|c| matches!(c.as_str(), "alcohol" | "gambling"))
            .cloned()
            .collect();
        out.push(RequiredMarker {
            id: "age_gate".to_owned(),
            description: "Age-verification flow required for restricted content \
                          (alcohol/gambling) in declared jurisdictions",
            needle: custom("age_gate", "data-loom-age-gate"),
            triggering_jurisdictions: cats,
        });
    }

    out
}

fn forge_toml_jurisdiction_compliance(root: &Path) -> Option<JurisdictionConfig> {
    let path = root.join("forge.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    let parsed: toml::Value = content.parse().ok()?;
    let section = parsed.get("jurisdiction_compliance")?;
    let jurisdictions: HashSet<String> = section
        .get("jurisdictions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_ascii_lowercase()))
                .collect()
        })
        .unwrap_or_default();
    let content_categories: HashSet<String> = section
        .get("content_categories")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_ascii_lowercase()))
                .collect()
        })
        .unwrap_or_default();
    let skip_pages: Vec<String> = section
        .get("skip_pages")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let custom_markers: std::collections::BTreeMap<String, String> = section
        .get("markers")
        .and_then(|v| v.as_table())
        .map(|t| {
            t.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                .collect()
        })
        .unwrap_or_default();
    Some(JurisdictionConfig {
        jurisdictions,
        content_categories,
        skip_pages,
        custom_markers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::{BuildMode, Severity};

    fn ctx_in(dir: &Path) -> BuildCtx {
        BuildCtx {
            root: dir.to_path_buf(),
            static_dir: dir.to_path_buf(),
            mode: BuildMode::Poc,
        }
    }

    fn write_forge_toml(dir: &Path, body: &str) {
        std::fs::write(dir.join("forge.toml"), body).unwrap();
    }

    fn write_page(dir: &Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).unwrap();
    }

    #[test]
    fn no_config_silent_skip() {
        let dir = tempfile::tempdir().unwrap();
        write_page(dir.path(), "page.html", "<html><body>x</body></html>");
        let findings = JurisdictionCompliancePhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn eu_jurisdiction_without_cookie_banner_emits_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[jurisdiction_compliance]
jurisdictions = ["eu"]
"#,
        );
        write_page(
            dir.path(),
            "index.html",
            "<html><body>plain page no banner</body></html>",
        );
        let findings = JurisdictionCompliancePhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Strict);
        assert!(findings[0].message.contains("cookie banner"));
        assert!(findings[0].message.contains("data-loom-cookie-banner"));
    }

    #[test]
    fn eu_jurisdiction_with_cookie_banner_accepted() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[jurisdiction_compliance]
jurisdictions = ["de"]
"#,
        );
        write_page(
            dir.path(),
            "index.html",
            r#"<html><body><div data-loom-cookie-banner></div></body></html>"#,
        );
        let findings = JurisdictionCompliancePhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn california_without_ccpa_link_emits_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[jurisdiction_compliance]
jurisdictions = ["us-ca"]
"#,
        );
        write_page(
            dir.path(),
            "index.html",
            "<html><body>no ccpa link</body></html>",
        );
        let findings = JurisdictionCompliancePhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert!(findings
            .iter()
            .any(|f| { f.severity == Severity::Strict && f.path == "ccpa_do_not_sell" }));
    }

    #[test]
    fn brazil_lgpd_required() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[jurisdiction_compliance]
jurisdictions = ["br"]
"#,
        );
        write_page(dir.path(), "index.html", "<html><body>x</body></html>");
        let findings = JurisdictionCompliancePhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert!(findings.iter().any(|f| f.path == "lgpd_consent"));
    }

    #[test]
    fn alcohol_content_us_requires_age_gate() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[jurisdiction_compliance]
jurisdictions = ["us"]
content_categories = ["alcohol"]
"#,
        );
        write_page(
            dir.path(),
            "index.html",
            "<html><body>spirits</body></html>",
        );
        let findings = JurisdictionCompliancePhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert!(findings.iter().any(|f| f.path == "age_gate"));
    }

    #[test]
    fn custom_marker_override_via_config() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[jurisdiction_compliance]
jurisdictions = ["eu"]

[jurisdiction_compliance.markers]
cookie_banner = "data-my-cookie-thing"
"#,
        );
        write_page(
            dir.path(),
            "index.html",
            r#"<html><body><div data-my-cookie-thing></div></body></html>"#,
        );
        let findings = JurisdictionCompliancePhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn marker_present_on_any_page_satisfies_all_pages() {
        // The marker only needs to appear on AT LEAST ONE page —
        // typical pattern is the cookie banner appearing in the
        // shared footer template included on every page, but the
        // phase doesn't require per-page enforcement (each page
        // checking is the wrong abstraction; if the operator's
        // shared chrome includes the banner, every page has it).
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[jurisdiction_compliance]
jurisdictions = ["eu"]
"#,
        );
        write_page(
            dir.path(),
            "index.html",
            r#"<html><body><div data-loom-cookie-banner></div></body></html>"#,
        );
        write_page(
            dir.path(),
            "about.html",
            "<html><body>about page no banner</body></html>",
        );
        let findings = JurisdictionCompliancePhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn skip_pages_excluded_from_marker_search() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[jurisdiction_compliance]
jurisdictions = ["eu"]
skip_pages = ["index.html"]
"#,
        );
        // banner IS on index but index is skipped → finding should fire
        write_page(
            dir.path(),
            "index.html",
            r#"<html><body><div data-loom-cookie-banner></div></body></html>"#,
        );
        write_page(dir.path(), "about.html", "<html><body>about</body></html>");
        let findings = JurisdictionCompliancePhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "cookie_banner");
    }

    #[test]
    fn multiple_eu_jurisdictions_collapse_to_one_finding() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[jurisdiction_compliance]
jurisdictions = ["de", "fr", "es", "it"]
"#,
        );
        write_page(dir.path(), "index.html", "<html><body>x</body></html>");
        let findings = JurisdictionCompliancePhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        // Single cookie_banner finding (not 4 — EU is one bucket)
        let banner_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.path == "cookie_banner")
            .collect();
        assert_eq!(banner_findings.len(), 1);
        // Message lists all 4 triggering jurisdictions
        assert!(banner_findings[0].message.contains("de"));
        assert!(banner_findings[0].message.contains("fr"));
    }

    #[test]
    fn combined_eu_california_brazil_independent_findings() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[jurisdiction_compliance]
jurisdictions = ["eu", "us-ca", "br"]
"#,
        );
        write_page(dir.path(), "index.html", "<html><body>x</body></html>");
        let findings = JurisdictionCompliancePhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        let ids: HashSet<String> = findings.iter().map(|f| f.path.clone()).collect();
        assert!(ids.contains("cookie_banner"));
        assert!(ids.contains("ccpa_do_not_sell"));
        assert!(ids.contains("lgpd_consent"));
    }
}
