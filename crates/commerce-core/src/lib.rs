//! `commerce-core` — typed Stripe billing + self-serve onboarding +
//! customer-admin permissions.
//!
//! The server runtime wires this up against Stripe's REST API
//! (or the test-mode equivalent). This crate is the typed
//! contract every billing-aware consumer (admin UI, webhook
//! handler, upgrade flow) projects through. Same pattern as
//! every other -core crate: no IO, no network, just the typed
//! shape + state-machine helpers.
//!
//! ### Surface
//!
//! - [`PlanTier`]          — closed Free / Starter / Pro / Enterprise enum
//! - [`PlanCatalog`]       — price + features + limits per tier
//! - [`StripeCustomerId`]  — opaque Stripe `cus_*` identifier
//! - [`SubscriptionStatus`]— closed enum mirroring Stripe's
//!                            subscription lifecycle
//! - [`Subscription`]      — current state for one customer
//! - [`OnboardingStep`]    — typed 5-step self-serve flow
//! - [`OnboardingState`]   — operator's current position
//! - [`AdminRole`]         — Owner / Admin / Editor / Viewer
//! - [`AdminPermission`]   — closed enum of permission slugs
//! - [`AdminRole`] × [`AdminPermission`] — role → permission set
//!
//! Per `super_society_tech_stack`: every customer-visible billing
//! decision is computed from typed inputs. No "we think the
//! quota is 100" string-matching.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

// ============================================================
// PLAN CATALOG
// ============================================================

/// Subscription plan tier — substrate-canonical billing levels.
/// Closed enum (new tiers are a typed change, not a free-form
/// string). Tiers are ordered (`Free` < `Starter` < `Pro` <
/// ...) so the natural `Ord` derive matches access-control
/// checks.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "kebab-case")]
pub enum PlanTier {
    /// Free tier — limited usage, no billing.
    #[default]
    Free,
    /// Starter — small-scale paid.
    Starter,
    /// Pro — production-grade.
    Pro,
    /// Enterprise — custom contract.
    Enterprise,
}

impl PlanTier {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Starter => "starter",
            Self::Pro => "pro",
            Self::Enterprise => "enterprise",
        }
    }

    /// All tiers in declaration order.
    pub const ALL: &'static [PlanTier] = &[Self::Free, Self::Starter, Self::Pro, Self::Enterprise];
}

/// Platform plan-catalog entry: price + limits + features per
/// tier. Operators build a [`PlanCatalog`] from a config file
/// at boot time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PlanEntry {
    /// Which tier this entry describes.
    pub tier: PlanTier,
    /// Display name (e.g. `"Pro"`, `"Pro Plus"`).
    pub name: String,
    /// One-line description for the pricing page.
    pub tagline: String,
    /// Monthly price in cents (USD by default; multi-currency
    /// stored in `prices` map).
    pub monthly_price_cents: u32,
    /// Stripe Price ID for this tier (`"price_..."`).
    pub stripe_price_id: String,
    /// Closed-enum feature flags this tier unlocks.
    #[serde(default)]
    pub features: Vec<PlanFeature>,
    /// Limits — usage caps before overage applies.
    pub limits: PlanLimits,
}

/// Operator-tunable limits per plan tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PlanLimits {
    /// Max sites this customer can host.
    pub max_sites: u32,
    /// Max bytes egress per month (0 = unlimited).
    pub max_egress_bytes_month: u64,
    /// Max admin seats included.
    pub max_admin_seats: u32,
    /// Max storage bytes for media (0 = unlimited).
    pub max_storage_bytes: u64,
}

/// Closed enum of feature flags that gate functionality per plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanFeature {
    /// Custom domain support.
    CustomDomain,
    /// Multi-region failover.
    MultiRegion,
    /// Tor / I2P / IPFS deployment targets.
    AltNetworkTargets,
    /// SAML / OIDC SSO for admin login.
    Sso,
    /// Audit log export.
    AuditExport,
    /// SLA contract.
    Sla,
    /// Per-tenant SQLite (DataIsolated tenancy tier).
    DataIsolatedTenancy,
    /// FullyIsolated tenancy (per-tenant subprocess + network
    /// namespace).
    FullyIsolatedTenancy,
}

