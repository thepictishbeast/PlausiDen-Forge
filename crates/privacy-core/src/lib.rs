//! `privacy-core` — typed privacy-by-design contract.
//!
//! Per `PLATFORM_ROADMAP.md` §10 + `super_society_tech_stack`,
//! every PlausiDen tenant exposes the legally-required privacy
//! controls — DSAR request lifecycle, retention timers, cookie
//! consent classification — as a typed surface, not a free-form
//! "privacy page" string field. This crate defines:
//!
//! * [`DsarRequestKind`] — GDPR Art. 15 (Access) / 16 (Rectify) /
//!   17 (Erase) / 18 (Restrict) / 20 (Portability) / 21 (Object)
//!   + CCPA §1798.100 (Right to Know) / §1798.105 (Delete) /
//!   §1798.120 (Opt-out of sale).
//! * [`DsarRequestState`] — ingest → verify-identity → process →
//!   fulfill | reject, with statutory clocks.
//! * [`DataCategory`] — taxonomy used to scope DSAR responses +
//!   retention timers (Account, Content, Telemetry, Payment,
//!   SupportTicket, etc.).
//! * [`RetentionPolicy`] — per-DataCategory retention duration
//!   + lawful basis. `policy_for()` is the operator's typed
//!   surface; the runtime enforces deletion when the timer
//!   expires.
//! * [`ConsentScope`] — ePrivacy Directive §5(3) cookie classes
//!   (StrictlyNecessary / Preferences / Statistics / Marketing)
//!   with legal-by-default policy: only StrictlyNecessary is
//!   active without explicit opt-in.
//!
//! ### Out of scope for this crate
//!
//! No storage. No UI. No email notification. No identity
//! verification. Those plug in downstream via the trait edges
//! at the bottom of the module.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// DSAR request kind. Mapping to statutory rights:
///
/// | Variant       | GDPR Art. | CCPA §              |
/// |---------------|-----------|---------------------|
/// | Access        | 15        | 1798.100 / 1798.110 |
/// | Rectify       | 16        | 1798.106            |
/// | Erase         | 17        | 1798.105            |
/// | Restrict      | 18        | —                   |
/// | Portability   | 20        | 1798.130            |
/// | Object        | 21        | 1798.120 (opt-out)  |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DsarRequestKind {
    /// Subject access — full export of every data category the
    /// tenant holds about the data subject.
    Access,
    /// Rectification — correct inaccurate personal data.
    Rectify,
    /// Erasure / "right to be forgotten" — delete personal data
    /// subject to legal-hold exceptions.
    Erase,
    /// Restriction of processing — pause processing pending a
    /// dispute over accuracy or lawfulness.
    Restrict,
    /// Data portability — machine-readable, interoperable export.
    Portability,
    /// Object / opt-out — typically for marketing, profiling,
    /// or sale-of-data.
    Object,
}

impl DsarRequestKind {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Access => "access",
            Self::Rectify => "rectify",
            Self::Erase => "erase",
            Self::Restrict => "restrict",
            Self::Portability => "portability",
            Self::Object => "object",
        }
    }

    /// Statutory fulfillment clock, in days, from the latest of
    /// GDPR (30 days, extendable to 90) / CCPA (45 days,
    /// extendable to 90). Used to derive the deadline from the
    /// request's `received_at`.
    ///
    /// The base figure is the strictest applicable default
    /// (30 days). Operators in lower-bar regions may set their
    /// own SLA below this number; they cannot extend it above
    /// the statute without invoking the complexity-extension
    /// procedure (out of scope for this crate).
    pub fn baseline_sla_days(&self) -> u32 {
        // GDPR Art. 12(3): "without undue delay and in any event
        // within one month" → 30 days. CCPA: 45 days. Strictest
        // wins.
        30
    }
}

/// DSAR request lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DsarRequestState {
    /// Received but identity not yet verified.
    Received,
    /// Identity verification in progress (email confirm,
    /// account-login challenge, etc.).
    VerifyingIdentity,
    /// Identity verified; data collection in progress.
    Processing,
    /// Request fulfilled (export delivered, deletion completed,
    /// etc.).
    Fulfilled,
    /// Request rejected (identity not verified, legal-hold
    /// exception applies, etc.).
    Rejected,
}

