//! `provenance` — per-build signed attestation snapshot.
//!
//! Task #246 per the variation-architecture spec. Captures the
//! load-bearing inputs to a build (identity declaration +
//! computed fingerprint commitment + build timestamp) into a
//! single signable struct so an auditor can verify "this build
//! ran against THIS identity and produced THIS fingerprint" via
//! Ed25519 signature replay.
//!
//! Where the existing [`crate::attest`] module signs the build
//! REPORT (findings, chain), this module signs the build INPUTS
//! (identity, fingerprint). The two attestations pair: the
//! report says "this is what the gates found"; the provenance
//! says "this is what was being audited."
//!
//! ## Why a separate module
//!
//! BuildReport changes when phases evolve (new findings, new
//! shapes). Provenance is a much narrower wire-shape — identity
//! hash + fingerprint commitment + timestamp — and SHOULD stay
//! tiny so external auditors can replay it without depending on
//! the substrate's full report shape.
//!
//! Per `[[backward-compat-version-discipline]]`: this struct is
//! a v1 commitment. Future evolutions add fields conservatively;
//! `spec` carries the version variant.
//!
//! ## API
//!
//! * [`Provenance::compute`] — pure function: hashes the
//!   `[site_identity]` section of forge.toml + accepts a
//!   precomputed fingerprint commitment + ISO-8601 timestamp,
//!   returns an unsigned `Provenance`.
//! * [`Provenance::sign`] — Ed25519-sign the canonical bytes
//!   using the supplied signing key.
//! * [`Provenance::verify`] — verify the embedded signature
//!   against the supplied public key + canonical re-derivation.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `#[non_exhaustive]` on every public type.
//! * No unwrap/expect in non-test code.
//! * Pure functions; filesystem walking lives in forge-cli.

use std::fs;
use std::path::Path;

use ed25519_dalek::{Signer as _, SigningKey, Verifier as _, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Provenance-spec version. Bumped only when the canonical-bytes
/// derivation changes in a way that invalidates existing
/// signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProvenanceSpec {
    /// Initial spec, 2026-05-20.
    #[default]
    V1,
}

impl ProvenanceSpec {
    /// Stable kebab-case slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::V1 => "v1",
        }
    }
}

/// One signed attestation of a build's load-bearing inputs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Provenance {
    /// Schema version.
    pub spec: ProvenanceSpec,
    /// SHA-256 over the `[site_identity]` section of forge.toml.
    /// 64-char lowercase hex. Empty string when no identity was
    /// declared.
    pub identity_hash: String,
    /// SHA-256 fingerprint commitment of the site at build time.
    /// Mirrors `SiteFingerprint::commitment_hex`.
    pub fingerprint_commitment_hex: String,
    /// ISO-8601 RFC-3339 UTC timestamp.
    pub timestamp: String,
    /// Site identifier (operator-supplied; from site_identity).
    pub site_id: String,
    /// Tenant identifier.
    pub tenant_id: String,
    /// Base64 Ed25519 signature over the canonical bytes.
    /// Empty when unsigned.
    pub signature_b64: String,
}

/// Errors compute / sign / verify can raise.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProvenanceError {
    /// I/O error reading forge.toml.
    #[error("provenance I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Base64 decode error verifying a signature.
    #[error("provenance signature base64 decode failed")]
    SignatureDecode,
    /// Signature verification failed.
    #[error("provenance signature verification failed")]
    BadSignature,
    /// Spec mismatch — provenance was emitted under a spec the
    /// verifier doesn't know about.
    #[error("provenance spec mismatch: expected {expected:?}, got {actual:?}")]
    SpecMismatch {
        /// Expected spec.
        expected: ProvenanceSpec,
        /// Spec carried by the provenance.
        actual: ProvenanceSpec,
    },
}

impl Provenance {
    /// Build a Provenance from the live forge.toml + a precomputed
    /// fingerprint commitment. Pure; no signing.
    pub fn compute(
        root: &Path,
        fingerprint_commitment_hex: impl Into<String>,
        timestamp: impl Into<String>,
        site_id: impl Into<String>,
        tenant_id: impl Into<String>,
    ) -> Result<Self, ProvenanceError> {
        let identity_hash = hash_identity_section(root)?;
        Ok(Self {
            spec: ProvenanceSpec::V1,
            identity_hash,
            fingerprint_commitment_hex: fingerprint_commitment_hex.into(),
            timestamp: timestamp.into(),
            site_id: site_id.into(),
            tenant_id: tenant_id.into(),
            signature_b64: String::new(),
        })
    }

