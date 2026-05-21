//! `doc_query` — progressive query interface for substrate
//! documentation.
//!
//! Per `docs/SUBSTRATE_REFRAME_2026_05_21.md` § Accessibility 2:
//! documentation is queryable, not loaded as prose. Claude asks
//! specific questions; structured answers return. Consumers
//! never need to read 50 pages of markdown to find one rule;
//! they query the index and get the relevant chunk.
//!
//! ## Wire shape
//!
//! Every doc-discoverable entity in the substrate has a
//! [`DocEntry`]:
//!
//! - `slug` — stable kebab-case identifier
//! - `title` — human-readable name
//! - `kind` — what this entry describes (`Doctrine`, `Primitive`,
//!   `AuditPhase`, `Workflow`, `Reframe`)
//! - `tags` — [`DocTag`] set for [`crate::session_scope`]
//!   filtering
//! - `summary` — one-line gist; never more than 240 chars
//! - `body` — full markdown body
//! - `related` — slugs of related entries; lets queries walk a
//!   small graph
//!
//! ## Filtering
//!
//! [`DocQueryFilter`] composes:
//! - `kind` — pin to one entry kind
//! - `tags_any` — match if entry has any of these tags
//! - `slug_prefix` — text-prefix match (case-insensitive)
//! - `contains_text` — body OR title OR summary contains
//!   substring (case-insensitive)
//! - `limit` — cap returned entries
//!
//! Returns [`DocEntry`] refs sorted by slug for deterministic
//! output.
//!
//! ## Where the index lives
//!
//! [`canonical_index`] returns a process-wide read-only index
//! seeded with hand-curated entries for the most-frequently
//! needed substrate doctrine, primitives, audit phases, and
//! workflows. Adding an entry is small (≈10 LoC); the index
//! grows incrementally as the substrate's documentation
//! surface evolves.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::session_scope::DocTag;

/// Categorical kind of a doc entry. Drives which CLI / MCP
/// surface presents it: `Primitive` entries go to the primitive
/// catalog, `Doctrine` entries go to `forge doctrine for`,
/// `Workflow` entries go to skill loaders.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum DocKind {
    Doctrine,
    Primitive,
    AuditPhase,
    Workflow,
    Reframe,
}

/// One queryable doc entry. Each carries a stable slug, a
/// kind, a tag set for scope filtering, a one-line summary, the
/// full body, and a graph of related slugs.
///
/// Serialize-only (no Deserialize derive) because the entries
/// are compile-time `&'static`s sourced from
/// [`CANONICAL_ENTRIES`]; the query surface only needs to emit
/// them as JSON, never parse them back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct DocEntry {
    /// Stable kebab-case identifier.
    pub slug: &'static str,
    /// Human-readable title.
    pub title: &'static str,
    /// Categorical kind.
    pub kind: DocKind,
    /// Tag set for [`crate::session_scope::DocTag`] filtering.
    pub tags: &'static [DocTag],
    /// One-line gist; ≤240 chars enforced by
    /// [`DocEntry::validate`].
    pub summary: &'static str,
    /// Full markdown body.
    pub body: &'static str,
    /// Slugs of related entries.
    pub related: &'static [&'static str],
}

impl DocEntry {
    /// Self-check that a hand-curated entry conforms to the
    /// invariants the query surface relies on. Called by the
    /// canonical_index test to catch drift at compile + test
    /// time.
    pub fn validate(&self) -> Result<(), String> {
        if self.slug.is_empty() {
            return Err("empty slug".to_owned());
        }
        if !self
            .slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(format!("non-kebab-case slug: {:?}", self.slug));
        }
        if self.title.is_empty() {
            return Err(format!("entry {:?}: empty title", self.slug));
        }
        if self.summary.is_empty() {
            return Err(format!("entry {:?}: empty summary", self.slug));
        }
        if self.summary.len() > 240 {
            return Err(format!(
                "entry {:?}: summary {} chars > 240 — keep gists tight",
                self.slug,
                self.summary.len()
            ));
        }
        if self.tags.is_empty() {
            return Err(format!(
                "entry {:?}: no tags — scope filter will starve it",
                self.slug
            ));
        }
        Ok(())
    }
}

