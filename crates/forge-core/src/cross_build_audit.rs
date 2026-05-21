//! `cross_build_audit` — general capability to verify properties
//! across the fingerprint registry.
//!
//! Per `docs/SUBSTRATE_REFRAME_2026_05_21.md` § Forge fix 6
//! (cross-build audit infrastructure). Beyond the registry
//! ([`crate::fingerprint_registry`], #232 / #349) and the
//! per-site uniqueness gate (forge-phases::uniqueness_gate,
//! #350), this module exposes a typed predicate runner so
//! arbitrary cross-build properties can be enforced as part of
//! the build pipeline.
//!
//! ## The predicate model
//!
//! Each predicate implements [`CrossBuildPredicate`]: it
//! receives the full registry entry slice (already filtered to
//! the active tenant scope when applicable) and produces zero
//! or more typed [`CrossBuildFinding`]s. Findings carry
//! structured severity + remediation, so downstream phases can
//! route them to the standard `Severity::Strict` / `Warn`
//! channels.
//!
//! ## Shipped predicates
//!
//! * [`TenantShareCap`] — refuses tenants whose recent sites
//!   share more than `max_shared_share` of their structural
//!   signature on average.
//! * [`VocabularyUtilizationFloor`] — refuses a registry where
//!   aggregate primitive usage spans fewer than `min_distinct`
//!   distinct primitive kinds.
//! * [`GradientRecencyCap`] — refuses sites whose token-override
//!   declared gradient pool entry matches one used by any of
//!   the most recent `lookback` sites.
//! * [`WithinSiteVariantCap`] — refuses sites that use the same
//!   (primitive, variant) pair more than `max_per_site`
//!   occurrences within a single site.
//!
//! Predicates compose: callers build a list, invoke
//! [`run_predicates`] once, and route the aggregated findings.
//!
//! ## Why this lives in forge-core, not forge-phases
//!
//! The predicate types are part of the substrate's public
//! contract — MCP tools, CLI subcommands, and the surveillance
//! dashboard all consume them. forge-phases consumes this
//! module via a phase wrapper but the core logic is reusable
//! anywhere registry inspection happens.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::fingerprint_registry::FingerprintRegistryEntry;

/// Severity tier for a cross-build finding. Mirrors the
/// substrate's standard severity ladder so phase wrappers can
/// route directly into `forge_core::Severity` without
/// translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum FindingSeverity {
    /// Informational — no action required.
    Info,
    /// Warning — surface to operator but don't block ship.
    Warn,
    /// Strict — blocks ship; operator must remediate.
    Strict,
}

/// One structured finding produced by a predicate evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CrossBuildFinding {
    /// Predicate identifier (kebab-case) for telemetry +
    /// dashboards. Stable wire identifier.
    pub predicate: String,
    /// Severity tier.
    pub severity: FindingSeverity,
    /// Human-readable message describing what was detected.
    pub message: String,
    /// Optional remediation hint — what the operator should
    /// change to clear the finding.
    pub remediation: Option<String>,
    /// Optional list of registry sequences involved in the
    /// finding (e.g., the pair of sites that share too much
    /// signature). Empty when the finding is platform-wide.
    pub sequences: Vec<u64>,
}

/// Trait implemented by every cross-build predicate. The
/// `eval` contract: pure function over the slice of entries,
/// returning zero or more findings. No side effects; no I/O;
/// no shared state between calls. Implementations must be
/// `Send + Sync` so the runner can parallelize when N grows.
pub trait CrossBuildPredicate: Send + Sync {
    /// Stable kebab-case predicate name. Recorded in every
    /// emitted finding's `predicate` field.
    fn name(&self) -> &'static str;

    /// Evaluate against the registry entries.
    fn eval(&self, entries: &[FingerprintRegistryEntry]) -> Vec<CrossBuildFinding>;
}

/// Run every predicate over the entries; concatenate findings.
/// Findings preserve predicate order (caller-supplied
/// ordering) and within-predicate order.
#[must_use]
pub fn run_predicates(
    predicates: &[Box<dyn CrossBuildPredicate>],
    entries: &[FingerprintRegistryEntry],
) -> Vec<CrossBuildFinding> {
    let mut out = Vec::new();
    for p in predicates {
        out.extend(p.eval(entries));
    }
    out
}

/// Predicate: tenant-level signature-share cap.
///
/// For each tenant with ≥2 sites, computes the mean pairwise
/// distance over the tenant's sites. When the mean is below
/// `min_mean_distance`, emits a finding (severity configurable
/// via `severity`).
#[derive(Debug, Clone)]
pub struct TenantShareCap {
    /// Mean pairwise distance below which the tenant fails.
    /// Distance metric is [`crate::fingerprint::SiteFingerprint::component_distance`].
    pub min_mean_distance: f64,
    /// Severity to assign when a tenant trips the cap.
    pub severity: FindingSeverity,
}

