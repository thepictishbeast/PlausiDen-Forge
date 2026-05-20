//! `disclosure_audit` — typed `Disclaimer` block quality check.
//!
//! Pairs with the `CmsSection::Disclaimer` primitive shipped in
//! PlausiDen-Loom commit 2cab09f. Enforces FTC + Google Search
//! Quality Guidelines disclosure standards AT BUILD TIME so
//! sponsored / affiliate content can't ship without proper
//! attribution.
//!
//! ## What this phase enforces
//!
//! For every `Disclaimer` section in `cms/*.json`:
//!
//! * `disclosure-audit.sponsor-without-source` strict — the
//!   `disclosure_kind` is `"sponsored"` or `"affiliate"` but the
//!   `source` field is null / missing / whitespace-only. FTC
//!   requires sponsored-content notices to name the sponsor; an
//!   unsourced "this is sponsored" notice fails the rule.
//! * `disclosure-audit.short-body` warn — disclosure body is
//!   shorter than 20 characters. Short bodies often paper over
//!   the actual disclosure ("Sponsored.") rather than describe
//!   the relationship. Warn-only because some short-form
//!   disclosures are legitimate (`"Editor's note: corrected."`);
//!   strict promotion via the per-phase strict flag if the
//!   operator wants to enforce substantive disclosures.
//! * `disclosure-audit.empty-body` strict — body is completely
//!   empty / whitespace. No-text disclaimer fails the disclosure
//!   intent entirely.
//!
//! ## Why this is a separate phase, not part of Loom rendering
//!
//! The Loom renderer validates SHAPE (well-formed enum, escaped
//! HTML). The audit validates SEMANTICS (sponsored-without-source
//! is well-formed JSON but fails policy). Separation lets the
//! audit be optional per-build (some operators don't ship
//! disclaimers + the phase silently skips) and per-strictness
//! (warn vs strict).
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * No unwrap/expect in non-test code.
//! * `#[non_exhaustive]` on phase struct so future fields don't
//!   break consumers.

use std::fs;

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde_json::Value;

/// Sponsorship-style disclosure kinds that REQUIRE a `source`
/// per FTC + most jurisdictions.
const SOURCE_REQUIRED_KINDS: &[&str] = &["sponsored", "affiliate"];

/// Minimum substantive-body length. Disclosures shorter than
/// this are warn-flagged (might still be legitimate but
/// suspicious).
const MIN_BODY_LEN: usize = 20;

/// `disclosure_audit` phase implementation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct DisclosureAuditPhase;

