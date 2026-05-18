//! `trust-safety-core` — typed Trust & Safety contract.
//!
//! Per `PLATFORM_ROADMAP.md` §9 + `super_society_tech_stack`:
//! every uploaded asset, every published page, every operator
//! account is screened against a typed set of T&S concerns
//! before it reaches the public surface. This crate defines the
//! cross-implementation contract — the actual detectors (CSAM
//! perceptual hashing, phishing-URL classifiers, sanctions-list
//! lookups, content-moderation models) plug in via the
//! [`SafetyScanner`] trait.
//!
//! ### Scope of THIS crate
//!
//! Typed surface only:
//!   * What kinds of T&S concerns the platform recognizes
//!   * What actions moderation can take
//!   * What a per-asset verdict looks like
//!   * What the trait every scanner satisfies looks like
//!
//! No actual detection logic. No NCMEC integration. No vendor
//! API client. Those live in downstream `trust-safety-*` crates
//! that plug into [`SafetyScanner`].
//!
//! ### Why typed
//!
//! Per `super_society_tech_stack`: free-form string-tagged
//! moderation is the canonical "we have safety" claim that
//! falls apart under audit. Closed-enum [`ConcernKind`] +
//! deterministic [`ModerationAction`] resolution from a typed
//! [`SafetyVerdict`] means every moderation decision is
//! reviewable + reversible + auditable through the same
//! observability surfaces (#69 hash-chained audit log) the
//! rest of the platform uses.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Closed enum of T&S concern kinds the platform recognises.
/// Adding a kind is a typed change reviewable in one commit,
/// not a free-form string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConcernKind {
    /// Child Sexual Abuse Material. The platform's
    /// non-negotiable highest-severity class. Mandatory reporting
    /// per US 18 U.S.C. § 2258A + EU CSAM Regulation.
    Csam,
    /// Phishing — page or asset designed to harvest credentials
    /// under a forged identity.
    Phishing,
    /// Spam — unsolicited bulk content; SEO spam; comment-spam
    /// network operations.
    Spam,
    /// Sanctions hit — operator or asset matches an OFAC SDN,
    /// EU sanctions, UK financial-sanctions list, or equivalent.
    Sanctions,
    /// Self-harm content where the contextual risk classifier
    /// flags the page as harmful (suicide methods, eating
    /// disorder content not in a recovery frame, etc.).
    SelfHarm,
    /// Violent extremism — terrorism-supporting content,
    /// glorification of mass violence, recruitment material.
    Extremism,
    /// Non-consensual intimate imagery (revenge porn).
    Nciii,
    /// Malware — page or asset distributes executable malware.
    Malware,
    /// Counterfeit / IP-violation — trademark + copyright
    /// infringement at sufficient scale to warrant T&S action
    /// (not minor disputes).
    IpViolation,
    /// Hate speech against a protected class as defined by the
    /// operator's declared region's hate-speech law.
    HateSpeech,
}

impl ConcernKind {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Csam => "csam",
            Self::Phishing => "phishing",
            Self::Spam => "spam",
            Self::Sanctions => "sanctions",
            Self::SelfHarm => "self-harm",
            Self::Extremism => "extremism",
            Self::Nciii => "nciii",
            Self::Malware => "malware",
            Self::IpViolation => "ip-violation",
            Self::HateSpeech => "hate-speech",
        }
    }

    /// Whether platform policy requires immediate mandatory
    /// reporting to authorities. The set is small + legally
    /// driven (CSAM under US 18 U.S.C. § 2258A; NCIII under
    /// some state laws; terrorism under various national laws).
    pub fn is_mandatory_report(&self) -> bool {
        matches!(self, Self::Csam | Self::Nciii | Self::Extremism)
    }

    /// Whether platform policy allows the operator to override.
    /// Mandatory-report concerns can never be overridden by the
    /// operator (policy + legal exposure both refuse).
    pub fn operator_can_override(&self) -> bool {
        !self.is_mandatory_report()
    }
}

/// Closed enum of moderation actions. Resolved deterministically
/// from a [`SafetyVerdict`] via [`ModerationAction::for_verdict`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModerationAction {
    /// Allow without restriction.
    Allow,
    /// Allow but flag for operator review.
    Warn,
    /// Hold from public surface; operator-review queue.
    Quarantine,
    /// Block + delete; operator cannot override.
    Block,
    /// Block + report to authorities under the relevant
    /// mandatory-reporting statute.
    BlockAndReport,
}

