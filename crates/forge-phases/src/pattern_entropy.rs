//! `pattern_entropy` — within-site primitive-distribution variation
//! gate.
//!
//! Task #236 per the variation-architecture spec. Where the
//! cross-site uniqueness gate (#233) refuses sites that look like
//! OTHER sites, the pattern-entropy gate refuses sites that look
//! like THEMSELVES too much — pages dominated by one or two
//! primitives, or sites where every page uses the same shape.
//!
//! ## What it measures
//!
//! Shannon entropy on the primitive-kind distribution, normalized
//! to `[0, 1]` against the maximum entropy attainable with the
//! number of distinct primitives in use:
//!
//! ```text
//! H = - Σ p_i log₂(p_i)
//! H_max = log₂(n)  // n = distinct primitive count
//! H_norm = H / H_max  ∈ [0, 1]
//! ```
//!
//! `H_norm = 1` when primitives are perfectly evenly distributed.
//! `H_norm → 0` when one primitive dominates. The default floor
//! is 0.65 — sites where 60%+ of sections are one primitive land
//! under threshold and are refused.
//!
//! ## forge.toml config
//!
//! ```toml
//! [pattern_entropy]
//! enforce = true
//! # min_entropy = 0.65                # default
//! # min_distinct_primitives = 3       # default
//! # scope = "site"                    # or "page"; default "site"
//! ```
//!
//! Without `[pattern_entropy]` the phase is silent.
//!
//! ## Two scopes
//!
//! * `scope = "site"` (default) — entropy computed over the whole
//!   site's primitive distribution. Refuses sites where one
//!   primitive dominates regardless of page.
//! * `scope = "page"` — entropy computed per-page. Refuses ANY
//!   page where one primitive dominates the page's sections.
//!   Stricter; useful for editorial-content sites where each page
//!   must compose multiple primitives.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * `#[non_exhaustive]` on phase struct.
//! * No unwrap/expect in non-test code.
//! * Pure walk over cms/*.json.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde_json::Value;

/// `pattern_entropy` phase implementation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct PatternEntropyPhase;

const DEFAULT_MIN_ENTROPY: f64 = 0.65;
const DEFAULT_MIN_DISTINCT: usize = 3;

impl Phase for PatternEntropyPhase {
    fn name(&self) -> &'static str {
        "pattern_entropy"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let Some(cfg) = EntropyConfig::load(&ctx.root) else {
            return Ok(findings);
        };
        if !cfg.enforce {
            return Ok(findings);
        }
        let cms_dir = ctx.root.join("cms");
        if !cms_dir.is_dir() {
            return Ok(findings);
        }

        let mut site_counts: BTreeMap<String, u64> = BTreeMap::new();
        let mut per_page: Vec<(String, BTreeMap<String, u64>)> = Vec::new();

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
            let mut page_counts: BTreeMap<String, u64> = BTreeMap::new();
            if let Some(sections) = value.get("sections").and_then(|s| s.as_array()) {
                for section in sections {
                    let Some(kind) = section.get("kind").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    *site_counts.entry(kind.to_owned()).or_insert(0) += 1;
                    *page_counts.entry(kind.to_owned()).or_insert(0) += 1;
                }
            }
            per_page.push((path_disp, page_counts));
        }

        match cfg.scope {
            EntropyScope::Site => {
                check_distribution(
                    &site_counts,
                    &cfg,
                    cms_dir.display().to_string(),
                    "site",
                    &mut findings,
                    self.name(),
                );
            }
            EntropyScope::Page => {
                for (path, counts) in &per_page {
                    if counts.is_empty() {
                        continue;
                    }
                    check_distribution(
                        counts,
                        &cfg,
                        path.clone(),
                        "page",
                        &mut findings,
                        self.name(),
                    );
                }
            }
        }

        Ok(findings)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntropyScope {
    Site,
    Page,
}

#[derive(Debug, Clone)]
struct EntropyConfig {
    enforce: bool,
    min_entropy: f64,
    min_distinct: usize,
    scope: EntropyScope,
}

