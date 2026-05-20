//! `migration-core` — typed migration framework per AVP-Doctrine
//! `VERSION_DISCIPLINE.md`.
//!
//! Every Category 3 or 4 change to a substrate artifact adds a
//! signed entry to the migration registry
//! (`PlausiDen-AVP-Doctrine/migrations/registry.toml`). This crate
//! provides:
//!
//!   - `ChangeCategory` enum encoding the 4-category taxonomy
//!     (Invisible / Additive / AutoMigration / OperatorAction)
//!   - `MigrationEntry` struct with the signed-registry shape
//!   - `MigrationRegistry` collection type with verification +
//!     query helpers
//!   - `Migration` trait — the runtime contract for typed
//!     auto-migration impls (Category 3 transforms)
//!   - `verify_entry()` / `verify_registry()` well-formedness audits
//!
//! Crypto signature verification (Ed25519 + ML-DSA dual per
//! `[[super-society-tech-stack]]`) is exposed as a stub via the
//! `verify_signature` boundary — the actual crypto wires through
//! `manifest-attest` in a follow-on cross-crate integration
//! (filed via capability-request).
//!
//! Per `[[backward-compat-version-discipline]]`: signed entries
//! are mandatory before merge; the registry rejects unsigned
//! entries at parse time.
//!
//! Per `[[deterministic-first-lfi-optional]]`: registry +
//! migration apply are pure deterministic Rust code. No AI
//! involvement — auto-migrations are mechanical transforms by
//! definition.
//!
//! Closes `#139 [backcompat-v3]`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 4-category taxonomy from `VERSION_DISCIPLINE.md`. Determines
/// the migration posture a change requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ChangeCategory {
    /// Internal refactor; no observable behavior or schema change.
    /// No registry entry needed; SemVer patch bump.
    Invisible,

    /// New optional field / new variant in an open enum / new
    /// subcommand. Both old + new readers continue to work. No
    /// registry entry needed; SemVer minor bump.
    Additive,

    /// Schema requires transformation, but the transformation is
    /// mechanical + complete. The substrate carries the migration
    /// code; reading an old artifact emits the new shape
    /// transparently. **Registry entry mandatory.** SemVer minor
    /// or major depending on backward-readability.
    AutoMigration,

    /// Schema change that cannot be mechanically migrated.
    /// Operator must intervene (review, decide, edit). The
    /// substrate refuses to read the old artifact + emits a
    /// diagnostic naming the playbook. **Registry entry +
    /// playbook URL mandatory.** SemVer major bump.
    OperatorAction,
}

impl ChangeCategory {
    /// Canonical slug (kebab-case).
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Invisible => "invisible",
            Self::Additive => "additive",
            Self::AutoMigration => "auto-migration",
            Self::OperatorAction => "operator-action",
        }
    }

    /// All canonical categories in stable order.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::Invisible,
            Self::Additive,
            Self::AutoMigration,
            Self::OperatorAction,
        ]
    }

    /// True if a registry entry is mandatory for this category.
    /// Cat-1 + Cat-2 changes don't need an entry; Cat-3 + Cat-4 do.
    #[must_use]
    pub fn requires_registry_entry(self) -> bool {
        matches!(self, Self::AutoMigration | Self::OperatorAction)
    }
}

