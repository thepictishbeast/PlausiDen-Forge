//! `surveillance` — diversity metrics computed across the
//! fingerprint registry.
//!
//! Per `docs/SUBSTRATE_REFRAME_2026_05_21.md` § Forge fix 5
//! (diversity surveillance metrics + dashboard). Forge
//! continuously analyzes its own outputs and surfaces drift +
//! convergence as substrate-level findings.
//!
//! ## What this module exposes
//!
//! [`DiversityMetrics`] — structured snapshot of the platform's
//! current substrate-output diversity:
//!
//! * `entry_count` — total number of sites in the registry
//! * `tenant_count` — distinct tenants represented
//! * `primitive_usage` — aggregate primitive-kind occurrence
//!   counts across every site
//! * `variant_usage` — aggregate variant occurrence counts
//!   (variants per primitive)
//! * `token_override_usage` — aggregate token-override
//!   occurrence counts (sites that override the same token name
//!   contribute to the same bucket)
//! * `mean_pairwise_distance` — mean of `component_distance`
//!   over all pairs in the registry
//! * `min_pairwise_distance` / `max_pairwise_distance` — bounds
//! * `pairwise_distance_histogram` — bucketed distribution
//!
//! Each metric is computed deterministically over the registry's
//! entries — no random sampling. Pure function: same registry
//! state → same metrics.
//!
//! ## Surfacing drift
//!
//! [`DiversityMetrics::convergence_risk`] returns a categorical
//! risk level (`Low` / `Medium` / `High`) based on the metrics.
//! The forge-phases `DiversitySurveillancePhase` (separate task)
//! reads this and emits findings when the level is `Medium` or
//! `High`.
//!
//! ## Performance
//!
//! Pairwise distance is O(N²) over the registry size. For
//! N=1000 that's ~500k distance computations — well within the
//! "runs as a forge build phase" budget. If registry sizes grow
//! into the tens of thousands, this module should switch to a
//! sketch-based approximation (LSH or MinHash). Not needed at
//! current scale.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::fingerprint_registry::{read_all, FingerprintRegistryEntry, RegistryError};

/// Bucketed histogram of pairwise distances. Each entry maps a
/// distance bucket (low-inclusive) to the count of pairs that
/// landed in it. Buckets: 0, 1-2, 3-5, 6-10, 11-20, 21-40, 41+.
///
/// Kept as a `Vec<(u32, u32)>` rather than a fixed array so the
/// JSON wire shape lists buckets in order with their thresholds
/// visible.
pub type DistanceHistogram = Vec<(u32, u32)>;

/// Convergence-risk categorical level. Surfaced by
/// [`DiversityMetrics::convergence_risk`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum ConvergenceRisk {
    /// Mean pairwise distance is comfortably above the floor;
    /// no concentration on a small primitive subset; ready to
    /// pass surveillance gates.
    Low,
    /// Mean distance dropping toward floor OR primitive usage
    /// concentrating on a small subset; gate emits a warning.
    Medium,
    /// Mean distance below floor OR clear primitive monoculture;
    /// gate emits strict findings.
    High,
}

/// Structured snapshot of the platform's substrate-output
/// diversity at a moment in time. Computed from the fingerprint
/// registry.
///
/// JSON wire shape is stable; downstream surfaces (forge-cli
/// `forge surveillance`, MCP `forge_diversity_metrics`, ops
/// dashboard) consume this directly without intermediate types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DiversityMetrics {
    /// Number of registry entries used to compute the snapshot.
    pub entry_count: usize,
    /// Distinct tenants represented in the registry.
    pub tenant_count: usize,
    /// Primitive-kind occurrence counts across every site in
    /// the registry (key = primitive kind name, value = total
    /// occurrence count).
    pub primitive_usage: BTreeMap<String, u32>,
    /// Variant occurrence counts across the registry. Key is
    /// `"<primitive_kind>::<variant>"` so the same variant name
    /// under different primitives doesn't collide.
    pub variant_usage: BTreeMap<String, u32>,
    /// Token-override occurrence counts. Key is the token name;
    /// value is the number of sites that overrode it.
    pub token_override_usage: BTreeMap<String, u32>,
    /// Mean of `component_distance` over all unique pairs in
    /// the registry. `None` when entry_count < 2.
    pub mean_pairwise_distance: Option<f64>,
    /// Minimum observed pairwise distance. `None` when
    /// entry_count < 2.
    pub min_pairwise_distance: Option<u32>,
    /// Maximum observed pairwise distance. `None` when
    /// entry_count < 2.
    pub max_pairwise_distance: Option<u32>,
    /// Bucketed pairwise-distance distribution.
    pub pairwise_distance_histogram: DistanceHistogram,
}

