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

/// Algorithm slug for ML-DSA-65 (FIPS 204, NIST level 3) —
/// post-quantum signatures. Backend lands in a downstream crate
/// `manifest-attest-mldsa`; this crate defines the wire shape
/// + verifier dispatch path so existing consumers don't change.
pub const ALG_ML_DSA_65: &str = "ml-dsa-65";

/// Algorithm slug for the hybrid Ed25519 + ML-DSA-65 mode.
/// In hybrid mode, an artifact is signed independently by both
/// algorithms; verification requires BOTH to succeed (the
/// quantum-broken side fails closed without compromising the
/// classical-strong side).
pub const ALG_HYBRID_ED25519_ML_DSA_65: &str = "hybrid-ed25519+ml-dsa-65";

/// Algorithm slug for ML-KEM-768 (FIPS 203, NIST level 3) —
/// post-quantum key encapsulation. Used for symmetric-key
/// transport (e.g. content-encryption-key wrap in
/// [`KemEncapsulation`]). Not a signature algorithm — wire-shape
/// difference reflected in distinct typed surface ([`KemEncapsulation`]
/// vs [`Attestation`]).
pub const KEM_ML_KEM_768: &str = "ml-kem-768";

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

/// Pluggable signature algorithm. Ed25519 backend lives in this
/// crate; ML-DSA + hybrid backends are registered by downstream
/// crates that bring in the heavy PQ deps. The verifier
/// dispatches on [`Attestation::algorithm`].
pub trait SignatureAlgorithm {
    /// Algorithm slug (e.g. `"ed25519"`, `"ml-dsa-65"`).
    fn slug(&self) -> &'static str;
    /// Verify a signature over `payload`.
    fn verify(&self, payload: &[u8], signature: &[u8], pubkey: &[u8]) -> Result<(), AttestError>;
}

// ============================================================
// POST-QUANTUM TYPES (task #65)
//
// Wire-shape + dispatch only. The actual ML-DSA + ML-KEM
// implementations live in `manifest-attest-mldsa` (a downstream
// crate bringing pqcrypto deps) so consumers that don't need PQ
// don't pay the dep weight. The verifier in THIS crate routes
// on the algorithm slug + falls back to UnsupportedAlgorithm
// when the backend isn't registered.
// ============================================================

/// Hybrid attestation — two independent signatures over the same
/// payload, one classical (Ed25519) and one post-quantum
/// (ML-DSA-65). Verification requires BOTH to succeed.
///
/// Why hybrid: lets the platform ship PQ today without trusting
/// any single algorithm. A future Shor-class attack against
/// Ed25519 doesn't invalidate the ML-DSA attestation; a
/// hypothetical ML-DSA classical break doesn't invalidate the
/// Ed25519 one. Both algorithms must independently succeed for
/// the bundle to verify.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct HybridAttestation {
    /// What the attestation covers.
    pub kind: AttestableKind,
    /// Always [`ALG_HYBRID_ED25519_ML_DSA_65`].
    pub algorithm: String,
    /// SHA-256 of the signed payload, base64url-no-pad. Shared
    /// across both signatures.
    pub payload_sha256_b64: String,
    /// Classical Ed25519 signature, base64url-no-pad.
    pub ed25519_signature_b64: String,
    /// Ed25519 signer fingerprint.
    pub ed25519_signer_fingerprint: KeyFingerprint,
    /// Post-quantum ML-DSA signature, base64url-no-pad.
    pub mldsa_signature_b64: String,
    /// ML-DSA signer fingerprint.
    pub mldsa_signer_fingerprint: KeyFingerprint,
}

/// Public key for an ML-DSA backend. Backend-specific bytes are
/// opaque; the slug discriminates against future PQ algorithms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PqPublicKey {
    /// Algorithm slug ([`ALG_ML_DSA_65`] or future variant).
    pub algorithm: String,
    /// Public key bytes, base64url-no-pad. Size depends on
    /// algorithm: ML-DSA-65 is 1952 bytes.
    pub public_key_b64: String,
    /// Fingerprint of the key (base64url(SHA256(pubkey_bytes))[..16]).
    pub fingerprint: KeyFingerprint,
}

