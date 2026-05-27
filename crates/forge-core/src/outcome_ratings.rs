//! `outcome_ratings` — append-only registry of tenant outcomes +
//! cohort-surveillance aggregations + operator-profile summaries.
//!
//! Layer-6 substrate-reframe doctrine (#379): after a tenant
//! ships, the substrate tracks OUTCOMES so future generation
//! can bias toward what worked. Without outcome data, the
//! substrate can't learn at the population level — every
//! tenant starts from the same defaults regardless of what
//! previously shipped tenants accomplished.
//!
//! ## What gets rated
//!
//! Outcomes are rated along orthogonal axes:
//! - `ship` — did the tenant actually ship (vs sat in dry-run)?
//! - `traffic` — visit volume (operator-reported or auto-fed)
//! - `engagement` — visitor depth (operator-reported)
//! - `retention` — did the operator return to make corrections?
//! - `revenue` — commerce / conversion (for commerce tenants)
//! - `aesthetic` — subjective quality call by the operator
//!
//! ## Cohort surveillance
//!
//! Aggregations group by axes the substrate cares about:
//! - by `operator_id` — what does this operator typically score
//! - by `page_kind` — do brief sites outperform marketing sites?
//! - by `theme` — does editorial outperform light in retention?
//! - by `fingerprint hash prefix` — do near-duplicate
//!   structures cluster on the same outcome?
//!
//! ## File format
//!
//! JSONL, one `Outcome` per line, append-only. The substrate
//! NEVER mutates a recorded outcome; revisions are new appends.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Axis along which a tenant outcome is being rated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OutcomeKind {
    /// Did the tenant ship (vs sat in dry-run)?
    Ship,
    /// Visit volume.
    Traffic,
    /// Visitor depth.
    Engagement,
    /// Operator returning for more corrections.
    Retention,
    /// Conversion / revenue.
    Revenue,
    /// Subjective quality.
    Aesthetic,
}

impl OutcomeKind {
    /// Stable slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Ship => "ship",
            Self::Traffic => "traffic",
            Self::Engagement => "engagement",
            Self::Retention => "retention",
            Self::Revenue => "revenue",
            Self::Aesthetic => "aesthetic",
        }
    }

    /// Parse from a snake_case slug.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ship" => Some(Self::Ship),
            "traffic" => Some(Self::Traffic),
            "engagement" => Some(Self::Engagement),
            "retention" => Some(Self::Retention),
            "revenue" => Some(Self::Revenue),
            "aesthetic" => Some(Self::Aesthetic),
            _ => None,
        }
    }
}

/// One outcome record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub struct Outcome {
    /// Sequential ID.
    pub id: u32,
    /// RFC-3339 UTC.
    pub rated_at: String,
    /// Tenant rated.
    pub tenant_id: String,
    /// Who recorded the rating.
    pub rater_id: String,
    /// What axis was rated.
    pub kind: OutcomeKind,
    /// Score on a 0..=100 scale. Specific meaning depends on `kind`.
    pub score: u32,
    /// Optional notes from the rater.
    #[serde(default)]
    pub notes: Option<String>,
}

/// Read every outcome from the JSONL registry.
pub fn read_all(path: &Path) -> std::io::Result<Vec<Outcome>> {
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
        let entry: Outcome = serde_json::from_str(trimmed).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?;
        out.push(entry);
    }
    Ok(out)
}

