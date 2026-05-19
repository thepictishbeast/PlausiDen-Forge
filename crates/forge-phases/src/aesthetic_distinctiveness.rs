//! `aesthetic_distinctiveness` — flag pages that fall into well-known
//! SaaS-marketing slop patterns. Detects compositional shapes that
//! signal "generic template" rather than considered authoring.
//!
//! Motivation: per the consumer-shaped-substrate diagnosis, the
//! substrate has no pressure toward distinctness — phases check
//! correctness, not completeness or originality. This phase is the
//! density / distinctiveness gate the substrate was missing.
//!
//! ## What flags as a finding
//!
//! * **`centered_single_word_hero`** — `image_hero` whose title is
//!   ≤ 4 words, no eyebrow, no lede. The "Welcome." trope.
//! * **`monotonous_feature_grid`** — `feature_spotlight` with 3+
//!   columns AND every item using the same icon slug class
//!   (same character of icon shape).
//! * **`fake_testimonials`** — two or more `testimonial` blocks
//!   where attribution names match a fictional-stub pattern
//!   ("J. K.", "fictional pilot team", role contains "fictional").
//! * **`most_popular_badge`** — `pricing` tier with `highlighted:
//!   true` AND tier name in {"Pro", "Plus", "Team", "Business"}
//!   (the green-check pricing trope).
//! * **`numbers_that_compose`** — `stat_band` heading contains the
//!   exact phrase "Numbers that" or "by the numbers".
//! * **`sparse_page`** — total non-decorative sections < 5.
//! * **`scaffold_only`** — page contains only hero-class sections
//!   plus a single `call_to_action`. No editorial body.
//!
//! ## Severity
//!
//! `Warn` by default — these are aesthetic signals, not correctness
//! gates. Strict mode (`forge.toml [aesthetic_distinctiveness]
//! strict = true`) promotes to `Strict` so the slop dictionary
//! becomes a real gate.
//!
//! ## Slop dictionary
//!
//! v1 ships with 7 signatures inline. The structure is open for
//! expansion via cms-side overrides + per-tenant corpora (queued
//! as #109).

use std::fs;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde_json::Value;

/// `aesthetic_distinctiveness` phase.
#[derive(Debug, Default)]
pub struct AestheticDistinctivenessPhase;

impl Phase for AestheticDistinctivenessPhase {
    fn name(&self) -> &'static str {
        "aesthetic_distinctiveness"
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
            check_page(&path, &value, &mut findings, self.name());
        }
        Ok(findings)
    }
}

fn check_page(path: &Path, page: &Value, findings: &mut Vec<Finding>, phase: &'static str) {
    let Some(sections) = page.get("sections").and_then(|s| s.as_array()) else {
        return;
    };
    let path_disp = path.display().to_string();

    check_sparse_page(sections, &path_disp, findings, phase);
    check_scaffold_only(sections, &path_disp, findings, phase);
    check_corporate_jargon(sections, &path_disp, findings, phase);

    for (index, section) in sections.iter().enumerate() {
        let kind = section.get("kind").and_then(|k| k.as_str()).unwrap_or("");
        let where_at = format!("{path_disp}#section-{index}-{kind}");
        match kind {
            "image_hero" => {
                check_centered_single_word_hero(section, &where_at, findings, phase);
                check_vague_eyebrow(section, &where_at, findings, phase);
            }
            "split_hero" | "call_to_action" => {
                check_vague_eyebrow(section, &where_at, findings, phase);
            }
            "feature_spotlight" => {
                check_monotonous_feature_grid(section, &where_at, findings, phase);
            }
            "pricing" => check_most_popular_badge(section, &where_at, findings, phase),
            "stat_band" => check_numbers_that_compose(section, &where_at, findings, phase),
            _ => {}
        }
    }

    check_fake_testimonials(sections, &path_disp, findings, phase);
    check_placeholder_email(sections, &path_disp, findings, phase);
}

fn check_sparse_page(
    sections: &[Value],
    path: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let content_sections = sections
        .iter()
        .filter(|s| {
            let k = s.get("kind").and_then(|k| k.as_str()).unwrap_or("");
            !matches!(k, "divider" | "spacer" | "announcement_bar")
        })
        .count();
    if content_sections < 5 {
        findings.push(Finding::warn(
            phase,
            path.to_owned(),
            format!(
                "sparse_page: only {content_sections} content section(s); marketing landings should compose at least 5 distinct content blocks (heroes, body, comparison, pricing, CTA, etc.)"
            ),
        ));
    }
}

