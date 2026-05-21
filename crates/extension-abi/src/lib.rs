//! `extension-abi` — typed ABI every extension projects
//! through.
//!
//! Per `PLATFORM_ROADMAP.md` §6 + the
//! `super_society_tech_stack` doctrine: third-party extensions
//! must run in a sandbox the platform can audit and revoke. Wasm
//! Component Model is the planned backend (no host filesystem, no
//! ambient network, every host call goes through a typed import).
//!
//! This crate is **backend-independent** — it defines the typed
//! ABI so an extension can be compiled once and run against:
//!   * the planned wasmtime backend (full Wasm Component Model
//!     isolation)
//!   * the NullHost backend (cargo-test-friendly, no side
//!     effects)
//!   * any future backend (native subprocess, V8 isolate, etc.)
//!
//! ### Why typed-ABI-first
//!
//! Bringing in wasmtime is a 200-transitive-dep commitment.
//! Defining the ABI typed surface first lets every consumer
//! (Forge phase loader, CMS plugin slot, Loom theme extension)
//! integrate against a stable interface even while the wasmtime
//! backend is still landing. The typed surface is the contract;
//! the runtime is the implementation.
//!
//! ### Public surface
//!
//! - [`ExtensionId`]        — kebab-case newtype
//! - [`ExtensionManifest`]  — declared extension metadata +
//!                             requested capabilities
//! - [`Capability`]         — closed-enum what extensions can ask
//!                             for (read-fs / network-egress /
//!                             timer / log / event-emit / …)
//! - [`InvokeRequest`]      — typed host→extension invoke
//! - [`InvokeResponse`]     — typed extension→host return
//! - [`AbiError`]           — typed errors at every boundary

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Typed kebab-case extension identifier.
///
/// Same shape contract as `manifest_core::CapabilityId` and
/// `tenancy_core::TenantId` — kebab-case, [a-z0-9-], must start
/// with [a-z], 1..=64 chars, no consecutive hyphens, no
/// leading/trailing hyphen.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExtensionId(String);

impl ExtensionId {
    /// Maximum identifier length.
    pub const MAX_LEN: usize = 64;

    /// Build an `ExtensionId` from a string slice, validating shape.
    pub fn parse(s: impl AsRef<str>) -> Result<Self, AbiError> {
        let s = s.as_ref();
        if s.is_empty() {
            return Err(AbiError::InvalidId("empty".into()));
        }
        if s.len() > Self::MAX_LEN {
            return Err(AbiError::InvalidId(format!(
                "{s:?} exceeds {} chars",
                Self::MAX_LEN
            )));
        }
        if !s
            .chars()
            .next()
            .map(|c| c.is_ascii_lowercase())
            .unwrap_or(false)
        {
            return Err(AbiError::InvalidId(format!("{s:?} must start with [a-z]")));
        }
        if s.ends_with('-') {
            return Err(AbiError::InvalidId(format!("{s:?} has trailing hyphen")));
        }
        if s.contains("--") {
            return Err(AbiError::InvalidId(format!(
                "{s:?} has consecutive hyphens"
            )));
        }
        for c in s.chars() {
            if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
                return Err(AbiError::InvalidId(format!(
                    "{s:?} contains {c:?} not in [a-z0-9-]"
                )));
            }
        }
        Ok(Self(s.to_string()))
    }

    /// Raw string slice. Use [`Self::parse`] for construction.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ExtensionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Capability an extension can request. Closed enum — every new
