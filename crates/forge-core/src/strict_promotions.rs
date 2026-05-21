//! `strict_promotions` — per-phase Warn→Strict promotion via
//! `[strict]` section of `forge.toml`.
//!
//! Per the architecture audit 2026-05-21: variation-arc phases
//! (aesthetic_distinctiveness, pattern_emergence, content_substance,
//! etc.) already emit findings, but at Warn severity. Operators
//! see the findings, builds pass, findings get ignored. The
//! enforcement infrastructure is in place; it just isn't
//! tightened.
//!
//! This module is the per-tenant tightening knob. A tenant
//! declares which phase's Warns should be Strict in its
//! `forge.toml`:
//!
//! ```toml
//! [strict]
//! aesthetic_distinctiveness = true
//! content_substance = true
//! pattern_emergence = true
//! ```
//!
//! The build pipeline calls [`StrictPromotions::promote`] over
//! the collected findings AFTER every phase has run. Any
//! finding whose phase is flagged in `[strict]` is rewritten
//! from `Severity::Warn` to `Severity::Strict`, so it blocks
//! ship instead of getting glossed over.
//!
//! ## Why post-pass, not in-phase
//!
//! Putting the strict flag inside every phase's run method
//! requires touching ~60 phases. A single post-pass over the
//! collected findings is one place to maintain and naturally
//! composes with new phases — adding a new variation-arc phase
//! is just adding it to the canonical recommended set in
//! `recommended_variation_arc_phases()`; no per-phase wiring.
//!
//! ## Wire format
//!
//! `[strict]` is a TOML table mapping phase name (kebab-case
//! string matching `Phase::name()`) to `bool`. Unknown phase
//! names are silently ignored (back-compat — adding a phase
//! that doesn't exist anymore must not break the build). Phase
//! names with `false` values are explicit no-ops (lets
//! operators document their decision-not-to-strict).

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::{Finding, Severity};

/// Set of phase names whose Warn findings should be promoted
/// to Strict. Constructed by [`StrictPromotions::load`] from a
/// `[strict]` TOML section.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StrictPromotions {
    promoted: BTreeMap<String, bool>,
}

#[derive(Debug, Default, Deserialize)]
struct ForgeTomlEnvelope {
    #[serde(default)]
    strict: BTreeMap<String, bool>,
}

impl StrictPromotions {
    /// Load `[strict]` from `<root>/forge.toml`. Fail-tolerant:
    /// missing file / missing section / non-UTF8 / malformed
    /// TOML all return an empty promotion set (no promotions).
    ///
    /// This matches the `[poc].suppress_X` pattern used
    /// elsewhere — operators opt in; absence is no-op.
    #[must_use]
    pub fn load(root: &Path) -> Self {
        let path = root.join("forge.toml");
        let Ok(body) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        let Ok(envelope) = toml::from_str::<ForgeTomlEnvelope>(&body) else {
            return Self::default();
        };
        Self {
            promoted: envelope.strict,
        }
    }

    /// Build directly from a slice of (phase_name, promoted)
    /// pairs. Useful for tests + for programmatic configuration
    /// from sources other than forge.toml.
    #[must_use]
    pub fn from_pairs<S: Into<String>>(pairs: impl IntoIterator<Item = (S, bool)>) -> Self {
        Self {
            promoted: pairs
                .into_iter()
                .map(|(k, v)| (k.into(), v))
                .collect(),
        }
    }

    /// True when this phase has been declared strict-promoted.
    /// Missing entries default to `false`.
    #[must_use]
    pub fn is_promoted(&self, phase: &str) -> bool {
        self.promoted.get(phase).copied().unwrap_or(false)
    }

    /// True when no promotions are configured. The build
    /// pipeline can skip the post-pass entirely when this is
    /// true (micro-optimization).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.promoted.values().any(|v| *v)
    }

    /// Promote findings in place. Walks the slice; for each
    /// finding whose `phase` is strict-promoted AND whose
    /// current severity is `Warn`, rewrites the severity to
    /// `Strict`. Other findings are left untouched (Strict
    /// findings stay Strict; promoted findings that were
    /// already Strict stay Strict).
    ///
    /// Returns the number of findings that were promoted —
    /// callers can log "promoted N findings under [strict]" for
    /// operator visibility.
    pub fn promote(&self, findings: &mut [Finding]) -> usize {
        if self.is_empty() {
            return 0;
        }
        let mut promoted_count = 0;
        for f in findings {
            if f.severity == Severity::Warn && self.is_promoted(&f.phase) {
                f.severity = Severity::Strict;
                promoted_count += 1;
            }
        }
        promoted_count
    }
}

