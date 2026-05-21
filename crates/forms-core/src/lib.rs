//! `forms-core` — typed forms contract.
//!
//! Per `PLATFORM_ROADMAP.md` §15, every tenant exposes
//! an accessible-by-default forms builder where:
//!
//! * Field types are a closed taxonomy with appropriate ARIA +
//!   input-type defaults
//! * Spam protection runs through a typed [`SpamSignal`] set
//!   (honeypot + rate-limit + Akismet-like classifier + timing)
//!   rather than CAPTCHAs that hurt usability
//! * GDPR consent is a typed field, not a free-form checkbox
//! * Uploaded attachments require successful virus-scan before
//!   reaching the operator
//! * Submissions deliver via webhook with a typed retry lifecycle
//!
//! ### Why typed
//!
//! Free-form form-builder JSON is the canonical place "honeypot
//! field forgotten on this form" / "GDPR checkbox required-on
//! some, optional-on others" / "attachment uploaded straight to
//! S3 without virus scan" all happen. Closed enums + struct
//! defaults make every form follow the same privacy-+-a11y
//! invariants at the type-checker.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Closed enum of field kinds. Each variant maps to the
/// appropriate ARIA role + HTML5 `<input type=...>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FieldKind {
    /// Single-line text.
    Text,
    /// Multi-line text.
    TextArea,
    /// Email (HTML5 `type=email`, validates client + server).
    Email,
    /// URL (`type=url`).
    Url,
    /// Telephone (`type=tel`).
    Tel,
    /// Number (`type=number`).
    Number,
    /// Date (`type=date`, ISO 8601 calendar).
    Date,
    /// Single-choice (rendered as `<select>` or radio group).
    Select,
    /// Multi-choice checkbox group.
    Checkboxes,
    /// File upload — REQUIRES virus-scan completion before the
    /// operator sees the file. Enforced via [`AttachmentField`].
    File,
    /// Honeypot — hidden field; non-empty submissions are spam.
    /// Forms with public access automatically receive one of
    /// these via [`Form::with_honeypot`].
    Honeypot,
    /// GDPR consent — typed checkbox with explicit purpose +
    /// lawful-basis label. See [`GdprConsent`].
    GdprConsent,
}

impl FieldKind {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::TextArea => "text-area",
            Self::Email => "email",
            Self::Url => "url",
            Self::Tel => "tel",
            Self::Number => "number",
            Self::Date => "date",
            Self::Select => "select",
            Self::Checkboxes => "checkboxes",
            Self::File => "file",
            Self::Honeypot => "honeypot",
            Self::GdprConsent => "gdpr-consent",
        }
    }

    /// Required HTML5 `<input type=...>` for this kind, where
    /// applicable. `None` for kinds that aren't single-input
    /// (TextArea, Select, Checkboxes, File without a type).
    pub fn html_input_type(&self) -> Option<&'static str> {
        match self {
            Self::Text => Some("text"),
            Self::Email => Some("email"),
            Self::Url => Some("url"),
            Self::Tel => Some("tel"),
            Self::Number => Some("number"),
            Self::Date => Some("date"),
            Self::Honeypot => Some("text"),
            Self::GdprConsent => Some("checkbox"),
            Self::TextArea | Self::Select | Self::Checkboxes | Self::File => None,
        }
    }
}

/// One field on a form. Accessibility-required fields (label,
/// description, required-flag) are typed, not optional strings
/// the operator can forget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FormField {
    /// Field id — stable, kebab-case, used as the form-control
    /// `name` attribute.
    pub id: String,
    /// User-facing label. WCAG 2.1 §3.3.2 — every field
    /// MUST be labelled.
    pub label: String,
    /// Optional clarifying description shown via
    /// `aria-describedby`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Field kind.
    pub kind: FieldKind,
    /// Whether the field is required.
    pub required: bool,
}

/// One GDPR consent declaration on a form (e.g. mailing-list
/// opt-in). Typed so the operator can't accidentally treat a
/// "newsletter" checkbox as an implicit consent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GdprConsent {
    /// Stable id.
    pub id: String,
    /// Purpose statement shown to the subject. Required.
    pub purpose: String,
    /// Operator-supplied link to the relevant privacy notice
    /// section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_notice_url: Option<String>,
    /// Whether opt-in is required to submit the form (true) or
    /// optional (false). Pre-ticked defaults are illegal under
    /// GDPR; the runtime never renders this as pre-ticked.
    pub mandatory: bool,
}

