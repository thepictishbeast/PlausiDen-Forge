//! `trait_implications` — verify trait implication rules + mutual
//! exclusions across every entity in the trait manifest.
//!
//! Per AVP-Doctrine `TRAIT_DAG.md` § "Implication arrows" + §
//! "Mutual-exclusion": declaring one trait may imply another
//! (e.g. `local` → `private`), and certain pairs are mutually
//! exclusive (e.g. `client-only` ⊕ `server-only`). The substrate
//! refuses to expand implications automatically; declaration is
//! positive per `[[deterministic-first-lfi-optional]]`.
//!
//! This phase reads `trait-manifest.{toml,json}` at the project
//! root, runs `loom_traits::verify_manifest_implications`, and
//! emits a strict finding for every violation.
//!
//! Closes part of `#169 [trait-v4]`. Companion to
//! `trait_consistency` (which enforces default-required sets per
//! entity class).

use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use loom_traits::{verify_manifest_implications, ImplicationViolation, TraitManifest};

/// `trait_implications` phase implementation.
#[derive(Debug, Default)]
pub struct TraitImplicationsPhase;

impl Phase for TraitImplicationsPhase {
    fn name(&self) -> &'static str {
        "trait_implications"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let manifest = match load_manifest(&ctx.root)? {
            Some(m) => m,
            None => {
                tracing::debug!("trait_implications: no manifest; skip");
                return Ok(vec![]);
            }
        };

        let violations = verify_manifest_implications(&manifest);
        let mut findings = Vec::new();
        for v in violations {
            findings.push(violation_to_finding(&v));
        }
        Ok(findings)
    }
}

fn load_manifest(root: &Path) -> Result<Option<TraitManifest>, BuildError> {
    let toml_path = root.join("trait-manifest.toml");
    let json_path = root.join("trait-manifest.json");
    if toml_path.is_file() {
        let text = std::fs::read_to_string(&toml_path).map_err(|e| BuildError::Io {
            context: format!("trait_implications: read {}", toml_path.display()),
            source: e,
        })?;
        let m = toml::from_str(&text).map_err(|e| BuildError::Io {
            context: format!("trait_implications: parse {}: {}", toml_path.display(), e),
            source: std::io::Error::other(e.to_string()),
        })?;
        Ok(Some(m))
    } else if json_path.is_file() {
        let text = std::fs::read_to_string(&json_path).map_err(|e| BuildError::Io {
            context: format!("trait_implications: read {}", json_path.display()),
            source: e,
        })?;
        let m = serde_json::from_str(&text).map_err(|e| BuildError::Io {
            context: format!("trait_implications: parse {}: {}", json_path.display(), e),
            source: std::io::Error::other(e.to_string()),
        })?;
        Ok(Some(m))
    } else {
        Ok(None)
    }
}