    /// Canonical byte string used for hashing/signing. Order is
    /// fixed; no map-iteration ambiguity.
    fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"provenance/");
        out.extend_from_slice(self.spec.slug().as_bytes());
        out.push(b'\n');
        out.extend_from_slice(b"identity_hash=");
        out.extend_from_slice(self.identity_hash.as_bytes());
        out.push(b'\n');
        out.extend_from_slice(b"fingerprint=");
        out.extend_from_slice(self.fingerprint_commitment_hex.as_bytes());
        out.push(b'\n');
        out.extend_from_slice(b"timestamp=");
        out.extend_from_slice(self.timestamp.as_bytes());
        out.push(b'\n');
        out.extend_from_slice(b"site=");
        out.extend_from_slice(self.site_id.as_bytes());
        out.push(b'\n');
        out.extend_from_slice(b"tenant=");
        out.extend_from_slice(self.tenant_id.as_bytes());
        out.push(b'\n');
        out
    }

    /// Sign the provenance with the supplied Ed25519 key.
    /// Updates `signature_b64` in place.
    pub fn sign(&mut self, key: &SigningKey) {
        let bytes = self.canonical_bytes();
        let sig = key.sign(&bytes);
        use base64::Engine as _;
        self.signature_b64 = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());
    }

    /// Verify the provenance signature against the supplied public
    /// key. Returns Ok(()) on success.
    pub fn verify(&self, key: &VerifyingKey) -> Result<(), ProvenanceError> {
        if self.spec != ProvenanceSpec::V1 {
            return Err(ProvenanceError::SpecMismatch {
                expected: ProvenanceSpec::V1,
                actual: self.spec,
            });
        }
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.signature_b64)
            .map_err(|_| ProvenanceError::SignatureDecode)?;
        if bytes.len() != ed25519_dalek::SIGNATURE_LENGTH {
            return Err(ProvenanceError::BadSignature);
        }
        let mut arr = [0u8; ed25519_dalek::SIGNATURE_LENGTH];
        arr.copy_from_slice(&bytes);
        let sig = ed25519_dalek::Signature::from_bytes(&arr);
        key.verify(&self.canonical_bytes(), &sig)
            .map_err(|_| ProvenanceError::BadSignature)
    }
}

