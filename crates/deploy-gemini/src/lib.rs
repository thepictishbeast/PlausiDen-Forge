//! `deploy-gemini` — Gemini protocol (smolnet) [`DeployAdapter`].
//!
//! Gemini is a text-first hypermedia protocol (RFC-not-yet, but
//! widely-adopted spec at <gemini://gemini.circumlunar.space/>).
//! Capsules are directories of `.gmi` files served over TLS on
//! port 1965. Server identity is established via TLS-on-TOFU.
//!
//! Same scope rule as the other adapters: this iteration ships
//! the config + capsule-root-validation half. The actual TLS
//! server + cert hot-reload land in a follow-up.
//!
//! Per `super_society_tech_stack`: Gemini is the "fast +
//! reliable + private" axis pulled to its extreme — no scripts,
//! no inline anything, no third-party state. Adding it to the
//! deploy adapter set means a site can publish a stripped-down
//! reading view alongside its clearnet + Tor + I2P targets.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::path::{Path, PathBuf};

use deploy_core::{
    AnonymityLevel, CensorshipResistance, DeployAdapter, DeployArtifact, DeployError, DeployResult,
    DeployTarget, NetworkClass, SecurityProfile, TrafficObservability,
};
use serde::{Deserialize, Serialize};

/// IANA-assigned Gemini port.
pub const DEFAULT_GEMINI_PORT: u16 = 1965;

/// Parsed adapter-specific config from `DeployTarget::extra`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct GeminiConfig {
    /// Hostname the capsule is served under (e.g.
    /// `gemini.example.com`).
    pub hostname: String,
    /// Port to listen on (default 1965).
    #[serde(default = "default_port")]
    pub port: u16,
    /// Local filesystem path the capsule (`.gmi` files) is
    /// rooted at.
    pub capsule_root: PathBuf,
    /// PEM-encoded server certificate path. TLS is mandatory in
    /// Gemini.
    pub cert_pem: PathBuf,
    /// PEM-encoded server private key path. Long-lived secret;
    /// the adapter NEVER reads or transmits its contents.
    pub key_pem: PathBuf,
}

fn default_port() -> u16 {
    DEFAULT_GEMINI_PORT
}

impl GeminiConfig {
    /// Parse from a [`DeployTarget`]'s `extra` field.
    pub fn from_target(target: &DeployTarget) -> Result<Self, DeployError> {
        let json = serde_json::to_value(&target.extra)
            .map_err(|e| DeployError::InvalidTarget(format!("extra → json: {e}")))?;
        serde_json::from_value(json)
            .map_err(|e| DeployError::InvalidTarget(format!("extra schema: {e}")))
    }

    /// Build the public URL the capsule serves at.
    pub fn public_url(&self) -> String {
        if self.port == DEFAULT_GEMINI_PORT {
            format!("gemini://{}/", self.hostname)
        } else {
            format!("gemini://{}:{}/", self.hostname, self.port)
        }
    }
}

/// The Gemini protocol adapter.
#[derive(Debug, Default, Clone, Copy)]
pub struct GeminiAdapter;

