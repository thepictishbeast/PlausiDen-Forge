//! `originality` — n-gram shingle-based overlap detection.
//!
//! Anti-reuse gate for tenant content per task #369. Pure
//! functions; no I/O. The MCP tool layer is responsible for
//! reading files; this module operates on already-loaded strings.
//!
//! ## Approach
//!
//! Tokenize each string into lowercase whitespace-separated
//! words, then build word-shingles of length `min_ngram_words`.
//! An overlap is any shingle that appears in both the tenant
//! corpus and any reference corpus.
//!
//! This is a deterministic measure of verbatim reuse, not a
//! semantic similarity check. A tenant that paraphrases a
//! reference site won't trigger; a tenant that copies verbatim
//! will trigger immediately.
//!
//! ## Verdict policy
//!
//! - `ok` — zero overlaps
//! - `flag` — 1-3 overlaps (review by operator)
//! - `block` — 4+ overlaps (substrate refuses to ship)
//!
//! Thresholds are deliberately conservative. They are tuned
//! against the assumption that legitimate boilerplate (legal
//! disclaimers, ARIA labels) accounts for ~0-1 overlaps;
//! anything more represents real reuse.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

/// One overlapping shingle.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Overlap {
    /// The shared shingle (lowercase, whitespace-joined).
    pub phrase: String,
    /// How many times the shingle appears in the tenant corpus.
    pub tenant_count: u32,
    /// How many times the shingle appears in the reference corpus.
    pub corpus_count: u32,
}

/// Verdict for an originality check.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Verdict {
    /// No overlaps; safe to ship.
    Ok,
    /// 1-3 overlaps; operator review.
    Flag,
    /// 4+ overlaps; refuse to ship.
    Block,
}

impl Verdict {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Flag => "flag",
            Self::Block => "block",
        }
    }
}

/// Originality check report.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct OriginalityReport {
    /// Number of tenant strings scanned.
    pub total_tenant_strings: u32,
    /// Number of corpus strings scanned (across all corpora).
    pub total_corpus_strings: u32,
    /// Shingle length used.
    pub min_ngram_words: u32,
    /// Each overlap detected (deduplicated).
    pub overlaps: Vec<Overlap>,
    /// Verdict.
    pub verdict: Verdict,
}

/// Run an originality check on tenant strings vs corpus strings.
///
/// `min_ngram_words` is clamped to 2..=20. Strings shorter than
/// the shingle length contribute no shingles. Both inputs are
/// already-loaded slices of strings; the caller handles file I/O.
#[must_use]
pub fn check_originality(
    tenant_strings: &[String],
    corpus_strings: &[String],
    min_ngram_words: u32,
) -> OriginalityReport {
    let n = min_ngram_words.clamp(2, 20) as usize;

    // Build corpus shingle count map.
    let mut corpus_shingles: HashMap<String, u32> = HashMap::new();
    for s in corpus_strings {
        for shingle in shingles(s, n) {
            *corpus_shingles.entry(shingle).or_insert(0) += 1;
        }
    }

    // Walk tenant shingles; intersect with corpus.
    let mut tenant_shingles: HashMap<String, u32> = HashMap::new();
    for s in tenant_strings {
        for shingle in shingles(s, n) {
            *tenant_shingles.entry(shingle).or_insert(0) += 1;
        }
    }

    let mut overlaps: Vec<Overlap> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    for (phrase, t_count) in &tenant_shingles {
        if let Some(c_count) = corpus_shingles.get(phrase) {
            if !seen.contains(phrase.as_str()) {
                seen.insert(phrase.as_str());
                overlaps.push(Overlap {
                    phrase: phrase.clone(),
                    tenant_count: *t_count,
                    corpus_count: *c_count,
                });
            }
        }
    }

    // Deterministic ordering for stable test output.
    overlaps.sort_by(|a, b| a.phrase.cmp(&b.phrase));

    let verdict = match overlaps.len() {
        0 => Verdict::Ok,
        1..=3 => Verdict::Flag,
        _ => Verdict::Block,
    };

    OriginalityReport {
        total_tenant_strings: tenant_strings.len() as u32,
        total_corpus_strings: corpus_strings.len() as u32,
        min_ngram_words: n as u32,
        overlaps,
        verdict,
    }
}

