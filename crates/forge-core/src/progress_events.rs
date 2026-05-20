//! `progress_events` — structured live-progress event log.
//!
//! Task #288 per the MCP cluster (#284-#288). Finer-grained
//! companion to skill_telemetry (#278): where skill_telemetry
//! records ONE entry per skill invocation (started + finished +
//! outcome), progress_events records MANY events DURING one
//! invocation so operators + Claude can stream the log for
//! live status.
//!
//! Used by long-running operations:
//!
//! * `forge build` emits one event per phase (started / ok /
//!   finding-count / completed).
//! * `crawler.capture` emits one event per viewport.
//! * `reference-match` emits one event per extraction axis.
//! * skills that loop emit per-iteration progress events.
//!
//! Append-only JSONL at an operator-supplied path (default
//! `reports/progress-events.jsonl`). One [`ProgressEvent`] per
//! line. Tailable in real time for terminal UIs; queryable
//! after the fact via [`read_events`] / [`recent_for_source`].
//!
//! ## Wire shape
//!
//! ```text
//! {"timestamp":"2026-05-20T13:45:00Z","source":"forge.build",
//!  "stage":"phase.editorial_purity_gate","status":"completed",
//!  "progress_pct":42,"detail":"0 findings","correlation_id":"build-xyz"}
//! ```
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `#[non_exhaustive]` on every public type.
//! * No unwrap/expect in non-test code.
//! * Pure functions over filesystem reads — no spawn, no network.

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead as _, BufReader, Write as _};
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Status of a single progress event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProgressStatus {
    /// Stage starting.
    Started,
    /// Stage in flight (intermediate ticks).
    InProgress,
    /// Stage completed successfully.
    Completed,
    /// Stage failed.
    Failed,
    /// Stage skipped (preconditions unmet).
    Skipped,
}

impl ProgressStatus {
    /// Stable kebab-case slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

/// One progress event record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ProgressEvent {
    /// ISO-8601 RFC-3339 UTC timestamp.
    pub timestamp: String,
    /// Source identifier (e.g. `"forge.build"`, `"crawler.capture"`,
    /// `"skill.reference-match"`).
    pub source: String,
    /// Stage within the source (e.g. `"phase.editorial_purity_gate"`,
    /// `"viewport.1280"`, `"axis.palette"`).
    pub stage: String,
    /// Current status.
    pub status: ProgressStatus,
    /// Progress percentage 0-100. 0 = unset; consumers ignore
    /// when status is Started or Skipped.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub progress_pct: u8,
    /// Optional human-readable detail (< 200 chars).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
    /// Optional correlation id (e.g. build run id) linking
    /// events from the same logical invocation.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub correlation_id: String,
}

fn is_zero_u8(n: &u8) -> bool {
    *n == 0
}

impl ProgressEvent {
    /// Construct with the minimum required fields.
    #[must_use]
    pub fn new(
        timestamp: impl Into<String>,
        source: impl Into<String>,
        stage: impl Into<String>,
        status: ProgressStatus,
    ) -> Self {
        Self {
            timestamp: timestamp.into(),
            source: source.into(),
            stage: stage.into(),
            status,
            progress_pct: 0,
            detail: String::new(),
            correlation_id: String::new(),
        }
    }

    /// Attach a progress percentage (clamped 0-100).
    #[must_use]
    pub fn with_progress(mut self, pct: u8) -> Self {
        self.progress_pct = pct.min(100);
        self
    }

    /// Attach a detail string (truncated to 200 chars).
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        let s: String = detail.into();
        self.detail = s.chars().take(200).collect();
        self
    }

    /// Attach a correlation id.
    #[must_use]
    pub fn with_correlation(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = id.into();
        self
    }
}

/// Append one event to the JSONL log.
pub fn append_event(path: &Path, event: &ProgressEvent) -> Result<(), std::io::Error> {
    let line = serde_json::to_string(event)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

/// Read all events from the JSONL log in append order. Empty
/// Vec for missing or empty file.
pub fn read_events(path: &Path) -> Result<Vec<ProgressEvent>, std::io::Error> {
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
        if let Ok(rec) = serde_json::from_str::<ProgressEvent>(&line) {
            out.push(rec);
        }
    }
    Ok(out)
}

/// Most recent N events for a specific source, in append order.
#[must_use]
pub fn recent_for_source(events: &[ProgressEvent], source: &str, n: usize) -> Vec<ProgressEvent> {
    let mut matches: Vec<&ProgressEvent> =
        events.iter().filter(|e| e.source == source).collect();
    let start = matches.len().saturating_sub(n);
    matches.drain(..start);
    matches.into_iter().cloned().collect()
}

/// Most recent N events for a specific correlation_id, in
/// append order. Empty id is treated as "match anything with
/// empty id" — useful for legacy events.
#[must_use]
pub fn recent_for_correlation(
    events: &[ProgressEvent],
    correlation_id: &str,
    n: usize,
) -> Vec<ProgressEvent> {
    let mut matches: Vec<&ProgressEvent> = events
        .iter()
        .filter(|e| e.correlation_id == correlation_id)
        .collect();
    let start = matches.len().saturating_sub(n);
    matches.drain(..start);
    matches.into_iter().cloned().collect()
}

