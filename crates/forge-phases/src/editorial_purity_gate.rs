//! `editorial_purity_gate` — strict refusal of SaaS-trope shapes.
//!
//! Per paul 2026-05-20 directive: "dev.prosperityclub.com looks
//! a lot like SkillShots still. you need to radically improve
//! forge so this stops happening." This phase is the gate.
//!
//! When `forge.toml [editorial_purity] enforce = true`, the
//! build REFUSES (strict findings) every SaaS-trope shape the
//! substrate has corrective editorial counterparts for. Default
//! enforce = false so existing sites don't break on the
//! introduction; tenants opt in per site by setting the flag.
//!
//! Comprehensive coverage in one phase rather than scattering
//! across multiple — operators get a single switch + one report
//! row for the gate.
//!
//! ## Tropes flagged
//!
//! `editorial-purity.saas-hero`           strict — `CmsSection::
//!   Hero` is used (use `HeroEditorial` instead). The SaaS-default
//!   centered hero is the canonical SkillShots-shape signal;
//!   editorial pages use HeroEditorial which has asymmetric layout,
//!   monospace kicker, no gradient backdrop.
//! `editorial-purity.feature-spotlight-grid` strict — `FeatureSpotlight`
//!   with 3+ columns. The 3-column icon-tile-card grid is THE
//!   SaaS-marketing trope. Use `KvPairCard` dense info panels
//!   instead.
//! `editorial-purity.stat-band`           strict — `StatBand` variant
//!   used at all (the "Numbers that compose" / 99.99% / 10M+ users
//!   trope). Editorial pages use `Sparkline` / `Histogram` / per-
//!   metric reporting instead.
//! `editorial-purity.pricing-most-popular` strict — `Pricing` with
//!   any tier marked `highlighted: true` (the green-check "MOST
//!   POPULAR" badge trope). Drop the highlight, let the operator's
//!   reader compare without the marketing nudge.
//! `editorial-purity.testimonial-card-avatar` strict — `Testimonial`
//!   with `avatar_slug` set (the fake/stock-photo testimonial card
//!   with circle-avatar trope). Use `PullQuote` editorial mark
//!   instead — left-border rule, no avatar, no card chrome.
//! `editorial-purity.centered-single-line-hero` strict — Hero or
//!   HeroEditorial title < 30 chars AND no lede AND no eyebrow.
//!   The "Welcome." / "We Build Things." monolithic-single-line
//!   trope — under-content for hero density.
//! `editorial-purity.cookie-notice-cta`   strict — `CookieNotice`
//!   with reject_label being a non-prominent secondary action.
//!   GDPR + ePrivacy compliance + editorial decency: the reject
//!   button must be as prominent as accept.
//!
//! ## forge.toml config
//!
//! ```toml
//! [editorial_purity]
//! enforce = true
//! # Optional: per-trope opt-out (use sparingly; defeats the gate).
//! # exempt = ["editorial-purity.saas-hero"]
//! ```
//!
//! Without the section the phase is silent — back-compat for
//! sites that haven't migrated yet. With it, the build refuses
//! to ship until every flagged trope is removed.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * `#[non_exhaustive]` on phase struct.
//! * Pure walk over JSON; no I/O beyond standard cms read.

use std::fs;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde_json::Value;

/// `editorial_purity_gate` phase implementation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct EditorialPurityGatePhase;

impl Phase for EditorialPurityGatePhase {
    fn name(&self) -> &'static str {
        "editorial_purity_gate"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        // Gate must be explicitly enabled per [editorial_purity]
        // enforce = true. Phase is silent otherwise — back-compat
        // for sites that haven't migrated.
        if !read_enforce_flag(&ctx.root) {
            return Ok(findings);
        }
        let exempt = read_exempt_list(&ctx.root);
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
            check_page(&path_disp, &value, &exempt, &mut findings, self.name());
        }
        Ok(findings)
    }
}

fn check_page(
    path: &str,
    page: &Value,
    exempt: &[String],
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let Some(sections) = page.get("sections").and_then(|s| s.as_array()) else {
        return;
    };
    for (idx, section) in sections.iter().enumerate() {
        let Some(tag) = section.get("kind").and_then(|v| v.as_str()) else {
            continue;
        };
        let where_at = format!("{path}#section-{idx}-{tag}");
        check_section(tag, section, &where_at, exempt, findings, phase);
    }
}

