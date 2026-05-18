//! `observability-core` — typed OpenTelemetry-compatible span
//! shape + immutable hash-chained audit log.
//!
//! Per `PLATFORM_ROADMAP.md` §8 + `super_society_tech_stack`:
//! every platform observation is either a structured span (Otel
//! semantic-conventions compatible) or an audit log entry
//! cryptographically linked to its predecessor. No free-form
//! `info!()` calls drift into the load-bearing signal path.
//!
//! ### Two complementary surfaces
//!
//! **Tracing half** — [`TraceId`] + [`SpanId`] + [`Span`] mirror
//! the W3C Trace Context spec + the OpenTelemetry semantic
//! conventions. Consumers map to whichever exporter they prefer
//! (otlp / zipkin / jaeger / stdout); this crate defines the
//! cross-exporter shape so emitter + consumer agree on
//! attribute names + types.
//!
//! **Audit half** — [`AuditEntry`] + [`AuditChain`] form an
//! append-only hash-chained log: every entry's
//! [`AuditEntry::prev_hash`] is the SHA-256 of the previous
//! entry's canonical-form bytes, so any tampering with
//! historical entries invalidates every following entry's hash.
//! [`AuditChain::verify`] walks the chain.
//!
//! ### Why one crate
//!
//! Tracing + auditing are both "what happened, recorded for
//! later inspection." The shapes are different — spans are
//! time-bounded, audit entries are point-in-time — but the
//! consumer surfaces overlap heavily. Shipping both behind one
//! typed contract keeps the emit + consume + verify story
//! coherent across the platform.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ============================================================
// TRACING HALF
// ============================================================

/// 16-byte trace identifier (per W3C Trace Context). Stored as a
/// 32-char lowercase hex string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TraceId(String);

impl TraceId {
    /// Validate + construct from a 32-char hex string.
    pub fn parse(s: impl AsRef<str>) -> Result<Self, ObserveError> {
        let s = s.as_ref();
        if s.len() != 32 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ObserveError::InvalidTraceId(format!(
                "{s:?} not a 32-char hex id"
            )));
        }
        Ok(Self(s.to_ascii_lowercase()))
    }

    /// Raw view.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 8-byte span identifier (per W3C Trace Context). Stored as a
/// 16-char lowercase hex string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpanId(String);

impl SpanId {
    /// Validate + construct from a 16-char hex string.
    pub fn parse(s: impl AsRef<str>) -> Result<Self, ObserveError> {
        let s = s.as_ref();
        if s.len() != 16 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ObserveError::InvalidSpanId(format!(
                "{s:?} not a 16-char hex id"
            )));
        }
        Ok(Self(s.to_ascii_lowercase()))
    }

    /// Raw view.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Span status — closed enum mirroring OpenTelemetry
/// `SpanStatusCode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SpanStatus {
    /// Status not yet set (default).
    #[default]
    Unset,
    /// Operation completed successfully.
    Ok,
    /// Operation failed — error attribute usually carries
    /// human-readable detail.
    Error,
}

impl SpanStatus {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Unset => "unset",
            Self::Ok => "ok",
            Self::Error => "error",
        }
    }
}

/// Span kind — closed enum mirroring OpenTelemetry `SpanKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SpanKind {
    /// Internal operation (default).
    #[default]
    Internal,
    /// Server-side handler for an incoming request.
    Server,
    /// Client-side request to an external service.
    Client,
    /// Asynchronous message producer.
    Producer,
    /// Asynchronous message consumer.
    Consumer,
}

impl SpanKind {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Internal => "internal",
            Self::Server => "server",
            Self::Client => "client",
            Self::Producer => "producer",
            Self::Consumer => "consumer",
        }
    }
}

