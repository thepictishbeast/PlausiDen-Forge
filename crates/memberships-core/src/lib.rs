//! `memberships-core` — typed memberships + paywall contract.
//!
//! Per `PLATFORM_ROADMAP.md` §19, every PlausiDen tenant can run
//! a creator-economy / reader-supported model: tiered
//! memberships, metered paywalls (NYT-style), hard paywalls
//! (Substack-style), or time-based previews. This crate defines
//! the typed surface; per-payment-provider integrations
//! (commerce-storefront-core T84 + Stripe / etc.) drive the
//! actual billing.
//!
//! ### Why typed
//!
//! Paywalls are the canonical place a creator's "free article"
//! goes behind the meter because some helper function defaulted
//! wrong. Closed [`PaywallStrategy`] + typed [`AccessDecision`]
//! makes the access path explicit at every call site.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Closed enum of paywall strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PaywallStrategy {
    /// No paywall — content is freely accessible.
    Open,
    /// Hard paywall — Substack-style; non-members can see
    /// metadata + first paragraph only.
    Hard,
    /// Metered paywall — NYT-style; N free reads per
    /// rolling 30-day window, then prompted to subscribe.
    Metered,
    /// Time-based — content is free for a window after publish
    /// (e.g. 7 days) then becomes member-only.
    TimeBased,
    /// Preview — non-members see a fixed-length preview then a
    /// gated remainder.
    Preview,
}

impl PaywallStrategy {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Hard => "hard",
            Self::Metered => "metered",
            Self::TimeBased => "time-based",
            Self::Preview => "preview",
        }
    }
}

/// One membership tier the operator offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Tier {
    /// Stable tier id (kebab-case).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Rank — higher rank = more privileges. Used for upgrade
    /// detection in [`AccessDecision`].
    pub rank: u8,
    /// Monthly price in smallest currency unit (cents etc.).
    /// `0` means free tier.
    pub monthly_price: i64,
    /// Currency code (ISO 4217 3-upper).
    pub currency: String,
    /// Whether this tier is annual-billed only (no monthly).
    pub annual_only: bool,
}

impl Tier {
    /// Validate the tier:
    ///   * id non-empty + kebab-case
    ///   * name non-empty
    ///   * monthly_price ≥ 0
    ///   * currency is 3 uppercase ASCII
    pub fn validate(&self) -> Result<(), MembershipError> {
        if self.name.trim().is_empty() {
            return Err(MembershipError::Invalid("tier name empty".into()));
        }
        if !is_kebab(&self.id) {
            return Err(MembershipError::Invalid(format!(
                "tier id not kebab: {}",
                self.id
            )));
        }
        if self.monthly_price < 0 {
            return Err(MembershipError::Invalid(format!(
                "tier {} monthly_price negative",
                self.id
            )));
        }
        if self.currency.len() != 3 || !self.currency.chars().all(|c| c.is_ascii_uppercase()) {
            return Err(MembershipError::Invalid(format!(
                "tier {} currency not ISO 4217: {}",
                self.id, self.currency
            )));
        }
        Ok(())
    }
}

/// Paywall configuration for one piece of content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ContentPaywall {
    /// Stable content id (operator-defined; usually a CmsSection id).
    pub content_id: String,
    /// Paywall strategy.
    pub strategy: PaywallStrategy,
    /// Minimum tier required to read (only meaningful for
    /// Hard / Preview strategies). `None` = any member tier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_tier: Option<String>,
    /// Free-window length for TimeBased strategy. Days.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_window_days: Option<u32>,
    /// Preview length for Preview strategy. Characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_char_count: Option<u32>,
}

/// Meter state for a single reader against a metered paywall.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct MeterState {
    /// Subject id (opaque).
    pub subject_id: String,
    /// Reads consumed in the current rolling window.
    pub reads_used: u32,
    /// Operator-configured limit.
    pub reads_per_window: u32,
    /// Window start. Rolling — when (now - window_start) >
    /// window_length, reset reads_used to 0 + slide
    /// window_start forward.
    pub window_start: time::OffsetDateTime,
    /// Window length (days).
    pub window_days: u32,
}

impl MeterState {
    /// Whether the meter is exhausted at the supplied "now".
    pub fn is_exhausted(&self, now: time::OffsetDateTime) -> bool {
        if self.has_window_rolled(now) {
            return false;
        }
        self.reads_used >= self.reads_per_window
    }

    /// Whether the rolling window has rolled over at the
    /// supplied "now".
    pub fn has_window_rolled(&self, now: time::OffsetDateTime) -> bool {
        (now - self.window_start).whole_days() >= i64::from(self.window_days)
    }
}

