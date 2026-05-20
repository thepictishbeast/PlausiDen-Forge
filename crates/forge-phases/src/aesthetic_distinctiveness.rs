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

use forge_core::tenant_corpus::TenantCorpus;
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
        // Layer tenant-corpus extensions on top of the baseline
        // per [[per-tenant-corpora-doctrine]] / commit 534f02c.
        let tenant = TenantCorpus::load(&ctx.root);
        let tenant_extras: Vec<&str> = tenant
            .as_ref()
            .map(|t| t.extra_jargon.iter().map(String::as_str).collect())
            .unwrap_or_default();
        let tenant_suppress: Vec<&str> = tenant
            .as_ref()
            .map(|t| t.suppress_jargon.iter().map(String::as_str).collect())
            .unwrap_or_default();
        // Operator-typo guard: a suppress entry that doesn't match
        // any baseline phrase emits a warn finding so the operator
        // knows their suppression is dead.
        for sup in &tenant_suppress {
            if !JARGON_PHRASES.contains(sup) {
                findings.push(Finding::warn(
                    self.name(),
                    "forge.toml".to_owned(),
                    format!(
                        "tenant_corpus.suppress_jargon — entry `{sup}` does not match any baseline jargon phrase; check for a typo, or remove the entry."
                    ),
                ));
            }
        }
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
            check_page(
                &path,
                &value,
                &tenant_extras,
                &tenant_suppress,
                &mut findings,
                self.name(),
            );
        }
        Ok(findings)
    }
}

fn check_page(
    path: &Path,
    page: &Value,
    tenant_extras: &[&str],
    tenant_suppress: &[&str],
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let Some(sections) = page.get("sections").and_then(|s| s.as_array()) else {
        return;
    };
    let path_disp = path.display().to_string();

    check_sparse_page(sections, &path_disp, findings, phase);
    check_scaffold_only(sections, &path_disp, findings, phase);
    check_corporate_jargon(
        sections,
        &path_disp,
        tenant_extras,
        tenant_suppress,
        findings,
        phase,
    );

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
    check_vague_cta_label(sections, &path_disp, findings, phase);
    check_roadmap_vagueness(sections, &path_disp, findings, phase);
    check_image_desert(sections, &path_disp, findings, phase);
    check_short_paragraph_dominance(sections, &path_disp, findings, phase);
    check_adjacent_section_repetition(sections, &path_disp, findings, phase);
    check_emoji_in_body(sections, &path_disp, findings, phase);
    check_identical_section_text(sections, &path_disp, findings, phase);
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
            "paragraph" | "heading" | "pull_quote" | "kv_pair" | "comparison" | "code" | "faq"
            | "steps" | "feature_spotlight" | "alert" | "roadmap" | "logo_cloud" | "timeline"
            | "stat_band" | "marquee" | "auth_card" | "mfa_prompt" | "crucible_widget" | "form"
            | "composer" | "card_feed" => {
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
    let columns = section.get("columns").and_then(|c| c.as_u64()).unwrap_or(3);
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
        let slug = item.get("icon_slug").and_then(|s| s.as_str()).unwrap_or("");
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
    // 2026-05-20 expansion: marketing-amplifier verbs that promise
    // transformation without naming what is being transformed.
    "supercharge",
    "supercharged",
    "unleash",
    "revolutionize",
    "revolutionary",
    "reimagine",
    "reimagined",
    "transform your",
    "elevate your",
    "empower your team",
    "ignite your",
    "unlock the power",
    // 2026-05-20 expansion: "future of X" framings — vague aspirational
    // claim with no falsifiable content.
    "the future of work",
    "the future of business",
    "the future of finance",
    "the future of software",
    "the future of ai",
    // 2026-05-20 expansion: vague superlatives + reach claims that
    // gesture at scale without naming any verifiable instance.
    "trusted by leading",
    "the leader in",
    "the most advanced",
    "unparalleled",
    "best-of-class",
    // 2026-05-20 expansion: AI-generated-copy tells. Phrases that
    // appear disproportionately often in LLM-generated marketing
    // copy. Human marketers use these too — but at much lower
    // frequency. Hitting one signals "consider whether the operator
    // read what they shipped"; hitting three or more is a strong
    // signal the page was bulk-generated without curation.
    "in today's fast-paced",
    "in today's fast paced",
    "in today's digital age",
    "in the digital age",
    "in the modern era",
    "harness the power of",
    "delve into",
    "delves into",
    "navigate the complexities",
    "navigating the complexities",
    "stand the test of time",
    "stands the test of time",
    "at the forefront of",
    "the perfect blend of",
    "blend seamlessly",
    "a treasure trove of",
    "a wealth of information",
    "embark on a journey",
    "embark on this journey",
    "without further ado",
    "let's dive in",
    "let's dive into",
    "dive deep into",
    "in a nutshell",
    "needless to say",
    "the rise of",
    "in conclusion,",
    "in summary,",
    // 2026-05-20 expansion: "transform" + "journey" + "ecosystem"
    // bingo board — words that compose into marketing fog more
    // often than they describe anything specific.
    "transformative journey",
    "your journey",
    "comprehensive ecosystem",
    "robust ecosystem",
    "thriving ecosystem",
    "vibrant ecosystem",
    "engage with our",
];

/// Eyebrow text that adds zero information — "Beta", "New", "Latest",
/// etc. without further context. Cheap signal that gets reused
/// because it costs nothing to type.
const VAGUE_EYEBROW_LITERALS: &[&str] = &[
    "beta",
    "new",
    "alpha",
    "latest",
    "introducing",
    "coming soon",
    "now available",
    "announcement",
    "tba",
    "tbd",
];

/// Classify a slop phrase into a category so the finding message
/// tells the operator WHAT kind of slop the phrase exemplifies, not
/// just that it matched some phrase list.
///
/// Categories:
/// * `superlative` — "best-in-class" / "world-class" / "the most
///   advanced" / etc. Reach claims without falsifiable content.
/// * `amplifier` — "supercharge" / "transform your" / "elevate" /
///   marketing-verb empty-promise vocabulary.
/// * `future-of` — "the future of work" / etc. Vague aspirational
///   framings.
/// * `ai-buzzword` — "ai-powered" / "ai-driven" / "blockchain-powered".
/// * `business-jargon` — "synergy" / "low-hanging fruit" / classic
///   corporate-speak filler.
fn classify_jargon(phrase: &str) -> &'static str {
    // Match category by recognized roots; falls back to
    // "business-jargon" for the legacy bucket.
    if phrase.starts_with("the future of") {
        return "future-of";
    }
    if phrase.starts_with("ai-") || phrase.starts_with("ai ") || phrase == "blockchain-powered" {
        return "ai-buzzword";
    }
    let amplifier = matches!(
        phrase,
        "supercharge"
            | "supercharged"
            | "unleash"
            | "revolutionize"
            | "revolutionary"
            | "reimagine"
            | "reimagined"
            | "transform your"
            | "elevate your"
            | "empower your team"
            | "ignite your"
            | "unlock the power"
            | "game-changer"
            | "game-changing"
            | "paradigm shift"
    );
    if amplifier {
        return "amplifier";
    }
    let superlative = matches!(
        phrase,
        "best-in-class"
            | "best of breed"
            | "best-of-breed"
            | "best-of-class"
            | "world-class"
            | "world class"
            | "cutting-edge"
            | "cutting edge"
            | "next-generation"
            | "next generation"
            | "next-gen"
            | "industry-leading"
            | "industry leading"
            | "trusted by leading"
            | "the leader in"
            | "the most advanced"
            | "unparalleled"
            | "future-proof"
            | "future proof"
    );
    if superlative {
        return "superlative";
    }
    "business-jargon"
}