/// Filter compose-set for [`DocIndex::query`]. Every field is
/// optional; only set fields constrain the query.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocQueryFilter {
    /// Restrict to one kind.
    pub kind: Option<DocKind>,
    /// Match if entry has any of these tags (OR-semantics).
    pub tags_any: Vec<DocTag>,
    /// Slug prefix (case-insensitive).
    pub slug_prefix: Option<String>,
    /// Substring search over title + summary + body
    /// (case-insensitive).
    pub contains_text: Option<String>,
    /// Cap on returned entries.
    pub limit: Option<usize>,
}

/// Read-only doc index. Wraps a `BTreeMap<slug, &DocEntry>` so
/// callers can both enumerate entries (deterministic order) and
/// look up by slug in `O(log N)`.
pub struct DocIndex {
    entries: BTreeMap<&'static str, &'static DocEntry>,
}

impl DocIndex {
    /// Build an index from a slice of entries. Panics if
    /// `entries` has duplicate slugs — duplicates are a
    /// compile-time invariant violation, caught at the seam.
    #[must_use]
    pub fn from_entries(entries: &'static [DocEntry]) -> Self {
        let mut map: BTreeMap<&'static str, &'static DocEntry> = BTreeMap::new();
        for e in entries {
            assert!(
                map.insert(e.slug, e).is_none(),
                "duplicate doc slug: {:?}",
                e.slug
            );
        }
        Self { entries: map }
    }

    /// Lookup by exact slug.
    #[must_use]
    pub fn get(&self, slug: &str) -> Option<&DocEntry> {
        self.entries.get(slug).copied()
    }

    /// Query the index. Returns entries sorted by slug.
    #[must_use]
    pub fn query(&self, filter: &DocQueryFilter) -> Vec<&DocEntry> {
        let lower_prefix = filter.slug_prefix.as_deref().map(str::to_lowercase);
        let lower_text = filter.contains_text.as_deref().map(str::to_lowercase);
        let mut out: Vec<&DocEntry> = Vec::new();
        for entry in self.entries.values() {
            if let Some(k) = filter.kind {
                if entry.kind != k {
                    continue;
                }
            }
            if !filter.tags_any.is_empty()
                && !filter
                    .tags_any
                    .iter()
                    .any(|t| entry.tags.contains(t))
            {
                continue;
            }
            if let Some(prefix) = &lower_prefix {
                if !entry.slug.to_ascii_lowercase().starts_with(prefix) {
                    continue;
                }
            }
            if let Some(needle) = &lower_text {
                let hay = format!(
                    "{} {} {}",
                    entry.title.to_ascii_lowercase(),
                    entry.summary.to_ascii_lowercase(),
                    entry.body.to_ascii_lowercase()
                );
                if !hay.contains(needle) {
                    continue;
                }
            }
            out.push(entry);
        }
        if let Some(cap) = filter.limit {
            out.truncate(cap);
        }
        out
    }