impl PlanFeature {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::CustomDomain => "custom-domain",
            Self::MultiRegion => "multi-region",
            Self::AltNetworkTargets => "alt-network-targets",
            Self::Sso => "sso",
            Self::AuditExport => "audit-export",
            Self::Sla => "sla",
            Self::DataIsolatedTenancy => "data-isolated-tenancy",
            Self::FullyIsolatedTenancy => "fully-isolated-tenancy",
        }
    }
}

/// The full plan catalog operators ship.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PlanCatalog {
    /// One entry per tier the platform offers.
    pub plans: Vec<PlanEntry>,
}

impl PlanCatalog {
    /// Look up a plan entry by tier.
    pub fn get(&self, tier: PlanTier) -> Option<&PlanEntry> {
        self.plans.iter().find(|p| p.tier == tier)
    }

    /// Verify the catalog contains every declared tier.
    /// Operators MUST declare every tier they advertise; missing
    /// entries are a config bug.
    pub fn verify_complete(&self, tiers: &[PlanTier]) -> Result<(), CommerceError> {
        for tier in tiers {
            if self.get(*tier).is_none() {
                return Err(CommerceError::CatalogMissingTier(*tier));
            }
        }
        Ok(())
    }
}

// ============================================================
// STRIPE IDENTIFIERS + SUBSCRIPTION STATE
// ============================================================

/// Opaque Stripe customer identifier. Validates the `cus_*`
/// shape so a stray UUID can't get plugged in by accident.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StripeCustomerId(String);

impl StripeCustomerId {
    /// Construct from a string. Validates `cus_` prefix + length.
    pub fn parse(s: impl AsRef<str>) -> Result<Self, CommerceError> {
        let s = s.as_ref();
        if !s.starts_with("cus_") || s.len() < 8 || s.len() > 64 {
            return Err(CommerceError::InvalidStripeId(format!(
                "{s:?} not a valid cus_* id"
            )));
        }
        Ok(Self(s.to_string()))
    }

    /// Raw view.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque Stripe subscription identifier (`sub_*`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StripeSubscriptionId(String);

impl StripeSubscriptionId {
    /// Construct from a string. Validates `sub_` prefix + length.
    pub fn parse(s: impl AsRef<str>) -> Result<Self, CommerceError> {
        let s = s.as_ref();
        if !s.starts_with("sub_") || s.len() < 8 || s.len() > 64 {
            return Err(CommerceError::InvalidStripeId(format!(
                "{s:?} not a valid sub_* id"
            )));
        }
        Ok(Self(s.to_string()))
    }

    /// Raw view.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Closed enum mirroring Stripe's subscription lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubscriptionStatus {
    /// Trial period — paid features available.
    Trialing,
    /// Paid + current.
    Active,
    /// Payment failed; in dunning.
    PastDue,
    /// Operator-initiated cancellation; service ends at period end.
    Canceled,
    /// Stripe abandoned the subscription (e.g. trial expired
    /// without payment).
    Unpaid,
    /// Suspended pending operator action.
    Incomplete,
}

impl SubscriptionStatus {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Trialing => "trialing",
            Self::Active => "active",
            Self::PastDue => "past-due",
            Self::Canceled => "canceled",
            Self::Unpaid => "unpaid",
            Self::Incomplete => "incomplete",
        }
    }

    /// Whether paid features should be enabled in this state.
    pub fn grants_paid_features(&self) -> bool {
        matches!(self, Self::Trialing | Self::Active)
    }
}

/// Current subscription state for one customer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Subscription {
    /// Stripe customer id.
    pub customer: StripeCustomerId,
    /// Stripe subscription id.
    pub subscription: StripeSubscriptionId,
    /// Plan tier the customer is on.
    pub tier: PlanTier,
    /// Current status.
    pub status: SubscriptionStatus,
    /// Current period end (when the next renewal charge / expiry
    /// fires).
    pub current_period_end: time::OffsetDateTime,
    /// Whether `cancel_at_period_end` is set (operator-initiated
    /// downgrade pending).
    #[serde(default)]
    pub cancel_at_period_end: bool,
}

