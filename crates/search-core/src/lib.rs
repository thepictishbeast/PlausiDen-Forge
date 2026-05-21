//! `search-core` — typed search contract.
//!
//! Per `PLATFORM_ROADMAP.md` §16, every tenant gets search
//! built in. This crate defines the cross-backend contract;
//! per-backend clients plug in via [`SearchBackend`].
//!
//! Supported backends (closed enum):
//!   * Typesense — typo-tolerant, faceted, low-latency
//!   * Meilisearch — typo-tolerant, faceted
//!   * Tantivy — embedded, Rust-native (no external service)
//!   * SqliteFts5 — embedded zero-deps
//!   * Quickwit — distributed time-series + full-text
//!
//! Cross-cutting concerns covered:
//!   * [`IndexDoc`] — the typed search document shape
//!   * [`Query`] — typo tolerance + facets + filter
//!   * [`AnalyticsEvent`] — typed click + zero-result tracking
//!   * [`ContentGap`] — derived from zero-result queries for the
//!     operator's content-strategy dashboard
//!
//! ### Why typed
//!
//! Switching from "search powered by ${vendor}" to another vendor
//! is the canonical place where index schema drift breaks
//! everything. Closing the [`SearchBackend`] enum + pinning
//! [`IndexDoc`] + [`Query`] + [`SearchHit`] makes the swap a
//! per-backend impl change, not a per-document migration.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Closed enum of supported search backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchBackend {
    /// Typesense — single-binary, typo-tolerant, faceted.
    Typesense,
    /// Meilisearch — Rust-native, typo-tolerant, faceted.
    Meilisearch,
    /// Tantivy — embedded Rust search library (no external
    /// service).
    Tantivy,
    /// SQLite FTS5 — embedded zero-deps, suitable for small
    /// tenants.
    #[serde(rename = "sqlite-fts5")]
    SqliteFts5,
    /// Quickwit — distributed time-series + full-text.
    Quickwit,
}

impl SearchBackend {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Typesense => "typesense",
            Self::Meilisearch => "meilisearch",
            Self::Tantivy => "tantivy",
            Self::SqliteFts5 => "sqlite-fts5",
            Self::Quickwit => "quickwit",
        }
    }

    /// Whether this backend runs in-process (no external service
    /// to operate).
    pub fn is_embedded(&self) -> bool {
        matches!(self, Self::Tantivy | Self::SqliteFts5)
    }
}

/// One indexed document. Field set is closed — adding a field
/// means updating every backend's index schema in one commit,
/// not free-form drift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct IndexDoc {
    /// Stable document id (typically the CmsSection id from #77).
    pub id: String,
    /// Indexable title text.
    pub title: String,
    /// Indexable full body text.
    pub body: String,
    /// Operator-supplied tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Operator-supplied facets — kebab-case key → value list.
    /// Used for filter narrowing ("category=blog AND
    /// author=alice").
    #[serde(default)]
    pub facets: Vec<(String, Vec<String>)>,
    /// BCP-47 language tag for analyzer selection.
    pub lang: String,
    /// Publish time, for date-faceted ranking + per-period
    /// content-gap analysis. RFC 3339 wire format.
    #[serde(with = "time::serde::rfc3339")]
    pub published_at: time::OffsetDateTime,
}

/// Typed query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Query {
    /// User-typed query string.
    pub q: String,
    /// Whether typo tolerance is enabled. Default true.
    #[serde(default = "default_true")]
    pub typo_tolerance: bool,
    /// Optional language hint (BCP-47).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// Facet filters as (key, value) pairs. Multiple values per
    /// key are OR'd; different keys are AND'd.
    #[serde(default)]
    pub filters: Vec<(String, String)>,
    /// Max hits to return.
    pub limit: u32,
    /// Pagination offset.
    pub offset: u32,
}

fn default_true() -> bool {
    true
}

/// One hit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SearchHit {
    /// Document id.
    pub doc_id: String,
    /// Backend's relevance score for the hit.
    pub score: f32,
    /// Optional highlighted snippet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

/// Search results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SearchResults {
    /// Hits ordered by descending score.
    pub hits: Vec<SearchHit>,
    /// Estimated total matches (per-backend accuracy varies).
    pub total: u64,
    /// Facet bucket counts, by (key, value).
    #[serde(default)]
    pub facet_counts: Vec<(String, String, u64)>,
    /// Server-side query time in milliseconds.
    pub query_ms: u32,
}

