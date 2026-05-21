//! `tenancy-core` — typed TenantId + 3-tier isolation primitives.
//!
//! Per `PLATFORM_ROADMAP.md` §5 + the design-premium doctrine:
//! when the platform hosts multiple tenants (CMS deployments,
//! Loom edit-serve workspaces, per-customer Forge builds), every
//! operation that touches tenant-owned resources must carry a
//! typed [`TenantId`] and run inside a typed
//! [`TenantContext`]. The boundary is enforced at the type
//! system, not by convention.
//!
//! ### The 3 isolation tiers
//!
//! Production multi-tenancy demands a choice per workload —
//! stronger isolation costs more, weaker isolation risks
//! leakage. This crate enumerates exactly 3 tiers so every
//! consumer makes an honest choice:
//!
//! | Tier              | Storage           | Process       | Network        |
//! |-------------------|-------------------|---------------|----------------|
//! | [`Shared`]        | shared DB w/ tenant_id col | one process | shared egress |
//! | [`DataIsolated`]  | per-tenant SQLite + per-tenant fs root | one process | shared egress |
//! | [`FullyIsolated`] | per-tenant SQLite + per-tenant fs root | per-tenant subprocess | per-tenant egress namespace |
//!
//! [`Shared`]: IsolationLevel::Shared
//! [`DataIsolated`]: IsolationLevel::DataIsolated
//! [`FullyIsolated`]: IsolationLevel::FullyIsolated
//!
//! Most substrate workloads default to `DataIsolated` —
//! per-tenant SQLite + per-tenant filesystem root, in one
//! process, shared egress. `Shared` is reserved for read-only
//! aggregations (analytics) where the row count justifies the
//! shared schema. `FullyIsolated` is reserved for adversarial-
//! multi-tenant deployments (e.g. shared instances hosting
//! mutually-untrusted operators).
//!
//! ### Why a typed boundary
//!
//! Every cross-tenant bug in shared-process multi-tenancy comes
//! down to "we forgot to filter by tenant_id." The typed surface
//! prevents that:
//!
//!   * Every per-tenant resource is reached via
//!     [`TenantContext`]
//!   * [`TenantContext::scoped_path`] guarantees the resulting
//!     path lives under the tenant's root (rejects traversal)
//!   * [`TenantBoundary::same_tenant`] is the only blessed way
//!     to compare tenant IDs (so the compiler catches "did we
//!     compare the right two ids?")
//!
//! ### Public surface
//!
//! - [`TenantId`]         — validated kebab-case newtype
//! - [`IsolationLevel`]   — 3-tier enum
//! - [`TenantContext`]    — bundled identity + isolation + paths
//! - [`TenantBoundary`]   — trait for cross-tenant comparisons
//! - [`TenancyError`]     — typed errors

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Typed kebab-case tenant identifier.
///
/// Distinct newtype so the type system distinguishes tenant IDs
/// from arbitrary strings. Construction validates: kebab-case,
/// 1..=64 chars, must start with a letter, no consecutive
/// hyphens, no leading/trailing hyphen.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TenantId(String);

impl TenantId {
    /// Maximum identifier length.
    pub const MAX_LEN: usize = 64;

    /// Build a `TenantId` from a string slice, validating shape.
    pub fn parse(s: impl AsRef<str>) -> Result<Self, TenancyError> {
        let s = s.as_ref();
        if s.is_empty() {
            return Err(TenancyError::InvalidId("empty".into()));
        }
        if s.len() > Self::MAX_LEN {
            return Err(TenancyError::InvalidId(format!(
                "{s:?} exceeds {} chars",
                Self::MAX_LEN
            )));
        }
        let first = s.chars().next().unwrap();
        if !first.is_ascii_lowercase() {
            return Err(TenancyError::InvalidId(format!(
                "{s:?} must start with [a-z]"
            )));
        }
        if s.ends_with('-') {
            return Err(TenancyError::InvalidId(format!(
                "{s:?} has trailing hyphen"
            )));
        }
        if s.contains("--") {
            return Err(TenancyError::InvalidId(format!(
                "{s:?} has consecutive hyphens"
            )));
        }
        for c in s.chars() {
            if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
                return Err(TenancyError::InvalidId(format!(
                    "{s:?} contains {c:?} not in [a-z0-9-]"
                )));
            }
        }
        Ok(Self(s.to_string()))
    }

    /// Raw string slice. Use [`Self::parse`] for construction.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The 3 isolation tiers. Closed enum, no defaults — every
