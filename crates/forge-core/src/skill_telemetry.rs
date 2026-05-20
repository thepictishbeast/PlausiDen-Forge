//! `skill_telemetry` — structured skill-invocation log.
//!
//! Task #278 per the long-term-compounding arc. Records every
//! AI-skill invocation (Claude Code skills, MCP typed-tool
//! calls, future LFI policy queries) as one JSONL line so the
//! substrate can analyze which skills run, how often, with what
//! outcomes — driving the skill-library evolution surface that
//! the MCP cluster (#284-#288) consumes.
//!
//! ## Wire shape
//!
//! Append-only JSONL at an operator-supplied path
//! (default `reports/skill-telemetry.jsonl`). Each line is one
//! [`SkillInvocation`] record. Compact, narrow, hashable —
//! future MCP integrations stream events into this file without
//! per-invocation coordination cost.
//!
//! ## API
//!
//! * [`SkillInvocation::record`] — construct from skill_id +
//!   outcome + duration.
//! * [`append_invocation`] — write one record to the JSONL file.
//! * [`read_invocations`] — parse the full log.
//! * [`recent_for_skill`] — last N invocations of one skill.
//! * [`aggregate_outcomes`] — per-skill outcome counts for
//!   the recent N records.
//!
//! No I/O in this module beyond the explicit JSONL file. Pure
//! functions over Vec<SkillInvocation> for analytics.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `#[non_exhaustive]` on every public type.
//! * No unwrap/expect in non-test code.

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead as _, BufReader, Write as _};
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Outcome of a skill invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SkillOutcome {
    /// Skill ran and returned a successful result.
    Success,
    /// Skill ran and returned an error.
    Failure,
    /// Skill was skipped (preconditions unmet).
    Skipped,
    /// Skill was cancelled mid-execution.
    Cancelled,
}

impl SkillOutcome {
    /// Stable kebab-case slug for serialization.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Skipped => "skipped",
            Self::Cancelled => "cancelled",
        }
    }
}

/// One skill invocation record. Stable wire shape; new optional
/// fields land additive per [[backward-compat-version-discipline]].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SkillInvocation {
    /// Skill identifier (e.g. `"add-loom-primitive"`,
    /// `"forge-fingerprint-verify"`, `"author-cms-content"`).
    pub skill_id: String,
    /// ISO-8601 RFC-3339 UTC start timestamp.
    pub started_at: String,
    /// ISO-8601 RFC-3339 UTC finish timestamp.
    pub finished_at: String,
    /// Duration in milliseconds. 0 when start == finish.
    pub duration_ms: u64,
    /// Outcome.
    pub outcome: SkillOutcome,
    /// Optional context hash (SHA-256 hex) — what state the
    /// skill ran against. Empty when not relevant.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub context_hash: String,
    /// Optional brief summary of arguments (NOT the full payload;
    /// keep < 200 chars). Empty when not relevant.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub args_summary: String,
    /// Optional human-readable result snippet. < 200 chars.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub result_summary: String,
}

impl SkillInvocation {
    /// Construct a record with the minimum required fields. Use
    /// the builder methods for optional fields.
    #[must_use]
    pub fn record(
        skill_id: impl Into<String>,
        started_at: impl Into<String>,
        finished_at: impl Into<String>,
        duration_ms: u64,
        outcome: SkillOutcome,
    ) -> Self {
        Self {
            skill_id: skill_id.into(),
            started_at: started_at.into(),
            finished_at: finished_at.into(),
            duration_ms,
            outcome,
            context_hash: String::new(),
            args_summary: String::new(),
            result_summary: String::new(),
        }
    }

    /// Attach a context hash.
    #[must_use]
    pub fn with_context_hash(mut self, hash: impl Into<String>) -> Self {
        self.context_hash = hash.into();
        self
    }

    /// Attach an args summary (truncated to 200 chars).
    #[must_use]
    pub fn with_args(mut self, args: impl Into<String>) -> Self {
        let s: String = args.into();
        self.args_summary = s.chars().take(200).collect();
        self
    }

    /// Attach a result summary (truncated to 200 chars).
    #[must_use]
    pub fn with_result(mut self, result: impl Into<String>) -> Self {
        let s: String = result.into();
        self.result_summary = s.chars().take(200).collect();
        self
    }
}

/// Append one invocation record to the JSONL log.
pub fn append_invocation(path: &Path, invocation: &SkillInvocation) -> Result<(), std::io::Error> {
    let line = serde_json::to_string(invocation)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

/// Read every invocation from the JSONL log in append order.
pub fn read_invocations(path: &Path) -> Result<Vec<SkillInvocation>, std::io::Error> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut out = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(rec) = serde_json::from_str::<SkillInvocation>(&line) {
            out.push(rec);
        }
    }
    Ok(out)
}

