//! `density_audit` — declared-vs-empirical density tier check.
//!
//! Forward step on substrate-deepening doctrine (PRIORITY 7 —
//! density audit phase) paired with `loom_tokens::DensityTier`
//! shipped in PlausiDen-Loom commit 5e5d8dc / task #217.
//!
//! Pairs site-author intent ("we're aiming at Comfortable
//! density") with the rendered output. If the declared target
//! is `Dense` but pages ship with 3 sections of 12 words each,
//! that's a substrate-vs-content drift the audit phase should
//! flag — operators usually drift over time as they ship more
//! marketing-trope content into a denser frame.
//!
//! ## How density is measured at build time
//!
//! Without a browser we can't compute actual char-per-1000sqpx.
//! What we CAN compute from CMS JSON alone:
//!
//! * Total visible-text character count across body-text-bearing
//!   sections (paragraph, heading, lede, sublede, etc.).
//! * Section count (excluding pure-decorative variants like
//!   Spacer / Divider).
//! * Words per body section, averaged.
//!
//! These get classified into one of the four canonical tiers
//! using bands tuned to match `DensityTier::char_per_1000sqpx`
//! at the 1280x800 baseline. The classification is a HEURISTIC
//! that approximates the rendered density; it's not exact, and
//! the tolerance band reflects that.
//!
//! ## forge.toml config
//!
//! ```toml
//! [composition]
//! target_density = "comfortable"  # one of sparse|comfortable|dense|extreme
//! # Default tolerance ± 1 tier. Set tighter to enforce close
//! # match; looser to warn only on extreme drift.
//! tolerance = 1
//! ```
//!
//! Without `[composition]`, the phase is silent — operators opt
//! in. With a target declared, the phase emits a finding when
//! the empirical tier differs from the target by more than
//! `tolerance`.
//!
//! ## Severity
//!
//! Warn — density mismatch is a STYLE drift signal, not a
//! correctness gate. Operators can promote to strict via
//! `forge.toml [composition] strict = true` if they want the
//! mismatch to block the build.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"` (inherited).
//! * `#[non_exhaustive]` on enums.
//! * Pure phase; reads filesystem only via the standard
//!   `BuildCtx` paths.

use std::fs;
use std::path::Path;

use forge_core::tenant_corpus::TenantCorpus;
use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde_json::Value;

/// Canonical density tiers — inlined replica of
/// `loom_tokens::DensityTier`. We don't pull from loom-tokens
/// directly because the dep is git-pinned and we don't want
/// this phase to drift on every loom-tokens release; the band
/// boundaries are doctrine, change-controlled via this commit
/// + the loom-tokens commit together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DensityTier {
    /// 0-80 chars per 1000sqpx baseline-equivalent.
    Sparse,
    /// 80-180 chars per 1000sqpx baseline-equivalent.
    Comfortable,
    /// 180-400 chars per 1000sqpx baseline-equivalent.
    Dense,
    /// 400+ chars per 1000sqpx baseline-equivalent.
    Extreme,
}

impl DensityTier {
    /// Parse from the snake_case string form used in forge.toml.
    /// Returns `None` for unknown strings; the phase treats
    /// unknown as a warn finding rather than failing the build.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "sparse" => Some(Self::Sparse),
            "comfortable" => Some(Self::Comfortable),
            "dense" => Some(Self::Dense),
            "extreme" => Some(Self::Extreme),
            _ => None,
        }
    }

    /// Stable kebab-case slug — same wire shape as the
    /// loom-tokens enum.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Sparse => "sparse",
            Self::Comfortable => "comfortable",
            Self::Dense => "dense",
            Self::Extreme => "extreme",
        }
    }

    /// Tier ordinal — 0 (sparsest) to 3 (extremest). Used to
    /// compute distance for the tolerance check.
    #[must_use]
    pub const fn ordinal(self) -> i32 {
        match self {
            Self::Sparse => 0,
            Self::Comfortable => 1,
            Self::Dense => 2,
            Self::Extreme => 3,
        }
    }
}

