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
    /// Resolve the canonical Loom skin.css. Resolution order:
    ///   1. `LOOM_PATH` env var (operator / CI override)
    ///   2. Sibling-of-Forge-root: `<ctx.root>/../PlausiDen-Loom/loom-tokens/src/skin.css`
    ///      — matches the canonical `~/projects/PlausiDen-<Name>/`
    ///      layout (memory: plausiden_canonical_dir).
    ///   3. `$HOME/projects/PlausiDen-Loom/loom-tokens/src/skin.css`
    ///   4. `$HOME/Development/PlausiDen/PlausiDen-Loom/loom-tokens/src/skin.css`
    ///      (legacy layout from early dev environments).
    /// First existing path wins; if none exist, returns the
    /// sibling-of-root candidate so the operator-facing warn
    /// quotes the most-likely-intended location.
    fn loom_skin_path(ctx_root: &std::path::Path) -> PathBuf {
        const TAIL: &str = "PlausiDen-Loom/loom-tokens/src/skin.css";
        if let Ok(p) = std::env::var("LOOM_PATH") {
            return PathBuf::from(p);
        }
        let sibling = ctx_root
            .parent()
            .map(|p| p.join("PlausiDen-Loom/loom-tokens/src/skin.css"));
        let home = std::env::var("HOME").ok().map(PathBuf::from);
        let candidates: Vec<PathBuf> = [
            sibling.clone(),
            home.as_ref().map(|h| h.join("projects").join(TAIL)),
            home.as_ref()
                .map(|h| h.join("Development/PlausiDen").join(TAIL)),
        ]
        .into_iter()
        .flatten()
        .collect();
        for c in &candidates {
            if c.exists() {
                return c.clone();
            }
        }
        // Nothing exists — return the sibling candidate so the
        // failure message points at the right place.
        sibling.unwrap_or_else(|| PathBuf::from(TAIL))
    }
}

impl Phase for LoomSyncPhase {
    fn name(&self) -> &'static str {
        "loom_sync"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let loom_path = Self::loom_skin_path(&ctx.root);
        if !loom_path.exists() {
            // SUPERSOCIETY: we don't fail the build — sibling
            // checkout being absent is operator config, not a
            // forge bug. We DO surface it as a warn so the
            // operator can re-checkout if intended.
            return Ok(vec![Finding::warn(
                self.name(),
                loom_path.display().to_string(),
                format!(
                    "Loom skin.css not found at {}. Set LOOM_PATH or check out PlausiDen-Loom as a sibling of PlausiDen-Forge.",
                    loom_path.display()
                ),
            )
            .why(
                "loom_sync verifies static/loom-skin.css matches the canonical bytes from a \
                 PlausiDen-Loom checkout; without that checkout the sync chain is unverifiable",
            )
            .fix(
                "set LOOM_PATH=/path/to/PlausiDen-Loom OR check out PlausiDen-Loom as a sibling \
                 directory of PlausiDen-Forge (the default loom_skin_path probe location)",
            )
            .avoid(
                "don't hand-copy skin.css bytes from Loom releases — the SYNC-FROM-LOOM \
                 marker won't match and the next build will flag drift",
            )]);
        }
        let poc_path = ctx.static_dir.join("loom-skin.css");
        if !poc_path.exists() {
            return Ok(vec![Finding::warn(
                self.name(),
                poc_path.display().to_string(),
                "PoC skin.css missing — run `forge --sync-loom` to bootstrap.".to_owned(),
            )
            .why(
                "static/loom-skin.css is the design-system bytes every rendered page links to; \
                 a missing file means every <link rel=stylesheet> 404s in the browser",
            )
            .fix("run `forge --sync-loom` to copy current Loom skin.css bytes into static/")
            .avoid(
                "don't `touch static/loom-skin.css` — an empty file passes existence check but \
                 strips every rendered page of its design system",
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
            )
            .why(
                "the SYNC-FROM-LOOM:sha384-<hash> marker is the chain-of-custody proof that \
                 the CSS bytes match a specific Loom revision; without it, drift can't be \
                 detected on subsequent builds",
            )
            .fix(
                "run `forge --sync-loom` to write the canonical bytes + marker to \
                 static/loom-skin.css in one step",
            )
            .avoid(
                "don't edit static/loom-skin.css by hand — Forge regenerates it on \
                 `forge --sync-loom` and any manual edit gets clobbered",
            )]),
            Some(rec) if rec == expected => Ok(vec![]),
            Some(rec) => Ok(vec![Finding::warn(
                self.name(),
                relative(&poc_path, &ctx.root),
                format!(
                    "Loom skin.css drift (recorded {rec}, current {expected}). Run `forge --sync-loom` to update."
                ),
            )
            .why(
                "Loom's canonical skin.css has changed (new design-system tokens, fixed a11y \
                 contrast, etc.) but static/loom-skin.css still carries the old hash. The \
                 served site is shipping the stale design system",
            )
            .fix("run `forge --sync-loom` to copy the current Loom skin.css bytes + update the marker")
            .avoid(
                "don't manually update the marker — only the canonical sync command writes \
                 both bytes + hash atomically",
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
    fn loom_sync_findings_carry_advocacy() {
        // Run the phase in an empty temp root — loom_path won't
        // exist, triggering the first warn branch. Assert advocacy
        // is populated across the four code paths.
        let tmp =
            std::env::temp_dir().join(format!("forge-loom-sync-advocacy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("mkdir");
        std::fs::create_dir_all(tmp.join("static")).expect("mkdir static");
        let ctx = BuildCtx {
            root: tmp.clone(),
            static_dir: tmp.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = LoomSyncPhase.run(&ctx).expect("run");
        assert_eq!(findings.len(), 1, "expected one loom-missing warn");
        let adv = &findings[0].advocacy;
        assert!(!adv.why.is_empty(), "loom_sync warn must carry .why()");
        assert!(
            !adv.substrate_fix.is_empty(),
            "loom_sync warn must carry .fix()"
        );
        assert!(
            adv.anti_pattern.is_some(),
            "loom_sync warn must carry .avoid()"
        );
        let _ = std::fs::remove_dir_all(&tmp);
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