impl PqPublicKey {
    /// Construct from raw public-key bytes. Caller is responsible
    /// for ensuring the bytes match the declared algorithm.
    pub fn from_bytes(algorithm: impl Into<String>, bytes: &[u8]) -> Self {
        let pk_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        let fingerprint = KeyFingerprint::of_verifying_key_bytes(bytes);
        Self {
            algorithm: algorithm.into(),
            public_key_b64: pk_b64,
            fingerprint,
        }
    }
}

/// Hybrid public-key bundle — both algorithms' public keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct HybridPublicKey {
    /// Always [`ALG_HYBRID_ED25519_ML_DSA_65`].
    pub algorithm: String,
    /// Classical Ed25519 component.
    pub ed25519: AttestPublicKey,
    /// Post-quantum ML-DSA component.
    pub mldsa: PqPublicKey,
}

impl HybridPublicKey {
    /// Build from the two component keys. Algorithm slug set
    /// to [`ALG_HYBRID_ED25519_ML_DSA_65`].
    pub fn new(ed25519: AttestPublicKey, mldsa: PqPublicKey) -> Self {
        Self {
            algorithm: ALG_HYBRID_ED25519_ML_DSA_65.to_string(),
            ed25519,
            mldsa,
        }
    }
}

impl HybridAttestation {
    /// Verify the classical half (Ed25519) against the provided
    /// hybrid public-key bundle. Use [`Self::verify_hybrid`] to
    /// also check the post-quantum half; consumers that haven't
    /// registered an ML-DSA backend yet can call this method to
    /// at least gate on classical correctness.
    pub fn verify_ed25519_half(
        &self,
        payload: &[u8],
        pubkey: &HybridPublicKey,
    ) -> Result<(), AttestError> {
        // Reconstruct a single-algorithm Attestation for the
        // classical half + verify normally.
        let att = Attestation {
            kind: self.kind,
            algorithm: ALG_ED25519.to_string(),
            payload_sha256_b64: self.payload_sha256_b64.clone(),
            signature_b64: self.ed25519_signature_b64.clone(),
            signer_fingerprint: self.ed25519_signer_fingerprint.clone(),
        };
        att.verify(payload, &pubkey.ed25519)
    }

    /// Verify the full hybrid attestation. Requires the
    /// operator to have registered an ML-DSA verifier via
    /// `mldsa_backend`. Both classical AND post-quantum
    /// signatures must verify for this method to return Ok.
    ///
    /// Until `manifest-attest-mldsa` lands, callers can use
    /// [`Self::verify_ed25519_half`] which gates on the
    /// classical-strong signature alone.
    pub fn verify_hybrid(
        &self,
        payload: &[u8],
        pubkey: &HybridPublicKey,
        mldsa_backend: &dyn SignatureAlgorithm,
    ) -> Result<(), AttestError> {
        // Algorithm slug sanity check.
        if self.algorithm != ALG_HYBRID_ED25519_ML_DSA_65 {
            return Err(AttestError::UnsupportedAlgorithm(self.algorithm.clone()));
        }
        if mldsa_backend.slug() != pubkey.mldsa.algorithm {
            return Err(AttestError::UnsupportedAlgorithm(format!(
                "backend slug {:?} vs pubkey slug {:?}",
                mldsa_backend.slug(),
                pubkey.mldsa.algorithm
            )));
        }
        // Verify the classical half first — cheap check, fails
        // fast on tampered payload.
        self.verify_ed25519_half(payload, pubkey)?;
        // Verify the PQ half.
        if pubkey.mldsa.fingerprint != self.mldsa_signer_fingerprint {
            return Err(AttestError::FingerprintMismatch {
                expected: self.mldsa_signer_fingerprint.clone(),
                got: pubkey.mldsa.fingerprint.clone(),
            });
        }
        let sig_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&self.mldsa_signature_b64)
            .map_err(|e| AttestError::Base64(e.to_string()))?;
        let pk_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&pubkey.mldsa.public_key_b64)
            .map_err(|e| AttestError::Base64(e.to_string()))?;
        mldsa_backend.verify(payload, &sig_bytes, &pk_bytes)
    }
}

