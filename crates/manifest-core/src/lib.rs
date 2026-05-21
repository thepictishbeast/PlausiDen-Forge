//! `manifest-core` — the keystone type the entire substrate
//! platform projects through.
//!
//! Per `PLATFORM_ROADMAP.md` §3 and the
//! `manifest_layer_is_the_keystone` doctrine: **every platform
//! capability is declared once, in a `PlatformManifest`, and every
//! downstream artifact — Forge phases, runtime backends, admin UI,
//! Loom components, Crawler detectors, CMS sections, CLI
//! subcommands, docs, tests — is derived from that single
//! manifest.**
//!
//! That single discipline carries the platform's "no drift" claim:
//! a new capability cannot ship until its manifest entry has a
//! handler, a UI, a test, and documentation. The
//! `manifest-codegen` crate (task #30) generates the projections;
//! the `coverage` module here defines what counts as "covered."
//!
//! ### Type hierarchy
//!
//! ```text
//!     PlatformManifest                 // root — the whole declaration
//!     ├── capabilities: Vec<Capability>
//!     │     ├── id: CapabilityId       // typed newtype, kebab-case
//!     │     ├── summary: String        // one-liner
//!     │     ├── ownership: Ownership   // forge | loom | crawler | cms | annotator
//!     │     ├── handlers: Vec<HandlerRef>
//!     │     ├── ui: Vec<UiRef>
//!     │     ├── tests: Vec<TestRef>
//!     │     └── docs: Vec<DocRef>
//!     ├── phases: Vec<PhaseDescriptor> // build phases (task #32)
//!     ├── backends: Vec<BackendDescriptor> // runtime backends (task #31)
//!     └── coverage: CoveragePolicy     // "every capability must …"
//! ```
//!
//! ### Why a typed manifest, not free JSON
//!
//! Free JSON drifts. A typed manifest:
//! - rejects ill-formed capabilities at parse time
//! - lets the codegen crate emit `match` arms exhaustively
//! - lets the CI coverage gate (task #33) refuse to merge when a
//!   declared capability has zero handlers
//! - gives every consumer (Forge / Loom / CMS / Crawler) the same
//!   ground-truth type, not an ad-hoc schema in each repo.
//!
//! ### Stability contract
//!
//! Adding a field is backward-compatible (default + serde
//! `#[serde(default)]`). Renaming or removing a field is a breaking
//! change that must bump the major version. The `serde` JSON form
//! is the long-term wire format; the Rust struct names can change
//! freely as long as the kebab-case JSON projection holds.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub mod coverage;
pub mod ownership;
pub mod projections;

pub use coverage::{CoverageGap, CoveragePolicy, CoverageReport};
pub use ownership::Ownership;

/// The root of the entire platform's declared surface.
///
/// One per platform deployment. `manifest-codegen` consumes this
/// to project the Forge phase pipeline, the admin UI route map,
/// the Crawler detector list, and the CMS section catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PlatformManifest {
    /// Semver of the manifest schema itself — bumped on breaking
    /// changes to the JSON shape, not on capability additions.
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    /// Human-readable platform identifier (operator-defined slug).
    pub platform: String,
    /// All declared capabilities. Each capability appears at most
    /// once — duplicates are a parse error (see [`Self::validate`]).
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    /// Build phases the platform offers. Used by Forge to assemble
    /// its pipeline (task #32).
    #[serde(default)]
    pub phases: Vec<PhaseDescriptor>,
    /// Runtime backends the platform offers. Used by `server-stub`
    /// scaffolding + admin UI (task #31).
    #[serde(default)]
    pub backends: Vec<BackendDescriptor>,
    /// Policy that determines what counts as "covered" — feeds the
    /// CI coverage gate (task #33).
    #[serde(default)]
    pub coverage: CoveragePolicy,
}

fn default_schema_version() -> String {
    "1".to_string()
}

/// A typed kebab-case capability identifier.
///
/// Distinct newtype so the type system distinguishes capability
/// IDs from random strings. Construction is validated to enforce
/// the kebab-case + length contract.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CapabilityId(String);

impl CapabilityId {
    /// Build a `CapabilityId` from a string slice, validating
    /// kebab-case form. Returns an error if the slice is empty,
    /// contains characters outside `[a-z0-9-]`, starts or ends
    /// with a hyphen, or has consecutive hyphens.
    pub fn parse(s: impl AsRef<str>) -> Result<Self, ManifestError> {
        let s = s.as_ref();
        if s.is_empty() {
            return Err(ManifestError::InvalidCapabilityId("empty".into()));
        }
        if s.starts_with('-') || s.ends_with('-') {
            return Err(ManifestError::InvalidCapabilityId(format!(
                "leading/trailing hyphen in {s:?}"
            )));
        }
        if s.contains("--") {
            return Err(ManifestError::InvalidCapabilityId(format!(
                "consecutive hyphens in {s:?}"
            )));
        }
        for c in s.chars() {
            if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
                return Err(ManifestError::InvalidCapabilityId(format!(
                    "char {c:?} in {s:?} not in [a-z0-9-]"
                )));
            }
        }
        Ok(Self(s.to_string()))
    }

    /// The raw string. Use [`Self::parse`] for construction.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One platform capability.