    /// Number of entries in the index.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when the index has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Canonical hand-curated entries seeded into the doc index.
/// Lives as a `static` so the index is zero-cost to construct.
/// Each new entry is ≈10 LoC; grow this incrementally as the
/// substrate evolves.
pub static CANONICAL_ENTRIES: &[DocEntry] = &[
    DocEntry {
        slug: "substrate-only-path",
        title: "Substrate is the only path",
        kind: DocKind::Doctrine,
        tags: &[DocTag::Doctrine, DocTag::Reframe],
        summary: "Hand-coding sites is forbidden. Every gap is a substrate change; CMS content + Rust substrate are the only action surfaces.",
        body: "Hand-coding HTML / CSS / JS is forbidden inside the substrate. Every gap surfaced during site construction is a substrate change — either a new primitive, a new theme variation, a new audit phase, or a new doc tag — not a hand-authored override. Canonical defaults (axum, tokio, sqlx, maud, serde-deny_unknown_fields, Ed25519, ML-DSA, clap, anyhow + thiserror, proptest, tracing) are not relitigated.",
        related: &["forge-lite-diagnostic", "substrate-reframe-2026-05-21"],
    },
    DocEntry {
        slug: "substrate-reframe-2026-05-21",
        title: "Substrate reframe (2026-05-21)",
        kind: DocKind::Reframe,
        tags: &[DocTag::Reframe, DocTag::Doctrine, DocTag::Primitive],
        summary: "Forge vocabulary too narrow for sites outside originating consumer band. Capability + accessibility axes both need expansion in parallel.",
        body: "See docs/SUBSTRATE_REFRAME_2026_05_21.md. Three options for individual sites outside the current band: build outside Forge (recommended for timeline-pressured work), mark as legacy inside Forge, or wait for vocabulary expansion. Layered defense across 7 layers: type-enforced MCP, paired skill+workflow, fingerprint registry, multi-pass generation, inline operator override, outcome ratings + surveillance, substrate vocabulary expansion. Forge-specific fixes shipped #349-#354. Accessibility-axis tasks #385-#391.",
        related: &["substrate-only-path", "forge-lite-diagnostic", "deterministic-first"],
    },
    DocEntry {
        slug: "forge-lite-diagnostic",
        title: "Forge Lite — narrow-surface diagnostic",
        kind: DocKind::Reframe,
        tags: &[DocTag::Reframe, DocTag::Primitive, DocTag::Workflow],
        summary: "Closed 10-primitive + 3-theme surface to test the complexity-bottleneck hypothesis. CLI: forge lite resolve <input.json>.",
        body: "Forge Lite exposes a deliberately narrow input contract: 10 primitives (hero, heading, paragraph, image_hero, feature_spotlight, pull_quote, call_to_action, logo_cloud, divider, spacer) and 3 themes (light, dark, warm). The lite typed surface lives at forge-core::forge_lite; the resolver at forge-phases::forge_lite_resolve. CLI subcommand forge lite resolve. Five fixtures at fixtures/forge-lite/ exercise variation; integration harness pins six diagnostic invariants. Verdict-producing comparison runs are operator work (task #396).",
        related: &["substrate-reframe-2026-05-21", "session-scope-pattern"],
    },
    DocEntry {
        slug: "deterministic-first",
        title: "Deterministic-first, LFI-optional",
        kind: DocKind::Doctrine,
        tags: &[DocTag::Doctrine, DocTag::Reframe],
        summary: "Substrate = deterministic baseline (types / schemas / lints / audit phases). LFI + LLM are opt-in augmentation layers with fail-closed defaults.",
        body: "Architectural inversion: the deterministic substrate produces sites regardless of AI capability. LFI (neurosymbolic) and LLM (candidate generator) are augmentation layers that can be opted in per-tenant. Defaults fail closed — a tenant that doesn't configure LFI gets the deterministic baseline; a tenant that does gets the additional gates. Supersedes earlier 'LFI-as-core-LLM-as-peripheral' framing.",
        related: &["substrate-only-path"],
    },
    DocEntry {
        slug: "session-scope-pattern",
        title: "Scoped session pattern",
        kind: DocKind::Workflow,
        tags: &[DocTag::Workflow, DocTag::Doctrine],
        summary: "Sessions declare scope via FORGE_SESSION_SCOPE env. MCP tool surface + doc tags are filtered to that scope to manage cognitive load.",
        body: "Per the accessibility axis (substrate reframe 2026-05-21). Closed enum of seven scopes: build_site, modify_primitive, debug_audit, extend_deploy_target, author_content, investigate_substrate, unscoped. Tools and docs irrelevant to the declared scope aren't surfaced. forge-mcp's tools/list response filters by env-declared scope; unknown / empty env → full pass-through. Substrate primitive at forge-core::session_scope.",
        related: &["forge-lite-diagnostic", "substrate-reframe-2026-05-21"],
    },
    DocEntry {
        slug: "gradient-pool",
        title: "Default-fragmentation gradient pool",
        kind: DocKind::Primitive,
        tags: &[DocTag::Primitive, DocTag::Doctrine],
        summary: "24-pair curated pool with identity-aware deterministic selection. Closes the 'same ugly gradient on every site' failure mode.",
        body: "loom_tokens::gradient_pool ships 24 gradient pairs spanning 6 moods (Cool / Warm / Monochrome / Duotone / Neutral / Photographic) plus Solid. Selection: SHA-256 over tenant\\0site_id → pool index. Different identities land on different pairs. forge-phases::render::inject_default_gradient wires the selection into the render cascade BEFORE the tenant [style] block so explicit tenant overrides still win. Pool growth is design-led; parallel pools for header / footer / button / spacing rhythm are captured as tasks #392-#395.",
        related: &["substrate-reframe-2026-05-21"],
    },
    DocEntry {
        slug: "fingerprint-registry",
        title: "Cross-build fingerprint registry",
        kind: DocKind::Primitive,
        tags: &[DocTag::Primitive, DocTag::AuditPhase, DocTag::Doctrine],
        summary: "Append-only, Merkle-chained, Ed25519-signed registry of every site Forge has built. Backs the uniqueness gate.",
        body: "forge-core::fingerprint_registry persists a queryable record of every build's structural signature: primitive sequence, variant choices, token usage, content silhouette, decorative elements, gradient choice. Queries: find_by_hash, find_near_duplicates (component_distance threshold), for_tenant (per-tenant scope). UniquenessGate, PatternEmergence, AestheticDistinctiveness, DifferentiationBudget, and other variation-arc phases consume this registry to refuse convergent outputs at the substrate boundary.",
        related: &["cross-build-audit", "diversity-surveillance"],
    },
    DocEntry {
        slug: "cross-build-audit",
        title: "Cross-build audit predicate runner",
        kind: DocKind::AuditPhase,
        tags: &[DocTag::AuditPhase, DocTag::Primitive, DocTag::Doctrine],
        summary: "Typed predicate trait + 4 shipped predicates: TenantShareCap, VocabularyUtilizationFloor, GradientRecencyCap, WithinSiteVariantCap.",
        body: "forge-core::cross_build_audit exposes CrossBuildPredicate trait + run_predicates() composer. Shipped predicates: TenantShareCap (per-tenant convergence), VocabularyUtilizationFloor (platform-wide breadth), GradientRecencyCap (consumes gradient_pool_name token; refuses recently-reused gradients), WithinSiteVariantCap (per-site (kind, variant) caps). Findings carry structured severity + remediation + sequences. Lives in forge-core so MCP + CLI + dashboard can all consume the same predicate library.",
        related: &["fingerprint-registry", "diversity-surveillance"],
    },
    DocEntry {
        slug: "diversity-surveillance",
        title: "Diversity surveillance metrics",
        kind: DocKind::AuditPhase,
        tags: &[DocTag::AuditPhase, DocTag::Primitive],
        summary: "DiversityMetrics snapshot from the fingerprint registry. Per-tenant + per-platform views. ConvergenceRisk Low/Medium/High verdict.",
        body: "forge-core::surveillance computes structured metrics across the registry: primitive_usage (per-kind aggregate counts), variant_usage (per primitive::variant), token_override_usage, mean/min/max pairwise distance, bucketed distance histogram, tenant_count. compute_from_registry(path) reads via fingerprint_registry::read_all; from_entries(slice) is the pure-function variant. convergence_risk() categorizes the snapshot: mean < 3 → High, mean < 6 OR top primitive > 50% share → Medium, else Low.",
        related: &["fingerprint-registry", "cross-build-audit"],
    },
    DocEntry {
        slug: "build-from-brief",
        title: "Workflow: forge build site from brief",
        kind: DocKind::Workflow,
        tags: &[DocTag::Workflow, DocTag::Tenant],
        summary: "Canonical end-to-end build workflow. Brief in, structured audit findings out. Internally runs verify_content_originality.",
        body: "Inputs: operator brief or content requirements, target site identity (existing or to-be-created), target deployment mode. Outputs: built site + structured audit results. Internally calls verify_content_originality (anti-reuse fingerprint check) as a non-bypassable phase. The workflow encodes the correct sequencing — skipping audit phases is structurally impossible because they are part of the workflow's own execution. Pair skill at skills/build-site-from-brief.md. Tracking: task #364.",
        related: &["session-scope-pattern", "fingerprint-registry"],
    },
];

/// Build the canonical, process-wide doc index. Cheap — just
/// walks the static slice + populates a BTreeMap.
#[must_use]
pub fn canonical_index() -> DocIndex {
    DocIndex::from_entries(CANONICAL_ENTRIES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_entries_pass_validation() {
        for e in CANONICAL_ENTRIES {
            e.validate().expect("entry valid");
        }
    }

    #[test]
    fn canonical_index_has_no_duplicate_slugs() {
        let _ = canonical_index();
    }

    #[test]
    fn canonical_index_ships_minimum_entries() {
        // Pin the seeded breadth: the diagnostic
        // (Accessibility 2) requires enough entries to be
        // useful. Initial floor is 10; grow as substrate doc
        // surface evolves.
        let idx = canonical_index();
        assert!(
            idx.len() >= 10,
            "canonical index should ship ≥10 entries, has {}",
            idx.len()
        );
    }

    #[test]
    fn related_slugs_all_resolve() {
        let idx = canonical_index();
        for e in CANONICAL_ENTRIES {
            for r in e.related {
                assert!(
                    idx.get(r).is_some(),
                    "entry {:?} references unknown slug {:?}",
                    e.slug,
                    r
                );
            }
        }
    }

    #[test]
    fn query_by_kind() {
        let idx = canonical_index();
        let doctrine = idx.query(&DocQueryFilter {
            kind: Some(DocKind::Doctrine),
            ..Default::default()
        });
        assert!(!doctrine.is_empty());
        for e in &doctrine {
            assert_eq!(e.kind, DocKind::Doctrine);
        }
    }

    #[test]
    fn query_by_tag_any() {
        let idx = canonical_index();
        let workflow_or_audit = idx.query(&DocQueryFilter {
            tags_any: vec![DocTag::Workflow, DocTag::AuditPhase],
            ..Default::default()
        });
        assert!(!workflow_or_audit.is_empty());
        for e in &workflow_or_audit {
            assert!(
                e.tags.contains(&DocTag::Workflow) || e.tags.contains(&DocTag::AuditPhase),
                "entry {:?} matched tag-any filter without holding any matched tag",
                e.slug
            );
        }
    }

    #[test]
    fn query_by_slug_prefix() {
        let idx = canonical_index();
        let r = idx.query(&DocQueryFilter {
            slug_prefix: Some("substrate-".to_owned()),
            ..Default::default()
        });
        assert!(!r.is_empty());
        for e in &r {
            assert!(e.slug.starts_with("substrate-"));
        }
    }

    #[test]
    fn query_by_contains_text() {
        let idx = canonical_index();
        let r = idx.query(&DocQueryFilter {
            contains_text: Some("MERKLE".to_owned()),
            ..Default::default()
        });
        assert!(!r.is_empty(), "fingerprint-registry entry should match 'merkle' case-insensitively");
    }

    #[test]
    fn query_limit_caps_results() {
        let idx = canonical_index();
        let r = idx.query(&DocQueryFilter {
            limit: Some(2),
            ..Default::default()
        });
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn query_results_sorted_by_slug() {
        let idx = canonical_index();
        let r = idx.query(&DocQueryFilter::default());
        for w in r.windows(2) {
            assert!(w[0].slug < w[1].slug, "results not slug-sorted");
        }
    }

    #[test]
    fn empty_filter_returns_everything() {
        let idx = canonical_index();
        let r = idx.query(&DocQueryFilter::default());
        assert_eq!(r.len(), idx.len());
    }

    #[test]
    fn unknown_slug_lookup_returns_none() {
        let idx = canonical_index();
        assert!(idx.get("does-not-exist").is_none());
    }
}
