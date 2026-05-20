//! `placeholder_value_audit` — scaffold-default value detector.
//!
//! Forge phase that scans CMS JSON for default values that
//! operators forgot to customize after running `forge init` /
//! `loom site init` / similar scaffolding. Catches the most
//! common "shipped the placeholder to production" bug class
//! BEFORE the build emits HTML that says "My Site" in the
//! browser tab.
//!
//! ## Why this is a separate phase
//!
//! - The `placeholder_text` Crawler detector catches BODY copy
//!   (Lorem ipsum, "delete me", "sample text"). This phase
//!   catches METADATA-level scaffold defaults that body-text
//!   detection misses because they're in titles / descriptions
//!   / brand / contact fields.
//! - Catches at BUILD time, not at runtime probe time — saves
//!   the round trip through Crawler. Operators get the warning
//!   the same moment they run `forge build`.
//!
//! ## Findings
//!
//! * `placeholder-value.unchanged-scaffold` strict — a field
//!   value exactly matches a known scaffold-default string.
//!   E.g., `title: "My Site"` ships unchanged.
//! * `placeholder-value.empty-required` warn — a typically-
//!   required field (title, brand, description) is empty or
//!   whitespace. Warn-only because some pages legitimately
//!   omit (e.g., a hidden admin shell).
//!
//! ## Detection strategy
//!
//! Walk `cms/*.json` (no recursion — top-level only). For each
//! page, check the top-level scaffold-prone fields against the
//! `SCAFFOLD_DEFAULTS` and `EMPTY_PLACEHOLDERS` lists. Hits emit
//! findings.
//!
//! Pure phase — no I/O beyond reading the cms directory.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"` (inherited).
//! * Pure detector logic; tests exercise via in-memory JSON.

use std::fs;

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde_json::Value;

/// Field-name → list-of-default-values map (inline).
/// Each entry is a (field_name, scaffold_value) pair the phase
/// flags as a strict finding when the rendered CMS JSON carries
/// exactly that value.
///
/// Curated from common scaffold templates: `forge site init`,
/// `loom site init`, generic CMS starter kits. Operators with
/// distinct conventional placeholders can extend via a per-
/// tenant config (future work — see CONSUMER_SHAPING_AUDIT.md
/// per-tenant corpora discussion).
const SCAFFOLD_DEFAULTS: &[(&str, &str)] = &[
    // Top-of-page identity
    ("title", "My Site"),
    ("title", "New Site"),
    ("title", "Untitled"),
    ("title", "Untitled Site"),
    ("title", "Hello World"),
    ("title", "Welcome"),
    ("title", "Welcome to my site"),
    ("title", "Welcome to your new site"),
    ("brand", "Brand"),
    ("brand", "Your Brand"),
    ("brand", "Site Name"),
    ("brand", "Acme"),
    ("brand", "Acme Inc"),
    ("brand", "Company Name"),
    // Description / SEO
    ("description", "Your site description"),
    ("description", "Add a description"),
    ("description", "Site description goes here"),
    ("description", "A short description"),
    ("description", "Lorem ipsum dolor sit amet"),
    // Contact placeholders
    ("email", "you@example.com"),
    ("email", "hello@example.com"),
    ("email", "info@example.com"),
    ("email", "contact@example.com"),
    ("email", "noreply@example.com"),
    ("email", "name@example.com"),
    ("phone", "(000) 000-0000"),
    ("phone", "000-000-0000"),
    ("phone", "555-555-5555"),
    ("phone", "+1 555 555 5555"),
    // Misc identity
    ("legal_name", "Your Company, LLC"),
    ("tagline", "Your tagline goes here"),
    ("tagline", "Add a tagline"),
];

/// Fields where empty/whitespace is a warn-level signal.
/// Title / brand / description aren't strict because some sites
/// (admin shells, single-purpose pages) legitimately omit; but
/// the operator should know.
const EMPTY_PLACEHOLDER_FIELDS: &[&str] = &["title", "brand", "description"];

/// `placeholder_value_audit` phase implementation.
#[derive(Debug, Default)]
pub struct PlaceholderValueAuditPhase;

impl Phase for PlaceholderValueAuditPhase {
    fn name(&self) -> &'static str {
        "placeholder_value_audit"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let cms_dir = ctx.root.join("cms");
        if !cms_dir.is_dir() {
            return Ok(findings);
        }
        let entries = fs::read_dir(&cms_dir).map_err(|e| BuildError::Io {
            context: format!("read_dir {}", cms_dir.display()),
            source: e,
        })?;
        for entry in entries {
            let entry = entry.map_err(|e| BuildError::Io {
                context: format!("read_dir entry in {}", cms_dir.display()),
                source: e,
            })?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let raw = fs::read_to_string(&path).map_err(|e| BuildError::Io {
                context: format!("read {}", path.display()),
                source: e,
            })?;
            let Ok(value) = serde_json::from_str::<Value>(&raw) else {
                continue;
            };
            let path_disp = path.display().to_string();
            check_page(&path_disp, &value, &mut findings, self.name());
        }
        Ok(findings)
    }
}