/// capability is a reviewed schema change, not a free-form string.
///
/// The host runtime grants only the capabilities the operator
/// explicitly approves at install time. Extensions cannot
/// escalate at runtime.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Capability {
    /// Read files under a declared root. Paths outside the root
    /// are refused by the host before they reach the extension.
    ReadFs {
        /// Filesystem root, relative to the tenant's fs_root.
        /// Empty == tenant root.
        root: String,
    },
    /// Write files under a declared root.
    WriteFs {
        /// Filesystem root, relative to the tenant's fs_root.
        root: String,
    },
    /// Make outgoing HTTP requests to one of an allowlisted set
    /// of hostnames. The host enforces the allowlist; the
    /// extension cannot widen it at runtime.
    NetworkEgress {
        /// Allowlisted hostnames the extension may reach.
        hosts: Vec<String>,
    },
    /// Schedule a one-shot timer.
    Timer,
    /// Emit structured log events under the extension's name.
    /// Always available; declaring it lets the operator see
    /// "this extension logs" at install time.
    Log,
    /// Emit typed platform events that other consumers subscribe
    /// to (build hooks, CMS publish hooks, etc.).
    EventEmit {
        /// Channel names the extension is allowed to publish on.
        channels: Vec<String>,
    },
    /// Read environment variables matching a declared prefix.
    EnvRead {
        /// Required prefix (e.g. `"FORGE_EXT_"`).
        prefix: String,
    },
}

impl Capability {
    /// Stable kebab-case slug naming the capability variant.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::ReadFs { .. } => "read-fs",
            Self::WriteFs { .. } => "write-fs",
            Self::NetworkEgress { .. } => "network-egress",
            Self::Timer => "timer",
            Self::Log => "log",
            Self::EventEmit { .. } => "event-emit",
            Self::EnvRead { .. } => "env-read",
        }
    }
}

/// Declared extension metadata. Lives next to the wasm module as
/// `extension.toml` or `extension.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ExtensionManifest {
    /// Stable identifier.
    pub id: ExtensionId,
    /// Semver of the extension itself.
    pub version: String,
    /// One-line human summary shown in the admin UI.
    pub summary: String,
    /// Capabilities the extension requests. Order is
    /// preserved + significant for review tooling.
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    /// Wasm ABI version the extension was built against.
    /// Mismatch with the host's supported set is a load-time
    /// refusal.
    #[serde(default = "default_abi_version")]
    pub abi_version: String,
}

fn default_abi_version() -> String {
    SUPPORTED_ABI_VERSION.to_string()
}

/// ABI version this crate ships. Extensions declare an
/// `abi_version` in their manifest; hosts refuse to load
/// extensions whose declared version isn't in
/// [`ExtensionManifest::COMPATIBLE_ABI_VERSIONS`].
pub const SUPPORTED_ABI_VERSION: &str = "1";

impl ExtensionManifest {
    /// ABI versions the current host can run. Single-element for
    /// v1; future versions add backward-compatible entries.
    pub const COMPATIBLE_ABI_VERSIONS: &'static [&'static str] = &[SUPPORTED_ABI_VERSION];

    /// Check whether `self.abi_version` is in the compatible set.
    pub fn abi_is_compatible(&self) -> bool {
        Self::COMPATIBLE_ABI_VERSIONS
            .iter()
            .any(|v| *v == self.abi_version)
    }
}

/// One typed host→extension invocation. Free-form `args` is
/// JSON because every extension speaks its own schema; the host
/// validates the wire shape, the extension validates the
/// semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct InvokeRequest {
    /// The extension to invoke.
    pub extension: ExtensionId,
    /// Function symbol exported by the extension.
    pub function: String,
    /// JSON arguments.
    #[serde(default)]
    pub args: serde_json::Value,
}

/// Result of one invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct InvokeResponse {
    /// Typed status — `ok` or `err(detail)`.
    #[serde(flatten)]
    pub status: InvokeStatus,
    /// JSON-serialized return value (free shape per extension).
    #[serde(default)]
    pub value: serde_json::Value,
    /// Log lines the extension emitted during this call.
    #[serde(default)]
    pub logs: Vec<String>,
}

/// Result discriminator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum InvokeStatus {
    /// Function returned cleanly.
    Ok,
    /// Function trapped or returned a typed error.
    Err {
        /// Human-readable detail.
        detail: String,
    },
}