/// A signed migration registry entry per `VERSION_DISCIPLINE.md` §
/// Migration registry. Stable TOML/JSON shape; signatures are
/// mandatory for the registry to parse a Cat-3 or Cat-4 entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationEntry {
    /// Globally-unique migration id. Convention:
    /// `<artifact-slug>-v<from>-v<to>-<yyyy-mm>` e.g.
    /// `loom-primitive-hero-v1-v2-2026-08`.
    pub id: String,

    /// Version range the migration applies *from*. Accepts a
    /// version-range expression per `forge-core::parse_version_range`
    /// (e.g. `"1.x"`, `">=1.0.0,<2.0.0"`).
    pub from_version: String,

    /// The exact version the migration produces. Must be canonical
    /// semver 2.0.0 (MAJOR.MINOR.PATCH).
    pub to_version: String,

    /// Artifact class slug (e.g. `Loom.Primitive.Hero`,
    /// `Forge.Config.Backends`).
    pub artifact_class: String,

    /// Change category. Must be `AutoMigration` or `OperatorAction`
    /// to land in the registry — `Invisible` and `Additive`
    /// changes don't carry registry entries.
    pub category: ChangeCategory,

    /// One-paragraph description of the transformation.
    pub description: String,

    /// Path / URL to the operator playbook (mandatory for
    /// `OperatorAction`; optional for `AutoMigration`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playbook: Option<String>,

    /// Path to the typed Rust implementation of the transformation
    /// (mandatory for `AutoMigration` — the substrate calls into
    /// this on read). Format: workspace-relative
    /// `crate/path/to/impl.rs`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation: Option<String>,

    /// Test corpus directory with before/after fixtures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_corpus: Option<String>,

    /// Cryptographic signatures (Ed25519 + ML-DSA dual per
    /// `[[super-society-tech-stack]]`). At least one signature is
    /// mandatory; the dual-stack requirement enforces both.
    pub signatures: Vec<String>,

    /// Slug of the signer (typically the operator/maintainer who
    /// reviewed + signed).
    pub signed_by: String,

    /// RFC 3339 timestamp of signing.
    pub signed_at: String,
}

/// The full migration registry. Backed by
/// `PlausiDen-AVP-Doctrine/migrations/registry.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationRegistry {
    /// Schema version for the registry shape itself per
    /// `VERSION_DISCIPLINE.md`. Defaults to "1.0.0" on read when
    /// absent (Cat-2 additive accommodation).
    #[serde(default = "default_registry_version")]
    pub schema_version: String,

    /// All migration entries.
    #[serde(default)]
    pub migrations: Vec<MigrationEntry>,
}

fn default_registry_version() -> String {
    "1.0.0".into()
}

/// Errors surfaced by registry parsing / verification.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RegistryError {
    /// Entry's `signatures` field is empty. The registry rejects
    /// unsigned entries per `VERSION_DISCIPLINE.md`.
    #[error("entry `{id}` has no signatures")]
    MissingSignature {
        /// Entry id.
        id: String,
    },

    /// Entry's category requires an implementation but the
    /// `implementation` field is absent.
    #[error("entry `{id}` is AutoMigration but has no implementation field")]
    MissingImplementation {
        /// Entry id.
        id: String,
    },

    /// Entry's category requires a playbook but the `playbook`
    /// field is absent.
    #[error("entry `{id}` is OperatorAction but has no playbook field")]
    MissingPlaybook {
        /// Entry id.
        id: String,
    },

    /// Entry's `to_version` is not canonical semver 2.0.0.
    #[error("entry `{id}` to_version `{value}` is not canonical semver")]
    InvalidToVersion {
        /// Entry id.
        id: String,
        /// The malformed value.
        value: String,
    },

    /// Two entries share the same `id` — globally-unique invariant
    /// violated.
    #[error("duplicate entry id `{id}`")]
    DuplicateId {
        /// The duplicated id.
        id: String,
    },

    /// Entry's `category` is not allowed to carry a registry entry
    /// (Invisible / Additive are surfaced as errors when present).
    #[error("entry `{id}` carries category `{category:?}` which doesn't require a registry entry")]
    UnnecessaryEntry {
        /// Entry id.
        id: String,
        /// The category that triggered the error.
        category: ChangeCategory,
    },
}

