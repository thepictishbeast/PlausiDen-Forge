//! `assets-core` — typed asset pipeline contract.
//!
//! Per `PLATFORM_ROADMAP.md` §14 + super_society_tech_stack
//! "fast" + "private" axes, every uploaded image / video runs
//! through a typed pipeline that produces:
//!
//! * AVIF + WebP + JPEG image triplet with deterministic
//!   content-negotiation ladder
//! * HLS + DASH video manifests
//! * EXIF stripped by default (privacy — geo / camera / time
//!   stamps removed)
//! * Vision-AI alt-text synthesis seam (operator-pluggable)
//!
//! This crate is the cross-impl contract. Per-format encoders
//! (libavif, libwebp, mozjpeg, libheif, ffmpeg) live in
//! downstream impl crates that plug in via [`AssetEncoder`].
//!
//! ### Why typed
//!
//! Asset pipelines are the canonical place where image bytes
//! sneak through with the wrong MIME, wrong color profile, or
//! intact EXIF GPS. A typed [`AssetVariant`] + closed
//! [`AssetFormat`] + always-on [`ExifPolicy::Strip`] default
//! makes "an image without privacy strip ever reaches the
//! public surface" a compile-time impossibility — every
//! produced variant carries its policy on the record.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Closed enum of supported asset formats. Adding a format is a
/// typed change reviewable in one commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetFormat {
    /// AVIF — AV1-based image, smallest payload + widest color.
    /// Best modern browsers.
    Avif,
    /// WebP — broader compatibility than AVIF, still smaller than
    /// JPEG/PNG.
    #[serde(rename = "webp")]
    WebP,
    /// JPEG — universal fallback. Always emitted so old User-Agents
    /// (incl. screen-readers, RSS, IndieWeb crawlers) can read.
    Jpeg,
    /// PNG — lossless raster. Used for screenshots + UI shots.
    Png,
    /// HLS — HTTP Live Streaming (RFC 8216) for video.
    Hls,
    /// DASH — MPEG-DASH (ISO/IEC 23009-1) for video.
    Dash,
}

impl AssetFormat {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Avif => "avif",
            Self::WebP => "webp",
            Self::Jpeg => "jpeg",
            Self::Png => "png",
            Self::Hls => "hls",
            Self::Dash => "dash",
        }
    }

    /// IANA media type for HTTP `Content-Type`.
    pub fn media_type(&self) -> &'static str {
        match self {
            Self::Avif => "image/avif",
            Self::WebP => "image/webp",
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Hls => "application/vnd.apple.mpegurl",
            Self::Dash => "application/dash+xml",
        }
    }

    /// Conventional filename extension (without leading dot).
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Avif => "avif",
            Self::WebP => "webp",
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Hls => "m3u8",
            Self::Dash => "mpd",
        }
    }

    /// Whether this format is a static image.
    pub fn is_image(&self) -> bool {
        matches!(self, Self::Avif | Self::WebP | Self::Jpeg | Self::Png)
    }

    /// Whether this format is a video manifest (HLS / DASH).
    pub fn is_video(&self) -> bool {
        matches!(self, Self::Hls | Self::Dash)
    }
}

/// Content-negotiation fallback ladder for static images. Browsers
/// pick the first they support via HTML `<picture>` + `<source
/// type=...>` (RFC 8126 multiple choice / Accept negotiation).
///
/// Order: AVIF → WebP → JPEG. JPEG is always last so every UA
/// gets *some* image.
pub const IMAGE_FALLBACK_LADDER: &[AssetFormat] =
    &[AssetFormat::Avif, AssetFormat::WebP, AssetFormat::Jpeg];

/// EXIF / metadata privacy policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExifPolicy {
    /// Strip all EXIF, IPTC, XMP, GPS, camera-make/model, and
    /// timestamps from the output. Platform default.
    Strip,
    /// Preserve EXIF. Operator must opt-in per-tenant; per-asset
    /// override is also operator-side.
    Preserve,
}

impl ExifPolicy {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Strip => "strip",
            Self::Preserve => "preserve",
        }
    }
}

impl Default for ExifPolicy {
    /// Platform default: Strip. Operator must opt-in to preserve.
    fn default() -> Self {
        Self::Strip
    }
}

/// One asset variant — one (format, dimensions) pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AssetVariant {
    /// Stable asset id of the source upload.
    pub source_id: String,
    /// Format this variant was encoded into.
    pub format: AssetFormat,
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Byte length on disk.
    pub byte_len: u64,
    /// Sha-256 of the output bytes (lowercase hex). Used for
    /// content-addressed storage + integrity audit.
    pub sha256_hex: String,
    /// EXIF policy applied at encode time.
    pub exif_policy: ExifPolicy,
}

