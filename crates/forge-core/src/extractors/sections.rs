//! `sections` — section detection + pattern classification.
//!
//! Task #268 per the reference-matching arc. Reads a
//! [`SectionDump`] (top-level DOM section candidates emitted
//! by the Crawler) and classifies each into a guessed Loom
//! primitive kind via heuristics: tag + bounding-box geometry
//! + text density + child-kind distribution.
//!
//! ## Inputs
//!
//! The Crawler walks the post-render DOM, picks candidate
//! section elements (any `<section>`, `<article>`, or
//! direct-child of `<main>`/`<body>` whose bounding box height
//! exceeds a small floor), and emits one [`CandidateSection`]
//! per candidate. The extractor here is the BACKEND that maps
//! candidate features to substrate-native CmsSection kinds.
//!
//! ## Output
//!
//! Per candidate: a [`PatternClassification`] carrying
//! `guessed_kind` (substrate-native CmsSection variant), a
//! confidence score 0-100, and the feature signature that
//! drove the guess. The mapping engine (#273) uses confidence
//! to decide whether to accept the guess or fall back to a
//! safer kind.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `#[non_exhaustive]` on every public type.
//! * No unwrap/expect in non-test code.
//! * Pure functions; no I/O beyond the explicit JSON path.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::extractors::ExtractorError;

/// Section-dump spec version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SectionsSpec {
    /// Initial spec.
    #[default]
    V1,
}

impl SectionsSpec {
    /// Stable kebab-case slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::V1 => "v1",
        }
    }
}

/// Crawler-emitted section dump for one capture (one viewport).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SectionDump {
    /// Schema version.
    pub spec: SectionsSpec,
    /// Total document height in CSS pixels.
    pub document_height_px: u32,
    /// Candidate sections in document order (top → bottom).
    pub candidates: Vec<CandidateSection>,
}

/// One candidate section element.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CandidateSection {
    /// HTML tag name (`section`, `article`, `header`, `aside`,
    /// `div`, etc.).
    pub tag: String,
    /// Top-of-element Y coordinate in document px.
    pub bounding_box_y_px: u32,
    /// Element height in document px.
    pub height_px: u32,
    /// Element width in document px.
    pub width_px: u32,
    /// Total visible text length in characters (descendants
    /// included).
    pub text_chars: u32,
    /// Count of `<h1>`-`<h6>` descendants (any level).
    pub heading_count: u32,
    /// Count of `<p>` descendants.
    pub paragraph_count: u32,
    /// Count of `<img>` / `<picture>` descendants.
    pub image_count: u32,
    /// Count of `<video>` / `<iframe>` descendants.
    pub video_count: u32,
    /// Count of `<form>` descendants.
    pub form_count: u32,
    /// Count of `<button>` + `<a class~="btn">` descendants.
    pub button_count: u32,
    /// Count of `<code>` / `<pre>` descendants.
    pub code_count: u32,
    /// Count of direct-child `<ul>`/`<ol>`/`<dl>` list elements.
    pub list_count: u32,
    /// Count of `<blockquote>` descendants.
    pub blockquote_count: u32,
    /// Has `<svg>` descendant (icon / illustration).
    pub has_svg: bool,
}

impl PatternClassification {
    /// Construct a PatternClassification. Public constructor
    /// because the struct is `#[non_exhaustive]`.
    #[must_use]
    pub fn new(
        guessed_kind: impl Into<String>,
        confidence: u8,
        feature_signature: impl Into<String>,
    ) -> Self {
        Self {
            guessed_kind: guessed_kind.into(),
            confidence,
            feature_signature: feature_signature.into(),
        }
    }
}

/// One classification result per candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PatternClassification {
    /// Substrate-native CmsSection kind slug
    /// (`"hero_editorial"`, `"paragraph"`, `"pull_quote"`,
    /// `"kv_pair"`, `"image_hero"`, `"code"`, `"form"`,
    /// `"call_to_action"`, `"unknown"`).
    pub guessed_kind: String,
    /// Confidence 0-100.
    pub confidence: u8,
    /// Short label describing the feature that drove the guess.
    pub feature_signature: String,
}

/// Extract section classifications from a JSON dump file.
pub fn extract_from_path(path: &Path) -> Result<Vec<PatternClassification>, ExtractorError> {
    let body = std::fs::read_to_string(path)?;
    let dump: SectionDump = serde_json::from_str(&body)?;
    Ok(classify(&dump))
}

/// Pure classification of every candidate in the dump.
#[must_use]
pub fn classify(dump: &SectionDump) -> Vec<PatternClassification> {
    dump.candidates
        .iter()
        .map(|c| classify_candidate(c, dump.document_height_px))
        .collect()
}

