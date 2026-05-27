//! `session_progress` — typed progress representation for AI-agent
//! session-state offload.
//!
//! Per task #390 (AI-DX accessibility #3 of 4). AI agents lose
//! context between sessions; the substrate stores progress
//! (task list with status + summary + blockers + cursor) so the
//! next session picks up cleanly.
//!
//! ## Why session-state offload
//!
//! Without offload, every new agent session re-derives "what
//! state are we in?" — re-reading commits, re-running audits,
//! re-asking the operator. With typed progress in a JSONL file
//! the agent can load, the resumption is bounded + deterministic.
//!
//! ## File format
//!
//! JSONL, one ProgressEvent per line, append-only. ProgressItem
//! state derives from event replay — the latest event for a
//! given item_id wins.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Status of one progress item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ItemStatus {
    /// Not started.
    Pending,
    /// Being worked on.
    InProgress,
    /// Done.
    Completed,
    /// Cancelled / abandoned.
    Cancelled,
    /// Blocked on something else.
    Blocked,
}

impl ItemStatus {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Blocked => "blocked",
        }
    }

    /// Parse from slug.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "in_progress" => Some(Self::InProgress),
            "completed" => Some(Self::Completed),
            "cancelled" => Some(Self::Cancelled),
            "blocked" => Some(Self::Blocked),
            _ => None,
        }
    }
}

/// One append-only event in the progress log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProgressEvent {
    /// Event sequence ID.
    pub event_id: u32,
    /// RFC-3339 UTC timestamp.
    pub recorded_at: String,
    /// Tenant + operator context.
    pub session_id: String,
    /// Item ID this event applies to. Multiple events per item
    /// represent state transitions; the latest wins.
    pub item_id: String,
    /// Item summary at the time of the event.
    pub summary: String,
    /// Item status this event sets.
    pub status: ItemStatus,
    /// Optional blocker description.
    #[serde(default)]
    pub blocker: Option<String>,
    /// Optional links to commits, PRs, tasks.
    #[serde(default)]
    pub refs: Vec<String>,
}

/// Resolved progress-item state (after event-log replay).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProgressItem {
    /// Item ID.
    pub item_id: String,
    /// Latest summary.
    pub summary: String,
    /// Latest status.
    pub status: ItemStatus,
    /// Latest blocker (if any).
    pub blocker: Option<String>,
    /// Latest refs.
    pub refs: Vec<String>,
    /// First-seen timestamp (event 1 for this item).
    pub created_at: String,
    /// Latest-event timestamp.
    pub updated_at: String,
}

/// Read all events from JSONL.
pub fn read_events(path: &Path) -> std::io::Result<Vec<ProgressEvent>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(path)?;
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let e: ProgressEvent = serde_json::from_str(trimmed).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?;
        out.push(e);
    }
    Ok(out)
}

/// Append a new event. Returns the assigned event_id.
pub fn record_event(
    path: &Path,
    session_id: &str,
    item_id: &str,
    summary: &str,
    status: ItemStatus,
    blocker: Option<&str>,
    refs: Vec<String>,
    now_rfc3339: &str,
) -> std::io::Result<ProgressEvent> {
    let existing = read_events(path)?;
    let next_id = existing.iter().map(|e| e.event_id).max().unwrap_or(0) + 1;

    let entry = ProgressEvent {
        event_id: next_id,
        recorded_at: now_rfc3339.to_owned(),
        session_id: session_id.to_owned(),
        item_id: item_id.to_owned(),
        summary: summary.to_owned(),
        status,
        blocker: blocker.map(|s| s.to_owned()),
        refs,
    };

    let line = serde_json::to_string(&entry).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
    })?;

    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(f, "{}", line)?;
    Ok(entry)
}

/// Replay events into resolved ProgressItems. Latest event per
/// item_id wins for current state.
#[must_use]
pub fn resolved_items(events: &[ProgressEvent]) -> Vec<ProgressItem> {
    use std::collections::BTreeMap;
    let mut by_item: BTreeMap<String, Vec<&ProgressEvent>> = BTreeMap::new();
    for e in events {
        by_item.entry(e.item_id.clone()).or_default().push(e);
    }

    let mut out = Vec::new();
    for (item_id, mut events) in by_item {
        events.sort_by_key(|e| e.event_id);
        let first = events.first().unwrap();
        let latest = events.last().unwrap();
        out.push(ProgressItem {
            item_id,
            summary: latest.summary.clone(),
            status: latest.status,
            blocker: latest.blocker.clone(),
            refs: latest.refs.clone(),
            created_at: first.recorded_at.clone(),
            updated_at: latest.recorded_at.clone(),
        });
    }
    out
}

