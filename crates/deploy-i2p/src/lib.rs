//! `deploy-i2p` — I2P eepsite [`DeployAdapter`].
//!
//! Implements the [`DeployAdapter`] trait from `deploy-core` for
//! I2P eepsites (`.b32.i2p` destinations) served either through
//! `i2pd` (C++ daemon) or the Java I2P router. Parallel in shape
//! to `deploy-onion` from #39:
//!
//!   * Parses adapter-specific config from [`DeployTarget::extra`]
//!   * Validates the tunnel-config + private-key paths
//!   * Reads the b32 destination address from the configured
//!     destination-info file (`destinations.txt` or per-tunnel
//!     `<tunnel-name>.dat` snapshot, depending on which router is
//!     in use)
//!   * Reports the right [`SecurityProfile`] (mirrors
//!     `tor_onion_baseline` for the privacy-axes view)
//!
//! ### Why the I2P + Tor adapters look similar
//!
//! Both are overlay networks that:
//!   * separate reader source-address from request content
//!     (Strong reader_anonymity)
//!   * separate publisher source-address from service
//!     destination (Strong publisher_anonymity)
//!   * resist on-path observability (Low traffic_observability)
//!   * resist protocol-level censorship (High
//!     censorship_resistance)
//!
//! Their wire protocols differ — I2P is unidirectional + bundles
//! garlic routing rather than circuits — but the adapter surface
//! is identical from the platform's POV. That's the whole point
//! of the typed deploy-core abstraction: site authors declare
//! "I want anonymous publication" and the adapter handles the
//! wire reality.
//!
//! ### Config schema (`DeployTarget::extra`)
//!
//! ```json
//! {
//!   "tunnel_conf":     "/var/lib/i2pd/tunnels.conf",
//!   "destination_key": "/var/lib/i2pd/site.dat",
//!   "destinations_db": "/var/lib/i2pd/destinations.txt",
//!   "web_root":        "/var/www/i2p-site",
//!   "virtual_port":    80,
//!   "target_port":     8080
//! }
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::path::{Path, PathBuf};

use deploy_core::{
    DeployAdapter, DeployArtifact, DeployError, DeployResult, DeployTarget, NetworkClass,
    SecurityProfile,
};
use serde::{Deserialize, Serialize};

/// Default virtual port the eepsite exposes (HTTP).
pub const DEFAULT_VIRTUAL_PORT: u16 = 80;

/// Default loopback port the local web server listens on.
pub const DEFAULT_TARGET_PORT: u16 = 8080;

/// Parsed adapter-specific config from `DeployTarget::extra`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct I2pConfig {
    /// Path to the I2P tunnel-config file (`tunnels.conf` for
    /// i2pd; `i2ptunnel.config` for the Java router).
    pub tunnel_conf: PathBuf,
    /// Path to the destination's private-key file. Long-lived
    /// secret — the platform never copies or transmits this.
    pub destination_key: PathBuf,
    /// Optional file the router uses to record b32 destination
    /// addresses after registration. When present, the adapter
    /// reads the eepsite's b32 from here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destinations_db: Option<PathBuf>,
    /// Local filesystem path the site content is published to.
    #[serde(default = "default_web_root")]
    pub web_root: PathBuf,
    /// Virtual port the eepsite exposes to readers.
    #[serde(default = "default_virtual_port")]
    pub virtual_port: u16,
    /// Loopback port the local web server is listening on.
    #[serde(default = "default_target_port")]
    pub target_port: u16,
}

fn default_web_root() -> PathBuf {
    PathBuf::from("/var/www/i2p")
}
fn default_virtual_port() -> u16 {
    DEFAULT_VIRTUAL_PORT
}
fn default_target_port() -> u16 {
    DEFAULT_TARGET_PORT
}

impl I2pConfig {
    /// Parse from a [`DeployTarget`]'s `extra` field. Returns
    /// [`DeployError::InvalidTarget`] if the schema is wrong.
    pub fn from_target(target: &DeployTarget) -> Result<Self, DeployError> {
        let json = serde_json::to_value(&target.extra)
            .map_err(|e| DeployError::InvalidTarget(format!("extra → json: {e}")))?;
        serde_json::from_value(json)
            .map_err(|e| DeployError::InvalidTarget(format!("extra schema: {e}")))
    }