fn check_section(
    tag: &str,
    section: &Value,
    where_at: &str,
    exempt: &[String],
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let push = |kind: &str, detail: String, findings: &mut Vec<Finding>| {
        if exempt.iter().any(|e| e == kind) {
            return;
        }
        findings.push(Finding::strict(phase, where_at.to_owned(), format!("editorial_purity_gate — `{kind}` — {detail}")));
    };
    match tag {
        "hero" => {
            push(
                "editorial-purity.saas-hero",
                "`CmsSection::Hero` used — the SaaS-default centered hero is the canonical SkillShots-shape signal. Use `HeroEditorial` (asymmetric layout, monospace kicker, no gradient backdrop) instead.".to_owned(),
                findings,
            );
            check_centered_single_line(section, where_at, exempt, findings, phase, "hero");
        }
        "hero_editorial" => {
            check_centered_single_line(section, where_at, exempt, findings, phase, "hero_editorial");
        }
        "feature_spotlight" => {
            let columns = section
                .get("columns")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let items_len = section
                .get("items")
                .and_then(|v| v.as_array())
                .map(Vec::len)
                .unwrap_or(0);
            if columns >= 3 || items_len >= 3 {
                push(
                    "editorial-purity.feature-spotlight-grid",
                    format!("`FeatureSpotlight` with {} columns / {} items — the 3-column icon-tile-card grid is THE SaaS-marketing trope. Use `KvPairCard` dense info panels instead.", columns, items_len),
                    findings,
                );
            }
        }
        "stat_band" => {
            push(
                "editorial-purity.stat-band",
                "`StatBand` variant used — the \"Numbers that compose\" / 99.99% / 10M+ users trope. Editorial pages use `Sparkline` / `Histogram` / per-metric editorial reporting instead.".to_owned(),
                findings,
            );
        }
        "pricing" => {
            // Check tier-level highlighted flag.
            if let Some(tiers) = section.get("tiers").and_then(|v| v.as_array()) {
                let any_highlighted = tiers
                    .iter()
                    .any(|t| t.get("highlighted").and_then(|v| v.as_bool()).unwrap_or(false));
                if any_highlighted {
                    push(
                        "editorial-purity.pricing-most-popular",
                        "`Pricing` with a tier marked `highlighted: true` — the green-check \"MOST POPULAR\" badge trope. Drop the highlight; let the reader compare without the marketing nudge.".to_owned(),
                        findings,
                    );
                }
            }
        }
        "testimonial" => {
            let has_avatar = section
                .get("avatar_slug")
                .and_then(|v| v.as_str())
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            if has_avatar {
                push(
                    "editorial-purity.testimonial-card-avatar",
                    "`Testimonial` with `avatar_slug` set — the fake/stock-photo testimonial card with circle-avatar trope. Use `PullQuote` editorial mark instead (left-border rule, no avatar, no card chrome).".to_owned(),
                    findings,
                );
            }
        }
        "cookie_notice" => {
            // The trope: reject button is visually buried while
            // accept is the primary CTA. Reject_label being a
            // single small word ("No") signals the imbalance.
            let reject = section
                .get("reject_label")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if reject.is_empty() || reject.eq_ignore_ascii_case("no") || reject.eq_ignore_ascii_case("dismiss") {
                push(
                    "editorial-purity.cookie-notice-cta",
                    format!("`CookieNotice` reject_label=`{reject}` — GDPR + ePrivacy compliance + editorial decency requires the reject button to be AS PROMINENT as accept. Use full label like 'Decline non-essential cookies'."),
                    findings,
                );
            }
        }
        _ => {}
    }
}

/// Hero centered-single-line trope: title < 30 chars + no lede +
/// no eyebrow. Operators ship "Welcome." or "We Build Things." as
/// the entire above-fold content — under-density for a hero band.
fn check_centered_single_line(
    section: &Value,
    where_at: &str,
    exempt: &[String],
    findings: &mut Vec<Finding>,
    phase: &'static str,
    kind: &str,
) {
    let title = section
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let lede = section.get("lede").and_then(|v| v.as_str()).unwrap_or("");
    let eyebrow = section
        .get("eyebrow")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let title_chars = title.chars().count();
    if title_chars > 0 && title_chars < 30 && lede.is_empty() && eyebrow.is_empty() {
        let trope_kind = "editorial-purity.centered-single-line-hero";
        if exempt.iter().any(|e| e == trope_kind) {
            return;
        }
        findings.push(Finding::strict(
            phase,
            where_at.to_owned(),
            format!(
                "editorial_purity_gate — `{trope_kind}` — `{kind}` title is `\"{title}\"` ({title_chars} chars, no lede, no eyebrow). The monolithic-single-line hero is under-content for a hero band; either expand to a 2-3 sentence lede + monospace eyebrow OR use a different primitive for short copy."
            ),
        ));
    }
}

