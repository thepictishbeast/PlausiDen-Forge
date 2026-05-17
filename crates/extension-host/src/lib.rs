//! `extension-host` — trait every PlausiDen extension-host
//! backend implements, plus a `NullHost` reference impl for
//! tests + CI.
//!
//! Per `PLATFORM_ROADMAP.md` §6: the Wasm Component Model
//! backend (wasmtime) is the production target, but loading
//! wasmtime is a 200-transitive-dep commitment. This crate
//! defines the host trait first so every consumer (Forge phase
//! loader, CMS plugin slot, Loom theme extension slot)
//! integrates against a stable interface even before the
//! wasmtime backend is in tree.
//!
//! Today shipping:
//!   * [`ExtensionHost`] — the trait
//!   * [`NullHost`]      — returns deterministic stubs; useful
//!                         in tests + as the "extensions
//!                         disabled" default
//!   * [`HostError`]     — typed errors at every boundary
//!
//! Planned:
//!   * `extension-host-wasmtime` — wasmtime Component Model
//!     backend living in its own crate so the heavy dep doesn't
//!     pollute consumers that don't need it.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use extension_abi::{
    AbiError, Capability, ExtensionId, ExtensionManifest, InvokeRequest, InvokeResponse,
    InvokeStatus,
};

/// Errors a host backend can return.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    /// Underlying ABI-level failure (manifest, invoke shape, etc.)
    #[error("abi: {0}")]
    Abi(#[from] AbiError),
    /// Extension was not loaded under this host.
    #[error("extension not loaded: {0}")]
    NotLoaded(ExtensionId),
    /// Backend refused to load the manifest (capability denied,
    /// version mismatch, etc.).
    #[error("load refused: {0}")]
    LoadRefused(String),
    /// Backend-specific transport / runtime failure.
    #[error("runtime: {0}")]
    Runtime(String),
}

/// Trait every extension-host backend implements.
///
/// Lifecycle:
///   1. `load(&manifest, &wasm_bytes, &granted_capabilities)`
///      — host validates ABI version, intersects requested
///        capabilities with granted, returns loaded handle.
///   2. `invoke(req)` — host dispatches to the right extension.
///   3. `unload(&id)` — host releases the extension's resources.
///
/// Async-runtime-agnostic for the same reason `deploy-core` is:
/// different backends want different runtimes.
pub trait ExtensionHost {
    /// Stable identifier of this backend (e.g. `"wasmtime"`,
    /// `"null"`).
    fn id(&self) -> &'static str;

    /// Load an extension with the given manifest, module bytes,
    /// and operator-granted capability set.
    ///
    /// `wasm_bytes` may be ignored by backends that don't actually
    /// execute Wasm (e.g. [`NullHost`]).
    fn load(
        &mut self,
        manifest: &ExtensionManifest,
        wasm_bytes: &[u8],
        granted: &[Capability],
    ) -> Result<(), HostError>;

    /// Whether the given extension is currently loaded.
    fn is_loaded(&self, id: &ExtensionId) -> bool;

    /// Invoke a function on a loaded extension.
    fn invoke(&mut self, req: &InvokeRequest) -> Result<InvokeResponse, HostError>;

    /// Release the extension's resources.
    fn unload(&mut self, id: &ExtensionId) -> Result<(), HostError>;
}

/// Reference implementation that never actually executes Wasm.
/// Returns deterministic stub responses — useful in tests + as
/// the "extensions disabled" default in environments where the
/// real backend isn't desired (CI, sandboxed shells, etc.).
#[derive(Debug, Default)]
pub struct NullHost {
    loaded: Vec<LoadedNull>,
}

#[derive(Debug)]
struct LoadedNull {
    id: ExtensionId,
    granted: Vec<Capability>,
}

impl ExtensionHost for NullHost {
    fn id(&self) -> &'static str {
        "null"
    }

    fn load(
        &mut self,
        manifest: &ExtensionManifest,
        _wasm_bytes: &[u8],
        granted: &[Capability],
    ) -> Result<(), HostError> {
        if !manifest.abi_is_compatible() {
            return Err(HostError::Abi(AbiError::IncompatibleAbi {
                got: manifest.abi_version.clone(),
                compat: ExtensionManifest::COMPATIBLE_ABI_VERSIONS,
            }));
        }
        // Reject capabilities the extension requested that weren't
        // granted by the operator.
        for req in &manifest.capabilities {
            if !granted.iter().any(|g| same_capability_variant(g, req)) {
                return Err(HostError::Abi(AbiError::CapabilityDenied(
                    req.slug().to_string(),
                )));
            }
        }
        if self.loaded.iter().any(|l| l.id == manifest.id) {
            return Err(HostError::LoadRefused(format!(
                "{} already loaded",
                manifest.id
            )));
        }
        self.loaded.push(LoadedNull {
            id: manifest.id.clone(),
            granted: granted.to_vec(),
        });
        Ok(())
    }

    fn is_loaded(&self, id: &ExtensionId) -> bool {
        self.loaded.iter().any(|l| &l.id == id)
    }

    fn invoke(&mut self, req: &InvokeRequest) -> Result<InvokeResponse, HostError> {
        if !self.is_loaded(&req.extension) {
            return Err(HostError::NotLoaded(req.extension.clone()));
        }
        // NullHost echoes the args under a typed Ok status. Real
        // backends route through the loaded module.
        Ok(InvokeResponse {
            status: InvokeStatus::Ok,
            value: serde_json::json!({
                "null_host_echo": req.args.clone(),
                "function": req.function.clone(),
            }),
            logs: vec![format!(
                "null-host invoke ext={} fn={}",
                req.extension, req.function
            )],
        })
    }

    fn unload(&mut self, id: &ExtensionId) -> Result<(), HostError> {
        let before = self.loaded.len();
        self.loaded.retain(|l| &l.id != id);
        if self.loaded.len() == before {
            return Err(HostError::NotLoaded(id.clone()));
        }
        Ok(())
    }
}

