//! `resumption` — cross-session hand-off briefs.
//!
//! Per task #391 (AI-DX accessibility #4 of 4). Pairs with
//! `session_progress` (#390): when a session ends mid-task, the
//! resumption brief loads the progress log, identifies the
//! "what was I doing" state, and surfaces it as a typed brief
//! the next session reads to pick up cleanly.
//!
//! ## Brief shape
//!
//! - **In-progress items**: highest priority on resume; the
//!   incoming session can immediately continue these.
//! - **Blocked items**: surfaces the blockers so the new session
//!   can check if any have resolved.
//! - **Recently completed**: the last 5 completed items, for
//!   continuity — "we just finished X, Y, Z".
//! - **Pending queue head**: the first 5 pending items, in
//!   insertion order, as the natural next-up after current
//!   in-progress finishes.
//!
//! No magic. The brief is just a structured query over the
//! event log. New sessions call it; old sessions can call it
//! to summarize before handoff.

use serde::Serialize;

use crate::session_progress::{ItemStatus, ProgressItem};

/// Cross-session resumption brief.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct ResumptionBrief {
    /// Items currently `in_progress`. New session should resume
    /// these first.
    pub in_progress: Vec<ProgressItem>,
    /// Items currently `blocked`. New session should check
    /// whether the blocker has resolved.
    pub blocked: Vec<ProgressItem>,
    /// Last N items completed, most recent first.
    pub recently_completed: Vec<ProgressItem>,
    /// First N pending items in insertion order — the queue
    /// head, what's next.
    pub pending_queue_head: Vec<ProgressItem>,
    /// Summary counts.
    pub counts: BriefCounts,
}

/// Counts summary inside a brief.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct BriefCounts {
    /// Total items observed.
    pub total: u32,
    /// In-progress count.
    pub in_progress: u32,
    /// Blocked count.
    pub blocked: u32,
    /// Completed count.
    pub completed: u32,
    /// Pending count.
    pub pending: u32,
    /// Cancelled count.
    pub cancelled: u32,
}

/// Default `recently_completed` cap.
pub const DEFAULT_RECENT_COMPLETED_LIMIT: usize = 5;

/// Default `pending_queue_head` cap.
pub const DEFAULT_PENDING_HEAD_LIMIT: usize = 5;