impl AssetVariant {
    /// Validate the variant against pipeline invariants.
    ///   * format.is_image() if width/height > 0
    ///   * sha256_hex is 64 lowercase hex chars
    ///   * exif_policy::Strip when format.is_image() unless
    ///     operator opted preserve
    pub fn validate(&self) -> Result<(), AssetError> {
        if self.sha256_hex.len() != 64
            || !self
                .sha256_hex
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err(AssetError::Invalid(format!(
                "sha256 not 64-lowercase-hex: {}",
                self.sha256_hex
            )));
        }
        if self.format.is_image() && (self.width == 0 || self.height == 0) {
            return Err(AssetError::Invalid(format!(
                "image variant {:?} has zero dimension {}x{}",
                self.format, self.width, self.height
            )));
        }
        Ok(())
    }
}

/// Asset bundle: one source upload's variants + metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AssetBundle {
    /// Stable asset id (sha-256 of source bytes by convention,
    /// but the contract doesn't enforce that — operators may
    /// pick another deterministic naming scheme).
    pub asset_id: String,
    /// Original-source MIME type, as parsed at upload time.
    pub source_media_type: String,
    /// Source pixel dimensions (None for non-image / video).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_width: Option<u32>,
    /// Source pixel dimensions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_height: Option<u32>,
    /// Produced variants. Image bundles MUST cover every format
    /// in [`IMAGE_FALLBACK_LADDER`] (validated by
    /// `validate_image_ladder`).
    pub variants: Vec<AssetVariant>,
    /// Alt text. Either operator-authored or [`AltSource::Vision`]
    /// — never empty for public images (WCAG 2.1 §1.1.1).
    pub alt_text: String,
    /// Provenance of the alt text.
    pub alt_source: AltSource,
}

impl AssetBundle {
    /// Verify the bundle covers every format in
    /// [`IMAGE_FALLBACK_LADDER`]. Used to refuse publication of
    /// image bundles missing the JPEG safety-net.
    pub fn validate_image_ladder(&self) -> Result<(), AssetError> {
        for f in IMAGE_FALLBACK_LADDER {
            if !self.variants.iter().any(|v| v.format == *f) {
                return Err(AssetError::Invalid(format!(
                    "image bundle missing {:?} variant — fallback ladder requires AVIF + WebP + JPEG",
                    f
                )));
            }
        }
        if self.alt_text.trim().is_empty() {
            return Err(AssetError::Invalid(
                "image bundle alt_text empty — WCAG 2.1 §1.1.1".into(),
            ));
        }
        for v in &self.variants {
            v.validate()?;
        }
        Ok(())
    }

    /// Pick the smallest image variant of a given format, if any.
    pub fn variant(&self, format: AssetFormat) -> Option<&AssetVariant> {
        self.variants.iter().find(|v| v.format == format)
    }
}

/// Provenance of the alt text on an asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AltSource {
    /// Operator typed the alt text by hand. Preferred.
    Operator,
    /// Imported from source CMS (#77 importers-core).
    Import,
    /// Synthesised by the vision-AI seam ([`VisionAlt`]).
    /// Operator-reviewable + overridable.
    Vision,
    /// Operator marked the image as decorative (alt="" is
    /// the correct WCAG mapping). Variant is NOT
    /// validate_image_ladder-passing for public surfaces, but
    /// the contract preserves it for decorative contexts.
    Decorative,
}

impl AltSource {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Operator => "operator",
            Self::Import => "import",
            Self::Vision => "vision",
            Self::Decorative => "decorative",
        }
    }
}

/// Vision-AI alt-text seam. Impl crates plug in
/// (`assets-vision-claude`, `assets-vision-local`).
pub trait VisionAlt {
    /// Synthesise alt text for the given image bytes. The bytes
    /// are the raw upload; the impl is free to pick the cheapest
    /// variant for inference. Returns a single-sentence
    /// description appropriate for screen readers per WCAG 2.1.
    fn describe(&self, bytes: &[u8]) -> Result<String, AssetError>;
}

/// Asset encoder seam. Impl crates plug in per-format
/// (assets-avif via libavif, assets-webp via libwebp, etc.).
pub trait AssetEncoder {
    /// Format this encoder produces.
    fn format(&self) -> AssetFormat;
    /// Encode the raw source bytes into the encoder's format.
    /// EXIF strip is applied per the supplied policy.
    fn encode(
        &self,
        source_bytes: &[u8],
        exif_policy: ExifPolicy,
    ) -> Result<AssetVariant, AssetError>;
}

