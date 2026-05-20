//! `composition_lineage` — within-site vocabulary coherence.
//!
//! Task #244 per the variation-architecture spec. Where the
//! pattern_entropy gate (#236) checks that a site uses enough
//! DISTINCT primitives, this phase checks the opposite — that
//! each primitive that IS used keeps a coherent VARIANT vocabulary.
//!
//! ## Why this matters
//!
//! A site can pass entropy + mood gates while still drifting
//! within-primitive: `hero_editorial` with `background=v1` on the
//! home page, `background=v7` on `about`, `background=v3` on
//! `programs` — same primitive, three unrelated treatments. The
//! reader experiences a different site shape on each page.
//!
//! Composition lineage refuses sites where any single primitive
//! kind exceeds the configured per-kind variant budget.
//!
//! ## What it measures
//!
//! Walks cms/*.json, builds `kind → set(variant signature)` map,
//! flags kinds where the set size exceeds the budget. Variant
//! signature is derived the same way the uniqueness_gate computes
//! `PrimitiveOccurrence::variant`:
//!
//! 1. Explicit fields: `variant` / `style` / `tone` / `background`.
//! 2. Count discriminators: `columns` / `tiers` / `items`.
//! 3. Default: empty string (kind-only variant).
//!
//! ## forge.toml config
//!
//! ```toml
//! [composition_lineage]
//! enforce = true
//! # Default per-primitive variant budget.
//! # default_budget = 3
//! # Per-primitive overrides (rare; default budget usually fine).
//! # budget.hero_editorial = 2
//! # budget.kv_pair = 5
//! ```
//!
//! Without `[composition_lineage]` the phase is silent.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * `#[non_exhaustive]` on phase struct.
//! * No unwrap/expect in non-test code.
//! * Pure walk over cms/*.json.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde_json::Value;

/// `composition_lineage` phase implementation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct CompositionLineagePhase;

const DEFAULT_BUDGET: usize = 3;

impl Phase for CompositionLineagePhase {
    fn name(&self) -> &'static str {
        "composition_lineage"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let Some(cfg) = LineageConfig::load(&ctx.root) else {
            return Ok(findings);
        };
        if !cfg.enforce {
            return Ok(findings);
        }
        let cms_dir = ctx.root.join("cms");
        if !cms_dir.is_dir() {
            return Ok(findings);
        }

        let lineage = tally_variants(&cms_dir)?;
        for (kind, variants) in &lineage {
            let budget = cfg.budget_for(kind);
            if variants.len() > budget {
                let listed: Vec<&str> = variants.iter().map(String::as_str).collect();
                findings.push(
                    Finding::strict(
                        self.name(),
                        cms_dir.display().to_string(),
                        format!(
                            "composition_lineage — primitive `{kind}` uses {} distinct variants {:?}; declared budget is {budget}",
                            variants.len(),
                            listed
                        ),
                    )
                    .citing(["pattern-003"])
                    .why("the same primitive shows up with too many different variant signatures across pages; the within-site vocabulary is incoherent and readers experience a different site shape on each page")
                    .fix(format!(
                        "reduce the variant spread on `{kind}`: pick {budget} or fewer variants and use them consistently; OR raise budget.{kind} in forge.toml if the spread is intentional"
                    )),
                );
            }
        }

        Ok(findings)
    }
}

#[derive(Debug, Clone, Default)]
struct LineageConfig {
    enforce: bool,
    default_budget: usize,
    per_kind: BTreeMap<String, usize>,
}

impl LineageConfig {
    fn load(root: &Path) -> Option<Self> {
        let body = fs::read_to_string(root.join("forge.toml")).ok()?;
        let value: toml::Value = toml::from_str(&body).ok()?;
        let section = value.get("composition_lineage")?.as_table()?;
        let enforce = section
            .get("enforce")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let default_budget = section
            .get("default_budget")
            .and_then(|v| v.as_integer())
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(DEFAULT_BUDGET);
        let mut per_kind = BTreeMap::new();
        if let Some(budget_table) = section.get("budget").and_then(|v| v.as_table()) {
            for (kind, value) in budget_table {
                if let Some(n) = value.as_integer().and_then(|n| usize::try_from(n).ok()) {
                    per_kind.insert(kind.clone(), n);
                }
            }
        }
        Some(Self {
            enforce,
            default_budget,
            per_kind,
        })
    }

