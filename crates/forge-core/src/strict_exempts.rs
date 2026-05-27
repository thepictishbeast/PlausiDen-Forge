//! `strict_exempts` — per-tenant suppression of specific findings.
//!
//! Per docs/FORGE_LITE_DIAGNOSTIC_2026_05_22.md Category 4: some
//! variation-arc phases (sparse_page, image_desert) misfire on
//! intentionally narrow content (brief / portfolio / editorial
//! shapes), and the tenant has no escape valve. Without this
//! mechanism, the tenant either accepts the false-positive
//! strict-fail OR turns off the whole phase via [strict] →
//! losing the legitimate findings the phase also produces.
//!
//! Wire format:
//!
//! ```toml
//! [strict.exempt]
//! aesthetic_distinctiveness = [
//!     "sparse_page",
//!     "image_desert"
//! ]
//! ```
//!
//! Each value is a list of substrings. Any finding from the
//! named phase whose message contains any of the substrings is
//! suppressed (removed from the build report).
//!
//! ## When to use exemption vs unset [strict]
//!
//! - `[strict] phase_name = false` (or unset) — phase findings
//!   remain at Warn; ship is unblocked but findings still visible
//! - `[strict] phase_name = true` + `[strict.exempt] phase_name`
//!   contains the substring — phase findings are promoted to
//!   Strict but the named pattern is suppressed entirely; ship
//!   proceeds, finding hidden
//!
//! The exempt mechanism is the operator saying "I've reviewed
//! this finding and the substrate is over-broad here; for THIS
//! tenant this specific pattern is a false positive." It should
//! be used sparingly + with a comment explaining why.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::Finding;

/// Per-phase finding suppression rules. Constructed by
/// [`StrictExempts::load`] from the `[strict.exempt]` TOML
/// section.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StrictExempts {
    /// phase name → list of substring patterns to suppress
    patterns: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
struct ForgeTomlEnvelope {
    #[serde(default)]
    strict: Option<StrictSection>,
}

#[derive(Debug, Default, Deserialize)]
struct StrictSection {
    #[serde(default)]
    exempt: BTreeMap<String, Vec<String>>,
}

