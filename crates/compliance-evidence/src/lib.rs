//! `compliance-evidence` — typed SOC 2 Type II + ISO 27001
//! evidence-collection surface.
//!
//! Per `PLATFORM_ROADMAP.md` §8 + `super_society_tech_stack`:
//! audit-readiness is a CONTINUOUS BYPRODUCT of normal platform
//! operation, not a quarter-end fire drill. Every typed
//! observability event (#69) + every signed artifact (#64) +
//! every status entry (#68) is potential evidence; this crate is
//! the contract for tagging it as such, indexing it by control,
//! and surfacing the readiness posture.
//!
//! ### Surface
//!
//! - [`ControlFramework`]   — closed enum: Soc2 / Iso27001
//! - [`ControlId`]          — opaque per-framework control id
//! - [`EvidenceKind`]        — what *kind* of evidence
//!                              (attestation / log entry /
//!                              policy doc / config snapshot /
//!                              drill result / etc.)
//! - [`EvidenceArtifact`]   — one collected piece
//! - [`ControlBinding`]     — links a Control → its evidence
//! - [`ReadinessReport`]    — full readiness posture
//! - [`ReadinessStatus`]    — per-control state
//! - [`EvidenceError`]      — typed errors
//!
//! Per `feedback_iso_standards`: tracks both SOC 2 Trust
//! Services Criteria (CC1–CC9, A1, C1, PI1, P1–P8) and ISO
//! 27001:2022 Annex A controls (5.1–8.34). Operators map their
//! platform configuration to the relevant control set; this
//! crate computes readiness from typed inputs.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Closed enum of compliance frameworks the platform tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControlFramework {
    /// AICPA SOC 2 Type II.
    Soc2,
    /// ISO/IEC 27001:2022.
    Iso27001,
}

impl ControlFramework {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Soc2 => "soc2",
            Self::Iso27001 => "iso-27001",
        }
    }
}

/// Per-framework control identifier (e.g. `"CC6.1"` for SOC 2,
/// `"A.5.1"` for ISO 27001). Validates shape lightly — the
/// framework's authoring conventions vary so we accept
/// `[A-Z0-9.]{1,16}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ControlId(String);

impl ControlId {
    /// Construct from a control-id string.
    pub fn parse(s: impl AsRef<str>) -> Result<Self, EvidenceError> {
        let s = s.as_ref();
        if s.is_empty() || s.len() > 16 {
            return Err(EvidenceError::InvalidControlId(format!(
                "{s:?} length out of range"
            )));
        }
        for c in s.chars() {
            if !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == '.') {
                return Err(EvidenceError::InvalidControlId(format!(
                    "{s:?} char {c:?} not in [A-Z0-9.]"
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

/// Closed enum of evidence kinds. Each maps to a different
/// platform surface that produces the artifact as a byproduct of
/// normal operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceKind {
    /// Signed attestation per manifest-attest (#64).
    Attestation,
    /// Hash-chained audit log entry per observability-core (#69).
    AuditLogEntry,
    /// Operator-authored policy document.
    PolicyDocument,
    /// Snapshot of a config file (deny.toml / phases.toml /
    /// backends.toml / manifest.toml).
    ConfigSnapshot,
    /// DR drill result per dr-core (#70).
    DrDrillResult,
    /// Status-page incident entry per ops-status (#68).
    IncidentEntry,
    /// Supply-chain SBOM per task #67.
    Sbom,
    /// Penetration test report (operator-supplied).
    PentestReport,
    /// Vendor security assessment.
    VendorAssessment,
    /// Training completion record.
    TrainingRecord,
}

impl EvidenceKind {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Attestation => "attestation",
            Self::AuditLogEntry => "audit-log-entry",
            Self::PolicyDocument => "policy-document",
            Self::ConfigSnapshot => "config-snapshot",
            Self::DrDrillResult => "dr-drill-result",
            Self::IncidentEntry => "incident-entry",
            Self::Sbom => "sbom",
            Self::PentestReport => "pentest-report",
            Self::VendorAssessment => "vendor-assessment",
            Self::TrainingRecord => "training-record",
        }
    }

    /// Whether the platform produces this kind continuously
    /// (during normal operation) vs operator-authored at intervals.
    pub fn is_continuous(&self) -> bool {
        matches!(
            self,
            Self::Attestation
                | Self::AuditLogEntry
                | Self::ConfigSnapshot
                | Self::DrDrillResult
                | Self::IncidentEntry
                | Self::Sbom
        )
    }
}

/// One piece of evidence. Hashed for tamper detection; consumers
/// can verify the artifact matches the claimed content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct EvidenceArtifact {
    /// Stable identifier (uuid v4 string or hash prefix).
    pub id: String,
    /// What kind of evidence this is.
    pub kind: EvidenceKind,
    /// SHA-256 of the artifact's canonical bytes (hex).
    pub content_sha256: String,
    /// When collected (ISO-8601 UTC).
    pub collected_at: time::OffsetDateTime,
    /// On-disk reference (path / URL / object-store key).
    pub reference: String,
    /// Optional free-form metadata.
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl EvidenceArtifact {
    /// Compute the canonical content hash from bytes.
    pub fn hash_content(bytes: &[u8]) -> String {
        let digest = Sha256::digest(bytes);
        digest.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Verify the artifact's `content_sha256` matches the
    /// provided bytes.
    pub fn verify_against(&self, bytes: &[u8]) -> Result<(), EvidenceError> {
        let computed = Self::hash_content(bytes);
        if computed != self.content_sha256 {
            return Err(EvidenceError::ContentTampered {
                expected: self.content_sha256.clone(),
                got: computed,
            });
        }
        Ok(())
    }
}

/// Binds a control to its supporting evidence. The readiness
/// gate checks every required control has ≥ 1 valid binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ControlBinding {
    /// Framework this control belongs to.
    pub framework: ControlFramework,
    /// Control identifier.
    pub control: ControlId,
    /// Human-readable control title.
    pub title: String,
    /// Evidence artifacts supporting this control.
    pub evidence: Vec<EvidenceArtifact>,
    /// Optional owner — who at the operator is responsible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

/// Readiness status per control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReadinessStatus {
    /// Control is fully supported by current evidence.
    Ready,
    /// Some evidence collected; gaps remain.
    Partial,
    /// No evidence yet.
    Missing,
    /// Evidence stale (older than the policy window).
    Stale,
}

impl ReadinessStatus {
    /// Stable slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Partial => "partial",
            Self::Missing => "missing",
            Self::Stale => "stale",
        }
    }
}

/// Per-control readiness assessment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ControlReadiness {
    /// Which control.
    pub control: ControlId,
    /// Framework.
    pub framework: ControlFramework,
    /// Computed readiness.
    pub status: ReadinessStatus,
    /// Number of evidence artifacts attached.
    pub evidence_count: u32,
    /// Age of the freshest evidence (seconds; 0 if no evidence).
    pub freshest_evidence_age_secs: u64,
}