impl ModerationAction {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Warn => "warn",
            Self::Quarantine => "quarantine",
            Self::Block => "block",
            Self::BlockAndReport => "block-and-report",
        }
    }

    /// Whether the action removes the asset from the public surface.
    pub fn is_blocking(&self) -> bool {
        matches!(self, Self::Quarantine | Self::Block | Self::BlockAndReport)
    }

    /// Derive the policy action from a verdict. Deterministic +
    /// auditable; the rule is:
    ///   * mandatory-report kind + Confirmed → BlockAndReport
    ///   * mandatory-report kind + Likely    → BlockAndReport
    ///   * non-mandatory + Confirmed         → Block
    ///   * non-mandatory + Likely            → Quarantine
    ///   * any + Suspected                   → Warn
    ///   * any + Cleared                     → Allow
    pub fn for_verdict(verdict: &SafetyVerdict) -> Self {
        match (verdict.kind, verdict.confidence) {
            (k, ScanConfidence::Confirmed) if k.is_mandatory_report() => Self::BlockAndReport,
            (k, ScanConfidence::Likely) if k.is_mandatory_report() => Self::BlockAndReport,
            (_, ScanConfidence::Confirmed) => Self::Block,
            (_, ScanConfidence::Likely) => Self::Quarantine,
            (_, ScanConfidence::Suspected) => Self::Warn,
            (_, ScanConfidence::Cleared) => Self::Allow,
        }
    }
}

/// Confidence tier for a single scan finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanConfidence {
    /// Scanner is sure the asset is in this kind's class.
    /// Example: PhotoDNA hash matches a known NCMEC entry.
    Confirmed,
    /// High-probability match. Example: model output > 0.9.
    Likely,
    /// Possible match worth operator review. Example: model
    /// output 0.5-0.9 or partial heuristic match.
    Suspected,
    /// Asset is NOT in this kind's class.
    Cleared,
}

impl ScanConfidence {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Likely => "likely",
            Self::Suspected => "suspected",
            Self::Cleared => "cleared",
        }
    }
}

/// One scanner's verdict against one asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SafetyVerdict {
    /// Which T&S concern this verdict covers.
    pub kind: ConcernKind,
    /// Scanner confidence.
    pub confidence: ScanConfidence,
    /// Stable identifier of the scanner that produced this
    /// verdict (e.g. `"photodna-2.1"`, `"sanctions-ofac-v3"`,
    /// `"phishtank-2026-05"`).
    pub scanner_id: String,
    /// Free-form scanner-specific detail (matched hash, list
    /// entry id, rule slug). Not interpreted by this crate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// When the scan ran.
    pub scanned_at: time::OffsetDateTime,
}

/// Aggregated report across multiple scanners + concern kinds for
/// one asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ScanReport {
    /// Stable identifier for the asset under review (URL, hash,
    /// content-id — operator-defined).
    pub asset_id: String,
    /// Every scanner's verdict.
    pub verdicts: Vec<SafetyVerdict>,
}

impl ScanReport {
    /// Determine the platform's final moderation action for
    /// this asset. Picks the worst (highest-impact) action
    /// across all verdicts.
    pub fn final_action(&self) -> ModerationAction {
        let mut worst = ModerationAction::Allow;
        for v in &self.verdicts {
            let a = ModerationAction::for_verdict(v);
            worst = worst_of(worst, a);
        }
        worst
    }

    /// Whether ≥1 verdict triggered mandatory reporting.
    pub fn requires_mandatory_report(&self) -> bool {
        self.verdicts.iter().any(|v| {
            v.kind.is_mandatory_report()
                && matches!(
                    v.confidence,
                    ScanConfidence::Confirmed | ScanConfidence::Likely
                )
        })
    }

    /// Verdicts requiring operator review (Warn / Quarantine).
    pub fn reviewable(&self) -> Vec<&SafetyVerdict> {
        self.verdicts
            .iter()
            .filter(|v| {
                let a = ModerationAction::for_verdict(v);
                matches!(a, ModerationAction::Warn | ModerationAction::Quarantine)
            })
            .collect()
    }
}

/// Pick the worse of two moderation actions. Order from least
/// severe to most: Allow < Warn < Quarantine < Block < BlockAndReport.
fn worst_of(a: ModerationAction, b: ModerationAction) -> ModerationAction {
    let rank = |x: ModerationAction| match x {
        ModerationAction::Allow => 0,
        ModerationAction::Warn => 1,
        ModerationAction::Quarantine => 2,
        ModerationAction::Block => 3,
        ModerationAction::BlockAndReport => 4,
    };
    if rank(a) >= rank(b) {
        a
    } else {
        b
    }
}

