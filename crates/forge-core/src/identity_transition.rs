//! `identity_transition` — atomic, reviewable, attested identity
//! changes.
//!
//! Task #238 per the variation-architecture spec. Where
//! `crate::provenance` captures the identity at a single build,
//! this module captures the CHANGE from one identity to the next:
//! an append-only chain at `reports/identity-transitions.jsonl`
//! records every operator-driven identity edit with a structured
//! diff + Ed25519 signature.
//!
//! ## Atomic-change discipline
//!
//! An identity transition is **atomic** when every axis that
//! should change together actually changes together. The
//! [`classify_diff`] helper marks each axis change with a
//! category; [`is_atomic`] cross-checks the categories against
//! the cascade rules:
//!
//! * voice.tier change → expect a related allowed_primitives /
//!   forbidden_primitives change (technical/plain voice tiers
//!   should cascade to primitive whitelist/blacklist).
//! * mood.primary change → expect a related theme_variant or
//!   allowed_primitives change.
//! * density_preference change → expect a related
//!   tokens.max_per_page_overrides change.
//!
//! Per AVP-2: this is the prevent-partial-change architecture
//! that pairs with #240's identity_coherence audit. The audit
//! catches static inconsistencies; the transition workflow
//! catches DYNAMIC partial-update sequences.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `#[non_exhaustive]` on every public type.
//! * No unwrap/expect in non-test code.
//! * Pure functions; filesystem walking lives in forge-cli.

use std::collections::BTreeSet;
use std::path::Path;

use ed25519_dalek::{Signer as _, SigningKey, Verifier as _, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::provenance::ProvenanceError;
use crate::site_identity::SiteIdentity;

/// Spec version. Bumped only when canonical-bytes derivation
/// changes in a way that invalidates existing signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TransitionSpec {
    /// Initial spec, 2026-05-20.
    #[default]
    V1,
}

impl TransitionSpec {
    /// Stable kebab-case slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::V1 => "v1",
        }
    }
}

/// One identity transition entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct IdentityTransition {
    /// Schema version.
    pub spec: TransitionSpec,
    /// SHA-256 hex of the prior identity's [site_identity] bytes.
    /// Empty for the genesis transition.
    pub from_identity_hash: String,
    /// SHA-256 hex of the new identity's [site_identity] bytes.
    pub to_identity_hash: String,
    /// ISO-8601 RFC-3339 UTC timestamp.
    pub timestamp: String,
    /// Operator-supplied site identifier.
    pub site_id: String,
    /// Operator-supplied tenant identifier.
    pub tenant_id: String,
    /// Axes that changed (e.g. "voice.tier", "mood.primary").
    pub axes_changed: Vec<String>,
    /// Human-readable diff summary; one line per axis.
    pub diff_summary: String,
    /// Whether this transition was declared atomic
    /// (operator-confirmed cascade-aware).
    pub atomic: bool,
    /// Base64 Ed25519 signature over the canonical bytes.
    /// Empty when unsigned.
    pub signature_b64: String,
}

impl IdentityTransition {
    /// Build a transition from old + new identities. Pure; no
    /// signing.
    #[must_use]
    pub fn build(
        from_identity_hash: impl Into<String>,
        to_identity_hash: impl Into<String>,
        timestamp: impl Into<String>,
        site_id: impl Into<String>,
        tenant_id: impl Into<String>,
        diff: IdentityDiff,
    ) -> Self {
        let from = from_identity_hash.into();
        let to = to_identity_hash.into();
        let axes_changed: Vec<String> = diff
            .axes_changed
            .iter()
            .map(String::from)
            .collect();
        let diff_summary = diff
            .axes_changed
            .iter()
            .map(|axis| format!("- {axis}"))
            .collect::<Vec<_>>()
            .join("\n");
        let atomic = diff.is_atomic();
        Self {
            spec: TransitionSpec::V1,
            from_identity_hash: from,
            to_identity_hash: to,
            timestamp: timestamp.into(),
            site_id: site_id.into(),
            tenant_id: tenant_id.into(),
            axes_changed,
            diff_summary,
            atomic,
            signature_b64: String::new(),
        }
    }

