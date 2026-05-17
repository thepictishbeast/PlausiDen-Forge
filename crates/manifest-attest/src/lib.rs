//! `manifest-attest` — consolidated signing layer for every
//! PlausiDen artifact.
//!
//! Per `PLATFORM_ROADMAP.md` §6 + the
//! `super_society_tech_stack` doctrine: every artifact the
//! platform produces (Forge build report, manifest snapshot,
//! deploy bundle, content snapshot, audit report) carries a
//! detached cryptographic signature the consumer can verify
//! without re-running the build.
//!
//! ### Why one signing layer
//!
//! Today there are ad-hoc Ed25519 signing flows in `forge-cli`
//! attest + the build-report chain. Consolidating them into a
//! single `manifest-attest` crate gives:
//!   * one keypair format + storage convention
//!   * one signature wire shape (base64 SHA-256-of-payload +
//!     base64 signature + key fingerprint)
//!   * one verification path consumers (admin UI, downstream
//!     CI, external auditors) target
//!   * one place to swap in post-quantum signatures (#65 lands
//!     ML-DSA hybrid through this same surface)
//!
//! ### Public surface
//!
//! - [`AttestKey`]            — Ed25519 keypair (public + secret)
//! - [`AttestPublicKey`]      — verifier-only key
//! - [`KeyFingerprint`]       — short stable identifier
//! - [`Attestation`]          — payload digest + signature +
//!                              fingerprint + algorithm slug
//! - [`AttestableKind`]       — closed enum of artifact kinds
//! - [`AttestedBundle<T>`]    — payload + attestation
//! - [`AttestError`]          — typed errors
//!
//! ### Post-quantum migration seam
//!
//! [`Attestation::algorithm`] is a string (`"ed25519"` today,
//! `"ml-dsa-65"` + `"hybrid-ed25519+ml-dsa-65"` after #65). The
//! verifier dispatches on the algorithm slug; backends are
//! pluggable via [`SignatureAlgorithm`] trait. Hybrid mode signs
//! with BOTH algorithms so a quantum-broken Ed25519 doesn't
//! invalidate the same artifact's ML-DSA signature.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Algorithm slug emitted in [`Attestation::algorithm`] when
/// signing with the classical Ed25519 backend.
pub const ALG_ED25519: &str = "ed25519";

/// Closed enum naming what an attestation covers. Lets the
/// verifier choose the correct canonical-form policy without
/// guessing from the bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttestableKind {
    /// Forge build report (typically `reports/build-*.json`).
    BuildReport,
    /// Platform manifest (manifest-core::PlatformManifest serialization).
    Manifest,
    /// Deploy bundle (a directory tree's content hash).
    DeployBundle,
    /// Content snapshot (a Page version's CRDT state per cms-collab).
    ContentSnapshot,
    /// Crawler journey result.
    JourneyResult,
    /// Audit report (per cms-pre-publish-audit or forge audit).
    AuditReport,
    /// Generic / operator-supplied artifact kind.
    Other,
}

impl AttestableKind {
    /// Stable kebab-case slug for serialization + filenames.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::BuildReport => "build-report",
            Self::Manifest => "manifest",
            Self::DeployBundle => "deploy-bundle",
            Self::ContentSnapshot => "content-snapshot",
            Self::JourneyResult => "journey-result",
            Self::AuditReport => "audit-report",
            Self::Other => "other",
        }
    }
}

/// Short stable identifier of a public key.
/// Computed as `base64url(SHA256(verifying_key_bytes))[..16]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KeyFingerprint(String);

impl KeyFingerprint {
    /// Compute from a verifying key's raw 32-byte form.
    pub fn of_verifying_key_bytes(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
        Self(b64[..16].to_string())
    }

    /// Raw view.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Verifier-only Ed25519 public key bundle (no secret).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AttestPublicKey {
    /// Algorithm slug (currently always `"ed25519"`; see
    /// [`ALG_ED25519`]).
    pub algorithm: String,
    /// Public key as base64 (32 raw bytes encoded URL-safe).
    pub public_key_b64: String,
    /// Fingerprint of the key — useful for log lines, attestation
    /// metadata, and operator-facing identification.
    pub fingerprint: KeyFingerprint,
}

impl AttestPublicKey {
    /// Reconstruct the underlying VerifyingKey for verification.
    pub fn verifying_key(&self) -> Result<VerifyingKey, AttestError> {
        if self.algorithm != ALG_ED25519 {
            return Err(AttestError::UnsupportedAlgorithm(self.algorithm.clone()));
        }
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&self.public_key_b64)
            .map_err(|e| AttestError::Base64(e.to_string()))?;
        let array: [u8; 32] = bytes.try_into().map_err(|_| {
            AttestError::InvalidKey("wrong public key length (need 32 bytes)".into())
        })?;
        VerifyingKey::from_bytes(&array)
            .map_err(|e| AttestError::InvalidKey(format!("ed25519: {e}")))
    }
}