impl Subscription {
    /// Effective tier — Free if status doesn't grant paid features.
    pub fn effective_tier(&self) -> PlanTier {
        if self.status.grants_paid_features() {
            self.tier
        } else {
            PlanTier::Free
        }
    }
}

// ============================================================
// ONBOARDING STATE MACHINE
// ============================================================

/// Closed enum of self-serve onboarding steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OnboardingStep {
    /// Email + handle entered.
    Signup,
    /// Plan tier selected.
    PlanPick,
    /// Payment method captured (Stripe Setup Intent confirmed).
    Payment,
    /// Email + identity verification complete.
    Verify,
    /// First site bootstrapped + ready.
    Ready,
}

impl OnboardingStep {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Signup => "signup",
            Self::PlanPick => "plan-pick",
            Self::Payment => "payment",
            Self::Verify => "verify",
            Self::Ready => "ready",
        }
    }

    /// Next step in the canonical flow, or None at Ready.
    pub fn next(&self) -> Option<OnboardingStep> {
        match self {
            Self::Signup => Some(Self::PlanPick),
            Self::PlanPick => Some(Self::Payment),
            Self::Payment => Some(Self::Verify),
            Self::Verify => Some(Self::Ready),
            Self::Ready => None,
        }
    }

    /// All steps in flow order.
    pub const ALL: &'static [OnboardingStep] = &[
        Self::Signup,
        Self::PlanPick,
        Self::Payment,
        Self::Verify,
        Self::Ready,
    ];
}

/// Operator's current position in the onboarding flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct OnboardingState {
    /// Step the operator is currently on.
    pub current: OnboardingStep,
    /// Tier selected (or Free if not yet picked).
    pub tier: PlanTier,
    /// Email confirmed.
    pub email_verified: bool,
    /// Payment method on file.
    pub payment_on_file: bool,
}

impl OnboardingState {
    /// Initial state — fresh signup, no choices yet.
    pub fn fresh() -> Self {
        Self {
            current: OnboardingStep::Signup,
            tier: PlanTier::Free,
            email_verified: false,
            payment_on_file: false,
        }
    }

    /// Advance to `target` if all prerequisites are met.
    /// Refuses skipping ahead — e.g. can't go to `Ready` until
    /// email is verified AND payment is on file for paid tiers.
    pub fn advance_to(&mut self, target: OnboardingStep) -> Result<(), CommerceError> {
        match target {
            OnboardingStep::Signup => {}
            OnboardingStep::PlanPick => {
                if self.current < OnboardingStep::Signup {
                    return Err(CommerceError::OnboardingOutOfOrder {
                        from: self.current,
                        to: target,
                    });
                }
            }
            OnboardingStep::Payment => {
                if self.current < OnboardingStep::PlanPick {
                    return Err(CommerceError::OnboardingOutOfOrder {
                        from: self.current,
                        to: target,
                    });
                }
                if self.tier == PlanTier::Free {
                    // Free tier skips Payment.
                    return Err(CommerceError::OnboardingOutOfOrder {
                        from: self.current,
                        to: target,
                    });
                }
            }
            OnboardingStep::Verify => {
                if self.current < OnboardingStep::PlanPick {
                    return Err(CommerceError::OnboardingOutOfOrder {
                        from: self.current,
                        to: target,
                    });
                }
                if self.tier != PlanTier::Free && !self.payment_on_file {
                    return Err(CommerceError::OnboardingMissingPrereq(
                        "payment_on_file".into(),
                    ));
                }
            }
            OnboardingStep::Ready => {
                if !self.email_verified {
                    return Err(CommerceError::OnboardingMissingPrereq(
                        "email_verified".into(),
                    ));
                }
                if self.tier != PlanTier::Free && !self.payment_on_file {
                    return Err(CommerceError::OnboardingMissingPrereq(
                        "payment_on_file".into(),
                    ));
                }
            }
        }
        self.current = target;
        Ok(())
    }
}