impl CrossBuildPredicate for TenantShareCap {
    fn name(&self) -> &'static str {
        "tenant-share-cap"
    }

    fn eval(&self, entries: &[FingerprintRegistryEntry]) -> Vec<CrossBuildFinding> {
        let mut by_tenant: BTreeMap<&str, Vec<&FingerprintRegistryEntry>> = BTreeMap::new();
        for e in entries {
            by_tenant.entry(e.tenant_id.as_str()).or_default().push(e);
        }
        let mut out = Vec::new();
        for (tenant, tenant_entries) in &by_tenant {
            if tenant_entries.len() < 2 {
                continue;
            }
            let mut total: u64 = 0;
            let mut count: u64 = 0;
            for i in 0..tenant_entries.len() {
                for j in (i + 1)..tenant_entries.len() {
                    let d = tenant_entries[i]
                        .fingerprint
                        .component_distance(&tenant_entries[j].fingerprint);
                    total += u64::from(d);
                    count += 1;
                }
            }
            if count == 0 {
                continue;
            }
            #[allow(clippy::cast_precision_loss)]
            let mean = (total as f64) / (count as f64);
            if mean < self.min_mean_distance {
                let seqs: Vec<u64> = tenant_entries.iter().map(|e| e.sequence).collect();
                out.push(CrossBuildFinding {
                    predicate: self.name().to_owned(),
                    severity: self.severity,
                    message: format!(
                        "tenant {tenant:?} portfolio mean pairwise distance {mean:.2} < cap {:.2} — sites within this tenant are converging",
                        self.min_mean_distance
                    ),
                    remediation: Some(format!(
                        "differentiate the tenant's sites: vary hero variant, footer composition, gradient pool entry, or primitive sequence so mean distance crosses {:.2}",
                        self.min_mean_distance
                    )),
                    sequences: seqs,
                });
            }
        }
        out
    }
}

/// Predicate: platform-level vocabulary-utilization floor.
///
/// Counts distinct primitive kinds present across the entire
/// registry. When fewer than `min_distinct` kinds are present,
/// emits a finding indicating the substrate's vocabulary is
/// under-utilized — either Claude / operators aren't reaching
/// for breadth, or the substrate's primitive surface is too
/// narrow.
#[derive(Debug, Clone)]
pub struct VocabularyUtilizationFloor {
    /// Minimum number of distinct primitive kinds the
    /// platform's outputs must collectively use.
    pub min_distinct: usize,
    /// Severity to assign when the floor is breached.
    pub severity: FindingSeverity,
}

impl CrossBuildPredicate for VocabularyUtilizationFloor {
    fn name(&self) -> &'static str {
        "vocabulary-utilization-floor"
    }

    fn eval(&self, entries: &[FingerprintRegistryEntry]) -> Vec<CrossBuildFinding> {
        if entries.is_empty() {
            return Vec::new();
        }
        let mut kinds: BTreeSet<&str> = BTreeSet::new();
        for e in entries {
            for occ in &e.fingerprint.primitives {
                kinds.insert(occ.kind.as_str());
            }
        }
        if kinds.len() < self.min_distinct {
            return vec![CrossBuildFinding {
                predicate: self.name().to_owned(),
                severity: self.severity,
                message: format!(
                    "platform vocabulary spans {} distinct primitive kinds across {} sites — floor is {}",
                    kinds.len(),
                    entries.len(),
                    self.min_distinct
                ),
                remediation: Some(
                    "either reach for more of the substrate's primitive surface in new sites OR investigate whether the substrate's primitive count is too narrow (substrate-roadmap signal)"
                        .to_owned(),
                ),
                sequences: Vec::new(),
            }];
        }
        Vec::new()
    }
}

/// Predicate: gradient-recency cap.
///
/// Reads the `data-pool-name` style token override that the
/// default-fragmentation gradient cascade emits (or any
/// gradient name in `token_overrides`) and refuses entries
/// whose chosen gradient matches one used by any of the
/// previous `lookback` entries. Stops sites in a single
/// tenant from rotating through the same 2-3 gradients on
/// repeat.
///
/// Token override key examined: `gradient_pool_name`. Sites
/// without this key are skipped (no signal).
#[derive(Debug, Clone)]
pub struct GradientRecencyCap {
    /// How many most-recent entries to compare against.
    pub lookback: usize,
    /// Severity to assign when a site reuses a recent gradient.
    pub severity: FindingSeverity,
}

