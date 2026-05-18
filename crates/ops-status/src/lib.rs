//! `ops-status` — typed SLO/SLI definitions, error-budget
//! computation, and status-page state.
//!
//! Per `PLATFORM_ROADMAP.md` §8 + the
//! `super_society_tech_stack` doctrine: every operator-visible
//! reliability number is computed deterministically from typed
//! inputs — no free-text "we think we're at 99.9%" claims.
//!
//! ### Surface
//!
//! - [`Sli`]               — Service Level Indicator
//!                           ("availability" / "latency-p95" /
//!                           "request-success-rate")
//! - [`Slo`]                — Service Level Objective
//!                           (target ratio + rolling window)
//! - [`SliMeasurement`]    — one observation
//! - [`SliWindow`]         — a window of measurements
//! - [`ErrorBudget`]       — derived remaining budget +
//!                            burn rate
//! - [`StatusEntry`]       — one status-page row
//! - [`StatusLevel`]       — Ok / Degraded / PartialOutage /
//!                            MajorOutage
//! - [`IncidentSeverity`]  — Sev1 / Sev2 / Sev3 / Sev4 / Sev5
//!
//! Per `feedback_iso_standards`: tracks Google SRE Workbook
//! definitions + ISO 19770-1 ops conventions. Operators feed in
//! real metrics; this crate computes the derived state.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// A typed SLI identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SliId(String);

impl SliId {
    /// Construct from a kebab-case slug.
    pub fn parse(s: impl AsRef<str>) -> Result<Self, OpsError> {
        let s = s.as_ref();
        if s.is_empty() || s.len() > 64 {
            return Err(OpsError::InvalidSliId(format!("{s:?} length out of range")));
        }
        let mut chars = s.chars();
        let first = chars.next().unwrap();
        if !first.is_ascii_lowercase() {
            return Err(OpsError::InvalidSliId(format!(
                "{s:?} must start with [a-z]"
            )));
        }
        for c in chars {
            if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
                return Err(OpsError::InvalidSliId(format!(
                    "{s:?} char {c:?} not in [a-z0-9-]"
                )));
            }
        }
        Ok(Self(s.to_string()))
    }

    /// Raw view.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Service Level Indicator definition — what we're measuring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Sli {
    /// Stable identifier (e.g. `"availability"`, `"latency-p95"`).
    pub id: SliId,
    /// Human-readable description shown on the status page.
    pub description: String,
    /// Unit slug (`"ratio"`, `"ms"`, `"requests-per-second"`).
    pub unit: String,
}

/// Service Level Objective: an SLI + a target.
///
/// No `Eq` derive — target is f64 (NaN ≠ NaN). PartialEq is
/// sufficient for tests; operators comparing SLOs explicitly
/// must accept the float-equality caveats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Slo {
    /// Indicator this objective constrains.
    pub sli: SliId,
    /// Target value (for `"ratio"` units, in 0..=1; for latency,
    /// the ceiling in ms; etc.).
    pub target: f64,
    /// Window the target applies over, in seconds.
    pub window_secs: u64,
    /// Direction of compliance: `true` if higher values are
    /// better (availability ratio); `false` if lower is better
    /// (latency in ms).
    pub higher_is_better: bool,
}

impl Slo {
    /// Whether a single measurement value satisfies the target
    /// in isolation. (Real compliance is computed over a window;
    /// this is the per-sample predicate.)
    pub fn sample_compliant(&self, value: f64) -> bool {
        if self.higher_is_better {
            value >= self.target
        } else {
            value <= self.target
        }
    }
}

/// One SLI observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SliMeasurement {
    /// The indicator being measured.
    pub sli: SliId,
    /// Measured value at this point.
    pub value: f64,
    /// ISO-8601 timestamp of the measurement.
    pub at: time::OffsetDateTime,
}

/// A window of measurements for one SLI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SliWindow {
    /// The indicator covered.
    pub sli: SliId,
    /// Measurements in temporal order.
    pub samples: Vec<SliMeasurement>,
}