    /// Read the `.b32.i2p` address from the destinations DB if
    /// the file exists. Returns `Ok(None)` when the file does
    /// not exist (router hasn't registered the tunnel yet).
    ///
    /// Recognised line forms (one per line):
    ///   * raw b32:           `abcdef...32chars.b32.i2p`
    ///   * `name b32-address` (whitespace-separated)
    pub fn read_b32_address(&self) -> Result<Option<String>, DeployError> {
        let Some(path) = &self.destinations_db else {
            return Ok(None);
        };
        let s = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(DeployError::Io(e)),
        };
        for line in s.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Either "name addr" or just "addr"; take the last token.
            let last = line.split_whitespace().last().unwrap_or("");
            if last.ends_with(".b32.i2p") {
                return Ok(Some(last.to_string()));
            }
        }
        Ok(None)
    }
}

/// The I2P eepsite adapter.
#[derive(Debug, Default, Clone, Copy)]
pub struct I2pAdapter;

impl DeployAdapter for I2pAdapter {
    fn id(&self) -> &'static str {
        "i2p-eepsite"
    }

    fn profile(&self) -> SecurityProfile {
        // I2P's privacy axes match Tor onion's for this view: Strong
        // reader+publisher anonymity, Low traffic observability, High
        // censorship resistance, no standard TLS chain, not
        // content-addressed. Reusing the Tor baseline preset
        // keeps the security dashboard honest — both adapters
        // present equivalent guarantees on the typed axes the
        // platform tracks.
        SecurityProfile::tor_onion_baseline()
    }

    fn validate(&self, target: &DeployTarget) -> Result<(), DeployError> {
        if target.class != NetworkClass::I2pEepsite {
            return Err(DeployError::InvalidTarget(format!(
                "expected class=i2p-eepsite, got {:?}",
                target.class
            )));
        }
        let cfg = I2pConfig::from_target(target)?;
        if cfg.tunnel_conf.as_os_str().is_empty() {
            return Err(DeployError::InvalidTarget(
                "tunnel_conf cannot be empty".into(),
            ));
        }
        if cfg.destination_key.as_os_str().is_empty() {
            return Err(DeployError::InvalidTarget(
                "destination_key cannot be empty".into(),
            ));
        }
        if cfg.virtual_port == 0 || cfg.target_port == 0 {
            return Err(DeployError::InvalidTarget(format!(
                "ports must be > 0 (virtual={} target={})",
                cfg.virtual_port, cfg.target_port,
            )));
        }
        // Existence checks are non-fatal — fresh installs may not
        // have the files yet. Only refuse when something exists
        // but is the wrong type.
        if cfg.tunnel_conf.exists() && cfg.tunnel_conf.is_dir() {
            return Err(DeployError::InvalidTarget(format!(
                "tunnel_conf {} is a directory, expected a file",
                cfg.tunnel_conf.display()
            )));
        }
        if cfg.destination_key.exists() && cfg.destination_key.is_dir() {
            return Err(DeployError::InvalidTarget(format!(
                "destination_key {} is a directory, expected a file",
                cfg.destination_key.display()
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
        let cfg = I2pConfig::from_target(target)?;
        let b32 = cfg.read_b32_address()?;
        let public_url = b32.as_ref().map(|h| format!("http://{h}"));
        let extra = serde_json::json!({
            "tunnel_conf": cfg.tunnel_conf.display().to_string(),
            "virtual_port": cfg.virtual_port,
            "target_port": cfg.target_port,
            "b32_resolved": b32.is_some(),
            "publish_implemented": false,
        });
        Ok(DeployResult {
            target_id: target.id.clone(),
            public_url,
            extra,
        })
    }
}

/// Convenience constructor for a [`DeployTarget`] pointing at an
/// I2P eepsite with the given tunnel-config + destination-key
/// paths and defaults for everything else.
pub fn target_with_defaults(
    id: impl Into<String>,
    tunnel_conf: &Path,
    destination_key: &Path,
) -> DeployTarget {
    let mut extra = std::collections::BTreeMap::new();
    extra.insert(
        "tunnel_conf".to_string(),
        serde_json::Value::String(tunnel_conf.display().to_string()),
    );
    extra.insert(
        "destination_key".to_string(),
        serde_json::Value::String(destination_key.display().to_string()),
    );
    DeployTarget {
        id: id.into(),
        class: NetworkClass::I2pEepsite,
        public_url: None,
        extra,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target_with(extra: serde_json::Map<String, serde_json::Value>) -> DeployTarget {
        DeployTarget {
            id: "test-eepsite".into(),
            class: NetworkClass::I2pEepsite,
            public_url: None,
            extra: extra.into_iter().collect(),
        }
    }

    fn minimal_extra() -> serde_json::Map<String, serde_json::Value> {
        let mut e = serde_json::Map::new();
        e.insert(
            "tunnel_conf".into(),
            serde_json::Value::String("/var/lib/i2pd/tunnels.conf".into()),
        );
        e.insert(
            "destination_key".into(),
            serde_json::Value::String("/var/lib/i2pd/site.dat".into()),
        );
        e
    }

    #[test]
    fn id_is_stable() {
        assert_eq!(I2pAdapter.id(), "i2p-eepsite");
    }

    #[test]
    fn profile_matches_overlay_baseline() {
        let p = I2pAdapter.profile();
        assert_eq!(p, SecurityProfile::tor_onion_baseline());
    }

    #[test]
    fn validate_accepts_minimal_config() {
        let t = target_with(minimal_extra());
        I2pAdapter.validate(&t).unwrap();
    }

    #[test]
    fn validate_refuses_wrong_network_class() {
        let mut t = target_with(minimal_extra());
        t.class = NetworkClass::Clearnet;
        let r = I2pAdapter.validate(&t);
        assert!(matches!(r, Err(DeployError::InvalidTarget(_))));
    }

    #[test]
    fn validate_refuses_unknown_field() {
        let mut extra = minimal_extra();
        extra.insert("ahem_typo".into(), serde_json::Value::Bool(true));
        let t = target_with(extra);
        let r = I2pAdapter.validate(&t);
        assert!(matches!(r, Err(DeployError::InvalidTarget(_))));
    }

    #[test]
    fn validate_refuses_zero_port() {
        let mut extra = minimal_extra();
        extra.insert("virtual_port".into(), serde_json::json!(0));
        let t = target_with(extra);
        let r = I2pAdapter.validate(&t);
        assert!(matches!(r, Err(DeployError::InvalidTarget(_))));
    }

    #[test]
    fn deploy_resolves_b32_when_db_present() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("destinations.txt");
        std::fs::write(
            &db,
            "# auto-generated by i2pd
site abcdefghijklmnopqrstuvwxyz234567.b32.i2p
",
        )
        .unwrap();
        let mut extra = minimal_extra();
        extra.insert(
            "destinations_db".into(),
            serde_json::Value::String(db.display().to_string()),
        );
        let t = target_with(extra);
        let r = I2pAdapter
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
            Some("http://abcdefghijklmnopqrstuvwxyz234567.b32.i2p")
        );
    }

    #[test]
    fn deploy_returns_none_url_when_db_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let t = target_with(minimal_extra());
        let r = I2pAdapter
            .deploy(
                &t,
                &DeployArtifact {
                    site_root: tmp.path().to_path_buf(),
                    manifest_hash: None,
                },
            )
            .unwrap();
        assert!(r.public_url.is_none());
        assert_eq!(r.extra["b32_resolved"], serde_json::Value::Bool(false));
    }

    #[test]
    fn deploy_handles_raw_b32_line() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("destinations.txt");
        std::fs::write(&db, "abcdefghijklmnopqrstuvwxyz234567.b32.i2p\n").unwrap();
        let mut extra = minimal_extra();
        extra.insert(
            "destinations_db".into(),
            serde_json::Value::String(db.display().to_string()),
        );
        let t = target_with(extra);
        let r = I2pAdapter
            .deploy(
                &t,
                &DeployArtifact {
                    site_root: tmp.path().to_path_buf(),
                    manifest_hash: None,
                },
            )
            .unwrap();
        assert!(r.public_url.is_some());
    }

    #[test]
    fn target_with_defaults_round_trips_through_validate() {
        let t = target_with_defaults(
            "x",
            Path::new("/etc/i2pd/tunnels.conf"),
            Path::new("/var/lib/i2pd/x.dat"),
        );
        assert_eq!(t.class, NetworkClass::I2pEepsite);
        assert_eq!(t.id, "x");
        I2pAdapter.validate(&t).unwrap();
    }
}