fn check_scaffold_only(
    sections: &[Value],
    path: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let mut has_editorial = false;
    let mut has_hero = false;
    let mut has_cta = false;
    for s in sections {
        let k = s.get("kind").and_then(|k| k.as_str()).unwrap_or("");
        match k {
            "image_hero" | "split_hero" | "hero" => has_hero = true,
            "call_to_action" => has_cta = true,
            "paragraph" | "heading" | "pull_quote" | "kv_pair" | "comparison" | "code"
            | "faq" | "steps" | "feature_spotlight" | "alert" | "roadmap" | "logo_cloud"
            | "timeline" | "stat_band" | "marquee" | "auth_card" | "mfa_prompt"
            | "crucible_widget" | "form" | "composer" | "card_feed" => {
                has_editorial = true;
            }
            _ => {}
        }
    }
    if has_hero && has_cta && !has_editorial {
        findings.push(Finding::warn(
            phase,
            path.to_owned(),
            "scaffold_only: page is hero(s) + CTA with no editorial body — looks like an unfilled template",
        ));
    }
}

fn check_centered_single_word_hero(
    section: &Value,
    path: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let title = section.get("title").and_then(|t| t.as_str()).unwrap_or("");
    let word_count = title.split_whitespace().count();
    let has_eyebrow = section
        .get("eyebrow")
        .map(|e| e.as_str().is_some_and(|s| !s.trim().is_empty()))
        .unwrap_or(false);
    let has_lede = section
        .get("lede")
        .map(|l| l.as_str().is_some_and(|s| !s.trim().is_empty()))
        .unwrap_or(false);
    if word_count <= 4 && !has_eyebrow && !has_lede {
        findings.push(Finding::warn(
            phase,
            path.to_owned(),
            format!(
                "centered_single_word_hero: title \"{title}\" is {word_count} word(s), no eyebrow, no lede — classic trope hero; add an eyebrow chip or substantive lede"
            ),
        ));
    }
}

fn check_monotonous_feature_grid(
    section: &Value,
    path: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let columns = section
        .get("columns")
        .and_then(|c| c.as_u64())
        .unwrap_or(3);
    if columns < 3 {
        return;
    }
    let Some(items) = section.get("items").and_then(|i| i.as_array()) else {
        return;
    };
    if items.len() < 3 {
        return;
    }
    let mut slug_set = std::collections::BTreeSet::new();
    for item in items {
        let slug = item
            .get("icon_slug")
            .and_then(|s| s.as_str())
            .unwrap_or("");
        slug_set.insert(slug);
    }
    if slug_set.len() <= 1 {
        findings.push(Finding::warn(
            phase,
            path.to_owned(),
            format!(
                "monotonous_feature_grid: {columns}-column feature_spotlight has {} unique icon_slug(s) across {} items — varying the iconography breaks the visual repeat",
                slug_set.len(),
                items.len()
            ),
        ));
    }
}

fn check_most_popular_badge(
    section: &Value,
    path: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let Some(tiers) = section.get("tiers").and_then(|t| t.as_array()) else {
        return;
    };
    for tier in tiers {
        let highlighted = tier
            .get("highlighted")
            .and_then(|h| h.as_bool())
            .unwrap_or(false);
        if !highlighted {
            continue;
        }
        let name = tier.get("name").and_then(|n| n.as_str()).unwrap_or("");
        if matches!(name, "Pro" | "Plus" | "Team" | "Business" | "Standard") {
            findings.push(Finding::warn(
                phase,
                path.to_owned(),
                format!(
                    "most_popular_badge: pricing tier \"{name}\" is highlighted — this is the green-check pricing trope; consider distinguishing the middle tier by value not by badge"
                ),
            ));
        }
    }
}

fn check_numbers_that_compose(
    section: &Value,
    path: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let heading = section
        .get("heading")
        .and_then(|h| h.as_str())
        .unwrap_or("");
    let lower = heading.to_lowercase();
    if lower.contains("numbers that")
        || lower.contains("by the numbers")
        || lower.contains("stats that matter")
    {
        findings.push(Finding::warn(
            phase,
            path.to_owned(),
            format!(
                "numbers_that_compose: stat_band heading \"{heading}\" uses a SaaS-trope phrase; substitute a concrete claim about what the numbers prove"
            ),
        ));
    }
}

