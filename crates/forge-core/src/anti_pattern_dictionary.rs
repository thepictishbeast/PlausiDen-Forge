//! `anti_pattern_dictionary` — curated registry of structural
//! patterns that the substrate KNOWS produce bad output.
//!
//! Layer-3 substrate-reframe doctrine (#376): the fingerprint
//! registry (#370) detects per-tenant duplication; the
//! anti-pattern dictionary detects substrate-default-band
//! collapse — patterns that aren't per-tenant duplicates but
//! ARE known-bad shapes any tenant matching them shouldn't
//! ship.
//!
//! Example anti-pattern: "SaaS template collapse" — a site
//! shaped Hero → FeatureSpotlight 3-up → Testimonial → CTA. No
//! single tenant owns this; it's a shape any of 50 tenants might
//! converge on without intent. The dictionary refuses to ship
//! sites that match it without explicit operator opt-in.
//!
//! ## Severity tiers
//!
//! - `info` — pattern is mildly worrying but acceptable
//! - `warn` — pattern fires a soft finding; ship still allowed
//! - `block` — pattern refuses to ship without `[strict.exempt]`
//!
//! ## Match semantics
//!
//! Each anti-pattern declares a `kind_sequence` (the ordered
//! list of CmsSection kinds it matches). A site matches if its
//! per-page primitive occurrences contain the sequence as a
//! consecutive subsequence on any single page.

use serde::Serialize;

use crate::fingerprint::SiteFingerprint;

/// Severity tier for an anti-pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AntiPatternSeverity {
    /// Mildly worrying; acceptable with awareness.
    Info,
    /// Soft finding; ship allowed.
    Warn,
    /// Refuses to ship without explicit exempt entry.
    Block,
}

impl AntiPatternSeverity {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Block => "block",
        }
    }
}

/// One anti-pattern entry.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct AntiPattern {
    /// Stable kebab-case identifier.
    pub id: &'static str,
    /// One-line human-readable name.
    pub name: &'static str,
    /// Why this pattern is anti.
    pub rationale: &'static str,
    /// Ordered sequence of CmsSection `kind` strings that match.
    pub kind_sequence: &'static [&'static str],
    /// Severity.
    pub severity: AntiPatternSeverity,
    /// Suggested alternative shape.
    pub alternative_suggestion: &'static str,
}

/// One match result from running the dictionary against a fingerprint.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct AntiPatternMatch {
    /// The matched anti-pattern.
    pub pattern_id: &'static str,
    /// Severity.
    pub severity: AntiPatternSeverity,
    /// Page on which the pattern was detected.
    pub page: String,
    /// The consecutive section kinds that triggered the match.
    pub matched_kinds: Vec<String>,
}

/// Canonical anti-pattern dictionary. Hand-curated. Grows over
/// time as the substrate's surveillance phase (#379) surfaces
/// new emergent bad patterns.
pub const DICTIONARY: &[AntiPattern] = &[
    AntiPattern {
        id: "saas-template-collapse",
        name: "SaaS marketing template collapse",
        rationale: "Hero + FeatureSpotlight 3-up + Testimonial + CTA \
                    is the SaaS-template default that every consumer-band \
                    site converges on without intent. Per the substrate \
                    reframe, this shape is the bias to neutralize.",
        kind_sequence: &[
            "hero",
            "feature_spotlight",
            "testimonial",
            "call_to_action",
        ],
        severity: AntiPatternSeverity::Warn,
        alternative_suggestion: "Substitute hero_editorial / image_hero / \
                                 SplitHero for the opening; replace \
                                 feature_spotlight with a content-first \
                                 section (pull_quote, source_list, kv_pair) \
                                 OR change Decoration from Decorated to \
                                 Editorial/Minimal.",
    },
    AntiPattern {
        id: "modern-saas-stack",
        name: "Modern-SaaS gradient stack",
        rationale: "Hero (GradientMesh) + Stat band + 3-up feature card + \
                    pricing — the YC-launch template. Distinct from #1 only \
                    by including pricing; same SaaS-band collapse.",
        kind_sequence: &[
            "hero",
            "stat_band",
            "feature_spotlight",
            "pricing",
        ],
        severity: AntiPatternSeverity::Warn,
        alternative_suggestion: "If the site genuinely sells a product, \
                                 differentiate via image_hero with photo \
                                 backdrop + editorial decoration; reserve \
                                 GradientMesh for SaaS landing only.",
    },
    AntiPattern {
        id: "decorated-everywhere",
        name: "Decorated decoration on every section",
        rationale: "Every FeatureSpotlight uses the SaaS-card chrome. \
                    Per the neutralize-defaults audit (#360), PageKind \
                    should drive decoration; uniform Decorated is the \
                    band-default that collapses brief / editorial / civic \
                    pages into the SaaS register.",
        kind_sequence: &[
            "feature_spotlight",
            "feature_spotlight",
            "feature_spotlight",
        ],
        severity: AntiPatternSeverity::Info,
        alternative_suggestion: "Vary decoration across sections: at least \
                                 one Editorial or Minimal variant.",
    },
    AntiPattern {
        id: "hero-cta-only",
        name: "Hero + immediate CTA (no substance)",
        rationale: "Site is a hero followed directly by a call-to-action \
                    with no intervening content. Common in scammy / \
                    affiliate landings; substrate refuses without \
                    explicit content-substance exempt.",
        kind_sequence: &["hero", "call_to_action"],
        severity: AntiPatternSeverity::Block,
        alternative_suggestion: "Add at least 2 substantive content sections \
                                 between hero and CTA (paragraph, pull_quote, \
                                 kv_pair, source_list, feature_spotlight).",
    },
    AntiPattern {
        id: "marquee-cascade",
        name: "Marquee cascade",
        rationale: "Multiple marquee sections in sequence — visual noise \
                    that signals 'we have nothing to say'.",
        kind_sequence: &["marquee", "marquee"],
        severity: AntiPatternSeverity::Warn,
        alternative_suggestion: "Keep at most one marquee per page; \
                                 substitute the rest with kv_pair, \
                                 source_list, or stat_band for substance.",
    },
];