/// Walk a CmsPage JSON value + check top-level fields against
/// the scaffold-default + empty-placeholder lists.
fn check_page(
    path: &str,
    page: &Value,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let Some(obj) = page.as_object() else {
        return;
    };
    for (field, scaffold_value) in SCAFFOLD_DEFAULTS {
        if let Some(actual) = obj.get(*field).and_then(|v| v.as_str()) {
            // Exact match (case-sensitive — typical scaffold defaults
            // are title-case; matching loosely would false-fire on
            // legitimate content).
            if actual == *scaffold_value {
                findings.push(Finding::strict(
                    phase,
                    path.to_owned(),
                    format!(
                        "placeholder_value_audit — {field} = \"{scaffold_value}\" is the unmodified scaffold default; operator should customize before shipping."
                    ),
                ));
            }
        }
    }
    for field in EMPTY_PLACEHOLDER_FIELDS {
        if let Some(actual) = obj.get(*field).and_then(|v| v.as_str()) {
            if actual.trim().is_empty() {
                findings.push(Finding::warn(
                    phase,
                    path.to_owned(),
                    format!(
                        "placeholder_value_audit — {field} is empty or whitespace-only; most pages should set this. If the omission is intentional (admin shell, hidden page), suppress this check via per-tenant config (future)."
                    ),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run_check(page: Value) -> Vec<Finding> {
        let mut findings = Vec::new();
        check_page("/cms/test.json", &page, &mut findings, "placeholder_value_audit");
        findings
    }

    #[test]
    fn customized_page_emits_no_findings() {
        let page = json!({
            "title": "Acme Solar — Residential Installations",
            "brand": "Acme Solar",
            "description": "Residential solar installation in the Pacific Northwest. Custom design, installation, monitoring.",
            "email": "info@acmesolar.example",
        });
        assert!(run_check(page).is_empty());
    }

    #[test]
    fn default_title_is_strict() {
        let findings = run_check(json!({"title": "My Site"}));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, forge_core::Severity::Strict);
        assert!(findings[0].message.contains("title"));
        assert!(findings[0].message.contains("My Site"));
    }

    #[test]
    fn default_brand_is_strict() {
        let findings = run_check(json!({"brand": "Acme"}));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, forge_core::Severity::Strict);
        assert!(findings[0].message.contains("brand"));
        assert!(findings[0].message.contains("Acme"));
    }

    #[test]
    fn default_email_is_strict() {
        let findings = run_check(json!({"email": "you@example.com"}));
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("email"));
        assert!(findings[0].message.contains("you@example.com"));
    }

    #[test]
    fn empty_title_is_warn() {
        let findings = run_check(json!({"title": ""}));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, forge_core::Severity::Warn);
        assert!(findings[0].message.contains("empty"));
    }

    #[test]
    fn whitespace_only_title_is_warn() {
        let findings = run_check(json!({"title": "   "}));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, forge_core::Severity::Warn);
    }

    #[test]
    fn case_sensitive_matching() {
        // Lowercase scaffold value should NOT match the title-cased
        // default. Operators with deliberately-lowercase brand
        // names ("acme") don't get false-positive findings.
        let findings = run_check(json!({"brand": "acme"}));
        assert!(findings.is_empty());
    }

    #[test]
    fn multiple_fields_with_defaults_emit_multiple_findings() {
        let findings = run_check(json!({
            "title": "My Site",
            "brand": "Brand",
            "description": "Your site description",
            "email": "you@example.com",
        }));
        assert_eq!(findings.len(), 4);
        assert!(findings.iter().all(|f| f.severity == forge_core::Severity::Strict));
    }

    #[test]
    fn scaffold_defaults_list_covers_known_offenders() {
        // Drift-guard against the const accidentally shrinking.
        let names: std::collections::BTreeSet<&str> =
            SCAFFOLD_DEFAULTS.iter().map(|(n, _)| *n).collect();
        for needle in ["title", "brand", "description", "email", "phone"] {
            assert!(names.contains(needle), "SCAFFOLD_DEFAULTS missing {needle}");
        }
    }

    #[test]
    fn empty_placeholder_fields_list_covers_known_required() {
        for needle in ["title", "brand", "description"] {
            assert!(
                EMPTY_PLACEHOLDER_FIELDS.contains(&needle),
                "EMPTY_PLACEHOLDER_FIELDS missing {needle}"
            );
        }
    }

    #[test]
    fn non_string_values_silent() {
        // Numeric / boolean / null values for these fields don't
        // match any scaffold-string; phase should silently skip
        // rather than panic on type mismatch.
        let findings = run_check(json!({
            "title": 42,
            "brand": null,
            "description": true,
        }));
        assert!(findings.is_empty());
    }

    #[test]
    fn missing_object_root_silent() {
        // CMS JSON that's not an object (array, string, etc.)
        // should silently skip, not panic.
        let findings = run_check(json!([1, 2, 3]));
        assert!(findings.is_empty());
        let findings = run_check(json!("just a string"));
        assert!(findings.is_empty());
    }
}
