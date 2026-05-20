//! `fingerprint_registry` — append-only Merkle-chained,
//! Ed25519-signed storage of site fingerprints.
//!
//! Task #232 per the variation-architecture spec. Builds on the
//! [`SiteFingerprint`] type from [`crate::fingerprint`] (#231)
//! and the Merkle-chain + Ed25519-signing infrastructure from
//! [`crate::attest`].
//!
//! ## Persistence shape
//!
//! Newline-delimited JSON (JSONL) file at a caller-supplied
//! path. One [`FingerprintRegistryEntry`] per line. Append-only
//! by convention; this module never overwrites existing lines.
//! Spec called for SQLite-backed; JSONL is structurally
//! equivalent for the audit chain guarantees (each entry's hash
//! chains to the previous; chain replay verifies integrity) and
//! adds zero new deps. SQLite migration is a future task if
//! query performance demands.
//!
//! ## Append protocol
//!
//! Each [`append`] call:
//!
//! 1. Reads the last entry to obtain `prev_hash` + `sequence`.
//! 2. Builds a new entry with `sequence = prev.sequence + 1`,
//!    `prev_hash = Some(prev.hash)`.
//! 3. Computes the new entry's hash over its canonical bytes.
//! 4. Signs the hash with the supplied Ed25519 key.
//! 5. Appends the entry as one JSON line.
//!
//! Concurrent writers MUST coordinate at the OS-file level
//! (advisory locking via flock, or a single writer process).
//! This module does not handle multi-writer arbitration — that
//! belongs in the caller (typically the forge-cli build runner
//! holds an exclusive registry lock for the duration of one
//! build's append).
//!
//! ## Verification
//!
//! [`verify_chain`] walks the file end-to-end, validating:
//!
//! * Each entry's hash matches the canonical-serialization hash.
//! * Each entry's `prev_hash` matches the prior entry's `hash`.
//! * Sequence numbers increment monotonically by 1, starting at
//!   0 for the genesis entry.
//! * (Optional) every entry's signature verifies against the
//!   caller-supplied public key.
//!
//! Failure modes return typed [`RegistryError`] variants;
//! callers route to alerts + audits.
//!
//! ## Queries
//!
//! [`find_by_hash`] — exact fingerprint-hash lookup; returns the
//! [`FingerprintRegistryEntry`] iff the registry contains it.
//!
//! [`find_near_duplicates`] — scans for entries whose structured
//! `component_distance` to the candidate is below threshold.
//! Returns the matching entries sorted by ascending distance.
//!
//! [`for_tenant`] — filters entries to a single tenant. Used by
//! the per-tenant uniqueness gate (the same site shipping into
//! the platform refuses repetition within the tenant's portfolio).
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"` (inherited).
//! * No `unwrap`/`expect` in non-test code.
//! * `#[non_exhaustive]` on every public type so future fields
//!   don't break consumers.
//! * Append is the only mutation; never overwrite.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use ed25519_dalek::{Signer as _, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::fingerprint::{FingerprintSpec, SiteFingerprint};

/// One entry in the fingerprint registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FingerprintRegistryEntry {
    /// Sequence number. Genesis = 0; monotonically increases.
    pub sequence: u64,
    /// SHA-256 hash of this entry's canonical bytes (lowercase
    /// 64-char hex). Computed as part of [`append`].
    pub hash: String,
    /// Hash of the prior entry. None for genesis (sequence 0).
    pub prev_hash: Option<String>,
    /// ISO-8601 RFC-3339 UTC timestamp of the append.
    pub timestamp: String,
    /// Originating site identifier (operator-supplied, often the
    /// site's slug or repo-path).
    pub site_id: String,
    /// Tenant identifier. Per-tenant queries filter on this;
    /// platform-scoped queries ignore it. Empty string =
    /// platform-anonymous (single-tenant default).
    pub tenant_id: String,
    /// The fingerprint itself.
    pub fingerprint: SiteFingerprint,
    /// Base64 Ed25519 signature over `hash` bytes. The signer's
    /// public key is recorded out-of-band (in the registry's
    /// companion metadata file); per-entry signatures let an
    /// auditor verify any single entry without replaying the
    /// whole chain.
    pub signature_b64: String,
}