/// Signing keypair. Holds the secret — handle with care.
pub struct AttestKey {
    signing: SigningKey,
}

impl AttestKey {
    /// Generate a fresh Ed25519 keypair from the OS RNG.
    pub fn generate() -> Self {
        let mut csprng = rand_core::OsRng;
        Self {
            signing: SigningKey::generate(&mut csprng),
        }
    }

    /// Load from a base64 secret (32 raw bytes).
    pub fn from_secret_b64(s: &str) -> Result<Self, AttestError> {
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(s)
            .map_err(|e| AttestError::Base64(e.to_string()))?;
        let array: [u8; 32] = bytes
            .try_into()
            .map_err(|_| AttestError::InvalidKey("wrong secret length (need 32 bytes)".into()))?;
        Ok(Self {
            signing: SigningKey::from_bytes(&array),
        })
    }

    /// Export the secret as base64. Treat the result as a secret.
    pub fn to_secret_b64(&self) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(self.signing.to_bytes())
    }

    /// Project to the verifier-only public-key bundle.
    pub fn public(&self) -> AttestPublicKey {
        let vk = self.signing.verifying_key();
        let pk_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(vk.to_bytes());
        AttestPublicKey {
            algorithm: ALG_ED25519.to_string(),
            public_key_b64: pk_b64,
            fingerprint: KeyFingerprint::of_verifying_key_bytes(vk.as_bytes()),
        }
    }

    /// Sign `payload`. Returns a typed [`Attestation`] capturing
    /// the kind + digest + signature + signer fingerprint.
    pub fn sign(&self, kind: AttestableKind, payload: &[u8]) -> Attestation {
        let digest = Sha256::digest(payload);
        let sig: Signature = self.signing.sign(payload);
        let pubkey = self.public();
        Attestation {
            kind,
            algorithm: ALG_ED25519.to_string(),
            payload_sha256_b64: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest),
            signature_b64: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sig.to_bytes()),
            signer_fingerprint: pubkey.fingerprint,
        }
    }
}

impl std::fmt::Debug for AttestKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Don't render the secret.
        write!(
            f,
            "AttestKey {{ fingerprint: {:?} }}",
            self.public().fingerprint
        )
    }
}

/// Self-contained attestation record. Stored alongside the
/// signed payload (typically `<artifact>.attest.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Attestation {
    /// What the attestation covers.
    pub kind: AttestableKind,
    /// Algorithm slug (`"ed25519"` today; PQ variants land in #65).
    pub algorithm: String,
    /// SHA-256 of the signed payload, base64url-no-pad.
    pub payload_sha256_b64: String,
    /// Signature bytes, base64url-no-pad.
    pub signature_b64: String,
    /// Short stable signer identifier.
    pub signer_fingerprint: KeyFingerprint,
}

impl Attestation {
    /// Verify the attestation against `payload` using `pubkey`.
    pub fn verify(&self, payload: &[u8], pubkey: &AttestPublicKey) -> Result<(), AttestError> {
        if self.algorithm != ALG_ED25519 {
            return Err(AttestError::UnsupportedAlgorithm(self.algorithm.clone()));
        }
        if pubkey.fingerprint != self.signer_fingerprint {
            return Err(AttestError::FingerprintMismatch {
                expected: self.signer_fingerprint.clone(),
                got: pubkey.fingerprint.clone(),
            });
        }
        // Verify payload digest matches.
        let expected_digest = Sha256::digest(payload);
        let claimed_digest = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&self.payload_sha256_b64)
            .map_err(|e| AttestError::Base64(e.to_string()))?;
        if expected_digest.as_slice() != claimed_digest.as_slice() {
            return Err(AttestError::DigestMismatch);
        }
        // Verify the signature.
        let vk = pubkey.verifying_key()?;
        let sig_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&self.signature_b64)
            .map_err(|e| AttestError::Base64(e.to_string()))?;
        let sig_array: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|_| AttestError::InvalidSignature("wrong signature length".into()))?;
        let sig = Signature::from_bytes(&sig_array);
        vk.verify(payload, &sig)
            .map_err(|e| AttestError::InvalidSignature(format!("ed25519: {e}")))
    }
}

/// Payload + its attestation, bundled for storage / transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AttestedBundle<T> {
    /// The payload (any serializable value).
    pub payload: T,
    /// Attestation over the canonical-form bytes of `payload`.
    pub attestation: Attestation,
}

/// Pluggable signature algorithm. Today only Ed25519; #65 adds
/// ML-DSA (FIPS 204) + a hybrid variant. The verifier dispatches
/// on [`Attestation::algorithm`].
pub trait SignatureAlgorithm {
    /// Algorithm slug (e.g. `"ed25519"`, `"ml-dsa-65"`).
    fn slug(&self) -> &'static str;
    /// Verify a signature over `payload`.
    fn verify(&self, payload: &[u8], signature: &[u8], pubkey: &[u8]) -> Result<(), AttestError>;
}

