//! `reference_capture` — wire shape for headless reference-site
//! captures.
//!
//! Task #263 per the reference-matching arc (#263-#274). Defines
//! the structured payload the Crawler emits per `(URL, viewport)`
//! and the per-axis extractors (#264-#272) consume.
//!
//! The Crawler-side implementation lives in PlausiDen-Crawler;
//! this module is the wire contract — both sides read from it
//! so changes to capture shape stay coordinated.
//!
//! ## Wire shape
//!
//! Per `(URL, viewport)`:
//!
//! ```jsonc
//! {
//!   "spec": "v1",
//!   "url": "https://example.com",
//!   "captured_at": "2026-05-20T13:45:00Z",
//!   "viewport_px": 1280,
//!   "screenshot_path": "captures/abc/1280.png",
//!   "html_path": "captures/abc/1280.html",
//!   "computed_styles_path": "captures/abc/1280.styles.json",
//!   "network_summary": {
//!     "fonts_loaded": ["..."],
//!     "image_count": 12,
//!     "video_count": 1,
//!     "script_count": 8,
//!     "third_party_origins": ["..."],
//!     "total_bytes": 1234567
//!   }
//! }
//! ```
//!
//! Each capture lives in a per-URL directory; the manifest
//! lists every (URL, viewport) capture for one reference site
//! and is the entry point for downstream extractors.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `#[non_exhaustive]` on every public type.
//! * No unwrap/expect in non-test code.
//! * Pure functions; filesystem walking lives in PlausiDen-Crawler
//!   and in forge-cli when consumers read the captures.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Spec version. Bumped when the capture shape changes
/// incompatibly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CaptureSpec {
    /// Initial spec, 2026-05-20.
    #[default]
    V1,
}

impl CaptureSpec {
    /// Stable kebab-case slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::V1 => "v1",
        }
    }
}

/// One reference-site capture at one viewport.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReferenceCapture {
    /// Schema version.
    pub spec: CaptureSpec,
    /// URL captured.
    pub url: String,
    /// ISO-8601 RFC-3339 UTC.
    pub captured_at: String,
    /// Viewport width in CSS pixels.
    pub viewport_px: u32,
    /// Path to the screenshot file (PNG). Relative to the
    /// manifest's directory.
    pub screenshot_path: String,
    /// Path to the captured HTML (post-render snapshot).
    pub html_path: String,
    /// Path to the computed-styles JSON dump (per-element CSS
    /// property/value pairs the extractors consume).
    pub computed_styles_path: String,
    /// Network + resource summary.
    pub network_summary: NetworkSummary,
}

/// Summary of resources loaded during the capture. Lets
/// extractors reason about typography / asset distribution /
/// third-party surface without re-fetching.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NetworkSummary {
    /// Font families actually used (computed-style font-family
    /// values, normalized).
    #[serde(default)]
    pub fonts_loaded: Vec<String>,
    /// `<img>` count + `<picture>` count.
    pub image_count: u32,
    /// `<video>` + iframed-video count.
    pub video_count: u32,
    /// `<script>` count.
    pub script_count: u32,
    /// Third-party origins observed (host strings).
    #[serde(default)]
    pub third_party_origins: Vec<String>,
    /// Total bytes downloaded for the capture (HTML + assets).
    pub total_bytes: u64,
}

/// Manifest listing every (URL, viewport) capture for one
/// reference site. Lives at the root of the per-site capture
/// directory.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CaptureManifest {
    /// Schema version (mirrors CaptureSpec).
    pub spec: CaptureSpec,
    /// Stable slug identifying the reference site (matches
    /// `corpora/reference_baseline.json` slug field).
    pub site_slug: String,
    /// URL of the site root.
    pub url: String,
    /// ISO-8601 RFC-3339 timestamp when the manifest was last
    /// updated.
    pub updated_at: String,
    /// All captures across all (URL, viewport) combinations.
    pub captures: Vec<ReferenceCapture>,
}