/// ML-KEM-768 (FIPS 203) encapsulation record. Used to wrap a
/// symmetric key (e.g. content-encryption key) under a recipient's
/// PQ public key without ever transmitting the symmetric key
/// directly.
///
/// Wire shape only — actual key-encapsulation math lives in the
/// `manifest-attest-mldsa` crate (or a future `manifest-attest-mlkem`).
/// This crate defines the typed contract so encapsulation +
/// transport + on-disk storage formats agree platform-wide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct KemEncapsulation {
    /// Algorithm slug ([`KEM_ML_KEM_768`]).
    pub algorithm: String,
    /// Recipient public key fingerprint.
    pub recipient_fingerprint: KeyFingerprint,
    /// Encapsulated ciphertext, base64url-no-pad.
    /// ML-KEM-768 ciphertext is 1088 bytes.
    pub ciphertext_b64: String,
    /// Optional sender identity hint (operator-facing only —
    /// not cryptographically bound).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender_hint: Option<String>,
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

    /// Mock ML-DSA backend for tests. Accepts any signature whose
    /// first 4 bytes equal the first 4 bytes of the public key
    /// (just enough to test dispatch logic without pulling pqcrypto).
    struct MockMldsa;
    impl SignatureAlgorithm for MockMldsa {
        fn slug(&self) -> &'static str {
            ALG_ML_DSA_65
        }
        fn verify(
            &self,
            _payload: &[u8],
            signature: &[u8],
            pubkey: &[u8],
        ) -> Result<(), AttestError> {
            if signature.len() >= 4 && pubkey.len() >= 4 && &signature[..4] == &pubkey[..4] {
                Ok(())
            } else {
                Err(AttestError::InvalidSignature("mock-mismatch".into()))
            }
        }
    }

    fn mk_hybrid_pubkey(ed: AttestPublicKey, mldsa_pk_bytes: &[u8]) -> HybridPublicKey {
        HybridPublicKey::new(ed, PqPublicKey::from_bytes(ALG_ML_DSA_65, mldsa_pk_bytes))
    }

    #[test]
    fn pq_pubkey_round_trips_via_bytes() {
        let bytes = vec![1u8; 64];
        let pk = PqPublicKey::from_bytes(ALG_ML_DSA_65, &bytes);
        assert_eq!(pk.algorithm, ALG_ML_DSA_65);
        assert_eq!(pk.fingerprint.as_str().len(), 16);
    }

    #[test]
    fn hybrid_pubkey_carries_both_halves() {
        let ed = AttestKey::generate().public();
        let pq_bytes = vec![7u8; 32];
        let h = mk_hybrid_pubkey(ed.clone(), &pq_bytes);
        assert_eq!(h.algorithm, ALG_HYBRID_ED25519_ML_DSA_65);
        assert_eq!(h.ed25519.fingerprint, ed.fingerprint);
    }

    #[test]
    fn hybrid_attestation_serde_round_trips() {
        let ed_key = AttestKey::generate();
        let ed_pub = ed_key.public();
        let payload = b"hybrid-payload";
        let classical = ed_key.sign(AttestableKind::BuildReport, payload);
        let pq_pk_bytes = vec![9u8; 32];
        let pq_pk = PqPublicKey::from_bytes(ALG_ML_DSA_65, &pq_pk_bytes);
        let att = HybridAttestation {
            kind: AttestableKind::BuildReport,
            algorithm: ALG_HYBRID_ED25519_ML_DSA_65.to_string(),
            payload_sha256_b64: classical.payload_sha256_b64.clone(),
            ed25519_signature_b64: classical.signature_b64.clone(),
            ed25519_signer_fingerprint: ed_pub.fingerprint.clone(),
            mldsa_signature_b64: "ZmFrZS1tbGRzYS1zaWc".into(),
            mldsa_signer_fingerprint: pq_pk.fingerprint.clone(),
        };
        let s = serde_json::to_string(&att).unwrap();
        let back: HybridAttestation = serde_json::from_str(&s).unwrap();
        assert_eq!(att, back);
    }

    #[test]
    fn hybrid_verify_ed25519_half_uses_classical_path() {
        let ed_key = AttestKey::generate();
        let payload = b"hybrid-payload";
        let classical = ed_key.sign(AttestableKind::BuildReport, payload);
        let pq_pk_bytes = vec![9u8; 32];
        let att = HybridAttestation {
            kind: AttestableKind::BuildReport,
            algorithm: ALG_HYBRID_ED25519_ML_DSA_65.to_string(),
            payload_sha256_b64: classical.payload_sha256_b64.clone(),
            ed25519_signature_b64: classical.signature_b64.clone(),
            ed25519_signer_fingerprint: ed_key.public().fingerprint.clone(),
            mldsa_signature_b64: "ZmFrZQ".into(),
            mldsa_signer_fingerprint: PqPublicKey::from_bytes(ALG_ML_DSA_65, &pq_pk_bytes)
                .fingerprint,
        };
        let hpub = mk_hybrid_pubkey(ed_key.public(), &pq_pk_bytes);
        att.verify_ed25519_half(payload, &hpub)
            .expect("classical half should verify");
    }

    #[test]
    fn hybrid_verify_full_requires_both_halves() {
        let ed_key = AttestKey::generate();
        let payload = b"hybrid-payload";
        let classical = ed_key.sign(AttestableKind::BuildReport, payload);
        // The mock PQ backend accepts when sig[..4] == pubkey[..4],
        // so build the key bytes + signature with the same prefix.
        let pq_pk_bytes: Vec<u8> = (0..32).collect();
        let mldsa_sig_bytes: Vec<u8> = (0..64).collect(); // first 4 bytes match
        let mldsa_sig_b64 =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&mldsa_sig_bytes);
        let pq_pk = PqPublicKey::from_bytes(ALG_ML_DSA_65, &pq_pk_bytes);
        let att = HybridAttestation {
            kind: AttestableKind::BuildReport,
            algorithm: ALG_HYBRID_ED25519_ML_DSA_65.to_string(),
            payload_sha256_b64: classical.payload_sha256_b64.clone(),
            ed25519_signature_b64: classical.signature_b64.clone(),
            ed25519_signer_fingerprint: ed_key.public().fingerprint.clone(),
            mldsa_signature_b64: mldsa_sig_b64,
            mldsa_signer_fingerprint: pq_pk.fingerprint.clone(),
        };
        let hpub = mk_hybrid_pubkey(ed_key.public(), &pq_pk_bytes);
        att.verify_hybrid(payload, &hpub, &MockMldsa)
            .expect("both halves should verify with mock backend");
    }

    #[test]
    fn hybrid_verify_fails_when_pq_half_rejects() {
        let ed_key = AttestKey::generate();
        let payload = b"hybrid-payload";
        let classical = ed_key.sign(AttestableKind::BuildReport, payload);
        // MockMldsa requires sig[..4] == pubkey[..4]. We deliberately
        // misalign them — pubkey starts with 0,1,2,3; signature
        // starts with 9,9,9,9.
        let pq_pk_bytes: Vec<u8> = (0..32).collect();
        let mldsa_sig_bytes: Vec<u8> = vec![9u8; 64];
        let mldsa_sig_b64 =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&mldsa_sig_bytes);
        let pq_pk = PqPublicKey::from_bytes(ALG_ML_DSA_65, &pq_pk_bytes);
        let att = HybridAttestation {
            kind: AttestableKind::BuildReport,
            algorithm: ALG_HYBRID_ED25519_ML_DSA_65.to_string(),
            payload_sha256_b64: classical.payload_sha256_b64.clone(),
            ed25519_signature_b64: classical.signature_b64.clone(),
            ed25519_signer_fingerprint: ed_key.public().fingerprint.clone(),
            mldsa_signature_b64: mldsa_sig_b64,
            mldsa_signer_fingerprint: pq_pk.fingerprint.clone(),
        };
        let hpub = mk_hybrid_pubkey(ed_key.public(), &pq_pk_bytes);
        let r = att.verify_hybrid(payload, &hpub, &MockMldsa);
        assert!(matches!(r, Err(AttestError::InvalidSignature(_))));
    }

    #[test]
    fn hybrid_verify_fails_when_classical_half_tampered() {
        let ed_key = AttestKey::generate();
        let classical = ed_key.sign(AttestableKind::BuildReport, b"original");
        let pq_pk_bytes: Vec<u8> = (0..32).collect();
        let mldsa_sig_bytes: Vec<u8> = (0..64).collect();
        let mldsa_sig_b64 =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&mldsa_sig_bytes);
        let pq_pk = PqPublicKey::from_bytes(ALG_ML_DSA_65, &pq_pk_bytes);
        let att = HybridAttestation {
            kind: AttestableKind::BuildReport,
            algorithm: ALG_HYBRID_ED25519_ML_DSA_65.to_string(),
            payload_sha256_b64: classical.payload_sha256_b64.clone(),
            ed25519_signature_b64: classical.signature_b64.clone(),
            ed25519_signer_fingerprint: ed_key.public().fingerprint.clone(),
            mldsa_signature_b64: mldsa_sig_b64,
            mldsa_signer_fingerprint: pq_pk.fingerprint.clone(),
        };
        let hpub = mk_hybrid_pubkey(ed_key.public(), &pq_pk_bytes);
        // Tampered payload — classical half should fail first.
        let r = att.verify_hybrid(b"tampered", &hpub, &MockMldsa);
        assert!(matches!(r, Err(AttestError::DigestMismatch)));
    }

    #[test]
    fn hybrid_verify_refuses_wrong_algorithm_slug() {
        let ed_key = AttestKey::generate();
        let classical = ed_key.sign(AttestableKind::BuildReport, b"x");
        let pq_pk_bytes: Vec<u8> = (0..32).collect();
        let pq_pk = PqPublicKey::from_bytes(ALG_ML_DSA_65, &pq_pk_bytes);
        let att = HybridAttestation {
            kind: AttestableKind::BuildReport,
            algorithm: "not-a-real-algorithm".into(),
            payload_sha256_b64: classical.payload_sha256_b64.clone(),
            ed25519_signature_b64: classical.signature_b64.clone(),
            ed25519_signer_fingerprint: ed_key.public().fingerprint.clone(),
            mldsa_signature_b64: "x".into(),
            mldsa_signer_fingerprint: pq_pk.fingerprint.clone(),
        };
        let hpub = mk_hybrid_pubkey(ed_key.public(), &pq_pk_bytes);
        let r = att.verify_hybrid(b"x", &hpub, &MockMldsa);
        assert!(matches!(r, Err(AttestError::UnsupportedAlgorithm(_))));
    }

    #[test]
    fn kem_encapsulation_serde_round_trips() {
        let pq_pk_bytes = vec![1u8; 64];
        let recipient = PqPublicKey::from_bytes(KEM_ML_KEM_768, &pq_pk_bytes);
        let enc = KemEncapsulation {
            algorithm: KEM_ML_KEM_768.to_string(),
            recipient_fingerprint: recipient.fingerprint.clone(),
            ciphertext_b64: "ZmFrZS1jaXBoZXJ0ZXh0".into(),
            sender_hint: Some("admin@example.com".into()),
        };
        let s = serde_json::to_string(&enc).unwrap();
        let back: KemEncapsulation = serde_json::from_str(&s).unwrap();
        assert_eq!(enc, back);
    }

    #[test]
    fn kem_encapsulation_refuses_unknown_field() {
        let bad = r#"{"algorithm":"ml-kem-768","recipient-fingerprint":"x","ciphertext-b64":"y","ahem":1}"#;
        let r: Result<KemEncapsulation, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    #[test]
    fn pq_constants_are_distinct() {
        assert_ne!(ALG_ED25519, ALG_ML_DSA_65);
        assert_ne!(ALG_ED25519, ALG_HYBRID_ED25519_ML_DSA_65);
        assert_ne!(ALG_ML_DSA_65, ALG_HYBRID_ED25519_ML_DSA_65);
        assert_ne!(KEM_ML_KEM_768, ALG_ML_DSA_65);
    }

    #[test]
    fn debug_does_not_leak_secret() {
        let k = AttestKey::generate();
        let dbg = format!("{k:?}");
        assert!(!dbg.contains(&k.to_secret_b64()));
        assert!(dbg.contains("fingerprint"));
    }
}