/// Verify a single registry entry. Returns the violation list;
/// empty Vec = clean.
///
/// Crypto signature *content* is not verified here — the
/// signature *presence* is checked (the registry must reject
/// unsigned entries). Full crypto verification lands when the
/// manifest-attest integration cross-wires.
#[must_use]
pub fn verify_entry(entry: &MigrationEntry) -> Vec<RegistryError> {
    let mut errs = Vec::new();

    // 1. Signatures mandatory for any registry entry.
    if entry.signatures.is_empty() {
        errs.push(RegistryError::MissingSignature {
            id: entry.id.clone(),
        });
    }

    // 2. Category must require a registry entry. Invisible /
    //    Additive shouldn't be in the registry at all.
    if !entry.category.requires_registry_entry() {
        errs.push(RegistryError::UnnecessaryEntry {
            id: entry.id.clone(),
            category: entry.category,
        });
    }

    // 3. Category-specific field requirements.
    match entry.category {
        ChangeCategory::AutoMigration => {
            if entry.implementation.is_none() {
                errs.push(RegistryError::MissingImplementation {
                    id: entry.id.clone(),
                });
            }
        }
        ChangeCategory::OperatorAction => {
            if entry.playbook.is_none() {
                errs.push(RegistryError::MissingPlaybook {
                    id: entry.id.clone(),
                });
            }
        }
        _ => {}
    }

    // 4. to_version must be canonical MAJOR.MINOR.PATCH semver
    //    (with optional pre-release / build suffix).
    if !is_canonical_semver(&entry.to_version) {
        errs.push(RegistryError::InvalidToVersion {
            id: entry.id.clone(),
            value: entry.to_version.clone(),
        });
    }

    errs
}

/// Verify the whole registry. Walks every entry, plus the
/// global uniqueness invariant on `id`.
#[must_use]
pub fn verify_registry(reg: &MigrationRegistry) -> Vec<RegistryError> {
    let mut errs = Vec::new();
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for entry in &reg.migrations {
        if !seen.insert(entry.id.as_str()) {
            errs.push(RegistryError::DuplicateId {
                id: entry.id.clone(),
            });
        }
        errs.extend(verify_entry(entry));
    }
    errs
}

/// The runtime contract for typed auto-migration implementations.
/// Cat-3 (AutoMigration) changes carry a Rust impl that
/// transforms an old artifact's serialized form into the new
/// shape. Calling code reads through this trait; never
/// hand-rolls migrations.
pub trait Migration: Send + Sync {
    /// Migration registry id. Matches `MigrationEntry::id`.
    fn id(&self) -> &str;

    /// Apply the transformation to an artifact body
    /// (serialized as a UTF-8 string — TOML, JSON, etc.).
    /// Returns the migrated body, or an error if the input
    /// doesn't match the expected from_version shape.
    fn apply(&self, body: &str) -> Result<String, MigrationError>;
}

/// Errors a Migration implementation may return.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum MigrationError {
    /// Input doesn't parse as the migration's expected from_version
    /// shape.
    #[error("migration `{id}`: input does not match from_version shape: {reason}")]
    InputShapeMismatch {
        /// Migration id.
        id: String,
        /// Why the shape didn't match.
        reason: String,
    },

    /// Migration logic encountered an unrecoverable error.
    #[error("migration `{id}`: apply failed: {reason}")]
    ApplyFailed {
        /// Migration id.
        id: String,
        /// Why apply failed.
        reason: String,
    },
}

/// Stub for crypto signature verification. The actual Ed25519 +
/// ML-DSA dual verification cross-wires through `manifest-attest`.
/// Returns `Ok(false)` until the integration lands (per stub
/// semantics — callers fail-closed).
///
/// Filed as follow-on capability-request: `migration-attest-cross-wire`.
///
/// # Errors
///
/// Returns `Err(VerificationError::NotImplemented)` to signal
/// callers that signature verification is not yet wired. Callers
/// should treat this as "do not trust" and refuse merge.
pub fn verify_signature(
    _entry: &MigrationEntry,
    _signature: &str,
) -> Result<bool, VerificationError> {
    Err(VerificationError::NotImplemented)
}

/// Errors signal returned by `verify_signature`.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum VerificationError {
    /// Signature verification is not yet wired to manifest-attest.
    /// Treat as fail-closed; never as "ok."
    #[error("signature verification stub — not yet wired to manifest-attest crypto")]
    NotImplemented,
}

