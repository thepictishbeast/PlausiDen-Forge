//! `differentiation_budget` — multi-dimensional aggregate score.
//!
//! Task #237 per the variation-architecture spec. Composes the
//! signals the individual gates surface (pattern entropy + variant
//! diversity + asset-distribution spread) into one weighted score
//! and refuses builds below a declared minimum.
//!
//! Where the per-axis gates (#236 pattern_entropy, #244 composition_
//! lineage) refuse hard-floor violations on a single dimension, the
//! differentiation budget catches the "lukewarm on every axis" case:
//! no single metric tanks below its individual gate, but the
//! aggregate still falls short of editorial discipline.
//!
//! ## Dimensions (v1)
//!
//! Each dimension is normalized to [0, 100]; the weighted sum is
//! the differentiation score.
//!
//! 1. **Primitive entropy** — Shannon entropy normalized over
//!    distinct primitive count (same math as pattern_entropy).
//! 2. **Variant diversity** — average distinct variants per
//!    primitive, capped at the configured target.
//! 3. **Asset spread** — entropy over image / video / interactive /
//!    text-only section counts; captures whether the site reaches
//!    across media types.
//!
//! Future dimensions (additive, non-breaking): voice match,
//! mood alignment, density consistency, theme coverage.
//!
//! ## forge.toml config
//!
//! ```toml
//! [differentiation_budget]
//! enforce = true
//! # Minimum aggregate score 0-100; refuses below.
//! min_score = 55
//! # Optional weights (defaults sum to 1.0).
//! # weight_entropy = 0.5
//! # weight_variant_diversity = 0.3
//! # weight_asset_spread = 0.2
//! # Variant-diversity target — score saturates at this average
//! # distinct-variant count per primitive.
//! # variant_target = 2.5
//! ```
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

/// `differentiation_budget` phase.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct DifferentiationBudgetPhase;

impl Phase for DifferentiationBudgetPhase {
    fn name(&self) -> &'static str {
        "differentiation_budget"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let Some(cfg) = BudgetConfig::load(&ctx.root) else {
            return Ok(findings);
        };
        if !cfg.enforce {
            return Ok(findings);
        }
        let cms_dir = ctx.root.join("cms");
        if !cms_dir.is_dir() {
            return Ok(findings);
        }
        let signals = compute_signals(&cms_dir, &cfg)?;
        if signals.total_sections == 0 {
            return Ok(findings);
        }

        let score = (signals.entropy_score * cfg.weight_entropy
            + signals.variant_score * cfg.weight_variant_diversity
            + signals.asset_score * cfg.weight_asset_spread)
            .round() as u32;

        if score < cfg.min_score {
            let breakdown = format!(
                "entropy={:.0} × {:.2}, variants={:.0} × {:.2}, assets={:.0} × {:.2}",
                signals.entropy_score,
                cfg.weight_entropy,
                signals.variant_score,
                cfg.weight_variant_diversity,
                signals.asset_score,
                cfg.weight_asset_spread,
            );
            findings.push(
                Finding::strict(
                    self.name(),
                    cms_dir.display().to_string(),
                    format!(
                        "differentiation_budget — aggregate score {score} falls below declared minimum {} (breakdown: {breakdown})",
                        cfg.min_score
                    ),
                )
                .citing(["pattern-201"])
                .why("no single axis tanks below its individual gate, but the aggregate differentiation across primitive entropy + variant diversity + asset spread is lukewarm — the substrate's editorial-discipline floor isn't met")
                .fix("raise the weakest signal: add more distinct primitives, vary section variants, OR diversify asset types (image / video / interactive / text)"),
            );
        }

        Ok(findings)
    }
}

#[derive(Debug, Clone)]
struct BudgetConfig {
    enforce: bool,
    min_score: u32,
    weight_entropy: f64,
    weight_variant_diversity: f64,
    weight_asset_spread: f64,
    variant_target: f64,
}

impl BudgetConfig {
    fn load(root: &Path) -> Option<Self> {
        let body = fs::read_to_string(root.join("forge.toml")).ok()?;
        let value: toml::Value = toml::from_str(&body).ok()?;
        let section = value.get("differentiation_budget")?.as_table()?;
        let enforce = section
            .get("enforce")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let min_score = section
            .get("min_score")
            .and_then(|v| v.as_integer())
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(55);
        let weight_entropy = section
            .get("weight_entropy")
            .and_then(|v| v.as_float())
            .unwrap_or(0.5);
        let weight_variant_diversity = section
            .get("weight_variant_diversity")
            .and_then(|v| v.as_float())
            .unwrap_or(0.3);
        let weight_asset_spread = section
            .get("weight_asset_spread")
            .and_then(|v| v.as_float())
            .unwrap_or(0.2);
        let variant_target = section
            .get("variant_target")
            .and_then(|v| v.as_float())
            .unwrap_or(2.5);
        Some(Self {
            enforce,
            min_score,
            weight_entropy,
            weight_variant_diversity,
            weight_asset_spread,
            variant_target,
        })
    }
}

#[derive(Debug, Default)]
struct Signals {
    total_sections: u64,
    entropy_score: f64,
    variant_score: f64,
    asset_score: f64,
}

