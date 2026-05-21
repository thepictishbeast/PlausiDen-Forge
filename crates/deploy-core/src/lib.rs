//! `deploy-core` — typed deployment surface every adapter in the
//! substrate implements.
//!
//! Per `PLATFORM_ROADMAP.md` §4 and the
//! `super_society_tech_stack` doctrine: deployment isn't a single
//! "ship to CDN" verb. The platform supports a constellation of
//! network targets — clearnet, Tor v3 hidden service, I2P eepsite,
//! IPFS/IPNS, Gemini, Lokinet — each with different security +
//! anonymity + censorship-resistance properties. This crate
//! defines the *typed* interface every adapter projects through
//! so:
//!
//!   * site authors declare deploy targets in one place
//!   * the build pipeline (phase_network_target_enforcement)
//!     refuses content that would leak across targets
//!   * the admin UI renders a per-target security rating
//!     (task #43) from the same typed fields adapters report
//!   * adding a new adapter is a single trait implementation;
//!     downstream consumers pick it up automatically.
//!
//! ### Why a trait + a typed enum, not a string slug?
//!
//! A free-form string would let any code declare `target =
//! "tor"` and assume one set of properties. The typed surface:
//!
//!   * forces every adapter to enumerate its
//!     [`SecurityProfile`] at compile time
//!   * lets the [`NetworkClass`] discriminator drive
//!     phase_network_target_enforcement without per-adapter
//!     special-casing
//!   * gives the manifest-codegen layer (task #30) typed
//!     constants per declared target.
//!
//! ### Stability contract
//!
//! Adding a [`NetworkClass`] variant or a [`SecurityProfile`]
//! field with `#[serde(default)]` is backward-compatible. Renames
//! and removals are breaking changes that require a major bump.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod profile;
pub mod target;

pub use profile::{AnonymityLevel, CensorshipResistance, SecurityProfile, TrafficObservability};
pub use target::{DeployTarget, NetworkClass};

/// One concrete artifact a deploy adapter receives — the built
/// site directory + the typed target metadata it's heading to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployArtifact {
    /// Path to the built site root on the local filesystem.
    /// Adapters MUST NOT mutate this directory.
    pub site_root: PathBuf,
    /// Optional manifest hash the build pipeline computed.
    /// Adapters that produce content-addressed URLs (IPFS) include
    /// this in their result.
    pub manifest_hash: Option<String>,
}

/// Result of a single deploy run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DeployResult {
    /// The target this run was destined for.
    pub target_id: String,
    /// Public URL (or onion address / IPNS root / etc.) the
    /// deploy produced. None when the adapter only ships content
    /// to a queue and the URL is announced asynchronously.
    pub public_url: Option<String>,
    /// Adapter-specific extra fields (Tor hostname, IPFS CID,
    /// eepsite b32 hash, etc.). Free-form JSON because every
    /// adapter has a different reporting shape.
    #[serde(default)]
    pub extra: serde_json::Value,
}

/// Errors deploy adapters can return.
#[derive(Debug, thiserror::Error)]
pub enum DeployError {
    /// The configured target has fields incompatible with this adapter.
    #[error("invalid target configuration: {0}")]
    InvalidTarget(String),
    /// A prerequisite was missing (the Tor daemon isn't reachable, the
    /// IPFS daemon isn't running, etc.).
    #[error("prerequisite missing: {0}")]
    Prerequisite(String),
    /// The remote endpoint refused the deploy.
    #[error("remote rejected: {0}")]
    Remote(String),
    /// IO error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Adapter-specific catch-all.
    #[error("adapter: {0}")]
    Other(String),
}

/// Every adapter implements this trait. The platform's CLI +
/// `forge deploy` flow + admin UI all consume adapters through
/// this single interface.
///
/// Implementations are intentionally async-free at this layer —
/// each adapter chooses its own runtime (tokio, smol, blocking
/// IPC, shelling out to `torsocks curl`). Wrap async behavior in
/// the adapter's body if needed. This keeps `deploy-core` itself
/// async-runtime-agnostic.
pub trait DeployAdapter {
    /// Stable kebab-case identifier (e.g. `"tor-onion"`,
    /// `"ipfs-ipns"`).
    fn id(&self) -> &'static str;
    /// The static security/anonymity profile this adapter
    /// provides. Returns the *adapter's intrinsic* properties,
    /// not the deployment's current state.
    fn profile(&self) -> SecurityProfile;
    /// Validate the typed `target` before any side-effects.
    /// Returns `Ok(())` if the adapter can ship to this target;
    /// `Err(DeployError::InvalidTarget(_))` otherwise.
    fn validate(&self, target: &DeployTarget) -> Result<(), DeployError>;
    /// Actually push the artifact to the remote. Adapters block
    /// until the deploy is complete or fails.
    fn deploy(
        &self,
        target: &DeployTarget,
        artifact: &DeployArtifact,
    ) -> Result<DeployResult, DeployError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trivial in-memory adapter used by downstream tests + the
    /// admin UI mock. Confirms the trait shape is implementable.
    struct EchoAdapter;
    impl DeployAdapter for EchoAdapter {
        fn id(&self) -> &'static str {
            "echo"
        }
        fn profile(&self) -> SecurityProfile {
            SecurityProfile::clearnet_baseline()
        }
        fn validate(&self, _target: &DeployTarget) -> Result<(), DeployError> {
            Ok(())
        }
        fn deploy(
            &self,
            target: &DeployTarget,
            _artifact: &DeployArtifact,
        ) -> Result<DeployResult, DeployError> {
            Ok(DeployResult {
                target_id: target.id.clone(),
                public_url: target.public_url.clone(),
                extra: serde_json::Value::Null,
            })
        }
    }

    #[test]
    fn echo_adapter_round_trips() {
        let a = EchoAdapter;
        let t = DeployTarget {
            id: "test".into(),
            class: NetworkClass::Clearnet,
            public_url: Some("https://example.com".into()),
            extra: Default::default(),
        };
        let art = DeployArtifact {
            site_root: PathBuf::from("/tmp/site"),
            manifest_hash: None,
        };
        a.validate(&t).unwrap();
        let r = a.deploy(&t, &art).unwrap();
        assert_eq!(r.target_id, "test");
        assert_eq!(r.public_url.as_deref(), Some("https://example.com"));
        assert_eq!(a.id(), "echo");
    }

    #[test]
    fn deploy_error_io_conversion() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "x");
        let e: DeployError = io.into();
        assert!(matches!(e, DeployError::Io(_)));
    }
}
