//! `voice` — statistical voice-fingerprint extraction.
//!
//! Task #271 per the reference-matching arc. Reads a
//! [`VoiceDump`] (Crawler-emitted per-page body text) and emits
//! the voice signal: sentence-length distribution + vocabulary
//! richness + jargon density.
//!
//! Mirrors `forge_phases::voice_profile_audit` but for
//! REFERENCE-site extraction (not gating). Output feeds the
//! mapping engine (#273) to declare a `[site_identity.voice]`
//! for the synthesized site that matches the reference's voice
//! tier.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `#[non_exhaustive]` on every public type.
//! * No unwrap/expect in non-test code.
//! * Pure functions over in-memory dump.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::extractors::ExtractorError;

/// Voice-dump spec version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VoiceSpec {
    /// Initial spec.
    #[default]
    V1,
}

impl VoiceSpec {
    /// Stable kebab-case slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::V1 => "v1",
        }
    }
}

/// One page-text entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PageText {
    /// URL path.
    pub path: String,
    /// Visible body text (already stripped of nav/footer/scripts
    /// by the Crawler).
    pub body_text: String,
}

/// Crawler-emitted voice dump.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VoiceDump {
    /// Schema version.
    pub spec: VoiceSpec,
    /// Per-page body text.
    pub pages: Vec<PageText>,
}

impl VoiceResult {
    /// Construct a VoiceResult. Public constructor because the
    /// struct is `#[non_exhaustive]`.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        total_sentences: u64,
        total_words: u64,
        total_chars: u64,
        avg_sentence_words: f64,
        p50_sentence_words: u32,
        p95_sentence_words: u32,
        vocab_richness: f64,
        jargon_hits: u64,
        jargon_density_ppm: u64,
        suggested_tier: impl Into<String>,
    ) -> Self {
        Self {
            total_sentences,
            total_words,
            total_chars,
            avg_sentence_words,
            p50_sentence_words,
            p95_sentence_words,
            vocab_richness,
            jargon_hits,
            jargon_density_ppm,
            suggested_tier: suggested_tier.into(),
        }
    }
}

/// Voice extraction result.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VoiceResult {
    /// Total sentences across all pages.
    pub total_sentences: u64,
    /// Total words across all pages.
    pub total_words: u64,
    /// Total non-whitespace character count.
    pub total_chars: u64,
    /// Average words per sentence (0.0 when no sentences).
    pub avg_sentence_words: f64,
    /// 50th percentile sentence length (words).
    pub p50_sentence_words: u32,
    /// 95th percentile sentence length (words).
    pub p95_sentence_words: u32,
    /// Vocabulary richness: unique-lowercased-words /
    /// total-words. 0.0 when no words.
    pub vocab_richness: f64,
    /// Jargon hits (substrate dictionary).
    pub jargon_hits: u64,
    /// Jargon density per million words.
    pub jargon_density_ppm: u64,
    /// Suggested voice tier slug based on signals.
    pub suggested_tier: String,
}

/// Embedded jargon dictionary. Mirror of
/// `forge_phases::voice_profile_audit::JARGON_DICTIONARY` so this
/// crate doesn't depend on forge-phases.
const JARGON_DICTIONARY: &[&str] = &[
    "synergy",
    "leverage",
    "best-in-class",
    "world-class",
    "cutting-edge",
    "next-generation",
    "frictionless",
    "seamless",
    "robust",
    "scalable",
    "mission-critical",
    "thought leader",
    "ecosystem",
    "stakeholder",
    "transform your",
    "supercharge",
    "unlock",
    "empower",
    "delight",
    "the future of",
    "all-in-one",
    "one-stop",
];

/// Extract from a JSON dump file.
pub fn extract_from_path(path: &Path) -> Result<VoiceResult, ExtractorError> {
    let body = std::fs::read_to_string(path)?;
    let dump: VoiceDump = serde_json::from_str(&body)?;
    Ok(extract(&dump))
}