impl SearchResults {
    /// Whether this result is "zero result" — informs the
    /// content-gap analytics path.
    pub fn is_zero_result(&self) -> bool {
        self.hits.is_empty()
    }
}

/// Typed search analytics event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AnalyticsEvent {
    /// User issued a query.
    QueryIssued {
        /// Query string.
        q: String,
        /// Hit count returned to the user.
        hit_count: u32,
        /// When the query was issued.
        at: time::OffsetDateTime,
    },
    /// User clicked a hit.
    HitClicked {
        /// Query string.
        q: String,
        /// Clicked document id.
        doc_id: String,
        /// 1-indexed position in the result list.
        position: u32,
        /// When the click happened.
        at: time::OffsetDateTime,
    },
    /// Zero-result query — feeds content-gap detection.
    ZeroResult {
        /// Query string.
        q: String,
        /// When the query was issued.
        at: time::OffsetDateTime,
    },
}

impl AnalyticsEvent {
    /// Stable kebab-case discriminant slug.
    pub fn kind_slug(&self) -> &'static str {
        match self {
            Self::QueryIssued { .. } => "query-issued",
            Self::HitClicked { .. } => "hit-clicked",
            Self::ZeroResult { .. } => "zero-result",
        }
    }
}

/// Content gap derived from zero-result queries. Aggregated
/// downstream — this crate defines the typed record so the
/// dashboard layer is identical across backends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ContentGap {
    /// Normalized query string (lowercase, whitespace-collapsed).
    pub query: String,
    /// How many zero-result issuings of this query the operator
    /// has seen in the analytics window.
    pub occurrences: u64,
    /// First time seen.
    pub first_seen: time::OffsetDateTime,
    /// Most-recent time seen.
    pub last_seen: time::OffsetDateTime,
}

impl ContentGap {
    /// Normalise a query for content-gap aggregation.
    pub fn normalize_query(q: &str) -> String {
        let mut s = String::with_capacity(q.len());
        let mut prev_ws = false;
        for c in q.trim().chars() {
            if c.is_whitespace() {
                if !prev_ws {
                    s.push(' ');
                }
                prev_ws = true;
            } else {
                for lc in c.to_lowercase() {
                    s.push(lc);
                }
                prev_ws = false;
            }
        }
        s
    }
}

/// Typed errors at the search boundary.
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    /// Index schema mismatch (e.g. trying to index a doc
    /// missing a required field).
    #[error("schema: {0}")]
    Schema(String),
    /// Backend rejected the query (syntax, unknown facet, etc.).
    #[error("query: {0}")]
    Query(String),
    /// Network / IO error.
    #[error("backend: {0}")]
    Backend(String),
}

/// Per-backend client. Impl crates land per backend
/// (search-typesense, search-meilisearch, search-tantivy,
/// search-sqlite-fts5, search-quickwit).
pub trait SearchClient {
    /// Which backend this client connects to.
    fn backend(&self) -> SearchBackend;
    /// Index (insert or update) one document.
    fn index(&self, doc: &IndexDoc) -> Result<(), SearchError>;
    /// Delete one document by id.
    fn delete(&self, id: &str) -> Result<(), SearchError>;
    /// Run a query.
    fn search(&self, query: &Query) -> Result<SearchResults, SearchError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn backend_slugs_distinct() {
        let bs = [
            SearchBackend::Typesense,
            SearchBackend::Meilisearch,
            SearchBackend::Tantivy,
            SearchBackend::SqliteFts5,
            SearchBackend::Quickwit,
        ];
        let mut s = std::collections::HashSet::new();
        for b in bs {
            assert!(s.insert(b.slug()));
        }
    }

    #[test]
    fn embedded_set() {
        assert!(SearchBackend::Tantivy.is_embedded());
        assert!(SearchBackend::SqliteFts5.is_embedded());
        assert!(!SearchBackend::Typesense.is_embedded());
        assert!(!SearchBackend::Meilisearch.is_embedded());
        assert!(!SearchBackend::Quickwit.is_embedded());
    }