impl StrictExempts {
    /// Load `[strict.exempt]` from `<root>/forge.toml`.
    /// Fail-tolerant — missing file / section / malformed TOML
    /// all return an empty exempt set.
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
            patterns: envelope
                .strict
                .map(|s| s.exempt)
                .unwrap_or_default(),
        }
    }

    /// Build directly from a slice of (phase_name, patterns)
    /// pairs. Useful for tests.
    #[must_use]
    pub fn from_pairs<S, I, P>(pairs: I) -> Self
    where
        S: Into<String>,
        P: Into<String>,
        I: IntoIterator<Item = (S, Vec<P>)>,
    {
        Self {
            patterns: pairs
                .into_iter()
                .map(|(phase, patterns)| {
                    (
                        phase.into(),
                        patterns.into_iter().map(Into::into).collect(),
                    )
                })
                .collect(),
        }
    }

    /// True when no exemptions are configured. The build
    /// pipeline can skip the filter entirely when this is true.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty() || self.patterns.values().all(Vec::is_empty)
    }

    /// True when this finding should be suppressed: its `phase`
    /// has exempt patterns configured AND its `message`
    /// contains at least one of them.
    #[must_use]
    pub fn should_suppress(&self, finding: &Finding) -> bool {
        self.patterns
            .get(&finding.phase)
            .is_some_and(|patterns| patterns.iter().any(|p| finding.message.contains(p)))
    }

    /// Filter findings in place, removing any that match
    /// configured exempt patterns. Returns the number removed
    /// so callers can log "[strict.exempt] suppressed N
    /// finding(s)" for operator visibility.
    pub fn filter(&self, findings: &mut Vec<Finding>) -> usize {
        if self.is_empty() {
            return 0;
        }
        let before = findings.len();
        findings.retain(|f| !self.should_suppress(f));
        before - findings.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warn(phase: &str, message: &str) -> Finding {
        Finding::warn(phase, "tests/fixture", message)
    }

    fn strict(phase: &str, message: &str) -> Finding {
        Finding::strict(phase, "tests/fixture", message)
    }

    #[test]
    fn empty_exempts_no_op() {
        let e = StrictExempts::default();
        let mut findings = vec![warn("aesthetic_distinctiveness", "sparse_page: 3 sections")];
        let n = e.filter(&mut findings);
        assert_eq!(n, 0);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn suppresses_matching_pattern() {
        let e = StrictExempts::from_pairs([(
            "aesthetic_distinctiveness",
            vec!["sparse_page", "image_desert"],
        )]);
        let mut findings = vec![
            warn("aesthetic_distinctiveness", "sparse_page: 3 sections"),
            warn("aesthetic_distinctiveness", "monotonous_feature_grid: 1 icon"),
            warn("aesthetic_distinctiveness", "image_desert: 0 images"),
        ];
        let n = e.filter(&mut findings);
        assert_eq!(n, 2);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("monotonous_feature_grid"));
    }

    #[test]
    fn does_not_suppress_other_phases() {
        let e = StrictExempts::from_pairs([(
            "aesthetic_distinctiveness",
            vec!["sparse_page"],
        )]);
        let mut findings = vec![
            warn("aesthetic_distinctiveness", "sparse_page: 3 sections"),
            warn("contrast", "sparse_page: this isn't actually the same finding"),
        ];
        let n = e.filter(&mut findings);
        assert_eq!(n, 1);
        // Contrast finding survives because its phase isn't exempted.
        assert!(findings.iter().any(|f| f.phase == "contrast"));
    }

    #[test]
    fn suppresses_strict_findings_too() {
        // Severity is irrelevant; exemption is by phase + message
        // substring regardless of severity.
        let e = StrictExempts::from_pairs([(
            "aesthetic_distinctiveness",
            vec!["sparse_page"],
        )]);
        let mut findings = vec![strict("aesthetic_distinctiveness", "sparse_page: 3 sections")];
        let n = e.filter(&mut findings);
        assert_eq!(n, 1);
        assert!(findings.is_empty());
    }

    #[test]
    fn empty_pattern_list_for_phase_is_no_op() {
        let e = StrictExempts::from_pairs([("aesthetic_distinctiveness", Vec::<String>::new())]);
        assert!(e.is_empty());
        let mut findings = vec![warn("aesthetic_distinctiveness", "sparse_page: 3")];
        let n = e.filter(&mut findings);
        assert_eq!(n, 0);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn is_empty_when_no_phases_configured() {
        assert!(StrictExempts::default().is_empty());
    }

    #[test]
    fn load_parses_strict_exempt_section() {
        let tmp = std::env::temp_dir().join(format!(
            "strict-exempts-load-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("forge.toml"),
            "[strict.exempt]\naesthetic_distinctiveness = [\"sparse_page\", \"image_desert\"]\n",
        )
        .unwrap();
        let e = StrictExempts::load(&tmp);
        let mut findings = vec![
            warn("aesthetic_distinctiveness", "sparse_page: 3 sections"),
            warn("aesthetic_distinctiveness", "image_desert: 0 images"),
            warn("aesthetic_distinctiveness", "monotonous_feature_grid: 1 icon"),
        ];
        let n = e.filter(&mut findings);
        assert_eq!(n, 2);
        assert_eq!(findings.len(), 1);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn load_returns_default_on_malformed_toml() {
        let tmp = std::env::temp_dir().join(format!(
            "strict-exempts-malformed-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("forge.toml"), "this is not toml: {{").unwrap();
        let e = StrictExempts::load(&tmp);
        assert!(e.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn load_returns_default_when_strict_section_absent() {
        let tmp = std::env::temp_dir().join(format!(
            "strict-exempts-no-section-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("forge.toml"), "[forge]\nmode = \"poc\"\n").unwrap();
        let e = StrictExempts::load(&tmp);
        assert!(e.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
