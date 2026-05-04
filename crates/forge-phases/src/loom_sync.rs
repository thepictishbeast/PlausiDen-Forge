//! `loom_sync` — verify `static/loom-skin.css` matches the canonical
//! Loom design-system source.
//!
//! Owner doctrine 2026-05-04: "you can hard code fixes into loom
//! cms and forge just dont hard code fixes into what it generates."
//! This phase enforces that boundary mechanically. Loom is the
//! source of truth; the PoC's `static/loom-skin.css` should mirror
//! it. Drift surfaces as a `Warn` finding, not `Strict` — drift
//! is sometimes intentional during a multi-step migration; the
//! warning makes it visible without blocking the build.
//!
//! Detection algorithm:
//!
//! 1. Locate Loom skin.css at `LOOM_PATH` env var or default sibling
//!    `~/Development/PlausiDen/PlausiDen-Loom/loom-tokens/src/skin.css`.
//! 2. Compute SHA-384 of that file.
//! 3. Read `static/loom-skin.css` and look for the marker comment
//!    `/* SYNC-FROM-LOOM:sha384-... */` on the first line.
//! 4. If marker matches the freshly-computed hash → silent ok.
//! 5. If marker absent or mismatched → emit `Warn` with the diff
//!    summary + suggestion to run `forge --sync-loom`.
//!
//! BUG ASSUMPTION: a future split of skin.css into multiple files
//! (e.g. `loom-skin.css` for synced + `poc-extensions.css` for
//! PoC-specific composite components) will require the marker to
//! reference all sources. The single-file v1 records exactly one
//! hash; extending the marker grammar is a non-breaking change
//! (the parser regex already accepts arbitrary text after the
//! sha384 digest).

use std::path::{Path, PathBuf};

use base64::Engine as _;
use forge_core::{BuildCtx, BuildError, Finding, Phase};
use sha2::{Digest, Sha384};

/// `loom_sync` phase implementation.
#[derive(Debug, Default)]
pub struct LoomSyncPhase;

impl LoomSyncPhase {
    /// Default Loom skin.css path resolution. Reads `LOOM_PATH`
    /// env var if set, falls back to the canonical sibling-repo
    /// layout.
    fn loom_skin_path() -> PathBuf {
        if let Ok(p) = std::env::var("LOOM_PATH") {
            return PathBuf::from(p);
        }
        // BUG ASSUMPTION: this default is correct for the dev
        // environment used by the project owner. CI environments
        // MUST set LOOM_PATH explicitly — checking out a sibling
        // Loom repo into the build runner's HOME is fragile.
        PathBuf::from("/home/user/Development/PlausiDen/PlausiDen-Loom/loom-tokens/src/skin.css")
    }
}

impl Phase for LoomSyncPhase {
    fn name(&self) -> &'static str {
        "loom_sync"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let loom_path = Self::loom_skin_path();
        if !loom_path.exists() {
            // SUPERSOCIETY: we don't fail the build — sibling
            // checkout being absent is operator config, not a
            // forge bug. We DO surface it as a warn so the
            // operator can re-checkout if intended.
            return Ok(vec![Finding::warn(
                self.name(),
                loom_path.display().to_string(),
                format!(
                    "Loom skin.css not found at {}. Set LOOM_PATH or check out PlausiDen-Loom into ~/Development/PlausiDen.",
                    loom_path.display()
                ),
            )]);
        }
        let poc_path = ctx.static_dir.join("loom-skin.css");
        if !poc_path.exists() {
            return Ok(vec![Finding::warn(
                self.name(),
                poc_path.display().to_string(),
                "PoC skin.css missing — run `forge --sync-loom` to bootstrap.".to_owned(),
            )]);
        }

        let loom_bytes = read_file(&loom_path, self.name())?;
        let loom_hash = sha384_b64(&loom_bytes);

        let poc_text = read_text(&poc_path, self.name())?;
        let recorded = parse_marker(&poc_text);
        let expected = format!("sha384-{loom_hash}");

