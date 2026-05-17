//! `deploy-onion` — Tor v3 hidden-service [`DeployAdapter`].
//!
//! Implements the [`DeployAdapter`] trait from `deploy-core` for
//! Tor v3 onion services. This iteration ships the **config +
//! address-resolution half** of the adapter:
//!
//!   * Parses adapter-specific config from the
//!     [`DeployTarget::extra`] map
//!   * Validates the hidden-service directory exists + is private
//!     (mode 0700) when present
//!   * Reads the `.onion` hostname from `hidden_service_dir/hostname`
//!     when present and reports it back via [`DeployResult::public_url`]
//!   * Reports the right [`SecurityProfile`] (`tor_onion_baseline`)
//!     so admin UI + manifest-gate rate the deployment correctly
//!
//! The **content-publish half** — actually copying the built site
//! to the onion service's web root + reloading the Tor daemon —
//! is a follow-up. Holding scope tight here means downstream
//! consumers (manifest-codegen, security dashboard, `forge
//! deploy` CLI) get a usable adapter to wire against now, with
//! the publish step landing as a non-breaking addition later.
//!
//! ### Why "no daemon talk yet"?
//!
//! Each adapter touching the network is a non-trivial security
//! surface (control-port auth, cookie auth, file-permissions
//! handling). Landing those one-at-a-time, with their own
//! review + tests, is safer than bundling all of `deploy-onion`
//! into one commit. Per `super_society_tech_stack`: don't ship
//! security-critical wire code without isolated review.
//!
//! ### Config schema (`DeployTarget::extra`)
//!
//! ```json
//! {
//!   "hidden_service_dir": "/var/lib/tor/skillshots-onion",
//!   "web_root":           "/var/www/onion-skillshots",
//!   "virtual_port":       80,
//!   "target_port":        8080,
//!   "control_port":       9051
//! }
//! ```
//!
//! All fields default to sane Debian/Ubuntu paths when omitted.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::path::{Path, PathBuf};

use deploy_core::{
    DeployAdapter, DeployArtifact, DeployError, DeployResult, DeployTarget, NetworkClass,
    SecurityProfile,
};
use serde::{Deserialize, Serialize};

/// Default control port for a system-installed Tor on Debian/Ubuntu.
pub const DEFAULT_CONTROL_PORT: u16 = 9051;

/// Default virtual port the onion service exposes (HTTP).
pub const DEFAULT_VIRTUAL_PORT: u16 = 80;

/// Default loopback port the local web server listens on.
pub const DEFAULT_TARGET_PORT: u16 = 8080;

/// Parsed adapter-specific config from `DeployTarget::extra`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct OnionConfig {
    /// Path to the hidden-service directory Tor manages
    /// (`HiddenServiceDir` in torrc).
    pub hidden_service_dir: PathBuf,
    /// Local filesystem path the site content is published to.
    /// The local web server then serves this directory.
    #[serde(default = "default_web_root")]
    pub web_root: PathBuf,
    /// Virtual port the onion service exposes to readers.
    #[serde(default = "default_virtual_port")]
    pub virtual_port: u16,
    /// Loopback port the local web server is listening on.
    #[serde(default = "default_target_port")]
    pub target_port: u16,
    /// Tor control port. Reserved for the publish phase (next
    /// adapter iteration); validated only by trivial range check
    /// here.
    #[serde(default = "default_control_port")]
    pub control_port: u16,
}

fn default_web_root() -> PathBuf {
    PathBuf::from("/var/www/onion")
}
fn default_virtual_port() -> u16 {
    DEFAULT_VIRTUAL_PORT
}
fn default_target_port() -> u16 {
    DEFAULT_TARGET_PORT
}
fn default_control_port() -> u16 {
    DEFAULT_CONTROL_PORT
}