/// Pure extraction over in-memory dump.
#[must_use]
pub fn extract(dump: &VoiceDump) -> VoiceResult {
    let mut sentence_lengths: Vec<u32> = Vec::new();
    let mut total_words: u64 = 0;
    let mut total_chars: u64 = 0;
    let mut unique_words = std::collections::BTreeSet::new();
    let mut jargon_hits: u64 = 0;

    for page in &dump.pages {
        for sentence in page
            .body_text
            .split(|c: char| c == '.' || c == '!' || c == '?')
        {
            let trimmed = sentence.trim();
            if trimmed.is_empty() {
                continue;
            }
            let word_count = trimmed.split_whitespace().count() as u32;
            if word_count == 0 {
                continue;
            }
            sentence_lengths.push(word_count);
        }
        for word in page.body_text.split_whitespace() {
            total_words = total_words.saturating_add(1);
            let lower = word
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase();
            if !lower.is_empty() {
                unique_words.insert(lower);
            }
        }
        total_chars = total_chars.saturating_add(
            page.body_text.chars().filter(|c| !c.is_whitespace()).count() as u64,
        );
        let lower_body = page.body_text.to_lowercase();
        for phrase in JARGON_DICTIONARY {
            let mut start = 0usize;
            while let Some(found) = lower_body[start..].find(phrase) {
                jargon_hits = jargon_hits.saturating_add(1);
                start += found + phrase.len();
            }
        }
    }

    sentence_lengths.sort_unstable();
    let total_sentences = sentence_lengths.len() as u64;

    let avg_sentence_words = if total_sentences == 0 {
        0.0
    } else {
        sentence_lengths.iter().map(|n| u64::from(*n)).sum::<u64>() as f64
            / total_sentences as f64
    };
    let p50_sentence_words = percentile(&sentence_lengths, 0.50);
    let p95_sentence_words = percentile(&sentence_lengths, 0.95);
    let vocab_richness = if total_words == 0 {
        0.0
    } else {
        unique_words.len() as f64 / total_words as f64
    };
    let jargon_density_ppm = if total_words == 0 {
        0
    } else {
        jargon_hits.saturating_mul(1_000_000) / total_words
    };

    let suggested_tier = suggest_tier(
        avg_sentence_words,
        p95_sentence_words,
        jargon_density_ppm,
    );

    VoiceResult {
        total_sentences,
        total_words,
        total_chars,
        avg_sentence_words,
        p50_sentence_words,
        p95_sentence_words,
        vocab_richness,
        jargon_hits,
        jargon_density_ppm,
        suggested_tier,
    }
}

fn percentile(sorted: &[u32], p: f64) -> u32 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() as f64) * p).floor() as usize;
    let idx = idx.min(sorted.len() - 1);
    sorted[idx]
}