fn read_enforce_flag(root: &Path) -> bool {
    read_toml_string(root, "[editorial_purity]", "enforce")
        .map(|s| matches!(s.to_lowercase().as_str(), "true" | "1"))
        .unwrap_or(false)
}

fn read_exempt_list(root: &Path) -> Vec<String> {
    // Read `[editorial_purity] exempt = [...]` — array-of-strings.
    // Naive parser since we only need this specific shape; full
    // TOML parser is available in forge-core::tenant_corpus but
    // adding a dep for one config-read pulls more than it gives.
    let path = root.join("forge.toml");
    let Ok(body) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut in_section = false;
    for line in body.lines() {
        let stripped = line.split('#').next().unwrap_or("").trim();
        if stripped.is_empty() {
            continue;
        }
        if stripped.starts_with('[') {
            in_section = stripped == "[editorial_purity]";
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some(rest) = stripped.strip_prefix("exempt") {
            let value = rest.trim_start().trim_start_matches('=').trim();
            if let Some(inner) = value.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                return inner
                    .split(',')
                    .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_owned())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
    }
    Vec::new()
}

fn read_toml_string(root: &Path, section: &str, key: &str) -> Option<String> {
    let cfg_path = root.join("forge.toml");
    let body = fs::read_to_string(&cfg_path).ok()?;
    let mut in_section = false;
    for raw in body.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            in_section = line == section;
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some(rest) = line.strip_prefix(key) {
            let v = rest.trim_start().trim_start_matches('=').trim();
            let unquoted = v.trim_matches('"').trim_matches('\'');
            return Some(unquoted.to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run_check_enforced(page: Value) -> Vec<Finding> {
        let mut findings = Vec::new();
        check_page(
            "/cms/test.json",
            &page,
            &[],
            &mut findings,
            "editorial_purity_gate",
        );
        findings
    }

    #[test]
    fn editorial_page_with_no_tropes_emits_no_findings() {
        let page = json!({
            "sections": [
                {
                    "kind": "hero_editorial",
                    "title": "A substantial, opinionated, multi-clause editorial hero title.",
                    "lede": "With an actual lede explaining what this is about.",
                    "eyebrow": "Editorial",
                    "kicker": "monospace"
                },
                { "kind": "paragraph", "text": "Body" },
                { "kind": "kv_pair", "items": [{"label": "X", "value": "Y"}] },
                { "kind": "pull_quote", "body": "A real editorial mark." }
            ]
        });
        assert!(run_check_enforced(page).is_empty());
    }

    #[test]
    fn saas_hero_variant_fires_strict() {
        let page = json!({
            "sections": [{
                "kind": "hero",
                "title": "A reasonably substantive hero title that's not also short.",
                "lede": "Some lede text."
            }]
        });
        let findings = run_check_enforced(page);
        assert!(findings.iter().any(|f| f.message.contains("editorial-purity.saas-hero")));
    }

    #[test]
    fn centered_single_line_hero_fires_strict() {
        let page = json!({
            "sections": [{
                "kind": "hero_editorial",
                "title": "Welcome.",
                "lede": null,
                "eyebrow": null
            }]
        });
        let findings = run_check_enforced(page);
        assert!(findings.iter().any(|f| f.message.contains("centered-single-line-hero")
            && f.message.contains("Welcome.")));
    }

    #[test]
    fn feature_spotlight_3_columns_fires_strict() {
        let page = json!({
            "sections": [{
                "kind": "feature_spotlight",
                "columns": 3,
                "items": [
                    {"title": "Fast"},
                    {"title": "Simple"},
                    {"title": "Cheap"}
                ]
            }]
        });
        let findings = run_check_enforced(page);
        assert!(findings.iter().any(|f| f.message.contains("feature-spotlight-grid")));
    }

    #[test]
    fn feature_spotlight_2_columns_silent() {
        let page = json!({
            "sections": [{
                "kind": "feature_spotlight",
                "columns": 2,
                "items": [
                    {"title": "Fast"},
                    {"title": "Simple"}
                ]
            }]
        });
        let findings = run_check_enforced(page);
        assert!(!findings.iter().any(|f| f.message.contains("feature-spotlight-grid")));
    }

    #[test]
    fn stat_band_fires_strict() {
        let page = json!({
            "sections": [{
                "kind": "stat_band",
                "heading": "Numbers that compose",
                "items": [{"value": "10M+", "label": "users"}]
            }]
        });
        let findings = run_check_enforced(page);
        assert!(findings.iter().any(|f| f.message.contains("editorial-purity.stat-band")));
    }

    #[test]
    fn pricing_with_highlighted_tier_fires_strict() {
        let page = json!({
            "sections": [{
                "kind": "pricing",
                "tiers": [
                    {"name": "Free", "highlighted": false},
                    {"name": "Pro", "highlighted": true}
                ]
            }]
        });
        let findings = run_check_enforced(page);
        assert!(findings.iter().any(|f| f.message.contains("pricing-most-popular")));
    }

    #[test]
    fn pricing_without_highlighted_tier_silent() {
        let page = json!({
            "sections": [{
                "kind": "pricing",
                "tiers": [
                    {"name": "Free", "highlighted": false},
                    {"name": "Pro", "highlighted": false}
                ]
            }]
        });
        let findings = run_check_enforced(page);
        assert!(!findings.iter().any(|f| f.message.contains("pricing-most-popular")));
    }

    #[test]
    fn testimonial_with_avatar_fires_strict() {
        let page = json!({
            "sections": [{
                "kind": "testimonial",
                "body": "Great product!",
                "attribution": "Jane Doe",
                "avatar_slug": "jane"
            }]
        });
        let findings = run_check_enforced(page);
        assert!(findings.iter().any(|f| f.message.contains("testimonial-card-avatar")));
    }

    #[test]
    fn testimonial_without_avatar_silent() {
        let page = json!({
            "sections": [{
                "kind": "testimonial",
                "body": "Great product!",
                "attribution": "Jane Doe",
                "avatar_slug": ""
            }]
        });
        let findings = run_check_enforced(page);
        assert!(!findings.iter().any(|f| f.message.contains("testimonial-card-avatar")));
    }

    #[test]
    fn cookie_notice_with_buried_reject_fires_strict() {
        for short_reject in ["", "no", "No", "Dismiss"] {
            let page = json!({
                "sections": [{
                    "kind": "cookie_notice",
                    "text": "We use cookies",
                    "accept_label": "Accept all cookies",
                    "reject_label": short_reject
                }]
            });
            let findings = run_check_enforced(page);
            assert!(
                findings.iter().any(|f| f.message.contains("cookie-notice-cta")),
                "should fire on short reject_label=\"{short_reject}\""
            );
        }
    }

    #[test]
    fn cookie_notice_with_full_reject_silent() {
        let page = json!({
            "sections": [{
                "kind": "cookie_notice",
                "text": "We use cookies",
                "accept_label": "Accept all cookies",
                "reject_label": "Decline non-essential cookies"
            }]
        });
        let findings = run_check_enforced(page);
        assert!(!findings.iter().any(|f| f.message.contains("cookie-notice-cta")));
    }

    #[test]
    fn multiple_tropes_per_page_emit_independent_findings() {
        let page = json!({
            "sections": [
                { "kind": "hero", "title": "We Build.", "lede": null, "eyebrow": null },
                { "kind": "feature_spotlight", "columns": 3, "items": [{},{},{}] },
                { "kind": "stat_band", "items": [] }
            ]
        });
        let findings = run_check_enforced(page);
        let kinds: Vec<&str> = findings.iter().map(|f| f.message.as_str()).collect();
        assert!(kinds.iter().any(|k| k.contains("saas-hero")));
        assert!(kinds.iter().any(|k| k.contains("centered-single-line-hero")));
        assert!(kinds.iter().any(|k| k.contains("feature-spotlight-grid")));
        assert!(kinds.iter().any(|k| k.contains("stat-band")));
    }

    #[test]
    fn exempt_list_suppresses_specific_kind() {
        let mut findings = Vec::new();
        check_page(
            "/cms/test.json",
            &json!({
                "sections": [
                    { "kind": "stat_band", "items": [] }
                ]
            }),
            &["editorial-purity.stat-band".to_owned()],
            &mut findings,
            "editorial_purity_gate",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn missing_sections_array_silent() {
        let page = json!({ "title": "no sections" });
        assert!(run_check_enforced(page).is_empty());
    }
}
