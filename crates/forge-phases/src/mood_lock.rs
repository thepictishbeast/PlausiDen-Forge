//! `mood_lock` — aesthetic-mood drift detection.
//!
//! Task #243 per the variation-architecture spec. Where the
//! site_identity_conformance phase (#235) checks declared
//! primitives, this phase checks the AGGREGATE AESTHETIC of the
//! site's primitive usage against the declared mood lock.
//!
//! ## What this phase checks
//!
//! Given a declared [`site_identity.mood`] (primary + optional
//! secondary + drift_budget), the phase walks cms/*.json,
//! tallies primitive usage, and scores the site's actual
//! aesthetic signal against each declared mood's affinity
//! profile. If actual drift exceeds `drift_budget` (0-100, where
//! 0 = strict lock, 100 = descriptive only), a strict finding
//! refuses the build.
//!
//! ## Mood affinity profiles
//!
//! Each mood declares which primitives ALIGN with it (positive
//! affinity) and which CONTRADICT it (negative affinity). The
//! site's mood score for each declared mood is:
//!
//! ```text
//! S = (Σ aligned-primitive uses) / total_uses
//! C = (Σ contradicting-primitive uses) / total_uses
//! drift = round(C * 100) - round(S * 0)  // C dominates
//! ```
//!
//! Lower is better; 0 means no contradicting primitives. The
//! drift_budget caps how much contradiction the operator tolerates.
//!
//! ## Supported moods (v1)
//!
//! * `editorial` — long-form, asymmetric, monospace-kicker
//! * `industrial` — code + terminal + KV info panels
//! * `organic` — image-driven, photographic, soft
//! * `minimal` — typography-driven, sparse, no decoration
//! * `kinetic` — motion, sparklines, marquees
//! * `archival` — timeline, table, citation, dated
//! * `playful` — gallery, emoji, marquee, color-rich
//! * `severe` — typography, KV, no soft decoration
//!
//! Unknown mood strings produce no affinity — silent skip
//! (back-compat).
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * `#[non_exhaustive]` on phase struct.
//! * No unwrap/expect in non-test code.
//! * Pure walk over cms/*.json.
//! * Integer-only mood scoring to avoid f64 non-determinism
//!   beyond a single division.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use forge_core::site_identity::SiteIdentity;
use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde_json::Value;

/// `mood_lock` phase implementation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct MoodLockPhase;

impl Phase for MoodLockPhase {
    fn name(&self) -> &'static str {
        "mood_lock"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let Some(identity) = SiteIdentity::load(&ctx.root) else {
            return Ok(findings);
        };
        let Some(primary) = identity.mood.primary.as_deref() else {
            return Ok(findings);
        };
        let affinity = MoodAffinity::for_mood(primary);
        if affinity.is_none() {
            // Unknown mood — silent skip per back-compat.
            return Ok(findings);
        }
        let Some(primary_affinity) = affinity else {
            return Ok(findings);
        };

        let cms_dir = ctx.root.join("cms");
        if !cms_dir.is_dir() {
            return Ok(findings);
        }

        let counts = tally_primitives(&cms_dir)?;
        let total: u64 = counts.values().sum();
        if total == 0 {
            return Ok(findings);
        }

        let drift_primary = compute_drift(&counts, total, &primary_affinity);
        let budget = u32::from(identity.mood.drift_budget);
        if drift_primary > budget {
            // Build a witness — the worst-offending contradicting
            // primitive so the operator knows where to act.
            let worst = worst_contradictor(&counts, &primary_affinity);
            findings.push(
                Finding::strict(
                    self.name(),
                    cms_dir.display().to_string(),
                    format!(
                        "mood_lock — primary mood `{}` drift {} exceeds declared budget {}; primitive `{}` contradicts the declared aesthetic most strongly",
                        primary, drift_primary, budget, worst.unwrap_or("?")
                    ),
                )
                .citing(["theme-001"])
                .why("the site declared a mood lock but the actual primitive composition contradicts the declared aesthetic; readers experience a different mood than the operator intended")
                .fix(format!(
                    "remove or replace `{}`-style primitives, OR switch [site_identity.mood].primary to a mood whose affinity profile matches the actual composition, OR raise drift_budget if the contradiction is intentional",
                    worst.unwrap_or("contradicting")
                )),
            );
        }