/// A finished span. Emitter constructs one of these at span-end
/// and hands to the exporter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Span {
    /// Trace this span belongs to.
    pub trace_id: TraceId,
    /// Stable per-span id.
    pub span_id: SpanId,
    /// Optional parent span id (`None` for the root).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<SpanId>,
    /// Human-readable operation name (e.g. `"forge.build"`,
    /// `"crawler.journey.run"`).
    pub name: String,
    /// Span kind.
    pub kind: SpanKind,
    /// Span status.
    pub status: SpanStatus,
    /// ISO-8601 timestamp when the span began.
    pub start: time::OffsetDateTime,
    /// ISO-8601 timestamp when the span ended.
    pub end: time::OffsetDateTime,
    /// Typed attributes (Otel semantic-conventions slugs as keys).
    #[serde(default)]
    pub attributes: BTreeMap<String, AttributeValue>,
    /// Events recorded during the span.
    #[serde(default)]
    pub events: Vec<SpanEvent>,
}

impl Span {
    /// Span duration in milliseconds, or 0 if end < start
    /// (operator bug — clock skew handled by clamping to 0
    /// rather than panicking).
    pub fn duration_ms(&self) -> i64 {
        ((self.end - self.start).whole_milliseconds() as i64).max(0)
    }
}

/// One typed attribute value. Closed enum so consumers don't need
/// to reparse arbitrary JSON shapes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AttributeValue {
    /// String value.
    String {
        /// The string.
        value: String,
    },
    /// 64-bit integer.
    Int {
        /// The integer.
        value: i64,
    },
    /// Boolean.
    Bool {
        /// The boolean.
        value: bool,
    },
    /// Double-precision float (NaN/Infinity not permitted at the
    /// serialization layer; tested elsewhere).
    Float {
        /// The float.
        value: f64,
    },
}

/// One event recorded mid-span.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SpanEvent {
    /// ISO-8601 timestamp.
    pub at: time::OffsetDateTime,
    /// Event name.
    pub name: String,
    /// Optional typed attributes.
    #[serde(default)]
    pub attributes: BTreeMap<String, AttributeValue>,
}

// ============================================================
// AUDIT HALF — immutable hash-chained log
// ============================================================

/// SHA-256 of an audit entry's canonical-form bytes. 64-char
/// lowercase hex.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntryHash(String);

impl EntryHash {
    /// Compute SHA-256 of arbitrary bytes.
    pub fn of_bytes(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        Self(digest.iter().map(|b| format!("{b:02x}")).collect())
    }

    /// Parse from 64-char hex.
    pub fn parse(s: impl AsRef<str>) -> Result<Self, ObserveError> {
        let s = s.as_ref();
        if s.len() != 64 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ObserveError::InvalidEntryHash(format!(
                "{s:?} not a 64-char hex digest"
            )));
        }
        Ok(Self(s.to_ascii_lowercase()))
    }

    /// The canonical "no predecessor" hash — all-zeroes. Used as
    /// the [`AuditEntry::prev_hash`] of the first entry in a chain.
    pub fn zero() -> Self {
        Self("0".repeat(64))
    }

    /// Raw hex view.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One append-only audit log entry. Hash-chained: each entry's
/// `prev_hash` is the [`EntryHash`] of the previous entry, and
/// `entry_hash` is the SHA-256 of `(prev_hash, sequence, actor,
/// at, kind, payload)`.
///
/// Tampering with any historical entry's payload invalidates that
/// entry's `entry_hash`, which invalidates every following entry's
/// `prev_hash`, which detection [`AuditChain::verify`] runs at
/// audit-time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AuditEntry {
    /// Sequence number within the chain (0 for the first entry).
    pub sequence: u64,
    /// SHA-256 of the previous entry, or [`EntryHash::zero`] if
    /// this is the first.
    pub prev_hash: EntryHash,
    /// SHA-256 of THIS entry's canonical form (deterministic
    /// linearization of the other fields).
    pub entry_hash: EntryHash,
    /// Actor responsible — operator handle, service name, or
    /// `"system"`.
    pub actor: String,
    /// ISO-8601 timestamp.
    pub at: time::OffsetDateTime,
    /// Event kind slug (e.g. `"forge.build.start"`,
    /// `"cms.page.publish"`, `"deploy.target.add"`).
    pub kind: String,
    /// Free-form JSON payload — typed at consumer time via the
    /// kind discriminator.
    pub payload: serde_json::Value,
}

