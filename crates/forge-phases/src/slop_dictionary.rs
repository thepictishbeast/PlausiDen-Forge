//! `slop_dictionary` — build-time SaaS-marketing-cliche detector.
//!
//! Companion to PlausiDen-Crawler's runtime `heading_quality`
//! detector. Where Crawler scans the rendered DOM, this phase
//! reads cms/*.json directly so the operator gets the warn
//! BEFORE the page ships.
//!
//! Single finding kind:
//!   * `slop_dictionary.saas-cliche` warn — a text field on
//!     some section contains a phrase from the canonical SaaS-
//!     marketing-cliche list. Phrases match case-insensitively
//!     after whitespace collapse, exact-string-equality on the
//!     whole field value.
//!
//! warn-only — build doesn't fail. Operator decides whether to
//! rewrite or suppress. The substrate's job is to surface
//! consumer-shaped phrasing, not block the build.
//!
//! ## Phrase list
//!
//! Mirrors `crawler_detectors::heading_quality::SAAS_CLICHES` so
//! the build-time + runtime gates flag identical phrasing.
//! When new cliches land, they go on both lists.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * `#[non_exhaustive]` on phase struct.
//! * No unwrap/expect in non-test code.

use std::fs;

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde_json::Value;

/// Canonical SaaS-cliche list. Match case-insensitively as the
/// WHOLE field value (after trimming + whitespace collapse).
/// Conservative on purpose — these are unambiguous filler, not
/// borderline editorial choices.
const SAAS_CLICHES: &[&str] = &[
    "get started",
    "learn more",
    "built for speed",
    "built for scale",
    "powered by ai",
    "numbers that compose",
    "trusted by leaders",
    "trusted by teams",
    "join the waitlist",
    "see how it works",
    "ready to get started",
    "ready to build",
    "level up",
    "supercharge your workflow",
    "unlock the power",
    "redefine the way",
    "future of work",
    "modern stack",
];

/// Text-bearing section fields the phase scans. Pairs:
/// (section kind, field name). Mirrors the rendered surface
/// where SaaS-cliche text typically lives.
const TEXT_FIELDS: &[(&str, &str)] = &[
    ("heading", "text"),
    ("paragraph", "text"),
    ("lede", "text"),
    ("sublede", "text"),
    ("kicker", "text"),
    ("sub_heading", "text"),
    ("image_hero", "title"),
    ("image_hero", "lede"),
    ("image_hero", "eyebrow"),
    ("hero", "title"),
    ("hero", "lede"),
    ("hero", "eyebrow"),
    ("split_hero", "title"),
    ("split_hero", "lede"),
    ("call_to_action", "title"),
    ("call_to_action", "lede"),
    ("call_to_action", "eyebrow"),
    ("pull_quote", "body"),
    ("pull_quote", "attribution"),
    ("epigraph", "body"),
    ("feature_spotlight", "heading"),
    ("stat_band", "heading"),
    ("kv_pair", "heading"),
];

/// `slop_dictionary` phase implementation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct SlopDictionaryPhase;

impl Phase for SlopDictionaryPhase {
    fn name(&self) -> &'static str {
        "slop_dictionary"
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
            if let Some(sections) = value.get("sections").and_then(|s| s.as_array()) {
                for (idx, section) in sections.iter().enumerate() {
                    let Some(kind) = section.get("kind").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    for (target_kind, field) in TEXT_FIELDS {
                        if *target_kind != kind {
                            continue;
                        }
                        let Some(raw_text) = section.get(*field).and_then(|v| v.as_str())
                        else {
                            continue;
                        };
                        let normalized = normalize(raw_text);
                        if SAAS_CLICHES.iter().any(|c| *c == normalized.as_str()) {
                            findings.push(
                                Finding::warn(
                                    self.name(),
                                    format!("{path_disp}#section-{idx}-{kind}.{field}"),
                                    format!(
                                        "{kind}.{field} text {raw_text:?} is a known SaaS-marketing cliche. The editorial substrate refuses this shape by design — rewrite as something the writer would actually say, or drop the field entirely."
                                    ),
                                )
                                .why(
                                    "single-phrase filler reads as consumer-shaped scaffolding; \
                                     a real editorial voice carries the load-bearing claim of the \
                                     section in its own words. The phrase list catches the unambiguous \
                                     cases that paul has flagged as substrate-violating.",
                                )
                                .fix(
                                    "rewrite the field in the author's own voice — a real sentence \
                                     that names the actual subject. Example: 'Get Started' → \
                                     'Read the first article free, then upgrade if it's useful.'",
                                )
                                .skill("author-cms-content")
                                .avoid(
                                    "don't suppress the warn by removing one word — the rewrite \
                                     should change the SHAPE of the phrasing, not just dodge the \
                                     exact-match list.",
                                ),
                            );
                        }
                    }
                }
            }
        }
        Ok(findings)
    }
}

