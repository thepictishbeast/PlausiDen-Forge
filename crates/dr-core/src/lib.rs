//! `dr-core` — typed disaster-recovery surface.
//!
//! Per `PLATFORM_ROADMAP.md` §8 + `super_society_tech_stack`:
//! every shipping platform deployment declares typed
//! [`RtoTarget`] + [`RpoTarget`] per [`DrTier`], schedules
//! [`Drill`]s to validate them, and records [`DrillResult`]s the
//! ops-status surface (task #68) renders. Without typed targets
//! a "we tested DR" claim is unverifiable; with them, every drill
//! produces a comparable artifact.
//!
//! ### Surface
//!
//! - [`DrTier`]              — closed Tier1 / Tier2 / Tier3 enum
//! - [`RtoTarget`]           — Recovery Time Objective (max
//!                              acceptable downtime)
//! - [`RpoTarget`]           — Recovery Point Objective (max
//!                              acceptable data loss)
//! - [`TierBudget`]          — RTO + RPO + drill cadence per tier
//! - [`Drill`]               — scheduled DR drill definition
//! - [`DrillResult`]         — measured outcome
//! - [`RegionId`]            — region identifier
//! - [`RegionFailoverState`] — current active vs standby state
//!
//! Per `feedback_iso_standards`: tracks ISO 22301 BCMS
//! (business-continuity management system) conventions.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Disaster-recovery tier. Lower number = higher criticality.
/// Closed three-tier enum: every workload picks exactly one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DrTier {
    /// Tier 1 — mission-critical. RTO minutes, RPO seconds.
    /// Examples: payment processing, auth, live publish path.
    #[serde(rename = "tier-1")]
    Tier1,
    /// Tier 2 — business-important. RTO hours, RPO minutes.
    /// Examples: admin UI, content draft storage, deploy queue.
    #[serde(rename = "tier-2")]
    Tier2,
    /// Tier 3 — internal / asynchronous. RTO days, RPO hours.
    /// Examples: analytics aggregates, internal dashboards,
    /// historical reports.
    #[serde(rename = "tier-3")]
    Tier3,
}

impl DrTier {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Tier1 => "tier-1",
            Self::Tier2 => "tier-2",
            Self::Tier3 => "tier-3",
        }
    }

    /// All tiers in declaration order (T1 → T3 = most → least critical).
    pub const ALL: &'static [DrTier] = &[Self::Tier1, Self::Tier2, Self::Tier3];

    /// Platform default RTO target per tier.
    pub fn default_rto(&self) -> RtoTarget {
        match self {
            Self::Tier1 => RtoTarget { max_secs: 15 * 60 }, // 15 min
            Self::Tier2 => RtoTarget {
                max_secs: 4 * 60 * 60,
            }, // 4 h
            Self::Tier3 => RtoTarget {
                max_secs: 24 * 60 * 60,
            }, // 24 h
        }
    }

    /// Platform default RPO target per tier.
    pub fn default_rpo(&self) -> RpoTarget {
        match self {
            Self::Tier1 => RpoTarget { max_secs: 30 },      // 30 s
            Self::Tier2 => RpoTarget { max_secs: 15 * 60 }, // 15 min
            Self::Tier3 => RpoTarget {
                max_secs: 4 * 60 * 60,
            }, // 4 h
        }
    }

    /// Platform default drill cadence per tier.
    pub fn default_drill_cadence(&self) -> DrillCadence {
        match self {
            Self::Tier1 => DrillCadence::Monthly,
            Self::Tier2 => DrillCadence::Quarterly,
            Self::Tier3 => DrillCadence::SemiAnnual,
        }
    }
}

/// Recovery Time Objective. Maximum acceptable downtime in
/// seconds before the workload's tier SLA is breached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RtoTarget {
    /// Maximum downtime in seconds.
    pub max_secs: u64,
}

impl RtoTarget {
    /// Whether `observed_downtime_secs` is within budget.
    pub fn is_within_budget(&self, observed_downtime_secs: u64) -> bool {
        observed_downtime_secs <= self.max_secs
    }
}

/// Recovery Point Objective. Maximum acceptable data loss
/// (replication / backup lag) in seconds before the workload's
/// tier SLA is breached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RpoTarget {
    /// Maximum data-loss window in seconds.
    pub max_secs: u64,
}

impl RpoTarget {
    /// Whether `observed_lag_secs` is within budget.
    pub fn is_within_budget(&self, observed_lag_secs: u64) -> bool {
        observed_lag_secs <= self.max_secs
    }
}

/// Drill cadence — how often this tier's DR plan must be
/// rehearsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DrillCadence {
    /// Every 30 days.
    Monthly,
    /// Every 90 days.
    Quarterly,
    /// Every 180 days.
    SemiAnnual,
    /// Every 365 days.
    Annual,
}