        match recorded {
            None => Ok(vec![Finding::warn(
                self.name(),
                relative(&poc_path, &ctx.root),
                "no SYNC-FROM-LOOM marker — skin.css has never been auto-synced from Loom. Run `forge --sync-loom`.".to_owned(),
            )]),
            Some(rec) if rec == expected => Ok(vec![]),
            Some(rec) => Ok(vec![Finding::warn(
                self.name(),
                relative(&poc_path, &ctx.root),
                format!(
                    "Loom skin.css drift (recorded {rec}, current {expected}). Run `forge --sync-loom` to update."
                ),
            )]),
        }
    }
}

/// Read a file's bytes, mapping I/O errors into `BuildError::Io`.
fn read_file(path: &Path, phase: &str) -> Result<Vec<u8>, BuildError> {
    std::fs::read(path).map_err(|e| BuildError::Io {
        context: format!("{phase}: read {}", path.display()),
        source: e,
    })
}

/// Read a file as UTF-8 text. Replaces invalid UTF-8 with U+FFFD —
/// for skin.css this is fine because invalid UTF-8 in CSS is
/// already a parse error other phases catch.
fn read_text(path: &Path, phase: &str) -> Result<String, BuildError> {
    let bytes = read_file(path, phase)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// SHA-384 of bytes, base64-encoded with the standard alphabet.
/// Matches the SRI v2 spec (W3C SRI § 3.5).
fn sha384_b64(bytes: &[u8]) -> String {
    let mut h = Sha384::new();
    h.update(bytes);
    let digest = h.finalize();
    base64::engine::general_purpose::STANDARD.encode(digest)
}

/// Parse the first SYNC-FROM-LOOM marker out of a CSS file.
///
/// Format: `/* SYNC-FROM-LOOM:sha384-<base64> ... */` on or near
/// the first line. The trailing-text portion is ignored; we just
/// extract the `sha384-...` token.
fn parse_marker(css: &str) -> Option<String> {
    // Examine only the first KB to avoid pathological "marker on
    // line 9000" scans. The marker is supposed to be the first
    // line; if it's not in the first 1024 bytes, treat as absent.
    let head: &str = if css.len() > 1024 { &css[..1024] } else { css };
    let needle = "sha384-";
    let idx = head.find("SYNC-FROM-LOOM:")?;
    let after = &head[idx..];
    let needle_idx = after.find(needle)?;
    let rest = &after[needle_idx..];
    // Stop on whitespace or `*` (end-of-comment). Base64 alphabet
    // never contains either, so this safely bounds the digest.
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '*' || c == ',' || c == ';')
        .unwrap_or(rest.len());
    Some(rest[..end].to_owned())
}

/// Make a relative path display string for finding messages.
fn relative(p: &Path, root: &Path) -> String {
    p.strip_prefix(root)
        .map(|s| s.display().to_string())
        .unwrap_or_else(|_| p.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_marker_extracts_digest() {
        let css = "/* SYNC-FROM-LOOM:sha384-AAAA1234abcd== — auto-synced */\n.foo {}";
        assert_eq!(parse_marker(css), Some("sha384-AAAA1234abcd==".to_owned()));
    }

    #[test]
    fn parse_marker_absent_returns_none() {
        let css = "/* not the right marker */ .foo {}";
        assert_eq!(parse_marker(css), None);
    }

    #[test]
    fn parse_marker_only_scans_first_kb() {
        let mut css = String::with_capacity(2048);
        css.push_str(&".foo {} ".repeat(140)); // > 1KB of content
        css.push_str("/* SYNC-FROM-LOOM:sha384-XYZ */");
        assert_eq!(parse_marker(&css), None);
    }

    #[test]
    fn sha384_b64_known_vector() {
        // Empty-input SHA-384, base64-encoded. Hex per FIPS-180-4
        // is 38b060a751ac96384cd9327eb1b1e36a21fdb71114be07434c
        // 0cc7bf63f6e1da274edebfe76f65fbd51ad2f14898b95b which
        // base64-encodes to the value below. Verifies sha2 + base64
        // crates are integrated correctly.
        let h = sha384_b64(b"");
        assert_eq!(
            h,
            "OLBgp1GsljhM2TJ+sbHjaiH9txEUvgdDTAzHv2P24donTt6/529l+9Ua0vFImLlb"
        );
    }
}