/// SaaS-marketing jargon that almost always reads as filler. Drawn
/// from corpora of agency / SaaS / consultancy landings. Extend over
/// time as new clichés crystallize.
const JARGON_PHRASES: &[&str] = &[
    "best-in-class",
    "best of breed",
    "best-of-breed",
    "cutting-edge",
    "cutting edge",
    "next-generation",
    "next generation",
    "next-gen",
    "industry-leading",
    "industry leading",
    "world-class",
    "world class",
    "game-changer",
    "game-changing",
    "synergy",
    "synergies",
    "leverage our",
    "robust solution",
    "frictionless",
    "seamless integration",
    "thought leader",
    "thought leadership",
    "mission-critical",
    "value-add",
    "value add",
    "low-hanging fruit",
    "move the needle",
    "circle back",
    "ecosystem of",
    "holistic approach",
    "deep dive",
    "ai-powered",
    "ai powered",
    "ai-driven",
    "ai driven",
    "blockchain-powered",
    "future-proof",
    "future proof",
    "paradigm shift",
    "core competency",
    "value proposition",
    "scalable solution",
    "turnkey solution",
];

/// Eyebrow text that adds zero information — "Beta", "New", "Latest",
/// etc. without further context. Cheap signal that gets reused
/// because it costs nothing to type.
const VAGUE_EYEBROW_LITERALS: &[&str] = &[
    "beta", "new", "alpha", "latest", "introducing", "coming soon",
    "now available", "announcement", "tba", "tbd",
];

fn check_corporate_jargon(
    sections: &[Value],
    path: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let mut hits: Vec<String> = Vec::new();
    collect_text(sections, &mut |t| {
        let lower = t.to_lowercase();
        for phrase in JARGON_PHRASES {
            if lower.contains(phrase) {
                hits.push((*phrase).to_owned());
            }
        }
    });
    if hits.is_empty() {
        return;
    }
    hits.sort();
    hits.dedup();
    findings.push(Finding::warn(
        phase,
        path.to_owned(),
        format!(
            "corporate_jargon: page text contains SaaS-cliché phrase(s) [{}] — these read as filler; substitute a concrete claim that names a real thing",
            hits.join(", ")
        ),
    ));
}

fn check_vague_eyebrow(
    section: &Value,
    path: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let eyebrow = section.get("eyebrow").and_then(|e| e.as_str()).unwrap_or("");
    let trimmed = eyebrow.trim().to_lowercase();
    if trimmed.is_empty() {
        return;
    }
    for vague in VAGUE_EYEBROW_LITERALS {
        if trimmed == *vague {
            findings.push(Finding::warn(
                phase,
                path.to_owned(),
                format!(
                    "vague_eyebrow: eyebrow \"{eyebrow}\" carries no information beyond a status label; pair with a version, primitive count, or named release to add density"
                ),
            ));
            return;
        }
    }
}

fn check_placeholder_email(
    sections: &[Value],
    path: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    // Form-input placeholders anywhere in the section tree are
    // legitimate UX affordances (auth_card.methods[].placeholder,
    // newsletter_signup.placeholder, form fields nested in
    // composites, etc.). Walk the whole tree and collect every
    // value-of-a-"placeholder"-key. A match against one of those
    // strings is the UX affordance, not slop.
    let needles = [
        "you@yourcompany.com",
        "name@example.com",
        "user@example.com",
        "your@email.com",
    ];
    let mut legit: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    fn collect_placeholders(v: &Value, out: &mut std::collections::BTreeSet<String>) {
        match v {
            Value::Object(obj) => {
                for (k, val) in obj {
                    if k == "placeholder" {
                        if let Some(s) = val.as_str() {
                            out.insert(s.to_lowercase());
                        }
                    }
                    collect_placeholders(val, out);
                }
            }
            Value::Array(arr) => {
                for item in arr {
                    collect_placeholders(item, out);
                }
            }
            _ => {}
        }
    }
    for section in sections {
        collect_placeholders(section, &mut legit);
    }
    let mut hits: Vec<String> = Vec::new();
    collect_text(sections, &mut |t| {
        let lower = t.to_lowercase();
        for needle in &needles {
            if lower.contains(needle) && !legit.iter().any(|p| p.contains(needle)) {
                hits.push((*needle).to_owned());
            }
        }
    });
    if hits.is_empty() {
        return;
    }
    hits.sort();
    hits.dedup();
    findings.push(Finding::warn(
        phase,
        path.to_owned(),
        format!(
            "placeholder_email: non-input copy contains generic email placeholder [{}] — these read as scaffolding; replace with a specific example or remove",
            hits.join(", ")
        ),
    ));
}