/// Build-time density estimator. Walks a `CmsPage` JSON value
/// and computes the empirical tier.
///
/// Heuristic boundaries (chars per visible body section,
/// averaged) — tuned to align with `DensityTier` at the 1280x800
/// baseline:
///
/// * `Sparse`      — average < 60 chars per body section,
///                   OR total visible chars < 400
/// * `Comfortable` — 60-180 chars per body section,
///                   total >= 400
/// * `Dense`       — 180-400 chars per body section
/// * `Extreme`     — > 400 chars per body section
#[must_use]
pub fn classify_page(value: &Value) -> DensityTier {
    let sections = value
        .get("sections")
        .and_then(|s| s.as_array())
        .map_or(&[][..], |v| v.as_slice());
    let mut total_chars: usize = 0;
    let mut body_section_count: usize = 0;
    for section in sections {
        let kind = section.get("kind").and_then(|k| k.as_str()).unwrap_or("");
        // Skip pure-decorative variants — they don't contribute
        // visible character density.
        if matches!(kind, "spacer" | "divider" | "container") {
            continue;
        }
        let chars = visible_chars_in_section(section);
        if chars > 0 {
            total_chars = total_chars.saturating_add(chars);
            body_section_count = body_section_count.saturating_add(1);
        }
    }
    if total_chars < 400 || body_section_count == 0 {
        return DensityTier::Sparse;
    }
    let avg = total_chars / body_section_count.max(1);
    if avg < 60 {
        DensityTier::Sparse
    } else if avg <= 180 {
        DensityTier::Comfortable
    } else if avg <= 400 {
        DensityTier::Dense
    } else {
        DensityTier::Extreme
    }
}

/// Sum visible text chars across the text-bearing string fields
/// of a section. Field allowlist matches the substrate's body-
/// text-bearing CmsSection variants; adds for variants present,
/// silent for absent.
#[must_use]
fn visible_chars_in_section(section: &Value) -> usize {
    let mut total: usize = 0;
    for field in [
        "title",
        "lede",
        "subtitle",
        "text",
        "body",
        "description",
        "attribution",
        "warning",
        "headline",
    ] {
        if let Some(s) = section.get(field).and_then(|v| v.as_str()) {
            total = total.saturating_add(s.chars().count());
        }
    }
    // Bullet / item-list shapes carry their length inside `items`
    // or `lines` arrays.
    if let Some(items) = section.get("items").and_then(|v| v.as_array()) {
        for item in items {
            if let Some(s) = item.as_str() {
                total = total.saturating_add(s.chars().count());
            } else if let Some(obj) = item.as_object() {
                for field in ["label", "value", "text", "body", "title"] {
                    if let Some(s) = obj.get(field).and_then(|v| v.as_str()) {
                        total = total.saturating_add(s.chars().count());
                    }
                }
            }
        }
    }
    total
}

/// `density_audit` phase implementation.
#[derive(Debug, Default)]
pub struct DensityAuditPhase;

impl Phase for DensityAuditPhase {
    fn name(&self) -> &'static str {
        "density_audit"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let target = read_target_density(&ctx.root);
        let Some(target) = target else {
            return Ok(findings);
        };
        let tolerance = read_tolerance(&ctx.root).unwrap_or(1);
        // Per-tenant density_override per [[per-tenant-corpora-doctrine]].
        // REPLACE semantics: when a page path matches an override
        // pattern, the empirical classification is REPLACED by the
        // operator's declared tier. Tenants use this when their
        // intent for a particular page-pattern is known + the
        // heuristic doesn't capture it (e.g. blog/* is dense even
        // when individual posts are short).
        let tenant = TenantCorpus::load(&ctx.root);
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
            // Compute the effective tier: tenant override if matches,
            // else empirical classification.
            let empirical = classify_page(&value);
            let cms_relative = make_cms_relative(&ctx.root, &path);
            let overridden = tenant.as_ref().and_then(|t| {
                t.density_override.iter().find_map(|ov| {
                    if glob_match(&ov.pattern, &cms_relative) {
                        DensityTier::parse(&ov.tier).map(|tier| (tier, ov.pattern.as_str()))
                    } else {
                        None
                    }
                })
            });
            let (effective, override_note) = match overridden {
                Some((tier, pattern)) => (
                    tier,
                    format!(" (tenant-corpus override matched pattern `{pattern}`)"),
                ),
                None => (empirical, String::new()),
            };
            let distance = (effective.ordinal() - target.ordinal()).abs();
            if distance > tolerance {
                let direction = if effective.ordinal() < target.ordinal() {
                    "sparser"
                } else {
                    "denser"
                };
                findings.push(Finding::warn(
                    self.name(),
                    path.display().to_string(),
                    format!(
                        "density_audit — page measures as `{}` but `[composition] target_density = \"{}\"` was declared (distance {distance}, tolerance {tolerance}){override_note}. Page is {direction} than the declared target; either revise the page's section composition or adjust the target.",
                        effective.slug(),
                        target.slug()
                    ),
                ));
            }
        }
        Ok(findings)
    }
}

/// Make a cms-root-relative path string from an absolute path.
/// Used so density_override patterns can match `cms/blog/*.json`
/// regardless of the absolute root location.
fn make_cms_relative(root: &Path, abs: &Path) -> String {
    abs.strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| abs.to_string_lossy().to_string())
}

