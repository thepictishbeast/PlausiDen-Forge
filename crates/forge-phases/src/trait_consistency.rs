//! `trait_consistency` — verify every entity in the trait manifest
//! declares its entity-class default-required traits.
//!
//! Per AVP-Doctrine `TRAIT_DAG.md` § Default-required traits per
//! entity class + `[[manifest-layer-is-the-keystone]]`: the trait
//! manifest (`trait-manifest.toml` or `.json`) projects every
//! entity's declared trait set. This phase reads the manifest at
//! the project root, runs `loom_traits::verify_manifest`, and emits
//! a finding for every required-but-missing trait.
//!
//! Closes part of `#171 [trait-v6]` (the CI consistency check
//! piece; the manifest projection types live in `loom-traits`).
//!
//! Missing trait-manifest = silent skip (sites that haven't opted
//! into the contract aren't gated yet). Once `#168` lands and
//! primitives declare their traits, the absent-manifest case may
//! be promoted to a warn.

use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use loom_traits::{verify_manifest, TraitManifest};

/// `trait_consistency` phase implementation.
#[derive(Debug, Default)]
pub struct TraitConsistencyPhase;

impl Phase for TraitConsistencyPhase {
    fn name(&self) -> &'static str {
        "trait_consistency"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        // Try TOML first; fall back to JSON. Either is a valid
        // manifest format per VERSION_DISCIPLINE.md.
        let toml_path = ctx.root.join("trait-manifest.toml");
        let json_path = ctx.root.join("trait-manifest.json");

        let manifest = if toml_path.is_file() {
            load_toml(&toml_path)?
        } else if json_path.is_file() {
            load_json(&json_path)?
        } else {
            tracing::debug!(
                "trait_consistency: no manifest at {} or {}; skip",
                toml_path.display(),
                json_path.display()
            );
            return Ok(vec![]);
        };

        let missing = verify_manifest(&manifest);
        let mut findings = Vec::new();
        for m in missing {
            findings.push(
                Finding::strict(
                    "trait_consistency",
                    m.entity_id.clone(),
                    format!(
                        "{} is missing required trait `{}`",
                        m.entity_id,
                        m.required.slug()
                    ),
                )
                .why(format!(
                    "entity class declares `{}` as a default-required trait per TRAIT_DAG.md; absent declaration means the substrate cannot enforce the invariant",
                    m.required.slug()
                ))
                .fix(format!(
                    "add `{}` to the entity's traits list in trait-manifest.{{toml,json}} — OR if the trait genuinely doesn't apply, escalate via capability-request to amend the entity-class required set",
                    m.required.slug()
                ))
                .skill("add-loom-primitive"),
            );
        }
        Ok(findings)
    }
}

fn load_toml(path: &Path) -> Result<TraitManifest, BuildError> {
    let text = std::fs::read_to_string(path).map_err(|e| BuildError::Io {
        context: format!("trait_consistency: read {}", path.display()),
        source: e,
    })?;
    toml::from_str(&text).map_err(|e| BuildError::Io {
        context: format!("trait_consistency: parse {}: {}", path.display(), e),
        source: std::io::Error::other(e.to_string()),
    })
}

