//! `deploy-lokinet` — Lokinet (mesh-overlay) [`DeployAdapter`].
//!
//! Lokinet is an Oxen-network onion-routing protocol, similar in
//! shape to Tor but with built-in DNS-style name resolution via
//! ONS (Oxen Name Service). Addresses are either raw `.loki`
//! pubkey hashes or human-readable names registered through ONS.
//!
//! Same scope rule as the other adapters: this iteration ships
//! the config + ONS-name resolution half. Daemon talk lands in a
//! follow-up.
//!
//! Per `super_society_tech_stack`: Lokinet matches Tor + I2P on
//! every privacy axis the platform tracks, but offers ONS as a
//! distinguishing UX feature — operators can advertise
//! `mycapsule.loki` instead of a 52-character hash. Adding it to
//! the adapter set widens the alt-network coverage one further
//! step.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::path::{Path, PathBuf};

use deploy_core::{
    DeployAdapter, DeployArtifact, DeployError, DeployResult, DeployTarget, NetworkClass,
    SecurityProfile,
};
use serde::{Deserialize, Serialize};

/// Parsed adapter-specific config from `DeployTarget::extra`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct LokinetConfig {
    /// Path to the lokinet config file (`lokinet.ini` typically).
    pub config_ini: PathBuf,
    /// Path to the long-lived service key. The adapter NEVER
    /// reads or transmits this — only references the path so the
    /// future publish step can verify the daemon has registered
    /// the right identity.
    pub service_key: PathBuf,
    /// Optional ONS name registered for the service (e.g.
    /// `mycapsule.loki`). When present, the adapter reports it as
    /// the public URL; otherwise the raw `.loki` hash from the
    /// address-snapshot file is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ons_name: Option<String>,
    /// Local-fs snapshot file the adapter reads to learn the raw
    /// `.loki` address (`<pubkey>.loki`). Updated by the publish
    /// step (next adapter iteration).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address_snapshot: Option<PathBuf>,
    /// Local filesystem path the site content is published to.
    #[serde(default = "default_web_root")]
    pub web_root: PathBuf,
    /// Virtual port the service exposes to readers.
    #[serde(default = "default_virtual_port")]
    pub virtual_port: u16,
    /// Loopback port the local web server is listening on.
    #[serde(default = "default_target_port")]
    pub target_port: u16,
}

fn default_web_root() -> PathBuf {
    PathBuf::from("/var/www/lokinet")
}
fn default_virtual_port() -> u16 {
    80
}
fn default_target_port() -> u16 {
    8080
}

impl LokinetConfig {
    /// Parse from a [`DeployTarget`]'s `extra` field.
    pub fn from_target(target: &DeployTarget) -> Result<Self, DeployError> {
        let json = serde_json::to_value(&target.extra)
            .map_err(|e| DeployError::InvalidTarget(format!("extra → json: {e}")))?;
        serde_json::from_value(json)
            .map_err(|e| DeployError::InvalidTarget(format!("extra schema: {e}")))
    }

    /// Read the raw `.loki` address from the configured snapshot
    /// file. Returns `Ok(None)` if file missing or empty.
    pub fn read_address(&self) -> Result<Option<String>, DeployError> {
        let Some(path) = &self.address_snapshot else {
            return Ok(None);
        };
        let s = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(DeployError::Io(e)),
        };
        let trimmed = s.trim().to_string();
        if trimmed.is_empty() {
            return Ok(None);
        }
        if !trimmed.ends_with(".loki") {
            return Err(DeployError::InvalidTarget(format!(
                "address_snapshot {} contains {:?} which doesn't end .loki",
                path.display(),
                trimmed
            )));
        }
        Ok(Some(trimmed))
    }

    /// Best resolvable public URL — ONS name when set, raw `.loki`
    /// address otherwise.
    pub fn resolve_public_url(&self) -> Result<Option<String>, DeployError> {
        if let Some(name) = &self.ons_name {
            return Ok(Some(format!("http://{name}")));
        }
        if let Some(addr) = self.read_address()? {
            return Ok(Some(format!("http://{addr}")));
        }
        Ok(None)
    }
}

/// The Lokinet adapter.
#[derive(Debug, Default, Clone, Copy)]
pub struct LokinetAdapter;

