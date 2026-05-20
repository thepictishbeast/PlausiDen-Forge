//! `synthesis` — mapped-spec → cms/*.json emission.
//!
//! Task #291 per the reference-matching arc. Where #273 will
//! be the mapping engine that converts extracted reference
//! captures into a structured spec, this module is the BACKEND:
//! takes a [`SiteSpec`] (operator-built or engine-built) and
//! emits one `cms/<slug>.json` per declared page.
//!
//! The synthesis output is plain CmsPage JSON the existing
//! Forge build pipeline consumes — no new render path; the
//! variation-arc gates still apply to synthesized sites.
//!
//! ## Wire shape
//!
//! ```rust,ignore
//! let spec = SiteSpec::new("prosperityclub", "plausiden")
//!     .with_voice("editorial")
//!     .with_mood("editorial")
//!     .with_density("comfortable")
//!     .with_page("index", vec![
//!         SectionSpec::new("hero_editorial")
//!             .with_field("title", "Financial education for ordinary people")
//!             .with_field("kicker", "PROSPERITY CLUB"),
//!         SectionSpec::new("paragraph")
//!             .with_field("body", "..."),
//!     ]);
//! forge_core::synthesis::synthesize(&spec, Path::new("cms"))?;
//! ```
//!
//! ## What synthesis does NOT do
//!
//! * Doesn't fill in real content — placeholders only. The
//!   operator authoring the synthesized site supplies actual
//!   text; the spec carries STRUCTURE (which primitive on
//!   which page) plus content HINTS.
//! * Doesn't bypass variation gates — emitted JSON passes
//!   through every variation arc gate at build time.
//! * Doesn't re-author forge.toml. Operators handle
//!   [site_identity] separately (since identity changes need
//!   atomic-transition discipline per #238).
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `#[non_exhaustive]` on every public type.
//! * No unwrap/expect in non-test code.
//! * Pure functions over JSON serialization — filesystem I/O
//!   bounded to the explicit out_dir argument.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Top-level synthesis input.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SiteSpec {
    /// Site identifier (informational; not serialized to cms/).
    pub site_id: String,
    /// Tenant identifier (informational).
    pub tenant_id: String,
    /// Declared voice tier — `plain` / `casual` / `editorial` /
    /// `professional` / `technical` / `academic`. Carried into
    /// each emitted cms/<slug>.json as a top-level hint.
    pub voice: String,
    /// Declared mood primary — see mood_lock for valid values.
    pub mood: String,
    /// Declared density preference.
    pub density: String,
    /// Per-page section specs.
    pub pages: BTreeMap<String, Vec<SectionSpec>>,
}

impl SiteSpec {
    /// Construct an empty spec for the given site + tenant.
    #[must_use]
    pub fn new(site_id: impl Into<String>, tenant_id: impl Into<String>) -> Self {
        Self {
            site_id: site_id.into(),
            tenant_id: tenant_id.into(),
            voice: String::new(),
            mood: String::new(),
            density: String::new(),
            pages: BTreeMap::new(),
        }
    }

    /// Set the voice tier.
    #[must_use]
    pub fn with_voice(mut self, tier: impl Into<String>) -> Self {
        self.voice = tier.into();
        self
    }

    /// Set the mood primary.
    #[must_use]
    pub fn with_mood(mut self, mood: impl Into<String>) -> Self {
        self.mood = mood.into();
        self
    }

    /// Set the density preference.
    #[must_use]
    pub fn with_density(mut self, density: impl Into<String>) -> Self {
        self.density = density.into();
        self
    }

    /// Add a page with its section sequence.
    #[must_use]
    pub fn with_page(
        mut self,
        slug: impl Into<String>,
        sections: Vec<SectionSpec>,
    ) -> Self {
        self.pages.insert(slug.into(), sections);
        self
    }
}

/// One section's spec — kind + variant hint + ordered field
/// hints.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SectionSpec {
    /// CmsSection kind (e.g. `"hero_editorial"`, `"kv_pair"`).
    pub kind: String,
    /// Optional variant string (e.g. `"background=v1"`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub variant: String,
    /// Per-field hints. Synthesis emits these as the section's
    /// top-level JSON fields.
    pub fields: BTreeMap<String, String>,
}

