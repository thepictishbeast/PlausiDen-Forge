//! `operator_corrections` — append-only registry of inline
//! operator overrides + identity-pinned corrections.
//!
//! Layer-5 substrate-reframe doctrine (#378): when an operator
//! overrides a substrate decision (e.g., substrate picked
//! `theme=light` but operator chose `theme=editorial`), the
//! correction is PINNED to the tenant's identity. Future builds
//! of the same tenant remember it; nearby tenants (same operator,
//! same band) see it as a hint.
//!
//! ## Why identity-pinned
//!
//! Without persistence, every build round-trips the operator
//! through the same default → override cycle. The substrate
//! "forgets" what the operator already taught it. With
//! identity-pinned corrections, the substrate learns: this
//! tenant prefers editorial themes, this operator prefers
//! Editorial decoration over Decorated, etc.
//!
//! ## File format
//!
//! JSONL, one `Correction` per line, append-only. Corrections
//! are NEVER edited in place — superseding corrections are
//! appended with the same `tenant_id` + `axis` and the latest
//! revision wins per the `lookup_*` functions.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// One operator-recorded correction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Correction {
    /// Sequential ID. Assigned by `record`.
    pub id: u32,
    /// RFC-3339 UTC timestamp.
    pub recorded_at: String,
    /// Tenant the correction applies to.
    pub tenant_id: String,
    /// Operator who recorded the correction. Used for nearby-
    /// correction surfacing.
    pub operator_id: String,
    /// Axis of the correction (theme / decoration / density /
    /// page_kind / hero_background / other).
    pub axis: String,
    /// The value the substrate originally chose.
    pub original_value: String,
    /// The value the operator overrode it to.
    pub corrected_value: String,
    /// Optional reason (helps future review).
    #[serde(default)]
    pub reason: Option<String>,
}

/// Read all corrections from a JSONL file.
/// Missing file → empty Vec.
pub fn read_all(path: &Path) -> std::io::Result<Vec<Correction>> {
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
        let entry: Correction = serde_json::from_str(trimmed).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?;
        out.push(entry);
    }
    Ok(out)
}