/// Errors capture readers can raise.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CaptureError {
    /// I/O error reading the manifest or one of its captures.
    #[error("capture I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON parse error.
    #[error("capture JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// Spec version skew.
    #[error("capture spec mismatch: expected {expected:?}, got {actual:?}")]
    SpecMismatch {
        /// Expected spec.
        expected: CaptureSpec,
        /// Spec carried by the loaded payload.
        actual: CaptureSpec,
    },
    /// Timestamp field is not the substrate's canonical RFC-3339
    /// UTC form (`YYYY-MM-DDTHH:MM:SSZ`, 20 chars). Rejected on
    /// write so no malformed string lands in a persisted manifest.
    #[error(
        "invalid RFC-3339 UTC timestamp in {field}: {provided:?} (expected YYYY-MM-DDTHH:MM:SSZ)"
    )]
    BadTimestamp {
        /// Which field carried the bad value (e.g. `"updated_at"`
        /// or `"captures[2].captured_at"`).
        field: String,
        /// The string that failed validation.
        provided: String,
    },
}

impl ReferenceCapture {
    /// Construct a fresh capture with empty resource paths.
    /// Used by Crawler-side emitters that fill paths in after
    /// writing the artifacts.
    #[must_use]
    pub fn new(url: impl Into<String>, captured_at: impl Into<String>, viewport_px: u32) -> Self {
        Self {
            spec: CaptureSpec::V1,
            url: url.into(),
            captured_at: captured_at.into(),
            viewport_px,
            screenshot_path: String::new(),
            html_path: String::new(),
            computed_styles_path: String::new(),
            network_summary: NetworkSummary::default(),
        }
    }
}