impl EntropyConfig {
    fn load(root: &Path) -> Option<Self> {
        let body = fs::read_to_string(root.join("forge.toml")).ok()?;
        let value: toml::Value = toml::from_str(&body).ok()?;
        let section = value.get("pattern_entropy")?.as_table()?;
        let enforce = section
            .get("enforce")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let min_entropy = section
            .get("min_entropy")
            .and_then(|v| v.as_float())
            .unwrap_or(DEFAULT_MIN_ENTROPY);
        let min_distinct = section
            .get("min_distinct_primitives")
            .and_then(|v| v.as_integer())
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(DEFAULT_MIN_DISTINCT);
        let scope = match section
            .get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("site")
        {
            "page" => EntropyScope::Page,
            _ => EntropyScope::Site,
        };
        Some(Self {
            enforce,
            min_entropy,
            min_distinct,
            scope,
        })
    }
}

fn check_distribution(
    counts: &BTreeMap<String, u64>,
    cfg: &EntropyConfig,
    where_at: String,
    scope_label: &str,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let total: u64 = counts.values().sum();
    if total == 0 {
        return;
    }
    let distinct = counts.len();

    // Minimum-distinct check.
    if distinct < cfg.min_distinct {
        let dominant_kinds: Vec<&str> = counts.keys().map(String::as_str).collect();
        findings.push(
            Finding::strict(
                phase,
                where_at.clone(),
                format!(
                    "pattern_entropy — {} scope uses only {} distinct primitive(s) ({:?}); declared minimum is {}",
                    scope_label, distinct, dominant_kinds, cfg.min_distinct
                ),
            )
            .citing(["pattern-001"])
            .why("the site/page lacks compositional variety; using only one or two primitive kinds yields a monotonous reading experience")
            .fix(format!(
                "add more primitive kinds to this {scope_label}; aim for at least {} distinct kinds to clear the gate",
                cfg.min_distinct
            )),
        );
    }

    let entropy_norm = normalized_entropy(counts);
    if entropy_norm < cfg.min_entropy {
        // Identify the dominant primitive for the error message.
        let (dominant, dominant_count) = counts
            .iter()
            .max_by_key(|(_, c)| **c)
            .map(|(k, c)| (k.as_str(), *c))
            .unwrap_or(("?", 0));
        let dominance_pct = if total == 0 {
            0.0
        } else {
            (dominant_count as f64 * 100.0) / (total as f64)
        };
        findings.push(
            Finding::strict(
                phase,
                where_at,
                format!(
                    "pattern_entropy — {} scope normalized entropy is {:.3} (declared min {:.3}); primitive `{}` dominates with {:.0}% of sections ({}/{})",
                    scope_label, entropy_norm, cfg.min_entropy, dominant, dominance_pct, dominant_count, total
                ),
            )
            .citing(["pattern-002"])
            .why("one primitive dominates the composition; readers experience monotony and the site fails the within-site variation guarantee")
            .fix(format!(
                "reduce the count of `{}` sections OR add more counterweight primitives until normalized entropy clears {:.3}",
                dominant, cfg.min_entropy
            )),
        );
    }
}

