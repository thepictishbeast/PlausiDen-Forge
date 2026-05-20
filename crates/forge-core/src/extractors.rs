//! `extractors` — per-axis analyzers for reference-site
//! captures. Consume ComputedStylesDump payloads emitted by
//! Crawler-side capture (#263) and produce structured per-axis
//! results that the mapping engine (#273) composes into a
//! SiteSpec.
//!
//! Each submodule implements one axis from the reference-
//! matching arc (#264-#272). Currently shipped:
//!
//! * `palette` (#264) — color palette extraction
//!
//! Pending submodules: typography (#265), spacing (#266),
//! motion (#267), sections (#268), pattern_library (#269),
//! structural (#270), voice (#271), interactive (#272).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Computed-styles dump emitted by Crawler at
/// [`crate::reference_capture::ReferenceCapture::computed_styles_path`].
///
/// Aggregate shape: outer map keyed by CSS property name,
/// inner map keyed by computed value (normalized), value is
/// occurrence count across all matched DOM elements at the
/// captured viewport.
///
/// Example:
/// ```jsonc
/// {
///   "spec": "v1",
///   "property_values": {
///     "color": { "rgb(0, 0, 0)": 42, "rgb(80, 80, 80)": 17 },
///     "background-color": { "rgb(255, 255, 255)": 88 },
///     "font-family": { "Iowan Old Style, Georgia, serif": 12 }
///   }
/// }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ComputedStylesDump {
    /// Spec version. Bumped on incompatible shape changes.
    pub spec: StylesSpec,
    /// Per-property value distributions.
    pub property_values: BTreeMap<String, BTreeMap<String, u32>>,
}

/// Computed-styles spec version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StylesSpec {
    /// Initial spec.
    #[default]
    V1,
}

impl StylesSpec {
    /// Stable kebab-case slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::V1 => "v1",
        }
    }
}

/// Errors extractors can raise.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ExtractorError {
    /// I/O error reading the dump file.
    #[error("extractor I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON parse error.
    #[error("extractor JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// Spec version skew.
    #[error("computed-styles spec mismatch: expected {expected:?}, got {actual:?}")]
    SpecMismatch {
        /// Expected spec.
        expected: StylesSpec,
        /// Spec carried by the payload.
        actual: StylesSpec,
    },
}

pub mod motion;
pub mod palette;
pub mod sections;
pub mod spacing;
pub mod typography;
