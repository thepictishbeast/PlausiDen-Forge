//! `primitive_exhaustion` — per-kind overuse detector.
//!
//! Task #247 per the variation-architecture spec. Complements
//! pattern_entropy (#236): where entropy measures aggregate
//! distribution shape, this phase names SPECIFIC primitives that
//! have become over-used. The substrate's auto-rebalance hint:
//! "primitive X is at 47% of sections; introduce alternatives."
//!
//! ## What it measures
//!
//! Walks cms/*.json, builds the kind → count map, computes each
//! primitive's share of the total section count. Flags any
//! primitive whose share exceeds the configured threshold (default
//! 40%). The finding names the specific over-used primitive so
//! operators know exactly what to vary.
//!
//! ## Why named not aggregate
//!
//! pattern_entropy already refuses sites whose Shannon entropy is
//! too low. That's a single signal — when it fires, the operator
//! must investigate which primitive(s) dominate. This phase short-
//! circuits that investigation by naming the exhausted primitive
//! up front + suggesting concrete alternatives.
//!
//! ## forge.toml shape
//!
//! ```toml
//! [primitive_exhaustion]
//! enforce = true
//! # Per-kind share threshold 0.0..1.0. Default 0.4 = 40%.
//! threshold = 0.4
//! # Minimum total sections before the check fires (avoid false
//! # positives on tiny sites).
//! min_total = 6
//! # Optional per-kind override (e.g. paragraphs are expected
//! # high; relax the threshold for them).
//! # [primitive_exhaustion.threshold_by_kind]
//! # paragraph = 0.7
//! ```
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * `#[non_exhaustive]` on phase struct.
//! * No unwrap/expect in non-test code.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde_json::Value;

/// `primitive_exhaustion` phase implementation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct PrimitiveExhaustionPhase;

const DEFAULT_THRESHOLD: f64 = 0.4;
const DEFAULT_MIN_TOTAL: u64 = 6;

impl Phase for PrimitiveExhaustionPhase {
    fn name(&self) -> &'static str {
        "primitive_exhaustion"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let Some(cfg) = ExhaustionConfig::load(&ctx.root) else {
            return Ok(findings);
        };
        if !cfg.enforce {
            return Ok(findings);
        }

        let cms_dir = ctx.root.join("cms");
        if !cms_dir.is_dir() {
            return Ok(findings);
        }

        let counts = tally_kinds(&cms_dir)?;
        let total: u64 = counts.values().sum();
        if total < cfg.min_total {
            return Ok(findings);
        }

        // Sort kinds by descending share so the most-exhausted
        // primitive's finding appears first in the report.
        let mut ranked: Vec<(&String, u64)> = counts.iter().map(|(k, v)| (k, *v)).collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1));

        for (kind, count) in ranked {
            let share = count as f64 / total as f64;
            let threshold = cfg
                .threshold_by_kind
                .get(kind)
                .copied()
                .unwrap_or(cfg.threshold);
            if share > threshold {
                let alt = suggest_alternative(kind);
                findings.push(
                    Finding::strict(
                        self.name(),
                        cms_dir.display().to_string(),
                        format!(
                            "primitive_exhaustion — primitive `{kind}` at {:.0}% of sections ({count}/{total}) exceeds threshold {:.0}%",
                            share * 100.0,
                            threshold * 100.0
                        ),
                    )
                    .citing(["pattern-401"])
                    .why("a single primitive dominates the composition; the substrate's per-kind exhaustion threshold is breached and readers will perceive monotony before any aggregate-entropy alarm fires")
                    .fix(format!(
                        "introduce alternatives: {alt}; OR raise threshold_by_kind.{kind} in forge.toml if the concentration is intentional"
                    )),
                );
            }
        }

        Ok(findings)
    }
}

#[derive(Debug, Clone)]
struct ExhaustionConfig {
    enforce: bool,
    threshold: f64,
    min_total: u64,
    threshold_by_kind: BTreeMap<String, f64>,
}