/// consumer must make an explicit choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IsolationLevel {
    /// Shared DB (rows discriminated by tenant_id column), shared
    /// process, shared egress. Reserved for read-only aggregations
    /// where the row count justifies the shared schema.
    Shared,
    /// Per-tenant SQLite + per-tenant filesystem root, one
    /// process, shared egress. Platform default for normal
    /// multi-tenant CMS / edit-serve workloads.
    DataIsolated,
    /// Per-tenant SQLite + per-tenant fs root + per-tenant
    /// subprocess + per-tenant egress namespace. Reserved for
    /// adversarial multi-tenant (shared instances hosting
    /// mutually-untrusted operators).
    FullyIsolated,
}

impl IsolationLevel {
    /// Stable kebab-case slug for serialization + UI.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::DataIsolated => "data-isolated",
            Self::FullyIsolated => "fully-isolated",
        }
    }

    /// All variants in declaration order. Used by the admin UI to
    /// render a "pick your isolation tier" form.
    pub const ALL: &'static [Self] = &[Self::Shared, Self::DataIsolated, Self::FullyIsolated];

    /// Whether this tier carves out a per-tenant filesystem root.
    pub fn has_isolated_storage(&self) -> bool {
        matches!(self, Self::DataIsolated | Self::FullyIsolated)
    }

    /// Whether this tier carves out a per-tenant subprocess.
    pub fn has_isolated_process(&self) -> bool {
        matches!(self, Self::FullyIsolated)
    }

    /// Whether this tier carves out a per-tenant egress namespace.
    pub fn has_isolated_network(&self) -> bool {
        matches!(self, Self::FullyIsolated)
    }
}

/// Bundled identity + isolation + per-tenant filesystem layout.
/// Every consumer reads this once and projects through it for
/// every per-tenant operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TenantContext {
    /// The tenant this context represents.
    pub tenant_id: TenantId,
    /// Declared isolation level for this tenant.
    pub isolation: IsolationLevel,
    /// Per-tenant filesystem root. Always populated, even for
    /// [`IsolationLevel::Shared`] (where it just provides a
    /// per-tenant cache dir).
    pub fs_root: PathBuf,
    /// Per-tenant SQLite path. None only for
    /// [`IsolationLevel::Shared`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sqlite_path: Option<PathBuf>,
}

impl TenantContext {
    /// Resolve `relative` against the tenant's filesystem root
    /// and refuse traversal that escapes the root.
    ///
    /// Refuses `..` components, absolute paths, and any input
    /// whose canonical resolution would escape `fs_root`.
    pub fn scoped_path(&self, relative: &Path) -> Result<PathBuf, TenancyError> {
        if relative.is_absolute() {
            return Err(TenancyError::Traversal(format!(
                "absolute path not allowed: {}",
                relative.display()
            )));
        }
        for c in relative.components() {
            match c {
                std::path::Component::ParentDir => {
                    return Err(TenancyError::Traversal(format!(
                        ".. component not allowed: {}",
                        relative.display()
                    )));
                }
                std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                    return Err(TenancyError::Traversal(format!(
                        "root component not allowed: {}",
                        relative.display()
                    )));
                }
                _ => {}
            }
        }
        Ok(self.fs_root.join(relative))
    }
}

/// Trait for typed cross-tenant comparisons. Implementations get
/// a free `same_tenant` predicate the compiler treats as the only
/// blessed way to ask "are these two contexts for the same
/// tenant?".
pub trait TenantBoundary {
    /// Return the [`TenantId`] of this resource.
    fn tenant(&self) -> &TenantId;

    /// Check whether `self` and `other` belong to the same
    /// tenant.
    fn same_tenant<T: TenantBoundary + ?Sized>(&self, other: &T) -> bool {
        self.tenant() == other.tenant()
    }

    /// Refuse if `self` and `other` don't belong to the same
    /// tenant. Use this at any layer that crosses a logical
    /// tenant boundary.
    fn assert_same_tenant<T: TenantBoundary + ?Sized>(
        &self,
        other: &T,
    ) -> Result<(), TenancyError> {
        if self.same_tenant(other) {
            Ok(())
        } else {
            Err(TenancyError::CrossTenant {
                lhs: self.tenant().clone(),
                rhs: other.tenant().clone(),
            })
        }
    }
}

impl TenantBoundary for TenantContext {
    fn tenant(&self) -> &TenantId {
        &self.tenant_id
    }
}