impl DeployAdapter for LokinetAdapter {
    fn id(&self) -> &'static str {
        "lokinet"
    }

    fn profile(&self) -> SecurityProfile {
        // Lokinet matches Tor + I2P on every privacy axis the
        // platform tracks. Reusing the Tor baseline preset keeps
        // the dashboard honest — three overlay-network adapters,
        // one consistent rating shape.
        SecurityProfile::tor_onion_baseline()
    }

    fn validate(&self, target: &DeployTarget) -> Result<(), DeployError> {
        if target.class != NetworkClass::Lokinet {
            return Err(DeployError::InvalidTarget(format!(
                "expected class=lokinet, got {:?}",
                target.class
            )));
        }
        let cfg = LokinetConfig::from_target(target)?;
        if cfg.config_ini.as_os_str().is_empty() {
            return Err(DeployError::InvalidTarget(
                "config_ini cannot be empty".into(),
            ));
        }
        if cfg.service_key.as_os_str().is_empty() {
            return Err(DeployError::InvalidTarget(
                "service_key cannot be empty".into(),
            ));
        }
        if cfg.virtual_port == 0 || cfg.target_port == 0 {
            return Err(DeployError::InvalidTarget(format!(
                "ports must be > 0 (virtual={} target={})",
                cfg.virtual_port, cfg.target_port,
            )));
        }
        if let Some(name) = &cfg.ons_name {
            if !name.ends_with(".loki") {
                return Err(DeployError::InvalidTarget(format!(
                    "ons_name {:?} must end with .loki",
                    name
                )));
            }
        }
        if cfg.config_ini.exists() && cfg.config_ini.is_dir() {
            return Err(DeployError::InvalidTarget(format!(
                "config_ini {} is a directory, expected a file",
                cfg.config_ini.display()
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
        let cfg = LokinetConfig::from_target(target)?;
        let public_url = cfg.resolve_public_url()?;
        let extra = serde_json::json!({
            "config_ini": cfg.config_ini.display().to_string(),
            "virtual_port": cfg.virtual_port,
            "target_port": cfg.target_port,
            "ons_name_set": cfg.ons_name.is_some(),
            "address_resolved": public_url.is_some(),
            "publish_implemented": false,
        });
        Ok(DeployResult {
            target_id: target.id.clone(),
            public_url,
            extra,
        })
    }
}

/// Convenience constructor.
pub fn target_with_defaults(
    id: impl Into<String>,
    config_ini: &Path,
    service_key: &Path,
) -> DeployTarget {
    let mut extra = std::collections::BTreeMap::new();
    extra.insert(
        "config_ini".to_string(),
        serde_json::Value::String(config_ini.display().to_string()),
    );
    extra.insert(
        "service_key".to_string(),
        serde_json::Value::String(service_key.display().to_string()),
    );
    DeployTarget {
        id: id.into(),
        class: NetworkClass::Lokinet,
        public_url: None,
        extra,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target_with(extra: serde_json::Map<String, serde_json::Value>) -> DeployTarget {
        DeployTarget {
            id: "test-lokinet".into(),
            class: NetworkClass::Lokinet,
            public_url: None,
            extra: extra.into_iter().collect(),
        }
    }

    fn minimal_extra() -> serde_json::Map<String, serde_json::Value> {
        let mut e = serde_json::Map::new();
        e.insert(
            "config_ini".into(),
            serde_json::Value::String("/etc/loki/lokinet.ini".into()),
        );
        e.insert(
            "service_key".into(),
            serde_json::Value::String("/var/lib/lokinet/x.key".into()),
        );
        e
    }

    #[test]
    fn id_is_stable() {
        assert_eq!(LokinetAdapter.id(), "lokinet");
    }

    #[test]
    fn profile_matches_overlay_baseline() {
        let p = LokinetAdapter.profile();
        assert_eq!(p, SecurityProfile::tor_onion_baseline());
    }

    #[test]
    fn validate_accepts_minimal_config() {
        let t = target_with(minimal_extra());
        LokinetAdapter.validate(&t).unwrap();
    }

    #[test]
    fn validate_refuses_wrong_class() {
        let mut t = target_with(minimal_extra());
        t.class = NetworkClass::Clearnet;
        assert!(LokinetAdapter.validate(&t).is_err());
    }

    #[test]
    fn validate_refuses_unknown_field() {
        let mut extra = minimal_extra();
        extra.insert("ahem".into(), serde_json::Value::Bool(true));
        let t = target_with(extra);
        assert!(LokinetAdapter.validate(&t).is_err());
    }

    #[test]
    fn validate_refuses_non_loki_ons() {
        let mut extra = minimal_extra();
        extra.insert(
            "ons_name".into(),
            serde_json::Value::String("not-loki.example".into()),
        );
        let t = target_with(extra);
        assert!(LokinetAdapter.validate(&t).is_err());
    }

    #[test]
    fn validate_accepts_loki_ons() {
        let mut extra = minimal_extra();
        extra.insert(
            "ons_name".into(),
            serde_json::Value::String("mycapsule.loki".into()),
        );
        let t = target_with(extra);
        LokinetAdapter.validate(&t).unwrap();
    }

    #[test]
    fn deploy_prefers_ons_over_raw_address() {
        let tmp = tempfile::tempdir().unwrap();
        let snap = tmp.path().join("addr.txt");
        std::fs::write(&snap, "abcdef1234567890.loki\n").unwrap();
        let mut extra = minimal_extra();
        extra.insert(
            "ons_name".into(),
            serde_json::Value::String("nice.loki".into()),
        );
        extra.insert(
            "address_snapshot".into(),
            serde_json::Value::String(snap.display().to_string()),
        );
        let t = target_with(extra);
        let r = LokinetAdapter
            .deploy(
                &t,
                &DeployArtifact {
                    site_root: tmp.path().to_path_buf(),
                    manifest_hash: None,
                },
            )
            .unwrap();
        // ONS name wins over the raw address.
        assert_eq!(r.public_url.as_deref(), Some("http://nice.loki"));
        assert_eq!(r.extra["ons_name_set"], serde_json::Value::Bool(true));
    }

    #[test]
    fn deploy_falls_back_to_raw_address() {
        let tmp = tempfile::tempdir().unwrap();
        let snap = tmp.path().join("addr.txt");
        std::fs::write(&snap, "abcdef1234567890.loki\n").unwrap();
        let mut extra = minimal_extra();
        extra.insert(
            "address_snapshot".into(),
            serde_json::Value::String(snap.display().to_string()),
        );
        let t = target_with(extra);
        let r = LokinetAdapter
            .deploy(
                &t,
                &DeployArtifact {
                    site_root: tmp.path().to_path_buf(),
                    manifest_hash: None,
                },
            )
            .unwrap();
        assert_eq!(
            r.public_url.as_deref(),
            Some("http://abcdef1234567890.loki")
        );
    }

    #[test]
    fn deploy_returns_none_url_when_neither_set() {
        let tmp = tempfile::tempdir().unwrap();
        let t = target_with(minimal_extra());
        let r = LokinetAdapter
            .deploy(
                &t,
                &DeployArtifact {
                    site_root: tmp.path().to_path_buf(),
                    manifest_hash: None,
                },
            )
            .unwrap();
        assert!(r.public_url.is_none());
        assert_eq!(r.extra["address_resolved"], serde_json::Value::Bool(false));
    }

    #[test]
    fn deploy_rejects_non_loki_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let snap = tmp.path().join("addr.txt");
        std::fs::write(&snap, "not-a-loki-address\n").unwrap();
        let mut extra = minimal_extra();
        extra.insert(
            "address_snapshot".into(),
            serde_json::Value::String(snap.display().to_string()),
        );
        let t = target_with(extra);
        let r = LokinetAdapter.deploy(
            &t,
            &DeployArtifact {
                site_root: tmp.path().to_path_buf(),
                manifest_hash: None,
            },
        );
        assert!(matches!(r, Err(DeployError::InvalidTarget(_))));
    }

    #[test]
    fn target_with_defaults_validates() {
        let t = target_with_defaults(
            "x",
            Path::new("/etc/loki/lokinet.ini"),
            Path::new("/var/lib/lokinet/x.key"),
        );
        LokinetAdapter.validate(&t).unwrap();
    }
}