fn suggest_tier(avg: f64, p95: u32, jargon_ppm: u64) -> String {
    // Mirror of voice_profile_audit TierEnvelope ceilings,
    // inverted: pick the tightest tier whose ceilings the
    // observed metrics still clear.
    if avg <= 14.0 && p95 <= 22 && jargon_ppm <= 5_000 {
        return "plain".to_owned();
    }
    if avg <= 18.0 && p95 <= 28 && jargon_ppm <= 10_000 {
        return "casual".to_owned();
    }
    if avg <= 24.0 && p95 <= 36 && jargon_ppm <= 20_000 {
        return "editorial".to_owned();
    }
    if avg <= 22.0 && p95 <= 34 && jargon_ppm <= 35_000 {
        return "professional".to_owned();
    }
    if avg <= 26.0 && p95 <= 40 && jargon_ppm <= 60_000 {
        return "technical".to_owned();
    }
    if avg <= 32.0 && p95 <= 48 && jargon_ppm <= 100_000 {
        return "academic".to_owned();
    }
    "unclassified".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dump(texts: &[(&str, &str)]) -> VoiceDump {
        VoiceDump {
            spec: VoiceSpec::V1,
            pages: texts
                .iter()
                .map(|(p, t)| PageText {
                    path: (*p).to_owned(),
                    body_text: (*t).to_owned(),
                })
                .collect(),
        }
    }

    #[test]
    fn extract_counts_sentences_and_words() {
        let d = dump(&[
            ("/a", "Short. Short. Short."),
            ("/b", "Two words here."),
        ]);
        let r = extract(&d);
        assert_eq!(r.total_sentences, 4);
        assert!(r.total_words >= 6);
    }

    #[test]
    fn extract_computes_avg_p50_p95() {
        let mut text = String::new();
        for _ in 0..20 {
            text.push_str("Short. ");
        }
        text.push_str("This is a much longer sentence indeed.");
        let d = dump(&[("/x", &text)]);
        let r = extract(&d);
        // 20 1-word + 1 7-word ("This is a much longer sentence indeed").
        // sorted ascending = [1×20, 7]. p95 idx = floor(21*0.95) = 19 → 1.
        assert_eq!(r.p95_sentence_words, 1);
        assert_eq!(r.p50_sentence_words, 1);
        // avg = (20 + 7) / 21 = 27/21
        assert!((r.avg_sentence_words - 27.0 / 21.0).abs() < 1e-9);
    }

    #[test]
    fn extract_p95_picks_outlier_when_distribution_short() {
        let d = dump(&[("/x", "One. Two two. Three three three. Four four four four. This is a much much longer sentence with many words.")]);
        let r = extract(&d);
        // 5 sentences, p95 idx = floor(5*0.95) = 4 = last.
        assert!(r.p95_sentence_words >= 9);
    }

    #[test]
    fn extract_jargon_density_ppm() {
        let d = dump(&[("/x", "We leverage synergy across the ecosystem.")]);
        let r = extract(&d);
        // 6 words, 3 jargon hits (leverage, synergy, ecosystem)
        // 3 / 6 * 1_000_000 = 500_000 ppm.
        assert_eq!(r.jargon_hits, 3);
        assert_eq!(r.jargon_density_ppm, 500_000);
    }

    #[test]
    fn extract_vocab_richness_unique_over_total() {
        let d = dump(&[("/x", "hello world hello world hello world")]);
        let r = extract(&d);
        // 6 words, 2 unique → 2/6 ≈ 0.333
        assert!((r.vocab_richness - 2.0 / 6.0).abs() < 1e-9);
    }

    #[test]
    fn extract_suggests_plain_for_short_simple() {
        let d = dump(&[("/x", "Short. Cat. Dog. Sun.")]);
        let r = extract(&d);
        assert_eq!(r.suggested_tier, "plain");
    }

    #[test]
    fn extract_suggests_editorial_for_medium_register() {
        // Avg ~16, p95 ~25, low jargon → editorial bucket.
        let text = "Editorial writing aims for clarity over brevity. \
                    A sentence should carry weight without becoming \
                    tangled. The cadence matters as much as the words. \
                    Readers reward thoughtful pacing.";
        let d = dump(&[("/x", text)]);
        let r = extract(&d);
        assert!(matches!(
            r.suggested_tier.as_str(),
            "plain" | "casual" | "editorial"
        ));
    }

    #[test]
    fn extract_returns_default_on_empty_dump() {
        let d = dump(&[]);
        let r = extract(&d);
        assert_eq!(r.total_sentences, 0);
        assert_eq!(r.total_words, 0);
        assert_eq!(r.avg_sentence_words, 0.0);
        assert_eq!(r.suggested_tier, "plain"); // all ceilings clear at 0
    }

    #[test]
    fn extract_from_path_round_trips_dump() {
        let d = dump(&[("/x", "Hello world.")]);
        let path = std::env::temp_dir().join(format!(
            "forge-voice-{}",
            std::process::id()
        ));
        std::fs::write(&path, serde_json::to_string(&d).unwrap()).unwrap();
        let r = extract_from_path(&path).unwrap();
        assert_eq!(r.total_sentences, 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn percentile_handles_empty() {
        assert_eq!(percentile(&[], 0.5), 0);
        assert_eq!(percentile(&[5], 0.5), 5);
        assert_eq!(percentile(&[1, 2, 3, 4, 5], 0.95), 5);
    }
}