impl SectionSpec {
    /// Construct a section spec with no field hints.
    #[must_use]
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            variant: String::new(),
            fields: BTreeMap::new(),
        }
    }

    /// Set the variant hint.
    #[must_use]
    pub fn with_variant(mut self, variant: impl Into<String>) -> Self {
        self.variant = variant.into();
        self
    }

    /// Add a field hint. Multiple calls accumulate.
    #[must_use]
    pub fn with_field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }
}

/// Errors synthesize can return.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SynthesisError {
    /// I/O error writing the cms/<slug>.json file.
    #[error("synthesis I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON serialization error.
    #[error("synthesis JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// out_dir didn't exist and couldn't be created.
    #[error("synthesis: could not create output directory {path}: {reason}")]
    OutDir {
        /// Path that failed to be created.
        path: String,
        /// Underlying reason.
        reason: String,
    },
}

/// Emit one `cms/<slug>.json` per page in the spec.
///
/// File shape:
///
/// ```json
/// {
///   "title": "<spec.site_id>/<slug>",
///   "voice_tier": "<spec.voice>",
///   "mood_primary": "<spec.mood>",
///   "density_preference": "<spec.density>",
///   "sections": [ ... ]
/// }
/// ```
///
/// Each section in the array:
///
/// ```json
/// {
///   "kind": "<section.kind>",
///   "variant": "<section.variant>",   // omitted if empty
///   ...field hints...
/// }
/// ```
///
/// Returns the list of paths written.
pub fn synthesize(spec: &SiteSpec, out_dir: &Path) -> Result<Vec<PathBuf>, SynthesisError> {
    if !out_dir.exists() {
        fs::create_dir_all(out_dir).map_err(|e| SynthesisError::OutDir {
            path: out_dir.display().to_string(),
            reason: e.to_string(),
        })?;
    }
    let mut written = Vec::new();
    for (slug, sections) in &spec.pages {
        let path = out_dir.join(format!("{slug}.json"));
        let body = build_page_json(spec, slug, sections)?;
        fs::write(&path, body)?;
        written.push(path);
    }
    Ok(written)
}

