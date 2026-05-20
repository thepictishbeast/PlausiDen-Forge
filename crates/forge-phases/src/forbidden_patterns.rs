//! `forbidden_patterns` — tenant-extensible anti-template dictionary.
//!
//! Task #251 per the variation-architecture spec. Where the
//! editorial_purity_gate (12 hard-coded SaaS tropes) refuses a
//! fixed substrate-wide set of shapes, this phase reads an
//! arbitrary list of `[[forbidden_composition_pattern]]` entries
//! from `forge.toml` and refuses any CMS section that matches.
//!
//! Lets each tenant declare its own anti-patterns. Useful for:
//!
//! * Banning shapes the substrate doesn't catch (e.g.
//!   "no `heading` with `level = 'h6'`" — too deep is a code-smell
//!   for this tenant).
//! * Migrating off a deprecated primitive ("no `legacy_card`").
//! * Per-tenant editorial discipline beyond the substrate default.
//!
//! Per `[[per-tenant-corpora-doctrine]]` doctrine: tenants ADD
//! rules; they don't relax substrate baselines. This phase only
//! refuses; it never allows.
//!
//! ## forge.toml shape
//!
//! ```toml
//! [[forbidden_composition_pattern]]
//! id = "no-h6-headings"
//! kind = "heading"                      # required: section kind to match
//! when_field = "level"                  # optional: field to check
//! when_value = "h6"                     # optional: required value
//! description = "h6 is too deep for our editorial register"
//!
//! [[forbidden_composition_pattern]]
//! id = "no-pricing-with-most-popular"
//! kind = "pricing"
//! when_field = "highlighted_tier"
//! when_value_present = true             # optional: refuse when field is present at all
//! description = "Drop the 'most popular' upsell"
//! ```
//!
//! Match semantics:
//! - `kind` is required and matches the section's `kind` field
//!   exactly.
//! - If `when_field` is absent, every section with that kind is
//!   refused.
//! - If `when_field` is set + `when_value` is set, the field's
//!   string value must match exactly.
//! - If `when_field` is set + `when_value_present = true`, the
//!   field's mere presence (any non-null value) triggers the match.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * `#[non_exhaustive]` on phase struct.
//! * No unwrap/expect in non-test code.
//! * Pure walk over cms/*.json.

use std::fs;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde::Deserialize;
use serde_json::Value;

/// `forbidden_patterns` phase implementation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct ForbiddenPatternsPhase;

impl Phase for ForbiddenPatternsPhase {
    fn name(&self) -> &'static str {
        "forbidden_patterns"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let Some(patterns) = ForbiddenPatterns::load(&ctx.root) else {
            return Ok(findings);
        };
        if patterns.list.is_empty() {
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
                    for p in &patterns.list {
                        if p.matches(kind, section) {
                            let where_at = format!("{path_disp}#section-{idx}-{kind}");
                            findings.push(
                                Finding::strict(
                                    self.name(),
                                    where_at,
                                    format!(
                                        "forbidden_patterns — `{}` matched: {}",
                                        p.id,
                                        p.description
                                    ),
                                )
                                .citing(["pattern-101"])
                                .why("the tenant has declared this composition shape forbidden in forge.toml; this section matches the pattern")
                                .fix("either remove/replace this section, OR drop the matching [[forbidden_composition_pattern]] entry if the rule no longer applies"),
                            );
                        }
                    }
                }
            }
        }

        Ok(findings)
    }
}

#[derive(Debug, Default, Deserialize)]
struct ForbiddenPatterns {
    #[serde(default, rename = "forbidden_composition_pattern")]
    list: Vec<ForbiddenPattern>,
}

#[derive(Debug, Deserialize)]
struct ForbiddenPattern {
    id: String,
    kind: String,
    #[serde(default)]
    when_field: Option<String>,
    #[serde(default)]
    when_value: Option<String>,
    #[serde(default)]
    when_value_present: bool,
    #[serde(default)]
    description: String,
}

impl ForbiddenPattern {
    fn matches(&self, kind: &str, section: &Value) -> bool {
        if kind != self.kind {
            return false;
        }
        let Some(field) = self.when_field.as_deref() else {
            return true;
        };
        let value = section.get(field);
        match (&self.when_value, self.when_value_present, value) {
            (Some(expected), _, Some(v)) => v.as_str().map_or(false, |s| s == expected),
            (None, true, Some(v)) => !v.is_null(),
            (None, true, None) => false,
            (None, false, _) => true,
            (Some(_), _, None) => false,
        }
    }
}