/// Reader's membership snapshot at access time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct MemberSnapshot {
    /// Subject id (opaque).
    pub subject_id: String,
    /// Tier the subject currently holds, or `None` if not a
    /// member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier_id: Option<String>,
    /// Tier rank, mirrors Tier::rank for the held tier. `None`
    /// when not a member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier_rank: Option<u8>,
}

/// Access decision the runtime returns to the renderer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AccessDecision {
    /// Full access — render the content unchanged.
    Allowed,
    /// Reader needs to subscribe to access at all.
    RequiresMembership {
        /// Suggested tier id.
        suggested_tier: String,
    },
    /// Reader is a member but at the wrong tier — needs upgrade.
    RequiresUpgrade {
        /// Current tier the reader holds.
        current_tier: String,
        /// Minimum tier required.
        required_tier: String,
    },
    /// Metered paywall: reader has used their quota.
    MeterExhausted {
        /// Reads used.
        used: u32,
        /// Limit.
        limit: u32,
    },
    /// Preview only — render the first N chars + gated remainder.
    Preview {
        /// Characters to render in the open preview.
        char_count: u32,
    },
}

impl AccessDecision {
    /// Stable kebab-case discriminant slug.
    pub fn kind_slug(&self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::RequiresMembership { .. } => "requires-membership",
            Self::RequiresUpgrade { .. } => "requires-upgrade",
            Self::MeterExhausted { .. } => "meter-exhausted",
            Self::Preview { .. } => "preview",
        }
    }

    /// Whether the decision permits content to render fully.
    pub fn is_full_access(&self) -> bool {
        matches!(self, Self::Allowed)
    }
}

/// Decide access for one (content, reader, optional meter) triple.
pub fn decide(
    paywall: &ContentPaywall,
    reader: &MemberSnapshot,
    meter: Option<&MeterState>,
    publish_time: Option<time::OffsetDateTime>,
    now: time::OffsetDateTime,
) -> AccessDecision {
    match paywall.strategy {
        PaywallStrategy::Open => AccessDecision::Allowed,
        PaywallStrategy::Hard => decide_tier_gated(paywall, reader),
        PaywallStrategy::Metered => {
            if reader.tier_id.is_some() {
                return AccessDecision::Allowed;
            }
            match meter {
                Some(m) if m.is_exhausted(now) => AccessDecision::MeterExhausted {
                    used: m.reads_used,
                    limit: m.reads_per_window,
                },
                _ => AccessDecision::Allowed,
            }
        }
        PaywallStrategy::TimeBased => {
            if let (Some(pub_t), Some(days)) = (publish_time, paywall.free_window_days) {
                let age_days = (now - pub_t).whole_days();
                if age_days < i64::from(days) {
                    return AccessDecision::Allowed;
                }
            }
            decide_tier_gated(paywall, reader)
        }
        PaywallStrategy::Preview => {
            // Preview is always "gated for non-members" — members
            // get full access; non-members get a preview.
            if reader.tier_id.is_some() {
                AccessDecision::Allowed
            } else {
                AccessDecision::Preview {
                    char_count: paywall.preview_char_count.unwrap_or(280),
                }
            }
        }
    }
}

fn decide_tier_gated(paywall: &ContentPaywall, reader: &MemberSnapshot) -> AccessDecision {
    match (&paywall.min_tier, &reader.tier_id) {
        (Some(req), Some(held)) if req == held => AccessDecision::Allowed,
        (Some(req), Some(held)) => AccessDecision::RequiresUpgrade {
            current_tier: held.clone(),
            required_tier: req.clone(),
        },
        (Some(req), None) => AccessDecision::RequiresMembership {
            suggested_tier: req.clone(),
        },
        (None, Some(_)) => AccessDecision::Allowed,
        (None, None) => AccessDecision::RequiresMembership {
            suggested_tier: "any".into(),
        },
    }
}

fn is_kebab(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut prev_dash = false;
    for (i, c) in s.chars().enumerate() {
        let last = i + 1 == s.len();
        match c {
            'a'..='z' | '0'..='9' => prev_dash = false,
            '-' => {
                if i == 0 || last || prev_dash {
                    return false;
                }
                prev_dash = true;
            }
            _ => return false,
        }
    }
    true
}

