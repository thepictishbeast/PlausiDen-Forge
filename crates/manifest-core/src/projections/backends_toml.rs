//! Bridge from the app-level `backends.toml` schema into the
//! manifest-core [`BackendDescriptor`](crate::BackendDescriptor)
//! surface.
//!
//! The existing `backends.toml` (used by `forge-phases::phantom_button`
//! and `forge-phases::backend_coverage`) carries app-specific data:
//!
//! ```toml
//! [meta]
//! project = "SkillShots"
//! api_base = "https://api.skillshots.com"
//! schema_version = 1
//!
//! [backends.sign-in]
//! method     = "POST"
//! path       = "/auth/sign-in"
//! purpose    = "operator sign-in"
//! impl_files = ["src/handlers/sign_in.rs"]
//! ```
//!
//! That schema predates the manifest layer. This module projects
//! it through the keystone without forcing existing consumers to
//! change file format: pre-existing parsers keep working, new
//! consumers go through the typed projection.
//!
//! ### Bridge contract
//!
//! For each `[backends.NAME]` table we emit a
//! `BackendDescriptor` where:
//!   * `id`         — `NAME` (validated kebab-case)
//!   * `summary`    — the `purpose` string
//!   * `route`      — the `path` string
//!   * `method`     — the `method` string
//!   * `implements` — `None` (apps can override later by adding
//!                    `implements = "..."` to the TOML; we read
//!                    that field if present)
//!
//! Backends marked stub (empty `impl_files`) are still emitted —
//! consumers like the coverage gate decide what to do with them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{BackendDescriptor, CapabilityId, ManifestError};

/// Top-level shape of the app's `backends.toml` file.
///
/// Doesn't derive `Eq` because `toml::Value` contains `f64` and
/// can't be `Eq` — `PartialEq` is sufficient for the tests we run
/// here, and consumers comparing meta tables must do it
/// explicitly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct BackendsToml {
    /// Free-form `[meta]` table. We don't interpret the contents
    /// — anything the app wants to keep alongside its backend
    /// declarations.
    #[serde(default)]
    pub meta: BTreeMap<String, toml::Value>,
    /// Map of backend ID → declaration.
    #[serde(default)]
    pub backends: BTreeMap<String, BackendEntry>,
}

/// One `[backends.NAME]` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct BackendEntry {
    /// HTTP method (e.g. `GET` / `POST`).
    pub method: String,
    /// Route path (e.g. `/auth/sign-in`).
    pub path: String,
    /// Human-readable purpose.
    pub purpose: String,
    /// Filesystem paths of the handler implementation(s).
    /// Empty == stub (used by the coverage phase to flag PARTIAL).
    #[serde(default)]
    pub impl_files: Vec<String>,
    /// Optional capability ID this backend implements. New field
    /// — pre-existing entries omit it and project as
    /// `implements = None`.
    #[serde(default)]
    pub implements: Option<String>,
}

impl BackendsToml {
    /// Parse from a raw TOML string.
    pub fn from_toml(s: &str) -> Result<Self, ManifestError> {
        let parsed: Self = toml::from_str(s)?;
        Ok(parsed)
    }

    /// Project every `[backends.NAME]` entry into a
    /// [`BackendDescriptor`]. Stable iteration order (the underlying
    /// `BTreeMap` sorts by ID).
    pub fn to_descriptors(&self) -> Result<Vec<BackendDescriptor>, ManifestError> {
        let mut out = Vec::with_capacity(self.backends.len());
        for (id, entry) in &self.backends {
            let cap_id = CapabilityId::parse(id)?;
            let implements = match &entry.implements {
                Some(s) => Some(CapabilityId::parse(s)?),
                None => None,
            };
            out.push(BackendDescriptor {
                id: cap_id,
                implements,
                summary: entry.purpose.clone(),
                route: Some(entry.path.clone()),
                method: Some(entry.method.clone()),
            });
        }
        Ok(out)
    }