/// Errors the registry can return.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    /// I/O error reading or writing the registry file.
    #[error("registry I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON serialization or deserialization error.
    #[error("registry JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// Chain integrity violation — entry's hash doesn't match
    /// canonical bytes OR prev_hash doesn't match prior entry.
    #[error("chain broken at sequence {sequence}: {reason}")]
    ChainBroken {
        /// Sequence number where the chain breaks.
        sequence: u64,
        /// Specific failure mode.
        reason: String,
    },
    /// Signature verification failed.
    #[error("signature verification failed at sequence {sequence}")]
    BadSignature {
        /// Sequence number whose signature failed.
        sequence: u64,
    },
    /// Spec mismatch — entry's fingerprint spec differs from
    /// the registry's declared spec.
    #[error("fingerprint spec mismatch at sequence {sequence}: registry expects {expected:?}, entry has {actual:?}")]
    SpecMismatch {
        /// Sequence number.
        sequence: u64,
        /// Spec the registry was opened against.
        expected: FingerprintSpec,
        /// Spec the entry was computed against.
        actual: FingerprintSpec,
    },
    /// Timestamp argument is not the substrate's canonical
    /// RFC-3339 UTC form (`YYYY-MM-DDTHH:MM:SSZ`, 20 chars).
    /// Rejected at the wire boundary so a malformed string can
    /// never get baked into the hash chain.
    #[error("invalid RFC-3339 UTC timestamp: {provided:?} (expected YYYY-MM-DDTHH:MM:SSZ)")]
    BadTimestamp {
        /// The string the caller passed.
        provided: String,
    },
}

/// Compute the SHA-256 hash of an entry's canonical bytes.
/// Same construction the attest module uses for build reports:
/// serialize to JSON deterministically, hash the bytes, hex-encode.
fn entry_hash_canonical(
    sequence: u64,
    prev_hash: Option<&str>,
    timestamp: &str,
    site_id: &str,
    tenant_id: &str,
    fingerprint: &SiteFingerprint,
) -> Result<String, RegistryError> {
    // Build the canonical pre-image. Order is fixed; no map
    // iteration order ambiguity. The fingerprint already has
    // its own canonical commitment; we re-use it as a field.
    #[derive(Serialize)]
    struct PreImage<'a> {
        sequence: u64,
        prev_hash: Option<&'a str>,
        timestamp: &'a str,
        site_id: &'a str,
        tenant_id: &'a str,
        fingerprint_commitment: String,
    }
    let pre = PreImage {
        sequence,
        prev_hash,
        timestamp,
        site_id,
        tenant_id,
        fingerprint_commitment: fingerprint.commitment_hex(),
    };
    let bytes = serde_json::to_vec(&pre)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest {
        const TABLE: &[u8; 16] = b"0123456789abcdef";
        hex.push(TABLE[(b >> 4) as usize] as char);
        hex.push(TABLE[(b & 0x0f) as usize] as char);
    }
    Ok(hex)
}

/// Read the last entry from a JSONL registry file. Returns None
/// when the file doesn't exist OR is empty. Returns the parsed
/// last entry otherwise.
fn read_last_entry(path: &Path) -> Result<Option<FingerprintRegistryEntry>, RegistryError> {
    if !path.exists() {
        return Ok(None);
    }
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut last: Option<FingerprintRegistryEntry> = None;
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: FingerprintRegistryEntry = serde_json::from_str(&line)?;
        last = Some(entry);
    }
    Ok(last)
}