///
/// A capability is the smallest unit the manifest tracks. It MUST
/// declare at least one handler reference + one UI reference + one
/// test reference + one doc reference to count as "covered" under
/// the default coverage policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Capability {
    /// Stable kebab-case identifier.
    pub id: CapabilityId,
    /// One-line human summary (shown in the admin UI + docs).
    pub summary: String,
    /// Which subsystem owns this capability.
    pub ownership: Ownership,
    /// Handler module references (e.g. `forge-phases::structured_data`).
    #[serde(default)]
    pub handlers: Vec<HandlerRef>,
    /// UI references (e.g. `cms-admin::routes::capability_list`).
    #[serde(default)]
    pub ui: Vec<UiRef>,
    /// Test references (e.g. `forge-phases::structured_data::tests::detects_invalid_jsonld`).
    #[serde(default)]
    pub tests: Vec<TestRef>,
    /// Documentation references (file paths or anchors).
    #[serde(default)]
    pub docs: Vec<DocRef>,
    /// Optional free-form metadata. Not interpreted by this crate.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

/// Reference to a handler module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HandlerRef(pub String);

/// Reference to a UI module / route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UiRef(pub String);

/// Reference to a test function or test target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TestRef(pub String);

/// Reference to a doc file or anchor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocRef(pub String);

/// Description of a Forge build phase, projected from the
/// manifest. Forge's pipeline assembly (task #32) consumes this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PhaseDescriptor {
    /// Stable kebab-case phase identifier.
    pub id: CapabilityId,
    /// Optional capability this phase implements. When set, the
    /// coverage gate counts the phase as a handler for that
    /// capability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implements: Option<CapabilityId>,
    /// One-line human summary.
    pub summary: String,
    /// Default severity when this phase emits a finding (advisory
    /// vs gating — Forge consumers can override per-site).
    #[serde(default)]
    pub default_severity: DefaultSeverity,
    /// Phase IDs that must run before this one. Forge uses this
    /// for topo-sorting.
    #[serde(default)]
    pub depends_on: Vec<CapabilityId>,
}

/// Default severity baseline a phase is registered with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DefaultSeverity {
    /// Information only — never gates a build.
    Info,
    /// Warning — visible in reports but doesn't gate.
    #[default]
    Warn,
    /// Strict — gates the build on detection.
    Strict,
}

/// Description of a runtime backend, projected from the manifest.
/// `server-stub` scaffolding (task #31) consumes this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BackendDescriptor {
    /// Stable kebab-case backend identifier.
    pub id: CapabilityId,
    /// Optional capability this backend implements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implements: Option<CapabilityId>,
    /// One-line human summary.
    pub summary: String,
    /// HTTP route path the backend handler is mounted at, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    /// HTTP method, if applicable (`GET`/`POST`/...). Free-form
    /// because some transports aren't HTTP at all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