impl AuditEntry {
    /// Build the canonical-form bytes hashed into `entry_hash`.
    /// Order is fixed so re-serialization is deterministic.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let canonical = serde_json::json!({
            "sequence": self.sequence,
            "prev_hash": self.prev_hash.as_str(),
            "actor": self.actor,
            "at": self.at.unix_timestamp(),
            "kind": self.kind,
            "payload": self.payload,
        });
        serde_json::to_vec(&canonical).expect("canonical serialization")
    }

    /// Recompute the `entry_hash` from current fields. Used by
    /// [`AuditChain::verify`] to detect tampering.
    pub fn recompute_entry_hash(&self) -> EntryHash {
        EntryHash::of_bytes(&self.canonical_bytes())
    }
}

/// Ordered chain of audit entries. Append-only at construction;
/// verification walks the chain looking for hash discontinuities.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AuditChain {
    /// Entries in append order (`sequence` field must match
    /// position).
    #[serde(default)]
    pub entries: Vec<AuditEntry>,
}

impl AuditChain {
    /// Empty chain.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a new entry. Sequence + prev_hash + entry_hash are
    /// computed automatically from the chain state + the
    /// caller-supplied actor/kind/payload/at.
    pub fn append(
        &mut self,
        actor: impl Into<String>,
        at: time::OffsetDateTime,
        kind: impl Into<String>,
        payload: serde_json::Value,
    ) -> &AuditEntry {
        let sequence = self.entries.len() as u64;
        let prev_hash = match self.entries.last() {
            Some(e) => e.entry_hash.clone(),
            None => EntryHash::zero(),
        };
        let mut entry = AuditEntry {
            sequence,
            prev_hash,
            entry_hash: EntryHash::zero(), // placeholder; recompute below
            actor: actor.into(),
            at,
            kind: kind.into(),
            payload,
        };
        entry.entry_hash = entry.recompute_entry_hash();
        self.entries.push(entry);
        self.entries.last().unwrap()
    }

    /// Verify the chain is internally consistent — sequence
    /// monotonicity, prev_hash linkage, entry_hash freshness.
    /// Returns the index of the first bad entry, or `Ok(())` if
    /// the chain is clean.
    pub fn verify(&self) -> Result<(), ObserveError> {
        let mut expected_prev = EntryHash::zero();
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.sequence != i as u64 {
                return Err(ObserveError::ChainSequenceMismatch {
                    index: i,
                    declared: entry.sequence,
                });
            }
            if entry.prev_hash != expected_prev {
                return Err(ObserveError::ChainBroken {
                    index: i,
                    expected: expected_prev.clone(),
                    got: entry.prev_hash.clone(),
                });
            }
            let recomputed = entry.recompute_entry_hash();
            if recomputed != entry.entry_hash {
                return Err(ObserveError::EntryTampered {
                    index: i,
                    declared: entry.entry_hash.clone(),
                    recomputed,
                });
            }
            expected_prev = entry.entry_hash.clone();
        }
        Ok(())
    }
}

// ============================================================
// ERRORS
// ============================================================