/// Build a resumption brief from resolved progress items.
/// `recent_completed_limit` and `pending_head_limit` cap the
/// per-category lists.
#[must_use]
pub fn build_brief(
    items: &[ProgressItem],
    recent_completed_limit: usize,
    pending_head_limit: usize,
) -> ResumptionBrief {
    let mut in_progress: Vec<ProgressItem> =
        items.iter().filter(|i| i.status == ItemStatus::InProgress).cloned().collect();
    let mut blocked: Vec<ProgressItem> =
        items.iter().filter(|i| i.status == ItemStatus::Blocked).cloned().collect();

    let mut completed: Vec<ProgressItem> =
        items.iter().filter(|i| i.status == ItemStatus::Completed).cloned().collect();
    // Most recent first.
    completed.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    completed.truncate(recent_completed_limit);

    let mut pending: Vec<ProgressItem> =
        items.iter().filter(|i| i.status == ItemStatus::Pending).cloned().collect();
    // Insertion order = created_at asc.
    pending.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    pending.truncate(pending_head_limit);

    // Stable sort in_progress and blocked by item_id for
    // deterministic output.
    in_progress.sort_by(|a, b| a.item_id.cmp(&b.item_id));
    blocked.sort_by(|a, b| a.item_id.cmp(&b.item_id));

    let counts = BriefCounts {
        total: items.len() as u32,
        in_progress: items.iter().filter(|i| i.status == ItemStatus::InProgress).count() as u32,
        blocked: items.iter().filter(|i| i.status == ItemStatus::Blocked).count() as u32,
        completed: items.iter().filter(|i| i.status == ItemStatus::Completed).count() as u32,
        pending: items.iter().filter(|i| i.status == ItemStatus::Pending).count() as u32,
        cancelled: items.iter().filter(|i| i.status == ItemStatus::Cancelled).count() as u32,
    };

    ResumptionBrief {
        in_progress,
        blocked,
        recently_completed: completed,
        pending_queue_head: pending,
        counts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, status: ItemStatus, created: &str, updated: &str) -> ProgressItem {
        ProgressItem {
            item_id: id.to_owned(),
            summary: format!("Item {id}"),
            status,
            blocker: None,
            refs: vec![],
            created_at: created.to_owned(),
            updated_at: updated.to_owned(),
        }
    }

    #[test]
    fn empty_items_empty_brief() {
        let brief = build_brief(&[], 5, 5);
        assert_eq!(brief.counts.total, 0);
        assert!(brief.in_progress.is_empty());
        assert!(brief.blocked.is_empty());
        assert!(brief.recently_completed.is_empty());
        assert!(brief.pending_queue_head.is_empty());
    }

    #[test]
    fn brief_filters_by_status() {
        let items = vec![
            item("a", ItemStatus::InProgress, "2026-05-01", "2026-05-02"),
            item("b", ItemStatus::Pending, "2026-05-01", "2026-05-01"),
            item("c", ItemStatus::Completed, "2026-05-01", "2026-05-03"),
            item("d", ItemStatus::Blocked, "2026-05-01", "2026-05-02"),
            item("e", ItemStatus::Cancelled, "2026-05-01", "2026-05-02"),
        ];
        let brief = build_brief(&items, 5, 5);
        assert_eq!(brief.in_progress.len(), 1);
        assert_eq!(brief.blocked.len(), 1);
        assert_eq!(brief.recently_completed.len(), 1);
        assert_eq!(brief.pending_queue_head.len(), 1);
        assert_eq!(brief.counts.cancelled, 1);
    }

    #[test]
    fn recently_completed_sorted_most_recent_first() {
        let items = vec![
            item("old", ItemStatus::Completed, "2026-05-01", "2026-05-01"),
            item("mid", ItemStatus::Completed, "2026-05-02", "2026-05-02"),
            item("new", ItemStatus::Completed, "2026-05-03", "2026-05-03"),
        ];
        let brief = build_brief(&items, 5, 5);
        assert_eq!(brief.recently_completed[0].item_id, "new");
        assert_eq!(brief.recently_completed[2].item_id, "old");
    }

    #[test]
    fn pending_head_sorted_oldest_first() {
        let items = vec![
            item("new", ItemStatus::Pending, "2026-05-03", "2026-05-03"),
            item("old", ItemStatus::Pending, "2026-05-01", "2026-05-01"),
            item("mid", ItemStatus::Pending, "2026-05-02", "2026-05-02"),
        ];
        let brief = build_brief(&items, 5, 5);
        // oldest first
        assert_eq!(brief.pending_queue_head[0].item_id, "old");
        assert_eq!(brief.pending_queue_head[2].item_id, "new");
    }

    #[test]
    fn recently_completed_caps_at_limit() {
        let items: Vec<ProgressItem> = (0..10)
            .map(|i| {
                item(
                    &format!("c{i}"),
                    ItemStatus::Completed,
                    "2026-05-01",
                    &format!("2026-05-{:02}", i + 1),
                )
            })
            .collect();
        let brief = build_brief(&items, 3, 5);
        assert_eq!(brief.recently_completed.len(), 3);
    }

    #[test]
    fn pending_head_caps_at_limit() {
        let items: Vec<ProgressItem> = (0..10)
            .map(|i| {
                item(
                    &format!("p{i}"),
                    ItemStatus::Pending,
                    &format!("2026-05-{:02}", i + 1),
                    "2026-05-01",
                )
            })
            .collect();
        let brief = build_brief(&items, 5, 3);
        assert_eq!(brief.pending_queue_head.len(), 3);
    }

    #[test]
    fn counts_aggregate_correctly() {
        let items = vec![
            item("a", ItemStatus::InProgress, "", ""),
            item("b", ItemStatus::InProgress, "", ""),
            item("c", ItemStatus::Pending, "", ""),
            item("d", ItemStatus::Completed, "", ""),
            item("e", ItemStatus::Blocked, "", ""),
        ];
        let brief = build_brief(&items, 5, 5);
        assert_eq!(brief.counts.total, 5);
        assert_eq!(brief.counts.in_progress, 2);
        assert_eq!(brief.counts.pending, 1);
        assert_eq!(brief.counts.completed, 1);
        assert_eq!(brief.counts.blocked, 1);
    }

    #[test]
    fn in_progress_deterministically_sorted_by_id() {
        let items = vec![
            item("zebra", ItemStatus::InProgress, "", ""),
            item("alpha", ItemStatus::InProgress, "", ""),
            item("mango", ItemStatus::InProgress, "", ""),
        ];
        let brief = build_brief(&items, 5, 5);
        assert_eq!(brief.in_progress[0].item_id, "alpha");
        assert_eq!(brief.in_progress[1].item_id, "mango");
        assert_eq!(brief.in_progress[2].item_id, "zebra");
    }
}