        // Optional secondary mood check — if declared, secondary
        // gets the same scoring but with a relaxed cap (2x budget)
        // since secondary is a blend, not a primary.
        if let Some(secondary) = identity.mood.secondary.as_deref() {
            if let Some(secondary_affinity) = MoodAffinity::for_mood(secondary) {
                let drift_secondary = compute_drift(&counts, total, &secondary_affinity);
                let secondary_budget = budget.saturating_mul(2);
                if drift_secondary > secondary_budget {
                    findings.push(
                        Finding::strict(
                            self.name(),
                            cms_dir.display().to_string(),
                            format!(
                                "mood_lock — secondary mood `{}` drift {} exceeds doubled budget {}; declared secondary blend not present in actual composition",
                                secondary, drift_secondary, secondary_budget
                            ),
                        )
                        .citing(["theme-002"])
                        .why("a secondary mood was declared as a blend element but the actual primitive composition contradicts it; the declared blend isn't expressed")
                        .fix("either align the composition with the declared blend OR drop the secondary mood from [site_identity.mood]"),
                    );
                }
            }
        }

        Ok(findings)
    }
}

/// Per-mood affinity profile.
struct MoodAffinity {
    aligned: &'static [&'static str],
    contradicting: &'static [&'static str],
}

impl MoodAffinity {
    fn for_mood(mood: &str) -> Option<Self> {
        let (aligned, contradicting) = match mood {
            "editorial" => (
                &[
                    "hero_editorial",
                    "pull_quote",
                    "paragraph",
                    "heading",
                    "sub_heading",
                    "kv_pair",
                    "photo",
                    "image_hero",
                    "section_heading",
                ][..],
                &[
                    "hero",
                    "feature_spotlight",
                    "stat_band",
                    "pricing",
                    "testimonial",
                    "logo_wall",
                    "marquee",
                    "cookie_notice",
                ][..],
            ),
            "industrial" => (
                &[
                    "split_hero",
                    "code",
                    "terminal",
                    "kv_pair",
                    "code_block",
                    "paragraph",
                    "heading",
                ][..],
                &[
                    "testimonial",
                    "pricing",
                    "marquee",
                    "logo_wall",
                    "image_hero",
                    "stat_band",
                ][..],
            ),
            "organic" => (
                &[
                    "photo",
                    "image_hero",
                    "image_grid",
                    "gallery",
                    "pull_quote",
                    "paragraph",
                    "section_heading",
                ][..],
                &[
                    "stat_band",
                    "pricing",
                    "code",
                    "terminal",
                    "feature_spotlight",
                ][..],
            ),
            "minimal" => (
                &[
                    "heading",
                    "paragraph",
                    "sub_heading",
                    "section_heading",
                    "pull_quote",
                ][..],
                &[
                    "gallery",
                    "feature_spotlight",
                    "stat_band",
                    "marquee",
                    "logo_wall",
                    "testimonial",
                    "pricing",
                ][..],
            ),
            "kinetic" => (
                &[
                    "marquee",
                    "motion_section",
                    "sparkline",
                    "histogram",
                    "bar_chart",
                    "diverging_bar",
                ][..],
                &[
                    "paragraph",
                    "kv_pair",
                    "table",
                    "timeline",
                    "citation",
                ][..],
            ),
            "archival" => (
                &[
                    "timeline",
                    "table",
                    "paragraph",
                    "citation",
                    "heading",
                    "pull_quote",
                    "section_heading",
                ][..],
                &[
                    "marquee",
                    "feature_spotlight",
                    "pricing",
                    "stat_band",
                    "motion_section",
                ][..],
            ),
            "playful" => (
                &[
                    "gallery",
                    "image_grid",
                    "marquee",
                    "emoji_band",
                    "photo",
                    "image_hero",
                ][..],
                &[
                    "code",
                    "terminal",
                    "table",
                    "timeline",
                    "citation",
                ][..],
            ),
            "severe" => (
                &[
                    "heading",
                    "paragraph",
                    "kv_pair",
                    "sub_heading",
                    "section_heading",
                    "code",
                    "table",
                ][..],
                &[
                    "gallery",
                    "pricing",
                    "marquee",
                    "stat_band",
                    "logo_wall",
                    "testimonial",
                ][..],
            ),
            _ => return None,
        };
        Some(Self {
            aligned,
            contradicting,
        })
    }
}