fn normalize(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::{BuildCtx, BuildMode};

    fn make_ctx_with_cms(pages: &[(&str, &str)]) -> (BuildCtx, std::path::PathBuf) {
        let tmp = std::env::temp_dir().join(format!(
            "slop-dict-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(tmp.join("cms")).expect("mk cms");
        for (name, body) in pages {
            std::fs::write(tmp.join("cms").join(format!("{name}.json")), body)
                .expect("write json");
        }
        let ctx = BuildCtx {
            root: tmp.clone(),
            static_dir: tmp.join("static"),
            mode: BuildMode::Poc,
        };
        (ctx, tmp)
    }

    #[test]
    fn cliche_in_heading_text_flags() {
        let body = r#"{
            "title": "x", "description": "x", "path": "/",
            "sections": [{"kind": "heading", "level": 2, "text": "Get Started"}]
        }"#;
        let (ctx, tmp) = make_ctx_with_cms(&[("a", body)]);
        let findings = SlopDictionaryPhase.run(&ctx).expect("run");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].phase, "slop_dictionary");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn editorial_voice_does_not_flag() {
        let body = r#"{
            "title": "x", "description": "x", "path": "/",
            "sections": [{"kind": "heading", "level": 2,
                "text": "Why insurance is the foundation of every plan."}]
        }"#;
        let (ctx, tmp) = make_ctx_with_cms(&[("a", body)]);
        let findings = SlopDictionaryPhase.run(&ctx).expect("run");
        assert!(findings.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn case_insensitive_and_whitespace_collapsed() {
        let body = r#"{
            "title": "x", "description": "x", "path": "/",
            "sections": [{"kind": "paragraph",
                "text": "  POWERED   BY   AI  "}]
        }"#;
        let (ctx, tmp) = make_ctx_with_cms(&[("a", body)]);
        let findings = SlopDictionaryPhase.run(&ctx).expect("run");
        assert_eq!(findings.len(), 1);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn cliche_in_image_hero_title_flags() {
        let body = r#"{
            "title": "x", "description": "x", "path": "/",
            "sections": [{"kind": "image_hero",
                "title": "Built for Speed",
                "lede": "Plain editorial sentence.",
                "background": {"kind": "photo", "src": "/x.jpg", "alt": "a"}}]
        }"#;
        let (ctx, tmp) = make_ctx_with_cms(&[("a", body)]);
        let findings = SlopDictionaryPhase.run(&ctx).expect("run");
        assert_eq!(findings.len(), 1);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn finding_carries_advocacy_structure() {
        let body = r#"{
            "title": "x", "description": "x", "path": "/",
            "sections": [{"kind": "call_to_action",
                "title": "Get Started",
                "cta": {"label": "x", "href": "/y", "data_backend": "z"}}]
        }"#;
        let (ctx, tmp) = make_ctx_with_cms(&[("a", body)]);
        let findings = SlopDictionaryPhase.run(&ctx).expect("run");
        let f = &findings[0];
        assert!(!f.advocacy.why.is_empty());
        assert!(!f.advocacy.substrate_fix.is_empty());
        assert!(f.advocacy.skill.is_some());
        assert!(f.advocacy.anti_pattern.is_some());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn no_cms_dir_no_findings() {
        let tmp = std::env::temp_dir().join(format!(
            "slop-dict-empty-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let ctx = BuildCtx {
            root: tmp.clone(),
            static_dir: tmp.join("static"),
            mode: BuildMode::Poc,
        };
        let findings = SlopDictionaryPhase.run(&ctx).expect("run");
        assert!(findings.is_empty());
    }
}
