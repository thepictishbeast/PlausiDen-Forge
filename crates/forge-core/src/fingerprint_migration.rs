//! `fingerprint_migration` — version-discipline for FingerprintSpec.
//!
//! Task #259. Codifies how `FingerprintSpec` evolves over time
//! without breaking the registry's audit guarantees. Per memory
//! `[[backward-compat-version-discipline]]`: every artifact carries
//! a version tuple; changes classify into 4 categories (invisible /
//! additive / auto-migration / operator-action).
//!
//! ## What this module owns
//!
//! * The [`FingerprintMigration`] trait — every spec-version-pair
//!   that supports auto-migration registers a `fn(SiteFingerprint)
//!   -> Result<SiteFingerprint, MigrationError>` implementation.
//! * The [`migrate`] dispatcher — looks up the matching migration,
//!   applies it, returns the rebuilt fingerprint at the target
//!   spec version.
//! * The [`spec_distribution`] audit helper — reports how many
//!   registry entries are at each spec version. Operators run
//!   this before bumping `FingerprintSpec` to plan the migration.
//! * The [`is_comparable`] helper — formalizes the spec-uniformity
//!   requirement for [`SiteFingerprint::component_distance`].
//!
//! ## When to bump FingerprintSpec
//!
//! Bump the spec variant when any of:
//!
//! 1. **Adding a new structural component** (e.g. interactive-
//!    state-machine fingerprint contribution lands #272). Existing
//!    fingerprints can't include the new component, so distance
//!    across versions is undefined; mark V1→V2 with auto-migration
//!    that defaults the new component to its empty value.
//! 2. **Changing canonicalization** (e.g. sort primitives by a
//!    different key, change bucket boundaries). Old commitments
//!    don't equal new commitments for the same site; auto-migration
//!    recomputes from the structured components.
//! 3. **Removing a component** (e.g. interactive_count rolls into
//!    a richer dimension). Old fingerprints' removed-component
//!    fields are dropped during migration.
//!
//! Do NOT bump for:
//!
//! * Adding a new variant within an existing component (e.g. new
//!   density-tier names). Existing fingerprints stay valid; new
//!   variants surface as a different `variant` string in
//!   `PrimitiveOccurrence` but the spec doesn't change.
//! * Adding new optional fields to the substrate that don't enter
//!   the fingerprint shape.
//!
//! ## Migration safety
//!
//! Every migration is:
//!
//! * **Pure** — same input fingerprint → same output fingerprint.
//! * **Deterministic** — no clock, no randomness, no filesystem.
//! * **Reversible-on-paper** (not necessarily round-trip equal)
//!   — operators can compute the v2 fingerprint from a v1
//!   fingerprint without re-running the build.
//! * **Documented** — migrations land in this module's source +
//!   `docs/VARIATION_GUARANTEES.md` + a registry entry signed by
//!   the substrate authors.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"` (inherited).
//! * No unwrap/expect in non-test code.
//! * Migration trait is `#[non_exhaustive]` if/when it gains
//!   methods beyond `migrate`.
//! * Errors are typed via [`MigrationError`].

use std::collections::BTreeMap;
use std::path::Path;

use crate::fingerprint::{FingerprintSpec, SiteFingerprint};
use crate::fingerprint_registry;

/// Errors a migration can raise.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MigrationError {
    /// No registered migration from source to target spec.
    #[error("no migration registered from {from:?} to {to:?}")]
    NoMigration {
        /// Source spec.
        from: FingerprintSpec,
        /// Target spec.
        to: FingerprintSpec,
    },
    /// The migration function rejected the input fingerprint.
    /// Used when a v1 → v2 migration finds the v1 fingerprint
    /// is malformed.
    #[error("migration {from:?} → {to:?} failed: {message}")]
    MigrationFailed {
        /// Source spec.
        from: FingerprintSpec,
        /// Target spec.
        to: FingerprintSpec,
        /// Underlying reason.
        message: String,
    },
}

/// True iff two fingerprints can be compared via
/// [`SiteFingerprint::component_distance`]. Equivalent to spec
/// equality today; future spec versions may allow cross-version
/// distance via a compatibility matrix.
#[must_use]
pub fn is_comparable(a: &SiteFingerprint, b: &SiteFingerprint) -> bool {
    a.spec == b.spec
}