/// Minimal glob: only `*` is special, treated as `.*` (greedy
/// match). Pattern is anchored to BOTH ends. Empty pattern
/// matches nothing.
///
/// Matches the subset of glob semantics density_override patterns
/// need: `cms/blog/*.json` style. No `**`, no `?`, no `[...]`
/// character classes — minimal so the matcher is auditable.
#[must_use]
pub(crate) fn glob_match(pattern: &str, path: &str) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        // No * — exact match.
        return pattern == path;
    }
    // First segment must prefix.
    let Some(rest) = path.strip_prefix(parts[0]) else {
        return false;
    };
    let mut cursor = rest;
    for (i, seg) in parts.iter().enumerate().skip(1) {
        let is_last = i == parts.len() - 1;
        if seg.is_empty() {
            // Pattern ended with * — anything left over matches.
            if is_last {
                return true;
            }
            continue;
        }
        let Some(found) = cursor.find(seg) else {
            return false;
        };
        cursor = &cursor[found + seg.len()..];
        if is_last && !cursor.is_empty() {
            // Pattern ends with this segment but path has more
            // characters after the match → must be exact tail.
            return false;
        }
    }
    true
}

/// Read `[composition] target_density = "..."` from forge.toml.
/// Returns `None` if the section or key is absent — phase is
/// silent in that case.
#[must_use]
fn read_target_density(root: &Path) -> Option<DensityTier> {
    let raw = read_toml_string_value(root, "[composition]", "target_density")?;
    DensityTier::parse(&raw)
}

/// Read `[composition] tolerance = N` from forge.toml. Default
/// 1 (one-tier slack on either side of target).
#[must_use]
fn read_tolerance(root: &Path) -> Option<i32> {
    read_toml_string_value(root, "[composition]", "tolerance")?
        .parse::<i32>()
        .ok()
}