impl DeployAdapter for GeminiAdapter {
    fn id(&self) -> &'static str {
        "gemini"
    }

    fn profile(&self) -> SecurityProfile {
        // Gemini's privacy axes: requests are mTLS-style with
        // self-signed certs (no CA chain), but reader IPs are
        // visible to the server (no overlay). Traffic observable
        // by ISP via SNI. Censorship resistance: protocol-level
        // blocking is non-trivial because Gemini servers don't
        // share infrastructure with HTTPS the way DNS-over-HTTPS
        // does — but state-level IP blocks still work.
        SecurityProfile {
            reader_anonymity: AnonymityLevel::None,
            publisher_anonymity: AnonymityLevel::None,
            traffic_observability: TrafficObservability::High,
            censorship_resistance: CensorshipResistance::Low,
            content_addressed: false,
            uses_standard_tls: false, // Gemini uses TLS but ignores CA chain (TOFU).
        }
    }

    fn validate(&self, target: &DeployTarget) -> Result<(), DeployError> {
        if target.class != NetworkClass::Gemini {
            return Err(DeployError::InvalidTarget(format!(
                "expected class=gemini, got {:?}",
                target.class
            )));
        }
        let cfg = GeminiConfig::from_target(target)?;
        if cfg.hostname.is_empty() {
            return Err(DeployError::InvalidTarget(
                "hostname cannot be empty".into(),
            ));
        }
        if cfg.port == 0 {
            return Err(DeployError::InvalidTarget("port must be > 0".into()));
        }
        if cfg.capsule_root.as_os_str().is_empty() {
            return Err(DeployError::InvalidTarget(
                "capsule_root cannot be empty".into(),
            ));
        }
        if cfg.cert_pem.as_os_str().is_empty() || cfg.key_pem.as_os_str().is_empty() {
            return Err(DeployError::InvalidTarget(
                "cert_pem and key_pem cannot be empty".into(),
            ));
        }
        if cfg.capsule_root.exists() && !cfg.capsule_root.is_dir() {
            return Err(DeployError::InvalidTarget(format!(
                "capsule_root {} exists but is not a directory",
                cfg.capsule_root.display()
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
        let cfg = GeminiConfig::from_target(target)?;
        let extra = serde_json::json!({
            "hostname": cfg.hostname,
            "port": cfg.port,
            "capsule_root": cfg.capsule_root.display().to_string(),
            "publish_implemented": false,
        });
        Ok(DeployResult {
            target_id: target.id.clone(),
            public_url: Some(cfg.public_url()),
            extra,
        })
    }
}

/// Convenience constructor with defaults for everything except
/// hostname + capsule root + cert paths.
pub fn target_with_defaults(
    id: impl Into<String>,
    hostname: impl Into<String>,
    capsule_root: &Path,
    cert_pem: &Path,
    key_pem: &Path,
) -> DeployTarget {
    let mut extra = std::collections::BTreeMap::new();
    extra.insert(
        "hostname".to_string(),
        serde_json::Value::String(hostname.into()),
    );
    extra.insert(
        "capsule_root".to_string(),
        serde_json::Value::String(capsule_root.display().to_string()),
    );
    extra.insert(
        "cert_pem".to_string(),
        serde_json::Value::String(cert_pem.display().to_string()),
    );
    extra.insert(
        "key_pem".to_string(),
        serde_json::Value::String(key_pem.display().to_string()),
    );
    DeployTarget {
        id: id.into(),
        class: NetworkClass::Gemini,
        public_url: None,
        extra,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target_with(extra: serde_json::Map<String, serde_json::Value>) -> DeployTarget {
        DeployTarget {
            id: "test-gemini".into(),
            class: NetworkClass::Gemini,
            public_url: None,
            extra: extra.into_iter().collect(),
        }
    }

    fn minimal_extra() -> serde_json::Map<String, serde_json::Value> {
        let mut e = serde_json::Map::new();
        e.insert(
            "hostname".into(),
            serde_json::Value::String("gemini.example.com".into()),
        );
        e.insert(
            "capsule_root".into(),
            serde_json::Value::String("/var/gemini/capsule".into()),
        );
        e.insert(
            "cert_pem".into(),
            serde_json::Value::String("/etc/gemini/cert.pem".into()),
        );
        e.insert(
            "key_pem".into(),
            serde_json::Value::String("/etc/gemini/key.pem".into()),
        );
        e
    }

    #[test]
    fn id_is_stable() {
        assert_eq!(GeminiAdapter.id(), "gemini");
    }

    #[test]
    fn profile_reports_non_standard_tls_and_no_anonymity() {
        let p = GeminiAdapter.profile();
        assert!(!p.uses_standard_tls);
        assert!(!p.content_addressed);
        assert_eq!(p.reader_anonymity, AnonymityLevel::None);
    }

    #[test]
    fn validate_accepts_minimal_config() {
        let t = target_with(minimal_extra());
        GeminiAdapter.validate(&t).unwrap();
    }

    #[test]
    fn validate_refuses_wrong_class() {
        let mut t = target_with(minimal_extra());
        t.class = NetworkClass::Clearnet;
        assert!(GeminiAdapter.validate(&t).is_err());
    }

    #[test]
    fn validate_refuses_empty_hostname() {
        let mut extra = minimal_extra();
        extra.insert("hostname".into(), serde_json::Value::String(String::new()));
        let t = target_with(extra);
        assert!(GeminiAdapter.validate(&t).is_err());
    }

    #[test]
    fn validate_refuses_unknown_field() {
        let mut extra = minimal_extra();
        extra.insert("ahem".into(), serde_json::Value::Bool(true));
        let t = target_with(extra);
        assert!(GeminiAdapter.validate(&t).is_err());
    }

    #[test]
    fn url_omits_default_port() {
        let cfg = GeminiConfig {
            hostname: "g.example".into(),
            port: 1965,
            capsule_root: PathBuf::from("/cap"),
            cert_pem: PathBuf::from("/c"),
            key_pem: PathBuf::from("/k"),
        };
        assert_eq!(cfg.public_url(), "gemini://g.example/");
    }

    #[test]
    fn url_includes_non_default_port() {
        let cfg = GeminiConfig {
            hostname: "g.example".into(),
            port: 1966,
            capsule_root: PathBuf::from("/cap"),
            cert_pem: PathBuf::from("/c"),
            key_pem: PathBuf::from("/k"),
        };
        assert_eq!(cfg.public_url(), "gemini://g.example:1966/");
    }

    #[test]
    fn deploy_reports_public_url() {
        let tmp = tempfile::tempdir().unwrap();
        let mut extra = minimal_extra();
        extra.insert(
            "capsule_root".into(),
            serde_json::Value::String(tmp.path().display().to_string()),
        );
        let t = target_with(extra);
        let r = GeminiAdapter
            .deploy(
                &t,
                &DeployArtifact {
                    site_root: tmp.path().to_path_buf(),
                    manifest_hash: None,
                },
            )
            .unwrap();
        assert!(r.public_url.as_ref().unwrap().starts_with("gemini://"));
    }

    #[test]
    fn target_with_defaults_validates() {
        let t = target_with_defaults(
            "x",
            "g.example",
            Path::new("/var/gemini"),
            Path::new("/etc/g/cert.pem"),
            Path::new("/etc/g/key.pem"),
        );
        GeminiAdapter.validate(&t).unwrap();
    }
}