impl CrossBuildPredicate for GradientRecencyCap {
    fn name(&self) -> &'static str {
        "gradient-recency-cap"
    }

    fn eval(&self, entries: &[FingerprintRegistryEntry]) -> Vec<CrossBuildFinding> {
        if entries.len() < 2 {
            return Vec::new();
        }
        let gradient_of = |e: &FingerprintRegistryEntry| {
            e.fingerprint
                .token_overrides
                .iter()
                .find(|t| t.name == "gradient_pool_name")
                .map(|t| t.value.clone())
        };
        let mut out = Vec::new();
        for (idx, e) in entries.iter().enumerate() {
            let Some(this_gradient) = gradient_of(e) else {
                continue;
            };
            let start = idx.saturating_sub(self.lookback);
            for prev in &entries[start..idx] {
                if let Some(prev_gradient) = gradient_of(prev) {
                    if prev_gradient == this_gradient && prev.sequence != e.sequence {
                        out.push(CrossBuildFinding {
                            predicate: self.name().to_owned(),
                            severity: self.severity,
                            message: format!(
                                "site {site:?} (seq {seq}) uses gradient {grad:?} also used by site {prev_site:?} (seq {prev_seq}) within the last {lookback} entries",
                                site = e.site_id,
                                seq = e.sequence,
                                grad = this_gradient,
                                prev_site = prev.site_id,
                                prev_seq = prev.sequence,
                                lookback = self.lookback,
                            ),
                            remediation: Some(format!(
                                "select a different entry from loom_tokens::gradient_pool, OR pass the recently-used gradient names to select_for_identity's recently_used hint so the pool walks forward to an unused entry"
                            )),
                            sequences: vec![prev.sequence, e.sequence],
                        });
                        break;
                    }
                }
            }
        }
        out
    }
}

/// Predicate: within-site (primitive, variant) cap.
///
/// For each site, counts occurrences of every
/// `(primitive_kind, variant)` pair and refuses sites that
/// exceed `max_per_site` of any single pair. Prevents the
/// "same hero shape repeated five times on one page" failure
/// mode.
#[derive(Debug, Clone)]
pub struct WithinSiteVariantCap {
    /// Maximum allowed occurrences of any single
    /// `(primitive_kind, variant)` pair within one site.
    pub max_per_site: u32,
    /// Severity to assign when the cap is breached.
    pub severity: FindingSeverity,
}