fn check_corporate_jargon(
    sections: &[Value],
    path: &str,
    tenant_extras: &[&str],
    tenant_suppress: &[&str],
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let mut hits: Vec<String> = Vec::new();
    collect_text(sections, &mut |t| {
        let lower = t.to_lowercase();
        // Baseline phrases, minus tenant suppress.
        for phrase in JARGON_PHRASES {
            if tenant_suppress.contains(phrase) {
                continue;
            }
            if lower.contains(phrase) {
                hits.push((*phrase).to_owned());
            }
        }
        // Tenant extra phrases (additive). Matched lowercase
        // against the lowered body text — same shape as the
        // baseline scan.
        for phrase in tenant_extras {
            let lower_phrase = phrase.to_lowercase();
            if lower.contains(&lower_phrase) {
                hits.push(phrase.to_string());
            }
        }
    });
    if hits.is_empty() {
        return;
    }
    hits.sort();
    hits.dedup();
    // Bucket hits by category so the finding surfaces which kind(s)
    // of slop are present, not just an undifferentiated list.
    let mut by_cat: std::collections::BTreeMap<&'static str, Vec<String>> =
        std::collections::BTreeMap::new();
    for h in &hits {
        by_cat
            .entry(classify_jargon(h))
            .or_default()
            .push(h.clone());
    }
    let cat_summary: Vec<String> = by_cat
        .iter()
        .map(|(cat, phrases)| format!("{cat}: [{}]", phrases.join(", ")))
        .collect();
    findings.push(
        Finding::warn(
            phase,
            path.to_owned(),
            format!(
                "corporate_jargon: page text contains {} SaaS-cliché phrase(s) across {} categor{}: {}. These read as filler — substitute concrete claims that name a real thing the operator can point to.",
                hits.len(),
                by_cat.len(),
                if by_cat.len() == 1 { "y" } else { "ies" },
                cat_summary.join(" · ")
            ),
        )
        .why(
            "SaaS-cliché phrases (\"elevate your\", \"AI-powered\", \"world-class\", \
             \"amplify\", \"unleash\") signal absent thought to readers who've seen them on \
             dozens of comparable sites — they read past as background noise rather than \
             evidence the page actually delivers anything. Editorial-press writing replaces \
             each cliché with the concrete claim the cliché was gesturing at.",
        )
        .fix(
            "rewrite each flagged phrase to a concrete claim that names a real artifact: \
             instead of \"AI-powered platform\" write the actual mechanism (e.g. \
             \"vector-indexed search across uploaded docs\"); instead of \"world-class \
             expertise\" name the specific credential (e.g. \"24 years building payment \
             rails at scale\"); instead of \"elevate your strategy\" describe the change \
             the reader will see (e.g. \"a one-week consultation that produces a written \
             recommendation\")",
        )
        .skill("author-cms-content")
        .avoid(
            "don't suppress the warn or add the phrase to the per-tenant suppress list — \
             the gate exists precisely because SaaS clichés erode the substrate's editorial \
             voice; suppressing them site-by-site undoes the cross-site signal the gate \
             provides",
        ),
    );
}

