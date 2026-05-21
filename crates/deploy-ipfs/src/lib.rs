//! `deploy-ipfs` — IPFS / IPNS [`DeployAdapter`].
//!
//! Third leaf network-target adapter on top of `deploy-core`.
//! Structurally different from `deploy-onion` + `deploy-i2p`
//! because IPFS is **content-addressed**: the "address" is the
//! hash of the content itself (a CID), and the operator's
//! long-lived identity is an IPNS key that maps a stable name to
//! the most-recently-published CID.
//!
//! ### What this iteration ships
//!
//! Same scope rule as the onion + I2P adapters: config + address
//! resolution half. No daemon talk.
//!
//!   * Parses adapter-specific config from [`DeployTarget::extra`]
//!   * Validates the IPNS key name + the local CID-snapshot file
//!     paths
//!   * Reads the most-recent published CID from the snapshot file
//!     (operator's local record of "what's currently published
//!     under this IPNS name") and projects:
//!         - `public_url` = `<gateway>/ipns/<key>` (mutable view)
//!         - `extra.current_cid` = the CID (immutable view)
//!   * Reports `SecurityProfile::ipfs_baseline()` so admin UI
//!     correctly shows the content-addressed property
//!
//! ### Why content-addressed changes the security shape
//!
//! Per `super_society_tech_stack`: the right rating for "1000
//! years from now" content addressing is its strongest feature.
//! Tampering with a CID produces a different CID — a censor
//! can suppress access but cannot rewrite. That's why
//! `SecurityProfile::ipfs_baseline()` sets `content_addressed:
//! true` and `censorship_resistance: Medium` (active enough to
//! resist content forgery; gateways can still be blocked, hence
//! not High).
//!
//! ### Config schema (`DeployTarget::extra`)
//!
//! ```json
//! {
//!   "api_endpoint":   "http://127.0.0.1:5001/api/v0",
//!   "gateway_url":    "https://ipfs.io",
//!   "ipns_key_name":  "site-pub",
//!   "cid_snapshot":   "/var/lib/forge/ipns-current-cid.txt",
//!   "pin_remote":     null
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

/// Default Kubo/IPFS API endpoint.
pub const DEFAULT_API_ENDPOINT: &str = "http://127.0.0.1:5001/api/v0";

/// Default public gateway used when none is configured.
pub const DEFAULT_GATEWAY: &str = "https://ipfs.io";

/// Parsed adapter-specific config from `DeployTarget::extra`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct IpfsConfig {
    /// IPFS HTTP API endpoint. Reserved for the publish phase;
    /// validated only by trivial URL-ish check here.
    #[serde(default = "default_api_endpoint")]
    pub api_endpoint: String,
    /// Public gateway URL the adapter resolves IPNS through for
    /// reporting. Operator-chosen because gateway selection is a
    /// privacy decision (your visitors' IPs end up there).
    #[serde(default = "default_gateway")]
    pub gateway_url: String,
    /// IPNS key name registered with the daemon. The
    /// content-mutable handle that the platform publishes under.
    pub ipns_key_name: String,
    /// Local-fs snapshot file the adapter reads to learn the
    /// most-recent CID published under `ipns_key_name`. Updated
    /// by the publish step (next adapter iteration).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cid_snapshot: Option<PathBuf>,
    /// Optional remote pinning service identifier (e.g.
    /// `"web3-storage"` or `"pinata"`). Reserved for the publish
    /// phase — not consulted in this iteration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_remote: Option<String>,
}

fn default_api_endpoint() -> String {
    DEFAULT_API_ENDPOINT.to_string()
}
fn default_gateway() -> String {
    DEFAULT_GATEWAY.to_string()
}

impl IpfsConfig {
    /// Parse from a [`DeployTarget`]'s `extra` field. Returns
    /// [`DeployError::InvalidTarget`] if the schema is wrong.
    pub fn from_target(target: &DeployTarget) -> Result<Self, DeployError> {
        let json = serde_json::to_value(&target.extra)
            .map_err(|e| DeployError::InvalidTarget(format!("extra → json: {e}")))?;
        serde_json::from_value(json)
            .map_err(|e| DeployError::InvalidTarget(format!("extra schema: {e}")))
    }