/// Full readiness report — every control + its assessment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ReadinessReport {
    /// Framework this report covers.
    pub framework: ControlFramework,
    /// When the report was generated.
    pub generated_at: time::OffsetDateTime,
    /// Per-control assessments.
    pub controls: Vec<ControlReadiness>,
    /// Maximum acceptable evidence age before status flips to
    /// Stale (in seconds; auditors typically expect 90-day
    /// freshness for continuous controls).
    pub stale_threshold_secs: u64,
}

impl ReadinessReport {
    /// Compute a report from a set of [`ControlBinding`]s.
    pub fn from_bindings(
        framework: ControlFramework,
        now: time::OffsetDateTime,
        bindings: &[ControlBinding],
        stale_threshold_secs: u64,
    ) -> Self {
        let mut controls = Vec::with_capacity(bindings.len());
        for binding in bindings {
            if binding.framework != framework {
                continue;
            }
            let evidence_count = binding.evidence.len() as u32;
            let freshest_age = binding
                .evidence
                .iter()
                .map(|e| (now - e.collected_at).whole_seconds().max(0) as u64)
                .min()
                .unwrap_or(0);
            let status = if evidence_count == 0 {
                ReadinessStatus::Missing
            } else if freshest_age > stale_threshold_secs {
                ReadinessStatus::Stale
            } else if evidence_count == 1 {
                ReadinessStatus::Partial
            } else {
                ReadinessStatus::Ready
            };
            controls.push(ControlReadiness {
                control: binding.control.clone(),
                framework: binding.framework,
                status,
                evidence_count,
                freshest_evidence_age_secs: freshest_age,
            });
        }
        Self {
            framework,
            generated_at: now,
            controls,
            stale_threshold_secs,
        }
    }

    /// Overall pass — true iff every control is Ready.
    pub fn is_audit_ready(&self) -> bool {
        !self.controls.is_empty()
            && self
                .controls
                .iter()
                .all(|c| c.status == ReadinessStatus::Ready)
    }

    /// Controls that aren't Ready.
    pub fn gaps(&self) -> Vec<&ControlReadiness> {
        self.controls
            .iter()
            .filter(|c| c.status != ReadinessStatus::Ready)
            .collect()
    }
}