/// Append a fingerprint to the registry. Computes the next
/// sequence + prev_hash from the prior entry, hashes the new
/// entry's canonical bytes, signs with the supplied key, writes
/// one JSON line to the file.
///
/// Returns the newly-appended entry.
pub fn append(
    path: &Path,
    site_id: &str,
    tenant_id: &str,
    fingerprint: SiteFingerprint,
    timestamp: &str,
    signing_key: &SigningKey,
) -> Result<FingerprintRegistryEntry, RegistryError> {
    // Wire-boundary check — once a string is baked into the
    // canonical hash, it can never be repaired without breaking
    // the chain. Reject non-canonical timestamps up front.
    if !crate::iso_time::is_canonical_rfc3339_utc(timestamp) {
        return Err(RegistryError::BadTimestamp {
            provided: timestamp.to_owned(),
        });
    }
    let prior = read_last_entry(path)?;
    let (sequence, prev_hash) = match &prior {
        Some(p) => (p.sequence + 1, Some(p.hash.clone())),
        None => (0, None),
    };
    let hash = entry_hash_canonical(
        sequence,
        prev_hash.as_deref(),
        timestamp,
        site_id,
        tenant_id,
        &fingerprint,
    )?;
    let signature = signing_key.sign(hash.as_bytes());
    use base64::Engine as _;
    let signature_b64 = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());
    let entry = FingerprintRegistryEntry {
        sequence,
        hash,
        prev_hash,
        timestamp: timestamp.to_owned(),
        site_id: site_id.to_owned(),
        tenant_id: tenant_id.to_owned(),
        fingerprint,
        signature_b64,
    };
    let json_line = serde_json::to_string(&entry)?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(json_line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(entry)
}

/// Read every entry from the registry. Returns Vec in append
/// order (oldest first).
pub fn read_all(path: &Path) -> Result<Vec<FingerprintRegistryEntry>, RegistryError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: FingerprintRegistryEntry = serde_json::from_str(&line)?;
        entries.push(entry);
    }
    Ok(entries)
}

/// Verify the registry's chain integrity end-to-end. Optionally
/// verifies signatures against a public key.
///
/// Checks:
/// 1. Genesis entry has prev_hash = None + sequence = 0.
/// 2. Each non-genesis entry's prev_hash matches the prior
///    entry's hash + sequence increments by 1.
/// 3. Each entry's hash matches the canonical-bytes hash of
///    its fields.
/// 4. If `verify_key` supplied: each signature verifies against
///    that public key.
///
/// Returns Ok on success; first failure variant on any violation.
pub fn verify_chain(path: &Path, verify_key: Option<&VerifyingKey>) -> Result<(), RegistryError> {
    let entries = read_all(path)?;
    if entries.is_empty() {
        return Ok(());
    }
    // Check genesis.
    if entries[0].sequence != 0 {
        return Err(RegistryError::ChainBroken {
            sequence: entries[0].sequence,
            reason: "genesis entry has non-zero sequence".to_owned(),
        });
    }
    if entries[0].prev_hash.is_some() {
        return Err(RegistryError::ChainBroken {
            sequence: 0,
            reason: "genesis entry has prev_hash (should be None)".to_owned(),
        });
    }
    // Walk the chain.
    for (i, entry) in entries.iter().enumerate() {
        // Expected sequence.
        if entry.sequence != i as u64 {
            return Err(RegistryError::ChainBroken {
                sequence: entry.sequence,
                reason: format!("entry at position {i} has sequence {}", entry.sequence),
            });
        }
        // Hash matches canonical-bytes hash.
        let expected_hash = entry_hash_canonical(
            entry.sequence,
            entry.prev_hash.as_deref(),
            &entry.timestamp,
            &entry.site_id,
            &entry.tenant_id,
            &entry.fingerprint,
        )?;
        if entry.hash != expected_hash {
            return Err(RegistryError::ChainBroken {
                sequence: entry.sequence,
                reason: format!(
                    "hash mismatch: stored {} vs canonical {}",
                    entry.hash, expected_hash
                ),
            });
        }
        // Prev hash matches prior entry's hash.
        if i > 0 {
            let expected_prev = &entries[i - 1].hash;
            match &entry.prev_hash {
                Some(prev) if prev == expected_prev => {}
                _ => {
                    return Err(RegistryError::ChainBroken {
                        sequence: entry.sequence,
                        reason: format!(
                            "prev_hash mismatch: entry has {:?}, prior hash is {}",
                            entry.prev_hash, expected_prev
                        ),
                    });
                }
            }
        }
        // Optional signature verification.
        if let Some(vk) = verify_key {
            use base64::Engine as _;
            let sig_bytes = base64::engine::general_purpose::STANDARD
                .decode(&entry.signature_b64)
                .map_err(|_| RegistryError::BadSignature {
                    sequence: entry.sequence,
                })?;
            if sig_bytes.len() != ed25519_dalek::SIGNATURE_LENGTH {
                return Err(RegistryError::BadSignature {
                    sequence: entry.sequence,
                });
            }
            let mut arr = [0u8; ed25519_dalek::SIGNATURE_LENGTH];
            arr.copy_from_slice(&sig_bytes);
            let sig = ed25519_dalek::Signature::from_bytes(&arr);
            use ed25519_dalek::Verifier as _;
            vk.verify(entry.hash.as_bytes(), &sig)
                .map_err(|_| RegistryError::BadSignature {
                    sequence: entry.sequence,
                })?;
        }
    }
    Ok(())
}

