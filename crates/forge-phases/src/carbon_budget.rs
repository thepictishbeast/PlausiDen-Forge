//! `carbon_budget` — track per-page transferred-byte total against
//! a declared budget + estimate g CO2 per page-load.
//!
//! Captures ENGINEERING_DISCIPLINES.md §13 + Sustainable Web Design
//! methodology (Wholegrain Digital's website-carbon model). Every
//! page-load has a measurable carbon cost; sites that hit performance
//! budgets correlate with sites that hit carbon budgets, so this
//! phase serves both sustainability AND performance.
//!
//! ## Configuration
//!
//! Reads `[carbon_budget]` from `forge.toml`:
//!
//! ```toml
//! [carbon_budget]
//! # Hard cap on per-page transferred bytes (HTML + linked CSS/JS/
//! # images/fonts referenced from <link>, <script>, <img>, <video>,
//! # <audio>, @font-face). Counts what a cold-cache visitor downloads.
//! kb_per_page = 200
//!
//! # Severity policy:
//! # - "strict" → fail builds that exceed budget (any mode)
//! # - "production_only" → Strict in production mode, Warn in poc
//! # - "warn" → always Warn, never block
//! severity = "production_only"
//!
//! # Optional: skip specific pages from the check.
//! skip_pages = ["embeds/widget.html"]
//!
//! # Optional emission factor (kWh per GB transferred). Defaults
//! # to 0.81 (current global grid mix per Sustainable Web Design
//! # 2024 model). Lower for greener grids (e.g. Norway 0.04).
//! kwh_per_gb = 0.81
//! ```
//!
//! Missing `[carbon_budget]` section → silent skip.
//!
//! ## Severity
//!
//! - Page exceeds `kb_per_page` budget → Strict OR Warn per `severity`
//!   policy. Message includes:
//!   * Total bytes (HTML + linked assets the cold-cache visitor pulls)
//!   * Budget + overage in KB
//!   * Estimated grams CO2 per page-load
//!   * The 3 heaviest assets to investigate first
//! - Asset referenced by a page but not present in static_dir → Warn
//!   (would be a 404; phase_unbuilt_route covers internal navigation,
//!   this catches asset-link drift specifically)
//!
//! ## Carbon math (Sustainable Web Design v4)
//!
//! Per-pageload bytes × 0.81 kWh/GB × 442 g CO2/kWh = grams CO2.
//! Crude but useful as a relative metric; lower is always better.
//! Per-tenant declared emission_factor allows grid-aware accuracy.

use std::collections::HashSet;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, BuildMode, Finding, Phase};

use crate::html_walk::walk_html;

/// `carbon_budget` phase.
#[derive(Debug, Default)]
pub struct CarbonBudgetPhase;

impl Phase for CarbonBudgetPhase {
    fn name(&self) -> &'static str {
        "carbon_budget"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let Some(cfg) = forge_toml_carbon_budget(&ctx.root) else {
            tracing::debug!("carbon_budget: no [carbon_budget] section — skip");
            return Ok(vec![]);
        };
        let budget_bytes = cfg.kb_per_page.saturating_mul(1024);
        let severity = cfg.effective_severity(ctx.mode);

        let mut findings = Vec::new();
        let pages = walk_html(&ctx.static_dir, self.name())?;
        for page in &pages {
            if cfg.skip_pages.iter().any(|s| s == &page.name) {
                continue;
            }
            let html_bytes = page.body.len() as u64;
            let refs = extract_asset_refs(&page.body);
            let mut assets: Vec<AssetSize> = Vec::with_capacity(refs.len() + 1);
            assets.push(AssetSize {
                path: page.name.clone(),
                bytes: html_bytes,
            });
            for r in &refs {
                let local = match resolve_asset(&ctx.static_dir, &page.name, r) {
                    Some(p) => p,
                    None => continue,
                };
                let size = match std::fs::metadata(&local) {
                    Ok(md) if md.is_file() => md.len(),
                    _ => {
                        findings.push(Finding::warn(
                            self.name(),
                            page.name.clone(),
                            format!(
                                "asset reference {r} → static/{rel} not found \
                                 on disk; cold-cache visitors would 404",
                                rel = local
                                    .strip_prefix(&ctx.static_dir)
                                    .unwrap_or(&local)
                                    .display()
                            ),
                        ));
                        continue;
                    }
                };
                assets.push(AssetSize {
                    path: r.clone(),
                    bytes: size,
                });
            }
            let total: u64 = assets.iter().map(|a| a.bytes).sum();
            if total > budget_bytes {
                let over = total - budget_bytes;
                let grams_co2 = estimate_grams_co2(total, cfg.kwh_per_gb);
                let heaviest = top_n_heaviest(&assets, 3);
                let msg = format!(
                    "page total {total_kb:.1} KB over budget {budget_kb} KB \
                     (+{over_kb:.1} KB / ~{co2:.2} g CO2 per cold-cache load). \
                     Heaviest: {heaviest}",
                    total_kb = total as f64 / 1024.0,
                    budget_kb = cfg.kb_per_page,
                    over_kb = over as f64 / 1024.0,
                    co2 = grams_co2,
                    heaviest = heaviest
                        .iter()
                        .map(|a| format!("{} ({:.1}KB)", a.path, a.bytes as f64 / 1024.0))
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                findings.push(match severity {
                    EffectiveSeverity::Strict => {
                        Finding::strict(self.name(), page.name.clone(), msg)
                    }
                    EffectiveSeverity::Warn => Finding::warn(self.name(), page.name.clone(), msg),
                });
            }
        }
        Ok(findings)
    }
}