impl SliWindow {
    /// Compute the compliance ratio against `slo` — fraction of
    /// samples in 0..=1 that satisfy the SLO predicate.
    /// Returns 1.0 for an empty window (no failures → "perfect"
    /// is the conservative choice for status display).
    pub fn compliance_ratio(&self, slo: &Slo) -> f64 {
        if self.samples.is_empty() {
            return 1.0;
        }
        let compliant = self
            .samples
            .iter()
            .filter(|m| slo.sample_compliant(m.value))
            .count();
        compliant as f64 / self.samples.len() as f64
    }

    /// Number of samples + number that fail the SLO predicate.
    pub fn fail_counts(&self, slo: &Slo) -> (usize, usize) {
        let total = self.samples.len();
        let fails = self
            .samples
            .iter()
            .filter(|m| !slo.sample_compliant(m.value))
            .count();
        (total, fails)
    }
}

/// Derived error-budget state.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ErrorBudget {
    /// Allowed failure ratio = `1.0 - slo.target` for ratio-typed
    /// SLOs, or simply 0 for latency/throughput SLOs where every
    /// sample is treated as binary.
    pub allowed_fail_ratio: f64,
    /// Observed failure ratio over the window.
    pub observed_fail_ratio: f64,
    /// Remaining budget = allowed_fail_ratio - observed_fail_ratio.
    /// Negative = budget exhausted; the deployment is OUT of SLO.
    pub remaining: f64,
    /// Burn rate = observed_fail_ratio / allowed_fail_ratio.
    /// >1.0 means burning the budget faster than the SLO permits
    /// over the configured window.
    pub burn_rate: f64,
}

impl ErrorBudget {
    /// Compute from an SLO + a window of measurements.
    /// Convention: for "higher is better" SLOs (availability),
    /// allowed_fail_ratio = 1 - target. For "lower is better"
    /// SLOs (latency), allowed_fail_ratio = 1 - target_compliance,
    /// but since we don't know the target_compliance here we treat
    /// it as 0.05 (95% of latency measurements must be under the
    /// p95 target). Operators override this via [`Self::compute_with_allowed`].
    pub fn compute(slo: &Slo, window: &SliWindow) -> Self {
        let default_allowed = if slo.higher_is_better {
            (1.0 - slo.target).max(0.0)
        } else {
            // Conventional 5% tolerance for latency-style SLOs.
            0.05
        };
        Self::compute_with_allowed(slo, window, default_allowed)
    }

    /// Same as [`Self::compute`] but with an explicit
    /// allowed-failure ratio. Use when the operator's SLO
    /// document spells out a non-conventional value.
    pub fn compute_with_allowed(slo: &Slo, window: &SliWindow, allowed_fail_ratio: f64) -> Self {
        let compliance = window.compliance_ratio(slo);
        let observed_fail_ratio = (1.0 - compliance).max(0.0);
        let remaining = allowed_fail_ratio - observed_fail_ratio;
        let burn_rate = if allowed_fail_ratio == 0.0 {
            if observed_fail_ratio == 0.0 {
                0.0
            } else {
                f64::INFINITY
            }
        } else {
            observed_fail_ratio / allowed_fail_ratio
        };
        Self {
            allowed_fail_ratio,
            observed_fail_ratio,
            remaining,
            burn_rate,
        }
    }

    /// Whether the SLO is currently being met.
    pub fn is_meeting_slo(&self) -> bool {
        self.remaining >= 0.0
    }

    /// Whether the burn rate is alarming — > 2x indicates the
    /// deployment will exhaust its monthly budget in half the
    /// expected window. Triggers the alert tier.
    pub fn is_alarming(&self) -> bool {
        self.burn_rate > 2.0
    }
}

/// Status-page state level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StatusLevel {
    /// All systems operating normally.
    Ok,
    /// Some degradation but service is usable.
    Degraded,
    /// One or more components down; some users impacted.
    PartialOutage,
    /// Service unavailable to most users.
    MajorOutage,
}

impl StatusLevel {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Degraded => "degraded",
            Self::PartialOutage => "partial-outage",
            Self::MajorOutage => "major-outage",
        }
    }

    /// Derive a level from a remaining-budget value:
    ///   remaining ≥ 0          → Ok
    ///   remaining ≥ -0.25      → Degraded
    ///   remaining ≥ -0.50      → PartialOutage
    ///   remaining <  -0.50     → MajorOutage
    /// (negative values are over-budget — the worse it is, the
    /// more severe the page.)
    pub fn from_budget_remaining(remaining: f64) -> Self {
        if remaining >= 0.0 {
            Self::Ok
        } else if remaining >= -0.25 {
            Self::Degraded
        } else if remaining >= -0.50 {
            Self::PartialOutage
        } else {
            Self::MajorOutage
        }
    }
}