impl NullHost {
    /// Return the granted-capability set for a loaded extension
    /// (useful for inspector UIs + tests).
    pub fn granted(&self, id: &ExtensionId) -> Option<&[Capability]> {
        self.loaded
            .iter()
            .find(|l| &l.id == id)
            .map(|l| l.granted.as_slice())
    }
}

/// Whether two `Capability`s are the same variant (ignoring
/// fields). Granted capabilities must match the *variant* the
/// extension requested; the fields' allowlists / roots are
/// enforced by the backend's per-call checks (not modeled in
/// NullHost).
fn same_capability_variant(a: &Capability, b: &Capability) -> bool {
    a.slug() == b.slug()
}

#[cfg(test)]
mod tests {
    use super::*;
    use extension_abi::SUPPORTED_ABI_VERSION;

    fn manifest(id: &str, caps: Vec<Capability>) -> ExtensionManifest {
        ExtensionManifest {
            id: ExtensionId::parse(id).unwrap(),
            version: "0.1.0".into(),
            summary: "x".into(),
            capabilities: caps,
            abi_version: SUPPORTED_ABI_VERSION.to_string(),
        }
    }

    #[test]
    fn null_host_id_is_stable() {
        let h = NullHost::default();
        assert_eq!(h.id(), "null");
    }

    #[test]
    fn null_host_loads_and_invokes() {
        let mut h = NullHost::default();
        let m = manifest("hello", vec![Capability::Log]);
        h.load(&m, &[], &[Capability::Log]).unwrap();
        assert!(h.is_loaded(&m.id));
        let resp = h
            .invoke(&InvokeRequest {
                extension: m.id.clone(),
                function: "greet".into(),
                args: serde_json::json!({"name": "world"}),
            })
            .unwrap();
        assert!(matches!(resp.status, InvokeStatus::Ok));
        assert_eq!(resp.logs.len(), 1);
    }

    #[test]
    fn null_host_refuses_invoke_for_unloaded() {
        let mut h = NullHost::default();
        let r = h.invoke(&InvokeRequest {
            extension: ExtensionId::parse("nope").unwrap(),
            function: "f".into(),
            args: serde_json::Value::Null,
        });
        assert!(matches!(r, Err(HostError::NotLoaded(_))));
    }

    #[test]
    fn null_host_refuses_incompatible_abi() {
        let mut h = NullHost::default();
        let mut m = manifest("hello", vec![]);
        m.abi_version = "99".into();
        let r = h.load(&m, &[], &[]);
        assert!(matches!(
            r,
            Err(HostError::Abi(AbiError::IncompatibleAbi { .. }))
        ));
    }

    #[test]
    fn null_host_refuses_ungranted_capability() {
        let mut h = NullHost::default();
        // Extension wants NetworkEgress, operator only granted Log.
        let m = manifest(
            "needy",
            vec![Capability::NetworkEgress {
                hosts: vec!["api".into()],
            }],
        );
        let r = h.load(&m, &[], &[Capability::Log]);
        assert!(matches!(
            r,
            Err(HostError::Abi(AbiError::CapabilityDenied(_)))
        ));
    }

    #[test]
    fn null_host_refuses_duplicate_load() {
        let mut h = NullHost::default();
        let m = manifest("hello", vec![]);
        h.load(&m, &[], &[]).unwrap();
        let r = h.load(&m, &[], &[]);
        assert!(matches!(r, Err(HostError::LoadRefused(_))));
    }

    #[test]
    fn null_host_unload_releases() {
        let mut h = NullHost::default();
        let m = manifest("hello", vec![]);
        h.load(&m, &[], &[]).unwrap();
        h.unload(&m.id).unwrap();
        assert!(!h.is_loaded(&m.id));
    }

    #[test]
    fn null_host_unload_unloaded_is_error() {
        let mut h = NullHost::default();
        let id = ExtensionId::parse("nope").unwrap();
        let r = h.unload(&id);
        assert!(matches!(r, Err(HostError::NotLoaded(_))));
    }

    #[test]
    fn null_host_granted_reports_capabilities() {
        let mut h = NullHost::default();
        let m = manifest("hello", vec![Capability::Log, Capability::Timer]);
        h.load(&m, &[], &[Capability::Log, Capability::Timer])
            .unwrap();
        let g = h.granted(&m.id).unwrap();
        assert_eq!(g.len(), 2);
    }

    #[test]
    fn same_capability_variant_matches_by_slug() {
        assert!(same_capability_variant(
            &Capability::ReadFs { root: "a".into() },
            &Capability::ReadFs { root: "b".into() },
        ));
        assert!(!same_capability_variant(
            &Capability::ReadFs { root: "a".into() },
            &Capability::WriteFs { root: "a".into() },
        ));
    }
}