/// Errors at the observability boundary.
#[derive(Debug, thiserror::Error)]
pub enum ObserveError {
    /// TraceId failed shape validation.
    #[error("invalid trace id: {0}")]
    InvalidTraceId(String),
    /// SpanId failed shape validation.
    #[error("invalid span id: {0}")]
    InvalidSpanId(String),
    /// Audit entry hash failed shape validation.
    #[error("invalid entry hash: {0}")]
    InvalidEntryHash(String),
    /// Audit chain entry's sequence doesn't match its position.
    #[error("audit entry at index {index} declares sequence {declared}")]
    ChainSequenceMismatch {
        /// Position in the chain.
        index: usize,
        /// Declared (wrong) sequence.
        declared: u64,
    },
    /// Audit entry's prev_hash doesn't match the preceding
    /// entry's entry_hash.
    #[error(
        "audit chain broken at index {index}: prev_hash {got:?} doesn't match expected {expected:?}"
    )]
    ChainBroken {
        /// Position in the chain.
        index: usize,
        /// What prev_hash should have been.
        expected: EntryHash,
        /// What it actually was.
        got: EntryHash,
    },
    /// Audit entry's entry_hash doesn't match the recomputed
    /// hash of its current fields. Indicates a tampered payload.
    #[error("audit entry {index} tampered: declared {declared:?}, recomputed {recomputed:?}")]
    EntryTampered {
        /// Position in the chain.
        index: usize,
        /// Hash declared in the entry.
        declared: EntryHash,
        /// Hash recomputed from the entry's current fields.
        recomputed: EntryHash,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn ts(s: &str) -> time::OffsetDateTime {
        time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).unwrap()
    }

    #[test]
    fn trace_id_validates_shape() {
        assert!(TraceId::parse(&"a".repeat(32)).is_ok());
        assert!(TraceId::parse(&"a".repeat(31)).is_err());
        assert!(TraceId::parse(&"z".repeat(32)).is_err());
    }

    #[test]
    fn span_id_validates_shape() {
        assert!(SpanId::parse(&"a".repeat(16)).is_ok());
        assert!(SpanId::parse(&"a".repeat(15)).is_err());
        assert!(SpanId::parse(&"z".repeat(16)).is_err());
    }

    #[test]
    fn span_duration_ms_handles_end_before_start() {
        let s = Span {
            trace_id: TraceId::parse(&"a".repeat(32)).unwrap(),
            span_id: SpanId::parse(&"b".repeat(16)).unwrap(),
            parent_span_id: None,
            name: "test".into(),
            kind: SpanKind::Internal,
            status: SpanStatus::Ok,
            start: datetime!(2026-05-18 12:00:00 UTC),
            end: datetime!(2026-05-18 11:00:00 UTC),
            attributes: BTreeMap::new(),
            events: vec![],
        };
        assert_eq!(s.duration_ms(), 0);
    }

    #[test]
    fn span_duration_ms_positive() {
        let s = Span {
            trace_id: TraceId::parse(&"a".repeat(32)).unwrap(),
            span_id: SpanId::parse(&"b".repeat(16)).unwrap(),
            parent_span_id: None,
            name: "test".into(),
            kind: SpanKind::Internal,
            status: SpanStatus::Ok,
            start: datetime!(2026-05-18 12:00:00.000 UTC),
            end: datetime!(2026-05-18 12:00:01.250 UTC),
            attributes: BTreeMap::new(),
            events: vec![],
        };
        assert_eq!(s.duration_ms(), 1250);
    }

    #[test]
    fn entry_hash_of_bytes_is_deterministic() {
        let a = EntryHash::of_bytes(b"hello");
        let b = EntryHash::of_bytes(b"hello");
        assert_eq!(a, b);
        assert_ne!(a, EntryHash::of_bytes(b"world"));
        assert_eq!(a.as_str().len(), 64);
    }

    #[test]
    fn entry_hash_zero_is_64_zeros() {
        assert_eq!(EntryHash::zero().as_str(), &"0".repeat(64));
    }

    #[test]
    fn audit_chain_append_links_hashes() {
        let mut chain = AuditChain::new();
        chain.append(
            "alice",
            ts("2026-05-18T12:00:00Z"),
            "forge.build.start",
            serde_json::json!({"site": "x"}),
        );
        chain.append(
            "alice",
            ts("2026-05-18T12:00:05Z"),
            "forge.build.end",
            serde_json::json!({"site": "x", "passed": true}),
        );
        assert_eq!(chain.entries.len(), 2);
        // Second entry's prev_hash == first entry's entry_hash.
        assert_eq!(chain.entries[1].prev_hash, chain.entries[0].entry_hash);
        // First entry's prev_hash is zero.
        assert_eq!(chain.entries[0].prev_hash, EntryHash::zero());
        // Sequences are 0, 1.
        assert_eq!(chain.entries[0].sequence, 0);
        assert_eq!(chain.entries[1].sequence, 1);
    }

    #[test]
    fn audit_chain_verify_clean_chain_ok() {
        let mut chain = AuditChain::new();
        for i in 0..5 {
            chain.append(
                "alice",
                ts("2026-05-18T12:00:00Z"),
                "test.event",
                serde_json::json!({"i": i}),
            );
        }
        chain.verify().expect("clean chain should verify");
    }

    #[test]
    fn audit_chain_verify_detects_tampered_payload() {
        let mut chain = AuditChain::new();
        chain.append(
            "alice",
            ts("2026-05-18T12:00:00Z"),
            "test.event",
            serde_json::json!({"value": 1}),
        );
        chain.append(
            "alice",
            ts("2026-05-18T12:00:01Z"),
            "test.event",
            serde_json::json!({"value": 2}),
        );
        // Tamper with payload of entry 0; entry_hash is now stale.
        chain.entries[0].payload = serde_json::json!({"value": 999});
        let err = chain.verify().unwrap_err();
        assert!(matches!(err, ObserveError::EntryTampered { index: 0, .. }));
    }

    #[test]
    fn audit_chain_verify_detects_broken_link() {
        let mut chain = AuditChain::new();
        chain.append(
            "alice",
            ts("2026-05-18T12:00:00Z"),
            "a",
            serde_json::json!({}),
        );
        chain.append(
            "alice",
            ts("2026-05-18T12:00:01Z"),
            "b",
            serde_json::json!({}),
        );
        // Forge a broken link by replacing entry 1's prev_hash.
        chain.entries[1].prev_hash = EntryHash::of_bytes(b"forged");
        let err = chain.verify().unwrap_err();
        assert!(matches!(err, ObserveError::ChainBroken { index: 1, .. }));
    }

    #[test]
    fn audit_chain_verify_detects_sequence_skip() {
        let mut chain = AuditChain::new();
        chain.append(
            "alice",
            ts("2026-05-18T12:00:00Z"),
            "a",
            serde_json::json!({}),
        );
        chain.entries[0].sequence = 42;
        let err = chain.verify().unwrap_err();
        assert!(matches!(
            err,
            ObserveError::ChainSequenceMismatch {
                index: 0,
                declared: 42
            }
        ));
    }

    #[test]
    fn span_kind_and_status_slugs_distinct() {
        let kinds = [
            SpanKind::Internal,
            SpanKind::Server,
            SpanKind::Client,
            SpanKind::Producer,
            SpanKind::Consumer,
        ];
        let mut seen = std::collections::HashSet::new();
        for k in kinds {
            assert!(seen.insert(k.slug()));
        }
        let statuses = [SpanStatus::Unset, SpanStatus::Ok, SpanStatus::Error];
        let mut seen2 = std::collections::HashSet::new();
        for s in statuses {
            assert!(seen2.insert(s.slug()));
        }
    }

    #[test]
    fn span_serde_round_trips() {
        let mut attrs = BTreeMap::new();
        attrs.insert(
            "service.name".into(),
            AttributeValue::String {
                value: "forge-cli".into(),
            },
        );
        attrs.insert(
            "build.findings.count".into(),
            AttributeValue::Int { value: 42 },
        );
        let s = Span {
            trace_id: TraceId::parse(&"a".repeat(32)).unwrap(),
            span_id: SpanId::parse(&"b".repeat(16)).unwrap(),
            parent_span_id: None,
            name: "forge.build".into(),
            kind: SpanKind::Internal,
            status: SpanStatus::Ok,
            start: ts("2026-05-18T12:00:00Z"),
            end: ts("2026-05-18T12:00:01Z"),
            attributes: attrs,
            events: vec![],
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Span = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn attribute_value_internal_tag() {
        let v = AttributeValue::String { value: "hi".into() };
        let s = serde_json::to_string(&v).unwrap();
        assert!(s.contains("\"kind\":\"string\""));
    }

    // T97: slug-vs-serde-wire regression guard.
    #[test]
    fn slug_matches_serde_wire_across_all_enums() {
        for v in [SpanStatus::Unset, SpanStatus::Ok, SpanStatus::Error] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            SpanKind::Internal,
            SpanKind::Server,
            SpanKind::Client,
            SpanKind::Producer,
            SpanKind::Consumer,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
    }
}
