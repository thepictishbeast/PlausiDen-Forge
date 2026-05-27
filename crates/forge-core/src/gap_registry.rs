//! `gap_registry` — append-only registry of substrate capability gaps.
//!
//! Per task #372 + the 2026-05-21 substrate reframe: when an
//! operator hits a substrate-capability gap (the substrate
//! doesn't express what they need), they must REGISTER the gap
//! rather than route around by hand-authoring outside the
//! substrate path. The registry is the canonical record of
//! "what the substrate doesn't yet do" so growth is observable
//! and prioritizable.
//!
//! File format: line-delimited JSON (one `GapEntry` per line).
//! Append-only — entries are never edited in place; status
//! transitions are appended as new revision entries that
//! reference the original by ID.
//!
//! ## Gap kinds
//!
//! The taxonomy mirrors the substrate's modification surfaces:
//!
//! - `primitive` — a missing CmsSection or CmsBlock variant
//! - `audit_phase` — a missing detection capability
//! - `theme` — a missing theme register (editorial / brutalist / etc.)
//! - `page_kind` — a missing PageKind variant
//! - `page_field` — a missing CmsPage field (e.g. `author`, `published_at`)
//! - `doctrine_rule` — a missing or unclear doctrine rule
//! - `tooling` — a missing CLI subcommand / MCP tool / skill

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Kind of substrate gap registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum GapKind {
    /// Missing CmsSection or CmsBlock variant.
    Primitive,
    /// Missing audit-phase capability.
    AuditPhase,
    /// Missing theme register.
    Theme,
    /// Missing PageKind variant.
    PageKind,
    /// Missing CmsPage field.
    PageField,
    /// Missing or unclear doctrine rule.
    DoctrineRule,
    /// Missing CLI / MCP / skill tooling.
    Tooling,
}

impl GapKind {
    /// Stable kebab-case slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Primitive => "primitive",
            Self::AuditPhase => "audit_phase",
            Self::Theme => "theme",
            Self::PageKind => "page_kind",
            Self::PageField => "page_field",
            Self::DoctrineRule => "doctrine_rule",
            Self::Tooling => "tooling",
        }
    }

    /// Parse from a snake_case slug.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "primitive" => Some(Self::Primitive),
            "audit_phase" => Some(Self::AuditPhase),
            "theme" => Some(Self::Theme),
            "page_kind" => Some(Self::PageKind),
            "page_field" => Some(Self::PageField),
            "doctrine_rule" => Some(Self::DoctrineRule),
            "tooling" => Some(Self::Tooling),
            _ => None,
        }
    }
}

/// Lifecycle status of a gap entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum GapStatus {
    /// Registered; not yet triaged.
    Open,
    /// Triaged + accepted as a real substrate gap.
    Accepted,
    /// Implementation underway.
    InProgress,
    /// Substrate change shipped.
    Shipped,
    /// Reviewed and rejected (out-of-band, duplicate, infeasible).
    Rejected,
}

impl GapStatus {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Accepted => "accepted",
            Self::InProgress => "in_progress",
            Self::Shipped => "shipped",
            Self::Rejected => "rejected",
        }
    }
}

/// One gap-registry entry. Append-only on the wire; status
/// transitions are appended as new entries pointing at the same
/// `id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GapEntry {
    /// Sequential identifier. Set by `append`.
    pub id: u32,
    /// RFC-3339 UTC timestamp.
    pub registered_at: String,
    /// Gap taxonomy entry.
    pub kind: GapKind,
    /// Tenant or URL where the gap was observed.
    pub observed_in: String,
    /// One-line summary.
    pub summary: String,
    /// Proposed resolution (substrate change, doctrine clarification, etc.).
    pub proposed_resolution: String,
    /// Lifecycle status.
    pub status: GapStatus,
    /// Optional related task IDs (e.g. ["#366", "#386"]).
    #[serde(default)]
    pub related_tasks: Vec<String>,
}

/// Read all gap entries from a JSONL registry file.
/// Missing file returns `Ok(empty)` — registration into a fresh
/// file is a normal flow.
pub fn read_all(path: &Path) -> std::io::Result<Vec<GapEntry>> {
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
        let entry: GapEntry = serde_json::from_str(trimmed).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?;
        out.push(entry);
    }
    Ok(out)
}