    fn budget_for(&self, kind: &str) -> usize {
        self.per_kind.get(kind).copied().unwrap_or(self.default_budget)
    }
}

fn tally_variants(cms_dir: &Path) -> Result<BTreeMap<String, BTreeSet<String>>, BuildError> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
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
                    let variant = variant_signature(section);
                    out.entry(kind.to_owned()).or_default().insert(variant);
                }
            }
        }
    }
    Ok(out)
}

/// Mirror of uniqueness_gate's `guess_variant`. Kept independent
/// so the phases don't depend on each other.
fn variant_signature(section: &Value) -> String {
    for field in &["variant", "style", "tone", "kind_detail", "background"] {
        if let Some(s) = section.get(field).and_then(|v| v.as_str()) {
            return format!("{field}={s}");
        }
    }
    if let Some(cols) = section.get("columns").and_then(|v| v.as_u64()) {
        return format!("columns={cols}");
    }
    if let Some(tiers) = section.get("tiers").and_then(|v| v.as_array()) {
        return format!("tiers={}", tiers.len());
    }
    if let Some(items) = section.get("items").and_then(|v| v.as_array()) {
        return format!("items={}", items.len());
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "forge-lineage-{name}-{}",
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
    fn phase_silent_when_no_section() {
        let root = temp_root("silent-no-section");
        write_cms(&root, "i.json", r#"{"sections":[]}"#);
        let findings = CompositionLineagePhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_not_enforced() {
        let root = temp_root("not-enforced");
        fs::write(
            root.join("forge.toml"),
            "[composition_lineage]\nenforce = false\n",
        )
        .unwrap();
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[
              {"kind":"hero_editorial","background":"a"},
              {"kind":"hero_editorial","background":"b"},
              {"kind":"hero_editorial","background":"c"},
              {"kind":"hero_editorial","background":"d"},
              {"kind":"hero_editorial","background":"e"}
            ]}"#,
        );
        let findings = CompositionLineagePhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_variant_explosion_beyond_default_budget() {
        let root = temp_root("explosion");
        fs::write(
            root.join("forge.toml"),
            "[composition_lineage]\nenforce = true\n",
        )
        .unwrap();
        // 5 distinct background variants on the same kind; default
        // budget is 3.
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[
              {"kind":"hero_editorial","background":"a"},
              {"kind":"hero_editorial","background":"b"},
              {"kind":"hero_editorial","background":"c"},
              {"kind":"hero_editorial","background":"d"},
              {"kind":"hero_editorial","background":"e"}
            ]}"#,
        );
        let findings = CompositionLineagePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("`hero_editorial`")
                && f.message.contains("5 distinct variants")),
            "expected hero_editorial variant-explosion finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_within_budget() {
        let root = temp_root("within-budget");
        fs::write(
            root.join("forge.toml"),
            "[composition_lineage]\nenforce = true\n",
        )
        .unwrap();
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[
              {"kind":"hero_editorial","background":"a"},
              {"kind":"hero_editorial","background":"a"},
              {"kind":"hero_editorial","background":"b"},
              {"kind":"paragraph"},
              {"kind":"paragraph"}
            ]}"#,
        );
        let findings = CompositionLineagePhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty(), "within-budget should pass; got: {findings:#?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_respects_per_kind_budget_override() {
        let root = temp_root("override");
        fs::write(
            root.join("forge.toml"),
            r#"
[composition_lineage]
enforce = true

[composition_lineage.budget]
kv_pair = 6
"#,
        )
        .unwrap();
        // 5 distinct items=N variants on kv_pair — under the
        // override budget 6, so it should pass even though it's
        // above the default 3.
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[
              {"kind":"kv_pair","items":[1]},
              {"kind":"kv_pair","items":[1,2]},
              {"kind":"kv_pair","items":[1,2,3]},
              {"kind":"kv_pair","items":[1,2,3,4]},
              {"kind":"kv_pair","items":[1,2,3,4,5]}
            ]}"#,
        );
        let findings = CompositionLineagePhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty(), "per-kind budget should let this pass; got: {findings:#?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn variant_signature_prefers_explicit_field() {
        assert_eq!(
            variant_signature(&serde_json::json!({"variant":"compact"})),
            "variant=compact"
        );
        assert_eq!(
            variant_signature(&serde_json::json!({"columns": 3})),
            "columns=3"
        );
        assert_eq!(
            variant_signature(&serde_json::json!({})),
            ""
        );
    }
}