impl ExhaustionConfig {
    fn load(root: &Path) -> Option<Self> {
        let body = fs::read_to_string(root.join("forge.toml")).ok()?;
        let value: toml::Value = toml::from_str(&body).ok()?;
        let section = value.get("primitive_exhaustion")?.as_table()?;
        let enforce = section
            .get("enforce")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let threshold = section
            .get("threshold")
            .and_then(|v| v.as_float())
            .unwrap_or(DEFAULT_THRESHOLD);
        let min_total = section
            .get("min_total")
            .and_then(|v| v.as_integer())
            .and_then(|n| u64::try_from(n).ok())
            .unwrap_or(DEFAULT_MIN_TOTAL);
        let mut threshold_by_kind = BTreeMap::new();
        if let Some(table) = section.get("threshold_by_kind").and_then(|v| v.as_table()) {
            for (k, v) in table {
                if let Some(f) = v.as_float() {
                    threshold_by_kind.insert(k.clone(), f);
                }
            }
        }
        Some(Self {
            enforce,
            threshold,
            min_total,
            threshold_by_kind,
        })
    }
}

/// Suggested alternative primitives for the named exhausted
/// kind. Returns a short comma-separated string of substrate-
/// native counterparts.
fn suggest_alternative(kind: &str) -> String {
    let alts: &[&str] = match kind {
        "paragraph" => &["pull_quote", "heading", "kv_pair", "code"],
        "heading" | "sub_heading" => &["pull_quote", "kv_pair", "image_hero"],
        "hero" | "hero_editorial" => &["split_hero", "pull_quote", "kv_pair"],
        "feature_spotlight" => &["kv_pair", "split_hero", "code"],
        "stat_band" => &["sparkline", "histogram", "bar_chart", "pull_stat"],
        "image" | "photo" | "image_hero" | "gallery" => {
            &["pull_quote", "code", "paragraph", "kv_pair"]
        }
        "code" | "terminal" | "code_block" => &["kv_pair", "paragraph", "diagram"],
        "marquee" => &["sparkline", "kv_pair", "image_grid"],
        "kv_pair" => &["pull_quote", "paragraph", "code", "image_hero"],
        "pricing" => &["kv_pair", "table", "paragraph"],
        "testimonial" => &["pull_quote", "paragraph"],
        _ => &["pull_quote", "paragraph", "kv_pair", "heading"],
    };
    alts.iter()
        .map(|s| format!("`{s}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn tally_kinds(cms_dir: &Path) -> Result<BTreeMap<String, u64>, BuildError> {
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let entries = fs::read_dir(cms_dir).map_err(|e| BuildError::Io {
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
        if let Some(sections) = value.get("sections").and_then(|s| s.as_array()) {
            for section in sections {
                if let Some(kind) = section.get("kind").and_then(|v| v.as_str()) {
                    *counts.entry(kind.to_owned()).or_insert(0) += 1;
                }
            }
        }
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let p =
            std::env::temp_dir().join(format!("forge-exhaustion-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("cms")).unwrap();
        p
    }

    fn ctx_for(root: &Path) -> BuildCtx {
        BuildCtx {
            root: root.to_path_buf(),
            static_dir: root.join("static"),
            mode: forge_core::BuildMode::Poc,
        }
    }

    fn write_cms(root: &Path, name: &str, body: &str) {
        fs::write(root.join("cms").join(name), body).unwrap();
    }

    #[test]
    fn phase_silent_when_not_enforced() {
        let root = temp_root("not-enforced");
        fs::write(
            root.join("forge.toml"),
            "[primitive_exhaustion]\nenforce = false\n",
        )
        .unwrap();
        write_cms(&root, "i.json", r#"{"sections":[{"kind":"hero"}]}"#);
        let findings = PrimitiveExhaustionPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_below_min_total() {
        let root = temp_root("below-min");
        fs::write(
            root.join("forge.toml"),
            "[primitive_exhaustion]\nenforce = true\nthreshold = 0.4\nmin_total = 10\n",
        )
        .unwrap();
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[{"kind":"a"},{"kind":"a"},{"kind":"a"}]}"#,
        );
        let findings = PrimitiveExhaustionPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_primitive_above_threshold() {
        let root = temp_root("over-threshold");
        fs::write(
            root.join("forge.toml"),
            "[primitive_exhaustion]\nenforce = true\nthreshold = 0.4\nmin_total = 5\n",
        )
        .unwrap();
        // paragraph 6 / 10 = 60% > 40% threshold.
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[
              {"kind":"paragraph"},{"kind":"paragraph"},{"kind":"paragraph"},
              {"kind":"paragraph"},{"kind":"paragraph"},{"kind":"paragraph"},
              {"kind":"heading"},{"kind":"kv_pair"},
              {"kind":"image"},{"kind":"code"}
            ]}"#,
        );
        let findings = PrimitiveExhaustionPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("`paragraph`") && f.message.contains("60%")),
            "expected paragraph-exhausted finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_distribution_balanced() {
        let root = temp_root("balanced");
        fs::write(
            root.join("forge.toml"),
            "[primitive_exhaustion]\nenforce = true\nthreshold = 0.4\nmin_total = 5\n",
        )
        .unwrap();
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[
              {"kind":"hero_editorial"},
              {"kind":"paragraph"},
              {"kind":"kv_pair"},
              {"kind":"pull_quote"},
              {"kind":"code"},
              {"kind":"image_hero"}
            ]}"#,
        );
        let findings = PrimitiveExhaustionPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.is_empty(),
            "balanced distribution; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_respects_per_kind_threshold_override() {
        let root = temp_root("kind-override");
        fs::write(
            root.join("forge.toml"),
            r#"
[primitive_exhaustion]
enforce = true
threshold = 0.3
min_total = 5

[primitive_exhaustion.threshold_by_kind]
paragraph = 0.7
"#,
        )
        .unwrap();
        // paragraph 5/10 = 50%, exceeds default 0.3 but under
        // per-kind 0.7. heading 3/10 = 30%, exactly at the
        // default — should NOT fire (strict greater-than).
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[
              {"kind":"paragraph"},{"kind":"paragraph"},{"kind":"paragraph"},
              {"kind":"paragraph"},{"kind":"paragraph"},
              {"kind":"heading"},{"kind":"heading"},{"kind":"heading"},
              {"kind":"kv_pair"},{"kind":"code"}
            ]}"#,
        );
        let findings = PrimitiveExhaustionPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            !findings.iter().any(|f| f.message.contains("`paragraph`")),
            "paragraph under override; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn suggest_alternative_returns_substrate_native_options() {
        let p = suggest_alternative("paragraph");
        assert!(p.contains("`pull_quote`"));
        let s = suggest_alternative("stat_band");
        assert!(s.contains("`sparkline`") || s.contains("`histogram`"));
        let h = suggest_alternative("hero");
        assert!(h.contains("`split_hero`"));
    }

    #[test]
    fn ranked_findings_emit_dominant_kind_first() {
        let root = temp_root("ranked");
        fs::write(
            root.join("forge.toml"),
            "[primitive_exhaustion]\nenforce = true\nthreshold = 0.3\nmin_total = 5\n",
        )
        .unwrap();
        // paragraph 5/10, image 4/10 — both exceed 30%. paragraph
        // dominates, should appear first.
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[
              {"kind":"paragraph"},{"kind":"paragraph"},{"kind":"paragraph"},
              {"kind":"paragraph"},{"kind":"paragraph"},
              {"kind":"image"},{"kind":"image"},{"kind":"image"},{"kind":"image"},
              {"kind":"heading"}
            ]}"#,
        );
        let findings = PrimitiveExhaustionPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.len() >= 2);
        assert!(findings[0].message.contains("`paragraph`"));
        let _ = fs::remove_dir_all(&root);
    }
}