impl DrillCadence {
    /// Cadence period in seconds.
    pub fn period_secs(&self) -> u64 {
        match self {
            Self::Monthly => 30 * 86_400,
            Self::Quarterly => 90 * 86_400,
            Self::SemiAnnual => 180 * 86_400,
            Self::Annual => 365 * 86_400,
        }
    }

    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Monthly => "monthly",
            Self::Quarterly => "quarterly",
            Self::SemiAnnual => "semi-annual",
            Self::Annual => "annual",
        }
    }
}

/// Per-tier policy bundle: RTO + RPO + cadence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TierBudget {
    /// Which tier this applies to.
    pub tier: DrTier,
    /// Recovery Time Objective.
    pub rto: RtoTarget,
    /// Recovery Point Objective.
    pub rpo: RpoTarget,
    /// Drill cadence — how often to rehearse.
    pub drill_cadence: DrillCadence,
}

impl TierBudget {
    /// Platform default budget for a tier.
    pub fn defaults_for(tier: DrTier) -> Self {
        Self {
            tier,
            rto: tier.default_rto(),
            rpo: tier.default_rpo(),
            drill_cadence: tier.default_drill_cadence(),
        }
    }
}

/// Region identifier — operator-chosen kebab-case slug (e.g.
/// `"us-east-1"`, `"eu-west-2"`, `"ap-northeast-1"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RegionId(String);