/// Normalized Shannon entropy of a primitive-kind distribution.
/// Returns 1.0 for perfectly even distributions, → 0 as one
/// primitive dominates. Returns 0.0 for empty input.
fn normalized_entropy(counts: &BTreeMap<String, u64>) -> f64 {
    let total: u64 = counts.values().sum();
    if total == 0 {
        return 0.0;
    }
    let n = counts.len();
    if n <= 1 {
        // Single primitive — entropy is 0 by convention.
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
        let p = std::env::temp_dir().join(format!("forge-entropy-{name}-{}", std::process::id()));
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
        let findings = PatternEntropyPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_enforce_false() {
        let root = temp_root("not-enforced");
        fs::write(
            root.join("forge.toml"),
            "[pattern_entropy]\nenforce = false\n",
        )
        .unwrap();
        write_cms(&root, "i.json", r#"{"sections":[{"kind":"hero"}]}"#);
        let findings = PatternEntropyPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_low_distinct_primitive_count() {
        let root = temp_root("low-distinct");
        fs::write(
            root.join("forge.toml"),
            "[pattern_entropy]\nenforce = true\nmin_distinct_primitives = 4\n",
        )
        .unwrap();
        // Only 2 distinct primitives — fails min_distinct=4.
        write_cms(
            &root,
            "index.json",
            r#"{"sections":[
              {"kind":"hero_editorial"},
              {"kind":"paragraph"},
              {"kind":"paragraph"},
              {"kind":"paragraph"}
            ]}"#,
        );
        let findings = PatternEntropyPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("distinct primitive(s)")),
            "expected distinct-count finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_low_entropy_when_one_primitive_dominates() {
        let root = temp_root("dominates");
        fs::write(
            root.join("forge.toml"),
            "[pattern_entropy]\nenforce = true\nmin_entropy = 0.6\nmin_distinct_primitives = 1\n",
        )
        .unwrap();
        // 8 paragraph + 1 hero ≈ 89% dominance → normalized entropy ≈ 0.503
        // over 2 distinct kinds; below the 0.6 threshold.
        write_cms(
            &root,
            "index.json",
            r#"{"sections":[
              {"kind":"paragraph"},{"kind":"paragraph"},{"kind":"paragraph"},
              {"kind":"paragraph"},{"kind":"paragraph"},{"kind":"paragraph"},
              {"kind":"paragraph"},{"kind":"paragraph"},
              {"kind":"hero_editorial"}
            ]}"#,
        );
        let findings = PatternEntropyPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("entropy")),
            "expected entropy finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_distribution_is_even() {
        let root = temp_root("even");
        fs::write(
            root.join("forge.toml"),
            "[pattern_entropy]\nenforce = true\nmin_entropy = 0.65\nmin_distinct_primitives = 3\n",
        )
        .unwrap();
        // 3 distinct primitives evenly distributed.
        write_cms(
            &root,
            "index.json",
            r#"{"sections":[
              {"kind":"hero_editorial"},
              {"kind":"kv_pair"},
              {"kind":"pull_quote"},
              {"kind":"hero_editorial"},
              {"kind":"kv_pair"},
              {"kind":"pull_quote"}
            ]}"#,
        );
        let findings = PatternEntropyPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.is_empty(),
            "even distribution should pass; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn page_scope_flags_individual_pages() {
        let root = temp_root("page-scope");
        fs::write(
            root.join("forge.toml"),
            "[pattern_entropy]\nenforce = true\nscope = \"page\"\nmin_entropy = 0.5\nmin_distinct_primitives = 2\n",
        )
        .unwrap();
        // Page A: dominated by one primitive (fails).
        // Page B: even (passes).
        write_cms(
            &root,
            "a.json",
            r#"{"sections":[{"kind":"paragraph"},{"kind":"paragraph"},{"kind":"paragraph"}]}"#,
        );
        write_cms(
            &root,
            "b.json",
            r#"{"sections":[{"kind":"hero_editorial"},{"kind":"kv_pair"}]}"#,
        );
        let findings = PatternEntropyPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.path.contains("a.json")),
            "expected page-a finding; got: {findings:#?}"
        );
        assert!(
            !findings.iter().any(|f| f.path.contains("b.json")),
            "page b should be silent; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn normalized_entropy_is_one_for_perfectly_even() {
        let mut m = BTreeMap::new();
        m.insert("a".to_string(), 3);
        m.insert("b".to_string(), 3);
        m.insert("c".to_string(), 3);
        let e = normalized_entropy(&m);
        assert!((e - 1.0).abs() < 1e-9, "expected 1.0 got {e}");
    }

    #[test]
    fn normalized_entropy_is_zero_for_single_primitive() {
        let mut m = BTreeMap::new();
        m.insert("a".to_string(), 10);
        assert_eq!(normalized_entropy(&m), 0.0);
    }

    #[test]
    fn normalized_entropy_is_zero_for_empty() {
        let m: BTreeMap<String, u64> = BTreeMap::new();
        assert_eq!(normalized_entropy(&m), 0.0);
    }
}