/// Per-page asset reference (HTML body + linked URL or path).
struct AssetSize {
    path: String,
    bytes: u64,
}

/// Pull every `href`, `src`, `srcset`, and CSS `url(...)` from
/// the page body that points at a same-origin / relative asset.
/// Skips external URLs (scheme present), data: URIs, fragments,
/// and protocol-relative refs.
fn extract_asset_refs(body: &str) -> Vec<String> {
    let mut out: HashSet<String> = HashSet::new();
    for attr in ["href=\"", "src=\"", "srcset=\""] {
        let needle = attr;
        let mut search = body;
        while let Some(idx) = search.find(needle) {
            let after = &search[idx + needle.len()..];
            let Some(end) = after.find('"') else {
                break;
            };
            let raw = &after[..end];
            if attr == "srcset=\"" {
                for piece in raw.split(',') {
                    let url = piece.trim().split_whitespace().next().unwrap_or("");
                    if is_relative_asset(url) {
                        out.insert(url.to_owned());
                    }
                }
            } else if is_relative_asset(raw) {
                out.insert(raw.to_owned());
            }
            search = &after[end + 1..];
        }
    }
    // Inline CSS url(...) references (basic)
    let mut s = body;
    while let Some(i) = s.find("url(") {
        let rest = &s[i + 4..];
        let Some(end) = rest.find(')') else {
            break;
        };
        let raw = rest[..end].trim().trim_matches('"').trim_matches('\'');
        if is_relative_asset(raw) {
            out.insert(raw.to_owned());
        }
        s = &rest[end + 1..];
    }
    out.into_iter().collect()
}

/// True if `href` is a same-origin relative path. Mirrors the
/// RFC 3986 scheme detection from `unbuilt_route`: skip absolute
/// URLs (any `scheme:`), data: URIs, javascript: URIs, fragments,
/// protocol-relative `//host`.
fn is_relative_asset(href: &str) -> bool {
    if href.is_empty() || href.starts_with('#') || href.starts_with("//") {
        return false;
    }
    // RFC 3986 scheme detect
    let mut chars = href.chars();
    let first = match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => c,
        _ => return true,
    };
    let _ = first;
    for c in chars {
        if c == ':' {
            return false;
        }
        if !(c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
            return true;
        }
    }
    true
}

/// Resolve `asset_ref` against `page_name`'s directory under
/// `static_dir`. Handles `/abs/path` as root-relative, `./rel`
/// + bare `rel` as page-directory-relative.
fn resolve_asset(
    static_dir: &Path,
    page_name: &str,
    asset_ref: &str,
) -> Option<std::path::PathBuf> {
    // Strip query + fragment for filesystem resolution
    let (path, _) = asset_ref.split_once('?').unwrap_or((asset_ref, ""));
    let (path, _) = path.split_once('#').unwrap_or((path, ""));
    if path.is_empty() {
        return None;
    }
    let normalized = if let Some(rest) = path.strip_prefix('/') {
        static_dir.join(rest)
    } else {
        // Page-directory-relative
        let page_dir = std::path::Path::new(page_name)
            .parent()
            .unwrap_or_else(|| std::path::Path::new(""));
        let stripped = path.trim_start_matches("./");
        static_dir.join(page_dir).join(stripped)
    };
    Some(normalized)
}

fn top_n_heaviest<'a>(assets: &'a [AssetSize], n: usize) -> Vec<&'a AssetSize> {
    let mut sorted: Vec<&AssetSize> = assets.iter().collect();
    sorted.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    sorted.truncate(n);
    sorted
}