fn classify_candidate(c: &CandidateSection, doc_height: u32) -> PatternClassification {
    // Hero detection: first 1/3 of doc, large height, has
    // heading, low text density.
    let is_above_fold = doc_height > 0 && c.bounding_box_y_px < doc_height / 3;
    let is_tall = c.height_px > 300;
    let text_density = if c.height_px > 0 {
        f64::from(c.text_chars) / f64::from(c.height_px)
    } else {
        0.0
    };

    if is_above_fold && is_tall && c.heading_count >= 1 && text_density < 1.0 {
        if c.image_count >= 1 {
            return PatternClassification {
                guessed_kind: "image_hero".to_owned(),
                confidence: 75,
                feature_signature: "above-fold + tall + heading + image".to_owned(),
            };
        }
        return PatternClassification {
            guessed_kind: "hero_editorial".to_owned(),
            confidence: 78,
            feature_signature: "above-fold + tall + heading + low-density".to_owned(),
        };
    }

    // Form section.
    if c.form_count >= 1 {
        return PatternClassification {
            guessed_kind: "form".to_owned(),
            confidence: 90,
            feature_signature: "contains <form>".to_owned(),
        };
    }

    // Code section.
    if c.code_count >= 1 && c.text_chars > 50 {
        return PatternClassification {
            guessed_kind: "code".to_owned(),
            confidence: 80,
            feature_signature: "contains <code>/<pre>".to_owned(),
        };
    }

    // Pull quote: blockquote-led + short.
    if c.blockquote_count >= 1 && c.text_chars > 0 && c.text_chars < 500 {
        return PatternClassification {
            guessed_kind: "pull_quote".to_owned(),
            confidence: 80,
            feature_signature: "blockquote-led + short text".to_owned(),
        };
    }

    // KV pair / definition-list: list-led with multiple items.
    if c.list_count >= 1 && c.paragraph_count <= 1 && c.heading_count <= 1 {
        return PatternClassification {
            guessed_kind: "kv_pair".to_owned(),
            confidence: 70,
            feature_signature: "list-led, few paragraphs".to_owned(),
        };
    }

    // Image hero / gallery anywhere on page.
    if c.image_count >= 1 && c.text_chars < 200 {
        return PatternClassification {
            guessed_kind: "image_hero".to_owned(),
            confidence: 65,
            feature_signature: "image + minimal text".to_owned(),
        };
    }

    // Heading + dense paragraph: long-form section.
    if c.heading_count >= 1 && c.paragraph_count >= 2 {
        return PatternClassification {
            guessed_kind: "paragraph".to_owned(),
            confidence: 70,
            feature_signature: "heading + multiple paragraphs".to_owned(),
        };
    }

    // Heading-only.
    if c.heading_count >= 1 && c.paragraph_count == 0 && c.text_chars < 200 {
        return PatternClassification {
            guessed_kind: "section_heading".to_owned(),
            confidence: 75,
            feature_signature: "heading-only".to_owned(),
        };
    }

    // CTA: short text + button.
    if c.button_count >= 1 && c.text_chars < 300 {
        return PatternClassification {
            guessed_kind: "call_to_action".to_owned(),
            confidence: 75,
            feature_signature: "button + short text".to_owned(),
        };
    }

    // Plain paragraph: paragraph-only.
    if c.paragraph_count >= 1 && c.heading_count == 0 {
        return PatternClassification {
            guessed_kind: "paragraph".to_owned(),
            confidence: 60,
            feature_signature: "paragraph-only".to_owned(),
        };
    }

    PatternClassification {
        guessed_kind: "unknown".to_owned(),
        confidence: 20,
        feature_signature: "no matching heuristic".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand() -> CandidateSection {
        CandidateSection::default()
    }

    #[test]
    fn classify_above_fold_tall_low_density_as_hero_editorial() {
        let dump = SectionDump {
            spec: SectionsSpec::V1,
            document_height_px: 4000,
            candidates: vec![CandidateSection {
                tag: "section".into(),
                bounding_box_y_px: 100,
                height_px: 600,
                width_px: 1280,
                text_chars: 200,
                heading_count: 1,
                ..cand()
            }],
        };
        let r = classify(&dump);
        assert_eq!(r[0].guessed_kind, "hero_editorial");
    }

    #[test]
    fn classify_above_fold_with_image_as_image_hero() {
        let dump = SectionDump {
            spec: SectionsSpec::V1,
            document_height_px: 4000,
            candidates: vec![CandidateSection {
                bounding_box_y_px: 0,
                height_px: 500,
                text_chars: 100,
                heading_count: 1,
                image_count: 1,
                ..cand()
            }],
        };
        let r = classify(&dump);
        assert_eq!(r[0].guessed_kind, "image_hero");
    }

    #[test]
    fn classify_form_section() {
        let dump = SectionDump {
            spec: SectionsSpec::V1,
            document_height_px: 4000,
            candidates: vec![CandidateSection {
                bounding_box_y_px: 2000,
                height_px: 400,
                text_chars: 300,
                form_count: 1,
                ..cand()
            }],
        };
        let r = classify(&dump);
        assert_eq!(r[0].guessed_kind, "form");
    }

    #[test]
    fn classify_code_section() {
        let dump = SectionDump {
            spec: SectionsSpec::V1,
            document_height_px: 4000,
            candidates: vec![CandidateSection {
                bounding_box_y_px: 1500,
                height_px: 300,
                text_chars: 500,
                code_count: 2,
                ..cand()
            }],
        };
        let r = classify(&dump);
        assert_eq!(r[0].guessed_kind, "code");
    }

    #[test]
    fn classify_blockquote_short_as_pull_quote() {
        let dump = SectionDump {
            spec: SectionsSpec::V1,
            document_height_px: 4000,
            candidates: vec![CandidateSection {
                bounding_box_y_px: 1800,
                height_px: 200,
                text_chars: 250,
                blockquote_count: 1,
                ..cand()
            }],
        };
        let r = classify(&dump);
        assert_eq!(r[0].guessed_kind, "pull_quote");
    }

    #[test]
    fn classify_list_led_as_kv_pair() {
        let dump = SectionDump {
            spec: SectionsSpec::V1,
            document_height_px: 4000,
            candidates: vec![CandidateSection {
                bounding_box_y_px: 1500,
                height_px: 400,
                text_chars: 400,
                heading_count: 1,
                list_count: 1,
                ..cand()
            }],
        };
        let r = classify(&dump);
        assert_eq!(r[0].guessed_kind, "kv_pair");
    }

    #[test]
    fn classify_image_minimal_text_anywhere_as_image_hero() {
        let dump = SectionDump {
            spec: SectionsSpec::V1,
            document_height_px: 4000,
            candidates: vec![CandidateSection {
                bounding_box_y_px: 2500, // below fold
                height_px: 400,
                text_chars: 50,
                image_count: 1,
                ..cand()
            }],
        };
        let r = classify(&dump);
        assert_eq!(r[0].guessed_kind, "image_hero");
    }

    #[test]
    fn classify_heading_plus_paragraphs_as_paragraph() {
        let dump = SectionDump {
            spec: SectionsSpec::V1,
            document_height_px: 4000,
            candidates: vec![CandidateSection {
                bounding_box_y_px: 2000,
                height_px: 600,
                text_chars: 2000,
                heading_count: 1,
                paragraph_count: 4,
                ..cand()
            }],
        };
        let r = classify(&dump);
        assert_eq!(r[0].guessed_kind, "paragraph");
    }

    #[test]
    fn classify_heading_only_short_as_section_heading() {
        let dump = SectionDump {
            spec: SectionsSpec::V1,
            document_height_px: 4000,
            candidates: vec![CandidateSection {
                bounding_box_y_px: 1500,
                height_px: 100,
                text_chars: 50,
                heading_count: 1,
                ..cand()
            }],
        };
        let r = classify(&dump);
        assert_eq!(r[0].guessed_kind, "section_heading");
    }

    #[test]
    fn classify_button_with_short_text_as_cta() {
        let dump = SectionDump {
            spec: SectionsSpec::V1,
            document_height_px: 4000,
            candidates: vec![CandidateSection {
                bounding_box_y_px: 3500,
                height_px: 200,
                text_chars: 150,
                button_count: 2,
                ..cand()
            }],
        };
        let r = classify(&dump);
        assert_eq!(r[0].guessed_kind, "call_to_action");
    }

    #[test]
    fn classify_paragraph_only_as_paragraph() {
        let dump = SectionDump {
            spec: SectionsSpec::V1,
            document_height_px: 4000,
            candidates: vec![CandidateSection {
                bounding_box_y_px: 2000,
                height_px: 300,
                text_chars: 800,
                paragraph_count: 2,
                ..cand()
            }],
        };
        let r = classify(&dump);
        assert_eq!(r[0].guessed_kind, "paragraph");
    }

    #[test]
    fn classify_unknown_when_no_signals() {
        let dump = SectionDump {
            spec: SectionsSpec::V1,
            document_height_px: 4000,
            candidates: vec![cand()],
        };
        let r = classify(&dump);
        assert_eq!(r[0].guessed_kind, "unknown");
        assert!(r[0].confidence <= 30);
    }

    #[test]
    fn extract_from_path_round_trips_dump() {
        let dump = SectionDump {
            spec: SectionsSpec::V1,
            document_height_px: 2000,
            candidates: vec![CandidateSection {
                bounding_box_y_px: 100,
                height_px: 500,
                heading_count: 1,
                text_chars: 100,
                ..cand()
            }],
        };
        let path = std::env::temp_dir().join(format!("forge-sections-{}", std::process::id()));
        std::fs::write(&path, serde_json::to_string(&dump).unwrap()).unwrap();
        let r = extract_from_path(&path).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].guessed_kind, "hero_editorial");
        let _ = std::fs::remove_file(&path);
    }
}
