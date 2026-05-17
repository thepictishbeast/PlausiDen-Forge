//! `deploy-security-rating` — typed projection of
//! [`deploy_core::SecurityProfile`] into a comparable per-target
//! score.
//!
//! Closes the deploy-adapter set (#38–#42) with the dashboard
//! layer paul's PLATFORM_ROADMAP §4 calls for. Every adapter
//! already declares its typed [`SecurityProfile`] at the trait
//! level — this crate is a **pure projection** over those, with
//! zero per-adapter special-casing. Add a new adapter, the
//! dashboard picks it up automatically.
//!
//! ### Scoring discipline
//!
//! Scores are **declarative**, not arbitrary. Each axis maps from
//! its closed-enum variant to a fixed integer (0..=4). The
//! integer is what consumers compare on. The mapping lives in
//! exactly one place (this crate) so cross-target comparisons are
//! always consistent.
//!
//! ### Why this matters
//!
//! Per `super_society_tech_stack`: claims of privacy + anonymity
//! + censorship-resistance only mean something when they're
//! comparable. The dashboard refuses to render a Tor-mode deploy
//! as "equivalent to clearnet" because every axis is typed +
//! every comparison is honest.
//!
//! ### Public surface
//!
//! - [`TargetRating`]       — one row per declared deploy target
//! - [`AxisScores`]         — typed integer score per axis
//! - [`OverallRating`]      — discrete summary (Excellent /
//!                             Strong / Moderate / Weak)
//! - [`RatingReport`]       — collection of [`TargetRating`]s
//! - [`rate_profile()`]     — pure projection from a profile
//! - [`rate_adapter()`]     — convenience over a `&dyn DeployAdapter`

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use deploy_core::{
    AnonymityLevel, CensorshipResistance, DeployAdapter, SecurityProfile, TrafficObservability,
};
use serde::{Deserialize, Serialize};

/// Integer scores per axis. Always 0..=4. The mapping is fixed
/// here and applied identically to every adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct AxisScores {
    /// Reader anonymity: None=0, Partial=2, Strong=4.
    pub reader_anonymity: u8,
    /// Publisher anonymity: None=0, Partial=2, Strong=4.
    pub publisher_anonymity: u8,
    /// Traffic observability (inverted): High=0, Medium=2, Low=4.
    pub traffic_privacy: u8,
    /// Censorship resistance: Low=0, Medium=2, High=4.
    pub censorship_resistance: u8,
    /// Tamper resistance — derived from `content_addressed` +
    /// `uses_standard_tls`. Content-addressed = 4. Standard TLS
    /// only = 2. Neither = 0.
    pub tamper_resistance: u8,
}

impl AxisScores {
    /// Maximum theoretical score = 5 × 4 = 20.
    pub const MAX_TOTAL: u8 = 20;

    /// Sum of all axes (0..=20).
    pub fn total(&self) -> u8 {
        self.reader_anonymity
            + self.publisher_anonymity
            + self.traffic_privacy
            + self.censorship_resistance
            + self.tamper_resistance
    }
}

/// Discrete summary derived from [`AxisScores::total`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverallRating {
    /// Total ≥ 16 — overlay-class privacy + tamper resistance.
    Excellent,
    /// Total 10..=15 — strong on most axes.
    Strong,
    /// Total 5..=9 — mixed; useful for some threat models.
    Moderate,
    /// Total ≤ 4 — best treated as a "public reading view" tier.
    Weak,
}

impl OverallRating {
    /// Derive the discrete rating from a total score.
    pub fn from_total(total: u8) -> Self {
        match total {
            16..=u8::MAX => Self::Excellent,
            10..=15 => Self::Strong,
            5..=9 => Self::Moderate,
            _ => Self::Weak,
        }
    }
}

/// One row of the dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TargetRating {
    /// The adapter ID (e.g. `"tor-onion"`).
    pub adapter_id: String,
    /// The raw profile the adapter advertised.
    pub profile: SecurityProfile,
    /// Per-axis scores (0..=4).
    pub axes: AxisScores,
    /// Total across all axes (0..=20).
    pub total: u8,
    /// Discrete summary.
    pub overall: OverallRating,
}

/// Aggregate report — what the admin dashboard renders + what the
/// manifest gate can attach to a build report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct RatingReport {
    /// One entry per declared target.
    pub ratings: Vec<TargetRating>,
}

impl RatingReport {
    /// Highest-rated target — the platform's "best privacy
    /// option" for this site.
    pub fn best(&self) -> Option<&TargetRating> {
        self.ratings.iter().max_by_key(|r| r.total)
    }

    /// Lowest-rated target — the platform's "weakest link" the
    /// admin should be aware of.
    pub fn weakest(&self) -> Option<&TargetRating> {
        self.ratings.iter().min_by_key(|r| r.total)
    }
}