/// Errors at the ABI boundary.
#[derive(Debug, thiserror::Error)]
pub enum AbiError {
    /// Extension ID failed the shape contract.
    #[error("invalid extension id: {0}")]
    InvalidId(String),
    /// Manifest declared an ABI version the host can't run.
    #[error("abi version {got:?} not in compatible set {compat:?}")]
    IncompatibleAbi {
        /// The version the manifest declared.
        got: String,
        /// The set of versions the host supports.
        compat: &'static [&'static str],
    },
    /// Extension requested a capability the host refused to grant.
    #[error("capability denied: {0}")]
    CapabilityDenied(String),
    /// JSON serialization or schema problem.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_id_validates() {
        assert!(ExtensionId::parse("a").is_ok());
        assert!(ExtensionId::parse("my-ext").is_ok());
        assert!(ExtensionId::parse("").is_err());
        assert!(ExtensionId::parse("0ext").is_err());
        assert!(ExtensionId::parse("MY-EXT").is_err());
        assert!(ExtensionId::parse("my--ext").is_err());
        assert!(ExtensionId::parse("my-ext-").is_err());
    }

    #[test]
    fn capability_slug_round_trip() {
        let caps = [
            Capability::ReadFs { root: "x".into() },
            Capability::WriteFs { root: "x".into() },
            Capability::NetworkEgress {
                hosts: vec!["a".into()],
            },
            Capability::Timer,
            Capability::Log,
            Capability::EventEmit {
                channels: vec!["c".into()],
            },
            Capability::EnvRead {
                prefix: "X_".into(),
            },
        ];
        let mut seen = std::collections::HashSet::new();
        for c in &caps {
            assert!(seen.insert(c.slug()), "duplicate slug {}", c.slug());
        }
    }

    #[test]
    fn capability_serde_round_trip() {
        let c = Capability::NetworkEgress {
            hosts: vec!["api.example.com".into()],
        };
        let s = serde_json::to_string(&c).unwrap();
        let back: Capability = serde_json::from_str(&s).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn manifest_round_trip() {
        let m = ExtensionManifest {
            id: ExtensionId::parse("hello").unwrap(),
            version: "0.1.0".into(),
            summary: "demo".into(),
            capabilities: vec![Capability::Log, Capability::Timer],
            abi_version: SUPPORTED_ABI_VERSION.to_string(),
        };
        let s = serde_json::to_string(&m).unwrap();
        let back: ExtensionManifest = serde_json::from_str(&s).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn manifest_default_abi_version_is_compatible() {
        let m = ExtensionManifest {
            id: ExtensionId::parse("x").unwrap(),
            version: "0".into(),
            summary: "".into(),
            capabilities: vec![],
            abi_version: default_abi_version(),
        };
        assert!(m.abi_is_compatible());
    }

    #[test]
    fn manifest_rejects_unknown_field() {
        let bad = r#"{"id":"x","version":"0.0.1","summary":"","ahem":1}"#;
        let r: Result<ExtensionManifest, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    #[test]
    fn abi_compatibility_check() {
        let m = ExtensionManifest {
            id: ExtensionId::parse("x").unwrap(),
            version: "0".into(),
            summary: "".into(),
            capabilities: vec![],
            abi_version: "99".into(),
        };
        assert!(!m.abi_is_compatible());
    }

    #[test]
    fn invoke_request_serde_round_trip() {
        let r = InvokeRequest {
            extension: ExtensionId::parse("hello").unwrap(),
            function: "greet".into(),
            args: serde_json::json!({"name": "world"}),
        };
        let s = serde_json::to_string(&r).unwrap();
        let back: InvokeRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn invoke_response_status_serializes_flat() {
        let r = InvokeResponse {
            status: InvokeStatus::Ok,
            value: serde_json::json!("hello"),
            logs: vec![],
        };
        let s = serde_json::to_string(&r).unwrap();
        // status field is hoisted (flatten + internally tagged).
        assert!(s.contains("\"status\":\"ok\""));
    }

    #[test]
    fn invoke_response_err_carries_detail() {
        let r = InvokeResponse {
            status: InvokeStatus::Err {
                detail: "oops".into(),
            },
            value: serde_json::Value::Null,
            logs: vec![],
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"detail\":\"oops\""));
    }
}