/// Closed enum of incident severity (Atlassian convention).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IncidentSeverity {
    /// Sev1 — total outage, all-hands.
    Sev1,
    /// Sev2 — major component down or significant degradation.
    Sev2,
    /// Sev3 — minor degradation; user-visible but workaroundable.
    Sev3,
    /// Sev4 — internal-only impact; no user-visible effect.
    Sev4,
    /// Sev5 — informational; e.g. planned-maintenance reminder.
    Sev5,
}

impl IncidentSeverity {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Sev1 => "sev1",
            Self::Sev2 => "sev2",
            Self::Sev3 => "sev3",
            Self::Sev4 => "sev4",
            Self::Sev5 => "sev5",
        }
    }
}

/// One row on the status page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct StatusEntry {
    /// Component / service being reported on.
    pub component: String,
    /// Current overall level.
    pub level: StatusLevel,
    /// Linked SLIs being computed.
    #[serde(default)]
    pub sli_ids: Vec<SliId>,
    /// Optional operator note (incident summary, planned-window
    /// notice, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Optional severity classification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<IncidentSeverity>,
    /// ISO-8601 last-updated timestamp.
    pub updated_at: time::OffsetDateTime,
}

/// Errors at the ops boundary.
#[derive(Debug, thiserror::Error)]
pub enum OpsError {
    /// SLI id failed shape validation.
    #[error("invalid sli id: {0}")]
    InvalidSliId(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn mk_slo_avail() -> Slo {
        Slo {
            sli: SliId::parse("availability").unwrap(),
            target: 0.999,
            window_secs: 30 * 86_400,
            higher_is_better: true,
        }
    }

    fn mk_slo_latency() -> Slo {
        Slo {
            sli: SliId::parse("latency-p95").unwrap(),
            target: 250.0,
            window_secs: 86_400,
            higher_is_better: false,
        }
    }

    fn sample(sli: &str, value: f64) -> SliMeasurement {
        SliMeasurement {
            sli: SliId::parse(sli).unwrap(),
            value,
            at: datetime!(2026-05-17 22:00:00 UTC),
        }
    }

    #[test]
    fn sli_id_validates_kebab_case() {
        assert!(SliId::parse("availability").is_ok());
        assert!(SliId::parse("latency-p95").is_ok());
        assert!(SliId::parse("").is_err());
        assert!(SliId::parse("Availability").is_err());
        assert!(SliId::parse("has space").is_err());
        assert!(SliId::parse(&"a".repeat(65)).is_err());
    }

    #[test]
    fn slo_sample_compliant_direction_matters() {
        let avail = mk_slo_avail();
        assert!(avail.sample_compliant(1.0));
        assert!(avail.sample_compliant(0.999));
        assert!(!avail.sample_compliant(0.998));

        let latency = mk_slo_latency();
        assert!(latency.sample_compliant(100.0));
        assert!(latency.sample_compliant(250.0));
        assert!(!latency.sample_compliant(300.0));
    }

    #[test]
    fn window_compliance_ratio_empty_is_perfect() {
        let w = SliWindow {
            sli: SliId::parse("availability").unwrap(),
            samples: vec![],
        };
        assert_eq!(w.compliance_ratio(&mk_slo_avail()), 1.0);
    }

    #[test]
    fn window_compliance_ratio_counts_compliant_samples() {
        let w = SliWindow {
            sli: SliId::parse("availability").unwrap(),
            samples: vec![
                sample("availability", 1.0),
                sample("availability", 1.0),
                sample("availability", 0.5), // fail
                sample("availability", 1.0),
            ],
        };
        assert_eq!(w.compliance_ratio(&mk_slo_avail()), 0.75);
        assert_eq!(w.fail_counts(&mk_slo_avail()), (4, 1));
    }

    #[test]
    fn error_budget_meeting_slo_when_compliant() {
        let w = SliWindow {
            sli: SliId::parse("availability").unwrap(),
            samples: vec![
                sample("availability", 1.0),
                sample("availability", 1.0),
                sample("availability", 1.0),
            ],
        };
        let b = ErrorBudget::compute(&mk_slo_avail(), &w);
        assert!(b.is_meeting_slo());
        assert!(!b.is_alarming());
        assert_eq!(b.observed_fail_ratio, 0.0);
        assert!(b.remaining > 0.0);
    }

    #[test]
    fn error_budget_burn_rate_above_2_is_alarming() {
        // 999 availability target → 0.001 allowed fail. Make 1% of
        // samples fail = 10x burn rate.
        let mut samples = Vec::new();
        for _ in 0..99 {
            samples.push(sample("availability", 1.0));
        }
        samples.push(sample("availability", 0.0));
        let w = SliWindow {
            sli: SliId::parse("availability").unwrap(),
            samples,
        };
        let b = ErrorBudget::compute(&mk_slo_avail(), &w);
        assert!(!b.is_meeting_slo());
        assert!(b.is_alarming());
        assert!(b.burn_rate > 2.0);
    }

    #[test]
    fn status_level_derives_from_budget() {
        assert_eq!(StatusLevel::from_budget_remaining(0.01), StatusLevel::Ok);
        assert_eq!(StatusLevel::from_budget_remaining(0.0), StatusLevel::Ok);
        assert_eq!(
            StatusLevel::from_budget_remaining(-0.1),
            StatusLevel::Degraded
        );
        assert_eq!(
            StatusLevel::from_budget_remaining(-0.3),
            StatusLevel::PartialOutage
        );
        assert_eq!(
            StatusLevel::from_budget_remaining(-0.7),
            StatusLevel::MajorOutage
        );
    }

    #[test]
    fn status_level_slugs_distinct() {
        let levels = [
            StatusLevel::Ok,
            StatusLevel::Degraded,
            StatusLevel::PartialOutage,
            StatusLevel::MajorOutage,
        ];
        let mut seen = std::collections::HashSet::new();
        for l in levels {
            assert!(seen.insert(l.slug()), "duplicate slug {l:?}");
        }
    }

    #[test]
    fn incident_severity_slugs_distinct() {
        let sevs = [
            IncidentSeverity::Sev1,
            IncidentSeverity::Sev2,
            IncidentSeverity::Sev3,
            IncidentSeverity::Sev4,
            IncidentSeverity::Sev5,
        ];
        let mut seen = std::collections::HashSet::new();
        for s in sevs {
            assert!(seen.insert(s.slug()));
        }
    }

    #[test]
    fn status_entry_serde_round_trips() {
        let e = StatusEntry {
            component: "ingest".into(),
            level: StatusLevel::Degraded,
            sli_ids: vec![SliId::parse("availability").unwrap()],
            note: Some("intermittent timeouts upstream".into()),
            severity: Some(IncidentSeverity::Sev2),
            updated_at: datetime!(2026-05-17 22:00:00 UTC),
        };
        let s = serde_json::to_string(&e).unwrap();
        let back: StatusEntry = serde_json::from_str(&s).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn status_entry_rejects_unknown_field() {
        let bad = r#"{"component":"x","level":"ok","updated-at":"2026-05-17T22:00:00Z","ahem":1}"#;
        let r: Result<StatusEntry, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    #[test]
    fn budget_with_explicit_allowed_overrides_default() {
        // 100% compliance → observed_fail = 0 → remaining = allowed.
        let w = SliWindow {
            sli: SliId::parse("availability").unwrap(),
            samples: vec![sample("availability", 1.0)],
        };
        let b = ErrorBudget::compute_with_allowed(&mk_slo_avail(), &w, 0.10);
        assert!((b.allowed_fail_ratio - 0.10).abs() < 1e-9);
        assert_eq!(b.observed_fail_ratio, 0.0);
        assert!((b.remaining - 0.10).abs() < 1e-9);
    }

    // T97: slug-vs-serde-wire regression guard.
    #[test]
    fn slug_matches_serde_wire_across_all_enums() {
        for v in [
            StatusLevel::Ok,
            StatusLevel::Degraded,
            StatusLevel::PartialOutage,
            StatusLevel::MajorOutage,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            IncidentSeverity::Sev1,
            IncidentSeverity::Sev2,
            IncidentSeverity::Sev3,
            IncidentSeverity::Sev4,
            IncidentSeverity::Sev5,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
    }
}