/// Typed errors at the evidence boundary.
#[derive(Debug, thiserror::Error)]
pub enum EvidenceError {
    /// Control id failed shape validation.
    #[error("invalid control id: {0}")]
    InvalidControlId(String),
    /// Evidence content didn't match the declared hash.
    #[error("content tampered: expected {expected:?}, got {got:?}")]
    ContentTampered {
        /// Declared hash.
        expected: String,
        /// Recomputed hash.
        got: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn mk_evidence(at: time::OffsetDateTime, kind: EvidenceKind) -> EvidenceArtifact {
        EvidenceArtifact {
            id: "e1".into(),
            kind,
            content_sha256: EvidenceArtifact::hash_content(b"test"),
            collected_at: at,
            reference: "/evidence/e1".into(),
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn control_id_validates_shape() {
        assert!(ControlId::parse("CC6.1").is_ok());
        assert!(ControlId::parse("A.5.1").is_ok());
        assert!(ControlId::parse("").is_err());
        assert!(ControlId::parse("cc6.1").is_err()); // lowercase
        assert!(ControlId::parse(&"A".repeat(17)).is_err());
        assert!(ControlId::parse("CC6 1").is_err()); // space
    }

    #[test]
    fn evidence_kind_continuous_set() {
        assert!(EvidenceKind::Attestation.is_continuous());
        assert!(EvidenceKind::AuditLogEntry.is_continuous());
        assert!(EvidenceKind::ConfigSnapshot.is_continuous());
        assert!(EvidenceKind::DrDrillResult.is_continuous());
        assert!(EvidenceKind::IncidentEntry.is_continuous());
        assert!(EvidenceKind::Sbom.is_continuous());
        assert!(!EvidenceKind::PolicyDocument.is_continuous());
        assert!(!EvidenceKind::PentestReport.is_continuous());
        assert!(!EvidenceKind::VendorAssessment.is_continuous());
        assert!(!EvidenceKind::TrainingRecord.is_continuous());
    }

    #[test]
    fn evidence_artifact_verify_against_content() {
        let bytes = b"hello world";
        let e = EvidenceArtifact {
            id: "e1".into(),
            kind: EvidenceKind::PolicyDocument,
            content_sha256: EvidenceArtifact::hash_content(bytes),
            collected_at: datetime!(2026-05-18 12:00:00 UTC),
            reference: "/x".into(),
            metadata: BTreeMap::new(),
        };
        e.verify_against(bytes).unwrap();
        let bad = e.verify_against(b"tampered");
        assert!(matches!(bad, Err(EvidenceError::ContentTampered { .. })));
    }

    #[test]
    fn readiness_status_progression() {
        let now = datetime!(2026-05-18 12:00:00 UTC);
        let stale_threshold = 90 * 86_400;

        // Missing — no evidence
        let b1 = ControlBinding {
            framework: ControlFramework::Soc2,
            control: ControlId::parse("CC6.1").unwrap(),
            title: "Logical access controls".into(),
            evidence: vec![],
            owner: None,
        };

        // Partial — one piece of fresh evidence
        let b2 = ControlBinding {
            framework: ControlFramework::Soc2,
            control: ControlId::parse("CC6.2").unwrap(),
            title: "Authentication".into(),
            evidence: vec![mk_evidence(
                datetime!(2026-05-15 12:00:00 UTC),
                EvidenceKind::Attestation,
            )],
            owner: None,
        };

        // Ready — two pieces of fresh evidence
        let b3 = ControlBinding {
            framework: ControlFramework::Soc2,
            control: ControlId::parse("CC7.1").unwrap(),
            title: "Detection".into(),
            evidence: vec![
                mk_evidence(
                    datetime!(2026-05-10 12:00:00 UTC),
                    EvidenceKind::AuditLogEntry,
                ),
                mk_evidence(
                    datetime!(2026-05-15 12:00:00 UTC),
                    EvidenceKind::IncidentEntry,
                ),
            ],
            owner: None,
        };

        // Stale — evidence older than threshold
        let b4 = ControlBinding {
            framework: ControlFramework::Soc2,
            control: ControlId::parse("CC8.1").unwrap(),
            title: "Change management".into(),
            evidence: vec![mk_evidence(
                datetime!(2025-01-01 12:00:00 UTC),
                EvidenceKind::ConfigSnapshot,
            )],
            owner: None,
        };

        let report = ReadinessReport::from_bindings(
            ControlFramework::Soc2,
            now,
            &[b1, b2, b3, b4],
            stale_threshold,
        );

        let status_for = |id: &str| {
            report
                .controls
                .iter()
                .find(|c| c.control.as_str() == id)
                .map(|c| c.status)
        };
        assert_eq!(status_for("CC6.1"), Some(ReadinessStatus::Missing));
        assert_eq!(status_for("CC6.2"), Some(ReadinessStatus::Partial));
        assert_eq!(status_for("CC7.1"), Some(ReadinessStatus::Ready));
        assert_eq!(status_for("CC8.1"), Some(ReadinessStatus::Stale));
    }

    #[test]
    fn readiness_audit_ready_requires_all_ready() {
        let now = datetime!(2026-05-18 12:00:00 UTC);
        let stale_threshold = 90 * 86_400;
        let b_ready = ControlBinding {
            framework: ControlFramework::Soc2,
            control: ControlId::parse("CC6.1").unwrap(),
            title: "x".into(),
            evidence: vec![
                mk_evidence(
                    datetime!(2026-05-15 12:00:00 UTC),
                    EvidenceKind::Attestation,
                ),
                mk_evidence(
                    datetime!(2026-05-15 12:00:00 UTC),
                    EvidenceKind::AuditLogEntry,
                ),
            ],
            owner: None,
        };

        let r = ReadinessReport::from_bindings(
            ControlFramework::Soc2,
            now,
            &[b_ready.clone()],
            stale_threshold,
        );
        assert!(r.is_audit_ready());
        assert_eq!(r.gaps().len(), 0);

        // Add a missing control — overall report no longer ready.
        let b_missing = ControlBinding {
            framework: ControlFramework::Soc2,
            control: ControlId::parse("CC7.1").unwrap(),
            title: "y".into(),
            evidence: vec![],
            owner: None,
        };
        let r2 = ReadinessReport::from_bindings(
            ControlFramework::Soc2,
            now,
            &[b_ready, b_missing],
            stale_threshold,
        );
        assert!(!r2.is_audit_ready());
        assert_eq!(r2.gaps().len(), 1);
    }

    #[test]
    fn report_filters_to_framework() {
        let now = datetime!(2026-05-18 12:00:00 UTC);
        let soc2_binding = ControlBinding {
            framework: ControlFramework::Soc2,
            control: ControlId::parse("CC6.1").unwrap(),
            title: "x".into(),
            evidence: vec![],
            owner: None,
        };
        let iso_binding = ControlBinding {
            framework: ControlFramework::Iso27001,
            control: ControlId::parse("A.5.1").unwrap(),
            title: "y".into(),
            evidence: vec![],
            owner: None,
        };
        let r = ReadinessReport::from_bindings(
            ControlFramework::Soc2,
            now,
            &[soc2_binding, iso_binding],
            90 * 86_400,
        );
        assert_eq!(r.controls.len(), 1);
        assert_eq!(r.controls[0].framework, ControlFramework::Soc2);
    }

    #[test]
    fn empty_report_not_audit_ready() {
        let now = datetime!(2026-05-18 12:00:00 UTC);
        let r = ReadinessReport::from_bindings(ControlFramework::Soc2, now, &[], 90 * 86_400);
        assert!(!r.is_audit_ready());
    }

    #[test]
    fn binding_serde_round_trips() {
        let b = ControlBinding {
            framework: ControlFramework::Iso27001,
            control: ControlId::parse("A.5.1").unwrap(),
            title: "Information security policies".into(),
            evidence: vec![],
            owner: Some("ciso@example.com".into()),
        };
        let s = serde_json::to_string(&b).unwrap();
        let back: ControlBinding = serde_json::from_str(&s).unwrap();
        assert_eq!(b, back);
    }

    #[test]
    fn binding_rejects_unknown_field() {
        let bad = r#"{"framework":"soc2","control":"CC6.1","title":"x","evidence":[],"ahem":1}"#;
        let r: Result<ControlBinding, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    #[test]
    fn framework_slugs_distinct() {
        assert_ne!(
            ControlFramework::Soc2.slug(),
            ControlFramework::Iso27001.slug()
        );
    }

    #[test]
    fn evidence_kind_slugs_distinct() {
        let kinds = [
            EvidenceKind::Attestation,
            EvidenceKind::AuditLogEntry,
            EvidenceKind::PolicyDocument,
            EvidenceKind::ConfigSnapshot,
            EvidenceKind::DrDrillResult,
            EvidenceKind::IncidentEntry,
            EvidenceKind::Sbom,
            EvidenceKind::PentestReport,
            EvidenceKind::VendorAssessment,
            EvidenceKind::TrainingRecord,
        ];
        let mut seen = std::collections::HashSet::new();
        for k in kinds {
            assert!(seen.insert(k.slug()));
        }
    }
}