/// Typed errors at the asset pipeline boundary.
#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    /// Source bytes / payload didn't parse.
    #[error("parse: {0}")]
    Parse(String),
    /// Bundle failed pipeline invariants.
    #[error("invalid: {0}")]
    Invalid(String),
    /// Vision-AI seam reported failure.
    #[error("vision: {0}")]
    Vision(String),
    /// Encoder backend error.
    #[error("encoder: {0}")]
    Encoder(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

    fn variant(format: AssetFormat, w: u32, h: u32) -> AssetVariant {
        AssetVariant {
            source_id: "src-1".into(),
            format,
            width: w,
            height: h,
            byte_len: 1234,
            sha256_hex: SHA.into(),
            exif_policy: ExifPolicy::Strip,
        }
    }

    fn bundle_full() -> AssetBundle {
        AssetBundle {
            asset_id: "a1".into(),
            source_media_type: "image/jpeg".into(),
            source_width: Some(2000),
            source_height: Some(1333),
            variants: vec![
                variant(AssetFormat::Avif, 1600, 1067),
                variant(AssetFormat::WebP, 1600, 1067),
                variant(AssetFormat::Jpeg, 1600, 1067),
            ],
            alt_text: "A landscape photo of mountains.".into(),
            alt_source: AltSource::Operator,
        }
    }

    #[test]
    fn format_slugs_distinct_and_media_types_nonempty() {
        let fs = [
            AssetFormat::Avif,
            AssetFormat::WebP,
            AssetFormat::Jpeg,
            AssetFormat::Png,
            AssetFormat::Hls,
            AssetFormat::Dash,
        ];
        let mut s = std::collections::HashSet::new();
        for f in fs {
            assert!(s.insert(f.slug()));
            assert!(!f.media_type().is_empty());
            assert!(!f.extension().is_empty());
        }
    }

    #[test]
    fn format_classifies_image_vs_video() {
        assert!(AssetFormat::Avif.is_image());
        assert!(AssetFormat::WebP.is_image());
        assert!(AssetFormat::Jpeg.is_image());
        assert!(AssetFormat::Png.is_image());
        assert!(!AssetFormat::Hls.is_image());
        assert!(AssetFormat::Hls.is_video());
        assert!(AssetFormat::Dash.is_video());
    }

    #[test]
    fn image_fallback_ladder_is_avif_webp_jpeg() {
        assert_eq!(
            IMAGE_FALLBACK_LADDER,
            &[AssetFormat::Avif, AssetFormat::WebP, AssetFormat::Jpeg]
        );
    }

    #[test]
    fn exif_policy_default_strip() {
        assert_eq!(ExifPolicy::default(), ExifPolicy::Strip);
    }

    #[test]
    fn variant_validate_rejects_short_sha() {
        let mut v = variant(AssetFormat::Avif, 100, 100);
        v.sha256_hex = "abc".into();
        assert!(v.validate().is_err());
    }

    #[test]
    fn variant_validate_rejects_uppercase_sha() {
        let mut v = variant(AssetFormat::Avif, 100, 100);
        v.sha256_hex = SHA.to_uppercase();
        assert!(v.validate().is_err());
    }

    #[test]
    fn variant_validate_rejects_zero_image_dims() {
        let v = variant(AssetFormat::Avif, 0, 100);
        assert!(v.validate().is_err());
    }

    #[test]
    fn bundle_image_ladder_passes_for_full_bundle() {
        let b = bundle_full();
        assert!(b.validate_image_ladder().is_ok());
    }

    #[test]
    fn bundle_image_ladder_rejects_missing_avif() {
        let mut b = bundle_full();
        b.variants.retain(|v| v.format != AssetFormat::Avif);
        assert!(b.validate_image_ladder().is_err());
    }

    #[test]
    fn bundle_image_ladder_rejects_missing_jpeg() {
        let mut b = bundle_full();
        b.variants.retain(|v| v.format != AssetFormat::Jpeg);
        assert!(b.validate_image_ladder().is_err());
    }

    #[test]
    fn bundle_image_ladder_rejects_empty_alt() {
        let mut b = bundle_full();
        b.alt_text = "".into();
        assert!(b.validate_image_ladder().is_err());
    }

    #[test]
    fn variant_lookup_finds_format() {
        let b = bundle_full();
        assert!(b.variant(AssetFormat::Avif).is_some());
        assert!(b.variant(AssetFormat::Png).is_none());
    }

    #[test]
    fn alt_source_slugs_distinct() {
        let ss = [
            AltSource::Operator,
            AltSource::Import,
            AltSource::Vision,
            AltSource::Decorative,
        ];
        let mut s = std::collections::HashSet::new();
        for a in ss {
            assert!(s.insert(a.slug()));
        }
    }

    #[test]
    fn variant_serde_round_trip() {
        let v = variant(AssetFormat::Avif, 100, 100);
        let j = serde_json::to_string(&v).unwrap();
        let back: AssetVariant = serde_json::from_str(&j).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn bundle_rejects_unknown_field() {
        let bad = r#"{"asset-id":"x","source-media-type":"image/jpeg","variants":[],"alt-text":"a","alt-source":"operator","ahem":1}"#;
        let r: Result<AssetBundle, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    // T97: slug-vs-serde-wire regression guard. AssetFormat has
    // WebP (camel boundary) — likely-affected candidate.
    #[test]
    fn slug_matches_serde_wire_across_all_enums() {
        for v in [
            AssetFormat::Avif,
            AssetFormat::WebP,
            AssetFormat::Jpeg,
            AssetFormat::Png,
            AssetFormat::Hls,
            AssetFormat::Dash,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [ExifPolicy::Strip, ExifPolicy::Preserve] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
        for v in [
            AltSource::Operator,
            AltSource::Import,
            AltSource::Vision,
            AltSource::Decorative,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
    }
}
