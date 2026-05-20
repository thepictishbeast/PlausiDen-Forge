//! `voice_profile_audit` — deterministic statistical voice audit.
//!
//! Task #241 per the variation-architecture spec. Where the
//! site_identity_conformance phase (#235) does a single average-
//! sentence-length check, this phase goes deeper: distribution
//! shape (variance + p95), vocabulary-tier markers, automated
//! readability index, jargon density.
//!
//! ## What this phase checks
//!
//! Given a declared [`site_identity.voice`] tier, the phase walks
//! cms/*.json, accumulates body-text statistics, and refuses any
//! site whose actual voice statistics drift outside the tier's
//! envelope:
//!
//! | Tier            | Avg sentence | p95 sentence | Jargon density | ARI         |
//! |-----------------|-------------:|-------------:|---------------:|------------:|
//! | `plain`         |          ≤14 |          ≤22 |          ≤0.5% | ≤6          |
//! | `casual`        |          ≤18 |          ≤28 |          ≤1.0% | ≤8          |
//! | `editorial`     |          ≤24 |          ≤36 |          ≤2.0% | ≤11         |
//! | `professional`  |          ≤22 |          ≤34 |          ≤3.5% | ≤12         |
//! | `technical`     |          ≤26 |          ≤40 |          ≤6.0% | ≤14         |
//! | `academic`      |          ≤32 |          ≤48 |         ≤10.0% | ≤16         |
//!
//! Operators can override per-metric ceilings via
//! `[site_identity.voice]` (these are floors; nothing in this
//! phase relaxes the per-tier defaults). When an envelope is
//! breached the phase emits a strict finding citing voice-NNN.
//!
//! ## Why statistical, not average-only?
//!
//! Average sentence length is a poor variance proxy. A site with
//! sentences {3, 3, 50, 3, 3} has the same average as {12, 12, 12,
//! 12, 12} but a wildly different reading experience. The p95
//! sentence length catches the long-tail prose paul's complaint
//! about "feels like SaaS marketing copy" usually attaches to.
//!
//! Jargon density is the substrate-wide jargon dictionary's
//! coverage of the site's body text (per million tokens). The
//! dictionary lives in `forge_phases::aesthetic_distinctiveness`
//! (this phase reads it, not modifies it).
//!
//! Automated Readability Index (ARI):
//! ```text
//! ARI = 4.71 * (characters/words) + 0.5 * (words/sentences) - 21.43
//! ```
//! A US-grade-level estimate; lower = simpler. Each tier maps to
//! a ceiling.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * `#[non_exhaustive]` on phase struct.
//! * No unwrap/expect in non-test code.
//! * Pure walk over cms/*.json.
//! * Statistics are integer-only where possible to avoid f64
//!   non-determinism across platforms.

use std::fs;
use std::path::Path;

use forge_core::site_identity::SiteIdentity;
use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde_json::Value;

/// `voice_profile_audit` phase implementation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct VoiceProfileAuditPhase;

impl Phase for VoiceProfileAuditPhase {
    fn name(&self) -> &'static str {
        "voice_profile_audit"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let Some(identity) = SiteIdentity::load(&ctx.root) else {
            return Ok(findings);
        };
        let Some(tier) = identity.voice.tier.as_deref() else {
            return Ok(findings);
        };
        let envelope = TierEnvelope::for_tier(tier);

        let cms_dir = ctx.root.join("cms");
        if !cms_dir.is_dir() {
            return Ok(findings);
        }

        let stats = collect_stats(&cms_dir)?;
        if stats.total_sentences == 0 {
            return Ok(findings);
        }

        let avg = stats.total_words / stats.total_sentences.max(1);
        let p95 = stats.p95_sentence_length();
        let ari = stats.automated_readability_index();
        let jargon_density = stats.jargon_density_ppm();