impl DiversityMetrics {
    /// Compute metrics over every entry in the registry at
    /// `path`. Empty registry produces zero-everything metrics
    /// (not an error — empty platforms exist).
    ///
    /// `path` is read via the canonical
    /// [`crate::fingerprint_registry::read_all`] reader.
    /// Forwards any read error.
    pub fn compute_from_registry(path: &Path) -> Result<Self, RegistryError> {
        let entries = read_all(path)?;
        Ok(Self::from_entries(&entries))
    }

    /// Compute metrics over an explicit slice of entries. Used
    /// by tests + by callers that already hold an entries slice
    /// (avoids re-reading the registry file).
    #[must_use]
    pub fn from_entries(entries: &[FingerprintRegistryEntry]) -> Self {
        let entry_count = entries.len();
        let mut tenants = std::collections::BTreeSet::<&str>::new();
        let mut primitive_usage: BTreeMap<String, u32> = BTreeMap::new();
        let mut variant_usage: BTreeMap<String, u32> = BTreeMap::new();
        let mut token_override_usage: BTreeMap<String, u32> = BTreeMap::new();

        for entry in entries {
            tenants.insert(entry.tenant_id.as_str());
            for occ in &entry.fingerprint.primitives {
                *primitive_usage.entry(occ.kind.clone()).or_insert(0) += 1;
                let variant_key = format!("{}::{}", occ.kind, occ.variant);
                *variant_usage.entry(variant_key).or_insert(0) += 1;
            }
            for tok in &entry.fingerprint.token_overrides {
                *token_override_usage.entry(tok.name.clone()).or_insert(0) += 1;
            }
        }

        let mut distances: Vec<u32> = Vec::new();
        if entry_count >= 2 {
            for i in 0..entry_count {
                for j in (i + 1)..entry_count {
                    let d = entries[i]
                        .fingerprint
                        .component_distance(&entries[j].fingerprint);
                    distances.push(d);
                }
            }
        }

        let (mean, min, max) = if distances.is_empty() {
            (None, None, None)
        } else {
            let sum: u64 = distances.iter().map(|d| u64::from(*d)).sum();
            #[allow(clippy::cast_precision_loss)]
            let mean = (sum as f64) / (distances.len() as f64);
            let min = distances.iter().copied().min();
            let max = distances.iter().copied().max();
            (Some(mean), min, max)
        };

        Self {
            entry_count,
            tenant_count: tenants.len(),
            primitive_usage,
            variant_usage,
            token_override_usage,
            mean_pairwise_distance: mean,
            min_pairwise_distance: min,
            max_pairwise_distance: max,
            pairwise_distance_histogram: histogram_of(&distances),
        }
    }

    /// Categorical convergence-risk surface for the gate phase.
    ///
    /// Heuristic:
    ///
    /// * `entry_count < 2` → `Low` (nothing to compare; no risk
    ///   signal yet)
    /// * `mean_pairwise_distance < 3.0` → `High` (sites are
    ///   nearly identical on average — clear convergence)
    /// * `mean_pairwise_distance < 6.0` OR top primitive
    ///   represents >50% of all primitive occurrences → `Medium`
    /// * else → `Low`
    ///
    /// Thresholds are tunable as more empirical data lands;
    /// initial values are conservative.
    #[must_use]
    pub fn convergence_risk(&self) -> ConvergenceRisk {
        if self.entry_count < 2 {
            return ConvergenceRisk::Low;
        }
        if let Some(mean) = self.mean_pairwise_distance {
            if mean < 3.0 {
                return ConvergenceRisk::High;
            }
            if mean < 6.0 {
                return ConvergenceRisk::Medium;
            }
        }
        let total: u32 = self.primitive_usage.values().sum();
        if total > 0 {
            let top = self.primitive_usage.values().copied().max().unwrap_or(0);
            #[allow(clippy::cast_precision_loss)]
            let top_share = f64::from(top) / f64::from(total);
            if top_share > 0.5 {
                return ConvergenceRisk::Medium;
            }
        }
        ConvergenceRisk::Low
    }
}