fn build_page_json(
    spec: &SiteSpec,
    slug: &str,
    sections: &[SectionSpec],
) -> Result<String, SynthesisError> {
    let mut page = serde_json::Map::new();
    page.insert(
        "title".to_owned(),
        serde_json::Value::String(format!("{}/{slug}", spec.site_id)),
    );
    if !spec.voice.is_empty() {
        page.insert(
            "voice_tier".to_owned(),
            serde_json::Value::String(spec.voice.clone()),
        );
    }
    if !spec.mood.is_empty() {
        page.insert(
            "mood_primary".to_owned(),
            serde_json::Value::String(spec.mood.clone()),
        );
    }
    if !spec.density.is_empty() {
        page.insert(
            "density_preference".to_owned(),
            serde_json::Value::String(spec.density.clone()),
        );
    }

    let mut section_array = Vec::new();
    for sec in sections {
        let mut obj = serde_json::Map::new();
        obj.insert("kind".to_owned(), serde_json::Value::String(sec.kind.clone()));
        if !sec.variant.is_empty() {
            obj.insert(
                "variant".to_owned(),
                serde_json::Value::String(sec.variant.clone()),
            );
        }
        for (k, v) in &sec.fields {
            obj.insert(k.clone(), serde_json::Value::String(v.clone()));
        }
        section_array.push(serde_json::Value::Object(obj));
    }
    page.insert(
        "sections".to_owned(),
        serde_json::Value::Array(section_array),
    );

    Ok(serde_json::to_string_pretty(&page)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_out_dir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "forge-synthesis-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn synthesize_emits_one_file_per_page() {
        let spec = SiteSpec::new("test-site", "test-tenant")
            .with_voice("editorial")
            .with_mood("editorial")
            .with_density("comfortable")
            .with_page(
                "index",
                vec![SectionSpec::new("hero_editorial")
                    .with_field("title", "Hello")],
            )
            .with_page(
                "about",
                vec![SectionSpec::new("heading"), SectionSpec::new("paragraph")],
            );
        let out_dir = temp_out_dir("emit");
        let written = synthesize(&spec, &out_dir).unwrap();
        assert_eq!(written.len(), 2);
        assert!(out_dir.join("index.json").is_file());
        assert!(out_dir.join("about.json").is_file());
        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn synthesize_writes_voice_mood_density_to_each_file() {
        let spec = SiteSpec::new("site", "")
            .with_voice("technical")
            .with_mood("industrial")
            .with_density("dense")
            .with_page("index", vec![SectionSpec::new("paragraph")]);
        let out_dir = temp_out_dir("metadata");
        synthesize(&spec, &out_dir).unwrap();
        let body = fs::read_to_string(out_dir.join("index.json")).unwrap();
        assert!(body.contains("\"voice_tier\": \"technical\""));
        assert!(body.contains("\"mood_primary\": \"industrial\""));
        assert!(body.contains("\"density_preference\": \"dense\""));
        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn synthesize_emits_section_kind_and_variant() {
        let spec = SiteSpec::new("s", "").with_page(
            "i",
            vec![
                SectionSpec::new("hero_editorial").with_variant("background=v3"),
                SectionSpec::new("paragraph"),
            ],
        );
        let out_dir = temp_out_dir("sections");
        synthesize(&spec, &out_dir).unwrap();
        let body = fs::read_to_string(out_dir.join("i.json")).unwrap();
        assert!(body.contains("\"kind\": \"hero_editorial\""));
        assert!(body.contains("\"variant\": \"background=v3\""));
        // paragraph has no variant; should NOT have variant key.
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        let sections = parsed.get("sections").and_then(|v| v.as_array()).unwrap();
        assert_eq!(sections.len(), 2);
        assert!(sections[1].get("variant").is_none());
        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn synthesize_carries_field_hints_into_section_json() {
        let spec = SiteSpec::new("s", "").with_page(
            "i",
            vec![SectionSpec::new("hero_editorial")
                .with_field("title", "Hello")
                .with_field("kicker", "INTRO")],
        );
        let out_dir = temp_out_dir("field-hints");
        synthesize(&spec, &out_dir).unwrap();
        let body = fs::read_to_string(out_dir.join("i.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        let sections = parsed.get("sections").and_then(|v| v.as_array()).unwrap();
        let hero = &sections[0];
        assert_eq!(hero.get("title").and_then(|v| v.as_str()), Some("Hello"));
        assert_eq!(hero.get("kicker").and_then(|v| v.as_str()), Some("INTRO"));
        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn synthesize_creates_missing_output_directory() {
        let out_dir = temp_out_dir("create-dir");
        assert!(!out_dir.exists());
        let spec = SiteSpec::new("s", "")
            .with_page("i", vec![SectionSpec::new("paragraph")]);
        synthesize(&spec, &out_dir).unwrap();
        assert!(out_dir.exists());
        assert!(out_dir.join("i.json").is_file());
        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn synthesize_empty_spec_writes_no_files() {
        let spec = SiteSpec::new("s", "");
        let out_dir = temp_out_dir("empty");
        let written = synthesize(&spec, &out_dir).unwrap();
        assert!(written.is_empty());
        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn synthesize_omits_voice_when_empty() {
        let spec = SiteSpec::new("s", "")
            .with_page("i", vec![SectionSpec::new("paragraph")]);
        let out_dir = temp_out_dir("no-voice");
        synthesize(&spec, &out_dir).unwrap();
        let body = fs::read_to_string(out_dir.join("i.json")).unwrap();
        assert!(!body.contains("voice_tier"));
        assert!(!body.contains("mood_primary"));
        assert!(!body.contains("density_preference"));
        let _ = fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn site_spec_builder_chains() {
        let spec = SiteSpec::new("s", "t")
            .with_voice("editorial")
            .with_mood("editorial")
            .with_density("dense")
            .with_page("index", vec![SectionSpec::new("hero")]);
        assert_eq!(spec.site_id, "s");
        assert_eq!(spec.tenant_id, "t");
        assert_eq!(spec.voice, "editorial");
        assert_eq!(spec.mood, "editorial");
        assert_eq!(spec.density, "dense");
        assert_eq!(spec.pages.len(), 1);
    }

    #[test]
    fn section_spec_builder_chains() {
        let s = SectionSpec::new("hero")
            .with_variant("compact")
            .with_field("title", "X")
            .with_field("kicker", "Y");
        assert_eq!(s.kind, "hero");
        assert_eq!(s.variant, "compact");
        assert_eq!(s.fields.get("title").map(String::as_str), Some("X"));
        assert_eq!(s.fields.get("kicker").map(String::as_str), Some("Y"));
    }
}