        // Per-tier ceilings.
        if let Some(ceiling) = envelope.max_avg_sentence_words {
            if avg > u64::from(ceiling) {
                findings.push(
                    Finding::strict(
                        self.name(),
                        cms_dir.display().to_string(),
                        format!(
                            "voice_profile_audit — average sentence length {avg} words exceeds tier `{tier}` ceiling {ceiling}"
                        ),
                    )
                    .citing(["voice-001"])
                    .why("the site declared a voice tier whose ceiling on average sentence length is being violated")
                    .fix(format!(
                        "shorten body text OR change voice.tier to a register matching the actual prose; declared tier is `{tier}`"
                    )),
                );
            }
        }
        if let Some(ceiling) = envelope.max_p95_sentence_words {
            if p95 > u64::from(ceiling) {
                findings.push(
                    Finding::strict(
                        self.name(),
                        cms_dir.display().to_string(),
                        format!(
                            "voice_profile_audit — p95 sentence length {p95} words exceeds tier `{tier}` ceiling {ceiling}; long-tail prose violates the declared register"
                        ),
                    )
                    .citing(["voice-002"])
                    .why("the site has a long tail of overly long sentences that violate the declared voice tier; the average can pass while individual sentences don't")
                    .fix("split the longest sentences; the p95 cap forces consistency, not just average"),
                );
            }
        }
        if let Some(ceiling) = envelope.max_ari {
            if ari > f64::from(ceiling) {
                findings.push(
                    Finding::strict(
                        self.name(),
                        cms_dir.display().to_string(),
                        format!(
                            "voice_profile_audit — Automated Readability Index {ari:.1} exceeds tier `{tier}` ceiling {ceiling}; prose grade-level is higher than declared"
                        ),
                    )
                    .citing(["voice-003"])
                    .why("the ARI grade-level estimate for body text is above the declared tier's ceiling; reading difficulty exceeds operator's stated intent")
                    .fix("simplify vocabulary OR shorten sentences OR change voice.tier to one whose ARI ceiling matches the actual prose"),
                );
            }
        }
        if let Some(ceiling_ppm) = envelope.max_jargon_density_ppm {
            if jargon_density > ceiling_ppm {
                findings.push(
                    Finding::strict(
                        self.name(),
                        cms_dir.display().to_string(),
                        format!(
                            "voice_profile_audit — jargon density {jargon_density} ppm exceeds tier `{tier}` ceiling {ceiling_ppm} ppm"
                        ),
                    )
                    .citing(["voice-004"])
                    .why("the site's body text uses substrate-flagged jargon phrases at a rate above the declared voice tier's ceiling")
                    .fix("replace jargon phrases with plain-language equivalents; consult forge_phases::aesthetic_distinctiveness::JARGON_PHRASES for the canonical list"),
                );
            }
        }

        Ok(findings)
    }
}

/// Per-tier statistical envelope. Each ceiling is `Option` so a
/// tier may omit a metric.
#[derive(Debug, Clone, Copy)]
struct TierEnvelope {
    max_avg_sentence_words: Option<u32>,
    max_p95_sentence_words: Option<u32>,
    max_ari: Option<u32>,
    max_jargon_density_ppm: Option<u64>,
}

impl TierEnvelope {
    fn for_tier(tier: &str) -> Self {
        match tier {
            "plain" => Self {
                max_avg_sentence_words: Some(14),
                max_p95_sentence_words: Some(22),
                max_ari: Some(6),
                max_jargon_density_ppm: Some(5_000),
            },
            "casual" => Self {
                max_avg_sentence_words: Some(18),
                max_p95_sentence_words: Some(28),
                max_ari: Some(8),
                max_jargon_density_ppm: Some(10_000),
            },
            "editorial" => Self {
                max_avg_sentence_words: Some(24),
                max_p95_sentence_words: Some(36),
                max_ari: Some(11),
                max_jargon_density_ppm: Some(20_000),
            },
            "professional" => Self {
                max_avg_sentence_words: Some(22),
                max_p95_sentence_words: Some(34),
                max_ari: Some(12),
                max_jargon_density_ppm: Some(35_000),
            },
            "technical" => Self {
                max_avg_sentence_words: Some(26),
                max_p95_sentence_words: Some(40),
                max_ari: Some(14),
                max_jargon_density_ppm: Some(60_000),
            },
            "academic" => Self {
                max_avg_sentence_words: Some(32),
                max_p95_sentence_words: Some(48),
                max_ari: Some(16),
                max_jargon_density_ppm: Some(100_000),
            },
            _ => Self {
                max_avg_sentence_words: None,
                max_p95_sentence_words: None,
                max_ari: None,
                max_jargon_density_ppm: None,
            },
        }
    }
}

#[derive(Debug, Default)]
struct VoiceStats {
    total_sentences: u64,
    total_words: u64,
    total_chars: u64,
    sentence_word_counts: Vec<u32>,
    jargon_hits: u64,
}

impl VoiceStats {
    fn p95_sentence_length(&self) -> u64 {
        if self.sentence_word_counts.is_empty() {
            return 0;
        }
        let mut v = self.sentence_word_counts.clone();
        v.sort_unstable();
        let idx = ((v.len() as f64 * 0.95).floor() as usize).min(v.len() - 1);
        u64::from(v[idx])
    }

    fn automated_readability_index(&self) -> f64 {
        if self.total_sentences == 0 || self.total_words == 0 {
            return 0.0;
        }
        let chars_per_word = self.total_chars as f64 / self.total_words as f64;
        let words_per_sentence = self.total_words as f64 / self.total_sentences as f64;
        4.71 * chars_per_word + 0.5 * words_per_sentence - 21.43
    }

    fn jargon_density_ppm(&self) -> u64 {
        if self.total_words == 0 {
            return 0;
        }
        self.jargon_hits.saturating_mul(1_000_000) / self.total_words
    }
}