/// Pluggable safety scanner. Implementations live downstream —
/// `trust-safety-csam-photodna`, `trust-safety-sanctions-ofac`,
/// `trust-safety-phishing-classifier`, etc.
///
/// Async-runtime-agnostic at this contract level. Backends pick
/// their own runtime in the impl crate.
pub trait SafetyScanner {
    /// Stable identifier for this scanner instance.
    fn scanner_id(&self) -> &'static str;
    /// Which concern kinds this scanner can detect. The
    /// orchestrator routes assets through scanners whose
    /// covers() set includes the concerns the operator wants
    /// checked.
    fn covers(&self) -> &'static [ConcernKind];
    /// Scan a single asset. The bytes are interpreted per the
    /// scanner — image hashers want pixel data, URL classifiers
    /// want the URL bytes, etc.
    fn scan(&self, asset_id: &str, bytes: &[u8]) -> Result<SafetyVerdict, SafetyError>;
}

/// Typed errors at the T&S boundary.
#[derive(Debug, thiserror::Error)]
pub enum SafetyError {
    /// Scanner couldn't process the asset (unsupported format,
    /// decode failure, etc.).
    #[error("scanner refused: {0}")]
    ScannerRefused(String),
    /// Scanner backend errored (network failure on remote
    /// PhotoDNA, sanctions-list fetch failure, etc.).
    #[error("scanner backend: {0}")]
    Backend(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn verdict(kind: ConcernKind, conf: ScanConfidence) -> SafetyVerdict {
        SafetyVerdict {
            kind,
            confidence: conf,
            scanner_id: "test-scanner".into(),
            detail: None,
            scanned_at: datetime!(2026-05-18 12:00:00 UTC),
        }
    }

    #[test]
    fn concern_kind_mandatory_report_set() {
        assert!(ConcernKind::Csam.is_mandatory_report());
        assert!(ConcernKind::Nciii.is_mandatory_report());
        assert!(ConcernKind::Extremism.is_mandatory_report());
        assert!(!ConcernKind::Spam.is_mandatory_report());
        assert!(!ConcernKind::Phishing.is_mandatory_report());
        assert!(!ConcernKind::Sanctions.is_mandatory_report());
    }

    #[test]
    fn mandatory_report_kinds_refuse_operator_override() {
        assert!(!ConcernKind::Csam.operator_can_override());
        assert!(!ConcernKind::Nciii.operator_can_override());
        assert!(ConcernKind::Spam.operator_can_override());
        assert!(ConcernKind::Phishing.operator_can_override());
    }

    #[test]
    fn moderation_action_for_csam_confirmed_is_block_and_report() {
        let v = verdict(ConcernKind::Csam, ScanConfidence::Confirmed);
        assert_eq!(
            ModerationAction::for_verdict(&v),
            ModerationAction::BlockAndReport
        );
    }

    #[test]
    fn moderation_action_for_csam_likely_is_block_and_report() {
        let v = verdict(ConcernKind::Csam, ScanConfidence::Likely);
        assert_eq!(
            ModerationAction::for_verdict(&v),
            ModerationAction::BlockAndReport
        );
    }

    #[test]
    fn moderation_action_for_spam_confirmed_is_block() {
        let v = verdict(ConcernKind::Spam, ScanConfidence::Confirmed);
        assert_eq!(ModerationAction::for_verdict(&v), ModerationAction::Block);
    }

    #[test]
    fn moderation_action_for_phishing_likely_is_quarantine() {
        let v = verdict(ConcernKind::Phishing, ScanConfidence::Likely);
        assert_eq!(
            ModerationAction::for_verdict(&v),
            ModerationAction::Quarantine
        );
    }

    #[test]
    fn moderation_action_for_any_suspected_is_warn() {
        let v1 = verdict(ConcernKind::Spam, ScanConfidence::Suspected);
        let v2 = verdict(ConcernKind::Csam, ScanConfidence::Suspected);
        assert_eq!(ModerationAction::for_verdict(&v1), ModerationAction::Warn);
        // Even CSAM-Suspected is a Warn (operator review queue) —
        // the platform doesn't auto-report on low-confidence
        // findings. False-positive cost is too high.
        assert_eq!(ModerationAction::for_verdict(&v2), ModerationAction::Warn);
    }

    #[test]
    fn moderation_action_for_cleared_is_allow() {
        let v = verdict(ConcernKind::Csam, ScanConfidence::Cleared);
        assert_eq!(ModerationAction::for_verdict(&v), ModerationAction::Allow);
    }

    #[test]
    fn moderation_action_is_blocking_predicate() {
        assert!(!ModerationAction::Allow.is_blocking());
        assert!(!ModerationAction::Warn.is_blocking());
        assert!(ModerationAction::Quarantine.is_blocking());
        assert!(ModerationAction::Block.is_blocking());
        assert!(ModerationAction::BlockAndReport.is_blocking());
    }

    #[test]
    fn scan_report_picks_worst_action_across_verdicts() {
        let report = ScanReport {
            asset_id: "asset-1".into(),
            verdicts: vec![
                verdict(ConcernKind::Spam, ScanConfidence::Suspected), // Warn
                verdict(ConcernKind::Phishing, ScanConfidence::Likely), // Quarantine
                verdict(ConcernKind::Malware, ScanConfidence::Cleared), // Allow
            ],
        };
        assert_eq!(report.final_action(), ModerationAction::Quarantine);
        assert!(!report.requires_mandatory_report());
    }

    #[test]
    fn scan_report_with_csam_confirmed_requires_mandatory_report() {
        let report = ScanReport {
            asset_id: "asset-2".into(),
            verdicts: vec![
                verdict(ConcernKind::Spam, ScanConfidence::Cleared),
                verdict(ConcernKind::Csam, ScanConfidence::Confirmed),
            ],
        };
        assert_eq!(report.final_action(), ModerationAction::BlockAndReport);
        assert!(report.requires_mandatory_report());
    }

    #[test]
    fn scan_report_with_csam_suspected_doesnt_require_report() {
        // Suspected (low confidence) for a mandatory-report kind
        // is still Warn — we don't auto-report on uncertainty.
        let report = ScanReport {
            asset_id: "asset-3".into(),
            verdicts: vec![verdict(ConcernKind::Csam, ScanConfidence::Suspected)],
        };
        assert_eq!(report.final_action(), ModerationAction::Warn);
        assert!(!report.requires_mandatory_report());
    }

    #[test]
    fn scan_report_reviewable_returns_warn_and_quarantine() {
        let report = ScanReport {
            asset_id: "asset-4".into(),
            verdicts: vec![
                verdict(ConcernKind::Spam, ScanConfidence::Cleared), // Allow
                verdict(ConcernKind::Phishing, ScanConfidence::Likely), // Quarantine
                verdict(ConcernKind::Spam, ScanConfidence::Suspected), // Warn
                verdict(ConcernKind::Malware, ScanConfidence::Confirmed), // Block (not reviewable)
            ],
        };
        let reviewable = report.reviewable();
        assert_eq!(reviewable.len(), 2);
    }

    #[test]
    fn empty_scan_report_resolves_allow() {
        let report = ScanReport {
            asset_id: "asset-5".into(),
            verdicts: vec![],
        };
        assert_eq!(report.final_action(), ModerationAction::Allow);
    }

    #[test]
    fn verdict_serde_round_trips() {
        let v = verdict(ConcernKind::Phishing, ScanConfidence::Likely);
        let s = serde_json::to_string(&v).unwrap();
        let back: SafetyVerdict = serde_json::from_str(&s).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn verdict_rejects_unknown_field() {
        let bad = r#"{"kind":"spam","confidence":"likely","scanner-id":"x","scanned-at":"2026-05-18T12:00:00Z","ahem":1}"#;
        let r: Result<SafetyVerdict, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    #[test]
    fn slugs_distinct_across_enums() {
        let concerns = [
            ConcernKind::Csam,
            ConcernKind::Phishing,
            ConcernKind::Spam,
            ConcernKind::Sanctions,
            ConcernKind::SelfHarm,
            ConcernKind::Extremism,
            ConcernKind::Nciii,
            ConcernKind::Malware,
            ConcernKind::IpViolation,
            ConcernKind::HateSpeech,
        ];
        let mut seen = std::collections::HashSet::new();
        for c in concerns {
            assert!(seen.insert(c.slug()));
        }
        let actions = [
            ModerationAction::Allow,
            ModerationAction::Warn,
            ModerationAction::Quarantine,
            ModerationAction::Block,
            ModerationAction::BlockAndReport,
        ];
        let mut seen2 = std::collections::HashSet::new();
        for a in actions {
            assert!(seen2.insert(a.slug()));
        }
    }
}