fn check_vague_eyebrow(
    section: &Value,
    path: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let eyebrow = section
        .get("eyebrow")
        .and_then(|e| e.as_str())
        .unwrap_or("");
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

/// CTA labels that add no information — generic verbs that
/// appear on millions of landing pages. Substitute with a
/// concrete verb tied to what happens.
/// Genuinely-slop CTA labels — these add no information that the
/// destination doesn't already imply. Deliberately conservative:
/// "Get started" and "Sign up" are out because they ARE concrete
/// primary actions on a marketing page; "Learn more" / "Click here"
/// / "Read more" / "Submit" / "Continue" / "Go" are not.
const VAGUE_CTA_LABELS: &[&str] = &[
    "learn more",
    "click here",
    "read more",
    "submit",
    "continue",
    "go",
    "try it",
    "try now",
    "explore",
    "discover",
];

fn check_vague_cta_label(
    sections: &[Value],
    path: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let mut hits: Vec<(String, String)> = Vec::new();
    fn walk(v: &Value, hits: &mut Vec<(String, String)>) {
        match v {
            Value::Object(obj) => {
                if let Some(label_val) = obj.get("label") {
                    if let Some(label) = label_val.as_str() {
                        let trimmed = label.trim().to_lowercase();
                        if VAGUE_CTA_LABELS.contains(&trimmed.as_str()) {
                            let href = obj
                                .get("href")
                                .and_then(|h| h.as_str())
                                .unwrap_or("?")
                                .to_owned();
                            hits.push((label.to_owned(), href));
                        }
                    }
                }
                for (_, val) in obj {
                    walk(val, hits);
                }
            }
            Value::Array(arr) => {
                for item in arr {
                    walk(item, hits);
                }
            }
            _ => {}
        }
    }
    for s in sections {
        walk(s, &mut hits);
    }
    if hits.is_empty() {
        return;
    }
    let examples: Vec<String> = hits
        .iter()
        .take(5)
        .map(|(label, href)| format!("\"{label}\" → {href}"))
        .collect();
    findings.push(Finding::warn(
        phase,
        path.to_owned(),
        format!(
            "vague_cta_label: {} CTA label(s) read as filler ({}); substitute with a concrete verb tied to what the action does (e.g. \"Read the comparison\" rather than \"Learn more\")",
            hits.len(),
            examples.join(", ")
        ),
    ));
}

fn check_roadmap_vagueness(
    sections: &[Value],
    path: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let mut total_items: usize = 0;
    let mut vague_items: usize = 0;
    for section in sections {
        if section.get("kind").and_then(|k| k.as_str()) != Some("roadmap") {
            continue;
        }
        for bucket_key in &["now", "next", "later", "soon"] {
            let Some(items) = section.get(bucket_key).and_then(|v| v.as_array()) else {
                continue;
            };
            for item in items {
                let Some(text) = item.as_str() else { continue };
                total_items += 1;
                let lower = text.to_lowercase();
                if lower.contains("soon")
                    || lower.contains("tbd")
                    || lower.contains("eventually")
                    || lower.contains("later")
                    || lower.contains("coming")
                    || lower.contains("planned")
                {
                    vague_items += 1;
                }
            }
        }
    }
    if total_items == 0 {
        return;
    }
    let ratio = vague_items as f32 / total_items as f32;
    if ratio > 0.5 {
        findings.push(Finding::warn(
            phase,
            path.to_owned(),
            format!(
                "roadmap_vagueness: {vague_items}/{total_items} roadmap items contain hedging language (soon / tbd / eventually / later / coming / planned) — substitute with concrete near-term commitments or remove"
            ),
        ));
    }
}

fn check_image_desert(
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
    if content_sections < 4 {
        return;
    }
    let mut image_count = 0;
    fn has_image(v: &Value) -> bool {
        match v {
            Value::Object(obj) => {
                if let Some(kind) = obj.get("kind").and_then(|k| k.as_str()) {
                    if matches!(
                        kind,
                        "picture"
                            | "asset_slug"
                            | "logo_cloud"
                            | "logo_wall"
                            | "feature_spotlight"
                            | "photo"
                    ) {
                        return true;
                    }
                }
                if obj.contains_key("src") || obj.contains_key("icon_slug") {
                    return true;
                }
                for (_, val) in obj {
                    if has_image(val) {
                        return true;
                    }
                }
                false
            }
            Value::Array(arr) => arr.iter().any(has_image),
            _ => false,
        }
    }
    for s in sections {
        if has_image(s) {
            image_count += 1;
        }
    }
    if image_count == 0 {
        findings.push(
            Finding::warn(
                phase,
                path.to_owned(),
                format!(
                    "image_desert: page has {content_sections} content section(s) and zero image / icon / illustration / logo references — the page feels uninhabited; consider adding visual texture via feature_spotlight icons, logo_cloud, picture, or image_hero with photo background"
                ),
            )
            .why("a page that's all text reads as either (a) a legal disclosure where text density is the point or (b) an unfinished page where the editor forgot the visual layer; the detector can't distinguish, so it flags both and lets the editor decide")
            .fix("if this is genuinely text-only by intent (privacy / terms / disclosure), add a single illustrative section above the body (an image_hero with a 'compact' or 'narrow' height, an asset_slug pointer to a relevant icon, or a feature_spotlight with one item carrying an icon_slug). If this is unfinished, add 2-3 visual sections to break up the wall of text")
            .skill("compose-loom-page")
            .avoid("don't stuff a stock photo onto a legal page just to silence this gate — the gate exists to flag uninhabited pages, not to require photos on every page. A single icon next to the title is enough visual anchor for a disclosure page"),
        );
    }
}

/// `short_paragraph_dominance` — scannable-bait page-shape detector.
///
/// SaaS marketing pages routinely chop prose into one-sentence
/// paragraphs to feel "scannable". The visual fingerprint: dozens of
/// tiny p-tags separated by big margins, no actual essay-density
/// anywhere. Real editorial body sits in multi-sentence paragraphs.
///
/// Heuristic: collect every `paragraph` / `lede` section's body text;
/// compute the average word count. Flag pages where:
/// * at least 5 paragraphs are present (small pages are exempt — a
///   landing splash legitimately has short copy)
/// * AND the average paragraph length is below 12 words.
///
/// Distinct from `sparse_page` (which flags pages with too few
/// sections). This one targets pages with PLENTY of sections, all
/// of them anemic.
fn check_short_paragraph_dominance(
    sections: &[Value],
    path: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    const MIN_PARAGRAPHS: usize = 5;
    const SHORT_THRESHOLD_WORDS: f32 = 12.0;

    let mut paragraph_word_counts: Vec<usize> = Vec::new();
    for s in sections {
        let Some(kind) = s.get("kind").and_then(|k| k.as_str()) else {
            continue;
        };
        if !matches!(kind, "paragraph" | "lede") {
            continue;
        }
        // `paragraph` carries a `text` field; `lede` carries `text`
        // or `body` depending on the variant. Try both.
        let body = s
            .get("text")
            .and_then(|t| t.as_str())
            .or_else(|| s.get("body").and_then(|b| b.as_str()))
            .unwrap_or("");
        let words = body.split_whitespace().count();
        if words > 0 {
            paragraph_word_counts.push(words);
        }
    }
    let total_paragraphs = paragraph_word_counts.len();
    if total_paragraphs < MIN_PARAGRAPHS {
        return;
    }
    let sum: usize = paragraph_word_counts.iter().sum();
    let avg = sum as f32 / total_paragraphs as f32;
    if avg >= SHORT_THRESHOLD_WORDS {
        return;
    }
    let short_count = paragraph_word_counts
        .iter()
        .filter(|n| **n < SHORT_THRESHOLD_WORDS as usize)
        .count();
    findings.push(Finding::warn(
        phase,
        path.to_owned(),
        format!(
            "short_paragraph_dominance: {} of {} paragraphs are under {} words (avg = {:.1}). Scannable-bait page shape — reads as marketing filler rather than editorial body. Combine related sentences into multi-sentence paragraphs; reserve one-sentence paragraphs for emphasis, not as the dominant rhythm.",
            short_count,
            total_paragraphs,
            SHORT_THRESHOLD_WORDS as usize,
            avg,
        ),
    ));
}

/// `adjacent_section_repetition` — same `kind` 3+ times in a row.
///
/// The structural fingerprint of a SkillShots-shape page: 5
/// `feature_spotlight` adjacent, or 8 `paragraph` in a row, or 4
/// `pricing` tiers stacked. The page reads as one repeating beat
/// rather than a composition of contrasting blocks.
///
/// Distinct from:
/// * `monotonous_feature_grid` — flags ONE feature_spotlight with
///   too-similar icons across its items. This check flags MULTIPLE
///   adjacent sections of the same kind.
/// * `sparse_page` — flags too few sections total. This one fires
///   on pages with PLENTY of sections, all of the same shape.
///
/// Heuristic:
/// 1. Walk `sections` linearly.
/// 2. Maintain a run-length counter on the current section's `kind`.
/// 3. Flag every run of 3+ same-kind adjacent sections.
///
/// Exempt kinds: section-glue primitives that legitimately repeat:
/// * `paragraph` — editorial body is supposed to be many paragraphs.
/// * `heading`, `sub_heading` — section labels naturally cluster.
/// * `divider`, `spacer` — decoration-only.
/// * `lede` — opening lede paragraphs.
///
/// The check targets compound primitives (feature_spotlight, pricing,
/// quote, testimonial, kv_pair, logo_wall, image_hero, etc.) where
/// repetition signals weak page composition.
fn check_adjacent_section_repetition(
    sections: &[Value],
    path: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    const MIN_RUN: usize = 3;
    const EXEMPT_KINDS: &[&str] = &[
        "paragraph",
        "heading",
        "sub_heading",
        "divider",
        "spacer",
        "lede",
        "drop_cap",
        "epigraph",
        "container",
    ];

    let mut runs: Vec<(String, usize)> = Vec::new();
    let mut current_kind: Option<String> = None;
    let mut current_run = 0usize;
    for s in sections {
        let kind = s
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or("")
            .to_owned();
        if Some(&kind) == current_kind.as_ref() {
            current_run += 1;
        } else {
            if let Some(prev_kind) = current_kind.take() {
                if current_run >= MIN_RUN && !EXEMPT_KINDS.contains(&prev_kind.as_str()) {
                    runs.push((prev_kind, current_run));
                }
            }
            current_kind = Some(kind);
            current_run = 1;
        }
    }
    // Flush the final run.
    if let Some(prev_kind) = current_kind.take() {
        if current_run >= MIN_RUN && !EXEMPT_KINDS.contains(&prev_kind.as_str()) {
            runs.push((prev_kind, current_run));
        }
    }

    if runs.is_empty() {
        return;
    }
    let summary: Vec<String> = runs.iter().map(|(k, n)| format!("{k}×{n}")).collect();
    findings.push(Finding::warn(
        phase,
        path.to_owned(),
        format!(
            "adjacent_section_repetition: {} run(s) of {}+ same-kind sections adjacent [{}]. Page reads as one repeating beat rather than a composition of contrasting blocks — interleave with other primitives (pull_quote / kv_pair / code_shell / heading) to break the rhythm.",
            runs.len(),
            MIN_RUN,
            summary.join(", "),
        ),
    ));
}

/// `emoji_in_body` — detect emoji glyphs in CMS body text.
///
/// Per the substrate's editorial-first directive (no decorative
/// chrome in editorial composition), emoji in CMS body text are a
/// marketing-flavor signal: they substitute for typography +
/// language. Editorial body should carry meaning in words, not
/// glyphs. The check flags any emoji codepoint anywhere in the
/// page's string-valued JSON fields.
///
/// Detection scans Unicode codepoint ranges:
/// * U+1F300 through U+1FAFF — Misc Symbols and Pictographs +
///   Emoticons + Transport + Misc Symbols-and-Arrows + extended
///   Symbols-and-Pictographs + Chess + Supplemental Arrows-C +
///   Supplemental Symbols-and-Pictographs.
/// * U+2600 through U+27BF — Misc Symbols + Dingbats (covers
///   ☀ ★ ✓ ✗ ✨ etc. when used as emoji).
/// * U+FE0F — variation selector-16 (forces emoji presentation).
/// * U+1F1E6 through U+1F1FF — Regional indicator symbols (flags).
///
/// Out of scope: ASCII characters that LOOK like emojis (`<3`,
/// `:)`) — these are character-set neutral text emoticons; no
/// substrate enforcement.
///
/// Severity: warn. Some sites legitimately ship with emoji content
/// (i18n hint, accessibility shortcut, deliberate editorial choice).
/// Flag draws the operator's attention without blocking the build.
fn check_emoji_in_body(
    sections: &[Value],
    path: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    fn is_emoji_codepoint(c: char) -> bool {
        let cp = c as u32;
        // Main emoji ranges.
        (0x1F300..=0x1FAFF).contains(&cp)
            // Misc Symbols + Dingbats.
            || (0x2600..=0x27BF).contains(&cp)
            // Regional indicator (flags).
            || (0x1F1E6..=0x1F1FF).contains(&cp)
            // Variation selector-16 (emoji presentation).
            || cp == 0xFE0F
    }

    let mut total_emoji = 0usize;
    let mut sample_strings: Vec<String> = Vec::new();
    collect_text(sections, &mut |t| {
        let count = t.chars().filter(|c| is_emoji_codepoint(*c)).count();
        if count > 0 {
            total_emoji += count;
            if sample_strings.len() < 3 {
                let truncated: String = t.chars().take(80).collect();
                sample_strings.push(truncated);
            }
        }
    });
    if total_emoji == 0 {
        return;
    }
    findings.push(Finding::warn(
        phase,
        path.to_owned(),
        format!(
            "emoji_in_body: {} emoji glyph(s) found across page text. Editorial composition prefers meaning carried in words/typography over decorative glyphs. Sample text: [{}]. If emoji is deliberate (i18n hint, deliberate editorial choice), suppress this finding via the operator workflow.",
            total_emoji,
            sample_strings
                .iter()
                .map(|s| format!("\"{}\"", s.replace('"', "\\\"")))
                .collect::<Vec<_>>()
                .join(", "),
        ),
    ));
}

/// `identical_section_text` — flag byte-identical body text across
/// multiple sections. Copy-paste filler signal.
///
/// Most legitimate pages don't ship the same paragraph twice. When
/// two sections carry the byte-identical body text it usually means:
/// 1. A scaffolding template stub the operator forgot to fill in.
/// 2. Copy-paste filler used to bulk up a thin page.
/// 3. A template-author error where placeholder text leaked into
///    multiple variants.
///
/// Skip rules:
/// * Bodies shorter than 20 chars — headings / eyebrows / kicker
///   text legitimately repeat at the surface level; the
///   `adjacent_section_repetition` check covers structural patterns.
/// * Whitespace-only bodies — collapses to empty after trim.
///
/// Heuristic:
/// 1. Walk sections; for each `paragraph` / `lede` / `quote` /
///    `pull_quote`, extract the body text.
/// 2. Group by trimmed-lowercased text.
/// 3. Flag any text that appears 2+ times.
fn check_identical_section_text(
    sections: &[Value],
    path: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    const MIN_LEN: usize = 20;

    let mut groups: std::collections::BTreeMap<String, Vec<usize>> =
        std::collections::BTreeMap::new();
    for (idx, s) in sections.iter().enumerate() {
        let Some(kind) = s.get("kind").and_then(|k| k.as_str()) else {
            continue;
        };
        // Body-text-bearing sections we audit. Exclude heading /
        // sub_heading — short labels are expected to repeat.
        if !matches!(
            kind,
            "paragraph" | "lede" | "quote" | "pull_quote" | "epigraph"
        ) {
            continue;
        }
        let body = s
            .get("text")
            .and_then(|t| t.as_str())
            .or_else(|| s.get("body").and_then(|b| b.as_str()))
            .unwrap_or("")
            .trim();
        if body.len() < MIN_LEN {
            continue;
        }
        groups.entry(body.to_lowercase()).or_default().push(idx);
    }

    let duplicates: Vec<(&String, &Vec<usize>)> = groups
        .iter()
        .filter(|(_, indices)| indices.len() >= 2)
        .collect();
    if duplicates.is_empty() {
        return;
    }
    // Use the first duplicate as the worst-offender fingerprint.
    let (first_body, first_indices) = duplicates[0];
    let snippet: String = first_body.chars().take(80).collect();
    findings.push(Finding::warn(
        phase,
        path.to_owned(),
        format!(
            "identical_section_text: {} distinct body text(s) appear in 2+ sections each. Copy-paste filler signal — operator likely scaffolded a section then never filled it in, OR pasted body text twice. Worst case: {} occurrence(s) of \"{}\" at section indices {:?}. Replace duplicates with unique authored copy.",
            duplicates.len(),
            first_indices.len(),
            snippet,
            first_indices,
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
        let attribution = t.get("attribution").and_then(|a| a.as_str()).unwrap_or("");
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
        check_page(
            Path::new("test.json"),
            &page,
            &[],
            &[],
            &mut findings,
            "aesthetic_distinctiveness",
        );
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
        check_page(
            Path::new("test.json"),
            &page,
            &[],
            &[],
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings.iter().any(|f| f.message.contains("scaffold_only")));
    }

    #[test]
    fn single_word_hero_warns() {
        let section = json!({"kind": "image_hero", "title": "Welcome."});
        let mut findings = vec![];
        check_centered_single_word_hero(
            &section,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings
            .iter()
            .any(|f| f.message.contains("centered_single_word_hero")));
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
        check_centered_single_word_hero(
            &section,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
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
        assert!(findings
            .iter()
            .any(|f| f.message.contains("monotonous_feature_grid")));
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
        assert!(findings
            .iter()
            .any(|f| f.message.contains("most_popular_badge")));
    }

    #[test]
    fn numbers_that_compose_warns() {
        let section = json!({
            "kind": "stat_band",
            "heading": "Numbers that compose"
        });
        let mut findings = vec![];
        check_numbers_that_compose(&section, "test", &mut findings, "aesthetic_distinctiveness");
        assert!(findings
            .iter()
            .any(|f| f.message.contains("numbers_that_compose")));
    }

    #[test]
    fn tenant_corpus_extra_jargon_phrases_fire() {
        // Operator declared "the Acme advantage" as a tenant-extra
        // jargon phrase; content matching it should fire the
        // detector alongside any baseline hits.
        let sections = vec![json!({
            "kind": "paragraph",
            "text": "Try the Acme advantage today and join us."
        })];
        let mut findings = vec![];
        check_corporate_jargon(
            &sections,
            "test",
            &["the Acme advantage"],
            &[],
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert_eq!(findings.len(), 1);
        let msg = &findings[0].message;
        assert!(msg.contains("the Acme advantage"));
    }

    #[test]
    fn tenant_corpus_extra_jargon_case_insensitive_match() {
        // Tenant extras match lowercased, like the baseline scan.
        let sections = vec![json!({
            "kind": "paragraph",
            "text": "EMBRACE THE SYNERGY"
        })];
        let mut findings = vec![];
        check_corporate_jargon(
            &sections,
            "test",
            &["embrace the synergy"],
            &[],
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("embrace the synergy"));
    }

    #[test]
    fn tenant_corpus_suppress_removes_baseline_phrase() {
        // Tenant suppresses "transform your" because they run an
        // actual transformation business. Content with that phrase
        // shouldn't fire.
        let sections = vec![json!({
            "kind": "paragraph",
            "text": "We will transform your industry."
        })];
        let mut findings = vec![];
        check_corporate_jargon(
            &sections,
            "test",
            &[],
            &["transform your"],
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn tenant_corpus_suppress_leaves_other_baseline_intact() {
        // Suppress one phrase, keep others. Content with the
        // suppressed phrase + a non-suppressed one should fire
        // ONLY for the non-suppressed.
        let sections = vec![json!({
            "kind": "paragraph",
            "text": "We will transform your best-in-class workflow."
        })];
        let mut findings = vec![];
        check_corporate_jargon(
            &sections,
            "test",
            &[],
            &["transform your"],
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert_eq!(findings.len(), 1);
        let msg = &findings[0].message;
        assert!(msg.contains("best-in-class"));
        assert!(!msg.contains("transform your"));
    }

    #[test]
    fn corporate_jargon_warns_on_known_phrases() {
        let sections = vec![
            json!({"kind": "paragraph", "text": "Our best-in-class platform delivers a seamless integration."}),
        ];
        let mut findings = vec![];
        check_corporate_jargon(
            &sections,
            "test",
            &[],
            &[],
            &mut findings,
            "aesthetic_distinctiveness",
        );
        let msg = &findings[0].message;
        assert!(msg.contains("corporate_jargon"));
        assert!(msg.contains("best-in-class"));
        assert!(msg.contains("seamless integration"));
        // After 2026-05-20 categorization: each phrase is bucketed.
        // best-in-class → superlative, seamless integration → business-jargon.
        assert!(msg.contains("superlative:"));
        assert!(msg.contains("business-jargon:"));
    }

    #[test]
    fn corporate_jargon_categorizes_amplifier_verbs() {
        let sections = vec![
            json!({"kind": "paragraph", "text": "Supercharge your workflow and unleash your team's potential."}),
        ];
        let mut findings = vec![];
        check_corporate_jargon(
            &sections,
            "test",
            &[],
            &[],
            &mut findings,
            "aesthetic_distinctiveness",
        );
        let msg = &findings[0].message;
        assert!(msg.contains("amplifier:"));
        assert!(msg.contains("supercharge"));
        assert!(msg.contains("unleash"));
    }

    #[test]
    fn corporate_jargon_categorizes_future_of_framings() {
        let sections = vec![json!({"kind": "heading", "text": "Welcome to the future of work."})];
        let mut findings = vec![];
        check_corporate_jargon(
            &sections,
            "test",
            &[],
            &[],
            &mut findings,
            "aesthetic_distinctiveness",
        );
        let msg = &findings[0].message;
        assert!(msg.contains("future-of:"));
        assert!(msg.contains("the future of work"));
    }

    #[test]
    fn corporate_jargon_categorizes_ai_buzzwords() {
        let sections = vec![
            json!({"kind": "paragraph", "text": "An AI-powered platform for blockchain-powered teams."}),
        ];
        let mut findings = vec![];
        check_corporate_jargon(
            &sections,
            "test",
            &[],
            &[],
            &mut findings,
            "aesthetic_distinctiveness",
        );
        let msg = &findings[0].message;
        assert!(msg.contains("ai-buzzword:"));
        assert!(msg.contains("ai-powered"));
        assert!(msg.contains("blockchain-powered"));
    }

    #[test]
    fn corporate_jargon_categorizes_superlatives() {
        let sections = vec![
            json!({"kind": "paragraph", "text": "Our world-class engineering team is industry-leading."}),
        ];
        let mut findings = vec![];
        check_corporate_jargon(
            &sections,
            "test",
            &[],
            &[],
            &mut findings,
            "aesthetic_distinctiveness",
        );
        let msg = &findings[0].message;
        assert!(msg.contains("superlative:"));
        assert!(msg.contains("world-class"));
        assert!(msg.contains("industry-leading"));
    }

    #[test]
    fn corporate_jargon_categorizes_business_jargon_default() {
        // Legacy bucket — phrases not in any specific category fall
        // here so the dictionary expansion stays back-compat.
        let sections = vec![
            json!({"kind": "paragraph", "text": "Let's circle back on the synergies after the deep dive."}),
        ];
        let mut findings = vec![];
        check_corporate_jargon(
            &sections,
            "test",
            &[],
            &[],
            &mut findings,
            "aesthetic_distinctiveness",
        );
        let msg = &findings[0].message;
        assert!(msg.contains("business-jargon:"));
        assert!(msg.contains("circle back"));
        assert!(msg.contains("synergies"));
        assert!(msg.contains("deep dive"));
    }

    #[test]
    fn corporate_jargon_reports_multiple_categories_in_one_finding() {
        // A page with mixed-category slop should produce ONE finding
        // surfacing all categories, separated by " · " for readability.
        let sections = vec![
            json!({"kind": "paragraph", "text": "Supercharge your workflow with our world-class AI-powered platform."}),
        ];
        let mut findings = vec![];
        check_corporate_jargon(
            &sections,
            "test",
            &[],
            &[],
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert_eq!(
            findings.len(),
            1,
            "should emit exactly one finding even with multi-category slop"
        );
        let msg = &findings[0].message;
        assert!(msg.contains("amplifier:"));
        assert!(msg.contains("ai-buzzword:"));
        assert!(msg.contains("superlative:"));
        assert!(msg.contains(" · "));
        assert!(msg.contains("3 categor"));
    }

    #[test]
    fn corporate_jargon_does_not_warn_on_clean_prose() {
        let sections = vec![
            json!({"kind": "paragraph", "text": "Typed contracts. Audited at every commit. Reproducible builds."}),
        ];
        let mut findings = vec![];
        check_corporate_jargon(
            &sections,
            "test",
            &[],
            &[],
            &mut findings,
            "aesthetic_distinctiveness",
        );
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
        check_placeholder_email(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings
            .iter()
            .any(|f| f.message.contains("placeholder_email")));
    }

    #[test]
    fn placeholder_email_skips_legit_input_placeholder() {
        // The hit IS the input placeholder — legitimate UX, not flagged.
        let sections =
            vec![json!({"kind": "newsletter_signup", "placeholder": "you@yourcompany.com"})];
        let mut findings = vec![];
        check_placeholder_email(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn collect_text_walks_nested_json() {
        let sections = vec![json!({"kind": "x", "items": [{"key": "deep", "value": "synergy"}]})];
        let mut hits = 0;
        collect_text(&sections, &mut |t| {
            if t.contains("synergy") {
                hits += 1;
            }
        });
        assert_eq!(hits, 1);
    }

    #[test]
    fn short_paragraph_dominance_warns_on_scannable_bait_shape() {
        // 6 paragraphs all very short — typical SaaS-marketing rhythm.
        let sections = vec![
            json!({"kind": "paragraph", "text": "Fast. Reliable. Built for you."}),
            json!({"kind": "paragraph", "text": "Use anywhere. Anytime."}),
            json!({"kind": "paragraph", "text": "Cancel any time."}),
            json!({"kind": "paragraph", "text": "Free to start."}),
            json!({"kind": "paragraph", "text": "Sign up below."}),
            json!({"kind": "paragraph", "text": "It just works."}),
        ];
        let mut findings = vec![];
        check_short_paragraph_dominance(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert_eq!(findings.len(), 1);
        let msg = &findings[0].message;
        assert!(msg.contains("short_paragraph_dominance"));
        assert!(msg.contains("6 paragraphs"));
        assert!(msg.contains("under 12 words"));
    }

    #[test]
    fn short_paragraph_dominance_silent_on_essay_density() {
        // Multi-sentence editorial paragraphs — should NOT warn.
        let sections = vec![
            json!({"kind": "paragraph", "text": "The minimum wage today is worth 40% less than it was in 1968. A household earning $75,000 can only afford 21% of home listings."}),
            json!({"kind": "paragraph", "text": "Buy, Borrow, Die. 1031 exchanges. Roth IRA stacking. None of these are loopholes — they are statutes written into the tax code."}),
            json!({"kind": "paragraph", "text": "Before maximizing returns, protect what you have. Disability, life, health, home — covered honestly, including which products are oversold."}),
            json!({"kind": "paragraph", "text": "Five weeks, member-only. Covers strategy, structure, and keeping what you build — not how to pick stocks."}),
            json!({"kind": "paragraph", "text": "Index funds. Asset location. Rebalancing rules. The boring strategy compounds harder than the exciting one."}),
        ];
        let mut findings = vec![];
        check_short_paragraph_dominance(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn short_paragraph_dominance_silent_below_min_paragraph_count() {
        // Only 4 short paragraphs — below the MIN_PARAGRAPHS=5 floor.
        // A landing splash can legitimately ship short copy.
        let sections = vec![
            json!({"kind": "paragraph", "text": "Fast."}),
            json!({"kind": "paragraph", "text": "Reliable."}),
            json!({"kind": "paragraph", "text": "Built for you."}),
            json!({"kind": "paragraph", "text": "Sign up."}),
        ];
        let mut findings = vec![];
        check_short_paragraph_dominance(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn short_paragraph_dominance_counts_lede_alongside_paragraph() {
        // `lede` sections also count toward the body-text average.
        let sections = vec![
            json!({"kind": "lede", "text": "Fast and clean."}),
            json!({"kind": "paragraph", "text": "Try it."}),
            json!({"kind": "paragraph", "text": "Use it."}),
            json!({"kind": "paragraph", "text": "Love it."}),
            json!({"kind": "paragraph", "text": "Buy it."}),
        ];
        let mut findings = vec![];
        check_short_paragraph_dominance(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("5 paragraphs"));
    }

    #[test]
    fn short_paragraph_dominance_ignores_non_body_sections() {
        // Heading / cta / kv_pair / etc. are NOT body text and don't
        // count toward the paragraph word-count average.
        let sections = vec![
            json!({"kind": "heading", "text": "Hi"}),
            json!({"kind": "call_to_action", "title": "Go"}),
            json!({"kind": "kv_pair", "items": []}),
        ];
        let mut findings = vec![];
        check_short_paragraph_dominance(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn short_paragraph_dominance_average_at_threshold_does_not_warn() {
        // Average exactly at the 12-word threshold should NOT warn —
        // the check is strict less-than.
        let sections = vec![
            json!({"kind": "paragraph", "text": "one two three four five six seven eight nine ten eleven twelve"}),
            json!({"kind": "paragraph", "text": "one two three four five six seven eight nine ten eleven twelve"}),
            json!({"kind": "paragraph", "text": "one two three four five six seven eight nine ten eleven twelve"}),
            json!({"kind": "paragraph", "text": "one two three four five six seven eight nine ten eleven twelve"}),
            json!({"kind": "paragraph", "text": "one two three four five six seven eight nine ten eleven twelve"}),
        ];
        let mut findings = vec![];
        check_short_paragraph_dominance(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn adjacent_section_repetition_warns_on_three_in_a_row() {
        // 3 feature_spotlight adjacent — the canonical SkillShots shape.
        let sections = vec![
            json!({"kind": "image_hero", "title": "x"}),
            json!({"kind": "feature_spotlight", "items": []}),
            json!({"kind": "feature_spotlight", "items": []}),
            json!({"kind": "feature_spotlight", "items": []}),
            json!({"kind": "call_to_action", "title": "Go"}),
        ];
        let mut findings = vec![];
        check_adjacent_section_repetition(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert_eq!(findings.len(), 1);
        let msg = &findings[0].message;
        assert!(msg.contains("adjacent_section_repetition"));
        assert!(msg.contains("feature_spotlight×3"));
        assert!(msg.contains("1 run"));
    }

    #[test]
    fn adjacent_section_repetition_silent_on_two_in_a_row() {
        // 2 adjacent — below the MIN_RUN=3 threshold.
        let sections = vec![
            json!({"kind": "quote", "body": "x"}),
            json!({"kind": "quote", "body": "y"}),
            json!({"kind": "call_to_action", "title": "Go"}),
        ];
        let mut findings = vec![];
        check_adjacent_section_repetition(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn adjacent_section_repetition_exempts_paragraph_runs() {
        // 8 paragraphs adjacent — editorial body legitimately
        // repeats paragraph. The exempt list catches this.
        let sections = vec![
            json!({"kind": "heading", "text": "Section"}),
            json!({"kind": "paragraph", "text": "First."}),
            json!({"kind": "paragraph", "text": "Second."}),
            json!({"kind": "paragraph", "text": "Third."}),
            json!({"kind": "paragraph", "text": "Fourth."}),
            json!({"kind": "paragraph", "text": "Fifth."}),
            json!({"kind": "paragraph", "text": "Sixth."}),
            json!({"kind": "paragraph", "text": "Seventh."}),
            json!({"kind": "paragraph", "text": "Eighth."}),
        ];
        let mut findings = vec![];
        check_adjacent_section_repetition(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn adjacent_section_repetition_multiple_runs_reported() {
        // Two distinct runs in the same page — both surface in the
        // summary.
        let sections = vec![
            json!({"kind": "pricing", "tiers": []}),
            json!({"kind": "pricing", "tiers": []}),
            json!({"kind": "pricing", "tiers": []}),
            json!({"kind": "heading", "text": "Or"}),
            json!({"kind": "feature_spotlight", "items": []}),
            json!({"kind": "feature_spotlight", "items": []}),
            json!({"kind": "feature_spotlight", "items": []}),
            json!({"kind": "feature_spotlight", "items": []}),
        ];
        let mut findings = vec![];
        check_adjacent_section_repetition(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        let msg = &findings[0].message;
        assert!(msg.contains("2 run"));
        assert!(msg.contains("pricing×3"));
        assert!(msg.contains("feature_spotlight×4"));
    }

    #[test]
    fn adjacent_section_repetition_run_at_end_of_page_caught() {
        // The final-run flush must catch runs that terminate at the
        // end of the section list (no following section breaks the run).
        let sections = vec![
            json!({"kind": "image_hero", "title": "x"}),
            json!({"kind": "kv_pair", "items": []}),
            json!({"kind": "kv_pair", "items": []}),
            json!({"kind": "kv_pair", "items": []}),
        ];
        let mut findings = vec![];
        check_adjacent_section_repetition(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        let msg = &findings[0].message;
        assert!(msg.contains("kv_pair×3"));
    }

    #[test]
    fn adjacent_section_repetition_does_not_count_non_adjacent_repeats() {
        // 3 feature_spotlight total but interleaved with other kinds
        // — the page composes contrast; no warning.
        let sections = vec![
            json!({"kind": "feature_spotlight", "items": []}),
            json!({"kind": "paragraph", "text": "Body."}),
            json!({"kind": "feature_spotlight", "items": []}),
            json!({"kind": "pull_quote", "body": "Quote."}),
            json!({"kind": "feature_spotlight", "items": []}),
        ];
        let mut findings = vec![];
        check_adjacent_section_repetition(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn emoji_in_body_warns_on_pictograph_emoji() {
        // Single 🚀 in a paragraph body — main-range emoji.
        let sections = vec![json!({"kind": "paragraph", "text": "Launching 🚀 our new tier."})];
        let mut findings = vec![];
        check_emoji_in_body(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert_eq!(findings.len(), 1);
        let msg = &findings[0].message;
        assert!(msg.contains("emoji_in_body"));
        assert!(msg.contains("1 emoji"));
        assert!(msg.contains("🚀"));
    }

    #[test]
    fn emoji_in_body_warns_on_dingbat_range() {
        // Dingbat ✨ — U+2728. Common decorative bullet.
        let sections = vec![json!({"kind": "paragraph", "text": "Subscribe ✨ today"})];
        let mut findings = vec![];
        check_emoji_in_body(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("1 emoji"));
    }

    #[test]
    fn emoji_in_body_counts_multiple_glyphs() {
        let sections = vec![
            json!({"kind": "paragraph", "text": "🔥🔥🔥 Hot deal"}),
            json!({"kind": "heading", "text": "✓ done"}),
        ];
        let mut findings = vec![];
        check_emoji_in_body(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("4 emoji"));
    }

    #[test]
    fn emoji_in_body_silent_on_clean_prose() {
        let sections = vec![
            json!({"kind": "paragraph", "text": "Substrate ships editorial composition. No decorative chrome."}),
            json!({"kind": "heading", "text": "Plain typography only."}),
        ];
        let mut findings = vec![];
        check_emoji_in_body(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn emoji_in_body_silent_on_ascii_emoticons() {
        // <3 and :) are character-set neutral text — NOT flagged.
        let sections = vec![json!({"kind": "paragraph", "text": "Love it <3 So good :)"})];
        let mut findings = vec![];
        check_emoji_in_body(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn emoji_in_body_detects_flag_codepoints() {
        // Regional indicator pair 🇺🇸 (U+1F1FA + U+1F1F8) renders as US flag.
        // Two regional-indicator chars + zero composing scalars.
        let sections = vec![json!({"kind": "paragraph", "text": "Welcome 🇺🇸 friends"})];
        let mut findings = vec![];
        check_emoji_in_body(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert_eq!(findings.len(), 1);
        // 2 regional indicator codepoints make one visual flag.
        assert!(findings[0].message.contains("2 emoji"));
    }

    #[test]
    fn emoji_in_body_surfaces_sample_text() {
        let sections =
            vec![json!({"kind": "paragraph", "text": "Get started 🚀 with PlausiDen today"})];
        let mut findings = vec![];
        check_emoji_in_body(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings[0]
            .message
            .contains(r#""Get started 🚀 with PlausiDen today""#));
    }

    #[test]
    fn emoji_in_body_caps_sample_text_at_three() {
        // 5 different paragraphs with emojis — sample list caps at 3.
        let sections = vec![
            json!({"kind": "paragraph", "text": "🎉 one"}),
            json!({"kind": "paragraph", "text": "🎊 two"}),
            json!({"kind": "paragraph", "text": "🚀 three"}),
            json!({"kind": "paragraph", "text": "💯 four"}),
            json!({"kind": "paragraph", "text": "🔥 five"}),
        ];
        let mut findings = vec![];
        check_emoji_in_body(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        let msg = &findings[0].message;
        // First 3 paragraphs surface in the sample; last 2 don't.
        assert!(msg.contains("🎉"));
        assert!(msg.contains("🎊"));
        assert!(msg.contains("🚀"));
        assert!(!msg.contains("💯"));
        assert!(!msg.contains("🔥"));
        // Total count is 5.
        assert!(msg.contains("5 emoji"));
    }

    #[test]
    fn identical_section_text_warns_on_duplicate_paragraph_body() {
        let sections = vec![
            json!({"kind": "paragraph", "text": "The substrate carries the page composition."}),
            json!({"kind": "heading", "text": "Section"}),
            json!({"kind": "paragraph", "text": "The substrate carries the page composition."}),
        ];
        let mut findings = vec![];
        check_identical_section_text(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert_eq!(findings.len(), 1);
        let msg = &findings[0].message;
        assert!(msg.contains("identical_section_text"));
        assert!(msg.contains("2 occurrence"));
        assert!(msg.contains("the substrate carries"));
    }

    #[test]
    fn identical_section_text_silent_on_short_repeats() {
        // Bodies < 20 chars are exempt (headings / eyebrows
        // legitimately repeat at the surface level).
        let sections = vec![
            json!({"kind": "paragraph", "text": "Yes."}),
            json!({"kind": "paragraph", "text": "Yes."}),
        ];
        let mut findings = vec![];
        check_identical_section_text(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn identical_section_text_case_insensitive_match() {
        // Same text different case still flagged — likely copy/paste
        // that picked up an upstream capitalization tweak.
        let sections = vec![
            json!({"kind": "paragraph", "text": "Editorial composition replaces SaaS marketing."}),
            json!({"kind": "paragraph", "text": "editorial composition replaces saas marketing."}),
        ];
        let mut findings = vec![];
        check_identical_section_text(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("2 occurrence"));
    }

    #[test]
    fn identical_section_text_silent_on_clean_page() {
        let sections = vec![
            json!({"kind": "paragraph", "text": "First unique paragraph explains the why."}),
            json!({"kind": "paragraph", "text": "Second unique paragraph names a concrete example."}),
            json!({"kind": "paragraph", "text": "Third unique paragraph closes with the call to action."}),
        ];
        let mut findings = vec![];
        check_identical_section_text(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn identical_section_text_handles_pull_quote_body_field() {
        // pull_quote carries its text in `body`, not `text`.
        let sections = vec![
            json!({"kind": "pull_quote", "body": "The same editorial pull quote shipped twice by mistake."}),
            json!({"kind": "paragraph", "text": "Different body content here entirely."}),
            json!({"kind": "pull_quote", "body": "The same editorial pull quote shipped twice by mistake."}),
        ];
        let mut findings = vec![];
        check_identical_section_text(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("2 occurrence"));
    }

    #[test]
    fn identical_section_text_reports_section_indices() {
        let sections = vec![
            json!({"kind": "heading", "text": "Intro"}),
            json!({"kind": "paragraph", "text": "Filler text that gets duplicated below."}),
            json!({"kind": "heading", "text": "Outro"}),
            json!({"kind": "paragraph", "text": "Filler text that gets duplicated below."}),
        ];
        let mut findings = vec![];
        check_identical_section_text(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        assert!(findings[0].message.contains("[1, 3]"));
    }

    #[test]
    fn identical_section_text_multiple_distinct_duplicates_reported() {
        let sections = vec![
            json!({"kind": "paragraph", "text": "First paragraph text that is long enough."}),
            json!({"kind": "paragraph", "text": "Second paragraph text that differs enough."}),
            json!({"kind": "paragraph", "text": "First paragraph text that is long enough."}),
            json!({"kind": "paragraph", "text": "Second paragraph text that differs enough."}),
        ];
        let mut findings = vec![];
        check_identical_section_text(
            &sections,
            "test",
            &mut findings,
            "aesthetic_distinctiveness",
        );
        let msg = &findings[0].message;
        // Two distinct duplicated bodies — count surfaces.
        assert!(msg.contains("2 distinct body text"));
    }
}