    #[test]
    fn query_typo_tolerance_default_true() {
        let j = r#"{"q":"hello","limit":10,"offset":0}"#;
        let q: Query = serde_json::from_str(j).unwrap();
        assert!(q.typo_tolerance);
    }

    #[test]
    fn results_zero_result_predicate() {
        let r = SearchResults {
            hits: vec![],
            total: 0,
            facet_counts: vec![],
            query_ms: 1,
        };
        assert!(r.is_zero_result());
        let r2 = SearchResults {
            hits: vec![SearchHit {
                doc_id: "x".into(),
                score: 0.5,
                snippet: None,
            }],
            total: 1,
            facet_counts: vec![],
            query_ms: 1,
        };
        assert!(!r2.is_zero_result());
    }

    #[test]
    fn analytics_event_kind_slug() {
        let now = datetime!(2026-05-18 12:00:00 UTC);
        let qi = AnalyticsEvent::QueryIssued {
            q: "x".into(),
            hit_count: 0,
            at: now,
        };
        let hc = AnalyticsEvent::HitClicked {
            q: "x".into(),
            doc_id: "d".into(),
            position: 1,
            at: now,
        };
        let zr = AnalyticsEvent::ZeroResult {
            q: "x".into(),
            at: now,
        };
        let mut s = std::collections::HashSet::new();
        for e in [qi.kind_slug(), hc.kind_slug(), zr.kind_slug()] {
            assert!(s.insert(e));
        }
    }

    #[test]
    fn content_gap_normalize_handles_case_and_whitespace() {
        assert_eq!(
            ContentGap::normalize_query("  Hello\tWorld  "),
            "hello world"
        );
        assert_eq!(ContentGap::normalize_query("MULTI   SPACE"), "multi space");
        assert_eq!(ContentGap::normalize_query("UPPER"), "upper");
        assert_eq!(ContentGap::normalize_query("a\nb"), "a b");
    }

    #[test]
    fn index_doc_serde_round_trip() {
        let d = IndexDoc {
            id: "d1".into(),
            title: "Title".into(),
            body: "Body".into(),
            tags: vec!["t1".into()],
            facets: vec![("category".into(), vec!["blog".into(), "tutorial".into()])],
            lang: "en".into(),
            published_at: datetime!(2026-05-18 12:00:00 UTC),
        };
        let j = serde_json::to_string(&d).unwrap();
        let back: IndexDoc = serde_json::from_str(&j).unwrap();
        assert_eq!(d, back);
    }

    #[test]
    fn index_doc_rejects_unknown_field() {
        let bad = r#"{"id":"x","title":"t","body":"b","lang":"en","published-at":"2026-05-18T12:00:00Z","ahem":1}"#;
        let r: Result<IndexDoc, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    #[test]
    fn analytics_event_serde_round_trip() {
        let now = datetime!(2026-05-18 12:00:00 UTC);
        let e = AnalyticsEvent::HitClicked {
            q: "rust".into(),
            doc_id: "d1".into(),
            position: 2,
            at: now,
        };
        let j = serde_json::to_string(&e).unwrap();
        let back: AnalyticsEvent = serde_json::from_str(&j).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn content_gap_serde_round_trip() {
        let g = ContentGap {
            query: "missing topic".into(),
            occurrences: 5,
            first_seen: datetime!(2026-05-01 00:00:00 UTC),
            last_seen: datetime!(2026-05-18 00:00:00 UTC),
        };
        let j = serde_json::to_string(&g).unwrap();
        let back: ContentGap = serde_json::from_str(&j).unwrap();
        assert_eq!(g, back);
    }

    // Regression-guard: serde rename_all="kebab-case" doesn't
    // insert a hyphen between `fts` and `5` in SqliteFts5, so
    // a bare rename_all would emit `sqlite-fts5` differently
    // from slug(). The per-variant #[serde(rename)] enforces a
    // match; this test pins it.
    #[test]
    fn backend_serde_wire_matches_slug() {
        for b in [
            SearchBackend::Typesense,
            SearchBackend::Meilisearch,
            SearchBackend::Tantivy,
            SearchBackend::SqliteFts5,
            SearchBackend::Quickwit,
        ] {
            let wire = serde_json::to_string(&b).unwrap();
            let stripped = wire.trim_matches('"');
            assert_eq!(stripped, b.slug(), "wire vs slug for {:?}", b);
        }
    }
}