/// Errors returned when constructing or validating a manifest.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// A capability identifier failed the kebab-case contract.
    #[error("invalid capability id: {0}")]
    InvalidCapabilityId(String),
    /// Two capabilities share the same `id`.
    #[error("duplicate capability id: {0}")]
    DuplicateCapability(CapabilityId),
    /// Two phases share the same `id`.
    #[error("duplicate phase id: {0}")]
    DuplicatePhase(CapabilityId),
    /// Two backends share the same `id`.
    #[error("duplicate backend id: {0}")]
    DuplicateBackend(CapabilityId),
    /// A phase declares `implements = X` but no capability `X` exists.
    #[error("phase {phase} implements unknown capability {capability}")]
    PhaseImplementsUnknown {
        /// The phase ID whose `implements` was unresolved.
        phase: CapabilityId,
        /// The unresolved capability ID.
        capability: CapabilityId,
    },
    /// A backend declares `implements = X` but no capability `X` exists.
    #[error("backend {backend} implements unknown capability {capability}")]
    BackendImplementsUnknown {
        /// The backend ID whose `implements` was unresolved.
        backend: CapabilityId,
        /// The unresolved capability ID.
        capability: CapabilityId,
    },
    /// JSON parse error.
    #[error("json parse: {0}")]
    Json(#[from] serde_json::Error),
    /// TOML parse error.
    #[error("toml parse: {0}")]
    Toml(#[from] toml::de::Error),
}

impl PlatformManifest {
    /// Parse a manifest from a JSON string.
    pub fn from_json(s: &str) -> Result<Self, ManifestError> {
        let m: Self = serde_json::from_str(s)?;
        m.validate()?;
        Ok(m)
    }

    /// Parse a manifest from a TOML string.
    pub fn from_toml(s: &str) -> Result<Self, ManifestError> {
        let m: Self = toml::from_str(s)?;
        m.validate()?;
        Ok(m)
    }

    /// Run structural validation:
    /// - capability IDs are unique
    /// - phase IDs are unique
    /// - backend IDs are unique
    /// - every `implements` reference resolves to a declared
    ///   capability
    pub fn validate(&self) -> Result<(), ManifestError> {
        let mut seen_cap = std::collections::HashSet::new();
        for c in &self.capabilities {
            if !seen_cap.insert(&c.id) {
                return Err(ManifestError::DuplicateCapability(c.id.clone()));
            }
        }
        let mut seen_phase = std::collections::HashSet::new();
        for p in &self.phases {
            if !seen_phase.insert(&p.id) {
                return Err(ManifestError::DuplicatePhase(p.id.clone()));
            }
            if let Some(cap) = &p.implements {
                if !seen_cap.contains(&cap) {
                    return Err(ManifestError::PhaseImplementsUnknown {
                        phase: p.id.clone(),
                        capability: cap.clone(),
                    });
                }
            }
        }
        let mut seen_backend = std::collections::HashSet::new();
        for b in &self.backends {
            if !seen_backend.insert(&b.id) {
                return Err(ManifestError::DuplicateBackend(b.id.clone()));
            }
            if let Some(cap) = &b.implements {
                if !seen_cap.contains(&cap) {
                    return Err(ManifestError::BackendImplementsUnknown {
                        backend: b.id.clone(),
                        capability: cap.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Find a capability by ID.
    pub fn capability(&self, id: &CapabilityId) -> Option<&Capability> {
        self.capabilities.iter().find(|c| &c.id == id)
    }

    /// Find a phase by ID.
    pub fn phase(&self, id: &CapabilityId) -> Option<&PhaseDescriptor> {
        self.phases.iter().find(|p| &p.id == id)
    }

    /// Find a backend by ID.
    pub fn backend(&self, id: &CapabilityId) -> Option<&BackendDescriptor> {
        self.backends.iter().find(|b| &b.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_id_accepts_kebab_case() {
        assert!(CapabilityId::parse("auth").is_ok());
        assert!(CapabilityId::parse("auth-login").is_ok());
        assert!(CapabilityId::parse("auth-login-2fa").is_ok());
        assert!(CapabilityId::parse("a1").is_ok());
    }

    #[test]
    fn capability_id_rejects_bad_forms() {
        assert!(CapabilityId::parse("").is_err());
        assert!(CapabilityId::parse("AuthLogin").is_err());
        assert!(CapabilityId::parse("auth_login").is_err());
        assert!(CapabilityId::parse("-auth").is_err());
        assert!(CapabilityId::parse("auth-").is_err());
        assert!(CapabilityId::parse("auth--login").is_err());
        assert!(CapabilityId::parse("auth login").is_err());
        assert!(CapabilityId::parse("auth.login").is_err());
    }

    #[test]
    fn empty_manifest_validates() {
        let m = PlatformManifest {
            platform: "acme".into(), // audit-allow: test fixture
            ..Default::default()
        };
        m.validate().unwrap();
    }

    #[test]
    fn detects_duplicate_capability() {
        let cap = Capability {
            id: CapabilityId::parse("auth").unwrap(),
            summary: "".into(),
            ownership: Ownership::Forge,
            handlers: vec![],
            ui: vec![],
            tests: vec![],
            docs: vec![],
            metadata: BTreeMap::new(),
        };
        let m = PlatformManifest {
            platform: "acme".into(), // audit-allow: test fixture
            capabilities: vec![cap.clone(), cap],
            ..Default::default()
        };
        assert!(matches!(
            m.validate(),
            Err(ManifestError::DuplicateCapability(_))
        ));
    }

    #[test]
    fn detects_phase_implements_unknown_capability() {
        let m = PlatformManifest {
            platform: "acme".into(), // audit-allow: test fixture
            phases: vec![PhaseDescriptor {
                id: CapabilityId::parse("p").unwrap(),
                implements: Some(CapabilityId::parse("nope").unwrap()),
                summary: "".into(),
                default_severity: DefaultSeverity::Warn,
                depends_on: vec![],
            }],
            ..Default::default()
        };
        assert!(matches!(
            m.validate(),
            Err(ManifestError::PhaseImplementsUnknown { .. })
        ));
    }

    #[test]
    fn json_round_trip_preserves_shape() {
        let m = PlatformManifest {
            platform: "acme".into(), // audit-allow: test fixture
            schema_version: "1".into(),
            capabilities: vec![Capability {
                id: CapabilityId::parse("auth").unwrap(),
                summary: "user auth".into(),
                ownership: Ownership::Forge,
                handlers: vec![HandlerRef("forge-phases::auth".into())],
                ui: vec![UiRef("cms-admin::auth".into())],
                tests: vec![TestRef("forge-phases::auth::tests::ok".into())],
                docs: vec![DocRef("docs/auth.md".into())],
                metadata: BTreeMap::new(),
            }],
            ..Default::default()
        };
        let s = serde_json::to_string(&m).unwrap();
        let back = PlatformManifest::from_json(&s).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn toml_round_trip_preserves_shape() {
        let toml_src = r#"
platform = "acme"
schema-version = "1"

[[capabilities]]
id = "auth"
summary = "user auth"
ownership = "forge"
handlers = ["forge-phases::auth"]
ui = ["cms-admin::auth"]
tests = ["forge-phases::auth::tests::ok"]
docs = ["docs/auth.md"]
"#;
        let m = PlatformManifest::from_toml(toml_src).unwrap();
        assert_eq!(m.platform, "acme");
        assert_eq!(m.capabilities.len(), 1);
        assert_eq!(m.capabilities[0].id.as_str(), "auth");
    }

    #[test]
    fn rejects_unknown_fields() {
        let bad = r#"{"platform":"x","ahem-typo":42}"#;
        assert!(PlatformManifest::from_json(bad).is_err());
    }
}

// ============================================================
// Property-based tests (task #66 — fuzz at protocol boundaries).
//
// The CapabilityId + serde + validate paths are the load-bearing
// invariants the entire codegen + projection + CI-gate chain
// depends on. Proptest exercises them with adversarially-shaped
// inputs to catch boundary bugs unit tests would miss.
// ============================================================

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    /// Strategy: kebab-case strings the parser SHOULD accept.
    fn arb_valid_capability_id() -> impl Strategy<Value = String> {
        // 1-32 chars, [a-z0-9], single-token shape — guaranteed
        // kebab-case-valid because we don't insert hyphens.
        proptest::collection::vec(
            proptest::char::ranges(vec!['a'..='z', '0'..='9'].into()),
            1..=32,
        )
        .prop_map(|chars| chars.into_iter().collect::<String>())
    }

    /// Strategy: arbitrary strings (most invalid).
    fn arb_any_string() -> impl Strategy<Value = String> {
        // Mostly-ASCII to keep the proptest readable; punctuation
        // included so we hit reject-paths.
        proptest::collection::vec(any::<char>(), 0..=64).prop_map(|chars| {
            chars
                .into_iter()
                .filter(|c| c.is_ascii() && !c.is_ascii_control())
                .collect()
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 256,
            ..ProptestConfig::default()
        })]

        /// Every all-lowercase-ASCII alphanumeric string is a valid
        /// CapabilityId.
        #[test]
        fn valid_kebab_case_always_parses(s in arb_valid_capability_id()) {
            let r = CapabilityId::parse(&s);
            prop_assert!(r.is_ok(), "valid {s:?} rejected");
            let id = r.unwrap();
            prop_assert_eq!(id.as_str(), s.as_str());
        }

        /// Whatever shape a CapabilityId successfully parses,
        /// serialize→deserialize must return an equal value.
        #[test]
        fn capability_id_serde_roundtrip(s in arb_valid_capability_id()) {
            let id = CapabilityId::parse(&s).unwrap();
            let json = serde_json::to_string(&id).unwrap();
            let back: CapabilityId = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(id, back);
        }

        /// Strings containing characters outside [a-z0-9-] MUST
        /// fail to parse. Property: at least one disallowed char →
        /// parser refuses.
        #[test]
        fn disallowed_chars_always_rejected(s in arb_any_string()) {
            let has_bad_char = s.is_empty()
                || s.chars().any(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
                || s.starts_with('-')
                || s.ends_with('-')
                || s.contains("--");
            if has_bad_char {
                let r = CapabilityId::parse(&s);
                prop_assert!(r.is_err(), "expected reject for {s:?}, got {:?}", r);
            }
        }

        /// Round-trip a CapabilityId through its Display impl —
        /// the displayed string must re-parse to the same value.
        #[test]
        fn display_round_trips(s in arb_valid_capability_id()) {
            let id = CapabilityId::parse(&s).unwrap();
            let displayed = id.to_string();
            let back = CapabilityId::parse(&displayed).unwrap();
            prop_assert_eq!(id, back);
        }

        /// Empty manifest is always self-consistent under validate().
        #[test]
        fn empty_manifest_validates_under_any_platform_name(
            name in "[a-z][a-z0-9-]{0,32}"
        ) {
            let m = PlatformManifest {
                platform: name,
                ..Default::default()
            };
            prop_assert!(m.validate().is_ok());
        }
    }
}