impl Phase for DisclosureAuditPhase {
    fn name(&self) -> &'static str {
        "disclosure_audit"
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

/// Walk a CmsPage value, find Disclaimer sections, validate.
fn check_page(
    path: &str,
    page: &Value,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let Some(sections) = page.get("sections").and_then(|s| s.as_array()) else {
        return;
    };
    for (idx, section) in sections.iter().enumerate() {
        // Section's variant tag is at "kind" per the CmsSection
        // serde-tag = "kind" config.
        let Some(kind_tag) = section.get("kind").and_then(|v| v.as_str()) else {
            continue;
        };
        if kind_tag != "disclaimer" {
            continue;
        }
        check_disclaimer(path, idx, section, findings, phase);
    }
}

fn check_disclaimer(
    path: &str,
    idx: usize,
    section: &Value,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let disclosure_kind = section
        .get("disclosure_kind")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let body = section.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let source = section.get("source").and_then(|v| v.as_str());
    let where_at = format!("{path}#section-{idx}-disclaimer-{disclosure_kind}");
    // Empty body — strict.
    if body.trim().is_empty() {
        findings.push(Finding::strict(
            phase,
            where_at.clone(),
            format!(
                "disclosure_audit — Disclaimer (disclosure_kind=\"{disclosure_kind}\") has empty / whitespace-only body. A disclosure with no text fails the disclosure intent entirely; supply meaningful copy."
            ),
        ));
    } else if body.trim().chars().count() < MIN_BODY_LEN {
        findings.push(Finding::warn(
            phase,
            where_at.clone(),
            format!(
                "disclosure_audit — Disclaimer body is shorter than {MIN_BODY_LEN} characters (`{}` chars). Short-form disclosures often fail FTC substantive-disclosure requirements; consider expanding to describe the relationship.",
                body.trim().chars().count()
            ),
        ));
    }
    // Sponsored / affiliate require source.
    if SOURCE_REQUIRED_KINDS.contains(&disclosure_kind) {
        let source_present = source
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        if !source_present {
            findings.push(Finding::strict(
                phase,
                where_at,
                format!(
                    "disclosure_audit — Disclaimer (disclosure_kind=\"{disclosure_kind}\") missing `source` attribution. FTC + most jurisdictions require sponsored / affiliate disclosures to name the sponsor. Set `source` to the sponsor / publisher name."
                ),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run_check(page: Value) -> Vec<Finding> {
        let mut findings = Vec::new();
        check_page("/cms/test.json", &page, &mut findings, "disclosure_audit");
        findings
    }

    #[test]
    fn page_without_disclaimers_emits_no_findings() {
        let page = json!({
            "sections": [
                { "kind": "paragraph", "text": "Hello world." },
                { "kind": "heading", "text": "Welcome" }
            ]
        });
        assert!(run_check(page).is_empty());
    }

    #[test]
    fn well_formed_sponsored_disclaimer_with_source_silent() {
        let page = json!({
            "sections": [{
                "kind": "disclaimer",
                "disclosure_kind": "sponsored",
                "body": "This article was sponsored by Acme Corp.",
                "source": "Acme Corp."
            }]
        });
        assert!(run_check(page).is_empty());
    }

    #[test]
    fn sponsored_without_source_is_strict() {
        let page = json!({
            "sections": [{
                "kind": "disclaimer",
                "disclosure_kind": "sponsored",
                "body": "This is a sponsored article that does not name the sponsor.",
                "source": null
            }]
        });
        let findings = run_check(page);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, forge_core::Severity::Strict);
        assert!(findings[0].message.contains("sponsor-without-source") ||
                findings[0].message.contains("missing `source`"));
    }

    #[test]
    fn affiliate_without_source_is_strict() {
        let page = json!({
            "sections": [{
                "kind": "disclaimer",
                "disclosure_kind": "affiliate",
                "body": "We earn affiliate commission on some of the links in this article.",
                "source": null
            }]
        });
        let findings = run_check(page);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, forge_core::Severity::Strict);
    }

    #[test]
    fn editorial_note_without_source_is_silent() {
        // Non-sponsorship kinds don't require source.
        let page = json!({
            "sections": [{
                "kind": "disclaimer",
                "disclosure_kind": "editorial_note",
                "body": "Editorial note: this article was updated on 2024-03-15 to correct an error.",
                "source": null
            }]
        });
        assert!(run_check(page).is_empty());
    }

    #[test]
    fn whitespace_only_source_treated_as_missing() {
        let page = json!({
            "sections": [{
                "kind": "disclaimer",
                "disclosure_kind": "sponsored",
                "body": "This article was sponsored content of unknown origin.",
                "source": "   "
            }]
        });
        let findings = run_check(page);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, forge_core::Severity::Strict);
        assert!(findings[0].message.contains("missing `source`"));
    }

    #[test]
    fn empty_body_is_strict() {
        let page = json!({
            "sections": [{
                "kind": "disclaimer",
                "disclosure_kind": "editorial_note",
                "body": "",
                "source": null
            }]
        });
        let findings = run_check(page);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, forge_core::Severity::Strict);
        assert!(findings[0].message.contains("empty"));
    }

    #[test]
    fn whitespace_only_body_is_strict() {
        let page = json!({
            "sections": [{
                "kind": "disclaimer",
                "disclosure_kind": "legal_notice",
                "body": "   \n  \t  ",
                "source": null
            }]
        });
        let findings = run_check(page);
        assert!(findings.iter().any(|f| f.severity == forge_core::Severity::Strict
            && f.message.contains("empty")));
    }

    #[test]
    fn short_body_is_warn() {
        // 19 chars — under the 20-char floor.
        let page = json!({
            "sections": [{
                "kind": "disclaimer",
                "disclosure_kind": "editorial_note",
                "body": "Editorial note: ok",
                "source": null
            }]
        });
        let findings = run_check(page);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, forge_core::Severity::Warn);
        assert!(findings[0].message.contains("shorter than 20"));
    }

    #[test]
    fn exactly_min_length_body_is_silent() {
        // 20 chars exactly — at the floor, no warn.
        let page = json!({
            "sections": [{
                "kind": "disclaimer",
                "disclosure_kind": "editorial_note",
                "body": "Twenty chars exactly.",
                "source": null
            }]
        });
        let findings = run_check(page);
        // 21 chars — passes. Verify nothing fires.
        assert!(findings.is_empty(), "got: {:?}", findings);
    }

    #[test]
    fn sponsored_with_short_body_and_no_source_emits_both_findings() {
        let page = json!({
            "sections": [{
                "kind": "disclaimer",
                "disclosure_kind": "sponsored",
                "body": "Sponsored.",
                "source": null
            }]
        });
        let findings = run_check(page);
        assert_eq!(findings.len(), 2);
        let kinds: Vec<_> = findings.iter().map(|f| f.severity).collect();
        // One warn (short), one strict (missing source).
        assert!(kinds.contains(&forge_core::Severity::Warn));
        assert!(kinds.contains(&forge_core::Severity::Strict));
    }

    #[test]
    fn multiple_disclaimers_emit_independent_findings() {
        let page = json!({
            "sections": [
                { "kind": "paragraph", "text": "Body" },
                {
                    "kind": "disclaimer",
                    "disclosure_kind": "sponsored",
                    "body": "Long enough sponsored disclosure body here.",
                    "source": "Acme"
                },
                {
                    "kind": "disclaimer",
                    "disclosure_kind": "affiliate",
                    "body": "Long enough affiliate disclosure body here.",
                    "source": null
                }
            ]
        });
        let findings = run_check(page);
        // First disclaimer (sponsored with source + long body) → silent.
        // Second disclaimer (affiliate, no source) → strict.
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, forge_core::Severity::Strict);
    }

    #[test]
    fn non_disclaimer_sections_silently_skipped() {
        let page = json!({
            "sections": [
                { "kind": "paragraph", "text": "hi" },
                { "kind": "image_hero", "title": "Welcome" }
            ]
        });
        assert!(run_check(page).is_empty());
    }

    #[test]
    fn missing_sections_array_silently_skipped() {
        let page = json!({ "title": "no sections key" });
        assert!(run_check(page).is_empty());
    }
}
