//! `email-core` — typed email contract.
//!
//! Per `PLATFORM_ROADMAP.md` §17, every PlausiDen tenant gets:
//!
//! * Transactional email (signup, password reset, receipt)
//! * Marketing email (newsletter, campaign) with built-in
//!   list-unsubscribe + segmentation
//! * DMARC alignment monitoring (RFC 7489) — SPF (RFC 7208) +
//!   DKIM (RFC 6376) per-domain telemetry
//! * BIMI brand indicator (RFC drafts; Verified Mark Certificate
//!   from a CA the operator manages)
//! * RFC 8058 one-click list-unsubscribe — mandatory on every
//!   marketing message, enforced by [`OutgoingMessage::validate`]
//!
//! ### Why typed
//!
//! Email is the canonical place a "tenant" accidentally sends a
//! marketing blast through the transactional pipeline (no
//! list-unsubscribe, no proper From, throttle skipped). Closing
//! [`MessageKind`] + per-kind invariants makes that impossible
//! at the type-checker.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Message classification. Drives invariants enforced by
/// [`OutgoingMessage::validate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MessageKind {
    /// Transactional — sent in response to a user action
    /// (signup, password reset, order confirmation). No
    /// list-unsubscribe required.
    Transactional,
    /// Marketing — bulk send, opt-in audience. RFC 8058
    /// one-click list-unsubscribe MANDATORY. Higher throttle
    /// limit; campaign analytics tracked.
    Marketing,
}

impl MessageKind {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Transactional => "transactional",
            Self::Marketing => "marketing",
        }
    }

    /// Whether RFC 8058 list-unsubscribe is mandatory for this
    /// kind. Marketing always; transactional never.
    pub fn requires_list_unsubscribe(&self) -> bool {
        matches!(self, Self::Marketing)
    }
}

/// DMARC (RFC 7489) alignment + verdict for a single message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DmarcResult {
    /// SPF + DKIM aligned; passes domain policy.
    Pass,
    /// One or both failed alignment.
    Fail,
    /// No DMARC record published for the From domain.
    None,
    /// Quarantined per policy (p=quarantine).
    Quarantine,
    /// Rejected per policy (p=reject).
    Reject,
}

impl DmarcResult {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::None => "none",
            Self::Quarantine => "quarantine",
            Self::Reject => "reject",
        }
    }
}

/// Per-channel auth result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthChannelResult {
    /// Channel passed.
    Pass,
    /// Channel failed.
    Fail,
    /// Channel not configured / no record.
    None,
}

impl AuthChannelResult {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::None => "none",
        }
    }
}

/// Authentication state for one outgoing message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AuthState {
    /// SPF (RFC 7208) verdict.
    pub spf: AuthChannelResult,
    /// DKIM (RFC 6376) verdict.
    pub dkim: AuthChannelResult,
    /// DMARC (RFC 7489) verdict.
    pub dmarc: DmarcResult,
    /// Whether SPF aligned with the From header domain (one of
    /// the two requirements for a DMARC pass).
    pub spf_aligned: bool,
    /// Whether DKIM signing domain aligned with the From header
    /// domain (the other DMARC requirement).
    pub dkim_aligned: bool,
}

impl AuthState {
    /// Whether the message would pass DMARC strictly.
    pub fn dmarc_aligned(&self) -> bool {
        (self.spf == AuthChannelResult::Pass && self.spf_aligned)
            || (self.dkim == AuthChannelResult::Pass && self.dkim_aligned)
    }
}

/// BIMI (Brand Indicators for Message Identification) reference.
/// Per draft RFC, BIMI requires a published BIMI record + a
/// VMC (Verified Mark Certificate) issued by a participating CA.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BimiReference {
    /// URL of the brand SVG (must be SVG Tiny PS).
    pub svg_url: String,
    /// URL of the VMC PEM (Verified Mark Certificate).
    /// Operator-managed. Optional in the BIMI draft (selector
    /// records can omit it for gmail-only deploys) but
    /// recommended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vmc_url: Option<String>,
}

