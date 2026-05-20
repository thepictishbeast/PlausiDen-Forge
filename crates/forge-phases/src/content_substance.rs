//! `content_substance` — flags scaffolded-but-unauthored CMS.
//!
//! Where the variation-arc gates check the SHAPE of the
//! composition (primitives, distribution, voice envelope),
//! this phase checks whether each section's CONTENT FIELDS are
//! actually filled in with substance — not just placeholder
//! length or scaffolded defaults.
//!
//! Catches the failure mode where a site passes every variation
//! gate (right primitives, right voice tier, right mood) while
//! READING as empty because the operator scaffolded the section
//! and never wrote real content.
//!
//! ## What it checks
//!
//! Per section, the phase asserts substance-floor for each
//! known kind. Defaults (operator-overridable per
//! `[content_substance]` in forge.toml):
//!
//! * `hero_editorial.title` ≥ 20 chars
//! * `hero_editorial.lede` ≥ 60 chars (when present)
//! * `paragraph.body` ≥ 80 chars
//! * `pull_quote.body` ≥ 40 chars
//! * `kv_pair.items` ≥ 3 entries
//! * `code.body` ≥ 20 chars
//! * `heading.title` ≥ 8 chars
//! * `call_to_action.label` ≥ 4 chars
//!
//! Section with an empty/missing required field → strict finding
//! by default (warn if `[content_substance] strict = false`).
//!
//! ## forge.toml
//!
//! ```toml
//! [content_substance]
//! enforce = true
//! strict = true                    # default; false → warn
//! # Override defaults per-kind:
//! # min_chars.paragraph.body = 200
//! # min_count.kv_pair.items = 5
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

/// `content_substance` phase implementation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct ContentSubstancePhase;

use forge_core::content_substance::{DEFAULT_MIN_CHARS, DEFAULT_MIN_COUNTS};

impl Phase for ContentSubstancePhase {
    fn name(&self) -> &'static str {
        "content_substance"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let Some(cfg) = SubstanceConfig::load(&ctx.root) else {
            return Ok(findings);
        };
        if !cfg.enforce {
            return Ok(findings);
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
            let path_disp = path.display().to_string();
            if let Some(sections) = value.get("sections").and_then(|s| s.as_array()) {
                for (idx, section) in sections.iter().enumerate() {
                    let Some(kind) = section.get("kind").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    let where_at = format!("{path_disp}#section-{idx}-{kind}");
                    check_section(kind, section, &where_at, &cfg, &mut findings, self.name());
                }
            }
        }

        Ok(findings)
    }
}

fn check_section(
    kind: &str,
    section: &Value,
    where_at: &str,
    cfg: &SubstanceConfig,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    for (target_kind, field, default_min) in DEFAULT_MIN_CHARS {
        if *target_kind != kind {
            continue;
        }
        let min_chars = cfg
            .min_chars
            .get(&(target_kind.to_string(), field.to_string()))
            .copied()
            .unwrap_or(*default_min);
        let actual = section
            .get(*field)
            .and_then(|v| v.as_str())
            .map(|s| s.chars().count() as u32)
            .unwrap_or(0);
        if actual < min_chars {
            push_finding(
                cfg.strict,
                phase,
                where_at,
                format!(
                    "content_substance — `{kind}.{field}` has {actual} chars; substance floor is {min_chars} (scaffolded-but-unauthored?)"
                ),
                "the section passes the variation arc's primitive-choice check but the content field is empty or stub-length — readers will perceive the page as scaffolded, not written",
                format!("write substantive {field} content (≥ {min_chars} chars) OR remove the section if it's not load-bearing for this page"),
                findings,
            );
        }
    }
    for (target_kind, field, default_min) in DEFAULT_MIN_COUNTS {
        if *target_kind != kind {
            continue;
        }
        let min_count = cfg
            .min_counts
            .get(&(target_kind.to_string(), field.to_string()))
            .copied()
            .unwrap_or(*default_min);
        let actual = section
            .get(*field)
            .and_then(|v| v.as_array())
            .map(|a| a.len() as u32)
            .unwrap_or(0);
        if actual < min_count {
            push_finding(
                cfg.strict,
                phase,
                where_at,
                format!(
                    "content_substance — `{kind}.{field}` has {actual} entries; substance floor is {min_count}"
                ),
                "list/grid section needs enough entries to read as deliberate; below the floor it reads as scaffolded",
                format!("populate {field} with at least {min_count} entries OR remove the section"),
                findings,
            );
        }
    }
}

fn push_finding(
    strict: bool,
    phase: &'static str,
    where_at: &str,
    message: String,
    why: &'static str,
    fix: String,
    findings: &mut Vec<Finding>,
) {
    let f = if strict {
        Finding::strict(phase, where_at.to_owned(), message)
    } else {
        Finding::warn(phase, where_at.to_owned(), message)
    };
    findings.push(f.citing(["substance-001"]).why(why).fix(fix));
}

#[derive(Debug, Clone)]
struct SubstanceConfig {
    enforce: bool,
    strict: bool,
    min_chars: BTreeMap<(String, String), u32>,
    min_counts: BTreeMap<(String, String), u32>,
}