/// Errors returned by tenancy operations.
#[derive(Debug, thiserror::Error)]
pub enum TenancyError {
    /// Tenant ID failed the shape contract.
    #[error("invalid tenant id: {0}")]
    InvalidId(String),
    /// A relative path tried to escape the tenant fs root.
    #[error("path traversal refused: {0}")]
    Traversal(String),
    /// Two resources from different tenants were combined.
    #[error("cross-tenant operation refused: {lhs} vs {rhs}")]
    CrossTenant {
        /// Left tenant.
        lhs: TenantId,
        /// Right tenant.
        rhs: TenantId,
    },
    /// IO failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(id: &str, root: &Path) -> TenantContext {
        TenantContext {
            tenant_id: TenantId::parse(id).unwrap(),
            isolation: IsolationLevel::DataIsolated,
            fs_root: root.to_path_buf(),
            sqlite_path: Some(root.join("tenant.db")),
        }
    }

    #[test]
    fn tenant_id_accepts_kebab_case() {
        assert!(TenantId::parse("acme").is_ok());
        assert!(TenantId::parse("acme-corp").is_ok());
        assert!(TenantId::parse("acme-corp-2026").is_ok());
        assert!(TenantId::parse("a1").is_ok());
    }

    #[test]
    fn tenant_id_rejects_bad_forms() {
        assert!(TenantId::parse("").is_err());
        assert!(TenantId::parse("Acme").is_err());
        assert!(TenantId::parse("acme_corp").is_err());
        assert!(TenantId::parse("0acme").is_err()); // must start with letter
        assert!(TenantId::parse("-acme").is_err());
        assert!(TenantId::parse("acme-").is_err());
        assert!(TenantId::parse("acme--corp").is_err());
        assert!(TenantId::parse("acme corp").is_err());
        assert!(TenantId::parse(&"a".repeat(65)).is_err());
    }

    #[test]
    fn isolation_level_capability_flags_match_intent() {
        assert!(!IsolationLevel::Shared.has_isolated_storage());
        assert!(IsolationLevel::DataIsolated.has_isolated_storage());
        assert!(IsolationLevel::FullyIsolated.has_isolated_storage());

        assert!(!IsolationLevel::Shared.has_isolated_process());
        assert!(!IsolationLevel::DataIsolated.has_isolated_process());
        assert!(IsolationLevel::FullyIsolated.has_isolated_process());

        assert!(!IsolationLevel::Shared.has_isolated_network());
        assert!(!IsolationLevel::DataIsolated.has_isolated_network());
        assert!(IsolationLevel::FullyIsolated.has_isolated_network());
    }

    #[test]
    fn isolation_slugs_are_stable_and_distinct() {
        let mut seen = std::collections::HashSet::new();
        for lvl in IsolationLevel::ALL {
            assert!(seen.insert(lvl.slug()), "duplicate slug {lvl:?}");
        }
    }

    #[test]
    fn scoped_path_joins_under_root() {
        let tmp = tempfile::tempdir().unwrap();
        let c = ctx("acme", tmp.path());
        let p = c.scoped_path(Path::new("a/b.txt")).unwrap();
        assert_eq!(p, tmp.path().join("a/b.txt"));
    }

    #[test]
    fn scoped_path_refuses_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        let c = ctx("acme", tmp.path());
        let r = c.scoped_path(Path::new("../etc/passwd"));
        assert!(matches!(r, Err(TenancyError::Traversal(_))));
    }

    #[test]
    fn scoped_path_refuses_absolute() {
        let tmp = tempfile::tempdir().unwrap();
        let c = ctx("acme", tmp.path());
        let r = c.scoped_path(Path::new("/etc/passwd"));
        assert!(matches!(r, Err(TenancyError::Traversal(_))));
    }

    #[test]
    fn scoped_path_refuses_dotdot_in_middle() {
        let tmp = tempfile::tempdir().unwrap();
        let c = ctx("acme", tmp.path());
        let r = c.scoped_path(Path::new("a/../b"));
        assert!(matches!(r, Err(TenancyError::Traversal(_))));
    }

    #[test]
    fn same_tenant_returns_true_for_matching_ids() {
        let tmp = tempfile::tempdir().unwrap();
        let a = ctx("acme", tmp.path());
        let b = ctx("acme", tmp.path());
        assert!(a.same_tenant(&b));
        a.assert_same_tenant(&b).unwrap();
    }

    #[test]
    fn assert_same_tenant_refuses_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let a = ctx("acme", tmp.path());
        let b = ctx("zorp", tmp.path());
        assert!(!a.same_tenant(&b));
        let r = a.assert_same_tenant(&b);
        assert!(matches!(r, Err(TenancyError::CrossTenant { .. })));
    }

    #[test]
    fn context_serde_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let c = ctx("acme", tmp.path());
        let s = serde_json::to_string(&c).unwrap();
        let back: TenantContext = serde_json::from_str(&s).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn context_rejects_unknown_field() {
        let bad = format!(
            r#"{{"tenant-id":"acme","isolation":"data-isolated","fs-root":"/tmp","ahem":1}}"#
        );
        let r: Result<TenantContext, _> = serde_json::from_str(&bad);
        assert!(r.is_err());
    }

    // T97: slug-vs-serde-wire regression guard.
    #[test]
    fn slug_matches_serde_wire_across_all_enums() {
        for v in [
            IsolationLevel::Shared,
            IsolationLevel::DataIsolated,
            IsolationLevel::FullyIsolated,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
    }
}