/// Canonical list of variation-arc phase names that an
/// opinionated tenant should consider promoting. Returned as a
/// `&'static [&'static str]` so the recommendation surfaces in
/// docs + CLI orient banners without allocating.
///
/// This is the prescriptive list per docs/SUBSTRATE_REFRAME_2026_05_21.md
/// — phases whose findings are real-quality signals that get
/// ignored at Warn severity.
#[must_use]
pub fn recommended_variation_arc_phases() -> &'static [&'static str] {
    &[
        "aesthetic_distinctiveness",
        "content_substance",
        "pattern_emergence",
        "differentiation_budget",
        "editorial_purity_gate",
        "slop_dictionary",
        "identity_coherence",
        "theme_variation_required",
        "monotonous_feature_grid",
        "image_desert",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warn(phase: &str) -> Finding {
        Finding::warn(phase, "tests/fixture", "test message")
    }

    fn strict(phase: &str) -> Finding {
        Finding::strict(phase, "tests/fixture", "test message")
    }

    #[test]
    fn empty_promotions_no_op() {
        let p = StrictPromotions::default();
        let mut findings = vec![warn("aesthetic_distinctiveness"), warn("contrast")];
        let n = p.promote(&mut findings);
        assert_eq!(n, 0);
        assert!(findings.iter().all(|f| f.severity == Severity::Warn));
    }

    #[test]
    fn promotes_only_flagged_phases() {
        let p = StrictPromotions::from_pairs([
            ("aesthetic_distinctiveness", true),
            ("contrast", false),
        ]);
        let mut findings = vec![
            warn("aesthetic_distinctiveness"),
            warn("contrast"),
            warn("seo"),
        ];
        let n = p.promote(&mut findings);
        assert_eq!(n, 1);
        assert_eq!(findings[0].severity, Severity::Strict);
        assert_eq!(findings[1].severity, Severity::Warn);
        assert_eq!(findings[2].severity, Severity::Warn);
    }

    #[test]
    fn leaves_strict_findings_alone() {
        // Strict findings stay Strict; promotion shouldn't
        // downgrade or re-mark them.
        let p = StrictPromotions::from_pairs([("aesthetic_distinctiveness", true)]);
        let mut findings = vec![strict("aesthetic_distinctiveness")];
        let n = p.promote(&mut findings);
        assert_eq!(n, 0);
        assert_eq!(findings[0].severity, Severity::Strict);
    }

    #[test]
    fn unknown_phase_names_silently_ignored() {
        // Promoting a phase that doesn't exist in this build's
        // findings is a no-op (back-compat — substrate evolves;
        // tenant forge.toml may reference renamed phases).
        let p = StrictPromotions::from_pairs([("phase_that_does_not_exist", true)]);
        let mut findings = vec![warn("aesthetic_distinctiveness")];
        let n = p.promote(&mut findings);
        assert_eq!(n, 0);
        assert_eq!(findings[0].severity, Severity::Warn);
    }

    #[test]
    fn explicit_false_is_no_op() {
        // Operator explicitly recording "we choose NOT to strict
        // this" should not change behavior — clearer signal than
        // omitting the key.
        let p = StrictPromotions::from_pairs([("aesthetic_distinctiveness", false)]);
        let mut findings = vec![warn("aesthetic_distinctiveness")];
        let n = p.promote(&mut findings);
        assert_eq!(n, 0);
        assert_eq!(findings[0].severity, Severity::Warn);
    }

    #[test]
    fn is_promoted_returns_correct_flag() {
        let p = StrictPromotions::from_pairs([
            ("aesthetic_distinctiveness", true),
            ("contrast", false),
        ]);
        assert!(p.is_promoted("aesthetic_distinctiveness"));
        assert!(!p.is_promoted("contrast"));
        assert!(!p.is_promoted("unknown_phase"));
    }

    #[test]
    fn is_empty_when_no_true_values() {
        let p = StrictPromotions::from_pairs([("aesthetic_distinctiveness", false)]);
        assert!(p.is_empty());
        let p = StrictPromotions::from_pairs([("aesthetic_distinctiveness", true)]);
        assert!(!p.is_empty());
    }

    #[test]
    fn recommended_set_contains_audited_phases() {
        // Pin the recommendation — these are the phases the
        // 2026-05-21 architecture audit identified as emitting
        // real-quality signals at Warn severity. Removing one
        // from the list is a substrate-doctrine event.
        let rec = recommended_variation_arc_phases();
        for required in &[
            "aesthetic_distinctiveness",
            "content_substance",
            "pattern_emergence",
        ] {
            assert!(
                rec.contains(required),
                "recommended set missing audit-flagged phase {required:?}"
            );
        }
    }

    #[test]
    fn load_returns_default_when_no_forge_toml() {
        let tmp = std::env::temp_dir().join(format!(
            "strict-promotions-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let p = StrictPromotions::load(&tmp);
        assert!(p.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn load_parses_strict_section() {
        let tmp = std::env::temp_dir().join(format!(
            "strict-promotions-load-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("forge.toml"),
            "[strict]\naesthetic_distinctiveness = true\ncontent_substance = true\nseo = false\n",
        )
        .unwrap();
        let p = StrictPromotions::load(&tmp);
        assert!(p.is_promoted("aesthetic_distinctiveness"));
        assert!(p.is_promoted("content_substance"));
        assert!(!p.is_promoted("seo"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn load_returns_default_on_malformed_toml() {
        let tmp = std::env::temp_dir().join(format!(
            "strict-promotions-malformed-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("forge.toml"), "this is not toml: {{ unbalanced").unwrap();
        let p = StrictPromotions::load(&tmp);
        assert!(p.is_empty(), "malformed forge.toml must not promote anything");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