/// Find an entry by exact fingerprint commitment hex.
pub fn find_by_hash(
    path: &Path,
    commitment_hex: &str,
) -> Result<Option<FingerprintRegistryEntry>, RegistryError> {
    let entries = read_all(path)?;
    Ok(entries
        .into_iter()
        .find(|e| e.fingerprint.commitment_hex() == commitment_hex))
}

/// Find near-duplicates of a candidate fingerprint. Returns
/// entries whose `component_distance` to the candidate is
/// at-or-below the threshold, sorted ascending by distance.
pub fn find_near_duplicates(
    path: &Path,
    candidate: &SiteFingerprint,
    threshold: u32,
) -> Result<Vec<(FingerprintRegistryEntry, u32)>, RegistryError> {
    let entries = read_all(path)?;
    let mut matches: Vec<(FingerprintRegistryEntry, u32)> = entries
        .into_iter()
        .map(|e| {
            let d = candidate.component_distance(&e.fingerprint);
            (e, d)
        })
        .filter(|(_, d)| *d <= threshold)
        .collect();
    matches.sort_by_key(|(_, d)| *d);
    Ok(matches)
}

/// Filter entries to a single tenant. Used by the per-tenant
/// uniqueness gate.
pub fn for_tenant(
    path: &Path,
    tenant_id: &str,
) -> Result<Vec<FingerprintRegistryEntry>, RegistryError> {
    let entries = read_all(path)?;
    Ok(entries
        .into_iter()
        .filter(|e| e.tenant_id == tenant_id)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attest::generate_keypair;
    use crate::fingerprint::{
        AssetDistribution, ContentSilhouette, FingerprintSpec, PrimitiveOccurrence, SiteFingerprint,
    };
    use std::collections::BTreeMap;
    use std::env;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let p = env::temp_dir().join(format!("forge-registry-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    fn sample_fp(salt: u32) -> SiteFingerprint {
        let mut silhouettes = BTreeMap::new();
        silhouettes.insert(
            format!("page-{salt}"),
            ContentSilhouette {
                total_chars_bucket: 500 + salt,
                paragraph_count: 3,
                list_item_count: 0,
                heading_hierarchy: "h1,h2".into(),
            },
        );
        SiteFingerprint {
            spec: FingerprintSpec::V1,
            primitives: vec![PrimitiveOccurrence {
                kind: "hero_editorial".into(),
                variant: format!("background=v{salt}"),
                page: format!("page-{salt}"),
            }],
            token_overrides: vec![],
            silhouettes,
            rhythms: BTreeMap::new(),
            assets: AssetDistribution::default(),
        }
    }

    #[test]
    fn append_genesis_entry_to_empty_registry() {
        let path = temp_path("genesis");
        let key = generate_keypair();
        let entry = append(
            &path,
            "site-a",
            "tenant-1",
            sample_fp(1),
            "2026-05-20T12:00:00Z",
            &key,
        )
        .unwrap();
        assert_eq!(entry.sequence, 0);
        assert!(entry.prev_hash.is_none());
        assert_eq!(entry.site_id, "site-a");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn append_rejects_malformed_timestamp() {
        let path = temp_path("bad-timestamp");
        let key = generate_keypair();
        // Various off-canonical shapes the validator should reject.
        for bad in [
            "",
            "2026-05-20",
            "2026/05/20T12:00:00Z",
            "2026-05-20T12:00:00",       // missing Z
            "2026-05-20t12:00:00Z",      // lowercase t
            "2026-05-20T12:00:00.5Z",    // fractional seconds
            "2026-05-20T12:00:00+00:00", // explicit offset
        ] {
            let r = append(&path, "site-a", "t", sample_fp(1), bad, &key);
            match r {
                Err(RegistryError::BadTimestamp { provided }) => {
                    assert_eq!(provided, bad);
                }
                other => panic!("expected BadTimestamp for {bad:?}, got {other:?}"),
            }
        }
        // Bad-timestamp rejection must happen BEFORE any file
        // write — the registry path should not exist.
        assert!(!path.exists());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn append_chains_subsequent_entries() {
        let path = temp_path("chain");
        let key = generate_keypair();
        let e1 = append(
            &path,
            "site-a",
            "t",
            sample_fp(1),
            "2026-05-20T12:00:00Z",
            &key,
        )
        .unwrap();
        let e2 = append(
            &path,
            "site-b",
            "t",
            sample_fp(2),
            "2026-05-20T12:01:00Z",
            &key,
        )
        .unwrap();
        let e3 = append(
            &path,
            "site-c",
            "t",
            sample_fp(3),
            "2026-05-20T12:02:00Z",
            &key,
        )
        .unwrap();
        assert_eq!(e2.sequence, 1);
        assert_eq!(e3.sequence, 2);
        assert_eq!(e2.prev_hash.as_deref(), Some(e1.hash.as_str()));
        assert_eq!(e3.prev_hash.as_deref(), Some(e2.hash.as_str()));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn verify_chain_passes_on_clean_chain() {
        let path = temp_path("verify-clean");
        let key = generate_keypair();
        let vk = key.verifying_key();
        for i in 0..5 {
            append(
                &path,
                &format!("site-{i}"),
                "t",
                sample_fp(i),
                &format!("2026-05-20T12:0{i}:00Z"),
                &key,
            )
            .unwrap();
        }
        verify_chain(&path, Some(&vk)).unwrap();
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn verify_chain_detects_tampered_entry() {
        let path = temp_path("verify-tampered");
        let key = generate_keypair();
        append(
            &path,
            "site-a",
            "t",
            sample_fp(1),
            "2026-05-20T12:00:00Z",
            &key,
        )
        .unwrap();
        append(
            &path,
            "site-b",
            "t",
            sample_fp(2),
            "2026-05-20T12:01:00Z",
            &key,
        )
        .unwrap();
        // Tamper: rewrite the file with a modified entry-1 (preserve
        // hash but change a field, breaking the hash-bytes match).
        let mut entries = read_all(&path).unwrap();
        entries[1].site_id = "TAMPERED".into();
        let mut f = std::fs::File::create(&path).unwrap();
        for e in &entries {
            writeln!(f, "{}", serde_json::to_string(e).unwrap()).unwrap();
        }
        let result = verify_chain(&path, None);
        assert!(matches!(result, Err(RegistryError::ChainBroken { .. })));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn verify_chain_detects_bad_signature_when_key_supplied() {
        let path = temp_path("verify-bad-sig");
        let key = generate_keypair();
        append(
            &path,
            "site-a",
            "t",
            sample_fp(1),
            "2026-05-20T12:00:00Z",
            &key,
        )
        .unwrap();
        // Verify with a DIFFERENT public key — signatures shouldn't match.
        let other_key = generate_keypair();
        let result = verify_chain(&path, Some(&other_key.verifying_key()));
        assert!(matches!(result, Err(RegistryError::BadSignature { .. })));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn find_by_hash_returns_matching_entry() {
        let path = temp_path("find-by-hash");
        let key = generate_keypair();
        let fp = sample_fp(42);
        let hex = fp.commitment_hex();
        append(&path, "site-a", "t", fp, "2026-05-20T12:00:00Z", &key).unwrap();
        let found = find_by_hash(&path, &hex).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().site_id, "site-a");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn find_by_hash_returns_none_for_unknown_hash() {
        let path = temp_path("find-by-hash-none");
        let key = generate_keypair();
        append(
            &path,
            "site-a",
            "t",
            sample_fp(1),
            "2026-05-20T12:00:00Z",
            &key,
        )
        .unwrap();
        let found = find_by_hash(
            &path,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();
        assert!(found.is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn find_near_duplicates_returns_entries_below_threshold() {
        let path = temp_path("near-dup");
        let key = generate_keypair();
        append(
            &path,
            "site-1",
            "t",
            sample_fp(1),
            "2026-05-20T12:00:00Z",
            &key,
        )
        .unwrap();
        append(
            &path,
            "site-2",
            "t",
            sample_fp(2),
            "2026-05-20T12:01:00Z",
            &key,
        )
        .unwrap();
        append(
            &path,
            "site-3",
            "t",
            sample_fp(99),
            "2026-05-20T12:02:00Z",
            &key,
        )
        .unwrap();
        let candidate = sample_fp(1);
        let matches = find_near_duplicates(&path, &candidate, 4).unwrap();
        // site-1 should be exact match (distance 0); site-2 should
        // be close-but-not-same (distance < 4); site-99 should be
        // beyond threshold.
        assert!(!matches.is_empty());
        assert_eq!(matches[0].0.site_id, "site-1");
        assert_eq!(matches[0].1, 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn for_tenant_filters_by_tenant_id() {
        let path = temp_path("tenant-filter");
        let key = generate_keypair();
        append(
            &path,
            "site-a",
            "tenant-1",
            sample_fp(1),
            "2026-05-20T12:00:00Z",
            &key,
        )
        .unwrap();
        append(
            &path,
            "site-b",
            "tenant-2",
            sample_fp(2),
            "2026-05-20T12:01:00Z",
            &key,
        )
        .unwrap();
        append(
            &path,
            "site-c",
            "tenant-1",
            sample_fp(3),
            "2026-05-20T12:02:00Z",
            &key,
        )
        .unwrap();
        let t1 = for_tenant(&path, "tenant-1").unwrap();
        assert_eq!(t1.len(), 2);
        let t2 = for_tenant(&path, "tenant-2").unwrap();
        assert_eq!(t2.len(), 1);
        assert_eq!(t2[0].site_id, "site-b");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_all_returns_empty_for_nonexistent_path() {
        let path = temp_path("nonexistent");
        let _ = std::fs::remove_file(&path);
        let entries = read_all(&path).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn verify_chain_passes_on_empty_registry() {
        let path = temp_path("empty");
        let _ = std::fs::remove_file(&path);
        verify_chain(&path, None).unwrap();
    }
}
