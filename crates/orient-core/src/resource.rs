//! Resource orientation — cost / budget envelopes.
//!
//! Per AVP-Doctrine `N_ORIENTATION_SUBSTRATE.md` § Orientation 9:
//! multi-valued envelope an entity declares — what it spends + what
//! shape of cost it carries. Audit phases like `carbon_budget`,
//! `bundle_size`, and `perf_budget` consume these declarations to
//! enforce per-tier budgets per `MAPPING_TABLES.md` §
//! `risk-to-required-tests` + the perf rule cluster.
//!
//! Closes `#194 [orient-v6]`.

use serde::{Deserialize, Serialize};

/// Cost / budget envelope declaration. Multi-valued per entity.
/// `#[non_exhaustive]` for additivity per
/// `[[backward-compat-version-discipline]]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Resource {
    /// `O(1)` or `O(log n)` per invocation. Default for substrate
    /// operations.
    CpuCheap,
    /// `O(n)` bounded by an enum or small list size.
    CpuBounded,
    /// Requires explicit budget declaration; audit phase enforces.
    CpuExpensive,
    /// Allocates `≤` declared bytes. Bounded memory.
    MemoryBounded,
    /// Constant memory for arbitrary input. Streaming-friendly.
    MemoryStreaming,
    /// `≤ 1` round-trip per operation. Network-frugal envelope.
    NetworkFrugal,
    /// Batched but explicit budget. Multi-round-trip but capped.
    NetworkBursty,
    /// Declares CO2e per invocation per rule perf-006.
    CarbonBudgeted,
    /// `≤` declared bytes written. Disk-frugal envelope.
    DiskFrugal,
    /// Append-only, no reads. Audit / log envelope.
    DiskArchival,
}

impl Resource {
    /// Canonical kebab-case slug.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::CpuCheap => "cpu-cheap",
            Self::CpuBounded => "cpu-bounded",
            Self::CpuExpensive => "cpu-expensive",
            Self::MemoryBounded => "memory-bounded",
            Self::MemoryStreaming => "memory-streaming",
            Self::NetworkFrugal => "network-frugal",
            Self::NetworkBursty => "network-bursty",
            Self::CarbonBudgeted => "carbon-budgeted",
            Self::DiskFrugal => "disk-frugal",
            Self::DiskArchival => "disk-archival",
        }
    }

    /// All canonical values in stable iteration order.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::CpuCheap,
            Self::CpuBounded,
            Self::CpuExpensive,
            Self::MemoryBounded,
            Self::MemoryStreaming,
            Self::NetworkFrugal,
            Self::NetworkBursty,
            Self::CarbonBudgeted,
            Self::DiskFrugal,
            Self::DiskArchival,
        ]
    }

    /// Parse from canonical slug.
    #[must_use]
    pub fn from_slug(s: &str) -> Option<Self> {
        Self::all().iter().copied().find(|r| r.slug() == s)
    }
}

impl std::fmt::Display for Resource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_canonical_values() {
        assert_eq!(Resource::all().len(), 10);
    }

    #[test]
    fn all_unique_slugs() {
        let mut seen = std::collections::HashSet::new();
        for r in Resource::all() {
            assert!(seen.insert(r.slug()), "duplicate: {:?}", r.slug());
        }
    }

    #[test]
    fn from_slug_roundtrip() {
        for r in Resource::all() {
            assert_eq!(Resource::from_slug(r.slug()), Some(*r));
        }
    }

    #[test]
    fn serde_specific_slug() {
        assert_eq!(
            serde_json::to_string(&Resource::CarbonBudgeted).unwrap(),
            "\"carbon-budgeted\""
        );
    }
}