/// Last N invocations of a specific skill, in append order.
#[must_use]
pub fn recent_for_skill(
    invocations: &[SkillInvocation],
    skill_id: &str,
    n: usize,
) -> Vec<SkillInvocation> {
    let mut matches: Vec<&SkillInvocation> = invocations
        .iter()
        .filter(|i| i.skill_id == skill_id)
        .collect();
    let start = matches.len().saturating_sub(n);
    matches.drain(..start);
    matches.into_iter().cloned().collect()
}

/// Per-skill outcome counts (success/failure/skipped/cancelled).
/// Returns BTreeMap keyed by skill_id then by outcome slug.
#[must_use]
pub fn aggregate_outcomes(
    invocations: &[SkillInvocation],
) -> BTreeMap<String, BTreeMap<&'static str, u64>> {
    let mut out: BTreeMap<String, BTreeMap<&'static str, u64>> = BTreeMap::new();
    for inv in invocations {
        let by_skill = out.entry(inv.skill_id.clone()).or_default();
        *by_skill.entry(inv.outcome.slug()).or_insert(0) += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_invocation(skill: &str, outcome: SkillOutcome) -> SkillInvocation {
        SkillInvocation::record(
            skill,
            "2026-05-20T00:00:00Z",
            "2026-05-20T00:00:01Z",
            1000,
            outcome,
        )
    }

    #[test]
    fn record_builder_chains_optional_fields() {
        let r = sample_invocation("test", SkillOutcome::Success)
            .with_context_hash("abc123")
            .with_args("--foo bar")
            .with_result("ok");
        assert_eq!(r.context_hash, "abc123");
        assert_eq!(r.args_summary, "--foo bar");
        assert_eq!(r.result_summary, "ok");
    }

    #[test]
    fn args_and_result_truncate_to_200_chars() {
        let long = "x".repeat(500);
        let r = sample_invocation("test", SkillOutcome::Success)
            .with_args(&long)
            .with_result(&long);
        assert_eq!(r.args_summary.len(), 200);
        assert_eq!(r.result_summary.len(), 200);
    }

    #[test]
    fn append_and_read_round_trip() {
        let path =
            std::env::temp_dir().join(format!("forge-skill-telemetry-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        append_invocation(&path, &sample_invocation("a", SkillOutcome::Success)).unwrap();
        append_invocation(&path, &sample_invocation("b", SkillOutcome::Failure)).unwrap();
        append_invocation(&path, &sample_invocation("a", SkillOutcome::Skipped)).unwrap();
        let entries = read_invocations(&path).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[1].skill_id, "b");
        assert_eq!(entries[2].outcome, SkillOutcome::Skipped);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn recent_for_skill_filters_and_caps() {
        let mut entries = Vec::new();
        for i in 0..10 {
            let outcome = if i % 2 == 0 {
                SkillOutcome::Success
            } else {
                SkillOutcome::Failure
            };
            entries.push(sample_invocation(if i < 5 { "a" } else { "b" }, outcome));
        }
        let recent_a = recent_for_skill(&entries, "a", 3);
        assert_eq!(recent_a.len(), 3);
        // All should be skill_id "a".
        for r in &recent_a {
            assert_eq!(r.skill_id, "a");
        }
        let recent_b = recent_for_skill(&entries, "b", 100);
        // Only 5 b-entries; cap doesn't multiply.
        assert_eq!(recent_b.len(), 5);
    }

    #[test]
    fn aggregate_outcomes_counts_per_skill_per_outcome() {
        let entries = vec![
            sample_invocation("a", SkillOutcome::Success),
            sample_invocation("a", SkillOutcome::Success),
            sample_invocation("a", SkillOutcome::Failure),
            sample_invocation("b", SkillOutcome::Success),
            sample_invocation("b", SkillOutcome::Skipped),
        ];
        let agg = aggregate_outcomes(&entries);
        assert_eq!(
            agg.get("a").and_then(|m| m.get("success")).copied(),
            Some(2)
        );
        assert_eq!(
            agg.get("a").and_then(|m| m.get("failure")).copied(),
            Some(1)
        );
        assert_eq!(
            agg.get("b").and_then(|m| m.get("skipped")).copied(),
            Some(1)
        );
    }

    #[test]
    fn read_invocations_empty_for_missing_file() {
        let path = std::env::temp_dir().join("forge-skill-telemetry-missing-xyz");
        let _ = fs::remove_file(&path);
        let entries = read_invocations(&path).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn outcome_serialization_uses_snake_case() {
        let r = sample_invocation("test", SkillOutcome::Cancelled);
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"outcome\":\"cancelled\""));
    }

    #[test]
    fn skips_serialization_of_empty_optional_fields() {
        let r = sample_invocation("test", SkillOutcome::Success);
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("context_hash"));
        assert!(!json.contains("args_summary"));
        assert!(!json.contains("result_summary"));
    }
}