/// Append a new outcome rating.
pub fn record(
    path: &Path,
    tenant_id: &str,
    rater_id: &str,
    kind: OutcomeKind,
    score: u32,
    notes: Option<&str>,
    now_rfc3339: &str,
) -> std::io::Result<Outcome> {
    let existing = read_all(path)?;
    let next_id = existing.iter().map(|e| e.id).max().unwrap_or(0) + 1;
    let entry = Outcome {
        id: next_id,
        rated_at: now_rfc3339.to_owned(),
        tenant_id: tenant_id.to_owned(),
        rater_id: rater_id.to_owned(),
        kind,
        score: score.min(100),
        notes: notes.map(|s| s.to_owned()),
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

/// One row in a cohort-summary report.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct CohortRow {
    /// Cohort key (e.g. tenant_id, rater_id, or composite).
    pub key: String,
    /// Number of outcomes in the cohort.
    pub count: u32,
    /// Average score across the cohort (rounded to nearest integer).
    pub avg_score: u32,
    /// Outcome kind being aggregated.
    pub kind: OutcomeKind,
}

/// Aggregate outcomes by tenant_id for a specific kind. Useful
/// for "how is tenant X performing across rated dimensions".
#[must_use]
pub fn cohort_by_tenant(entries: &[Outcome], kind: OutcomeKind) -> Vec<CohortRow> {
    use std::collections::BTreeMap;
    let mut by_tenant: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    for e in entries.iter().filter(|e| e.kind == kind) {
        let (sum, count) = by_tenant.entry(e.tenant_id.clone()).or_insert((0, 0));
        *sum += e.score;
        *count += 1;
    }
    let mut rows: Vec<CohortRow> = by_tenant
        .into_iter()
        .map(|(key, (sum, count))| CohortRow {
            key,
            count,
            avg_score: if count == 0 { 0 } else { sum / count },
            kind,
        })
        .collect();
    rows.sort_by(|a, b| b.avg_score.cmp(&a.avg_score).then_with(|| a.key.cmp(&b.key)));
    rows
}

/// Aggregate outcomes by rater_id for a specific kind.
#[must_use]
pub fn cohort_by_rater(entries: &[Outcome], kind: OutcomeKind) -> Vec<CohortRow> {
    use std::collections::BTreeMap;
    let mut by_rater: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    for e in entries.iter().filter(|e| e.kind == kind) {
        let (sum, count) = by_rater.entry(e.rater_id.clone()).or_insert((0, 0));
        *sum += e.score;
        *count += 1;
    }
    let mut rows: Vec<CohortRow> = by_rater
        .into_iter()
        .map(|(key, (sum, count))| CohortRow {
            key,
            count,
            avg_score: if count == 0 { 0 } else { sum / count },
            kind,
        })
        .collect();
    rows.sort_by(|a, b| b.avg_score.cmp(&a.avg_score).then_with(|| a.key.cmp(&b.key)));
    rows
}

/// Operator profile: per-kind averages across every outcome
/// the operator has rated.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct OperatorProfile {
    /// Operator ID.
    pub operator_id: String,
    /// Total outcomes the operator has rated.
    pub total_ratings: u32,
    /// Per-kind average score (0 if the operator has zero
    /// outcomes of that kind).
    pub per_kind_avg: Vec<(OutcomeKind, u32)>,
}