/// Outgoing message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct OutgoingMessage {
    /// Stable message id (operator-defined; usually a ULID).
    pub id: String,
    /// Message kind.
    pub kind: MessageKind,
    /// From address (RFC 5322).
    pub from: String,
    /// To address(es).
    pub to: Vec<String>,
    /// Subject line.
    pub subject: String,
    /// RFC 8058 one-click list-unsubscribe URL. MUST be set for
    /// Marketing messages; transactional messages MAY include
    /// for parity but it's not enforced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_unsubscribe: Option<String>,
    /// Optional BIMI reference (operator-side branded sender).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bimi: Option<BimiReference>,
    /// Wall-clock when the message was queued.
    pub queued_at: time::OffsetDateTime,
}

impl OutgoingMessage {
    /// Validate the message against the typed invariants.
    ///   * from + subject non-empty
    ///   * at least one to
    ///   * Marketing messages MUST have list_unsubscribe set
    ///     to an https URL (RFC 8058 §2.1)
    pub fn validate(&self) -> Result<(), EmailError> {
        if self.from.trim().is_empty() {
            return Err(EmailError::Invalid("from empty".into()));
        }
        if self.subject.trim().is_empty() {
            return Err(EmailError::Invalid("subject empty".into()));
        }
        if self.to.is_empty() {
            return Err(EmailError::Invalid("to empty".into()));
        }
        if self.kind.requires_list_unsubscribe() {
            match &self.list_unsubscribe {
                None => {
                    return Err(EmailError::Invalid(
                        "marketing message requires RFC 8058 list-unsubscribe".into(),
                    ))
                }
                Some(u) if !u.starts_with("https://") => {
                    return Err(EmailError::Invalid("list-unsubscribe must be https".into()));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Bounce or complaint reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BounceReason {
    /// Hard bounce — recipient address invalid / mailbox not
    /// found. Suppress the address permanently.
    HardInvalidAddress,
    /// Hard bounce — recipient mailbox full / over quota.
    /// Permanently? No, but suppression is policy.
    HardMailboxFull,
    /// Soft bounce — transient (greylisted, deferred, retry).
    SoftTransient,
    /// Complaint — recipient marked the message as spam (FBL).
    Complaint,
    /// Reject — receiving mail server rejected (DMARC policy
    /// reject / IP reputation).
    Reject,
    /// Suppression — operator added the address to a
    /// suppression list (manual or automated).
    Suppressed,
}

impl BounceReason {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::HardInvalidAddress => "hard-invalid-address",
            Self::HardMailboxFull => "hard-mailbox-full",
            Self::SoftTransient => "soft-transient",
            Self::Complaint => "complaint",
            Self::Reject => "reject",
            Self::Suppressed => "suppressed",
        }
    }

    /// Whether the operator should suppress (never re-send to)
    /// the recipient address after this reason. Hard bounces +
    /// complaints + explicit suppression are sticky; soft
    /// bounces are not.
    pub fn should_suppress(&self) -> bool {
        matches!(
            self,
            Self::HardInvalidAddress | Self::HardMailboxFull | Self::Complaint | Self::Suppressed
        )
    }
}

/// One delivery outcome record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DeliveryOutcome {
    /// Message id.
    pub message_id: String,
    /// Recipient address.
    pub recipient: String,
    /// Whether delivery succeeded (None) or bounced (Some).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounce: Option<BounceReason>,
    /// Auth state at acceptance time.
    pub auth: AuthState,
    /// When the receiving server reported the outcome.
    pub reported_at: time::OffsetDateTime,
}

impl DeliveryOutcome {
    /// Whether the outcome was a successful delivery (no bounce).
    pub fn is_delivered(&self) -> bool {
        self.bounce.is_none()
    }
}

/// Typed errors at the email boundary.
#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    /// Message failed validation.
    #[error("invalid: {0}")]
    Invalid(String),
    /// Submission to MTA failed.
    #[error("submit: {0}")]
    Submit(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn auth_pass() -> AuthState {
        AuthState {
            spf: AuthChannelResult::Pass,
            dkim: AuthChannelResult::Pass,
            dmarc: DmarcResult::Pass,
            spf_aligned: true,
            dkim_aligned: true,
        }
    }

    fn marketing_msg() -> OutgoingMessage {
        OutgoingMessage {
            id: "m1".into(),
            kind: MessageKind::Marketing,
            from: "newsletter@example.com".into(),
            to: vec!["alice@example.com".into()],
            subject: "May newsletter".into(),
            list_unsubscribe: Some("https://example.com/unsub?id=abc".into()),
            bimi: None,
            queued_at: datetime!(2026-05-18 12:00:00 UTC),
        }
    }

    #[test]
    fn message_kind_slugs_distinct() {
        let ks = [MessageKind::Transactional, MessageKind::Marketing];
        let mut s = std::collections::HashSet::new();
        for k in ks {
            assert!(s.insert(k.slug()));
        }
    }

    #[test]
    fn only_marketing_requires_unsubscribe() {
        assert!(MessageKind::Marketing.requires_list_unsubscribe());
        assert!(!MessageKind::Transactional.requires_list_unsubscribe());
    }

    #[test]
    fn dmarc_result_slugs_distinct() {
        let rs = [
            DmarcResult::Pass,
            DmarcResult::Fail,
            DmarcResult::None,
            DmarcResult::Quarantine,
            DmarcResult::Reject,
        ];
        let mut s = std::collections::HashSet::new();
        for r in rs {
            assert!(s.insert(r.slug()));
        }
    }

    #[test]
    fn auth_dmarc_aligned_via_spf_or_dkim() {
        let mut a = auth_pass();
        a.spf_aligned = false;
        // Still aligned via DKIM.
        assert!(a.dmarc_aligned());

        a.dkim_aligned = false;
        // Now neither aligned.
        assert!(!a.dmarc_aligned());

        a.spf_aligned = true;
        // Now SPF-only aligned.
        assert!(a.dmarc_aligned());
    }

    #[test]
    fn auth_dmarc_aligned_requires_pass_not_just_alignment() {
        let mut a = auth_pass();
        a.spf = AuthChannelResult::Fail;
        a.dkim = AuthChannelResult::Fail;
        // Both channels failed even though they're aligned.
        assert!(!a.dmarc_aligned());
    }

    #[test]
    fn marketing_validates_with_https_unsubscribe() {
        assert!(marketing_msg().validate().is_ok());
    }

    #[test]
    fn marketing_rejects_missing_unsubscribe() {
        let mut m = marketing_msg();
        m.list_unsubscribe = None;
        assert!(m.validate().is_err());
    }

    #[test]
    fn marketing_rejects_http_unsubscribe() {
        let mut m = marketing_msg();
        m.list_unsubscribe = Some("http://example.com/unsub".into());
        assert!(m.validate().is_err());
    }

    #[test]
    fn transactional_allowed_without_unsubscribe() {
        let mut m = marketing_msg();
        m.kind = MessageKind::Transactional;
        m.list_unsubscribe = None;
        assert!(m.validate().is_ok());
    }

    #[test]
    fn message_rejects_empty_required_fields() {
        let mut m = marketing_msg();
        m.from = "".into();
        assert!(m.validate().is_err());

        let mut m2 = marketing_msg();
        m2.subject = "".into();
        assert!(m2.validate().is_err());

        let mut m3 = marketing_msg();
        m3.to = vec![];
        assert!(m3.validate().is_err());
    }

    #[test]
    fn bounce_suppression_set() {
        assert!(BounceReason::HardInvalidAddress.should_suppress());
        assert!(BounceReason::HardMailboxFull.should_suppress());
        assert!(BounceReason::Complaint.should_suppress());
        assert!(BounceReason::Suppressed.should_suppress());
        assert!(!BounceReason::SoftTransient.should_suppress());
        assert!(!BounceReason::Reject.should_suppress());
    }

    #[test]
    fn delivery_outcome_is_delivered_when_no_bounce() {
        let o = DeliveryOutcome {
            message_id: "m1".into(),
            recipient: "alice@example.com".into(),
            bounce: None,
            auth: auth_pass(),
            reported_at: datetime!(2026-05-18 12:01:00 UTC),
        };
        assert!(o.is_delivered());

        let o2 = DeliveryOutcome {
            bounce: Some(BounceReason::HardInvalidAddress),
            ..o
        };
        assert!(!o2.is_delivered());
    }

    #[test]
    fn message_serde_round_trip() {
        let m = marketing_msg();
        let j = serde_json::to_string(&m).unwrap();
        let back: OutgoingMessage = serde_json::from_str(&j).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn message_rejects_unknown_field() {
        let bad = r#"{"id":"x","kind":"transactional","from":"a","to":["b"],"subject":"s","queued-at":"2026-05-18T12:00:00Z","ahem":1}"#;
        let r: Result<OutgoingMessage, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }
}
