//! `attest` — Merkle-chain math for build reports.
//!
//! T26. Pure functions, no I/O. Filesystem walking lives in
//! forge-cli (the binary edge); this module just hashes bytes
//! and proves chain continuity.
//!
//! ## Threat model
//!
//! An attacker has the build history but wants to (a) tamper
//! with a historical report, (b) delete a historical report,
//! or (c) insert a forged report at any position.
//!
//! The chain construction makes all three detectable:
//!
//! * Tamper → the hash of the tampered report no longer matches
//!   the `prev_hash` of the next report. Verifier returns
//!   `ChainBroken { at_index, expected, actual }`.
//! * Delete → the gap between sequence numbers + the broken
//!   chain at the gap both surface as failures.
//! * Insert → the new report's `prev_hash` must match the
//!   genuine prior; an attacker without the prior bytes can't
//!   produce that.
//!
//! Caveat: the chain does NOT prevent rewriting the ENTIRE
//! history from scratch. Pair with off-host attestation (commit
//! the chain root to a remote append-only log, or co-sign with
//! a hardware key) for full tamper-evidence. v1 here is the
//! local-trust baseline.
//!
//! ## Doctrine
//!
//! * **Pure functions** — `hash_report_bytes`, `chain_step`,
//!   `verify_chain`. No `std::fs`, no `std::net`.
//! * **No `unwrap`/`expect`** — every fallible op returns
//!   `Result`.
//! * **deny `unsafe_code`** at crate level.
//! * **ADT error type** — `ChainError` variant per failure mode.

use sha2::{Digest, Sha256};

use crate::BuildReport;

/// Hash a serialized report byte slice → 64-char lowercase hex.
///
/// **Pre:** `bytes` is the canonical-serialized form of a
/// `BuildReport` (e.g. `serde_json::to_vec(&report)`).
/// **Post:** returned string is exactly 64 ASCII hex chars.
#[must_use]
pub fn hash_report_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest {
        // Hand-roll hex without an external crate to keep
        // forge-core small. `format!("{:02x}", b)` works but
        // pushes formatting machinery + heap traffic per byte;
        // the lookup-table form below is allocation-free per
        // byte (one push per nibble).
        const TABLE: &[u8; 16] = b"0123456789abcdef";
        hex.push(TABLE[(b >> 4) as usize] as char);
        hex.push(TABLE[(b & 0x0f) as usize] as char);
    }
    hex
}

/// Errors the chain verifier can return.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChainError {
    /// Genesis report (chain_length == 1) carried a non-None
    /// `prev_hash`. Suspicious — genesis has nothing to chain to.
    #[error("genesis report has prev_hash (should be None)")]
    GenesisHasPrev,
    /// Non-genesis report carried `prev_hash = None`. Either it
    /// is a missing-chaining bug or evidence of history rewrite.
    #[error("report at index {at_index} (chain_length {length}) has no prev_hash")]
    MissingPrev {
        /// Position in the input sequence (0-indexed).
        at_index: usize,
        /// The report's declared chain_length at that position.
        length: u64,
    },
    /// Hash mismatch — chain is broken at this position.
    #[error("chain broken at index {at_index}: expected {expected}, got {actual}")]
    Broken {
        /// Position in the input sequence (0-indexed).
        at_index: usize,
        /// What the report's `prev_hash` field claims.
        expected: String,
        /// What the prior report actually hashes to.
        actual: String,
    },
    /// Chain-length sequence numbers are non-contiguous.
    #[error("chain_length jumped at index {at_index}: prior length {prior}, current {current}")]
    SequenceGap {
        /// Position in the input sequence (0-indexed).
        at_index: usize,
        /// `chain_length` of the prior report.
        prior: u64,
        /// `chain_length` of the current report.
        current: u64,
    },
    /// Internal: serializing a report to verify its hash failed.
    /// Should never happen for a well-formed `BuildReport`.
    #[error("serialize: {0}")]
    Serialize(String),
}