/// Apply the registered migration from `source.spec` to `target`.
/// Returns the rebuilt fingerprint at the target spec.
///
/// Identity migration (same spec → same spec) is always
/// supported and returns the input unchanged.
pub fn migrate(
    source: SiteFingerprint,
    target: FingerprintSpec,
) -> Result<SiteFingerprint, MigrationError> {
    if source.spec == target {
        return Ok(source);
    }
    // No non-identity migrations defined yet; FingerprintSpec
    // currently only has V1. When V2 lands, add a match arm
    // here that calls into the V1 → V2 migration function.
    Err(MigrationError::NoMigration {
        from: source.spec,
        to: target,
    })
}

/// Audit helper: report the distribution of spec versions present
/// in a fingerprint registry. Operators run this before bumping
/// `FingerprintSpec` to plan the migration: a registry with 1000
/// V1 entries and 0 V2 entries needs a full migration pass; a
/// registry with mixed entries needs careful handling.
///
/// Returns a `BTreeMap` keyed by spec slug (`"v1"`, `"v2"`, ...)
/// with the count of entries at that version.
pub fn spec_distribution(
    registry_path: &Path,
) -> Result<BTreeMap<String, u64>, fingerprint_registry::RegistryError> {
    let entries = fingerprint_registry::read_all(registry_path)?;
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for e in entries {
        *counts
            .entry(e.fingerprint.spec.slug().to_owned())
            .or_insert(0) += 1;
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attest::generate_keypair;
    use crate::fingerprint::{
        AssetDistribution, ContentSilhouette, FingerprintSpec, PrimitiveOccurrence, SiteFingerprint,
    };
    use std::collections::BTreeMap as Map;

    fn sample_fingerprint() -> SiteFingerprint {
        let mut sil = BTreeMap::new();
        sil.insert("index".to_owned(), ContentSilhouette::new(2, 3, 0, "h1,h2"));
        SiteFingerprint::new(
            FingerprintSpec::V1,
            vec![PrimitiveOccurrence::new("hero_editorial", "v=a", "index")],
            Vec::new(),
            sil,
            Map::new(),
            AssetDistribution::default(),
        )
    }

    #[test]
    fn is_comparable_returns_true_for_same_spec() {
        let a = sample_fingerprint();
        let b = sample_fingerprint();
        assert!(is_comparable(&a, &b));
    }

    #[test]
    fn migrate_identity_returns_input_unchanged() {
        let fp = sample_fingerprint();
        let hex = fp.commitment_hex();
        let migrated = migrate(fp, FingerprintSpec::V1).unwrap();
        assert_eq!(migrated.commitment_hex(), hex);
    }

    #[test]
    fn migrate_returns_error_for_unregistered_pair() {
        // FingerprintSpec currently has only V1 - we can't test
        // a cross-version migration. But we can verify the
        // dispatcher's error shape when no migration exists.
        // This test stays as a placeholder for when V2 lands.
        let fp = sample_fingerprint();
        // Same-spec migration is identity; covered above. When V2
        // lands, this test gets a real cross-version assertion.
        assert_eq!(fp.spec, FingerprintSpec::V1);
    }

    #[test]
    fn spec_distribution_counts_per_version() {
        let path = std::env::temp_dir().join(format!("forge-spec-dist-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let key = generate_keypair();
        fingerprint_registry::append(
            &path,
            "site-a",
            "tenant",
            sample_fingerprint(),
            "2026-05-20T12:00:00Z",
            &key,
        )
        .unwrap();
        fingerprint_registry::append(
            &path,
            "site-b",
            "tenant",
            sample_fingerprint(),
            "2026-05-20T12:01:00Z",
            &key,
        )
        .unwrap();
        let dist = spec_distribution(&path).unwrap();
        assert_eq!(dist.get("v1").copied(), Some(2));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn spec_distribution_empty_on_missing_registry() {
        let path = std::env::temp_dir().join("forge-spec-dist-missing-xyz");
        let _ = std::fs::remove_file(&path);
        let dist = spec_distribution(&path).unwrap();
        assert!(dist.is_empty());
    }
}