fn histogram_of(distances: &[u32]) -> DistanceHistogram {
    // Buckets keyed by inclusive lower bound. The last bucket
    // (40) is open-ended.
    let buckets: &[u32] = &[0, 1, 3, 6, 11, 21, 41];
    let mut counts: Vec<(u32, u32)> = buckets.iter().map(|b| (*b, 0)).collect();
    for d in distances {
        for i in (0..buckets.len()).rev() {
            if *d >= buckets[i] {
                counts[i].1 += 1;
                break;
            }
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::{
        AssetDistribution, FingerprintSpec, PrimitiveOccurrence, SiteFingerprint, TokenOverride,
    };

    fn fp(primitives: &[(&str, &str)], tokens: &[(&str, &str)]) -> SiteFingerprint {
        SiteFingerprint {
            spec: FingerprintSpec::V1,
            primitives: primitives
                .iter()
                .map(|(k, v)| PrimitiveOccurrence {
                    kind: (*k).to_owned(),
                    variant: (*v).to_owned(),
                    page: "/".to_owned(),
                })
                .collect(),
            token_overrides: tokens
                .iter()
                .map(|(n, v)| TokenOverride::new(*n, *v))
                .collect(),
            silhouettes: BTreeMap::new(),
            rhythms: BTreeMap::new(),
            assets: AssetDistribution::default(),
        }
    }

    fn entry(
        seq: u64,
        tenant: &str,
        site: &str,
        fingerprint: SiteFingerprint,
    ) -> FingerprintRegistryEntry {
        FingerprintRegistryEntry {
            sequence: seq,
            hash: format!("h{seq:032x}"),
            prev_hash: if seq == 0 {
                None
            } else {
                Some(format!("h{:032x}", seq - 1))
            },
            timestamp: "2026-05-21T00:00:00Z".to_owned(),
            site_id: site.to_owned(),
            tenant_id: tenant.to_owned(),
            fingerprint,
            signature_b64: String::new(),
        }
    }

    #[test]
    fn empty_registry_produces_zero_metrics() {
        let m = DiversityMetrics::from_entries(&[]);
        assert_eq!(m.entry_count, 0);
        assert_eq!(m.tenant_count, 0);
        assert!(m.primitive_usage.is_empty());
        assert!(m.mean_pairwise_distance.is_none());
        assert!(matches!(m.convergence_risk(), ConvergenceRisk::Low));
    }

    #[test]
    fn single_entry_has_no_pairwise_distance() {
        let e = entry(0, "tenant-x", "site-1", fp(&[("hero", "centered")], &[]));
        let m = DiversityMetrics::from_entries(&[e]);
        assert_eq!(m.entry_count, 1);
        assert_eq!(m.tenant_count, 1);
        assert_eq!(m.primitive_usage.get("hero"), Some(&1));
        assert!(m.mean_pairwise_distance.is_none());
    }

    #[test]
    fn primitive_usage_aggregates_across_sites() {
        let entries = vec![
            entry(0, "tenant-x", "site-a", fp(&[("hero", "v1"), ("footer", "v1")], &[])),
            entry(1, "tenant-x", "site-b", fp(&[("hero", "v2"), ("footer", "v1")], &[])),
        ];
        let m = DiversityMetrics::from_entries(&entries);
        assert_eq!(m.primitive_usage.get("hero"), Some(&2));
        assert_eq!(m.primitive_usage.get("footer"), Some(&2));
        assert_eq!(m.variant_usage.get("hero::v1"), Some(&1));
        assert_eq!(m.variant_usage.get("hero::v2"), Some(&1));
        assert_eq!(m.variant_usage.get("footer::v1"), Some(&2));
    }

    #[test]
    fn token_override_usage_aggregates() {
        let entries = vec![
            entry(0, "t", "a", fp(&[], &[("primary", "#000")])),
            entry(1, "t", "b", fp(&[], &[("primary", "#111"), ("accent", "#222")])),
        ];
        let m = DiversityMetrics::from_entries(&entries);
        assert_eq!(m.token_override_usage.get("primary"), Some(&2));
        assert_eq!(m.token_override_usage.get("accent"), Some(&1));
    }

    #[test]
    fn tenant_count_dedupes() {
        let entries = vec![
            entry(0, "tenant-x", "a", fp(&[], &[])),
            entry(1, "tenant-x", "b", fp(&[], &[])),
            entry(2, "tenant-y", "c", fp(&[], &[])),
        ];
        let m = DiversityMetrics::from_entries(&entries);
        assert_eq!(m.tenant_count, 2);
    }

    #[test]
    fn convergence_risk_high_when_sites_identical() {
        // Two identical sites → pairwise distance 0 → mean 0 →
        // high convergence risk.
        let f = fp(&[("hero", "centered")], &[]);
        let entries = vec![
            entry(0, "t", "a", f.clone()),
            entry(1, "t", "b", f.clone()),
        ];
        let m = DiversityMetrics::from_entries(&entries);
        assert!(matches!(m.convergence_risk(), ConvergenceRisk::High));
    }

    #[test]
    fn histogram_buckets_distances() {
        let m = DiversityMetrics::from_entries(&[
            entry(0, "t", "a", fp(&[("hero", "v1")], &[])),
            entry(1, "t", "b", fp(&[("hero", "v1")], &[])),
        ]);
        // Two identical → 1 pair at distance 0.
        let zero_bucket = m
            .pairwise_distance_histogram
            .iter()
            .find(|(b, _)| *b == 0)
            .map(|(_, c)| *c);
        assert_eq!(zero_bucket, Some(1));
    }
}