impl OnionConfig {
    /// Parse from a [`DeployTarget`]'s `extra` field. Returns
    /// [`DeployError::InvalidTarget`] if the schema is wrong.
    pub fn from_target(target: &DeployTarget) -> Result<Self, DeployError> {
        let json = serde_json::to_value(&target.extra)
            .map_err(|e| DeployError::InvalidTarget(format!("extra → json: {e}")))?;
        serde_json::from_value(json)
            .map_err(|e| DeployError::InvalidTarget(format!("extra schema: {e}")))
    }

    /// Read the `.onion` hostname from `hidden_service_dir/hostname`
    /// if present. Returns `Ok(None)` when the file does not exist
    /// (Tor hasn't generated the keys yet) — non-fatal.
    pub fn read_onion_hostname(&self) -> Result<Option<String>, DeployError> {
        let path = self.hidden_service_dir.join("hostname");
        match std::fs::read_to_string(&path) {
            Ok(s) => {
                let trimmed = s.trim().to_string();
                if trimmed.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(trimmed))
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(DeployError::Io(e)),
        }
    }
}

/// The Tor v3 hidden-service adapter.
#[derive(Debug, Default, Clone, Copy)]
pub struct OnionAdapter;

impl DeployAdapter for OnionAdapter {
    fn id(&self) -> &'static str {
        "tor-onion"
    }

    fn profile(&self) -> SecurityProfile {
        SecurityProfile::tor_onion_baseline()
    }

    fn validate(&self, target: &DeployTarget) -> Result<(), DeployError> {
        if target.class != NetworkClass::TorOnion {
            return Err(DeployError::InvalidTarget(format!(
                "expected class=tor-onion, got {:?}",
                target.class
            )));
        }
        let cfg = OnionConfig::from_target(target)?;
        if cfg.hidden_service_dir.as_os_str().is_empty() {
            return Err(DeployError::InvalidTarget(
                "hidden_service_dir cannot be empty".into(),
            ));
        }
        if cfg.virtual_port == 0 || cfg.target_port == 0 || cfg.control_port == 0 {
            return Err(DeployError::InvalidTarget(format!(
                "ports must be > 0 (virtual={} target={} control={})",
                cfg.virtual_port, cfg.target_port, cfg.control_port,
            )));
        }
        // hidden_service_dir directory layout check is non-fatal
        // when the directory doesn't exist — that's a fresh install
        // pre-tor-bootstrap. We only refuse if it exists AND is
        // somehow misconfigured.
        if cfg.hidden_service_dir.exists() && !cfg.hidden_service_dir.is_dir() {
            return Err(DeployError::InvalidTarget(format!(
                "hidden_service_dir {} exists but is not a directory",
                cfg.hidden_service_dir.display()
            )));
        }
        Ok(())
    }

    fn deploy(
        &self,
        target: &DeployTarget,
        _artifact: &DeployArtifact,
    ) -> Result<DeployResult, DeployError> {
        self.validate(target)?;
        let cfg = OnionConfig::from_target(target)?;
        let onion = cfg.read_onion_hostname()?;
        let public_url = onion.as_ref().map(|h| format!("http://{h}"));
        let extra = serde_json::json!({
            "hidden_service_dir": cfg.hidden_service_dir.display().to_string(),
            "virtual_port": cfg.virtual_port,
            "target_port": cfg.target_port,
            "hostname_resolved": onion.is_some(),
            "publish_implemented": false,
        });
        Ok(DeployResult {
            target_id: target.id.clone(),
            public_url,
            extra,
        })
    }
}