fn violation_to_finding(v: &ImplicationViolation) -> Finding {
    match v {
        ImplicationViolation::MissingImpliedTrait { entity_id, trigger, implied } => {
            Finding::strict(
                "trait_implications",
                entity_id.clone(),
                format!(
                    "{entity_id} declares trait `{}` but is missing implied trait `{}`",
                    trigger.slug(),
                    implied.slug()
                ),
            )
            .why(format!(
                "per TRAIT_DAG.md implication rules, declaring `{}` requires also declaring `{}` — the substrate refuses to expand implications automatically (declaration is positive per [[deterministic-first-lfi-optional]])",
                trigger.slug(),
                implied.slug()
            ))
            .fix(format!(
                "add `{}` to the entity's traits list in trait-manifest.{{toml,json}}",
                implied.slug()
            ))
            .skill("add-loom-primitive")
        }
        ImplicationViolation::MutuallyExclusiveBoth { entity_id, first, second } => {
            Finding::strict(
                "trait_implications",
                entity_id.clone(),
                format!(
                    "{entity_id} declares mutually-exclusive traits `{}` AND `{}`",
                    first.slug(),
                    second.slug()
                ),
            )
            .why(format!(
                "traits `{}` and `{}` are mutually exclusive per TRAIT_DAG.md § Mutual-exclusion; an entity declares one OR the other, never both",
                first.slug(),
                second.slug()
            ))
            .fix(format!(
                "remove one of `{}` or `{}` from the entity's traits list",
                first.slug(),
                second.slug()
            ))
            .skill("add-loom-primitive")
        }
    }
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
    fn missing_manifest_silent_skip() {
        let tmp = tempfile::tempdir().unwrap();
        let findings = TraitImplicationsPhase.run(&ctx_in(tmp.path())).unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn clean_manifest_emits_zero_findings() {
        let tmp = tempfile::tempdir().unwrap();
        // CMS section that satisfies its required set + the
        // NoSiteSpecific → SubstrateNative implication.
        let manifest = r#"
[[projections]]
entity_id = "Cms.Section.PullQuote"
entity_class = "cms_section"
traits = ["no-site-specific", "substrate-native", "manifested", "versioned"]
"#;
        std::fs::write(tmp.path().join("trait-manifest.toml"), manifest).unwrap();
        let findings = TraitImplicationsPhase.run(&ctx_in(tmp.path())).unwrap();
        assert!(findings.is_empty(), "expected clean, got: {findings:?}");
    }

    #[test]
    fn missing_implied_trait_emits_finding_with_advocacy() {
        let tmp = tempfile::tempdir().unwrap();
        // Declares `local` without `private` (implication violation).
        let manifest = r#"
[[projections]]
entity_id = "Cms.Section.LocalCache"
entity_class = "cms_section"
traits = ["local", "no-site-specific", "substrate-native", "manifested", "versioned"]
"#;
        std::fs::write(tmp.path().join("trait-manifest.toml"), manifest).unwrap();
        let findings = TraitImplicationsPhase.run(&ctx_in(tmp.path())).unwrap();
        assert_eq!(findings.len(), 1);
        let f = &findings[0];
        assert!(f.message.contains("local"));
        assert!(f.message.contains("private"));
        assert!(!f.advocacy.why.is_empty());
        assert!(!f.advocacy.substrate_fix.is_empty());
        assert_eq!(f.advocacy.skill.as_deref(), Some("add-loom-primitive"));
    }

    #[test]
    fn mutually_exclusive_pair_emits_finding() {
        let tmp = tempfile::tempdir().unwrap();
        let manifest = r#"
[[projections]]
entity_id = "Loom.Primitive.Contradictory"
entity_class = "loom_visible_primitive"
traits = [
  "client-only", "server-only",
  "mobile-friendly", "rtl-aware", "reduced-motion-aware", "theme-aware",
  "no-site-specific", "substrate-native",
  "manifested", "versioned", "doctrine-cited"
]
"#;
        std::fs::write(tmp.path().join("trait-manifest.toml"), manifest).unwrap();
        let findings = TraitImplicationsPhase.run(&ctx_in(tmp.path())).unwrap();
        assert_eq!(findings.len(), 1);
        let f = &findings[0];
        assert!(f.message.contains("mutually-exclusive"));
        assert!(f.message.contains("client-only"));
        assert!(f.message.contains("server-only"));
        assert!(!f.advocacy.why.is_empty());
    }

    #[test]
    fn multiple_violations_emit_multiple_findings() {
        let tmp = tempfile::tempdir().unwrap();
        // Two violations in one projection:
        //   1. amoled-optimized without dark-mode-first
        //   2. fuzz-tested without property-tested
        let manifest = r#"
[[projections]]
entity_id = "Loom.Primitive.Buggy"
entity_class = "cms_section"
traits = [
  "amoled-optimized",
  "fuzz-tested",
  "no-site-specific", "substrate-native",
  "manifested", "versioned"
]
"#;
        std::fs::write(tmp.path().join("trait-manifest.toml"), manifest).unwrap();
        let findings = TraitImplicationsPhase.run(&ctx_in(tmp.path())).unwrap();
        assert_eq!(findings.len(), 2);
        let messages: Vec<&str> = findings.iter().map(|f| f.message.as_str()).collect();
        assert!(messages
            .iter()
            .any(|m| m.contains("amoled-optimized") && m.contains("dark-mode-first")));
        assert!(messages
            .iter()
            .any(|m| m.contains("fuzz-tested") && m.contains("property-tested")));
    }

    #[test]
    fn malformed_manifest_is_buildror() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("trait-manifest.toml"),
            "[[projections\nentity_id = \"x\"\n",
        )
        .unwrap();
        let r = TraitImplicationsPhase.run(&ctx_in(tmp.path()));
        assert!(r.is_err());
    }
}