/// Typed errors at the membership boundary.
#[derive(Debug, thiserror::Error)]
pub enum MembershipError {
    /// Invalid configuration.
    #[error("invalid: {0}")]
    Invalid(String),
    /// Backend error.
    #[error("backend: {0}")]
    Backend(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn tier_ok() -> Tier {
        Tier {
            id: "supporter".into(),
            name: "Supporter".into(),
            rank: 1,
            monthly_price: 500,
            currency: "USD".into(),
            annual_only: false,
        }
    }

    #[test]
    fn paywall_strategy_slugs_distinct() {
        let ss = [
            PaywallStrategy::Open,
            PaywallStrategy::Hard,
            PaywallStrategy::Metered,
            PaywallStrategy::TimeBased,
            PaywallStrategy::Preview,
        ];
        let mut s = std::collections::HashSet::new();
        for x in ss {
            assert!(s.insert(x.slug()));
        }
    }

    #[test]
    fn tier_validate_happy_path() {
        assert!(tier_ok().validate().is_ok());
    }

    #[test]
    fn tier_rejects_non_kebab_id() {
        let mut t = tier_ok();
        t.id = "Supporter".into();
        assert!(t.validate().is_err());
    }

    #[test]
    fn tier_rejects_empty_name() {
        let mut t = tier_ok();
        t.name = "".into();
        assert!(t.validate().is_err());
    }

    #[test]
    fn tier_rejects_negative_price() {
        let mut t = tier_ok();
        t.monthly_price = -1;
        assert!(t.validate().is_err());
    }

    #[test]
    fn tier_rejects_lowercase_currency() {
        let mut t = tier_ok();
        t.currency = "usd".into();
        assert!(t.validate().is_err());
    }

    #[test]
    fn access_decision_kind_slug_unique() {
        let ds = [
            AccessDecision::Allowed.kind_slug(),
            AccessDecision::RequiresMembership {
                suggested_tier: "x".into(),
            }
            .kind_slug(),
            AccessDecision::RequiresUpgrade {
                current_tier: "a".into(),
                required_tier: "b".into(),
            }
            .kind_slug(),
            AccessDecision::MeterExhausted { used: 0, limit: 0 }.kind_slug(),
            AccessDecision::Preview { char_count: 0 }.kind_slug(),
        ];
        let mut s = std::collections::HashSet::new();
        for d in ds {
            assert!(s.insert(d));
        }
    }

    #[test]
    fn open_strategy_always_allows() {
        let pw = ContentPaywall {
            content_id: "c1".into(),
            strategy: PaywallStrategy::Open,
            min_tier: Some("supporter".into()),
            free_window_days: None,
            preview_char_count: None,
        };
        let reader = MemberSnapshot {
            subject_id: "alice".into(),
            tier_id: None,
            tier_rank: None,
        };
        let d = decide(&pw, &reader, None, None, datetime!(2026-05-18 00:00 UTC));
        assert_eq!(d, AccessDecision::Allowed);
    }

    #[test]
    fn hard_strategy_requires_membership_when_anonymous() {
        let pw = ContentPaywall {
            content_id: "c1".into(),
            strategy: PaywallStrategy::Hard,
            min_tier: Some("supporter".into()),
            free_window_days: None,
            preview_char_count: None,
        };
        let anon = MemberSnapshot {
            subject_id: "alice".into(),
            tier_id: None,
            tier_rank: None,
        };
        let d = decide(&pw, &anon, None, None, datetime!(2026-05-18 00:00 UTC));
        assert!(matches!(d, AccessDecision::RequiresMembership { .. }));
    }

    #[test]
    fn hard_strategy_allows_correct_tier() {
        let pw = ContentPaywall {
            content_id: "c1".into(),
            strategy: PaywallStrategy::Hard,
            min_tier: Some("supporter".into()),
            free_window_days: None,
            preview_char_count: None,
        };
        let member = MemberSnapshot {
            subject_id: "alice".into(),
            tier_id: Some("supporter".into()),
            tier_rank: Some(1),
        };
        let d = decide(&pw, &member, None, None, datetime!(2026-05-18 00:00 UTC));
        assert_eq!(d, AccessDecision::Allowed);
    }

    #[test]
    fn hard_strategy_requires_upgrade_when_wrong_tier() {
        let pw = ContentPaywall {
            content_id: "c1".into(),
            strategy: PaywallStrategy::Hard,
            min_tier: Some("premium".into()),
            free_window_days: None,
            preview_char_count: None,
        };
        let basic = MemberSnapshot {
            subject_id: "alice".into(),
            tier_id: Some("supporter".into()),
            tier_rank: Some(1),
        };
        let d = decide(&pw, &basic, None, None, datetime!(2026-05-18 00:00 UTC));
        assert!(matches!(d, AccessDecision::RequiresUpgrade { .. }));
    }

    #[test]
    fn metered_allows_under_limit() {
        let pw = ContentPaywall {
            content_id: "c1".into(),
            strategy: PaywallStrategy::Metered,
            min_tier: None,
            free_window_days: None,
            preview_char_count: None,
        };
        let anon = MemberSnapshot {
            subject_id: "alice".into(),
            tier_id: None,
            tier_rank: None,
        };
        let meter = MeterState {
            subject_id: "alice".into(),
            reads_used: 2,
            reads_per_window: 5,
            window_start: datetime!(2026-05-01 00:00 UTC),
            window_days: 30,
        };
        let d = decide(
            &pw,
            &anon,
            Some(&meter),
            None,
            datetime!(2026-05-18 00:00 UTC),
        );
        assert_eq!(d, AccessDecision::Allowed);
    }

    #[test]
    fn metered_exhausts_at_limit() {
        let pw = ContentPaywall {
            content_id: "c1".into(),
            strategy: PaywallStrategy::Metered,
            min_tier: None,
            free_window_days: None,
            preview_char_count: None,
        };
        let anon = MemberSnapshot {
            subject_id: "alice".into(),
            tier_id: None,
            tier_rank: None,
        };
        let meter = MeterState {
            subject_id: "alice".into(),
            reads_used: 5,
            reads_per_window: 5,
            window_start: datetime!(2026-05-15 00:00 UTC),
            window_days: 30,
        };
        let d = decide(
            &pw,
            &anon,
            Some(&meter),
            None,
            datetime!(2026-05-18 00:00 UTC),
        );
        assert!(matches!(d, AccessDecision::MeterExhausted { .. }));
    }

    #[test]
    fn metered_resets_after_window() {
        // After window rolls, reads_used is treated as 0 again.
        let pw = ContentPaywall {
            content_id: "c1".into(),
            strategy: PaywallStrategy::Metered,
            min_tier: None,
            free_window_days: None,
            preview_char_count: None,
        };
        let anon = MemberSnapshot {
            subject_id: "alice".into(),
            tier_id: None,
            tier_rank: None,
        };
        let meter = MeterState {
            subject_id: "alice".into(),
            reads_used: 99,
            reads_per_window: 5,
            window_start: datetime!(2026-04-01 00:00 UTC),
            window_days: 30,
        };
        // now is well past window_start + 30 days.
        let d = decide(
            &pw,
            &anon,
            Some(&meter),
            None,
            datetime!(2026-05-18 00:00 UTC),
        );
        assert_eq!(d, AccessDecision::Allowed);
    }

    #[test]
    fn time_based_free_during_window() {
        let pw = ContentPaywall {
            content_id: "c1".into(),
            strategy: PaywallStrategy::TimeBased,
            min_tier: Some("supporter".into()),
            free_window_days: Some(7),
            preview_char_count: None,
        };
        let anon = MemberSnapshot {
            subject_id: "alice".into(),
            tier_id: None,
            tier_rank: None,
        };
        let d = decide(
            &pw,
            &anon,
            None,
            Some(datetime!(2026-05-15 00:00 UTC)),
            datetime!(2026-05-18 00:00 UTC),
        );
        assert_eq!(d, AccessDecision::Allowed);
    }

    #[test]
    fn time_based_gates_after_window() {
        let pw = ContentPaywall {
            content_id: "c1".into(),
            strategy: PaywallStrategy::TimeBased,
            min_tier: Some("supporter".into()),
            free_window_days: Some(7),
            preview_char_count: None,
        };
        let anon = MemberSnapshot {
            subject_id: "alice".into(),
            tier_id: None,
            tier_rank: None,
        };
        let d = decide(
            &pw,
            &anon,
            None,
            Some(datetime!(2026-04-01 00:00 UTC)),
            datetime!(2026-05-18 00:00 UTC),
        );
        assert!(matches!(d, AccessDecision::RequiresMembership { .. }));
    }

    #[test]
    fn preview_gates_anonymous_with_char_count() {
        let pw = ContentPaywall {
            content_id: "c1".into(),
            strategy: PaywallStrategy::Preview,
            min_tier: None,
            free_window_days: None,
            preview_char_count: Some(500),
        };
        let anon = MemberSnapshot {
            subject_id: "alice".into(),
            tier_id: None,
            tier_rank: None,
        };
        let d = decide(&pw, &anon, None, None, datetime!(2026-05-18 00:00 UTC));
        assert_eq!(d, AccessDecision::Preview { char_count: 500 });
    }

    #[test]
    fn preview_full_access_for_members() {
        let pw = ContentPaywall {
            content_id: "c1".into(),
            strategy: PaywallStrategy::Preview,
            min_tier: None,
            free_window_days: None,
            preview_char_count: Some(500),
        };
        let member = MemberSnapshot {
            subject_id: "alice".into(),
            tier_id: Some("supporter".into()),
            tier_rank: Some(1),
        };
        let d = decide(&pw, &member, None, None, datetime!(2026-05-18 00:00 UTC));
        assert_eq!(d, AccessDecision::Allowed);
    }

    #[test]
    fn tier_serde_round_trip() {
        let t = tier_ok();
        let j = serde_json::to_string(&t).unwrap();
        let back: Tier = serde_json::from_str(&j).unwrap();
        assert_eq!(t, back);
    }
}