    /// Return IDs of stub backends (those with empty `impl_files`).
    /// Used by the coverage gate (task #33) to flag PARTIAL.
    pub fn stub_ids(&self) -> Vec<String> {
        self.backends
            .iter()
            .filter(|(_, e)| e.impl_files.is_empty())
            .map(|(id, _)| id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[meta]
project = "test"
schema_version = 1

[backends.sign-in]
method   = "POST"
path     = "/auth/sign-in"
purpose  = "operator sign-in"
impl_files = ["src/handlers/sign_in.rs"]

[backends.list-touches]
method   = "GET"
path     = "/wallet"
purpose  = "wallet dashboard (My Wins)"
impl_files = ["src/handlers/list_touches.rs"]

[backends.stub-x]
method   = "POST"
path     = "/x"
purpose  = "queued"
impl_files = []
"#;

    #[test]
    fn parses_meta_and_backends() {
        let bt = BackendsToml::from_toml(SAMPLE).unwrap();
        assert_eq!(bt.backends.len(), 3);
        assert!(bt.meta.contains_key("project"));
        assert_eq!(bt.backends["sign-in"].method, "POST");
        assert_eq!(bt.backends["sign-in"].path, "/auth/sign-in");
    }

    #[test]
    fn projects_to_backend_descriptors_in_id_order() {
        let bt = BackendsToml::from_toml(SAMPLE).unwrap();
        let descs = bt.to_descriptors().unwrap();
        let ids: Vec<&str> = descs.iter().map(|d| d.id.as_str()).collect();
        // BTreeMap ordering: alphabetical.
        assert_eq!(ids, vec!["list-touches", "sign-in", "stub-x"]);
        let sign_in = descs.iter().find(|d| d.id.as_str() == "sign-in").unwrap();
        assert_eq!(sign_in.summary, "operator sign-in");
        assert_eq!(sign_in.route.as_deref(), Some("/auth/sign-in"));
        assert_eq!(sign_in.method.as_deref(), Some("POST"));
        assert_eq!(sign_in.implements, None);
    }

    #[test]
    fn detects_stub_backends() {
        let bt = BackendsToml::from_toml(SAMPLE).unwrap();
        assert_eq!(bt.stub_ids(), vec!["stub-x"]);
    }

    #[test]
    fn rejects_non_kebab_backend_id() {
        let bad = r#"
[backends.SignIn]
method   = "POST"
path     = "/x"
purpose  = "x"
impl_files = []
"#;
        let bt = BackendsToml::from_toml(bad).unwrap();
        let err = bt.to_descriptors().unwrap_err();
        assert!(matches!(err, ManifestError::InvalidCapabilityId(_)));
    }

    #[test]
    fn carries_implements_field_when_present() {
        let with_impl = r#"
[backends.sign-in]
method   = "POST"
path     = "/auth/sign-in"
purpose  = "operator sign-in"
impl_files = ["src/handlers/sign_in.rs"]
implements = "auth"
"#;
        let bt = BackendsToml::from_toml(with_impl).unwrap();
        let descs = bt.to_descriptors().unwrap();
        assert_eq!(descs[0].implements.as_ref().unwrap().as_str(), "auth");
    }

    #[test]
    fn projects_the_repo_backends_toml() {
        // The actual file at the workspace root. If this fails after a
        // legitimate schema change, regenerate against the new shape.
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../backends.toml");
        let s = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => return, // skip if the file isn't present (e.g. partial clone)
        };
        let bt = BackendsToml::from_toml(&s).expect("repo backends.toml parses");
        let descs = bt.to_descriptors().expect("repo backends.toml projects");
        // The current repo file declares ≥ 18 backends; the gate
        // guards against accidental erosion of the existing surface.
        assert!(
            descs.len() >= 18,
            "expected ≥ 18 backends in repo file, got {}",
            descs.len()
        );
        // Every ID must round-trip through CapabilityId validation —
        // this would have failed at to_descriptors() but we double-check
        // for explicit invariant documentation.
        for d in &descs {
            assert!(!d.id.as_str().is_empty());
            assert!(d.method.is_some());
            assert!(d.route.is_some());
        }
    }

    #[test]
    fn rejects_unknown_fields_in_entry() {
        let bad = r#"
[backends.sign-in]
method   = "POST"
path     = "/x"
purpose  = "x"
impl_files = []
ahem_typo = 1
"#;
        let err = BackendsToml::from_toml(bad).unwrap_err();
        assert!(matches!(err, ManifestError::Toml(_)));
    }
}