fn load_json(path: &Path) -> Result<TraitManifest, BuildError> {
    let text = std::fs::read_to_string(path).map_err(|e| BuildError::Io {
        context: format!("trait_consistency: read {}", path.display()),
        source: e,
    })?;
    serde_json::from_str(&text).map_err(|e| BuildError::Io {
        context: format!("trait_consistency: parse {}: {}", path.display(), e),
        source: std::io::Error::other(e.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::BuildMode;

    fn ctx_in(dir: &Path) -> BuildCtx {
        BuildCtx {
            root: dir.to_path_buf(),
            static_dir: dir.to_path_buf(),
            mode: BuildMode::Poc,
        }
    }

    #[test]
    fn missing_manifest_is_silent_skip() {
        let tmp = tempfile::tempdir().unwrap();
        let findings = TraitConsistencyPhase.run(&ctx_in(tmp.path())).unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn clean_manifest_emits_zero_findings() {
        let tmp = tempfile::tempdir().unwrap();
        let manifest = r#"
schema_version = "1.0.0"

[[projections]]
entity_id = "Loom.Primitive.Hero"
entity_class = "loom_visible_primitive"
traits = [
  "mobile-friendly",
  "rtl-aware",
  "reduced-motion-aware",
  "theme-aware",
  "no-site-specific",
  "manifested",
  "versioned",
  "doctrine-cited",
]
"#;
        std::fs::write(tmp.path().join("trait-manifest.toml"), manifest).unwrap();
        let findings = TraitConsistencyPhase.run(&ctx_in(tmp.path())).unwrap();
        assert!(
            findings.is_empty(),
            "expected clean, got: {:?}",
            findings.iter().map(|f| &f.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn missing_required_trait_emits_finding_with_advocacy() {
        let tmp = tempfile::tempdir().unwrap();
        // Sloppy primitive declares only 2 of 8 required.
        let manifest = r#"
[[projections]]
entity_id = "Loom.Primitive.SloppyHero"
entity_class = "loom_visible_primitive"
traits = ["mobile-friendly", "rtl-aware"]
"#;
        std::fs::write(tmp.path().join("trait-manifest.toml"), manifest).unwrap();
        let findings = TraitConsistencyPhase.run(&ctx_in(tmp.path())).unwrap();
        // 6 missing required traits.
        assert_eq!(findings.len(), 6);
        // Every finding cites SloppyHero + has advocacy populated.
        for f in &findings {
            assert_eq!(f.path, "Loom.Primitive.SloppyHero");
            assert!(!f.advocacy.why.is_empty());
            assert!(!f.advocacy.substrate_fix.is_empty());
            assert_eq!(f.advocacy.skill.as_deref(), Some("add-loom-primitive"));
        }
    }

    #[test]
    fn json_manifest_also_supported() {
        let tmp = tempfile::tempdir().unwrap();
        let manifest = r#"{
            "schema_version": "1.0.0",
            "projections": [{
                "entity_id": "Forge.Phase.Test",
                "entity_class": "forge_phase",
                "traits": ["doctrine-cited", "property-tested", "fails-closed"]
            }]
        }"#;
        std::fs::write(tmp.path().join("trait-manifest.json"), manifest).unwrap();
        let findings = TraitConsistencyPhase.run(&ctx_in(tmp.path())).unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn malformed_manifest_is_buildror() {
        let tmp = tempfile::tempdir().unwrap();
        // Invalid TOML — missing closing bracket.
        std::fs::write(
            tmp.path().join("trait-manifest.toml"),
            "[[projections\nentity_id = \"x\"\n",
        )
        .unwrap();
        let r = TraitConsistencyPhase.run(&ctx_in(tmp.path()));
        assert!(r.is_err());
    }

    #[test]
    fn toml_preferred_over_json_when_both_present() {
        let tmp = tempfile::tempdir().unwrap();
        // TOML is clean.
        let toml_manifest = r#"
[[projections]]
entity_id = "Toml.Entity"
entity_class = "cms_section"
traits = ["no-site-specific", "manifested", "versioned"]
"#;
        // JSON is dirty (missing required traits).
        let json_manifest = r#"{
            "projections": [{
                "entity_id": "Json.Entity",
                "entity_class": "cms_section",
                "traits": []
            }]
        }"#;
        std::fs::write(tmp.path().join("trait-manifest.toml"), toml_manifest).unwrap();
        std::fs::write(tmp.path().join("trait-manifest.json"), json_manifest).unwrap();
        let findings = TraitConsistencyPhase.run(&ctx_in(tmp.path())).unwrap();
        // If TOML was preferred + parsed clean, we get 0 findings.
        // (Behavior contract: when both present, TOML wins.)
        assert!(
            findings.is_empty(),
            "expected TOML preference; got JSON findings"
        );
    }
}