/// Check a site fingerprint against the anti-pattern dictionary.
/// Returns every match found, sorted by severity descending then
/// pattern id ascending for stable test output.
#[must_use]
pub fn check_against(fp: &SiteFingerprint) -> Vec<AntiPatternMatch> {
    let mut matches: Vec<AntiPatternMatch> = Vec::new();

    // Group occurrences by page.
    use std::collections::BTreeMap;
    let mut per_page: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for occ in &fp.primitives {
        per_page
            .entry(occ.page.as_str())
            .or_default()
            .push(occ.kind.as_str());
    }

    for pattern in DICTIONARY {
        for (page, kinds) in &per_page {
            if contains_subsequence(kinds, pattern.kind_sequence) {
                matches.push(AntiPatternMatch {
                    pattern_id: pattern.id,
                    severity: pattern.severity,
                    page: (*page).to_owned(),
                    matched_kinds: pattern
                        .kind_sequence
                        .iter()
                        .map(|s| (*s).to_owned())
                        .collect(),
                });
            }
        }
    }

    matches.sort_by(|a, b| {
        let sev_rank = |s: AntiPatternSeverity| match s {
            AntiPatternSeverity::Block => 0,
            AntiPatternSeverity::Warn => 1,
            AntiPatternSeverity::Info => 2,
        };
        sev_rank(a.severity)
            .cmp(&sev_rank(b.severity))
            .then_with(|| a.pattern_id.cmp(b.pattern_id))
    });

    matches
}

/// Returns true if `needle` appears as a consecutive subsequence
/// of `haystack`.
fn contains_subsequence(haystack: &[&str], needle: &[&str]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::{
        AssetDistribution, FingerprintSpec, PrimitiveOccurrence,
        SiteFingerprint,
    };
    use std::collections::BTreeMap;

    fn fp_with_page(page: &str, kinds: &[&str]) -> SiteFingerprint {
        let primitives: Vec<PrimitiveOccurrence> = kinds
            .iter()
            .map(|k| PrimitiveOccurrence::new(*k, "", page))
            .collect();
        SiteFingerprint {
            spec: FingerprintSpec::V1,
            primitives,
            token_overrides: Vec::new(),
            silhouettes: BTreeMap::new(),
            rhythms: BTreeMap::new(),
            assets: AssetDistribution::default(),
        }
    }

    #[test]
    fn dictionary_not_empty() {
        assert!(!DICTIONARY.is_empty());
    }

    #[test]
    fn unique_pattern_ids() {
        let mut ids: Vec<&str> = DICTIONARY.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        let original = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), original);
    }

    #[test]
    fn empty_fingerprint_no_matches() {
        let fp = SiteFingerprint {
            spec: FingerprintSpec::V1,
            primitives: Vec::new(),
            token_overrides: Vec::new(),
            silhouettes: BTreeMap::new(),
            rhythms: BTreeMap::new(),
            assets: AssetDistribution::default(),
        };
        assert!(check_against(&fp).is_empty());
    }

    #[test]
    fn saas_template_matches() {
        let fp = fp_with_page(
            "index",
            &["hero", "feature_spotlight", "testimonial", "call_to_action"],
        );
        let matches = check_against(&fp);
        assert!(matches.iter().any(|m| m.pattern_id == "saas-template-collapse"));
    }

    #[test]
    fn hero_cta_only_blocks() {
        let fp = fp_with_page("index", &["hero", "call_to_action"]);
        let matches = check_against(&fp);
        let block = matches.iter().find(|m| m.pattern_id == "hero-cta-only");
        assert!(block.is_some());
        assert_eq!(block.unwrap().severity, AntiPatternSeverity::Block);
    }

    #[test]
    fn non_matching_site_returns_empty() {
        let fp = fp_with_page(
            "index",
            &["hero_editorial", "paragraph", "pull_quote", "source_list"],
        );
        let matches = check_against(&fp);
        assert!(matches.is_empty());
    }

    #[test]
    fn marquee_cascade_warns() {
        let fp = fp_with_page("index", &["marquee", "marquee", "marquee"]);
        let matches = check_against(&fp);
        assert!(matches.iter().any(|m| m.pattern_id == "marquee-cascade"));
    }

    #[test]
    fn matches_sorted_block_before_warn() {
        // Site with both block (hero -> CTA) and warn (SaaS template)
        // SHOULD see block first.
        let fp = fp_with_page(
            "index",
            &[
                "hero",
                "call_to_action",
                "hero",
                "feature_spotlight",
                "testimonial",
                "call_to_action",
            ],
        );
        let matches = check_against(&fp);
        assert!(!matches.is_empty());
        // First match must be the block-severity one.
        assert_eq!(matches[0].severity, AntiPatternSeverity::Block);
    }

    #[test]
    fn severity_slug_stable() {
        assert_eq!(AntiPatternSeverity::Block.slug(), "block");
        assert_eq!(AntiPatternSeverity::Warn.slug(), "warn");
        assert_eq!(AntiPatternSeverity::Info.slug(), "info");
    }
}