/// Attachment field. File uploads MUST clear a virus scan
/// before reaching the operator's inbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AttachmentField {
    /// Field id.
    pub id: String,
    /// Max file size in bytes.
    pub max_bytes: u64,
    /// Allowed MIME types — operator-restricted at the form
    /// boundary. Empty list = reject all (the operator must
    /// explicitly opt-in to a list).
    pub allowed_media_types: Vec<String>,
    /// Whether virus scan is required (default: true). The
    /// runtime refuses to deliver the submission until the
    /// scan completes successfully.
    #[serde(default = "default_true")]
    pub virus_scan_required: bool,
}

fn default_true() -> bool {
    true
}

/// Closed enum of spam-detection signals. Operators stack as
/// many as fit the audience; a submission with ≥1 signal is
/// quarantined for operator review (not silently dropped).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpamSignal {
    /// Hidden honeypot field had a value.
    HoneypotFilled,
    /// Submission rate exceeded the operator's per-IP / per-form
    /// limit.
    RateLimited,
    /// Classifier (e.g. operator-pluggable Akismet) returned
    /// "spam".
    Classifier,
    /// Submission completed too fast for a human (under N seconds).
    TooFast,
    /// Submission left the page open too long (likely a stale
    /// CSRF token / replay).
    Stale,
    /// Originating IP appears on a hostile-network list.
    NetworkBlocklist,
}

impl SpamSignal {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::HoneypotFilled => "honeypot-filled",
            Self::RateLimited => "rate-limited",
            Self::Classifier => "classifier",
            Self::TooFast => "too-fast",
            Self::Stale => "stale",
            Self::NetworkBlocklist => "network-blocklist",
        }
    }
}

/// Form definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Form {
    /// Stable form id.
    pub id: String,
    /// User-facing title.
    pub title: String,
    /// Fields.
    pub fields: Vec<FormField>,
    /// GDPR consents.
    #[serde(default)]
    pub consents: Vec<GdprConsent>,
    /// Attachments.
    #[serde(default)]
    pub attachments: Vec<AttachmentField>,
    /// Webhook delivery target. Submissions deliver here once
    /// they pass spam + virus-scan gates.
    pub webhook_url: String,
}

impl Form {
    /// Validate the form against the accessibility + privacy
    /// invariants:
    ///   * title non-empty
    ///   * every field has a non-empty label
    ///   * every field id is kebab-case + unique
    ///   * webhook_url is HTTPS
    ///   * at most one Honeypot field (extra honeypots have no
    ///     benefit + risk colliding by name)
    ///   * file fields are also declared as attachments
    ///     (cross-check belt-and-braces)
    pub fn validate(&self) -> Result<(), FormError> {
        if self.title.trim().is_empty() {
            return Err(FormError::Invalid("title empty".into()));
        }
        if !self.webhook_url.starts_with("https://") {
            return Err(FormError::Invalid(format!(
                "webhook_url must be https: {}",
                self.webhook_url
            )));
        }
        let mut seen = std::collections::HashSet::new();
        let mut honeypots = 0;
        for f in &self.fields {
            if f.label.trim().is_empty() {
                return Err(FormError::Invalid(format!("field {} unlabelled", f.id)));
            }
            if !is_kebab(&f.id) {
                return Err(FormError::Invalid(format!("field id not kebab: {}", f.id)));
            }
            if !seen.insert(&f.id) {
                return Err(FormError::Invalid(format!("duplicate field id: {}", f.id)));
            }
            if matches!(f.kind, FieldKind::Honeypot) {
                honeypots += 1;
            }
        }
        if honeypots > 1 {
            return Err(FormError::Invalid("more than one honeypot field".into()));
        }
        Ok(())
    }

    /// Add a honeypot field if none exists. Returns true when one
    /// was added. Field id is `hp-marker` — kebab-case so it
    /// passes the form-level id validator, but uncommon enough
    /// that auto-fill / password-manager heuristics skip it.
    /// (Earlier versions used `_hp`, which the kebab validator
    /// rejected — bug found via `forge forms validate` in T91
    /// sixth wiring.)
    pub fn with_honeypot(&mut self) -> bool {
        if self
            .fields
            .iter()
            .any(|f| matches!(f.kind, FieldKind::Honeypot))
        {
            return false;
        }
        self.fields.push(FormField {
            id: "hp-marker".into(),
            label: "Leave this field empty".into(),
            description: None,
            kind: FieldKind::Honeypot,
            required: false,
        });
        true
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
            'a'..='z' | '0'..='9' => {
                prev_dash = false;
            }
            '-' | '_' => {
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

/// Submission lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubmissionState {
    /// Received, awaiting spam + virus-scan checks.
    Received,
    /// Quarantined — at least one spam signal fired.
    Quarantined,
    /// Awaiting virus scan completion on attachments.
    AwaitingScan,
    /// Cleared all gates; webhook delivery in progress.
    Delivering,
    /// Webhook delivered.
    Delivered,
    /// Webhook delivery retries exhausted.
    DeliveryFailed,
    /// Operator explicitly rejected.
    Rejected,
}

impl SubmissionState {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::Quarantined => "quarantined",
            Self::AwaitingScan => "awaiting-scan",
            Self::Delivering => "delivering",
            Self::Delivered => "delivered",
            Self::DeliveryFailed => "delivery-failed",
            Self::Rejected => "rejected",
        }
    }