// ============================================================
// ADMIN PERMISSIONS
// ============================================================

/// Closed enum of admin roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdminRole {
    /// Account owner — full control + billing.
    Owner,
    /// Admin — full control except billing.
    Admin,
    /// Editor — can publish + edit content.
    Editor,
    /// Viewer — read-only.
    Viewer,
}

impl AdminRole {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Editor => "editor",
            Self::Viewer => "viewer",
        }
    }

    /// Permissions granted to this role.
    pub fn permissions(&self) -> &'static [AdminPermission] {
        match self {
            Self::Owner => &[
                AdminPermission::ManageBilling,
                AdminPermission::ManageMembers,
                AdminPermission::ManageDeployTargets,
                AdminPermission::PublishContent,
                AdminPermission::EditContent,
                AdminPermission::ViewAudit,
            ],
            Self::Admin => &[
                AdminPermission::ManageMembers,
                AdminPermission::ManageDeployTargets,
                AdminPermission::PublishContent,
                AdminPermission::EditContent,
                AdminPermission::ViewAudit,
            ],
            Self::Editor => &[
                AdminPermission::PublishContent,
                AdminPermission::EditContent,
            ],
            Self::Viewer => &[],
        }
    }

    /// Whether this role has the given permission.
    pub fn has(&self, perm: AdminPermission) -> bool {
        self.permissions().contains(&perm)
    }
}

/// Closed enum of admin permissions. Every privileged action
/// gates on one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdminPermission {
    /// View + modify billing settings.
    ManageBilling,
    /// Invite + remove admin members.
    ManageMembers,
    /// Add / remove deploy targets.
    ManageDeployTargets,
    /// Publish a page (move from Draft/Reviewed → Published).
    PublishContent,
    /// Create / edit page content.
    EditContent,
    /// Export the immutable audit log.
    ViewAudit,
}

impl AdminPermission {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::ManageBilling => "manage-billing",
            Self::ManageMembers => "manage-members",
            Self::ManageDeployTargets => "manage-deploy-targets",
            Self::PublishContent => "publish-content",
            Self::EditContent => "edit-content",
            Self::ViewAudit => "view-audit",
        }
    }
}

// ============================================================
// ERRORS
// ============================================================