/// Walk every string-valued field across the JSON tree of the
/// given sections and call `visit` with each. Used by detectors
/// that look at full-page text rather than per-section structure.
fn collect_text<F: FnMut(&str)>(sections: &[Value], visit: &mut F) {
    fn walk<F: FnMut(&str)>(v: &Value, visit: &mut F) {
        match v {
            Value::String(s) => visit(s),
            Value::Array(arr) => {
                for item in arr {
                    walk(item, visit);
                }
            }
            Value::Object(obj) => {
                for (_, val) in obj {
                    walk(val, visit);
                }
            }
            _ => {}
        }
    }
    for s in sections {
        walk(s, visit);
    }
}

fn check_fake_testimonials(
    sections: &[Value],
    path: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let testimonials: Vec<&Value> = sections
        .iter()
        .filter(|s| s.get("kind").and_then(|k| k.as_str()) == Some("testimonial"))
        .collect();
    if testimonials.is_empty() {
        return;
    }
    for t in &testimonials {
        let role = t.get("role").and_then(|r| r.as_str()).unwrap_or("");
        let attribution = t
            .get("attribution")
            .and_then(|a| a.as_str())
            .unwrap_or("");
        let role_lower = role.to_lowercase();
        if role_lower.contains("fictional")
            || role_lower.contains("placeholder")
            || attribution.len() <= 4
        {
            findings.push(Finding::warn(
                phase,
                path.to_owned(),
                format!(
                    "fake_testimonials: testimonial attribution \"{attribution}\" / role \"{role}\" reads as a stub — drop or replace with a real quote, or convert to a pull_quote (which doesn't claim attribution)"
                ),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sparse_page_under_5_sections_warns() {
        let page = json!({
            "sections": [
                {"kind": "image_hero", "title": "Hi"},
                {"kind": "call_to_action", "title": "Go"}
            ]
        });
        let mut findings = vec![];
        check_page(Path::new("test.json"), &page, &mut findings, "aesthetic_distinctiveness");
        assert!(findings.iter().any(|f| f.message.contains("sparse_page")));
    }

    #[test]
    fn scaffold_only_warns_with_no_editorial() {
        let page = json!({
            "sections": [
                {"kind": "image_hero", "title": "Big Title"},
                {"kind": "split_hero", "title": "Another"},
                {"kind": "call_to_action", "title": "Go", "cta": {"label": "X", "href": "/", "data_backend": "x"}}
            ]
        });
        let mut findings = vec![];
        check_page(Path::new("test.json"), &page, &mut findings, "aesthetic_distinctiveness");
        assert!(findings.iter().any(|f| f.message.contains("scaffold_only")));
    }

    #[test]
    fn single_word_hero_warns() {
        let section = json!({"kind": "image_hero", "title": "Welcome."});
        let mut findings = vec![];
        check_centered_single_word_hero(&section, "test", &mut findings, "aesthetic_distinctiveness");
        assert!(findings.iter().any(|f| f.message.contains("centered_single_word_hero")));
    }

    #[test]
    fn substantive_hero_does_not_warn() {
        let section = json!({
            "kind": "image_hero",
            "eyebrow": "Beta · 0.18",
            "title": "A build platform that outlives its dependencies.",
            "lede": "Typed contracts at every boundary."
        });
        let mut findings = vec![];
        check_centered_single_word_hero(&section, "test", &mut findings, "aesthetic_distinctiveness");
        assert!(findings.is_empty());
    }

    #[test]
    fn monotonous_grid_warns() {
        let section = json!({
            "kind": "feature_spotlight",
            "columns": 3,
            "items": [
                {"icon_slug": "check", "title": "A", "body": "..."},
                {"icon_slug": "check", "title": "B", "body": "..."},
                {"icon_slug": "check", "title": "C", "body": "..."}
            ]
        });
        let mut findings = vec![];
        check_monotonous_feature_grid(&section, "test", &mut findings, "aesthetic_distinctiveness");
        assert!(findings.iter().any(|f| f.message.contains("monotonous_feature_grid")));
    }

    #[test]
    fn varied_grid_does_not_warn() {
        let section = json!({
            "kind": "feature_spotlight",
            "columns": 3,
            "items": [
                {"icon_slug": "terminal", "title": "A", "body": "..."},
                {"icon_slug": "code", "title": "B", "body": "..."},
                {"icon_slug": "globe", "title": "C", "body": "..."}
            ]
        });
        let mut findings = vec![];
        check_monotonous_feature_grid(&section, "test", &mut findings, "aesthetic_distinctiveness");
        assert!(findings.is_empty());
    }

    #[test]
    fn most_popular_badge_warns() {
        let section = json!({
            "kind": "pricing",
            "tiers": [
                {"name": "Solo", "price": "$0", "highlighted": false},
                {"name": "Pro", "price": "$10", "highlighted": true}
            ]
        });
        let mut findings = vec![];
        check_most_popular_badge(&section, "test", &mut findings, "aesthetic_distinctiveness");
        assert!(findings.iter().any(|f| f.message.contains("most_popular_badge")));
    }

    #[test]
    fn numbers_that_compose_warns() {
        let section = json!({
            "kind": "stat_band",
            "heading": "Numbers that compose"
        });
        let mut findings = vec![];
        check_numbers_that_compose(&section, "test", &mut findings, "aesthetic_distinctiveness");
        assert!(findings.iter().any(|f| f.message.contains("numbers_that_compose")));
    }

    #[test]
    fn corporate_jargon_warns_on_known_phrases() {
        let sections = vec![
            json!({"kind": "paragraph", "text": "Our best-in-class platform delivers a seamless integration."}),
        ];
        let mut findings = vec![];
        check_corporate_jargon(&sections, "test", &mut findings, "aesthetic_distinctiveness");
        let msg = &findings[0].message;
        assert!(msg.contains("corporate_jargon"));
        assert!(msg.contains("best-in-class"));
        assert!(msg.contains("seamless integration"));
    }

    #[test]
    fn corporate_jargon_does_not_warn_on_clean_prose() {
        let sections = vec![
            json!({"kind": "paragraph", "text": "Typed contracts. Audited at every commit. Reproducible builds."}),
        ];
        let mut findings = vec![];
        check_corporate_jargon(&sections, "test", &mut findings, "aesthetic_distinctiveness");
        assert!(findings.is_empty());
    }

    #[test]
    fn vague_eyebrow_warns() {
        let section = json!({"kind": "image_hero", "eyebrow": "Beta", "title": "Hello world"});
        let mut findings = vec![];
        check_vague_eyebrow(&section, "test", &mut findings, "aesthetic_distinctiveness");
        assert!(findings.iter().any(|f| f.message.contains("vague_eyebrow")));
    }

    #[test]
    fn substantive_eyebrow_does_not_warn() {
        let section = json!({
            "kind": "image_hero",
            "eyebrow": "Forge 0.18 · 125 Loom primitives",
            "title": "Hello world"
        });
        let mut findings = vec![];
        check_vague_eyebrow(&section, "test", &mut findings, "aesthetic_distinctiveness");
        assert!(findings.is_empty());
    }

    #[test]
    fn placeholder_email_warns_in_body_text() {
        // The hit is in `text`, NOT in `placeholder` — flagged.
        let sections = vec![
            json!({"kind": "paragraph", "text": "Sign up at you@yourcompany.com for updates."}),
        ];
        let mut findings = vec![];
        check_placeholder_email(&sections, "test", &mut findings, "aesthetic_distinctiveness");
        assert!(findings.iter().any(|f| f.message.contains("placeholder_email")));
    }

    #[test]
    fn placeholder_email_skips_legit_input_placeholder() {
        // The hit IS the input placeholder — legitimate UX, not flagged.
        let sections = vec![
            json!({"kind": "newsletter_signup", "placeholder": "you@yourcompany.com"}),
        ];
        let mut findings = vec![];
        check_placeholder_email(&sections, "test", &mut findings, "aesthetic_distinctiveness");
        assert!(findings.is_empty());
    }

    #[test]
    fn collect_text_walks_nested_json() {
        let sections = vec![
            json!({"kind": "x", "items": [{"key": "deep", "value": "synergy"}]}),
        ];
        let mut hits = 0;
        collect_text(&sections, &mut |t| {
            if t.contains("synergy") {
                hits += 1;
            }
        });
        assert_eq!(hits, 1);
    }
}