/// Append a new gap entry. Returns the assigned ID. Auto-assigns
/// `id = max(existing) + 1` (or 1 for a fresh registry).
pub fn append(
    path: &Path,
    kind: GapKind,
    observed_in: &str,
    summary: &str,
    proposed_resolution: &str,
    related_tasks: Vec<String>,
    now_rfc3339: &str,
) -> std::io::Result<GapEntry> {
    let existing = read_all(path)?;
    let next_id = existing.iter().map(|e| e.id).max().unwrap_or(0) + 1;

    let entry = GapEntry {
        id: next_id,
        registered_at: now_rfc3339.to_owned(),
        kind,
        observed_in: observed_in.to_owned(),
        summary: summary.to_owned(),
        proposed_resolution: proposed_resolution.to_owned(),
        status: GapStatus::Open,
        related_tasks,
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

/// Look up an entry by ID; returns the LATEST revision (highest
/// status priority + most recent timestamp).
#[must_use]
pub fn get(entries: &[GapEntry], id: u32) -> Option<&GapEntry> {
    entries.iter().rev().find(|e| e.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "forge-gap-test-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn fresh_registry_reads_empty() {
        let p = temp_path("empty");
        assert!(read_all(&p).unwrap().is_empty());
    }

    #[test]
    fn kind_slug_roundtrip() {
        for kind in [
            GapKind::Primitive,
            GapKind::AuditPhase,
            GapKind::Theme,
            GapKind::PageKind,
            GapKind::PageField,
            GapKind::DoctrineRule,
            GapKind::Tooling,
        ] {
            let slug = kind.slug();
            assert_eq!(GapKind::parse(slug), Some(kind));
        }
    }

    #[test]
    fn parse_rejects_unknown_kind() {
        assert!(GapKind::parse("not_a_kind").is_none());
    }

    #[test]
    fn append_assigns_sequential_ids() {
        let p = temp_path("seq");
        let a = append(
            &p,
            GapKind::Primitive,
            "tenant-alpha",
            "Need CmsSection::ComicStrip",
            "Add ComicStrip per Tier 2 of decorative audit",
            vec!["#359".to_owned()],
            "2026-05-27T17:00:00Z",
        )
        .unwrap();
        let b = append(
            &p,
            GapKind::Theme,
            "tenant-beta",
            "Need brutalist theme",
            "Ship brutalist register per theme growth task",
            vec!["#358".to_owned()],
            "2026-05-27T17:01:00Z",
        )
        .unwrap();
        assert_eq!(a.id, 1);
        assert_eq!(b.id, 2);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn read_all_round_trips() {
        let p = temp_path("rt");
        append(
            &p,
            GapKind::AuditPhase,
            "tenant-gamma",
            "Need image-dim required audit",
            "New phase image_dimension_required",
            vec![],
            "2026-05-27T17:02:00Z",
        )
        .unwrap();
        let entries = read_all(&p).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, GapKind::AuditPhase);
        assert_eq!(entries[0].status, GapStatus::Open);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn status_slugs_stable() {
        assert_eq!(GapStatus::Open.slug(), "open");
        assert_eq!(GapStatus::Accepted.slug(), "accepted");
        assert_eq!(GapStatus::InProgress.slug(), "in_progress");
        assert_eq!(GapStatus::Shipped.slug(), "shipped");
        assert_eq!(GapStatus::Rejected.slug(), "rejected");
    }

    #[test]
    fn get_returns_latest_revision() {
        let entries = vec![
            GapEntry {
                id: 1,
                registered_at: "2026-05-27T17:00:00Z".to_owned(),
                kind: GapKind::Primitive,
                observed_in: "tenant-alpha".to_owned(),
                summary: "Need X".to_owned(),
                proposed_resolution: "Add X".to_owned(),
                status: GapStatus::Open,
                related_tasks: vec![],
            },
            GapEntry {
                id: 1,
                registered_at: "2026-05-27T18:00:00Z".to_owned(),
                kind: GapKind::Primitive,
                observed_in: "tenant-alpha".to_owned(),
                summary: "Need X".to_owned(),
                proposed_resolution: "Add X".to_owned(),
                status: GapStatus::Accepted,
                related_tasks: vec![],
            },
        ];
        let latest = get(&entries, 1).unwrap();
        assert_eq!(latest.status, GapStatus::Accepted);
    }
}