fn compute_signals(cms_dir: &Path, cfg: &BudgetConfig) -> Result<Signals, BuildError> {
    let mut kind_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut variant_sets: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    // Asset categories: image / video / interactive / text-only.
    let mut asset_bins: [u64; 4] = [0, 0, 0, 0];

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
                let Some(kind) = section.get("kind").and_then(|v| v.as_str()) else {
                    continue;
                };
                *kind_counts.entry(kind.to_owned()).or_insert(0) += 1;
                let variant = variant_signature(section);
                variant_sets
                    .entry(kind.to_owned())
                    .or_default()
                    .insert(variant);
                match asset_bin(kind) {
                    AssetBin::Image => asset_bins[0] += 1,
                    AssetBin::Video => asset_bins[1] += 1,
                    AssetBin::Interactive => asset_bins[2] += 1,
                    AssetBin::Text => asset_bins[3] += 1,
                }
            }
        }
    }

    let total: u64 = kind_counts.values().sum();
    let mut signals = Signals {
        total_sections: total,
        entropy_score: 0.0,
        variant_score: 0.0,
        asset_score: 0.0,
    };
    if total == 0 {
        return Ok(signals);
    }

    // 1. Primitive entropy.
    let entropy_norm = normalized_entropy(&kind_counts);
    signals.entropy_score = entropy_norm * 100.0;

    // 2. Variant diversity — avg distinct-variants per primitive,
    //    scaled to the target.
    let primitive_kinds = variant_sets.len() as f64;
    let total_variants: f64 = variant_sets.values().map(|s| s.len() as f64).sum();
    let avg_variants = if primitive_kinds > 0.0 {
        total_variants / primitive_kinds
    } else {
        0.0
    };
    signals.variant_score = ((avg_variants / cfg.variant_target.max(0.001)) * 100.0).min(100.0);

    // 3. Asset spread — Shannon entropy over the 4 bins.
    let asset_total: u64 = asset_bins.iter().sum();
    if asset_total > 0 {
        let mut bin_counts = BTreeMap::new();
        for (i, c) in asset_bins.iter().enumerate() {
            if *c > 0 {
                bin_counts.insert(i.to_string(), *c);
            }
        }
        signals.asset_score = normalized_entropy(&bin_counts) * 100.0;
    }

    Ok(signals)
}

enum AssetBin {
    Image,
    Video,
    Interactive,
    Text,
}

fn asset_bin(kind: &str) -> AssetBin {
    match kind {
        "image" | "photo" | "gallery" | "image_grid" | "image_hero" | "hero_image" => {
            AssetBin::Image
        }
        "video" | "video_embed" | "video_section" => AssetBin::Video,
        "form" | "interactive" | "code" | "code_block" | "code_playground" | "terminal"
        | "embedded_widget" | "sparkline" | "histogram" | "bar_chart" | "diverging_bar"
        | "boxplot" | "heatmap" | "marquee" | "motion_section" => AssetBin::Interactive,
        _ => AssetBin::Text,
    }
}

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

fn normalized_entropy(counts: &BTreeMap<String, u64>) -> f64 {
    let total: u64 = counts.values().sum();
    if total == 0 {
        return 0.0;
    }
    let n = counts.len();
    if n <= 1 {
        return 0.0;
    }
    let total_f = total as f64;
    let mut h: f64 = 0.0;
    for &c in counts.values() {
        if c == 0 {
            continue;
        }
        let p = (c as f64) / total_f;
        h -= p * p.log2();
    }
    let h_max = (n as f64).log2();
    if h_max <= 0.0 {
        return 0.0;
    }
    (h / h_max).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("forge-budget-{name}-{}", std::process::id()));
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
    fn phase_silent_when_section_absent() {
        let root = temp_root("absent");
        write_cms(&root, "i.json", r#"{"sections":[]}"#);
        let findings = DifferentiationBudgetPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_not_enforced() {
        let root = temp_root("not-enforced");
        fs::write(
            root.join("forge.toml"),
            "[differentiation_budget]\nenforce = false\n",
        )
        .unwrap();
        write_cms(&root, "i.json", r#"{"sections":[{"kind":"hero"}]}"#);
        let findings = DifferentiationBudgetPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_low_aggregate_score() {
        let root = temp_root("low-score");
        fs::write(
            root.join("forge.toml"),
            "[differentiation_budget]\nenforce = true\nmin_score = 60\n",
        )
        .unwrap();
        // Single primitive, no variants, all text — every axis tanks.
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[
              {"kind":"paragraph"},
              {"kind":"paragraph"},
              {"kind":"paragraph"},
              {"kind":"paragraph"}
            ]}"#,
        );
        let findings = DifferentiationBudgetPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("aggregate score")),
            "expected aggregate score finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_well_differentiated() {
        let root = temp_root("differentiated");
        fs::write(
            root.join("forge.toml"),
            "[differentiation_budget]\nenforce = true\nmin_score = 50\n",
        )
        .unwrap();
        // Multiple primitives, distinct variants, multiple asset bins.
        write_cms(
            &root,
            "i.json",
            r#"{"sections":[
              {"kind":"hero_editorial","background":"a"},
              {"kind":"kv_pair","items":[1,2,3]},
              {"kind":"pull_quote","tone":"calm"},
              {"kind":"image_hero"},
              {"kind":"code"},
              {"kind":"paragraph"},
              {"kind":"sparkline"},
              {"kind":"heading"}
            ]}"#,
        );
        let findings = DifferentiationBudgetPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.is_empty(),
            "well-differentiated content should pass; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn asset_bin_classifies_known_kinds() {
        assert!(matches!(asset_bin("image"), AssetBin::Image));
        assert!(matches!(asset_bin("video"), AssetBin::Video));
        assert!(matches!(asset_bin("code"), AssetBin::Interactive));
        assert!(matches!(asset_bin("paragraph"), AssetBin::Text));
        assert!(matches!(asset_bin("hero_editorial"), AssetBin::Text));
    }

    #[test]
    fn normalized_entropy_matches_pattern_entropy_math() {
        let mut m = BTreeMap::new();
        m.insert("a".into(), 5);
        m.insert("b".into(), 5);
        let e = normalized_entropy(&m);
        assert!((e - 1.0).abs() < 1e-9);
    }
}