/// Pure projection from a [`SecurityProfile`] to typed scores.
/// Reused by [`rate_adapter`] and by future consumers that
/// synthesize profiles directly (e.g. cached snapshots).
pub fn rate_profile(adapter_id: impl Into<String>, profile: SecurityProfile) -> TargetRating {
    let axes = AxisScores {
        reader_anonymity: score_anonymity(profile.reader_anonymity),
        publisher_anonymity: score_anonymity(profile.publisher_anonymity),
        traffic_privacy: score_traffic(profile.traffic_observability),
        censorship_resistance: score_censorship(profile.censorship_resistance),
        tamper_resistance: score_tamper(profile.content_addressed, profile.uses_standard_tls),
    };
    let total = axes.total();
    TargetRating {
        adapter_id: adapter_id.into(),
        profile,
        axes,
        total,
        overall: OverallRating::from_total(total),
    }
}

/// Convenience: rate an adapter through its declared profile.
pub fn rate_adapter(adapter: &dyn DeployAdapter) -> TargetRating {
    rate_profile(adapter.id(), adapter.profile())
}

fn score_anonymity(a: AnonymityLevel) -> u8 {
    match a {
        AnonymityLevel::None => 0,
        AnonymityLevel::Partial => 2,
        AnonymityLevel::Strong => 4,
    }
}

fn score_traffic(t: TrafficObservability) -> u8 {
    match t {
        TrafficObservability::High => 0,
        TrafficObservability::Medium => 2,
        TrafficObservability::Low => 4,
    }
}

fn score_censorship(c: CensorshipResistance) -> u8 {
    match c {
        CensorshipResistance::Low => 0,
        CensorshipResistance::Medium => 2,
        CensorshipResistance::High => 4,
    }
}

fn score_tamper(content_addressed: bool, uses_standard_tls: bool) -> u8 {
    if content_addressed {
        4
    } else if uses_standard_tls {
        2
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deploy_core::SecurityProfile;

    #[test]
    fn clearnet_baseline_rates_weak() {
        let r = rate_profile("clearnet", SecurityProfile::clearnet_baseline());
        // None + None anonymity, High observability, Low censorship
        // resistance, standard TLS only = 0 + 0 + 0 + 0 + 2 = 2.
        assert_eq!(r.axes.total(), 2);
        assert_eq!(r.total, 2);
        assert_eq!(r.overall, OverallRating::Weak);
    }

    #[test]
    fn tor_baseline_rates_excellent() {
        let r = rate_profile("tor-onion", SecurityProfile::tor_onion_baseline());
        // Strong + Strong, Low observability, High censorship,
        // no content-address + no standard TLS
        // = 4 + 4 + 4 + 4 + 0 = 16.
        assert_eq!(r.axes.total(), 16);
        assert_eq!(r.overall, OverallRating::Excellent);
    }

    #[test]
    fn ipfs_baseline_rates_moderate_with_tamper_bonus() {
        let r = rate_profile("ipfs", SecurityProfile::ipfs_baseline());
        // None + None, Medium observability (2), Medium censorship (2),
        // content_addressed (4) = 0 + 0 + 2 + 2 + 4 = 8.
        assert_eq!(r.axes.total(), 8);
        assert_eq!(r.overall, OverallRating::Moderate);
    }

    #[test]
    fn overall_thresholds_match_boundaries() {
        assert_eq!(OverallRating::from_total(20), OverallRating::Excellent);
        assert_eq!(OverallRating::from_total(16), OverallRating::Excellent);
        assert_eq!(OverallRating::from_total(15), OverallRating::Strong);
        assert_eq!(OverallRating::from_total(10), OverallRating::Strong);
        assert_eq!(OverallRating::from_total(9), OverallRating::Moderate);
        assert_eq!(OverallRating::from_total(5), OverallRating::Moderate);
        assert_eq!(OverallRating::from_total(4), OverallRating::Weak);
        assert_eq!(OverallRating::from_total(0), OverallRating::Weak);
    }

    #[test]
    fn report_best_picks_highest_total() {
        let report = RatingReport {
            ratings: vec![
                rate_profile("clearnet", SecurityProfile::clearnet_baseline()),
                rate_profile("tor", SecurityProfile::tor_onion_baseline()),
                rate_profile("ipfs", SecurityProfile::ipfs_baseline()),
            ],
        };
        let best = report.best().unwrap();
        assert_eq!(best.adapter_id, "tor");
    }

    #[test]
    fn report_weakest_picks_lowest_total() {
        let report = RatingReport {
            ratings: vec![
                rate_profile("tor", SecurityProfile::tor_onion_baseline()),
                rate_profile("clearnet", SecurityProfile::clearnet_baseline()),
            ],
        };
        let w = report.weakest().unwrap();
        assert_eq!(w.adapter_id, "clearnet");
    }

    #[test]
    fn axis_scores_round_trip_serde() {
        let r = rate_profile("tor", SecurityProfile::tor_onion_baseline());
        let s = serde_json::to_string(&r).unwrap();
        let back: TargetRating = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn max_total_constant_matches_per_axis_max() {
        assert_eq!(AxisScores::MAX_TOTAL, 5 * 4);
    }

    #[test]
    fn tamper_score_orders_content_addressed_above_tls() {
        assert_eq!(score_tamper(true, true), 4);
        assert_eq!(score_tamper(true, false), 4);
        assert_eq!(score_tamper(false, true), 2);
        assert_eq!(score_tamper(false, false), 0);
    }
}