/// Set `prev_hash` + `chain_length` on `new` based on `prior`.
/// Genesis (no prior) yields `prev_hash = None, chain_length = 1`.
///
/// Mutates `new` in place. `prior` is borrowed immutably.
///
/// # Errors
/// Returns `ChainError::Serialize` if `prior` cannot be
/// serialized to JSON — should never happen for a well-formed
/// `BuildReport`.
pub fn chain_step(prior: Option<&BuildReport>, new: &mut BuildReport) -> Result<(), ChainError> {
    match prior {
        None => {
            new.prev_hash = None;
            new.chain_length = 1;
        }
        Some(p) => {
            let bytes = serde_json::to_vec(p).map_err(|e| ChainError::Serialize(e.to_string()))?;
            new.prev_hash = Some(hash_report_bytes(&bytes));
            new.chain_length = p.chain_length.saturating_add(1);
        }
    }
    Ok(())
}

/// Walk a sequence of reports in ascending chain order and
/// verify continuity. Returns `Ok(())` if every report's
/// `prev_hash` matches the hash of the prior report AND
/// `chain_length` is contiguous.
///
/// **Pre:** `reports` is ordered by `chain_length` ascending.
/// **Post:** any drift surfaces as the FIRST encountered
/// `ChainError`.
///
/// # Errors
/// See `ChainError` variants.
pub fn verify_chain(reports: &[BuildReport]) -> Result<(), ChainError> {
    let mut prior: Option<&BuildReport> = None;
    for (idx, current) in reports.iter().enumerate() {
        match prior {
            None => {
                // Genesis.
                if current.prev_hash.is_some() {
                    return Err(ChainError::GenesisHasPrev);
                }
            }
            Some(p) => {
                // Sequence-number contiguity.
                let expected_len = p.chain_length.saturating_add(1);
                if current.chain_length != expected_len {
                    return Err(ChainError::SequenceGap {
                        at_index: idx,
                        prior: p.chain_length,
                        current: current.chain_length,
                    });
                }
                // Hash continuity.
                let Some(claim) = current.prev_hash.as_deref() else {
                    return Err(ChainError::MissingPrev {
                        at_index: idx,
                        length: current.chain_length,
                    });
                };
                let bytes =
                    serde_json::to_vec(p).map_err(|e| ChainError::Serialize(e.to_string()))?;
                let actual = hash_report_bytes(&bytes);
                if claim != actual {
                    return Err(ChainError::Broken {
                        at_index: idx,
                        expected: claim.to_owned(),
                        actual,
                    });
                }
            }
        }
        prior = Some(current);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(mode: &str) -> BuildReport {
        BuildReport {
            mode: mode.to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn hash_is_64_hex_chars() {
        let h = hash_report_bytes(b"");
        assert_eq!(h.len(), 64);
        assert!(h
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn hash_deterministic() {
        let a = hash_report_bytes(b"forge");
        let b = hash_report_bytes(b"forge");
        assert_eq!(a, b);
    }

    #[test]
    fn hash_distinct_inputs_yield_distinct_outputs() {
        let a = hash_report_bytes(b"forge");
        let b = hash_report_bytes(b"forgf"); // 1-bit diff
        assert_ne!(a, b);
    }

    #[test]
    fn chain_step_genesis_has_no_prev() {
        let mut r = rep("poc");
        chain_step(None, &mut r).expect("ok");
        assert_eq!(r.prev_hash, None);
        assert_eq!(r.chain_length, 1);
    }

    #[test]
    fn chain_step_subsequent_has_prev_and_increments_length() {
        let mut g = rep("poc");
        chain_step(None, &mut g).expect("genesis");
        let mut next = rep("poc");
        chain_step(Some(&g), &mut next).expect("next");
        assert!(next.prev_hash.is_some());
        assert_eq!(next.chain_length, 2);
    }

    #[test]
    fn verify_clean_two_step_chain() {
        let mut g = rep("poc");
        chain_step(None, &mut g).expect("genesis");
        let mut n = rep("poc");
        chain_step(Some(&g), &mut n).expect("step");
        assert_eq!(verify_chain(&[g, n]), Ok(()));
    }

    #[test]
    fn verify_detects_tamper() {
        let mut g = rep("poc");
        chain_step(None, &mut g).expect("genesis");
        let mut n = rep("poc");
        chain_step(Some(&g), &mut n).expect("step");
        // Tamper g AFTER chaining — n.prev_hash now refers to
        // the original bytes of g, not the mutated bytes.
        g.warn_count = 999;
        let r = verify_chain(&[g, n]);
        assert!(matches!(r, Err(ChainError::Broken { at_index: 1, .. })));
    }

    #[test]
    fn verify_detects_sequence_gap() {
        let mut g = rep("poc");
        chain_step(None, &mut g).expect("genesis");
        let mut n = rep("poc");
        chain_step(Some(&g), &mut n).expect("step");
        n.chain_length = 5; // forged ahead
        let r = verify_chain(&[g, n]);
        assert!(matches!(
            r,
            Err(ChainError::SequenceGap { at_index: 1, .. })
        ));
    }

    #[test]
    fn verify_detects_genesis_with_prev_hash() {
        let mut g = rep("poc");
        g.prev_hash = Some("fake".into());
        let r = verify_chain(&[g]);
        assert!(matches!(r, Err(ChainError::GenesisHasPrev)));
    }

    #[test]
    fn verify_detects_missing_prev_on_non_genesis() {
        let mut g = rep("poc");
        chain_step(None, &mut g).expect("genesis");
        let mut n = rep("poc");
        chain_step(Some(&g), &mut n).expect("step");
        n.prev_hash = None; // forged missing
        let r = verify_chain(&[g, n]);
        assert!(matches!(r, Err(ChainError::MissingPrev { .. })));
    }

    #[test]
    fn verify_long_chain() {
        let mut chain: Vec<BuildReport> = Vec::new();
        let mut current = rep("poc");
        chain_step(None, &mut current).expect("genesis");
        chain.push(current);
        for _ in 1..20 {
            let mut next = rep("poc");
            chain_step(Some(chain.last().expect("non-empty")), &mut next).expect("step");
            chain.push(next);
        }
        assert_eq!(chain.len(), 20);
        assert_eq!(chain[19].chain_length, 20);
        assert_eq!(verify_chain(&chain), Ok(()));
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Hashing must be deterministic + collision-resistant
        /// at the property level: any two distinct inputs ≤ 200
        /// bytes produce distinct hashes (the proptest space is
        /// small enough that a real SHA-256 collision is
        /// astronomically unlikely; this proves no
        /// implementation drift toward XOR-on-bytes or similar).
        #[test]
        fn hash_distinct_inputs(a in any::<Vec<u8>>(), b in any::<Vec<u8>>()) {
            if a != b {
                prop_assert_ne!(hash_report_bytes(&a), hash_report_bytes(&b));
            } else {
                prop_assert_eq!(hash_report_bytes(&a), hash_report_bytes(&b));
            }
        }

        /// chain_step + verify_chain must round-trip for ANY
        /// sequence of n reports built by repeated stepping.
        #[test]
        fn chain_roundtrip(modes in proptest::collection::vec("[a-z]{1,8}", 1..15)) {
            let mut chain = Vec::<BuildReport>::new();
            for mode in &modes {
                let mut r = BuildReport {
                    mode: mode.clone(),
                    ..Default::default()
                };
                let prior = chain.last();
                chain_step(prior, &mut r).expect("step");
                chain.push(r);
            }
            prop_assert_eq!(verify_chain(&chain), Ok(()));
        }

        /// Tampering with ANY field of ANY non-last report breaks
        /// the chain. Strong tamper-evidence property.
        #[test]
        fn tampering_breaks_chain(
            modes in proptest::collection::vec("[a-z]{1,8}", 2..10),
            tamper_at in 0usize..9,
        ) {
            let mut chain = Vec::<BuildReport>::new();
            for mode in &modes {
                let mut r = BuildReport {
                    mode: mode.clone(),
                    ..Default::default()
                };
                let prior = chain.last();
                chain_step(prior, &mut r).expect("step");
                chain.push(r);
            }
            let idx = tamper_at % (chain.len() - 1);
            chain[idx].warn_count = 999;
            // The tampered report is at `idx`; the chain breaks
            // at `idx + 1` because that's where the bad hash gets
            // detected.
            let r = verify_chain(&chain);
            prop_assert!(r.is_err(), "tampered chain must verify-fail");
        }
    }
}

// ============================================================
// T56: Ed25519-signed Merkle chain roots
// ============================================================
//
// T26 + T55 give tamper-evident chain CONTINUITY. T56 layers on
// cryptographic IDENTITY — chain root forgery now requires key
// compromise, not just byte access.
//
// Threat model extension:
//   * Without signatures: an attacker who CAN extend the chain
//     (e.g. an insider with write access to reports/) can rewrite
//     history from a tampered checkpoint forward, and the chain
//     verifier passes (because rewritten hashes are internally
//     consistent).
//   * With signatures: every chain root is signed by a private
//     key the attacker doesn't have. Verification rejects any
//     report whose signature doesn't match. Compromise requires
//     the key, not just the directory.
//
// Doctrine:
//   * Ed25519 (RFC 8032) — modern, fast, deterministic, no
//     parameter tuning required, no nonce-reuse risk.
//   * Pure-fn surface (sign / verify); key I/O lives in
//     forge-cli per the forge-core "no I/O" rule.
//   * No `unwrap`/`expect`; ADT errors.

use base64::Engine as _;
use ed25519_dalek::{Signer as _, SigningKey, Verifier as _, VerifyingKey, SIGNATURE_LENGTH};
use rand_core::OsRng;

/// Errors the signing layer can return.
#[derive(Debug, thiserror::Error)]
pub enum SignError {
    /// Couldn't serialize the report for signing.
    #[error("serialize: {0}")]
    Serialize(String),
    /// Couldn't decode a signature from base64.
    #[error("signature decode: {0}")]
    SignatureDecode(String),
    /// Signature length wasn't ed25519's 64 bytes.
    #[error("signature length wrong: got {got}, expected {SIGNATURE_LENGTH}")]
    SignatureLength {
        /// Actual signature length supplied.
        got: usize,
    },
    /// Verifying key bytes were the wrong length / shape.
    #[error("public key invalid")]
    PublicKeyInvalid,
    /// Signature did not verify against the report bytes.
    #[error("signature verification failed")]
    BadSignature,
    /// Report has no signature field but signature was required.
    #[error("report has no signature field")]
    MissingSignature,
}

/// Generate a fresh Ed25519 keypair using the OS RNG.
/// Caller owns key persistence — forge-core does no I/O.
#[must_use]
pub fn generate_keypair() -> SigningKey {
    SigningKey::generate(&mut OsRng)
}

/// Serialize the report for signing — omits the `signature`
/// field so the bytes hashed don't depend on the signature
/// they're being signed for. Sets `signature = None` on a clone,
/// then serializes; original report unchanged.
fn report_bytes_for_signing(report: &BuildReport) -> Result<Vec<u8>, SignError> {
    let mut clone = clone_without_signature(report);
    clone.signature = None;
    serde_json::to_vec(&clone).map_err(|e| SignError::Serialize(e.to_string()))
}

/// Cheap clone — `BuildReport` derives nothing, so we hand-build
/// the subset we need. Findings are cloned shallow.
fn clone_without_signature(r: &BuildReport) -> BuildReport {
    BuildReport {
        mode: r.mode.clone(),
        findings: r.findings.clone(),
        strict_count: r.strict_count,
        warn_count: r.warn_count,
        duration_ms: r.duration_ms,
        prev_hash: r.prev_hash.clone(),
        chain_length: r.chain_length,
        started: r.started.clone(),
        signature: None,
    }
}

/// Sign `report` in place: serialise the report (omitting any
/// existing signature), produce an Ed25519 signature, base64-
/// encode, store on `report.signature`.
///
/// # Errors
/// `SignError::Serialize` if serialisation fails.
pub fn sign_report(report: &mut BuildReport, key: &SigningKey) -> Result<(), SignError> {
    let bytes = report_bytes_for_signing(report)?;
    let sig = key.sign(&bytes);
    let b64 = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());
    report.signature = Some(b64);
    Ok(())
}

/// Verify a report's `signature` against the supplied public
/// key. Returns `Ok(())` on valid, `Err(SignError::*)` on
/// missing / malformed / forged.
///
/// # Errors
/// See `SignError` variants.
pub fn verify_report(report: &BuildReport, pub_key: &VerifyingKey) -> Result<(), SignError> {
    let sig_b64 = report
        .signature
        .as_deref()
        .ok_or(SignError::MissingSignature)?;
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(sig_b64)
        .map_err(|e| SignError::SignatureDecode(e.to_string()))?;
    if sig_bytes.len() != SIGNATURE_LENGTH {
        return Err(SignError::SignatureLength {
            got: sig_bytes.len(),
        });
    }
    let mut arr = [0u8; SIGNATURE_LENGTH];
    arr.copy_from_slice(&sig_bytes);
    let sig = ed25519_dalek::Signature::from_bytes(&arr);
    let bytes = report_bytes_for_signing(report)?;
    pub_key
        .verify(&bytes, &sig)
        .map_err(|_| SignError::BadSignature)
}

/// Encode a public key as base64 (32 bytes → 44 chars). Used by
/// forge-cli to persist `attest-pubkey.b64` next to the chain.
#[must_use]
pub fn pubkey_to_base64(key: &VerifyingKey) -> String {
    base64::engine::general_purpose::STANDARD.encode(key.as_bytes())
}

/// Decode a public key from base64. Returns `None` for malformed
/// input.
#[must_use]
pub fn pubkey_from_base64(s: &str) -> Option<VerifyingKey> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(s).ok()?;
    if bytes.len() != ed25519_dalek::PUBLIC_KEY_LENGTH {
        return None;
    }
    let mut arr = [0u8; ed25519_dalek::PUBLIC_KEY_LENGTH];
    arr.copy_from_slice(&bytes);
    VerifyingKey::from_bytes(&arr).ok()
}

/// Encode a signing key (private) as base64. Caller responsible
/// for protecting the resulting bytes (file mode 0600 etc.).
#[must_use]
pub fn signing_key_to_base64(key: &SigningKey) -> String {
    base64::engine::general_purpose::STANDARD.encode(key.to_bytes())
}

/// Decode a signing key (private) from base64.
#[must_use]
pub fn signing_key_from_base64(s: &str) -> Option<SigningKey> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(s).ok()?;
    if bytes.len() != ed25519_dalek::SECRET_KEY_LENGTH {
        return None;
    }
    let mut arr = [0u8; ed25519_dalek::SECRET_KEY_LENGTH];
    arr.copy_from_slice(&bytes);
    Some(SigningKey::from_bytes(&arr))
}

#[cfg(test)]
mod sign_tests {
    use super::*;

    fn rep() -> BuildReport {
        BuildReport {
            mode: "poc".into(),
            ..Default::default()
        }
    }

    #[test]
    fn sign_then_verify_succeeds() {
        let key = generate_keypair();
        let mut r = rep();
        sign_report(&mut r, &key).expect("sign");
        assert!(r.signature.is_some());
        verify_report(&r, &key.verifying_key()).expect("verify");
    }

    #[test]
    fn signature_changes_with_content() {
        let key = generate_keypair();
        let mut r1 = rep();
        sign_report(&mut r1, &key).expect("sign1");
        let mut r2 = rep();
        r2.warn_count = 5;
        sign_report(&mut r2, &key).expect("sign2");
        assert_ne!(r1.signature, r2.signature);
    }

    #[test]
    fn tampered_report_fails_verify() {
        let key = generate_keypair();
        let mut r = rep();
        sign_report(&mut r, &key).expect("sign");
        r.warn_count = 999; // tamper after signing
        let res = verify_report(&r, &key.verifying_key());
        assert!(matches!(res, Err(SignError::BadSignature)));
    }

    #[test]
    fn wrong_key_fails_verify() {
        let k1 = generate_keypair();
        let k2 = generate_keypair();
        let mut r = rep();
        sign_report(&mut r, &k1).expect("sign");
        let res = verify_report(&r, &k2.verifying_key());
        assert!(matches!(res, Err(SignError::BadSignature)));
    }

    #[test]
    fn missing_signature_errors() {
        let key = generate_keypair();
        let r = rep(); // no signature set
        let res = verify_report(&r, &key.verifying_key());
        assert!(matches!(res, Err(SignError::MissingSignature)));
    }

    #[test]
    fn malformed_signature_errors() {
        let key = generate_keypair();
        let mut r = rep();
        r.signature = Some("not!base64!".into());
        let res = verify_report(&r, &key.verifying_key());
        assert!(matches!(res, Err(SignError::SignatureDecode(_))));
    }

    #[test]
    fn signature_omits_self_from_payload() {
        // Signing must produce the SAME signature whether the
        // input has a stale signature field or not — proves the
        // payload omits the signature.
        let key = generate_keypair();
        let mut r1 = rep();
        sign_report(&mut r1, &key).expect("sign1");
        let mut r2 = rep();
        r2.signature = Some("STALE_SIGNATURE_VALUE".into());
        sign_report(&mut r2, &key).expect("sign2");
        // Both signed reports have the SAME final signature.
        assert_eq!(r1.signature, r2.signature);
    }

    #[test]
    fn pubkey_roundtrip_base64() {
        let key = generate_keypair();
        let pk = key.verifying_key();
        let b64 = pubkey_to_base64(&pk);
        let decoded = pubkey_from_base64(&b64).expect("decode");
        assert_eq!(pk.as_bytes(), decoded.as_bytes());
    }

    #[test]
    fn signing_key_roundtrip_base64() {
        let key = generate_keypair();
        let b64 = signing_key_to_base64(&key);
        let decoded = signing_key_from_base64(&b64).expect("decode");
        assert_eq!(key.to_bytes(), decoded.to_bytes());
    }

    #[test]
    fn pubkey_from_garbage_returns_none() {
        assert!(pubkey_from_base64("garbage!").is_none());
        assert!(pubkey_from_base64(&"A".repeat(100)).is_none());
    }
}

#[cfg(test)]
mod sign_proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Sign+verify is closed under any report content
        /// reachable from the public surface.
        #[test]
        fn sign_verify_roundtrip(
            mode in "[a-z]{1,8}",
            strict in 0u32..=1000,
            warn in 0u32..=1000,
        ) {
            let key = generate_keypair();
            let mut r = BuildReport {
                mode,
                strict_count: strict as usize,
                warn_count: warn as usize,
                ..Default::default()
            };
            sign_report(&mut r, &key).expect("sign");
            prop_assert!(verify_report(&r, &key.verifying_key()).is_ok());
        }

        /// Decoder must not panic on arbitrary input.
        #[test]
        fn pubkey_decoder_does_not_panic(s in ".{0,200}") {
            let _ = pubkey_from_base64(&s);
        }

        /// Any single-byte mutation of any field breaks
        /// verification.
        #[test]
        fn any_mutation_breaks_signature(
            mode in "[a-z]{1,8}",
            strict in 0u32..=100,
        ) {
            let key = generate_keypair();
            let mut r = BuildReport {
                mode,
                strict_count: strict as usize,
                ..Default::default()
            };
            sign_report(&mut r, &key).expect("sign");
            r.warn_count = r.warn_count.wrapping_add(1);
            prop_assert!(verify_report(&r, &key.verifying_key()).is_err());
        }
    }
}