    /// Canonical byte string used for hashing/signing.
    fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"identity-transition/");
        out.extend_from_slice(self.spec.slug().as_bytes());
        out.push(b'\n');
        out.extend_from_slice(b"from=");
        out.extend_from_slice(self.from_identity_hash.as_bytes());
        out.push(b'\n');
        out.extend_from_slice(b"to=");
        out.extend_from_slice(self.to_identity_hash.as_bytes());
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
        out.extend_from_slice(b"axes=");
        out.extend_from_slice(self.axes_changed.join(",").as_bytes());
        out.push(b'\n');
        out.extend_from_slice(b"atomic=");
        out.extend_from_slice(if self.atomic { b"true" } else { b"false" });
        out.push(b'\n');
        out
    }

    /// Sign with the supplied Ed25519 key.
    pub fn sign(&mut self, key: &SigningKey) {
        let bytes = self.canonical_bytes();
        let sig = key.sign(&bytes);
        use base64::Engine as _;
        self.signature_b64 = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());
    }

    /// Verify against a public key.
    pub fn verify(&self, key: &VerifyingKey) -> Result<(), ProvenanceError> {
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

/// Structured diff between two SiteIdentity declarations.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct IdentityDiff {
    /// Axis names that changed.
    pub axes_changed: Vec<String>,
}

impl IdentityDiff {
    /// Returns true if every axis change has a matching cascade
    /// partner per the atomic-change discipline. Atomic when:
    ///
    /// * voice.tier change is paired with allowed_primitives or
    ///   forbidden_primitives change.
    /// * mood.primary change is paired with theme_variant or
    ///   allowed_primitives change.
    /// * density_preference change is paired with tokens.* change.
    ///
    /// A transition with no axis changes is trivially atomic.
    /// A transition that only changes within a single cascade
    /// group is atomic (e.g. just voice.tier + allowed_primitives).
    /// A transition that changes voice.tier AND mood.primary
    /// without their respective cascade partners is non-atomic.
    #[must_use]
    pub fn is_atomic(&self) -> bool {
        let axes: BTreeSet<&str> = self.axes_changed.iter().map(String::as_str).collect();
        if axes.is_empty() {
            return true;
        }
        // Voice cascade.
        if axes.contains("voice.tier") {
            let has_primitive_cascade = axes.contains("allowed_primitives")
                || axes.contains("forbidden_primitives");
            if !has_primitive_cascade {
                return false;
            }
        }
        // Mood cascade.
        if axes.contains("mood.primary") {
            let has_mood_cascade =
                axes.iter().any(|a| a.starts_with("theme_variant"))
                    || axes.contains("allowed_primitives");
            if !has_mood_cascade {
                return false;
            }
        }
        // Density cascade.
        if axes.contains("density_preference") {
            let has_density_cascade =
                axes.iter().any(|a| a.starts_with("tokens"));
            if !has_density_cascade {
                return false;
            }
        }
        true
    }
}

/// Classify which axes differ between two SiteIdentity values.
#[must_use]
pub fn classify_diff(prev: &SiteIdentity, next: &SiteIdentity) -> IdentityDiff {
    let mut axes = Vec::new();
    if prev.site_id != next.site_id {
        axes.push("site_id".to_owned());
    }
    if prev.tenant_id != next.tenant_id {
        axes.push("tenant_id".to_owned());
    }
    if prev.voice.tier != next.voice.tier
        || prev.voice.max_avg_sentence_words != next.voice.max_avg_sentence_words
        || prev.voice.vocabulary_tier != next.voice.vocabulary_tier
    {
        axes.push("voice.tier".to_owned());
    }
    if prev.mood.primary != next.mood.primary
        || prev.mood.secondary != next.mood.secondary
        || prev.mood.drift_budget != next.mood.drift_budget
    {
        axes.push("mood.primary".to_owned());
    }
    if prev.density_preference != next.density_preference {
        axes.push("density_preference".to_owned());
    }
    if prev.tokens.max_per_page_overrides != next.tokens.max_per_page_overrides
        || prev.tokens.max_site_distinct_overrides != next.tokens.max_site_distinct_overrides
    {
        axes.push("tokens".to_owned());
    }
    if prev.allowed_primitives != next.allowed_primitives {
        axes.push("allowed_primitives".to_owned());
    }
    if prev.forbidden_primitives != next.forbidden_primitives {
        axes.push("forbidden_primitives".to_owned());
    }
    if prev.content_type.len() != next.content_type.len() {
        axes.push("content_type".to_owned());
    }
    if prev.theme_variant.len() != next.theme_variant.len() {
        axes.push("theme_variant".to_owned());
    }
    IdentityDiff {
        axes_changed: axes,
    }
}