/// Filter resolved items by status.
#[must_use]
pub fn items_by_status(
    items: &[ProgressItem],
    status: ItemStatus,
) -> Vec<ProgressItem> {
    items.iter().filter(|i| i.status == status).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "forge-progress-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn status_slug_roundtrip() {
        for s in [
            ItemStatus::Pending,
            ItemStatus::InProgress,
            ItemStatus::Completed,
            ItemStatus::Cancelled,
            ItemStatus::Blocked,
        ] {
            assert_eq!(ItemStatus::parse(s.slug()), Some(s));
        }
    }

    #[test]
    fn empty_file_reads_empty() {
        let p = temp_path("empty");
        assert!(read_events(&p).unwrap().is_empty());
    }

    #[test]
    fn record_assigns_sequential_event_ids() {
        let p = temp_path("seq");
        let a = record_event(
            &p,
            "session-1",
            "item-A",
            "Build landing page",
            ItemStatus::InProgress,
            None,
            vec![],
            "2026-05-27T22:00:00Z",
        )
        .unwrap();
        let b = record_event(
            &p,
            "session-1",
            "item-A",
            "Build landing page",
            ItemStatus::Completed,
            None,
            vec!["commit:abc123".to_owned()],
            "2026-05-27T22:30:00Z",
        )
        .unwrap();
        assert_eq!(a.event_id, 1);
        assert_eq!(b.event_id, 2);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn resolved_items_latest_event_wins() {
        let events = vec![
            ProgressEvent {
                event_id: 1,
                recorded_at: "2026-05-27T22:00:00Z".to_owned(),
                session_id: "s1".to_owned(),
                item_id: "i1".to_owned(),
                summary: "Initial".to_owned(),
                status: ItemStatus::Pending,
                blocker: None,
                refs: vec![],
            },
            ProgressEvent {
                event_id: 2,
                recorded_at: "2026-05-27T22:15:00Z".to_owned(),
                session_id: "s1".to_owned(),
                item_id: "i1".to_owned(),
                summary: "Initial".to_owned(),
                status: ItemStatus::InProgress,
                blocker: None,
                refs: vec![],
            },
            ProgressEvent {
                event_id: 3,
                recorded_at: "2026-05-27T22:30:00Z".to_owned(),
                session_id: "s1".to_owned(),
                item_id: "i1".to_owned(),
                summary: "Done".to_owned(),
                status: ItemStatus::Completed,
                blocker: None,
                refs: vec!["commit:abc".to_owned()],
            },
        ];
        let items = resolved_items(&events);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, ItemStatus::Completed);
        assert_eq!(items[0].summary, "Done");
        assert_eq!(items[0].created_at, "2026-05-27T22:00:00Z");
        assert_eq!(items[0].updated_at, "2026-05-27T22:30:00Z");
    }

    #[test]
    fn items_by_status_filters() {
        let items = vec![
            ProgressItem {
                item_id: "a".to_owned(),
                summary: "".to_owned(),
                status: ItemStatus::Pending,
                blocker: None,
                refs: vec![],
                created_at: "".to_owned(),
                updated_at: "".to_owned(),
            },
            ProgressItem {
                item_id: "b".to_owned(),
                summary: "".to_owned(),
                status: ItemStatus::Completed,
                blocker: None,
                refs: vec![],
                created_at: "".to_owned(),
                updated_at: "".to_owned(),
            },
        ];
        let pending = items_by_status(&items, ItemStatus::Pending);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].item_id, "a");
    }

    #[test]
    fn blocked_item_preserves_blocker() {
        let p = temp_path("blocker");
        let entry = record_event(
            &p,
            "session-1",
            "item-A",
            "Waiting on DNS",
            ItemStatus::Blocked,
            Some("DNS propagation pending"),
            vec![],
            "2026-05-27T22:00:00Z",
        )
        .unwrap();
        assert_eq!(entry.blocker.as_deref(), Some("DNS propagation pending"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn round_trip_jsonl() {
        let p = temp_path("rt");
        record_event(
            &p,
            "s1",
            "i1",
            "Test",
            ItemStatus::InProgress,
            None,
            vec!["pr:42".to_owned()],
            "2026-05-27T23:00:00Z",
        )
        .unwrap();
        let events = read_events(&p).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].refs, vec!["pr:42".to_owned()]);
        let _ = std::fs::remove_file(&p);
    }
}