/// Per-source status distribution for the supplied event slice.
/// Returns BTreeMap keyed by source then by status slug.
#[must_use]
pub fn aggregate_status_by_source(
    events: &[ProgressEvent],
) -> BTreeMap<String, BTreeMap<&'static str, u64>> {
    let mut out: BTreeMap<String, BTreeMap<&'static str, u64>> = BTreeMap::new();
    for e in events {
        let by_source = out.entry(e.source.clone()).or_default();
        *by_source.entry(e.status.slug()).or_insert(0) += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_event(source: &str, stage: &str, status: ProgressStatus) -> ProgressEvent {
        ProgressEvent::new("2026-05-20T13:00:00Z", source, stage, status)
    }

    #[test]
    fn builder_chains_optional_fields() {
        let e = sample_event("forge.build", "phase.tokens", ProgressStatus::Completed)
            .with_progress(42)
            .with_detail("0 findings")
            .with_correlation("build-xyz");
        assert_eq!(e.progress_pct, 42);
        assert_eq!(e.detail, "0 findings");
        assert_eq!(e.correlation_id, "build-xyz");
    }

    #[test]
    fn progress_pct_clamps_to_100() {
        let e = sample_event("x", "y", ProgressStatus::InProgress).with_progress(150);
        assert_eq!(e.progress_pct, 100);
    }

    #[test]
    fn detail_truncates_to_200_chars() {
        let long = "x".repeat(500);
        let e = sample_event("x", "y", ProgressStatus::Started).with_detail(&long);
        assert_eq!(e.detail.len(), 200);
    }

    #[test]
    fn append_and_read_round_trip() {
        let path = std::env::temp_dir().join(format!(
            "forge-progress-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        append_event(
            &path,
            &sample_event("forge.build", "tokens", ProgressStatus::Started),
        )
        .unwrap();
        append_event(
            &path,
            &sample_event("forge.build", "tokens", ProgressStatus::Completed)
                .with_progress(100),
        )
        .unwrap();
        append_event(
            &path,
            &sample_event("crawler.capture", "viewport.1280", ProgressStatus::Started),
        )
        .unwrap();
        let entries = read_events(&path).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[1].status, ProgressStatus::Completed);
        assert_eq!(entries[1].progress_pct, 100);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn recent_for_source_filters_and_caps() {
        let mut events = Vec::new();
        for i in 0..7 {
            events.push(sample_event(
                if i < 4 { "forge.build" } else { "crawler.capture" },
                &format!("stage-{i}"),
                ProgressStatus::Started,
            ));
        }
        let recent_build = recent_for_source(&events, "forge.build", 2);
        assert_eq!(recent_build.len(), 2);
        assert_eq!(recent_build[1].stage, "stage-3");
        let recent_capture = recent_for_source(&events, "crawler.capture", 100);
        // Cap doesn't multiply — there are only 3 events.
        assert_eq!(recent_capture.len(), 3);
    }

    #[test]
    fn recent_for_correlation_filters_to_one_run() {
        let events = vec![
            sample_event("a", "1", ProgressStatus::Started).with_correlation("run-1"),
            sample_event("a", "2", ProgressStatus::Started).with_correlation("run-2"),
            sample_event("a", "3", ProgressStatus::Completed).with_correlation("run-1"),
        ];
        let run1 = recent_for_correlation(&events, "run-1", 10);
        assert_eq!(run1.len(), 2);
        assert_eq!(run1[1].stage, "3");
    }

    #[test]
    fn aggregate_status_by_source_counts() {
        let events = vec![
            sample_event("a", "1", ProgressStatus::Started),
            sample_event("a", "1", ProgressStatus::Completed),
            sample_event("a", "2", ProgressStatus::Failed),
            sample_event("b", "1", ProgressStatus::Started),
        ];
        let agg = aggregate_status_by_source(&events);
        assert_eq!(agg.get("a").and_then(|m| m.get("started")).copied(), Some(1));
        assert_eq!(agg.get("a").and_then(|m| m.get("completed")).copied(), Some(1));
        assert_eq!(agg.get("a").and_then(|m| m.get("failed")).copied(), Some(1));
        assert_eq!(agg.get("b").and_then(|m| m.get("started")).copied(), Some(1));
    }

    #[test]
    fn read_events_empty_for_missing_file() {
        let path = std::env::temp_dir().join("forge-progress-missing-xyz");
        let _ = fs::remove_file(&path);
        let entries = read_events(&path).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn status_serialization_uses_snake_case() {
        let e = sample_event("x", "y", ProgressStatus::InProgress);
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"status\":\"in_progress\""));
    }

    #[test]
    fn skips_empty_optional_fields_in_serialization() {
        let e = sample_event("x", "y", ProgressStatus::Started);
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("progress_pct"));
        assert!(!json.contains("detail"));
        assert!(!json.contains("correlation_id"));
    }
}