/// Append one transition to a JSONL chain.
pub fn append_to_chain(
    path: &Path,
    transition: &IdentityTransition,
) -> Result<(), std::io::Error> {
    let json = serde_json::to_string(transition)?;
    use std::io::Write as _;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(json.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

/// Read every transition from a JSONL chain in append order.
pub fn read_chain(path: &Path) -> Result<Vec<IdentityTransition>, std::io::Error> {
    use std::io::{BufRead as _, BufReader};
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut out = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(t) = serde_json::from_str::<IdentityTransition>(&line) {
            out.push(t);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attest::generate_keypair;
    use crate::site_identity::SiteIdentity;

    #[test]
    fn diff_empty_when_identities_equal() {
        let a = SiteIdentity::default();
        let b = SiteIdentity::default();
        let d = classify_diff(&a, &b);
        assert!(d.axes_changed.is_empty());
        assert!(d.is_atomic());
    }

    #[test]
    fn diff_detects_voice_tier_change() {
        let a = SiteIdentity::default();
        let mut b = a.clone();
        b.voice.tier = Some("technical".into());
        let d = classify_diff(&a, &b);
        assert!(d.axes_changed.contains(&"voice.tier".to_owned()));
    }

    #[test]
    fn diff_is_non_atomic_when_voice_changes_without_primitives() {
        let a = SiteIdentity::default();
        let mut b = a.clone();
        b.voice.tier = Some("technical".into());
        let d = classify_diff(&a, &b);
        assert!(!d.is_atomic());
    }

    #[test]
    fn diff_is_atomic_when_voice_paired_with_primitives() {
        let a = SiteIdentity::default();
        let mut b = a.clone();
        b.voice.tier = Some("technical".into());
        b.allowed_primitives.push("code".into());
        let d = classify_diff(&a, &b);
        assert!(d.is_atomic(), "axes: {:?}", d.axes_changed);
    }

    #[test]
    fn diff_is_non_atomic_for_mood_change_alone() {
        let a = SiteIdentity::default();
        let mut b = a.clone();
        b.mood.primary = Some("editorial".into());
        let d = classify_diff(&a, &b);
        assert!(!d.is_atomic());
    }

    #[test]
    fn diff_is_atomic_when_mood_paired_with_themes() {
        let a = SiteIdentity::default();
        let mut b = a.clone();
        b.mood.primary = Some("editorial".into());
        b.theme_variant
            .push(forge_core_helper::test_theme_variant("light"));
        let d = classify_diff(&a, &b);
        assert!(d.is_atomic());
    }

    #[test]
    fn diff_is_non_atomic_for_density_change_alone() {
        let a = SiteIdentity::default();
        let mut b = a.clone();
        b.density_preference = Some("dense".into());
        let d = classify_diff(&a, &b);
        assert!(!d.is_atomic());
    }

    #[test]
    fn diff_is_atomic_when_density_paired_with_tokens() {
        let a = SiteIdentity::default();
        let mut b = a.clone();
        b.density_preference = Some("dense".into());
        b.tokens.max_per_page_overrides = 5;
        let d = classify_diff(&a, &b);
        assert!(d.is_atomic());
    }

    #[test]
    fn transition_sign_verify_round_trip() {
        let key = generate_keypair();
        let vk = key.verifying_key();
        let mut t = IdentityTransition::build(
            "from",
            "to",
            "2026-05-20T12:00:00Z",
            "site-a",
            "tenant",
            IdentityDiff {
                axes_changed: vec!["voice.tier".into(), "allowed_primitives".into()],
            },
        );
        assert!(t.atomic);
        t.sign(&key);
        assert!(!t.signature_b64.is_empty());
        t.verify(&vk).unwrap();
    }

    #[test]
    fn transition_verify_fails_on_tamper() {
        let key = generate_keypair();
        let vk = key.verifying_key();
        let mut t = IdentityTransition::build(
            "from",
            "to",
            "ts",
            "site",
            "",
            IdentityDiff::default(),
        );
        t.sign(&key);
        t.to_identity_hash = "tampered".into();
        assert!(t.verify(&vk).is_err());
    }

    #[test]
    fn chain_append_and_read_round_trip() {
        let path = std::env::temp_dir().join(format!(
            "forge-transition-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let t1 = IdentityTransition::build("", "h1", "t1", "s", "", IdentityDiff::default());
        let t2 = IdentityTransition::build("h1", "h2", "t2", "s", "", IdentityDiff::default());
        append_to_chain(&path, &t1).unwrap();
        append_to_chain(&path, &t2).unwrap();
        let entries = read_chain(&path).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].from_identity_hash, "h1");
        let _ = std::fs::remove_file(&path);
    }

    // Helper module to construct ThemeVariant since the struct
    // is non_exhaustive in another module.
    mod forge_core_helper {
        use crate::site_identity::ThemeVariant;
        use serde_json::json;
        pub fn test_theme_variant(name: &str) -> ThemeVariant {
            serde_json::from_value::<ThemeVariant>(json!({
                "name": name,
                "required": false,
            }))
            .expect("test theme variant")
        }
    }
}