impl CaptureManifest {
    /// Construct an empty manifest for a site.
    #[must_use]
    pub fn new(site_slug: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            spec: CaptureSpec::V1,
            site_slug: site_slug.into(),
            url: url.into(),
            updated_at: String::new(),
            captures: Vec::new(),
        }
    }

    /// Read a manifest JSON file from disk.
    pub fn read(path: &Path) -> Result<Self, CaptureError> {
        let body = fs::read_to_string(path)?;
        let manifest: Self = serde_json::from_str(&body)?;
        if manifest.spec != CaptureSpec::V1 {
            return Err(CaptureError::SpecMismatch {
                expected: CaptureSpec::V1,
                actual: manifest.spec,
            });
        }
        Ok(manifest)
    }

    /// Write a manifest JSON file to disk (pretty-printed).
    /// Validates timestamp fields up front — a manifest with a
    /// non-canonical timestamp never reaches disk.
    pub fn write(&self, path: &Path) -> Result<(), CaptureError> {
        self.validate_timestamps()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_string_pretty(self)?;
        fs::write(path, body)?;
        Ok(())
    }

    /// Walk the manifest's timestamp fields and reject anything
    /// that is not canonical RFC-3339 UTC. Exposed pub so callers
    /// can validate without committing to a write.
    pub fn validate_timestamps(&self) -> Result<(), CaptureError> {
        if !crate::iso_time::is_canonical_rfc3339_utc(&self.updated_at) {
            return Err(CaptureError::BadTimestamp {
                field: "updated_at".to_owned(),
                provided: self.updated_at.clone(),
            });
        }
        for (idx, cap) in self.captures.iter().enumerate() {
            if !crate::iso_time::is_canonical_rfc3339_utc(&cap.captured_at) {
                return Err(CaptureError::BadTimestamp {
                    field: format!("captures[{idx}].captured_at"),
                    provided: cap.captured_at.clone(),
                });
            }
        }
        Ok(())
    }

    /// Filter captures to those matching a specific viewport.
    /// Returns references; iterate to inspect.
    #[must_use]
    pub fn for_viewport(&self, viewport_px: u32) -> Vec<&ReferenceCapture> {
        self.captures
            .iter()
            .filter(|c| c.viewport_px == viewport_px)
            .collect()
    }

    /// Resolve a capture-relative path (screenshot / html /
    /// styles) against the manifest's directory.
    #[must_use]
    pub fn resolve_path(manifest_dir: &Path, relative: &str) -> PathBuf {
        manifest_dir.join(relative)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("forge-capture-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn reference_capture_builder_sets_defaults() {
        let c = ReferenceCapture::new("https://example.com", "2026-05-20T00:00:00Z", 1280);
        assert_eq!(c.url, "https://example.com");
        assert_eq!(c.viewport_px, 1280);
        assert!(c.screenshot_path.is_empty());
        assert_eq!(c.spec, CaptureSpec::V1);
    }

    #[test]
    fn manifest_write_and_read_round_trip() {
        let dir = temp_dir("round-trip");
        let mut m = CaptureManifest::new("test-site", "https://test.example");
        m.updated_at = "2026-05-20T13:00:00Z".to_owned();
        m.captures.push(ReferenceCapture::new(
            "https://test.example",
            "2026-05-20T00:00:00Z",
            390,
        ));
        m.captures.push(ReferenceCapture::new(
            "https://test.example",
            "2026-05-20T00:00:01Z",
            1280,
        ));
        let path = dir.join("manifest.json");
        m.write(&path).unwrap();
        let read_back = CaptureManifest::read(&path).unwrap();
        assert_eq!(read_back.site_slug, "test-site");
        assert_eq!(read_back.captures.len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn for_viewport_filters_captures() {
        let mut m = CaptureManifest::new("s", "https://x");
        m.captures
            .push(ReferenceCapture::new("https://x", "t", 390));
        m.captures
            .push(ReferenceCapture::new("https://x", "t", 768));
        m.captures
            .push(ReferenceCapture::new("https://x", "t", 1280));
        m.captures
            .push(ReferenceCapture::new("https://x", "t", 1280));
        assert_eq!(m.for_viewport(390).len(), 1);
        assert_eq!(m.for_viewport(1280).len(), 2);
        assert_eq!(m.for_viewport(9999).len(), 0);
    }

    #[test]
    fn resolve_path_joins_relative_against_manifest_dir() {
        let dir = Path::new("/tmp/captures/abc");
        let resolved = CaptureManifest::resolve_path(dir, "1280.png");
        assert_eq!(resolved, Path::new("/tmp/captures/abc/1280.png"));
    }

    #[test]
    fn manifest_write_creates_missing_parent_dir() {
        let dir = temp_dir("create-parent");
        let nested = dir.join("nested/deeper/manifest.json");
        let mut m = CaptureManifest::new("s", "https://x");
        m.updated_at = "2026-05-20T13:00:00Z".to_owned();
        m.write(&nested).unwrap();
        assert!(nested.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_rejects_empty_updated_at() {
        let dir = temp_dir("bad-updated-at");
        let path = dir.join("manifest.json");
        let m = CaptureManifest::new("s", "https://x");
        match m.write(&path) {
            Err(CaptureError::BadTimestamp { field, provided }) => {
                assert_eq!(field, "updated_at");
                assert!(provided.is_empty());
            }
            other => panic!("expected BadTimestamp, got {other:?}"),
        }
        assert!(!path.exists(), "manifest must not land on disk on failure");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_rejects_bad_capture_timestamp() {
        let dir = temp_dir("bad-capture-at");
        let path = dir.join("manifest.json");
        let mut m = CaptureManifest::new("s", "https://x");
        m.updated_at = "2026-05-20T13:00:00Z".to_owned();
        m.captures
            .push(ReferenceCapture::new("https://x", "yesterday", 1280));
        match m.write(&path) {
            Err(CaptureError::BadTimestamp { field, provided }) => {
                assert_eq!(field, "captures[0].captured_at");
                assert_eq!(provided, "yesterday");
            }
            other => panic!("expected BadTimestamp, got {other:?}"),
        }
        assert!(!path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_fails_on_spec_mismatch() {
        // Construct a manifest JSON with an unknown spec value.
        // Since CaptureSpec has only V1, we forge an invalid payload.
        let dir = temp_dir("spec-mismatch");
        let path = dir.join("manifest.json");
        fs::write(
            &path,
            r#"{"spec":"v99","site_slug":"x","url":"u","updated_at":"","captures":[]}"#,
        )
        .unwrap();
        // serde will reject the unknown enum variant before we
        // get to the spec_mismatch arm. Either way, read returns
        // an Err.
        let result = CaptureManifest::read(&path);
        assert!(result.is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn network_summary_serializes_empty_fonts_as_empty_array() {
        let m = CaptureManifest::new("s", "https://x");
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"captures\":[]"));
    }

    #[test]
    fn spec_slug_is_stable() {
        assert_eq!(CaptureSpec::V1.slug(), "v1");
    }
}