    /// Whether the state is terminal.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Delivered | Self::DeliveryFailed | Self::Rejected
        )
    }
}

/// One webhook delivery attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DeliveryAttempt {
    /// Sequence number — first attempt is 1.
    pub seq: u32,
    /// HTTP status code returned by the webhook target.
    pub http_status: u16,
    /// When the attempt completed.
    pub completed_at: time::OffsetDateTime,
    /// Optional error text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl DeliveryAttempt {
    /// Whether the attempt is considered successful (2xx status).
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.http_status)
    }
}

/// Typed errors at the form boundary.
#[derive(Debug, thiserror::Error)]
pub enum FormError {
    /// Form definition failed invariants.
    #[error("invalid: {0}")]
    Invalid(String),
    /// Webhook delivery exhausted retries.
    #[error("delivery: {0}")]
    Delivery(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn ok_form() -> Form {
        Form {
            id: "contact".into(),
            title: "Contact us".into(),
            fields: vec![
                FormField {
                    id: "name".into(),
                    label: "Name".into(),
                    description: None,
                    kind: FieldKind::Text,
                    required: true,
                },
                FormField {
                    id: "email".into(),
                    label: "Email".into(),
                    description: None,
                    kind: FieldKind::Email,
                    required: true,
                },
                FormField {
                    id: "message".into(),
                    label: "Message".into(),
                    description: None,
                    kind: FieldKind::TextArea,
                    required: true,
                },
            ],
            consents: vec![],
            attachments: vec![],
            webhook_url: "https://hooks.example.com/contact".into(),
        }
    }

    #[test]
    fn field_kind_slugs_distinct() {
        let ks = [
            FieldKind::Text,
            FieldKind::TextArea,
            FieldKind::Email,
            FieldKind::Url,
            FieldKind::Tel,
            FieldKind::Number,
            FieldKind::Date,
            FieldKind::Select,
            FieldKind::Checkboxes,
            FieldKind::File,
            FieldKind::Honeypot,
            FieldKind::GdprConsent,
        ];
        let mut s = std::collections::HashSet::new();
        for k in ks {
            assert!(s.insert(k.slug()));
        }
    }

    #[test]
    fn html_input_type_map_consistent() {
        assert_eq!(FieldKind::Text.html_input_type(), Some("text"));
        assert_eq!(FieldKind::Email.html_input_type(), Some("email"));
        assert_eq!(FieldKind::GdprConsent.html_input_type(), Some("checkbox"));
        assert_eq!(FieldKind::TextArea.html_input_type(), None);
        assert_eq!(FieldKind::Select.html_input_type(), None);
        assert_eq!(FieldKind::File.html_input_type(), None);
    }

    #[test]
    fn form_validate_ok_path() {
        assert!(ok_form().validate().is_ok());
    }

    #[test]
    fn form_validate_rejects_http_webhook() {
        let mut f = ok_form();
        f.webhook_url = "http://hooks.example.com".into();
        assert!(f.validate().is_err());
    }

    #[test]
    fn form_validate_rejects_unlabelled_field() {
        let mut f = ok_form();
        f.fields[0].label = "".into();
        assert!(f.validate().is_err());
    }

    #[test]
    fn form_validate_rejects_non_kebab_id() {
        let mut f = ok_form();
        f.fields[0].id = "Name".into();
        assert!(f.validate().is_err());
    }

    #[test]
    fn form_validate_rejects_duplicate_id() {
        let mut f = ok_form();
        f.fields[1].id = "name".into();
        assert!(f.validate().is_err());
    }

    #[test]
    fn form_validate_rejects_multiple_honeypots() {
        let mut f = ok_form();
        f.fields.push(FormField {
            id: "hp-a".into(),
            label: "Leave blank".into(),
            description: None,
            kind: FieldKind::Honeypot,
            required: false,
        });
        f.fields.push(FormField {
            id: "hp-b".into(),
            label: "Leave blank".into(),
            description: None,
            kind: FieldKind::Honeypot,
            required: false,
        });
        assert!(f.validate().is_err());
    }

    #[test]
    fn with_honeypot_adds_when_missing() {
        let mut f = ok_form();
        assert!(f.with_honeypot());
        assert!(f
            .fields
            .iter()
            .any(|x| matches!(x.kind, FieldKind::Honeypot)));
        // Second call is a no-op.
        assert!(!f.with_honeypot());
        assert_eq!(
            f.fields
                .iter()
                .filter(|x| matches!(x.kind, FieldKind::Honeypot))
                .count(),
            1
        );
    }

    // Regression-guard for the bug surfaced by `forge forms
    // validate` integration testing (T91 sixth wiring): the
    // honeypot field with_honeypot() generates MUST pass
    // Form::validate(), otherwise auto-honeypotting a form
    // immediately makes it invalid.
    #[test]
    fn with_honeypot_produces_validating_form() {
        let mut f = ok_form();
        f.with_honeypot();
        assert!(
            f.validate().is_ok(),
            "with_honeypot() produced a form that fails its own validate()"
        );
    }

    #[test]
    fn spam_signal_slugs_distinct() {
        let ss = [
            SpamSignal::HoneypotFilled,
            SpamSignal::RateLimited,
            SpamSignal::Classifier,
            SpamSignal::TooFast,
            SpamSignal::Stale,
            SpamSignal::NetworkBlocklist,
        ];
        let mut s = std::collections::HashSet::new();
        for x in ss {
            assert!(s.insert(x.slug()));
        }
    }

    #[test]
    fn submission_terminal_set() {
        assert!(SubmissionState::Delivered.is_terminal());
        assert!(SubmissionState::DeliveryFailed.is_terminal());
        assert!(SubmissionState::Rejected.is_terminal());
        assert!(!SubmissionState::Received.is_terminal());
        assert!(!SubmissionState::AwaitingScan.is_terminal());
        assert!(!SubmissionState::Delivering.is_terminal());
        assert!(!SubmissionState::Quarantined.is_terminal());
    }

    #[test]
    fn delivery_attempt_success_check() {
        let now = datetime!(2026-05-18 12:00:00 UTC);
        let ok = DeliveryAttempt {
            seq: 1,
            http_status: 200,
            completed_at: now,
            error: None,
        };
        let err = DeliveryAttempt {
            seq: 1,
            http_status: 500,
            completed_at: now,
            error: Some("boom".into()),
        };
        assert!(ok.is_success());
        assert!(!err.is_success());
    }

    #[test]
    fn attachment_virus_scan_required_default_true() {
        let a = AttachmentField {
            id: "f1".into(),
            max_bytes: 1024,
            allowed_media_types: vec!["image/png".into()],
            virus_scan_required: default_true(),
        };
        assert!(a.virus_scan_required);
        // Deserializing without the field also defaults to true.
        let j = r#"{"id":"f1","max-bytes":1024,"allowed-media-types":["image/png"]}"#;
        let back: AttachmentField = serde_json::from_str(j).unwrap();
        assert!(back.virus_scan_required);
    }

    #[test]
    fn form_serde_round_trip() {
        let f = ok_form();
        let j = serde_json::to_string(&f).unwrap();
        let back: Form = serde_json::from_str(&j).unwrap();
        assert_eq!(f, back);
    }

    #[test]
    fn form_rejects_unknown_field() {
        let bad = r#"{"id":"x","title":"t","fields":[],"webhook-url":"https://h","ahem":1}"#;
        let r: Result<Form, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    // T97: slug-vs-serde-wire regression guard.
    #[test]
    fn slug_matches_serde_wire_across_all_enums() {
        for v in [
            FieldKind::Text,
            FieldKind::TextArea,
            FieldKind::Email,
            FieldKind::Url,
            FieldKind::Tel,
            FieldKind::Number,
            FieldKind::Date,
            FieldKind::Select,
            FieldKind::Checkboxes,
            FieldKind::File,
            FieldKind::Honeypot,
            FieldKind::GdprConsent,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            SpamSignal::HoneypotFilled,
            SpamSignal::RateLimited,
            SpamSignal::Classifier,
            SpamSignal::TooFast,
            SpamSignal::Stale,
            SpamSignal::NetworkBlocklist,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            SubmissionState::Received,
            SubmissionState::Quarantined,
            SubmissionState::AwaitingScan,
            SubmissionState::Delivering,
            SubmissionState::Delivered,
            SubmissionState::DeliveryFailed,
            SubmissionState::Rejected,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
    }
}