/// Canonical `MAJOR.MINOR.PATCH` (with optional `-pre` / `+build`)
/// semver acceptance — matches the parser in `forge-phases::
/// semver_enforcement` so the validation surface is consistent
/// across the substrate.
fn is_canonical_semver(s: &str) -> bool {
    let core = s.split('-').next().unwrap_or(s);
    let core = core.split('+').next().unwrap_or(core);
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed_entry(id: &str, category: ChangeCategory) -> MigrationEntry {
        let mut e = MigrationEntry {
            id: id.into(),
            from_version: "1.x".into(),
            to_version: "2.0.0".into(),
            artifact_class: "Loom.Primitive.Hero".into(),
            category,
            description: "test migration".into(),
            playbook: None,
            implementation: None,
            test_corpus: None,
            signatures: vec!["ed25519:stub".into(), "ml-dsa:stub".into()],
            signed_by: "test".into(),
            signed_at: "2026-08-15T12:00:00Z".into(),
        };
        match category {
            ChangeCategory::AutoMigration => {
                e.implementation = Some("loom-migrations/src/hero_v1_v2.rs".into());
            }
            ChangeCategory::OperatorAction => {
                e.playbook = Some("docs/migrations/hero-v1-v2.md".into());
            }
            _ => {}
        }
        e
    }

    #[test]
    fn category_slugs_canonical() {
        assert_eq!(ChangeCategory::Invisible.slug(), "invisible");
        assert_eq!(ChangeCategory::Additive.slug(), "additive");
        assert_eq!(ChangeCategory::AutoMigration.slug(), "auto-migration");
        assert_eq!(ChangeCategory::OperatorAction.slug(), "operator-action");
    }

    #[test]
    fn category_requires_registry_for_cat3_cat4_only() {
        assert!(!ChangeCategory::Invisible.requires_registry_entry());
        assert!(!ChangeCategory::Additive.requires_registry_entry());
        assert!(ChangeCategory::AutoMigration.requires_registry_entry());
        assert!(ChangeCategory::OperatorAction.requires_registry_entry());
    }

    #[test]
    fn verify_entry_clean_for_well_formed_automigration() {
        let entry = signed_entry("test-am-1", ChangeCategory::AutoMigration);
        let errs = verify_entry(&entry);
        assert!(errs.is_empty(), "expected clean, got: {errs:?}");
    }

    #[test]
    fn verify_entry_clean_for_well_formed_operator_action() {
        let entry = signed_entry("test-oa-1", ChangeCategory::OperatorAction);
        let errs = verify_entry(&entry);
        assert!(errs.is_empty(), "expected clean, got: {errs:?}");
    }

    #[test]
    fn verify_entry_rejects_missing_signature() {
        let mut entry = signed_entry("test-no-sig", ChangeCategory::AutoMigration);
        entry.signatures.clear();
        let errs = verify_entry(&entry);
        assert!(errs
            .iter()
            .any(|e| matches!(e, RegistryError::MissingSignature { .. })));
    }

    #[test]
    fn verify_entry_rejects_missing_implementation_on_automigration() {
        let mut entry = signed_entry("test-no-impl", ChangeCategory::AutoMigration);
        entry.implementation = None;
        let errs = verify_entry(&entry);
        assert!(errs
            .iter()
            .any(|e| matches!(e, RegistryError::MissingImplementation { .. })));
    }

    #[test]
    fn verify_entry_rejects_missing_playbook_on_operator_action() {
        let mut entry = signed_entry("test-no-pb", ChangeCategory::OperatorAction);
        entry.playbook = None;
        let errs = verify_entry(&entry);
        assert!(errs
            .iter()
            .any(|e| matches!(e, RegistryError::MissingPlaybook { .. })));
    }

    #[test]
    fn verify_entry_rejects_invalid_to_version() {
        let mut entry = signed_entry("test-bad-ver", ChangeCategory::AutoMigration);
        entry.to_version = "latest".into();
        let errs = verify_entry(&entry);
        assert!(errs
            .iter()
            .any(|e| matches!(e, RegistryError::InvalidToVersion { .. })));
    }

    #[test]
    fn verify_entry_rejects_unnecessary_entry_for_invisible() {
        let entry = signed_entry("test-invis", ChangeCategory::Invisible);
        let errs = verify_entry(&entry);
        assert!(errs
            .iter()
            .any(|e| matches!(e, RegistryError::UnnecessaryEntry { .. })));
    }

    #[test]
    fn verify_registry_aggregates_per_entry_errors() {
        let mut bad = signed_entry("bad-entry", ChangeCategory::AutoMigration);
        bad.signatures.clear();
        bad.implementation = None;
        let reg = MigrationRegistry {
            schema_version: "1.0.0".into(),
            migrations: vec![signed_entry("good-1", ChangeCategory::AutoMigration), bad],
        };
        let errs = verify_registry(&reg);
        // good-1 clean; bad-entry has missing sig + missing impl.
        assert_eq!(errs.len(), 2);
    }

    #[test]
    fn verify_registry_catches_duplicate_id() {
        let reg = MigrationRegistry {
            schema_version: "1.0.0".into(),
            migrations: vec![
                signed_entry("dup", ChangeCategory::AutoMigration),
                signed_entry("dup", ChangeCategory::AutoMigration),
            ],
        };
        let errs = verify_registry(&reg);
        assert!(errs
            .iter()
            .any(|e| matches!(e, RegistryError::DuplicateId { .. })));
    }

    #[test]
    fn registry_toml_roundtrips() {
        let reg = MigrationRegistry {
            schema_version: "1.0.0".into(),
            migrations: vec![signed_entry("rt-1", ChangeCategory::AutoMigration)],
        };
        let toml_str = toml::to_string(&reg).expect("serialize");
        let back: MigrationRegistry = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(back.schema_version, "1.0.0");
        assert_eq!(back.migrations.len(), 1);
        assert_eq!(back.migrations[0].id, "rt-1");
        assert_eq!(back.migrations[0].category, ChangeCategory::AutoMigration);
    }

    #[test]
    fn registry_supplies_default_version_when_absent() {
        let toml_str = r#"
[[migrations]]
id = "x"
from_version = "1.x"
to_version = "2.0.0"
artifact_class = "Loom.Primitive.Hero"
category = "auto-migration"
description = "..."
implementation = "loom/x.rs"
signatures = ["ed25519:sig"]
signed_by = "p"
signed_at = "2026-08-15T12:00:00Z"
"#;
        let reg: MigrationRegistry = toml::from_str(toml_str).expect("parse");
        assert_eq!(reg.schema_version, "1.0.0");
        assert_eq!(reg.migrations.len(), 1);
    }

    #[test]
    fn registry_rejects_unknown_top_level_field() {
        let toml_str = r#"
schema_version = "1.0.0"
made_up_field = "x"
"#;
        let r: Result<MigrationRegistry, _> = toml::from_str(toml_str);
        assert!(r.is_err());
    }

    #[test]
    fn entry_rejects_unknown_field() {
        let toml_str = r#"
id = "x"
from_version = "1.x"
to_version = "2.0.0"
artifact_class = "X"
category = "auto-migration"
description = "..."
made_up = "y"
signatures = ["ed25519:s"]
signed_by = "p"
signed_at = "2026-08-15T12:00:00Z"
"#;
        let r: Result<MigrationEntry, _> = toml::from_str(toml_str);
        assert!(r.is_err());
    }

    #[test]
    fn signature_verification_stub_fails_closed() {
        // Per spec: stub returns NotImplemented; callers MUST
        // treat as fail-closed (do not trust).
        let entry = signed_entry("test", ChangeCategory::AutoMigration);
        let r = verify_signature(&entry, "ed25519:fake");
        assert!(r.is_err());
        assert_eq!(r.unwrap_err(), VerificationError::NotImplemented);
    }

    // Test Migration trait via a stub impl.
    struct UpperCaseMigration;
    impl Migration for UpperCaseMigration {
        fn id(&self) -> &str {
            "test-upper-v1-v2"
        }
        fn apply(&self, body: &str) -> Result<String, MigrationError> {
            if body.is_empty() {
                return Err(MigrationError::InputShapeMismatch {
                    id: self.id().into(),
                    reason: "empty body".into(),
                });
            }
            Ok(body.to_uppercase())
        }
    }

    #[test]
    fn migration_trait_apply_succeeds_on_valid_input() {
        let m = UpperCaseMigration;
        let out = m.apply("hello").expect("apply");
        assert_eq!(out, "HELLO");
    }

    #[test]
    fn migration_trait_apply_fails_closed_on_invalid_input() {
        let m = UpperCaseMigration;
        let err = m.apply("").unwrap_err();
        assert!(matches!(err, MigrationError::InputShapeMismatch { .. }));
    }
}