/// Sustainable Web Design v4 model:
///   bytes × (kwh_per_gb / 1_073_741_824) × 442 g CO2/kWh = grams CO2.
fn estimate_grams_co2(bytes: u64, kwh_per_gb: f64) -> f64 {
    let gb = bytes as f64 / 1_073_741_824.0;
    let kwh = gb * kwh_per_gb;
    kwh * 442.0
}

#[derive(Debug, Clone, Default)]
struct CarbonConfig {
    kb_per_page: u64,
    severity: SeverityPolicy,
    skip_pages: Vec<String>,
    kwh_per_gb: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SeverityPolicy {
    Strict,
    #[default]
    ProductionOnly,
    Warn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EffectiveSeverity {
    Strict,
    Warn,
}

impl CarbonConfig {
    fn effective_severity(&self, mode: BuildMode) -> EffectiveSeverity {
        match self.severity {
            SeverityPolicy::Strict => EffectiveSeverity::Strict,
            SeverityPolicy::Warn => EffectiveSeverity::Warn,
            SeverityPolicy::ProductionOnly => match mode {
                BuildMode::Production => EffectiveSeverity::Strict,
                _ => EffectiveSeverity::Warn,
            },
        }
    }
}

fn forge_toml_carbon_budget(root: &Path) -> Option<CarbonConfig> {
    let path = root.join("forge.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    let parsed: toml::Value = content.parse().ok()?;
    let section = parsed.get("carbon_budget")?;
    let kb_per_page = section
        .get("kb_per_page")
        .and_then(|v| v.as_integer())
        .filter(|&n| n > 0)? as u64;
    let severity = match section.get("severity").and_then(|v| v.as_str()) {
        Some("strict") => SeverityPolicy::Strict,
        Some("warn") => SeverityPolicy::Warn,
        _ => SeverityPolicy::ProductionOnly,
    };
    let skip_pages = section
        .get("skip_pages")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let kwh_per_gb = section
        .get("kwh_per_gb")
        .and_then(|v| v.as_float())
        .unwrap_or(0.81);
    Some(CarbonConfig {
        kb_per_page,
        severity,
        skip_pages,
        kwh_per_gb,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::Severity;

    fn ctx_in(dir: &Path, mode: BuildMode) -> BuildCtx {
        BuildCtx {
            root: dir.to_path_buf(),
            static_dir: dir.to_path_buf(),
            mode,
        }
    }

    fn write_forge_toml(dir: &Path, body: &str) {
        std::fs::write(dir.join("forge.toml"), body).unwrap();
    }

    #[test]
    fn no_config_silent_skip() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("page.html"), "<html></html>").unwrap();
        let findings = CarbonBudgetPhase
            .run(&ctx_in(dir.path(), BuildMode::Poc))
            .unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn small_page_under_budget_no_finding() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            "[carbon_budget]\nkb_per_page = 1000\nseverity = \"strict\"\n",
        );
        std::fs::write(dir.path().join("page.html"), "<html>small</html>").unwrap();
        let findings = CarbonBudgetPhase
            .run(&ctx_in(dir.path(), BuildMode::Poc))
            .unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn page_over_budget_emits_strict_when_severity_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            "[carbon_budget]\nkb_per_page = 1\nseverity = \"strict\"\n",
        );
        let big = "x".repeat(2048);
        std::fs::write(dir.path().join("page.html"), format!("<html>{big}</html>")).unwrap();
        let findings = CarbonBudgetPhase
            .run(&ctx_in(dir.path(), BuildMode::Poc))
            .unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Strict);
        assert!(findings[0].message.contains("over budget"));
        assert!(findings[0].message.contains("CO2"));
    }