/// Produce all word-shingles of length `n` from the input string.
/// Lowercase + whitespace-split; punctuation stays attached to
/// words (it's part of the verbatim phrase).
fn shingles(s: &str, n: usize) -> Vec<String> {
    let lower = s.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    if words.len() < n {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(words.len() - n + 1);
    for window in words.windows(n) {
        out.push(window.join(" "));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_inputs_return_ok() {
        let report = check_originality(&[], &[], 6);
        assert_eq!(report.verdict, Verdict::Ok);
        assert!(report.overlaps.is_empty());
    }

    #[test]
    fn no_overlap_returns_ok() {
        let tenant = vec!["The quick brown fox jumps over the lazy dog".to_owned()];
        let corpus =
            vec!["Lorem ipsum dolor sit amet consectetur adipiscing elit".to_owned()];
        let report = check_originality(&tenant, &corpus, 6);
        assert_eq!(report.verdict, Verdict::Ok);
    }

    #[test]
    fn single_overlap_returns_flag() {
        let tenant = vec!["Plans for every team with flexible pricing tiers".to_owned()];
        let corpus = vec!["We offer plans for every team with flexible pricing".to_owned()];
        let report = check_originality(&tenant, &corpus, 6);
        assert_eq!(report.verdict, Verdict::Flag);
        assert!(!report.overlaps.is_empty());
    }

    #[test]
    fn many_overlaps_return_block() {
        // Repeated verbatim phrases — should block.
        let tenant = vec![
            "We help your team build better software faster every day".to_owned(),
            "Trusted by thousands of teams worldwide to scale your business".to_owned(),
            "Get started in minutes with our easy-to-use platform today".to_owned(),
            "Built for developers and product teams of every size".to_owned(),
        ];
        let corpus = tenant.clone();
        let report = check_originality(&tenant, &corpus, 6);
        assert_eq!(report.verdict, Verdict::Block);
    }

    #[test]
    fn shingles_below_threshold_skipped() {
        let tenant = vec!["short text".to_owned()];
        let corpus = vec!["short text".to_owned()];
        let report = check_originality(&tenant, &corpus, 6);
        // "short text" has 2 words < 6 → no shingles → no overlaps.
        assert_eq!(report.verdict, Verdict::Ok);
    }

    #[test]
    fn case_insensitive_match() {
        // 7-word phrase + n=6 = 2 shingles → 2 unique overlaps → Flag.
        let tenant = vec!["The Quick Brown Fox Jumps Over Walls".to_owned()];
        let corpus = vec!["the quick brown fox jumps over walls".to_owned()];
        let report = check_originality(&tenant, &corpus, 6);
        assert_eq!(report.verdict, Verdict::Flag);
    }

    #[test]
    fn min_ngram_clamped_low() {
        // 1-word ngram is too noisy; should clamp to 2.
        let tenant = vec!["alpha beta gamma".to_owned()];
        let corpus = vec!["delta alpha epsilon".to_owned()];
        let report = check_originality(&tenant, &corpus, 1);
        assert_eq!(report.min_ngram_words, 2);
    }

    #[test]
    fn min_ngram_clamped_high() {
        let report = check_originality(&[], &[], 100);
        assert_eq!(report.min_ngram_words, 20);
    }

    #[test]
    fn verdict_slugs_stable() {
        assert_eq!(Verdict::Ok.slug(), "ok");
        assert_eq!(Verdict::Flag.slug(), "flag");
        assert_eq!(Verdict::Block.slug(), "block");
    }

    #[test]
    fn overlap_count_tracked() {
        let tenant = vec![
            "foo bar baz qux quux corge grault".to_owned(),
            "foo bar baz qux quux corge grault".to_owned(),
        ];
        let corpus = vec!["foo bar baz qux quux corge grault".to_owned()];
        let report = check_originality(&tenant, &corpus, 6);
        // The shingle "foo bar baz qux quux corge" appears 2x in tenant,
        // 1x in corpus. Reported once with counts.
        assert!(!report.overlaps.is_empty());
        let first = &report.overlaps[0];
        assert_eq!(first.tenant_count, 2);
        assert_eq!(first.corpus_count, 1);
    }
}