impl DsarRequestState {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::VerifyingIdentity => "verifying-identity",
            Self::Processing => "processing",
            Self::Fulfilled => "fulfilled",
            Self::Rejected => "rejected",
        }
    }

    /// Whether the state is terminal (no further transitions).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Fulfilled | Self::Rejected)
    }
}

/// DSAR request record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DsarRequest {
    /// Operator-assigned stable request id.
    pub id: String,
    /// Request kind (Access / Erase / etc.).
    pub kind: DsarRequestKind,
    /// Subject identifier (opaque — typically email or
    /// account-id; this crate doesn't interpret it).
    pub subject_id: String,
    /// Lifecycle state.
    pub state: DsarRequestState,
    /// Wall-clock of original request receipt.
    pub received_at: time::OffsetDateTime,
    /// Statutory deadline = received_at + baseline_sla_days.
    /// Pre-computed at construction time so it's an
    /// observable field, not a derived expression.
    pub deadline: time::OffsetDateTime,
}

impl DsarRequest {
    /// Build a request, pre-computing the deadline from
    /// `kind.baseline_sla_days()`.
    pub fn new(
        id: impl Into<String>,
        kind: DsarRequestKind,
        subject_id: impl Into<String>,
        received_at: time::OffsetDateTime,
    ) -> Self {
        let deadline = received_at + time::Duration::days(i64::from(kind.baseline_sla_days()));
        Self {
            id: id.into(),
            kind,
            subject_id: subject_id.into(),
            state: DsarRequestState::Received,
            received_at,
            deadline,
        }
    }

    /// Whether the request is past its statutory deadline at
    /// the supplied "now".
    pub fn is_overdue(&self, now: time::OffsetDateTime) -> bool {
        !self.state.is_terminal() && now > self.deadline
    }
}

/// Closed taxonomy of data categories. Used to scope:
///   * DSAR exports (Access / Portability return the categories
///     the subject's data exists in)
///   * Retention timers (each category has its own
///     [`RetentionPolicy`])
///   * Erasure scope (Erase + Restrict run per-category)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DataCategory {
    /// Account identity (email, name, login id).
    Account,
    /// User-authored content (pages, posts, drafts, comments).
    Content,
    /// Audit log + admin-action history.
    AuditLog,
    /// Operational telemetry (web logs, error reports,
    /// performance traces).
    Telemetry,
    /// Payment + billing records.
    Payment,
    /// Support ticket history.
    SupportTicket,
    /// Marketing engagement (open / click events, list
    /// memberships).
    Marketing,
    /// Authentication artifacts (session tokens, 2FA recovery
    /// codes, OAuth grants).
    Auth,
    /// Backup snapshots.
    Backup,
}

impl DataCategory {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Account => "account",
            Self::Content => "content",
            Self::AuditLog => "audit-log",
            Self::Telemetry => "telemetry",
            Self::Payment => "payment",
            Self::SupportTicket => "support-ticket",
            Self::Marketing => "marketing",
            Self::Auth => "auth",
            Self::Backup => "backup",
        }
    }
}

/// Lawful basis under GDPR Art. 6 for processing a given
/// [`DataCategory`]. Drives which DSAR rights apply (Erase
/// doesn't apply to data held under legal obligation, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LawfulBasis {
    /// Art. 6(1)(a) — explicit consent.
    Consent,
    /// Art. 6(1)(b) — performance of a contract.
    Contract,
    /// Art. 6(1)(c) — compliance with a legal obligation.
    LegalObligation,
    /// Art. 6(1)(d) — vital interests.
    VitalInterests,
    /// Art. 6(1)(e) — public task.
    PublicTask,
    /// Art. 6(1)(f) — legitimate interests.
    LegitimateInterests,
}

impl LawfulBasis {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Consent => "consent",
            Self::Contract => "contract",
            Self::LegalObligation => "legal-obligation",
            Self::VitalInterests => "vital-interests",
            Self::PublicTask => "public-task",
            Self::LegitimateInterests => "legitimate-interests",
        }
    }

    /// Whether an Erase request can override this basis.
    /// LegalObligation refuses Erase (the tenant is legally
    /// required to retain — e.g. tax records under retention
    /// law).
    pub fn permits_erasure(&self) -> bool {
        !matches!(self, Self::LegalObligation)
    }
}