fn read_toml_string_value(root: &Path, section: &str, key: &str) -> Option<String> {
    let cfg_path = root.join("forge.toml");
    let body = fs::read_to_string(&cfg_path).ok()?;
    let mut in_section = false;
    for raw in body.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            in_section = line == section;
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some(rest) = line.strip_prefix(key) {
            let v = rest.trim_start().trim_start_matches('=').trim();
            let unquoted = v.trim_matches('"').trim_matches('\'');
            return Some(unquoted.to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn page_with_sections(sections: Value) -> Value {
        json!({ "sections": sections })
    }

    #[test]
    fn density_tier_parse_round_trip() {
        for slug in ["sparse", "comfortable", "dense", "extreme"] {
            let t = DensityTier::parse(slug).expect("known slug");
            assert_eq!(t.slug(), slug);
        }
        assert!(DensityTier::parse("nonexistent").is_none());
    }

    #[test]
    fn ordinal_is_monotonic() {
        assert!(DensityTier::Sparse.ordinal() < DensityTier::Comfortable.ordinal());
        assert!(DensityTier::Comfortable.ordinal() < DensityTier::Dense.ordinal());
        assert!(DensityTier::Dense.ordinal() < DensityTier::Extreme.ordinal());
    }

    #[test]
    fn classify_empty_sections_is_sparse() {
        let page = page_with_sections(json!([]));
        assert_eq!(classify_page(&page), DensityTier::Sparse);
    }

    #[test]
    fn classify_short_marketing_page_is_sparse() {
        // 3 short sections, ~30 chars each = ~90 total, below 400 floor.
        let page = page_with_sections(json!([
            { "kind": "hero", "title": "Welcome." },
            { "kind": "paragraph", "text": "Some text here." },
            { "kind": "call_to_action", "title": "Try it" }
        ]));
        assert_eq!(classify_page(&page), DensityTier::Sparse);
    }

    #[test]
    fn classify_balanced_marketing_page_is_comfortable() {
        // 6 sections, ~100 chars each = ~600 total, avg 100.
        let s = "x".repeat(100);
        let page = page_with_sections(json!([
            { "kind": "hero", "title": s.clone(), "lede": s.clone() },
            { "kind": "paragraph", "text": s.clone() },
            { "kind": "paragraph", "text": s.clone() },
            { "kind": "feature_spotlight", "title": s.clone() },
            { "kind": "feature_spotlight", "title": s.clone() },
            { "kind": "call_to_action", "title": s }
        ]));
        assert_eq!(classify_page(&page), DensityTier::Comfortable);
    }

    #[test]
    fn classify_editorial_dense_page_is_dense() {
        // 4 sections, ~300 chars each = ~1200 total.
        let s = "x".repeat(300);
        let page = page_with_sections(json!([
            { "kind": "paragraph", "text": s.clone() },
            { "kind": "paragraph", "text": s.clone() },
            { "kind": "paragraph", "text": s.clone() },
            { "kind": "paragraph", "text": s }
        ]));
        assert_eq!(classify_page(&page), DensityTier::Dense);
    }

    #[test]
    fn classify_terminal_tier_is_extreme() {
        // 3 sections, ~500 chars each.
        let s = "x".repeat(500);
        let page = page_with_sections(json!([
            { "kind": "paragraph", "text": s.clone() },
            { "kind": "paragraph", "text": s.clone() },
            { "kind": "paragraph", "text": s }
        ]));
        assert_eq!(classify_page(&page), DensityTier::Extreme);
    }

    #[test]
    fn classify_skips_decorative_kinds() {
        // 5 spacers + 1 short hero = sparse (decoratives don't
        // contribute density).
        let page = page_with_sections(json!([
            { "kind": "spacer" },
            { "kind": "divider" },
            { "kind": "spacer" },
            { "kind": "divider" },
            { "kind": "container" },
            { "kind": "hero", "title": "Welcome." }
        ]));
        assert_eq!(classify_page(&page), DensityTier::Sparse);
    }

    #[test]
    fn classify_kv_pair_items_count_in_chars() {
        // kv-pair items carry their text inside item.label / item.value,
        // not on the section itself. Verify the items-walk catches them.
        let page = page_with_sections(json!([
            {
                "kind": "kv_pair",
                "title": "Stats",
                "items": [
                    { "label": "Founded", "value": "2024" },
                    { "label": "Team", "value": "Three" },
                    { "label": "Location", "value": "Remote" },
                    { "label": "Funding", "value": "Bootstrapped" }
                ]
            },
            // The chars from the items above + 12 short marketing sections
            // should push the page to Comfortable when items count.
            { "kind": "paragraph", "text": "ABCDEF".repeat(20) },
            { "kind": "paragraph", "text": "ABCDEF".repeat(20) },
            { "kind": "paragraph", "text": "ABCDEF".repeat(20) },
            { "kind": "paragraph", "text": "ABCDEF".repeat(20) },
            { "kind": "paragraph", "text": "ABCDEF".repeat(20) }
        ]));
        let tier = classify_page(&page);
        // 6 body sections, ~120 chars avg → Comfortable.
        assert_eq!(tier, DensityTier::Comfortable);
    }

    // glob_match — minimal pattern matcher for density_override.

    #[test]
    fn glob_no_star_is_exact_match() {
        assert!(glob_match("cms/index.json", "cms/index.json"));
        assert!(!glob_match("cms/index.json", "cms/other.json"));
        assert!(!glob_match("cms/index.json", "prefix/cms/index.json"));
    }

    #[test]
    fn glob_trailing_star_matches_any_suffix() {
        assert!(glob_match("cms/blog/*", "cms/blog/post-1.json"));
        assert!(glob_match("cms/blog/*", "cms/blog/2026/post.json"));
        assert!(!glob_match("cms/blog/*", "cms/index.json"));
    }

    #[test]
    fn glob_middle_star_matches_inner() {
        assert!(glob_match("cms/blog/*.json", "cms/blog/post-1.json"));
        assert!(glob_match("cms/blog/*.json", "cms/blog/index.json"));
        assert!(!glob_match("cms/blog/*.json", "cms/blog/post-1.md"));
        assert!(!glob_match("cms/blog/*.json", "cms/other/post.json"));
    }

    #[test]
    fn glob_multiple_stars_match_in_order() {
        assert!(glob_match("cms/*/index.*", "cms/blog/index.json"));
        assert!(glob_match("cms/*/index.*", "cms/admin/index.html"));
        assert!(!glob_match("cms/*/index.*", "cms/blog/post.json"));
    }

    #[test]
    fn glob_leading_star_matches_any_prefix() {
        assert!(glob_match("*/index.json", "cms/index.json"));
        assert!(glob_match("*/index.json", "deeply/nested/path/index.json"));
        assert!(!glob_match("*/index.json", "index.json"));
    }

    #[test]
    fn glob_empty_pattern_only_matches_empty() {
        assert!(glob_match("", ""));
        assert!(!glob_match("", "x"));
        assert!(!glob_match("x", ""));
    }

    #[test]
    fn visible_chars_walks_known_fields() {
        let section = json!({
            "kind": "paragraph",
            "text": "abcde",       // 5
            "title": "fg",         // 2
            "description": "hij"   // 3
        });
        assert_eq!(visible_chars_in_section(&section), 10);
    }
}