/// Errors at the attest boundary.
#[derive(Debug, thiserror::Error)]
pub enum AttestError {
    /// Algorithm slug doesn't match any backend we support.
    #[error("unsupported algorithm: {0:?}")]
    UnsupportedAlgorithm(String),
    /// Signer fingerprint in attestation didn't match the
    /// provided public key.
    #[error("signer fingerprint mismatch: expected {expected:?}, got {got:?}")]
    FingerprintMismatch {
        /// Fingerprint the attestation declared.
        expected: KeyFingerprint,
        /// Fingerprint computed from the provided key.
        got: KeyFingerprint,
    },
    /// Payload SHA-256 in attestation didn't match recomputed digest.
    #[error("payload digest mismatch — payload bytes don't match the attested digest")]
    DigestMismatch,
    /// Public key parse failure.
    #[error("invalid key: {0}")]
    InvalidKey(String),
    /// Signature byte slice was wrong length or failed verification.
    #[error("invalid signature: {0}")]
    InvalidSignature(String),
    /// Base64 decode failure.
    #[error("base64: {0}")]
    Base64(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypair_round_trips_via_secret_b64() {
        let k1 = AttestKey::generate();
        let secret = k1.to_secret_b64();
        let k2 = AttestKey::from_secret_b64(&secret).unwrap();
        assert_eq!(k1.public(), k2.public());
    }

    #[test]
    fn public_key_fingerprint_is_16_chars() {
        let k = AttestKey::generate();
        assert_eq!(k.public().fingerprint.as_str().len(), 16);
    }

    #[test]
    fn sign_then_verify_roundtrips() {
        let k = AttestKey::generate();
        let payload = b"manifest blob";
        let att = k.sign(AttestableKind::Manifest, payload);
        let pub_key = k.public();
        att.verify(payload, &pub_key).expect("verify");
    }

    #[test]
    fn verify_refuses_tampered_payload() {
        let k = AttestKey::generate();
        let att = k.sign(AttestableKind::BuildReport, b"original");
        let pub_key = k.public();
        let r = att.verify(b"tampered", &pub_key);
        assert!(matches!(r, Err(AttestError::DigestMismatch)));
    }

    #[test]
    fn verify_refuses_wrong_public_key() {
        let k = AttestKey::generate();
        let att = k.sign(AttestableKind::Manifest, b"x");
        let other = AttestKey::generate().public();
        let r = att.verify(b"x", &other);
        assert!(matches!(r, Err(AttestError::FingerprintMismatch { .. })));
    }

    #[test]
    fn verify_refuses_unknown_algorithm() {
        let k = AttestKey::generate();
        let mut att = k.sign(AttestableKind::Manifest, b"x");
        att.algorithm = "ml-dsa-65".into();
        let r = att.verify(b"x", &k.public());
        assert!(matches!(r, Err(AttestError::UnsupportedAlgorithm(_))));
    }

    #[test]
    fn attestation_serde_round_trips() {
        let k = AttestKey::generate();
        let att = k.sign(AttestableKind::JourneyResult, b"x");
        let s = serde_json::to_string(&att).unwrap();
        let back: Attestation = serde_json::from_str(&s).unwrap();
        assert_eq!(att, back);
    }

    #[test]
    fn attestation_rejects_unknown_field() {
        let bad = r#"{"kind":"manifest","algorithm":"ed25519","payload-sha256-b64":"","signature-b64":"","signer-fingerprint":"x","ahem":1}"#;
        let r: Result<Attestation, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    #[test]
    fn attestable_kind_slugs_are_distinct() {
        let kinds = [
            AttestableKind::BuildReport,
            AttestableKind::Manifest,
            AttestableKind::DeployBundle,
            AttestableKind::ContentSnapshot,
            AttestableKind::JourneyResult,
            AttestableKind::AuditReport,
            AttestableKind::Other,
        ];
        let mut seen = std::collections::HashSet::new();
        for k in kinds {
            assert!(seen.insert(k.slug()), "duplicate slug for {k:?}");
        }
    }

    #[test]
    fn bundle_round_trips_with_string_payload() {
        let k = AttestKey::generate();
        let payload = "hello".to_string();
        let bytes = serde_json::to_vec(&payload).unwrap();
        let att = k.sign(AttestableKind::ContentSnapshot, &bytes);
        let bundle = AttestedBundle {
            payload: payload.clone(),
            attestation: att,
        };
        let s = serde_json::to_string(&bundle).unwrap();
        let back: AttestedBundle<String> = serde_json::from_str(&s).unwrap();
        assert_eq!(bundle.payload, back.payload);
        let payload_bytes = serde_json::to_vec(&back.payload).unwrap();
        back.attestation
            .verify(&payload_bytes, &k.public())
            .unwrap();
    }

    #[test]
    fn debug_does_not_leak_secret() {
        let k = AttestKey::generate();
        let dbg = format!("{k:?}");
        assert!(!dbg.contains(&k.to_secret_b64()));
        assert!(dbg.contains("fingerprint"));
    }
}