impl ForbiddenPatterns {
    fn load(root: &Path) -> Option<Self> {
        let body = fs::read_to_string(root.join("forge.toml")).ok()?;
        toml::from_str(&body).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("forge-forbidden-{name}-{}", std::process::id()));
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
    fn phase_silent_when_no_patterns_declared() {
        let root = temp_root("none");
        write_cms(&root, "i.json", r#"{"sections":[{"kind":"hero"}]}"#);
        let findings = ForbiddenPatternsPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_refuses_unconditional_kind_match() {
        let root = temp_root("kind-only");
        fs::write(
            root.join("forge.toml"),
            r#"
[[forbidden_composition_pattern]]
id = "no-legacy-card"
kind = "legacy_card"
description = "Deprecated; migrate to kv_pair"
"#,
        )
        .unwrap();
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[
              {"kind":"hero_editorial"},
              {"kind":"legacy_card","title":"X"},
              {"kind":"paragraph"}
            ]}"#,
        );
        let findings = ForbiddenPatternsPhase.run(&ctx_for(&root)).unwrap();
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("`no-legacy-card`"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_refuses_when_field_value_matches() {
        let root = temp_root("value-match");
        fs::write(
            root.join("forge.toml"),
            r#"
[[forbidden_composition_pattern]]
id = "no-h6"
kind = "heading"
when_field = "level"
when_value = "h6"
description = "too deep for the editorial register"
"#,
        )
        .unwrap();
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[
              {"kind":"heading","level":"h2"},
              {"kind":"heading","level":"h6"},
              {"kind":"heading"}
            ]}"#,
        );
        let findings = ForbiddenPatternsPhase.run(&ctx_for(&root)).unwrap();
        assert_eq!(findings.len(), 1, "only the h6 matches; got {findings:#?}");
        assert!(findings[0].message.contains("`no-h6`"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_refuses_when_value_present_with_any_value() {
        let root = temp_root("presence");
        fs::write(
            root.join("forge.toml"),
            r#"
[[forbidden_composition_pattern]]
id = "no-pricing-highlights"
kind = "pricing"
when_field = "highlighted_tier"
when_value_present = true
description = "Drop the most-popular upsell"
"#,
        )
        .unwrap();
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[
              {"kind":"pricing","tiers":[]},
              {"kind":"pricing","highlighted_tier":"pro"}
            ]}"#,
        );
        let findings = ForbiddenPatternsPhase.run(&ctx_for(&root)).unwrap();
        assert_eq!(findings.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn pattern_match_logic_handles_all_combinations() {
        // kind mismatch.
        let p = ForbiddenPattern {
            id: "x".into(),
            kind: "hero".into(),
            when_field: None,
            when_value: None,
            when_value_present: false,
            description: String::new(),
        };
        assert!(!p.matches("paragraph", &serde_json::json!({})));
        assert!(p.matches("hero", &serde_json::json!({})));

        // field + value mismatch.
        let p = ForbiddenPattern {
            id: "x".into(),
            kind: "h".into(),
            when_field: Some("level".into()),
            when_value: Some("h6".into()),
            when_value_present: false,
            description: String::new(),
        };
        assert!(p.matches("h", &serde_json::json!({"level":"h6"})));
        assert!(!p.matches("h", &serde_json::json!({"level":"h2"})));
        assert!(!p.matches("h", &serde_json::json!({})));

        // presence-only.
        let p = ForbiddenPattern {
            id: "x".into(),
            kind: "p".into(),
            when_field: Some("ad_slot".into()),
            when_value: None,
            when_value_present: true,
            description: String::new(),
        };
        assert!(p.matches("p", &serde_json::json!({"ad_slot":"top"})));
        assert!(!p.matches("p", &serde_json::json!({})));
    }

    #[test]
    fn phase_emits_multiple_findings_when_multiple_patterns_hit() {
        let root = temp_root("multi");
        fs::write(
            root.join("forge.toml"),
            r#"
[[forbidden_composition_pattern]]
id = "no-h6"
kind = "heading"
when_field = "level"
when_value = "h6"
description = "x"

[[forbidden_composition_pattern]]
id = "no-legacy"
kind = "legacy_card"
description = "y"
"#,
        )
        .unwrap();
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[
              {"kind":"heading","level":"h6"},
              {"kind":"legacy_card"},
              {"kind":"heading","level":"h6"}
            ]}"#,
        );
        let findings = ForbiddenPatternsPhase.run(&ctx_for(&root)).unwrap();
        assert_eq!(findings.len(), 3, "expected 3 findings; got {findings:#?}");
        let _ = fs::remove_dir_all(&root);
    }
}