impl SubstanceConfig {
    fn load(root: &Path) -> Option<Self> {
        let body = fs::read_to_string(root.join("forge.toml")).ok()?;
        let value: toml::Value = toml::from_str(&body).ok()?;
        let section = value.get("content_substance")?.as_table()?;
        let enforce = section
            .get("enforce")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let strict = section
            .get("strict")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let mut min_chars: BTreeMap<(String, String), u32> = BTreeMap::new();
        if let Some(table) = section.get("min_chars").and_then(|v| v.as_table()) {
            for (kind, inner) in table {
                if let Some(field_table) = inner.as_table() {
                    for (field, val) in field_table {
                        if let Some(n) = val.as_integer().and_then(|n| u32::try_from(n).ok()) {
                            min_chars.insert((kind.clone(), field.clone()), n);
                        }
                    }
                }
            }
        }
        let mut min_counts: BTreeMap<(String, String), u32> = BTreeMap::new();
        if let Some(table) = section.get("min_count").and_then(|v| v.as_table()) {
            for (kind, inner) in table {
                if let Some(field_table) = inner.as_table() {
                    for (field, val) in field_table {
                        if let Some(n) = val.as_integer().and_then(|n| u32::try_from(n).ok()) {
                            min_counts.insert((kind.clone(), field.clone()), n);
                        }
                    }
                }
            }
        }
        Some(Self {
            enforce,
            strict,
            min_chars,
            min_counts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("forge-substance-{name}-{}", std::process::id()));
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
    fn phase_silent_when_no_section() {
        let root = temp_root("no-section");
        write_cms(&root, "i.json", r#"{"sections":[]}"#);
        let findings = ContentSubstancePhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_not_enforced() {
        let root = temp_root("not-enforced");
        fs::write(
            root.join("forge.toml"),
            "[content_substance]\nenforce = false\n",
        )
        .unwrap();
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[{"kind":"paragraph","body":"x"}]}"#,
        );
        let findings = ContentSubstancePhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_short_paragraph_body() {
        let root = temp_root("short-paragraph");
        fs::write(
            root.join("forge.toml"),
            "[content_substance]\nenforce = true\n",
        )
        .unwrap();
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[{"kind":"paragraph","body":"too short"}]}"#,
        );
        let findings = ContentSubstancePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("`paragraph.body`")),
            "expected paragraph.body finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_paragraph_body_meets_floor() {
        let root = temp_root("good-paragraph");
        fs::write(
            root.join("forge.toml"),
            "[content_substance]\nenforce = true\n",
        )
        .unwrap();
        write_cms(
            &root,
            "i.json",
            &serde_json::json!({"sections":[{"kind":"paragraph","body":"x".repeat(100)}]})
                .to_string(),
        );
        let findings = ContentSubstancePhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty(), "should pass; got: {findings:#?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_short_hero_title() {
        let root = temp_root("short-hero");
        fs::write(
            root.join("forge.toml"),
            "[content_substance]\nenforce = true\n",
        )
        .unwrap();
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[{"kind":"hero_editorial","title":"Hi"}]}"#,
        );
        let findings = ContentSubstancePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("`hero_editorial.title`")),
            "expected hero_editorial.title finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_under_count_kv_pair_items() {
        let root = temp_root("kv-short");
        fs::write(
            root.join("forge.toml"),
            "[content_substance]\nenforce = true\n",
        )
        .unwrap();
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[{"kind":"kv_pair","items":[{"k":"a","v":"b"}]}]}"#,
        );
        let findings = ContentSubstancePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("`kv_pair.items`")),
            "expected kv_pair.items finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_respects_per_field_min_chars_override() {
        let root = temp_root("override-chars");
        fs::write(
            root.join("forge.toml"),
            r#"
[content_substance]
enforce = true

[content_substance.min_chars.paragraph]
body = 200
"#,
        )
        .unwrap();
        write_cms(
            &root,
            "i.json",
            &serde_json::json!({"sections":[{"kind":"paragraph","body":"x".repeat(150)}]})
                .to_string(),
        );
        let findings = ContentSubstancePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("substance floor is 200")),
            "expected override-floor finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn strict_false_escalates_to_warn() {
        let root = temp_root("warn-mode");
        fs::write(
            root.join("forge.toml"),
            "[content_substance]\nenforce = true\nstrict = false\n",
        )
        .unwrap();
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[{"kind":"paragraph","body":"x"}]}"#,
        );
        let findings = ContentSubstancePhase.run(&ctx_for(&root)).unwrap();
        assert!(!findings.is_empty());
        for f in &findings {
            assert_eq!(
                f.severity,
                forge_core::Severity::Warn,
                "expected warn-severity; got: {}",
                f.message
            );
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_missing_field_as_zero_chars() {
        let root = temp_root("missing-field");
        fs::write(
            root.join("forge.toml"),
            "[content_substance]\nenforce = true\n",
        )
        .unwrap();
        // paragraph without body field at all.
        write_cms(&root, "i.json", r#"{"sections":[{"kind":"paragraph"}]}"#);
        let findings = ContentSubstancePhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.iter().any(|f| f.message.contains("0 chars")));
        let _ = fs::remove_dir_all(&root);
    }
}