/// Build a profile for one operator across every outcome kind.
#[must_use]
pub fn operator_profile(entries: &[Outcome], operator_id: &str) -> OperatorProfile {
    use std::collections::HashMap;
    let mut by_kind: HashMap<OutcomeKind, (u32, u32)> = HashMap::new();
    let mut total = 0;
    for e in entries.iter().filter(|e| e.rater_id == operator_id) {
        total += 1;
        let (sum, count) = by_kind.entry(e.kind).or_insert((0, 0));
        *sum += e.score;
        *count += 1;
    }

    let mut per_kind_avg: Vec<(OutcomeKind, u32)> = by_kind
        .into_iter()
        .map(|(kind, (sum, count))| {
            (kind, if count == 0 { 0 } else { sum / count })
        })
        .collect();
    per_kind_avg.sort_by_key(|(k, _)| k.slug());

    OperatorProfile {
        operator_id: operator_id.to_owned(),
        total_ratings: total,
        per_kind_avg,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "forge-outcome-test-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn kind_slug_roundtrip() {
        for k in [
            OutcomeKind::Ship,
            OutcomeKind::Traffic,
            OutcomeKind::Engagement,
            OutcomeKind::Retention,
            OutcomeKind::Revenue,
            OutcomeKind::Aesthetic,
        ] {
            assert_eq!(OutcomeKind::parse(k.slug()), Some(k));
        }
    }

    #[test]
    fn empty_registry_reads_empty() {
        let p = temp_path("empty");
        assert!(read_all(&p).unwrap().is_empty());
    }

    #[test]
    fn record_assigns_sequential_ids_and_clamps_score() {
        let p = temp_path("seq");
        let a = record(
            &p,
            "alpha",
            "paul",
            OutcomeKind::Ship,
            85,
            None,
            "2026-05-27T20:00:00Z",
        )
        .unwrap();
        // score > 100 clamps to 100.
        let b = record(
            &p,
            "alpha",
            "paul",
            OutcomeKind::Aesthetic,
            150,
            Some("loves it"),
            "2026-05-27T20:01:00Z",
        )
        .unwrap();
        assert_eq!(a.id, 1);
        assert_eq!(b.id, 2);
        assert_eq!(b.score, 100);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn cohort_by_tenant_aggregates() {
        let entries = vec![
            Outcome {
                id: 1, rated_at: "".to_owned(), tenant_id: "alpha".to_owned(),
                rater_id: "paul".to_owned(), kind: OutcomeKind::Aesthetic,
                score: 80, notes: None,
            },
            Outcome {
                id: 2, rated_at: "".to_owned(), tenant_id: "alpha".to_owned(),
                rater_id: "paul".to_owned(), kind: OutcomeKind::Aesthetic,
                score: 90, notes: None,
            },
            Outcome {
                id: 3, rated_at: "".to_owned(), tenant_id: "beta".to_owned(),
                rater_id: "paul".to_owned(), kind: OutcomeKind::Aesthetic,
                score: 60, notes: None,
            },
        ];
        let rows = cohort_by_tenant(&entries, OutcomeKind::Aesthetic);
        // alpha avg = 85, beta avg = 60 — sorted desc
        assert_eq!(rows[0].key, "alpha");
        assert_eq!(rows[0].avg_score, 85);
        assert_eq!(rows[1].key, "beta");
        assert_eq!(rows[1].avg_score, 60);
    }

    #[test]
    fn cohort_by_rater_filters_by_kind() {
        let entries = vec![
            Outcome {
                id: 1, rated_at: "".to_owned(), tenant_id: "alpha".to_owned(),
                rater_id: "paul".to_owned(), kind: OutcomeKind::Ship,
                score: 100, notes: None,
            },
            Outcome {
                id: 2, rated_at: "".to_owned(), tenant_id: "beta".to_owned(),
                rater_id: "paul".to_owned(), kind: OutcomeKind::Traffic,
                score: 50, notes: None,
            },
        ];
        let ship_rows = cohort_by_rater(&entries, OutcomeKind::Ship);
        assert_eq!(ship_rows.len(), 1);
        assert_eq!(ship_rows[0].avg_score, 100);
    }

    #[test]
    fn operator_profile_aggregates_per_kind() {
        let entries = vec![
            Outcome {
                id: 1, rated_at: "".to_owned(), tenant_id: "alpha".to_owned(),
                rater_id: "paul".to_owned(), kind: OutcomeKind::Ship,
                score: 100, notes: None,
            },
            Outcome {
                id: 2, rated_at: "".to_owned(), tenant_id: "beta".to_owned(),
                rater_id: "paul".to_owned(), kind: OutcomeKind::Ship,
                score: 80, notes: None,
            },
            Outcome {
                id: 3, rated_at: "".to_owned(), tenant_id: "gamma".to_owned(),
                rater_id: "alice".to_owned(), kind: OutcomeKind::Ship,
                score: 50, notes: None,
            },
        ];
        let profile = operator_profile(&entries, "paul");
        assert_eq!(profile.total_ratings, 2);
        let ship_avg = profile
            .per_kind_avg
            .iter()
            .find(|(k, _)| *k == OutcomeKind::Ship)
            .map(|(_, v)| *v)
            .unwrap();
        assert_eq!(ship_avg, 90);
    }

    #[test]
    fn jsonl_round_trip() {
        let p = temp_path("rt");
        record(
            &p,
            "alpha",
            "paul",
            OutcomeKind::Retention,
            70,
            Some("came back twice"),
            "2026-05-27T21:00:00Z",
        )
        .unwrap();
        let entries = read_all(&p).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, OutcomeKind::Retention);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn empty_operator_profile_returns_zero() {
        let entries: Vec<Outcome> = Vec::new();
        let profile = operator_profile(&entries, "nobody");
        assert_eq!(profile.total_ratings, 0);
        assert!(profile.per_kind_avg.is_empty());
    }
}