    /// Read the current CID from the configured snapshot file
    /// (if present). Returns `Ok(None)` when the file does not
    /// exist (first publish hasn't happened yet) — non-fatal.
    pub fn read_current_cid(&self) -> Result<Option<String>, DeployError> {
        let Some(path) = &self.cid_snapshot else {
            return Ok(None);
        };
        match std::fs::read_to_string(path) {
            Ok(s) => {
                let trimmed = s.trim().to_string();
                if trimmed.is_empty() {
                    Ok(None)
                } else if looks_like_cid(&trimmed) {
                    Ok(Some(trimmed))
                } else {
                    Err(DeployError::InvalidTarget(format!(
                        "cid_snapshot {} contains {:?} which doesn't look like a CID",
                        path.display(),
                        trimmed
                    )))
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(DeployError::Io(e)),
        }
    }

    /// Build the IPNS public URL (mutable view) by joining gateway
    /// + key name.
    pub fn ipns_public_url(&self) -> String {
        let gw = self.gateway_url.trim_end_matches('/');
        format!("{gw}/ipns/{}", self.ipns_key_name)
    }
}

/// Cheap CID-shape check. CIDv0 starts with `Qm` and is 46 chars
/// of base58; CIDv1 starts with `b` followed by base32. This is a
/// shape filter, not a real CID parse — bringing in a CID parser
/// crate is out of scope for the validation-only layer.
fn looks_like_cid(s: &str) -> bool {
    if s.len() < 32 || s.len() > 128 {
        return false;
    }
    let cidv0 =
        s.starts_with("Qm") && s.len() == 46 && s.chars().all(|c| c.is_ascii_alphanumeric());
    let cidv1 = s.starts_with('b') && s.chars().skip(1).all(|c| c.is_ascii_alphanumeric());
    cidv0 || cidv1
}

/// The IPFS / IPNS adapter.
#[derive(Debug, Default, Clone, Copy)]
pub struct IpfsAdapter;

impl DeployAdapter for IpfsAdapter {
    fn id(&self) -> &'static str {
        "ipfs"
    }

    fn profile(&self) -> SecurityProfile {
        SecurityProfile::ipfs_baseline()
    }

    fn validate(&self, target: &DeployTarget) -> Result<(), DeployError> {
        if target.class != NetworkClass::Ipfs {
            return Err(DeployError::InvalidTarget(format!(
                "expected class=ipfs, got {:?}",
                target.class
            )));
        }
        let cfg = IpfsConfig::from_target(target)?;
        if cfg.ipns_key_name.is_empty() {
            return Err(DeployError::InvalidTarget(
                "ipns_key_name cannot be empty".into(),
            ));
        }
        if !cfg.api_endpoint.starts_with("http://") && !cfg.api_endpoint.starts_with("https://") {
            return Err(DeployError::InvalidTarget(format!(
                "api_endpoint {:?} must be http:// or https://",
                cfg.api_endpoint
            )));
        }
        if !cfg.gateway_url.starts_with("http://") && !cfg.gateway_url.starts_with("https://") {
            return Err(DeployError::InvalidTarget(format!(
                "gateway_url {:?} must be http:// or https://",
                cfg.gateway_url
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
        let cfg = IpfsConfig::from_target(target)?;
        let cid = cfg.read_current_cid()?;
        let ipns_url = cfg.ipns_public_url();
        let extra = serde_json::json!({
            "api_endpoint": cfg.api_endpoint,
            "gateway_url": cfg.gateway_url,
            "ipns_key_name": cfg.ipns_key_name,
            "current_cid": cid.clone().unwrap_or_default(),
            "cid_resolved": cid.is_some(),
            "publish_implemented": false,
        });
        Ok(DeployResult {
            target_id: target.id.clone(),
            // IPNS is the mutable view; we always report it because
            // operators want one stable bookmark. The CID is in
            // extra for the immutable view.
            public_url: Some(ipns_url),
            extra,
        })
    }
}

/// Convenience constructor for an IPFS deploy target with sane
/// defaults for everything except the IPNS key name.
pub fn target_with_defaults(
    id: impl Into<String>,
    ipns_key_name: impl Into<String>,
    cid_snapshot: Option<&Path>,
) -> DeployTarget {
    let mut extra = std::collections::BTreeMap::new();
    extra.insert(
        "ipns_key_name".to_string(),
        serde_json::Value::String(ipns_key_name.into()),
    );
    if let Some(p) = cid_snapshot {
        extra.insert(
            "cid_snapshot".to_string(),
            serde_json::Value::String(p.display().to_string()),
        );
    }
    DeployTarget {
        id: id.into(),
        class: NetworkClass::Ipfs,
        public_url: None,
        extra,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target_with(extra: serde_json::Map<String, serde_json::Value>) -> DeployTarget {
        DeployTarget {
            id: "test-ipfs".into(),
            class: NetworkClass::Ipfs,
            public_url: None,
            extra: extra.into_iter().collect(),
        }
    }

    fn minimal_extra() -> serde_json::Map<String, serde_json::Value> {
        let mut e = serde_json::Map::new();
        e.insert(
            "ipns_key_name".into(),
            serde_json::Value::String("test-key".into()),
        );
        e
    }

    #[test]
    fn id_is_stable() {
        assert_eq!(IpfsAdapter.id(), "ipfs");
    }

    #[test]
    fn profile_is_ipfs_baseline() {
        let p = IpfsAdapter.profile();
        assert!(p.content_addressed);
        assert_eq!(p, SecurityProfile::ipfs_baseline());
    }

    #[test]
    fn validate_accepts_minimal_config() {
        let t = target_with(minimal_extra());
        IpfsAdapter.validate(&t).unwrap();
    }

    #[test]
    fn validate_refuses_wrong_network_class() {
        let mut t = target_with(minimal_extra());
        t.class = NetworkClass::Clearnet;
        let r = IpfsAdapter.validate(&t);
        assert!(matches!(r, Err(DeployError::InvalidTarget(_))));
    }

    #[test]
    fn validate_refuses_empty_key_name() {
        let mut extra = minimal_extra();
        extra.insert(
            "ipns_key_name".into(),
            serde_json::Value::String(String::new()),
        );
        let t = target_with(extra);
        let r = IpfsAdapter.validate(&t);
        assert!(matches!(r, Err(DeployError::InvalidTarget(_))));
    }

    #[test]
    fn validate_refuses_non_http_endpoint() {
        let mut extra = minimal_extra();
        extra.insert(
            "api_endpoint".into(),
            serde_json::Value::String("ftp://x".into()),
        );
        let t = target_with(extra);
        let r = IpfsAdapter.validate(&t);
        assert!(matches!(r, Err(DeployError::InvalidTarget(_))));
    }

    #[test]
    fn validate_refuses_unknown_field() {
        let mut extra = minimal_extra();
        extra.insert("ahem_typo".into(), serde_json::Value::Bool(true));
        let t = target_with(extra);
        let r = IpfsAdapter.validate(&t);
        assert!(matches!(r, Err(DeployError::InvalidTarget(_))));
    }

    #[test]
    fn deploy_resolves_cid_when_snapshot_present_v0() {
        let tmp = tempfile::tempdir().unwrap();
        let snap = tmp.path().join("cid.txt");
        let cid_v0 = "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG";
        std::fs::write(&snap, format!("{cid_v0}\n")).unwrap();
        let mut extra = minimal_extra();
        extra.insert(
            "cid_snapshot".into(),
            serde_json::Value::String(snap.display().to_string()),
        );
        let t = target_with(extra);
        let r = IpfsAdapter
            .deploy(
                &t,
                &DeployArtifact {
                    site_root: tmp.path().to_path_buf(),
                    manifest_hash: None,
                },
            )
            .unwrap();
        assert_eq!(
            r.extra["current_cid"],
            serde_json::Value::String(cid_v0.into())
        );
        assert_eq!(r.extra["cid_resolved"], serde_json::Value::Bool(true));
        assert!(r.public_url.as_deref().unwrap().ends_with("/ipns/test-key"));
    }

    #[test]
    fn deploy_resolves_cid_v1() {
        let tmp = tempfile::tempdir().unwrap();
        let snap = tmp.path().join("cid.txt");
        let cid_v1 = "bafybeibwzifw7pwsfvbqxnp7g6q5b6fjazyqp5l4lhqf6yqfjf7zxlcvji";
        std::fs::write(&snap, format!("{cid_v1}\n")).unwrap();
        let mut extra = minimal_extra();
        extra.insert(
            "cid_snapshot".into(),
            serde_json::Value::String(snap.display().to_string()),
        );
        let t = target_with(extra);
        let r = IpfsAdapter
            .deploy(
                &t,
                &DeployArtifact {
                    site_root: tmp.path().to_path_buf(),
                    manifest_hash: None,
                },
            )
            .unwrap();
        assert_eq!(
            r.extra["current_cid"],
            serde_json::Value::String(cid_v1.into())
        );
    }

    #[test]
    fn deploy_returns_unresolved_when_snapshot_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let t = target_with(minimal_extra());
        let r = IpfsAdapter
            .deploy(
                &t,
                &DeployArtifact {
                    site_root: tmp.path().to_path_buf(),
                    manifest_hash: None,
                },
            )
            .unwrap();
        assert_eq!(r.extra["cid_resolved"], serde_json::Value::Bool(false));
        // IPNS URL is always reported — operators want the stable
        // handle even before the first publish.
        assert!(r.public_url.is_some());
    }

    #[test]
    fn deploy_rejects_garbage_snapshot_content() {
        let tmp = tempfile::tempdir().unwrap();
        let snap = tmp.path().join("cid.txt");
        std::fs::write(&snap, "not a CID\n").unwrap();
        let mut extra = minimal_extra();
        extra.insert(
            "cid_snapshot".into(),
            serde_json::Value::String(snap.display().to_string()),
        );
        let t = target_with(extra);
        let r = IpfsAdapter.deploy(
            &t,
            &DeployArtifact {
                site_root: tmp.path().to_path_buf(),
                manifest_hash: None,
            },
        );
        assert!(matches!(r, Err(DeployError::InvalidTarget(_))));
    }

    #[test]
    fn target_with_defaults_round_trips() {
        let t = target_with_defaults("x", "mykey", None);
        assert_eq!(t.class, NetworkClass::Ipfs);
        IpfsAdapter.validate(&t).unwrap();
    }

    #[test]
    fn ipns_url_builds_correctly() {
        let cfg = IpfsConfig {
            api_endpoint: "http://x".into(),
            gateway_url: "https://example.com/".into(),
            ipns_key_name: "abc".into(),
            cid_snapshot: None,
            pin_remote: None,
        };
        assert_eq!(cfg.ipns_public_url(), "https://example.com/ipns/abc");
    }
}