impl CrossBuildPredicate for WithinSiteVariantCap {
    fn name(&self) -> &'static str {
        "within-site-variant-cap"
    }

    fn eval(&self, entries: &[FingerprintRegistryEntry]) -> Vec<CrossBuildFinding> {
        let mut out = Vec::new();
        for e in entries {
            let mut counts: BTreeMap<(String, String), u32> = BTreeMap::new();
            for occ in &e.fingerprint.primitives {
                *counts
                    .entry((occ.kind.clone(), occ.variant.clone()))
                    .or_insert(0) += 1;
            }
            for ((kind, variant), count) in &counts {
                if *count > self.max_per_site {
                    out.push(CrossBuildFinding {
                        predicate: self.name().to_owned(),
                        severity: self.severity,
                        message: format!(
                            "site {site:?} uses ({kind:?}, {variant:?}) {count} times — cap is {cap}",
                            site = e.site_id,
                            cap = self.max_per_site
                        ),
                        remediation: Some(
                            "vary the variant choice across occurrences, OR split into different primitive kinds entirely if the same shape repeats this often"
                                .to_owned(),
                        ),
                        sequences: vec![e.sequence],
                    });
                }
            }
        }
        out
    }
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
    fn tenant_share_cap_fires_when_tenant_sites_converge() {
        let f = fp(&[("hero", "v1"), ("footer", "v1")], &[]);
        let entries = vec![
            entry(0, "tenant-x", "a", f.clone()),
            entry(1, "tenant-x", "b", f),
        ];
        let pred = TenantShareCap {
            min_mean_distance: 3.0,
            severity: FindingSeverity::Strict,
        };
        let findings = pred.eval(&entries);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, FindingSeverity::Strict);
        assert_eq!(findings[0].predicate, "tenant-share-cap");
        assert!(findings[0].message.contains("tenant-x"));
    }

    #[test]
    fn tenant_share_cap_passes_when_sites_diverge() {
        let entries = vec![
            entry(0, "t", "a", fp(&[("hero", "v1"), ("alpha", "v1"), ("beta", "v1"), ("gamma", "v1"), ("delta", "v1")], &[])),
            entry(1, "t", "b", fp(&[("footer", "v2"), ("epsilon", "v1"), ("zeta", "v1"), ("eta", "v1"), ("theta", "v1")], &[])),
        ];
        let pred = TenantShareCap {
            min_mean_distance: 3.0,
            severity: FindingSeverity::Strict,
        };
        let findings = pred.eval(&entries);
        assert!(findings.is_empty(), "diverged sites should not trip the cap");
    }

    #[test]
    fn vocabulary_floor_fires_when_substrate_underutilized() {
        let entries = vec![
            entry(0, "t", "a", fp(&[("hero", "v1")], &[])),
            entry(1, "t", "b", fp(&[("hero", "v2")], &[])),
        ];
        let pred = VocabularyUtilizationFloor {
            min_distinct: 5,
            severity: FindingSeverity::Warn,
        };
        let findings = pred.eval(&entries);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, FindingSeverity::Warn);
        assert!(findings[0].message.contains("1 distinct"));
    }

    #[test]
    fn vocabulary_floor_passes_with_breadth() {
        let entries = vec![
            entry(0, "t", "a", fp(&[("hero", "v1"), ("footer", "v1"), ("nav", "v1")], &[])),
            entry(1, "t", "b", fp(&[("split_hero", "v1"), ("aside", "v1"), ("logo_cloud", "v1")], &[])),
        ];
        let pred = VocabularyUtilizationFloor {
            min_distinct: 5,
            severity: FindingSeverity::Warn,
        };
        let findings = pred.eval(&entries);
        assert!(findings.is_empty());
    }

    #[test]
    fn gradient_recency_fires_on_repeat() {
        let entries = vec![
            entry(0, "t", "a", fp(&[], &[("gradient_pool_name", "cool-indigo-violet")])),
            entry(1, "t", "b", fp(&[], &[("gradient_pool_name", "warm-amber-rust")])),
            entry(2, "t", "c", fp(&[], &[("gradient_pool_name", "cool-indigo-violet")])),
        ];
        let pred = GradientRecencyCap {
            lookback: 5,
            severity: FindingSeverity::Warn,
        };
        let findings = pred.eval(&entries);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("cool-indigo-violet"));
        assert_eq!(findings[0].sequences, vec![0, 2]);
    }

    #[test]
    fn gradient_recency_passes_when_lookback_window_excluded() {
        let entries = vec![
            entry(0, "t", "a", fp(&[], &[("gradient_pool_name", "cool-indigo-violet")])),
            entry(1, "t", "b", fp(&[], &[("gradient_pool_name", "warm-amber-rust")])),
            entry(2, "t", "c", fp(&[], &[("gradient_pool_name", "cool-indigo-violet")])),
        ];
        let pred = GradientRecencyCap {
            lookback: 1,
            severity: FindingSeverity::Warn,
        };
        let findings = pred.eval(&entries);
        assert!(findings.is_empty(), "lookback=1 should not see entry 0 from entry 2");
    }

    #[test]
    fn within_site_variant_cap_fires_when_same_pair_repeats() {
        let entries = vec![entry(
            0,
            "t",
            "a",
            fp(
                &[
                    ("hero", "v1"),
                    ("hero", "v1"),
                    ("hero", "v1"),
                    ("hero", "v1"),
                ],
                &[],
            ),
        )];
        let pred = WithinSiteVariantCap {
            max_per_site: 2,
            severity: FindingSeverity::Strict,
        };
        let findings = pred.eval(&entries);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].predicate, "within-site-variant-cap");
        assert!(findings[0].message.contains("\"hero\""));
        assert!(findings[0].message.contains("\"v1\""));
    }

    #[test]
    fn run_predicates_composes_findings() {
        let entries = vec![
            entry(0, "t", "a", fp(&[("hero", "v1")], &[])),
            entry(1, "t", "b", fp(&[("hero", "v1")], &[])),
        ];
        let preds: Vec<Box<dyn CrossBuildPredicate>> = vec![
            Box::new(TenantShareCap {
                min_mean_distance: 3.0,
                severity: FindingSeverity::Strict,
            }),
            Box::new(VocabularyUtilizationFloor {
                min_distinct: 10,
                severity: FindingSeverity::Warn,
            }),
        ];
        let findings = run_predicates(&preds, &entries);
        assert_eq!(findings.len(), 2);
        let predicates: Vec<&str> = findings.iter().map(|f| f.predicate.as_str()).collect();
        assert!(predicates.contains(&"tenant-share-cap"));
        assert!(predicates.contains(&"vocabulary-utilization-floor"));
    }
}