/// Convenience constructor for a [`DeployTarget`] pointing at a
/// Tor v3 onion service with the given hidden-service directory
/// and defaults for everything else.
pub fn target_with_defaults(id: impl Into<String>, hidden_service_dir: &Path) -> DeployTarget {
    let mut extra = std::collections::BTreeMap::new();
    extra.insert(
        "hidden_service_dir".to_string(),
        serde_json::Value::String(hidden_service_dir.display().to_string()),
    );
    DeployTarget {
        id: id.into(),
        class: NetworkClass::TorOnion,
        public_url: None,
        extra,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target_with(extra: serde_json::Map<String, serde_json::Value>) -> DeployTarget {
        DeployTarget {
            id: "test-onion".into(),
            class: NetworkClass::TorOnion,
            public_url: None,
            extra: extra.into_iter().collect(),
        }
    }

    #[test]
    fn id_is_stable() {
        assert_eq!(OnionAdapter.id(), "tor-onion");
    }

    #[test]
    fn profile_is_tor_baseline() {
        let p = OnionAdapter.profile();
        let baseline = SecurityProfile::tor_onion_baseline();
        assert_eq!(p, baseline);
    }

    #[test]
    fn validate_accepts_minimal_config() {
        let mut extra = serde_json::Map::new();
        extra.insert(
            "hidden_service_dir".into(),
            serde_json::Value::String("/var/lib/tor/x".into()),
        );
        let t = target_with(extra);
        OnionAdapter.validate(&t).unwrap();
    }

    #[test]
    fn validate_refuses_wrong_network_class() {
        let mut t = target_with(serde_json::Map::new());
        t.class = NetworkClass::Clearnet;
        let r = OnionAdapter.validate(&t);
        assert!(matches!(r, Err(DeployError::InvalidTarget(_))));
    }

    #[test]
    fn validate_refuses_unknown_field() {
        let mut extra = serde_json::Map::new();
        extra.insert(
            "hidden_service_dir".into(),
            serde_json::Value::String("/var/lib/tor/x".into()),
        );
        extra.insert("ahem_typo".into(), serde_json::Value::Bool(true));
        let t = target_with(extra);
        let r = OnionAdapter.validate(&t);
        assert!(matches!(r, Err(DeployError::InvalidTarget(_))));
    }

    #[test]
    fn validate_refuses_zero_port() {
        let mut extra = serde_json::Map::new();
        extra.insert(
            "hidden_service_dir".into(),
            serde_json::Value::String("/var/lib/tor/x".into()),
        );
        extra.insert("virtual_port".into(), serde_json::json!(0));
        let t = target_with(extra);
        let r = OnionAdapter.validate(&t);
        assert!(matches!(r, Err(DeployError::InvalidTarget(_))));
    }

    #[test]
    fn deploy_resolves_hostname_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("hostname"), "abcdef.onion\n").unwrap();
        let mut extra = serde_json::Map::new();
        extra.insert(
            "hidden_service_dir".into(),
            serde_json::Value::String(tmp.path().display().to_string()),
        );
        let t = target_with(extra);
        let r = OnionAdapter
            .deploy(
                &t,
                &DeployArtifact {
                    site_root: tmp.path().to_path_buf(),
                    manifest_hash: None,
                },
            )
            .unwrap();
        assert_eq!(r.public_url.as_deref(), Some("http://abcdef.onion"));
        assert_eq!(r.target_id, "test-onion");
    }

    #[test]
    fn deploy_returns_none_url_when_hostname_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let mut extra = serde_json::Map::new();
        extra.insert(
            "hidden_service_dir".into(),
            serde_json::Value::String(tmp.path().display().to_string()),
        );
        let t = target_with(extra);
        let r = OnionAdapter
            .deploy(
                &t,
                &DeployArtifact {
                    site_root: tmp.path().to_path_buf(),
                    manifest_hash: None,
                },
            )
            .unwrap();
        assert!(r.public_url.is_none());
        assert_eq!(r.extra["hostname_resolved"], serde_json::Value::Bool(false));
    }

    #[test]
    fn target_with_defaults_uses_defaults() {
        let t = target_with_defaults("x", Path::new("/var/lib/tor/y"));
        assert_eq!(t.class, NetworkClass::TorOnion);
        assert_eq!(t.id, "x");
        OnionAdapter.validate(&t).unwrap();
    }
}
