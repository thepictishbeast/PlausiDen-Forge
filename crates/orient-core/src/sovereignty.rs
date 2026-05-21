//! Sovereignty orientation — the PSA (privacy / security / anonymity)
//! differentiator that distinguishes this substrate from
//! conventional ones.
//!
//! Per AVP-Doctrine `N_ORIENTATION_SUBSTRATE.md` § Orientation 11 +
//! `[[super-society-tech-stack]]`: every substrate entity scores
//! on sovereignty simultaneously with fast / reliable / robust /
//! secure. Multi-valued (an entity may be anonymous AND ephemeral
//! AND tor-compatible).
//!
//! Per `[[backward-compat-version-discipline]]`: enum is
//! `#[non_exhaustive]` — new sovereignty values are additive
//! (Cat 2) per the change taxonomy. Sovereignty values often
//! imply other axes (e.g. `local_only` implies the Resource
//! orientation's `network-frugal`) — those implications live in
//! the mapping tables per `MAPPING_TABLES.md`, not in this enum.
//!
//! Closes `#192 [orient-v4]`.

use serde::{Deserialize, Serialize};

/// PSA (privacy / security / anonymity) posture an entity declares.
/// Multi-valued in the manifest projection. Closed enum; new
/// values are added via doctrine + capability-request flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Sovereignty {
    /// No identifier links the entity / event / record to a person.
    /// Linkability bounded by design — even the operator cannot
    /// reconstruct the identity. Strongest posture.
    Anonymous,

    /// Identifier per session, no cross-session linkage. The user
    /// is recognizable for one session but a new session starts
    /// from zero.
    Pseudonymous,

    /// Identity is declared and persistent. Consent required.
    Identified,

    /// Data never leaves the substrate without explicit user
    /// consent. The default for any tenant-data-touching entity.
    Private,

    /// Data never persists to disk (in-memory only, scratch state).
    /// Implies Private; the mapping table records this.
    LocalOnly,

    /// Data expires per a declared TTL. Used for session caches,
    /// temporary uploads, OTPs.
    Ephemeral,

    /// Reachable over `.onion` addresses; no clearnet linkage is
    /// required for the entity to function. Drives Tor-mode
    /// site configurations.
    TorCompatible,

    /// Functions without network (Service Worker, local-first
    /// database, peer-to-peer transport). Tenants needing
    /// disaster-resilient operation pin this.
    OfflineCapable,

    /// Post-quantum secure (ML-DSA + ML-KEM where dual-stack
    /// applies). Per `[[super-society-tech-stack]]`: the substrate
    /// adopts PQ where standardized, not optional.
    PqSecure,

    /// Never transmits in cleartext. Mandatory for any payment /
    /// auth / health-data path.
    CleartextForbidden,

    /// Proves a claim about data without revealing the data
    /// itself (ZK proofs / commitments). Used by attestation,
    /// privacy-preserving analytics, sovereign credentials.
    ZeroKnowledge,
}

impl Sovereignty {
    /// Return the canonical snake_case slug. Stable across
    /// versions; consumers may match on this string.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Anonymous => "anonymous",
            Self::Pseudonymous => "pseudonymous",
            Self::Identified => "identified",
            Self::Private => "private",
            Self::LocalOnly => "local_only",
            Self::Ephemeral => "ephemeral",
            Self::TorCompatible => "tor_compatible",
            Self::OfflineCapable => "offline_capable",
            Self::PqSecure => "pq_secure",
            Self::CleartextForbidden => "cleartext_forbidden",
            Self::ZeroKnowledge => "zero_knowledge",
        }
    }

    /// All canonical Sovereignty values in stable iteration order.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::Anonymous,
            Self::Pseudonymous,
            Self::Identified,
            Self::Private,
            Self::LocalOnly,
            Self::Ephemeral,
            Self::TorCompatible,
            Self::OfflineCapable,
            Self::PqSecure,
            Self::CleartextForbidden,
            Self::ZeroKnowledge,
        ]
    }

    /// Parse a Sovereignty value from its canonical slug. Returns
    /// `None` for unknown / mistyped slugs — callers fail-closed
    /// per `[[deterministic-first-lfi-optional]]`.
    #[must_use]
    pub fn from_slug(s: &str) -> Option<Self> {
        Self::all().iter().copied().find(|v| v.slug() == s)
    }
}

impl std::fmt::Display for Sovereignty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_canonical_slugs_snake_case() {
        for s in Sovereignty::all() {
            let slug = s.slug();
            assert!(!slug.is_empty(), "{s:?} empty slug");
            assert!(
                slug.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "{s:?} slug {slug:?} not snake_case"
            );
        }
    }

    #[test]
    fn all_unique_slugs_count_eleven() {
        let mut seen = std::collections::HashSet::new();
        for s in Sovereignty::all() {
            assert!(seen.insert(s.slug()), "duplicate slug: {:?}", s.slug());
        }
        assert_eq!(seen.len(), 11, "expected 11 canonical sovereignty values");
    }

    #[test]
    fn from_slug_roundtrip() {
        for s in Sovereignty::all() {
            let back = Sovereignty::from_slug(s.slug()).expect("known");
            assert_eq!(back, *s);
        }
    }

    #[test]
    fn from_slug_rejects_unknown() {
        assert!(Sovereignty::from_slug("").is_none());
        assert!(Sovereignty::from_slug("Anonymous").is_none());
        assert!(Sovereignty::from_slug("local-only").is_none()); // dash, not underscore
    }

    #[test]
    fn serde_roundtrip_canonical() {
        for s in Sovereignty::all() {
            let json = serde_json::to_string(s).expect("serialize");
            let back: Sovereignty = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, *s);
        }
    }

    #[test]
    fn serde_pq_secure_slug() {
        let json = serde_json::to_string(&Sovereignty::PqSecure).expect("serialize");
        assert_eq!(json, "\"pq_secure\"");
    }
}