    #[test]
    fn production_only_severity_strict_in_production_warn_in_poc() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            "[carbon_budget]\nkb_per_page = 1\nseverity = \"production_only\"\n",
        );
        std::fs::write(dir.path().join("page.html"), "x".repeat(3000)).unwrap();

        let poc = CarbonBudgetPhase
            .run(&ctx_in(dir.path(), BuildMode::Poc))
            .unwrap();
        assert_eq!(poc[0].severity, Severity::Warn);

        let prod = CarbonBudgetPhase
            .run(&ctx_in(dir.path(), BuildMode::Production))
            .unwrap();
        assert_eq!(prod[0].severity, Severity::Strict);
    }

    #[test]
    fn warn_severity_never_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            "[carbon_budget]\nkb_per_page = 1\nseverity = \"warn\"\n",
        );
        std::fs::write(dir.path().join("page.html"), "x".repeat(3000)).unwrap();
        let prod = CarbonBudgetPhase
            .run(&ctx_in(dir.path(), BuildMode::Production))
            .unwrap();
        assert_eq!(prod[0].severity, Severity::Warn);
    }

    #[test]
    fn linked_asset_counted_against_budget() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            "[carbon_budget]\nkb_per_page = 2\nseverity = \"strict\"\n",
        );
        std::fs::write(
            dir.path().join("page.html"),
            r#"<html><head><link rel="stylesheet" href="/heavy.css"></head></html>"#,
        )
        .unwrap();
        // 3 KB css file → exceeds 2 KB budget
        std::fs::write(dir.path().join("heavy.css"), "a".repeat(3072)).unwrap();
        let findings = CarbonBudgetPhase
            .run(&ctx_in(dir.path(), BuildMode::Poc))
            .unwrap();
        assert!(findings.iter().any(|f| f.severity == Severity::Strict));
    }

    #[test]
    fn missing_linked_asset_emits_warn() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            "[carbon_budget]\nkb_per_page = 1000\nseverity = \"strict\"\n",
        );
        std::fs::write(
            dir.path().join("page.html"),
            r#"<html><head><link rel="stylesheet" href="/missing.css"></head></html>"#,
        )
        .unwrap();
        let findings = CarbonBudgetPhase
            .run(&ctx_in(dir.path(), BuildMode::Poc))
            .unwrap();
        assert!(findings
            .iter()
            .any(|f| f.severity == Severity::Warn && f.message.contains("not found on disk")));
    }

    #[test]
    fn external_assets_not_counted() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            "[carbon_budget]\nkb_per_page = 1\nseverity = \"strict\"\n",
        );
        // External CDN asset — should NOT count toward budget
        std::fs::write(
            dir.path().join("page.html"),
            r#"<html><script src="https://cdn.example.com/lib.js"></script></html>"#,
        )
        .unwrap();
        let findings = CarbonBudgetPhase
            .run(&ctx_in(dir.path(), BuildMode::Poc))
            .unwrap();
        // The page body itself is tiny, no overage
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn skip_pages_excludes_specific_pages() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[carbon_budget]
kb_per_page = 1
severity = "strict"
skip_pages = ["heavy.html"]
"#,
        );
        std::fs::write(dir.path().join("heavy.html"), "x".repeat(3000)).unwrap();
        std::fs::write(dir.path().join("page.html"), "small").unwrap();
        let findings = CarbonBudgetPhase
            .run(&ctx_in(dir.path(), BuildMode::Poc))
            .unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn extract_asset_refs_handles_relative_paths() {
        let body = r##"<link rel="stylesheet" href="/a.css">
        <script src="b.js"></script>
        <img src="https://external.com/c.png">
        <img src="data:image/png;base64,iVBOR=">
        <a href="#section">x</a>
        <a href="//cdn.example.com/d.js">y</a>"##;
        let refs = extract_asset_refs(body);
        // /a.css and b.js included; external + data + fragment + protocol-relative excluded
        assert!(refs.iter().any(|r| r == "/a.css"));
        assert!(refs.iter().any(|r| r == "b.js"));
        assert!(!refs.iter().any(|r| r.starts_with("https://")));
        assert!(!refs.iter().any(|r| r.starts_with("data:")));
        assert!(!refs.iter().any(|r| r == "#section"));
        assert!(!refs.iter().any(|r| r.starts_with("//cdn")));
    }

    #[test]
    fn srcset_extracted_per_candidate() {
        let body = r#"<img srcset="/a.png 1x, /b.png 2x" src="/a.png">"#;
        let refs = extract_asset_refs(body);
        assert!(refs.iter().any(|r| r == "/a.png"));
        assert!(refs.iter().any(|r| r == "/b.png"));
    }

    #[test]
    fn css_url_extracted() {
        let body = r#"<style>body { background: url(/bg.png); }</style>"#;
        let refs = extract_asset_refs(body);
        assert!(refs.iter().any(|r| r == "/bg.png"));
    }

    #[test]
    fn co2_estimate_monotonic() {
        let small = estimate_grams_co2(1024, 0.81);
        let big = estimate_grams_co2(10 * 1024 * 1024, 0.81);
        assert!(big > small);
        assert!(small > 0.0);
    }

    #[test]
    fn co2_estimate_lower_for_greener_grid() {
        let dirty = estimate_grams_co2(1024 * 1024, 0.81);
        let clean = estimate_grams_co2(1024 * 1024, 0.04);
        assert!(clean < dirty);
        // Specifically: 0.04/0.81 ≈ 0.049 ratio
        assert!((clean / dirty - 0.049).abs() < 0.005);
    }
}