/// Compute SHA-256 over the `[site_identity]` section bytes of
/// `<root>/forge.toml`. Returns empty string when no
/// `[site_identity]` table is present. The hash is taken over the
/// raw TOML bytes of just that section — operators can verify
/// equality by diffing forge.toml.
fn hash_identity_section(root: &Path) -> Result<String, ProvenanceError> {
    let path = root.join("forge.toml");
    if !path.is_file() {
        return Ok(String::new());
    }
    let body = fs::read_to_string(&path)?;
    let Some(section) = extract_section(&body, "[site_identity]") else {
        return Ok(String::new());
    };
    let mut hasher = Sha256::new();
    hasher.update(section.as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest {
        const TABLE: &[u8; 16] = b"0123456789abcdef";
        hex.push(TABLE[(b >> 4) as usize] as char);
        hex.push(TABLE[(b & 0x0f) as usize] as char);
    }
    Ok(hex)
}

/// Extract the lines belonging to a TOML section (including
/// every nested `[site_identity.xxx]` table) up to the next
/// top-level section header. Returns None if the section header
/// isn't found.
fn extract_section<'a>(body: &'a str, header: &str) -> Option<String> {
    let mut out = String::new();
    let mut inside = false;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') && !trimmed.starts_with("[[") {
            // Top-level table header.
            if trimmed.starts_with(header) {
                inside = true;
                out.push_str(line);
                out.push('\n');
                continue;
            } else if inside && !trimmed.starts_with(&format!("{}.", &header[..header.len() - 1])) {
                // New top-level section that ISN'T a sub-section
                // of the target. Stop.
                break;
            }
        } else if trimmed.starts_with("[[") {
            // Array-of-tables header.
            let inner = format!("[[{}.", &header[1..header.len() - 1]);
            if trimmed.starts_with(&inner) {
                if inside {
                    out.push_str(line);
                    out.push('\n');
                    continue;
                }
            } else if inside {
                break;
            }
        }
        if inside {
            out.push_str(line);
            out.push('\n');
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attest::generate_keypair;
    use std::env;

    fn temp_root(name: &str) -> std::path::PathBuf {
        let p = env::temp_dir().join(format!(
            "forge-provenance-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn compute_produces_empty_identity_hash_when_no_section() {
        let root = temp_root("no-section");
        std::fs::write(root.join("forge.toml"), "[other]\nfoo=1\n").unwrap();
        let p = Provenance::compute(
            &root,
            "fp_hex",
            "2026-05-20T12:00:00Z",
            "site-a",
            "tenant-1",
        )
        .unwrap();
        assert!(p.identity_hash.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn compute_hashes_identity_section_when_present() {
        let root = temp_root("present");
        std::fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]
site_id = "x"

[site_identity.voice]
tier = "editorial"

[other]
foo = 1
"#,
        )
        .unwrap();
        let p = Provenance::compute(
            &root,
            "fp_hex",
            "ts",
            "x",
            "",
        )
        .unwrap();
        assert!(!p.identity_hash.is_empty());
        assert_eq!(p.identity_hash.len(), 64);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sign_verify_round_trip() {
        let root = temp_root("roundtrip");
        std::fs::write(
            root.join("forge.toml"),
            "[site_identity]\nsite_id = \"x\"\n",
        )
        .unwrap();
        let key = generate_keypair();
        let vk = key.verifying_key();
        let mut p = Provenance::compute(&root, "fp", "ts", "x", "").unwrap();
        p.sign(&key);
        assert!(!p.signature_b64.is_empty());
        p.verify(&vk).unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn verify_fails_with_wrong_pubkey() {
        let root = temp_root("wrong-key");
        std::fs::write(
            root.join("forge.toml"),
            "[site_identity]\nsite_id = \"x\"\n",
        )
        .unwrap();
        let key = generate_keypair();
        let other = generate_keypair();
        let mut p = Provenance::compute(&root, "fp", "ts", "x", "").unwrap();
        p.sign(&key);
        let result = p.verify(&other.verifying_key());
        assert!(matches!(result, Err(ProvenanceError::BadSignature)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn verify_fails_when_canonical_bytes_tampered() {
        let root = temp_root("tampered");
        std::fs::write(
            root.join("forge.toml"),
            "[site_identity]\nsite_id = \"x\"\n",
        )
        .unwrap();
        let key = generate_keypair();
        let vk = key.verifying_key();
        let mut p = Provenance::compute(&root, "fp", "ts", "x", "").unwrap();
        p.sign(&key);
        // Tamper with fingerprint after signing.
        p.fingerprint_commitment_hex = "tampered".into();
        assert!(p.verify(&vk).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn extract_section_includes_subtables() {
        let body = r#"
[site_identity]
site_id = "x"

[site_identity.voice]
tier = "editorial"

[site_identity.mood]
primary = "editorial"

[other]
foo = 1
"#;
        let section = extract_section(body, "[site_identity]").unwrap();
        assert!(section.contains("[site_identity]"));
        assert!(section.contains("[site_identity.voice]"));
        assert!(section.contains("[site_identity.mood]"));
        assert!(!section.contains("[other]"));
    }

    #[test]
    fn extract_section_includes_array_tables() {
        let body = r#"
[site_identity]
site_id = "x"

[[site_identity.theme_variant]]
name = "light"

[[site_identity.theme_variant]]
name = "amoled"

[other]
foo = 1
"#;
        let section = extract_section(body, "[site_identity]").unwrap();
        assert!(section.contains("[[site_identity.theme_variant]]"));
        assert!(section.contains("amoled"));
        assert!(!section.contains("[other]"));
    }

    #[test]
    fn extract_section_returns_none_when_absent() {
        let body = "[other]\nfoo = 1\n";
        assert!(extract_section(body, "[site_identity]").is_none());
    }

    #[test]
    fn spec_slug_is_stable() {
        assert_eq!(ProvenanceSpec::V1.slug(), "v1");
    }
}