/// Retention policy for one data category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RetentionPolicy {
    /// Which category this policy governs.
    pub category: DataCategory,
    /// Retention duration in days from the data point's
    /// creation. The runtime enforces deletion at expiry.
    pub retention_days: u32,
    /// Lawful basis under GDPR Art. 6.
    pub basis: LawfulBasis,
    /// Operator-supplied notes (audit-trail context, regulation
    /// reference). Not interpreted by this crate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl RetentionPolicy {
    /// Whether this policy permits erasure on DSAR-Erase.
    pub fn permits_erasure(&self) -> bool {
        self.basis.permits_erasure()
    }
}

/// Cookie / tracker consent scope per ePrivacy Directive §5(3)
/// + GDPR Recital 32 + ICO 2019 guidance.
///
/// Legal-by-default policy: [`ConsentScope::StrictlyNecessary`]
/// is the only scope active without explicit opt-in. Every
/// other scope requires unambiguous, specific, informed,
/// freely-given consent — see [`Consent::default_state`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConsentScope {
    /// Strictly necessary cookies (session id, CSRF token,
    /// load-balancer pin). Allowed without consent under
    /// ePrivacy §5(3) exception.
    StrictlyNecessary,
    /// User preferences (theme, language). Requires opt-in.
    Preferences,
    /// Statistics / analytics. Requires opt-in.
    Statistics,
    /// Marketing / behavioural advertising. Requires opt-in.
    Marketing,
}

impl ConsentScope {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::StrictlyNecessary => "strictly-necessary",
            Self::Preferences => "preferences",
            Self::Statistics => "statistics",
            Self::Marketing => "marketing",
        }
    }

    /// The default state of this scope before any consent
    /// interaction: only StrictlyNecessary is active.
    pub fn default_state(&self) -> ConsentState {
        match self {
            Self::StrictlyNecessary => ConsentState::Granted,
            _ => ConsentState::Pending,
        }
    }

    /// Whether the operator legally needs opt-in for this scope.
    pub fn requires_opt_in(&self) -> bool {
        !matches!(self, Self::StrictlyNecessary)
    }
}

/// Per-scope consent decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConsentState {
    /// Subject has not yet been asked / has not responded.
    /// Treated as denial — no opt-in defaults are legal under
    /// GDPR.
    Pending,
    /// Subject explicitly granted.
    Granted,
    /// Subject explicitly denied.
    Denied,
    /// Subject revoked a previously-granted consent.
    Revoked,
}

impl ConsentState {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Granted => "granted",
            Self::Denied => "denied",
            Self::Revoked => "revoked",
        }
    }

    /// Whether the runtime should treat this scope as active
    /// (i.e. permit the corresponding cookies / trackers).
    /// Only [`Self::Granted`] is active; Pending / Denied /
    /// Revoked all refuse.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Granted)
    }
}

/// One consent record, scoped + dated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ConsentRecord {
    /// Subject identifier (opaque).
    pub subject_id: String,
    /// Which scope this record covers.
    pub scope: ConsentScope,
    /// Decision.
    pub state: ConsentState,
    /// Wall-clock of the most recent state change.
    pub updated_at: time::OffsetDateTime,
}

impl ConsentRecord {
    /// Construct the legal-by-default initial record for a
    /// subject — only StrictlyNecessary is granted.
    pub fn default_for(
        subject_id: impl Into<String>,
        scope: ConsentScope,
        now: time::OffsetDateTime,
    ) -> Self {
        Self {
            subject_id: subject_id.into(),
            scope,
            state: scope.default_state(),
            updated_at: now,
        }
    }
}

/// Trait for the operator-side DSAR fulfillment backend. The
/// crate's contract is asynchronous over the network /
/// database / file-system in real impls; this signature is the
/// sync seam so the contract is fixed without binding a runtime.
pub trait DsarFulfiller {
    /// Resolve a DSAR request against the operator's storage,
    /// producing the appropriate side-effects (export,
    /// deletion, restriction flag, etc.).
    fn fulfill(&self, request: &DsarRequest) -> Result<DsarOutcome, PrivacyError>;
}