fn compute_drift(counts: &BTreeMap<String, u64>, total: u64, affinity: &MoodAffinity) -> u32 {
    if total == 0 {
        return 0;
    }
    let mut contradicting: u64 = 0;
    for kind in affinity.contradicting {
        if let Some(c) = counts.get(*kind) {
            contradicting = contradicting.saturating_add(*c);
        }
    }
    // Drift = contradicting share as a percentage 0..=100.
    let pct = (contradicting.saturating_mul(100) / total).min(100);
    u32::try_from(pct).unwrap_or(100)
}

fn worst_contradictor<'a>(
    counts: &BTreeMap<String, u64>,
    affinity: &'a MoodAffinity,
) -> Option<&'a str> {
    affinity
        .contradicting
        .iter()
        .filter_map(|kind| counts.get(*kind).map(|c| (*kind, *c)))
        .max_by_key(|(_, c)| *c)
        .map(|(k, _)| k)
}

fn tally_primitives(cms_dir: &Path) -> Result<BTreeMap<String, u64>, BuildError> {
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
        let p = std::env::temp_dir().join(format!(
            "forge-mood-{name}-{}",
            std::process::id()
        ));
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
    fn phase_silent_when_no_mood_declared() {
        let root = temp_root("no-mood");
        write_cms(&root, "i.json", r#"{"sections":[{"kind":"hero"}]}"#);
        let findings = MoodLockPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_for_unknown_mood() {
        let root = temp_root("unknown");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity.mood]
primary = "florescent"
drift_budget = 0
"#,
        )
        .unwrap();
        write_cms(&root, "i.json", r#"{"sections":[{"kind":"hero"}]}"#);
        let findings = MoodLockPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty(), "unknown mood should be silent; got: {findings:#?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_editorial_drift_when_saas_tropes_dominate() {
        let root = temp_root("editorial-drift");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity.mood]
primary = "editorial"
drift_budget = 10
"#,
        )
        .unwrap();
        // 1 editorial-aligned + 4 contradicting = 80% contradicting,
        // far above the 10 budget.
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[
              {"kind":"hero_editorial"},
              {"kind":"feature_spotlight"},
              {"kind":"stat_band"},
              {"kind":"pricing"},
              {"kind":"testimonial"}
            ]}"#,
        );
        let findings = MoodLockPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("primary mood `editorial`")),
            "expected editorial drift finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_composition_matches_declared_mood() {
        let root = temp_root("aligned");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity.mood]
primary = "editorial"
drift_budget = 10
"#,
        )
        .unwrap();
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[
              {"kind":"hero_editorial"},
              {"kind":"paragraph"},
              {"kind":"pull_quote"},
              {"kind":"kv_pair"},
              {"kind":"paragraph"}
            ]}"#,
        );
        let findings = MoodLockPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty(), "aligned composition should pass; got: {findings:#?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_names_worst_contradicting_primitive_in_message() {
        let root = temp_root("worst");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity.mood]
primary = "minimal"
drift_budget = 5
"#,
        )
        .unwrap();
        // 4 marquees + 1 paragraph; minimal's worst contradictor
        // is the most-used contradicting primitive.
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[
              {"kind":"marquee"},
              {"kind":"marquee"},
              {"kind":"marquee"},
              {"kind":"marquee"},
              {"kind":"paragraph"}
            ]}"#,
        );
        let findings = MoodLockPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("`marquee`")),
            "expected marquee named as worst contradictor; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn compute_drift_is_zero_for_empty() {
        let counts = BTreeMap::new();
        let aff = MoodAffinity::for_mood("editorial").unwrap();
        assert_eq!(compute_drift(&counts, 0, &aff), 0);
    }

    #[test]
    fn compute_drift_caps_at_100() {
        let mut counts = BTreeMap::new();
        counts.insert("pricing".into(), 100);
        let aff = MoodAffinity::for_mood("editorial").unwrap();
        assert_eq!(compute_drift(&counts, 100, &aff), 100);
    }
}