impl RegionId {
    /// Construct from a kebab-case slug.
    pub fn parse(s: impl AsRef<str>) -> Result<Self, DrError> {
        let s = s.as_ref();
        if s.is_empty() || s.len() > 32 {
            return Err(DrError::InvalidRegionId(format!("{s:?} length")));
        }
        let mut chars = s.chars();
        let first = chars.next().unwrap();
        if !first.is_ascii_lowercase() {
            return Err(DrError::InvalidRegionId(format!(
                "{s:?} must start with [a-z]"
            )));
        }
        for c in chars {
            if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
                return Err(DrError::InvalidRegionId(format!(
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

/// A scheduled DR drill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Drill {
    /// Drill identifier (operator-chosen kebab-case slug).
    pub id: String,
    /// Tier this drill validates.
    pub tier: DrTier,
    /// Component or workload under test.
    pub component: String,
    /// Region this drill targets.
    pub region: RegionId,
    /// Human-readable scenario (e.g. `"full region failure"`).
    pub scenario: String,
    /// When the drill was last run, if ever.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run_at: Option<time::OffsetDateTime>,
}

impl Drill {
    /// Whether the drill is currently overdue under `budget`'s
    /// cadence. Drills never run → always overdue.
    pub fn is_overdue(&self, budget: &TierBudget, now: time::OffsetDateTime) -> bool {
        let Some(last) = self.last_run_at else {
            return true;
        };
        let elapsed = (now - last).whole_seconds().max(0) as u64;
        elapsed > budget.drill_cadence.period_secs()
    }

    /// Seconds until the drill goes overdue, or 0 if already
    /// overdue.
    pub fn secs_until_due(&self, budget: &TierBudget, now: time::OffsetDateTime) -> u64 {
        let Some(last) = self.last_run_at else {
            return 0;
        };
        let elapsed = (now - last).whole_seconds().max(0) as u64;
        budget.drill_cadence.period_secs().saturating_sub(elapsed)
    }
}

/// One executed drill's outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DrillResult {
    /// Which drill this records.
    pub drill_id: String,
    /// When the drill executed.
    pub ran_at: time::OffsetDateTime,
    /// Measured recovery time in seconds.
    pub observed_rto_secs: u64,
    /// Measured replication / backup lag at failover in seconds.
    pub observed_rpo_secs: u64,
    /// Whether the drill passed under the tier budget.
    pub passed: bool,
    /// Operator notes (incidents found, remediation queued).
    #[serde(default)]
    pub notes: String,
}

impl DrillResult {
    /// Check pass/fail against `budget`. Updates `self.passed`
    /// in place so callers don't have to track it separately.
    pub fn check_against(&mut self, budget: &TierBudget) {
        self.passed = budget.rto.is_within_budget(self.observed_rto_secs)
            && budget.rpo.is_within_budget(self.observed_rpo_secs);
    }
}

/// Failover state for a multi-region workload. The runner manages
/// transitions; this type captures the current observed state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RegionFailoverState {
    /// Workload identifier.
    pub workload: String,
    /// Region currently serving traffic.
    pub active: RegionId,
    /// Standby regions ready to take over (in failover priority).
    #[serde(default)]
    pub standby: Vec<RegionId>,
    /// When the active region last took over (None = never failed
    /// over from initial state).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_failover_at: Option<time::OffsetDateTime>,
}

impl RegionFailoverState {
    /// Promote the first standby to active. Pushes the previous
    /// active to the end of the standby list.
    /// Returns the new active region, or an error if no standby
    /// is available.
    pub fn fail_over(&mut self, now: time::OffsetDateTime) -> Result<&RegionId, DrError> {
        if self.standby.is_empty() {
            return Err(DrError::NoStandbyAvailable(self.workload.clone()));
        }
        let new_active = self.standby.remove(0);
        let old_active = std::mem::replace(&mut self.active, new_active);
        self.standby.push(old_active);
        self.last_failover_at = Some(now);
        Ok(&self.active)
    }
}

/// Typed errors at the DR boundary.
#[derive(Debug, thiserror::Error)]
pub enum DrError {
    /// Region id failed shape validation.
    #[error("invalid region id: {0}")]
    InvalidRegionId(String),
    /// Tried to fail over with no standby region available.
    #[error("no standby region available for workload {0:?}")]
    NoStandbyAvailable(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn dr_tier_defaults_are_ordered() {
        // T1 < T2 < T3 across all dimensions (lower number =
        // stricter target = smaller max_secs).
        assert!(DrTier::Tier1.default_rto().max_secs < DrTier::Tier2.default_rto().max_secs);
        assert!(DrTier::Tier2.default_rto().max_secs < DrTier::Tier3.default_rto().max_secs);
        assert!(DrTier::Tier1.default_rpo().max_secs < DrTier::Tier2.default_rpo().max_secs);
        assert!(DrTier::Tier2.default_rpo().max_secs < DrTier::Tier3.default_rpo().max_secs);
    }

    #[test]
    fn dr_tier_slugs_distinct() {
        let mut seen = std::collections::HashSet::new();
        for t in DrTier::ALL {
            assert!(seen.insert(t.slug()));
        }
    }

    #[test]
    fn drill_cadence_default_pairs_with_tier() {
        assert_eq!(DrTier::Tier1.default_drill_cadence(), DrillCadence::Monthly);
        assert_eq!(
            DrTier::Tier2.default_drill_cadence(),
            DrillCadence::Quarterly
        );
        assert_eq!(
            DrTier::Tier3.default_drill_cadence(),
            DrillCadence::SemiAnnual
        );
    }

    #[test]
    fn rto_rpo_budget_predicates() {
        let rto = RtoTarget { max_secs: 300 };
        assert!(rto.is_within_budget(200));
        assert!(rto.is_within_budget(300));
        assert!(!rto.is_within_budget(301));

        let rpo = RpoTarget { max_secs: 30 };
        assert!(rpo.is_within_budget(0));
        assert!(rpo.is_within_budget(30));
        assert!(!rpo.is_within_budget(31));
    }

    #[test]
    fn region_id_validates_kebab_case() {
        assert!(RegionId::parse("us-east-1").is_ok());
        assert!(RegionId::parse("eu-west-2").is_ok());
        assert!(RegionId::parse("").is_err());
        assert!(RegionId::parse("US-EAST-1").is_err());
        assert!(RegionId::parse("has space").is_err());
        assert!(RegionId::parse(&"a".repeat(33)).is_err());
    }

    #[test]
    fn drill_overdue_when_never_run() {
        let drill = Drill {
            id: "tier1-payments-failover".into(),
            tier: DrTier::Tier1,
            component: "payments".into(),
            region: RegionId::parse("us-east-1").unwrap(),
            scenario: "primary region down".into(),
            last_run_at: None,
        };
        let budget = TierBudget::defaults_for(DrTier::Tier1);
        let now = datetime!(2026-05-18 12:00:00 UTC);
        assert!(drill.is_overdue(&budget, now));
        assert_eq!(drill.secs_until_due(&budget, now), 0);
    }

    #[test]
    fn drill_not_overdue_within_cadence_window() {
        let drill = Drill {
            id: "x".into(),
            tier: DrTier::Tier1,
            component: "x".into(),
            region: RegionId::parse("us-east-1").unwrap(),
            scenario: "x".into(),
            last_run_at: Some(datetime!(2026-05-01 12:00:00 UTC)),
        };
        let budget = TierBudget::defaults_for(DrTier::Tier1);
        let now = datetime!(2026-05-15 12:00:00 UTC); // 14 days later
        assert!(!drill.is_overdue(&budget, now));
        assert!(drill.secs_until_due(&budget, now) > 0);
    }

    #[test]
    fn drill_overdue_past_cadence_window() {
        let drill = Drill {
            id: "x".into(),
            tier: DrTier::Tier1,
            component: "x".into(),
            region: RegionId::parse("us-east-1").unwrap(),
            scenario: "x".into(),
            // Tier1 cadence is monthly (30 days). 60 days ago →
            // overdue.
            last_run_at: Some(datetime!(2026-03-18 12:00:00 UTC)),
        };
        let budget = TierBudget::defaults_for(DrTier::Tier1);
        let now = datetime!(2026-05-18 12:00:00 UTC);
        assert!(drill.is_overdue(&budget, now));
    }

    #[test]
    fn drill_result_check_against_budget() {
        let budget = TierBudget::defaults_for(DrTier::Tier2);
        // Tier2 default: RTO 4h = 14_400s, RPO 15min = 900s.
        let mut pass = DrillResult {
            drill_id: "x".into(),
            ran_at: datetime!(2026-05-18 12:00:00 UTC),
            observed_rto_secs: 10 * 60, // 10 min — well under
            observed_rpo_secs: 5 * 60,  // 5 min — well under
            passed: false,
            notes: "".into(),
        };
        pass.check_against(&budget);
        assert!(pass.passed);

        let mut fail_rto = DrillResult {
            drill_id: "x".into(),
            ran_at: datetime!(2026-05-18 12:00:00 UTC),
            observed_rto_secs: 5 * 60 * 60, // 5h — over
            observed_rpo_secs: 5 * 60,
            passed: true, // wrongly preset
            notes: "".into(),
        };
        fail_rto.check_against(&budget);
        assert!(!fail_rto.passed);

        let mut fail_rpo = DrillResult {
            drill_id: "x".into(),
            ran_at: datetime!(2026-05-18 12:00:00 UTC),
            observed_rto_secs: 60,
            observed_rpo_secs: 20 * 60, // 20 min — over
            passed: true,
            notes: "".into(),
        };
        fail_rpo.check_against(&budget);
        assert!(!fail_rpo.passed);
    }

    #[test]
    fn region_failover_promotes_first_standby() {
        let mut state = RegionFailoverState {
            workload: "api".into(),
            active: RegionId::parse("us-east-1").unwrap(),
            standby: vec![
                RegionId::parse("us-west-2").unwrap(),
                RegionId::parse("eu-west-1").unwrap(),
            ],
            last_failover_at: None,
        };
        let now = datetime!(2026-05-18 12:00:00 UTC);
        let new_active = state.fail_over(now).unwrap().clone();
        assert_eq!(new_active.as_str(), "us-west-2");
        assert_eq!(state.active.as_str(), "us-west-2");
        assert_eq!(state.standby.len(), 2);
        // Old active is now at the END of the standby list.
        assert_eq!(state.standby[1].as_str(), "us-east-1");
        assert_eq!(state.last_failover_at, Some(now));
    }

    #[test]
    fn region_failover_no_standby_errors() {
        let mut state = RegionFailoverState {
            workload: "lonely".into(),
            active: RegionId::parse("us-east-1").unwrap(),
            standby: vec![],
            last_failover_at: None,
        };
        let err = state
            .fail_over(datetime!(2026-05-18 12:00:00 UTC))
            .unwrap_err();
        assert!(matches!(err, DrError::NoStandbyAvailable(_)));
    }

    #[test]
    fn tier_budget_defaults_match_tier_helpers() {
        for t in DrTier::ALL {
            let b = TierBudget::defaults_for(*t);
            assert_eq!(b.tier, *t);
            assert_eq!(b.rto, t.default_rto());
            assert_eq!(b.rpo, t.default_rpo());
            assert_eq!(b.drill_cadence, t.default_drill_cadence());
        }
    }

    #[test]
    fn drill_serde_round_trips() {
        let d = Drill {
            id: "tier1-payments".into(),
            tier: DrTier::Tier1,
            component: "payments".into(),
            region: RegionId::parse("us-east-1").unwrap(),
            scenario: "primary down".into(),
            last_run_at: Some(datetime!(2026-04-15 12:00:00 UTC)),
        };
        let s = serde_json::to_string(&d).unwrap();
        let back: Drill = serde_json::from_str(&s).unwrap();
        assert_eq!(d, back);
    }

    #[test]
    fn drill_rejects_unknown_field() {
        let bad = r#"{"id":"x","tier":"tier-1","component":"x","region":"us-east-1","scenario":"x","ahem":1}"#;
        let r: Result<Drill, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    // Regression-guard for the kebab-case-rename / manual-slug
    // divergence bug: serde's `rename_all = "kebab-case"` does
    // not insert a hyphen between `tier` and `1`, so a bare
    // rename_all would emit `tier1` on the wire — but slug()
    // returns `tier-1`. The per-variant `#[serde(rename)]`
    // attributes added in T91 fourth wiring make them match;
    // this test enforces that they stay matched.
    #[test]
    fn tier_serde_wire_matches_slug() {
        for t in DrTier::ALL.iter() {
            let wire = serde_json::to_string(t).unwrap();
            let stripped = wire.trim_matches('"');
            assert_eq!(stripped, t.slug(), "wire vs slug for {:?}", t);
        }
    }
}