/// Outcome of a successful DSAR fulfillment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DsarOutcome {
    /// Categories affected by the fulfillment.
    pub categories: Vec<DataCategory>,
    /// Optional opaque export-bundle identifier (e.g. blob
    /// reference); only populated for Access / Portability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_ref: Option<String>,
    /// When fulfillment completed.
    pub completed_at: time::OffsetDateTime,
}

/// Typed errors at the privacy boundary.
#[derive(Debug, thiserror::Error)]
pub enum PrivacyError {
    /// Subject identity could not be verified.
    #[error("identity verification failed: {0}")]
    IdentityUnverified(String),
    /// Erase requested but a legal-hold exception applies.
    #[error("erasure refused: legal hold on category {0:?}")]
    LegalHoldRefusesErasure(DataCategory),
    /// Backend storage / IO error.
    #[error("backend: {0}")]
    Backend(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn dsar_baseline_sla_is_30_days() {
        for k in [
            DsarRequestKind::Access,
            DsarRequestKind::Rectify,
            DsarRequestKind::Erase,
            DsarRequestKind::Restrict,
            DsarRequestKind::Portability,
            DsarRequestKind::Object,
        ] {
            assert_eq!(k.baseline_sla_days(), 30);
        }
    }

    #[test]
    fn dsar_request_deadline_is_received_plus_sla() {
        let now = datetime!(2026-05-18 00:00:00 UTC);
        let r = DsarRequest::new("r1", DsarRequestKind::Access, "alice", now);
        assert_eq!(r.deadline, now + time::Duration::days(30));
    }

    #[test]
    fn dsar_overdue_only_when_non_terminal_and_past_deadline() {
        let now = datetime!(2026-05-18 00:00:00 UTC);
        let mut r = DsarRequest::new("r1", DsarRequestKind::Erase, "alice", now);
        assert!(!r.is_overdue(now + time::Duration::days(29)));
        assert!(r.is_overdue(now + time::Duration::days(31)));
        r.state = DsarRequestState::Fulfilled;
        // Terminal state never reports overdue.
        assert!(!r.is_overdue(now + time::Duration::days(99)));
    }

    #[test]
    fn dsar_states_terminal_set() {
        assert!(DsarRequestState::Fulfilled.is_terminal());
        assert!(DsarRequestState::Rejected.is_terminal());
        assert!(!DsarRequestState::Received.is_terminal());
        assert!(!DsarRequestState::VerifyingIdentity.is_terminal());
        assert!(!DsarRequestState::Processing.is_terminal());
    }

    #[test]
    fn legal_obligation_refuses_erasure() {
        assert!(!LawfulBasis::LegalObligation.permits_erasure());
        for b in [
            LawfulBasis::Consent,
            LawfulBasis::Contract,
            LawfulBasis::VitalInterests,
            LawfulBasis::PublicTask,
            LawfulBasis::LegitimateInterests,
        ] {
            assert!(b.permits_erasure());
        }
    }

    #[test]
    fn retention_policy_inherits_basis_erasure_permission() {
        let p = RetentionPolicy {
            category: DataCategory::Payment,
            retention_days: 365 * 7,
            basis: LawfulBasis::LegalObligation,
            note: Some("tax-retention".into()),
        };
        assert!(!p.permits_erasure());
    }

    #[test]
    fn consent_strictly_necessary_is_granted_by_default() {
        assert_eq!(
            ConsentScope::StrictlyNecessary.default_state(),
            ConsentState::Granted
        );
        assert!(!ConsentScope::StrictlyNecessary.requires_opt_in());
    }

    #[test]
    fn consent_other_scopes_default_to_pending_and_require_opt_in() {
        for s in [
            ConsentScope::Preferences,
            ConsentScope::Statistics,
            ConsentScope::Marketing,
        ] {
            assert_eq!(s.default_state(), ConsentState::Pending);
            assert!(s.requires_opt_in());
        }
    }

    #[test]
    fn consent_state_only_granted_is_active() {
        assert!(ConsentState::Granted.is_active());
        assert!(!ConsentState::Pending.is_active());
        assert!(!ConsentState::Denied.is_active());
        assert!(!ConsentState::Revoked.is_active());
    }

    #[test]
    fn consent_record_default_for_marketing_is_pending() {
        let now = datetime!(2026-05-18 00:00:00 UTC);
        let r = ConsentRecord::default_for("alice", ConsentScope::Marketing, now);
        assert_eq!(r.state, ConsentState::Pending);
        assert!(!r.state.is_active());
    }

    #[test]
    fn dsar_request_serde_round_trip() {
        let now = datetime!(2026-05-18 00:00:00 UTC);
        let r = DsarRequest::new("r1", DsarRequestKind::Portability, "alice", now);
        let s = serde_json::to_string(&r).unwrap();
        let back: DsarRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn dsar_request_rejects_unknown_field() {
        let bad = r#"{"id":"r1","kind":"access","subject-id":"a","state":"received","received-at":"2026-05-18T00:00:00Z","deadline":"2026-06-17T00:00:00Z","ahem":1}"#;
        let r: Result<DsarRequest, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    #[test]
    fn retention_policy_rejects_unknown_field() {
        let bad =
            r#"{"category":"payment","retention-days":2555,"basis":"legal-obligation","ahem":1}"#;
        let r: Result<RetentionPolicy, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    #[test]
    fn slugs_distinct_across_each_enum() {
        let kinds = [
            DsarRequestKind::Access,
            DsarRequestKind::Rectify,
            DsarRequestKind::Erase,
            DsarRequestKind::Restrict,
            DsarRequestKind::Portability,
            DsarRequestKind::Object,
        ];
        let mut s = std::collections::HashSet::new();
        for k in kinds {
            assert!(s.insert(k.slug()));
        }

        let cats = [
            DataCategory::Account,
            DataCategory::Content,
            DataCategory::AuditLog,
            DataCategory::Telemetry,
            DataCategory::Payment,
            DataCategory::SupportTicket,
            DataCategory::Marketing,
            DataCategory::Auth,
            DataCategory::Backup,
        ];
        let mut s2 = std::collections::HashSet::new();
        for c in cats {
            assert!(s2.insert(c.slug()));
        }

        let scopes = [
            ConsentScope::StrictlyNecessary,
            ConsentScope::Preferences,
            ConsentScope::Statistics,
            ConsentScope::Marketing,
        ];
        let mut s3 = std::collections::HashSet::new();
        for sc in scopes {
            assert!(s3.insert(sc.slug()));
        }
    }

    // Regression-guard for the slug-vs-serde-wire divergence
    // bug class (T97 audit). See trust-safety-core for the
    // full explanation. Adding a variant requires extending
    // the slices below.
    #[test]
    fn slug_matches_serde_wire_across_all_enums() {
        for v in [
            DsarRequestKind::Access,
            DsarRequestKind::Rectify,
            DsarRequestKind::Erase,
            DsarRequestKind::Restrict,
            DsarRequestKind::Portability,
            DsarRequestKind::Object,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            DsarRequestState::Received,
            DsarRequestState::VerifyingIdentity,
            DsarRequestState::Processing,
            DsarRequestState::Fulfilled,
            DsarRequestState::Rejected,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            DataCategory::Account,
            DataCategory::Content,
            DataCategory::AuditLog,
            DataCategory::Telemetry,
            DataCategory::Payment,
            DataCategory::SupportTicket,
            DataCategory::Marketing,
            DataCategory::Auth,
            DataCategory::Backup,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            LawfulBasis::Consent,
            LawfulBasis::Contract,
            LawfulBasis::LegalObligation,
            LawfulBasis::VitalInterests,
            LawfulBasis::PublicTask,
            LawfulBasis::LegitimateInterests,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            ConsentScope::StrictlyNecessary,
            ConsentScope::Preferences,
            ConsentScope::Statistics,
            ConsentScope::Marketing,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            ConsentState::Pending,
            ConsentState::Granted,
            ConsentState::Denied,
            ConsentState::Revoked,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
    }
}