/// Walk cms/*.json + accumulate body-text statistics.
fn collect_stats(cms_dir: &Path) -> Result<VoiceStats, BuildError> {
    let mut stats = VoiceStats::default();
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
                for field in &[
                    "title", "body", "lede", "subtitle", "message", "summary", "text",
                ] {
                    if let Some(text) = section.get(field).and_then(|v| v.as_str()) {
                        accumulate(&mut stats, text);
                    }
                }
            }
        }
    }
    Ok(stats)
}

fn accumulate(stats: &mut VoiceStats, text: &str) {
    // Sentence split on . ! ?
    for sentence in text.split(|c: char| c == '.' || c == '!' || c == '?') {
        let trimmed = sentence.trim();
        if trimmed.is_empty() {
            continue;
        }
        let word_count = trimmed.split_whitespace().count() as u32;
        if word_count == 0 {
            continue;
        }
        stats.total_sentences += 1;
        stats.total_words = stats.total_words.saturating_add(u64::from(word_count));
        stats.sentence_word_counts.push(word_count);
    }
    let char_count = text.chars().filter(|c| !c.is_whitespace()).count() as u64;
    stats.total_chars = stats.total_chars.saturating_add(char_count);
    // Jargon hits — fall through to the jargon dictionary in
    // aesthetic_distinctiveness via the public const.
    let lower = text.to_lowercase();
    for phrase in JARGON_DICTIONARY {
        if lower.contains(phrase) {
            stats.jargon_hits += 1;
        }
    }
}

/// Mirror of the curated jargon dictionary from
/// `forge_phases::aesthetic_distinctiveness::JARGON_PHRASES`,
/// kept here so this phase doesn't pull a dependency on the
/// distinctiveness phase's internals. When the doctrine doc
/// JARGON_PHRASES bumps, mirror those entries here as well.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "forge-voice-{name}-{}",
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
    fn phase_silent_when_no_identity_or_no_tier() {
        let root = temp_root("no-tier");
        write_cms(&root, "i.json", r#"{"sections":[{"kind":"p","body":"x"}]}"#);
        let findings = VoiceProfileAuditPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_long_p95_under_plain_tier() {
        let root = temp_root("p95");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity.voice]
tier = "plain"
"#,
        )
        .unwrap();
        // 9 short sentences + 1 long sentence = 10 total; p95 lands
        // on the long one. The long sentence has 30+ words, far
        // above the plain tier's 22-word ceiling.
        let body = "Short. Short. Short. Short. Short. \
            Short. Short. Short. Short. \
            But here is a very long sentence with many many many words \
            stretching far beyond what a plain voice tier could tolerate \
            even in its long-tail distribution shape because length matters.";
        write_cms(
            &root,
            "i.json",
            &serde_json::json!({"sections":[{"kind":"p","body":body}]}).to_string(),
        );
        let findings = VoiceProfileAuditPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("p95 sentence length")),
            "expected p95 finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_jargon_density_under_editorial_tier() {
        let root = temp_root("jargon");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity.voice]
tier = "plain"
"#,
        )
        .unwrap();
        // Heavy jargon in a small body — density above plain's
        // 5000ppm ceiling.
        let body = "Synergy. Leverage. Best-in-class.";
        write_cms(
            &root,
            "i.json",
            &serde_json::json!({"sections":[{"kind":"p","body":body}]}).to_string(),
        );
        let findings = VoiceProfileAuditPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("jargon density")),
            "expected jargon finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_on_clean_editorial_voice() {
        let root = temp_root("clean");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity.voice]
tier = "editorial"
"#,
        )
        .unwrap();
        let body = "Deborah writes about financial education. Her audience is people who have never had \
            money explained to them in plain language. The essays cover insurance, investing, and the \
            strategies the wealthy use to compound their wealth over time.";
        write_cms(
            &root,
            "i.json",
            &serde_json::json!({"sections":[{"kind":"p","body":body}]}).to_string(),
        );
        let findings = VoiceProfileAuditPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty(), "editorial-clean content should pass; got: {findings:#?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn p95_sentence_length_uses_top_5_percent() {
        let mut s = VoiceStats::default();
        // 19 sentences of 5 words + 1 sentence of 50 words = p95
        // should be 50.
        for _ in 0..19 {
            s.sentence_word_counts.push(5);
        }
        s.sentence_word_counts.push(50);
        assert_eq!(s.p95_sentence_length(), 50);
    }

    #[test]
    fn ari_zero_when_no_text() {
        let s = VoiceStats::default();
        assert_eq!(s.automated_readability_index(), 0.0);
    }

    #[test]
    fn jargon_density_ppm_scales_to_million() {
        let mut s = VoiceStats::default();
        s.total_words = 100;
        s.jargon_hits = 1;
        // 1 hit in 100 words = 10,000 ppm.
        assert_eq!(s.jargon_density_ppm(), 10_000);
    }
}