/// Typed errors at the commerce boundary.
#[derive(Debug, thiserror::Error)]
pub enum CommerceError {
    /// Stripe id failed shape validation.
    #[error("invalid stripe id: {0}")]
    InvalidStripeId(String),
    /// Plan catalog missing an expected tier.
    #[error("plan catalog missing tier: {0:?}")]
    CatalogMissingTier(PlanTier),
    /// Onboarding state-machine transition skipped ahead.
    #[error("onboarding skip refused: {from:?} → {to:?}")]
    OnboardingOutOfOrder {
        /// Current step.
        from: OnboardingStep,
        /// Target step that was refused.
        to: OnboardingStep,
    },
    /// Onboarding step refused because a prerequisite (email
    /// verification, payment method) is missing.
    #[error("onboarding missing prereq: {0}")]
    OnboardingMissingPrereq(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn mk_subscription(tier: PlanTier, status: SubscriptionStatus) -> Subscription {
        Subscription {
            customer: StripeCustomerId::parse("cus_test12345").unwrap(),
            subscription: StripeSubscriptionId::parse("sub_test12345").unwrap(),
            tier,
            status,
            current_period_end: datetime!(2026-06-18 12:00:00 UTC),
            cancel_at_period_end: false,
        }
    }

    #[test]
    fn plan_tier_slugs_distinct() {
        let mut seen = std::collections::HashSet::new();
        for t in PlanTier::ALL {
            assert!(seen.insert(t.slug()));
        }
    }

    #[test]
    fn stripe_id_validates_prefix() {
        assert!(StripeCustomerId::parse("cus_abc123").is_ok());
        assert!(StripeCustomerId::parse("sub_abc123").is_err());
        assert!(StripeCustomerId::parse("").is_err());
        assert!(StripeCustomerId::parse("cus_").is_err()); // too short

        assert!(StripeSubscriptionId::parse("sub_abc123").is_ok());
        assert!(StripeSubscriptionId::parse("cus_abc123").is_err());
    }

    #[test]
    fn subscription_status_grants_paid_features_for_active_and_trialing() {
        assert!(SubscriptionStatus::Active.grants_paid_features());
        assert!(SubscriptionStatus::Trialing.grants_paid_features());
        assert!(!SubscriptionStatus::PastDue.grants_paid_features());
        assert!(!SubscriptionStatus::Canceled.grants_paid_features());
        assert!(!SubscriptionStatus::Unpaid.grants_paid_features());
        assert!(!SubscriptionStatus::Incomplete.grants_paid_features());
    }

    #[test]
    fn effective_tier_downgrades_when_subscription_unhealthy() {
        let s = mk_subscription(PlanTier::Pro, SubscriptionStatus::Active);
        assert_eq!(s.effective_tier(), PlanTier::Pro);

        let s = mk_subscription(PlanTier::Pro, SubscriptionStatus::PastDue);
        assert_eq!(s.effective_tier(), PlanTier::Free);

        let s = mk_subscription(PlanTier::Pro, SubscriptionStatus::Trialing);
        assert_eq!(s.effective_tier(), PlanTier::Pro);
    }

    #[test]
    fn onboarding_advances_in_order_through_free_path() {
        let mut s = OnboardingState::fresh();
        assert_eq!(s.current, OnboardingStep::Signup);
        s.advance_to(OnboardingStep::PlanPick).unwrap();
        s.tier = PlanTier::Free;
        // Free skips Payment.
        s.email_verified = true;
        s.advance_to(OnboardingStep::Verify).unwrap();
        s.advance_to(OnboardingStep::Ready).unwrap();
    }

    #[test]
    fn onboarding_paid_path_requires_payment() {
        let mut s = OnboardingState::fresh();
        s.advance_to(OnboardingStep::PlanPick).unwrap();
        s.tier = PlanTier::Pro;
        s.advance_to(OnboardingStep::Payment).unwrap();
        s.payment_on_file = true;
        s.email_verified = true;
        s.advance_to(OnboardingStep::Verify).unwrap();
        s.advance_to(OnboardingStep::Ready).unwrap();
    }

    #[test]
    fn onboarding_refuses_skip_to_ready_without_email() {
        let mut s = OnboardingState::fresh();
        s.advance_to(OnboardingStep::PlanPick).unwrap();
        s.tier = PlanTier::Free;
        let err = s.advance_to(OnboardingStep::Ready).unwrap_err();
        assert!(matches!(err, CommerceError::OnboardingMissingPrereq(_)));
    }

    #[test]
    fn onboarding_refuses_payment_for_free_tier() {
        let mut s = OnboardingState::fresh();
        s.advance_to(OnboardingStep::PlanPick).unwrap();
        s.tier = PlanTier::Free;
        let err = s.advance_to(OnboardingStep::Payment).unwrap_err();
        assert!(matches!(err, CommerceError::OnboardingOutOfOrder { .. }));
    }

    #[test]
    fn admin_role_permissions_layered() {
        // Owner has everything.
        assert!(AdminRole::Owner.has(AdminPermission::ManageBilling));
        assert!(AdminRole::Owner.has(AdminPermission::EditContent));

        // Admin loses billing but keeps the rest.
        assert!(!AdminRole::Admin.has(AdminPermission::ManageBilling));
        assert!(AdminRole::Admin.has(AdminPermission::ManageMembers));
        assert!(AdminRole::Admin.has(AdminPermission::PublishContent));

        // Editor only content.
        assert!(!AdminRole::Editor.has(AdminPermission::ManageMembers));
        assert!(AdminRole::Editor.has(AdminPermission::EditContent));
        assert!(AdminRole::Editor.has(AdminPermission::PublishContent));

        // Viewer nothing.
        assert!(!AdminRole::Viewer.has(AdminPermission::EditContent));
        assert!(AdminRole::Viewer.permissions().is_empty());
    }

    #[test]
    fn plan_catalog_verify_complete_detects_missing_tier() {
        let catalog = PlanCatalog {
            plans: vec![PlanEntry {
                tier: PlanTier::Pro,
                name: "Pro".into(),
                tagline: "x".into(),
                monthly_price_cents: 4900,
                stripe_price_id: "price_pro".into(),
                features: vec![],
                limits: PlanLimits {
                    max_sites: 10,
                    max_egress_bytes_month: 0,
                    max_admin_seats: 5,
                    max_storage_bytes: 0,
                },
            }],
        };
        let r = catalog.verify_complete(&[PlanTier::Pro, PlanTier::Enterprise]);
        assert!(matches!(
            r,
            Err(CommerceError::CatalogMissingTier(PlanTier::Enterprise))
        ));
    }

    #[test]
    fn subscription_serde_round_trips() {
        let s = mk_subscription(PlanTier::Pro, SubscriptionStatus::Active);
        let json = serde_json::to_string(&s).unwrap();
        let back: Subscription = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn subscription_rejects_unknown_field() {
        let bad = r#"{"customer":"cus_x","subscription":"sub_x","tier":"pro","status":"active","current-period-end":"2026-06-18T12:00:00Z","ahem":1}"#;
        let r: Result<Subscription, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    #[test]
    fn onboarding_step_next_chains_correctly() {
        assert_eq!(
            OnboardingStep::Signup.next(),
            Some(OnboardingStep::PlanPick)
        );
        assert_eq!(
            OnboardingStep::PlanPick.next(),
            Some(OnboardingStep::Payment)
        );
        assert_eq!(OnboardingStep::Payment.next(), Some(OnboardingStep::Verify));
        assert_eq!(OnboardingStep::Verify.next(), Some(OnboardingStep::Ready));
        assert_eq!(OnboardingStep::Ready.next(), None);
    }

    #[test]
    fn admin_permission_slugs_distinct() {
        let perms = [
            AdminPermission::ManageBilling,
            AdminPermission::ManageMembers,
            AdminPermission::ManageDeployTargets,
            AdminPermission::PublishContent,
            AdminPermission::EditContent,
            AdminPermission::ViewAudit,
        ];
        let mut seen = std::collections::HashSet::new();
        for p in perms {
            assert!(seen.insert(p.slug()));
        }
    }

    // T97: slug-vs-serde-wire regression guard.
    #[test]
    fn slug_matches_serde_wire_across_all_enums() {
        for v in [
            PlanTier::Free,
            PlanTier::Starter,
            PlanTier::Pro,
            PlanTier::Enterprise,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            PlanFeature::CustomDomain,
            PlanFeature::MultiRegion,
            PlanFeature::AltNetworkTargets,
            PlanFeature::Sso,
            PlanFeature::AuditExport,
            PlanFeature::Sla,
            PlanFeature::DataIsolatedTenancy,
            PlanFeature::FullyIsolatedTenancy,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            SubscriptionStatus::Trialing,
            SubscriptionStatus::Active,
            SubscriptionStatus::PastDue,
            SubscriptionStatus::Canceled,
            SubscriptionStatus::Unpaid,
            SubscriptionStatus::Incomplete,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            OnboardingStep::Signup,
            OnboardingStep::PlanPick,
            OnboardingStep::Payment,
            OnboardingStep::Verify,
            OnboardingStep::Ready,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            AdminRole::Owner,
            AdminRole::Admin,
            AdminRole::Editor,
            AdminRole::Viewer,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            AdminPermission::ManageBilling,
            AdminPermission::ManageMembers,
            AdminPermission::ManageDeployTargets,
            AdminPermission::PublishContent,
            AdminPermission::EditContent,
            AdminPermission::ViewAudit,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
    }
}