/// Record a new correction. Returns the assigned ID.
pub fn record(
    path: &Path,
    tenant_id: &str,
    operator_id: &str,
    axis: &str,
    original_value: &str,
    corrected_value: &str,
    reason: Option<&str>,
    now_rfc3339: &str,
) -> std::io::Result<Correction> {
    let existing = read_all(path)?;
    let next_id = existing.iter().map(|e| e.id).max().unwrap_or(0) + 1;

    let entry = Correction {
        id: next_id,
        recorded_at: now_rfc3339.to_owned(),
        tenant_id: tenant_id.to_owned(),
        operator_id: operator_id.to_owned(),
        axis: axis.to_owned(),
        original_value: original_value.to_owned(),
        corrected_value: corrected_value.to_owned(),
        reason: reason.map(|s| s.to_owned()),
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

/// Return the LATEST correction for a (tenant_id, axis) pair, or
/// None if no correction exists. "Latest" = highest id matching.
#[must_use]
pub fn latest_for(entries: &[Correction], tenant_id: &str, axis: &str) -> Option<Correction> {
    entries
        .iter()
        .filter(|e| e.tenant_id == tenant_id && e.axis == axis)
        .max_by_key(|e| e.id)
        .cloned()
}

/// Return all corrections by a given operator. Used to surface
/// "this operator typically overrides X" hints when building a
/// new tenant.
#[must_use]
pub fn by_operator(entries: &[Correction], operator_id: &str) -> Vec<Correction> {
    entries
        .iter()
        .filter(|e| e.operator_id == operator_id)
        .cloned()
        .collect()
}

/// Summarize the most common (axis, corrected_value) pairs for an
/// operator. Used to predict what the operator will likely
/// override on a new tenant.
#[must_use]
pub fn operator_preferences(
    entries: &[Correction],
    operator_id: &str,
) -> Vec<(String, String, u32)> {
    use std::collections::HashMap;
    let mut counts: HashMap<(String, String), u32> = HashMap::new();
    for e in entries.iter().filter(|e| e.operator_id == operator_id) {
        let key = (e.axis.clone(), e.corrected_value.clone());
        *counts.entry(key).or_insert(0) += 1;
    }
    let mut out: Vec<(String, String, u32)> = counts
        .into_iter()
        .map(|((axis, val), count)| (axis, val, count))
        .collect();
    out.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "forge-correction-test-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn empty_file_reads_empty() {
        let p = temp_path("empty");
        assert!(read_all(&p).unwrap().is_empty());
    }

    #[test]
    fn record_assigns_sequential_ids() {
        let p = temp_path("seq");
        let a = record(
            &p,
            "tenant-alpha",
            "paul",
            "theme",
            "light",
            "editorial",
            Some("brand prefers magazine register"),
            "2026-05-27T18:00:00Z",
        )
        .unwrap();
        let b = record(
            &p,
            "tenant-beta",
            "paul",
            "decoration",
            "decorated",
            "minimal",
            None,
            "2026-05-27T18:01:00Z",
        )
        .unwrap();
        assert_eq!(a.id, 1);
        assert_eq!(b.id, 2);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn latest_for_returns_most_recent() {
        let entries = vec![
            Correction {
                id: 1,
                recorded_at: "2026-05-27T18:00:00Z".to_owned(),
                tenant_id: "alpha".to_owned(),
                operator_id: "paul".to_owned(),
                axis: "theme".to_owned(),
                original_value: "light".to_owned(),
                corrected_value: "warm".to_owned(),
                reason: None,
            },
            Correction {
                id: 2,
                recorded_at: "2026-05-27T19:00:00Z".to_owned(),
                tenant_id: "alpha".to_owned(),
                operator_id: "paul".to_owned(),
                axis: "theme".to_owned(),
                original_value: "warm".to_owned(),
                corrected_value: "editorial".to_owned(),
                reason: None,
            },
        ];
        let latest = latest_for(&entries, "alpha", "theme").unwrap();
        assert_eq!(latest.corrected_value, "editorial");
    }

    #[test]
    fn latest_for_returns_none_for_unknown() {
        let entries: Vec<Correction> = Vec::new();
        assert!(latest_for(&entries, "alpha", "theme").is_none());
    }

    #[test]
    fn by_operator_filters() {
        let entries = vec![
            Correction {
                id: 1,
                recorded_at: "".to_owned(),
                tenant_id: "alpha".to_owned(),
                operator_id: "paul".to_owned(),
                axis: "theme".to_owned(),
                original_value: "light".to_owned(),
                corrected_value: "editorial".to_owned(),
                reason: None,
            },
            Correction {
                id: 2,
                recorded_at: "".to_owned(),
                tenant_id: "beta".to_owned(),
                operator_id: "alice".to_owned(),
                axis: "theme".to_owned(),
                original_value: "light".to_owned(),
                corrected_value: "warm".to_owned(),
                reason: None,
            },
        ];
        let paul = by_operator(&entries, "paul");
        assert_eq!(paul.len(), 1);
        assert_eq!(paul[0].tenant_id, "alpha");
    }

    #[test]
    fn operator_preferences_aggregates() {
        let entries = vec![
            Correction {
                id: 1,
                recorded_at: "".to_owned(),
                tenant_id: "alpha".to_owned(),
                operator_id: "paul".to_owned(),
                axis: "theme".to_owned(),
                original_value: "light".to_owned(),
                corrected_value: "editorial".to_owned(),
                reason: None,
            },
            Correction {
                id: 2,
                recorded_at: "".to_owned(),
                tenant_id: "beta".to_owned(),
                operator_id: "paul".to_owned(),
                axis: "theme".to_owned(),
                original_value: "light".to_owned(),
                corrected_value: "editorial".to_owned(),
                reason: None,
            },
            Correction {
                id: 3,
                recorded_at: "".to_owned(),
                tenant_id: "gamma".to_owned(),
                operator_id: "paul".to_owned(),
                axis: "decoration".to_owned(),
                original_value: "decorated".to_owned(),
                corrected_value: "minimal".to_owned(),
                reason: None,
            },
        ];
        let prefs = operator_preferences(&entries, "paul");
        // theme=editorial appears twice, decoration=minimal once.
        // theme=editorial should sort first.
        assert_eq!(prefs[0].0, "theme");
        assert_eq!(prefs[0].1, "editorial");
        assert_eq!(prefs[0].2, 2);
        assert_eq!(prefs[1].2, 1);
    }

    #[test]
    fn round_trip_jsonl() {
        let p = temp_path("rt");
        record(
            &p,
            "alpha",
            "paul",
            "density",
            "comfortable",
            "dense",
            Some("docs site"),
            "2026-05-27T20:00:00Z",
        )
        .unwrap();
        let entries = read_all(&p).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].axis, "density");
        assert_eq!(entries[0].corrected_value, "dense");
        let _ = std::fs::remove_file(&p);
    }
}
