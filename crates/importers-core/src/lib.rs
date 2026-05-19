//! `importers-core` — typed CMS importer contract.
//!
//! Per `PLATFORM_ROADMAP.md` §11, PlausiDen reads content out of
//! every major incumbent CMS so the move-in cost is zero:
//! WordPress export XML, Webflow CMS API, Squarespace export
//! XML, Wix Velo export, Ghost JSON, Contentful Delivery API.
//! Each source has its own native shape; this crate defines the
//! cross-source target — a canonical [`CmsSection`] with a closed
//! [`Block`] taxonomy — plus the [`Importer`] trait every per-source
//! impl crate satisfies.
//!
//! ### Why typed
//!
//! The canonical target stays small + closed: ~10 block kinds
//! covering every layout primitive the source CMSes export.
//! Vendor-specific oddities (Webflow's CSS classes, Squarespace's
//! grid metadata, Wix dynamic-bindings) are dropped at import
//! time rather than carried as untyped JSON. Round-trip fidelity
//! against arbitrary source is explicitly NOT a goal —
//! interoperability and ongoing portability are.
//!
//! ### Out of scope here
//!
//! No network. No HTML parsing. No image download. No vendor
//! API client. Those land in per-source impl crates (e.g.
//! `importers-wordpress`, `importers-contentful`) that consume
//! this trait.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Closed enum of supported import sources. Adding a source is a
/// typed change reviewable in one commit, not a free-form string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImporterKind {
    /// WordPress WXR export (`wordpress.xml`).
    #[serde(rename = "wordpress")]
    WordPress,
    /// Webflow CMS Collection Items API.
    Webflow,
    /// Squarespace site export (`Squarespace-Wordpress-Export.xml`).
    Squarespace,
    /// Wix site export / Velo content.
    Wix,
    /// Ghost content export (`ghost-export.json`).
    Ghost,
    /// Contentful Delivery API JSON.
    Contentful,
}

impl ImporterKind {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::WordPress => "wordpress",
            Self::Webflow => "webflow",
            Self::Squarespace => "squarespace",
            Self::Wix => "wix",
            Self::Ghost => "ghost",
            Self::Contentful => "contentful",
        }
    }
}

/// Closed taxonomy of content blocks the canonical target
/// supports. Every source-CMS block-equivalent maps onto one of
/// these. Sources that have richer concepts (Webflow components,
/// Wix dynamic dataset bindings) flatten into `Block::Custom`
/// with operator-reviewable detail, NOT free-form JSON in the
/// canonical lane.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Block {
    /// Plain paragraph (semantic `<p>`).
    Paragraph {
        /// Paragraph text. HTML inline markup (em/strong/code/a)
        /// is preserved verbatim; block-level markup is rejected
        /// at the importer boundary.
        text: String,
    },
    /// Heading (h1–h6).
    Heading {
        /// 1..=6, clamped at the importer boundary.
        level: u8,
        /// Heading text.
        text: String,
    },
    /// Image asset.
    Image {
        /// Stable asset reference (operator-defined; usually a
        /// hash or absolute URL).
        asset_ref: String,
        /// Alt text — required for WCAG 2.1 §1.1.1. Importer
        /// MAY synthesise via #80 vision-AI when source omits.
        alt: String,
        /// Optional caption.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        caption: Option<String>,
    },
    /// Code block.
    Code {
        /// Code text.
        text: String,
        /// Optional language hint (kebab-case slug, e.g.
        /// "rust", "ts", "python").
        #[serde(default, skip_serializing_if = "Option::is_none")]
        language: Option<String>,
    },
    /// Blockquote.
    Quote {
        /// Quoted text.
        text: String,
        /// Optional source attribution.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attribution: Option<String>,
    },
    /// External embed (YouTube, tweet, etc.). Stored as a URL +
    /// kind hint; the renderer decides how to inflate.
    Embed {
        /// URL of the embedded resource.
        url: String,
        /// Kebab-case kind hint ("youtube", "tweet", "vimeo",
        /// "codepen", etc.). Named `embed_kind` (not `kind`)
        /// because the enum-level `#[serde(tag = "kind")]`
        /// already claims that JSON field for the discriminant.
        embed_kind: String,
    },
    /// Bulleted or numbered list.
    List {
        /// Ordered (numbered) vs unordered (bulleted).
        ordered: bool,
        /// Top-level items (flat — nested lists land as text
        /// inline within an item per CommonMark).
        items: Vec<String>,
    },
    /// Horizontal rule / section break.
    Divider,
    /// Hero / page header (title + subtitle + optional asset).
    /// Importers MUST emit at most one Hero per CmsSection.
    Hero {
        /// Hero title.
        title: String,
        /// Optional subtitle / lede.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subtitle: Option<String>,
        /// Optional background asset reference.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        asset_ref: Option<String>,
    },
    /// Vendor-specific content the importer didn't have a canonical
    /// home for. Operator-reviewed at import time; downstream
    /// renderers may skip.
    Custom {
        /// Kebab-case slug for the original vendor block type
        /// (e.g. "webflow-component", "wix-dataset").
        source_kind: String,
        /// Free-form vendor payload, retained for operator review
        /// but NOT interpreted by the canonical pipeline.
        payload: serde_json::Value,
    },
}

impl Block {
    /// Stable kebab-case discriminant slug.
    pub fn kind_slug(&self) -> &'static str {
        match self {
            Self::Paragraph { .. } => "paragraph",
            Self::Heading { .. } => "heading",
            Self::Image { .. } => "image",
            Self::Code { .. } => "code",
            Self::Quote { .. } => "quote",
            Self::Embed { .. } => "embed",
            Self::List { .. } => "list",
            Self::Divider => "divider",
            Self::Hero { .. } => "hero",
            Self::Custom { .. } => "custom",
        }
    }
}

/// A single canonical content section produced by an importer.
/// One source-CMS page / post → one [`CmsSection`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CmsSection {
    /// Importer-assigned stable id (deterministic per source
    /// item).
    pub id: String,
    /// Source-side kind that produced this section.
    pub source: ImporterKind,
    /// Source-side identifier (post-id, item-id, entry-id —
    /// opaque, only kept for traceability).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// Stable slug (kebab-case) — derived deterministically by
    /// the importer from the source path / title.
    pub slug: String,
    /// Title.
    pub title: String,
    /// Optional canonical URL the operator wants the new section
    /// to live at after import. Defaults to `/{slug}` downstream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_path: Option<String>,
    /// Source publish-time, if known. UTC. RFC 3339 wire format.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "time::serde::rfc3339::option"
    )]
    pub published_at: Option<time::OffsetDateTime>,
    /// Source last-update time. UTC. RFC 3339 wire format.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "time::serde::rfc3339::option"
    )]
    pub updated_at: Option<time::OffsetDateTime>,
    /// Operator-side author handle (opaque).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Tag list (kebab-case slugs).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Ordered block list — the page content.
    pub blocks: Vec<Block>,
}

impl CmsSection {
    /// Count blocks matching a discriminant slug. Useful for
    /// asserting hero-uniqueness etc. at the importer boundary.
    pub fn count_blocks_of_kind(&self, kind_slug: &str) -> usize {
        self.blocks
            .iter()
            .filter(|b| b.kind_slug() == kind_slug)
            .count()
    }

    /// Validate the section against the canonical-target
    /// invariants. Returns Ok(()) when:
    ///   * slug is non-empty kebab-case
    ///   * title is non-empty
    ///   * at most one Hero block
    ///   * every Heading has level 1..=6
    ///   * every Image has non-empty alt (a11y; WCAG 2.1 §1.1.1)
    pub fn validate(&self) -> Result<(), ImportError> {
        if self.title.trim().is_empty() {
            return Err(ImportError::Invalid("title empty".into()));
        }
        if self.slug.is_empty() || !is_kebab(&self.slug) {
            return Err(ImportError::Invalid(format!(
                "slug not kebab: {}",
                self.slug
            )));
        }
        if self.count_blocks_of_kind("hero") > 1 {
            return Err(ImportError::Invalid("more than one hero".into()));
        }
        for b in &self.blocks {
            match b {
                Block::Heading { level, .. } if !(1..=6).contains(level) => {
                    return Err(ImportError::Invalid(format!(
                        "heading level {level} out of 1..=6"
                    )));
                }
                Block::Image { alt, .. } if alt.trim().is_empty() => {
                    return Err(ImportError::Invalid(
                        "image missing alt text (WCAG 2.1 §1.1.1)".into(),
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Lightweight kebab-case check: lower [a-z0-9-]+, no leading /
/// trailing / consecutive `-`.
fn is_kebab(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut prev_dash = false;
    for (i, c) in s.chars().enumerate() {
        let last = i + 1 == s.len();
        match c {
            'a'..='z' | '0'..='9' => {
                prev_dash = false;
            }
            '-' => {
                if i == 0 || last || prev_dash {
                    return false;
                }
                prev_dash = true;
            }
            _ => return false,
        }
    }
    true
}

/// Typed errors at the importer boundary.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// Source bytes / payload didn't parse.
    #[error("parse: {0}")]
    Parse(String),
    /// Source parsed but failed canonical-target invariants.
    #[error("invalid: {0}")]
    Invalid(String),
    /// IO / network / backend error from the impl crate.
    #[error("backend: {0}")]
    Backend(String),
}

/// Source-specific importer plug-in. Impl crates land per source
/// (`importers-wordpress`, `importers-webflow`, etc.).
pub trait Importer {
    /// Source kind this importer handles.
    fn kind(&self) -> ImporterKind;
    /// Import a single source payload into one or more
    /// [`CmsSection`]s. The bytes are interpreted per-source —
    /// WXR XML / Webflow JSON / Contentful JSON / etc.
    fn import(&self, bytes: &[u8]) -> Result<Vec<CmsSection>, ImportError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn section(blocks: Vec<Block>) -> CmsSection {
        CmsSection {
            id: "id-1".into(),
            source: ImporterKind::WordPress,
            source_id: Some("post-42".into()),
            slug: "hello-world".into(),
            title: "Hello World".into(),
            canonical_path: None,
            published_at: Some(datetime!(2026-05-18 12:00:00 UTC)),
            updated_at: None,
            author: Some("alice".into()),
            tags: vec!["test".into()],
            blocks,
        }
    }

    #[test]
    fn importer_kind_slugs_distinct() {
        let ks = [
            ImporterKind::WordPress,
            ImporterKind::Webflow,
            ImporterKind::Squarespace,
            ImporterKind::Wix,
            ImporterKind::Ghost,
            ImporterKind::Contentful,
        ];
        let mut s = std::collections::HashSet::new();
        for k in ks {
            assert!(s.insert(k.slug()));
        }
    }

    #[test]
    fn block_kind_slugs_distinct() {
        let bs = vec![
            Block::Paragraph { text: "".into() },
            Block::Heading {
                level: 1,
                text: "".into(),
            },
            Block::Image {
                asset_ref: "".into(),
                alt: "x".into(),
                caption: None,
            },
            Block::Code {
                text: "".into(),
                language: None,
            },
            Block::Quote {
                text: "".into(),
                attribution: None,
            },
            Block::Embed {
                url: "".into(),
                embed_kind: "".into(),
            },
            Block::List {
                ordered: false,
                items: vec![],
            },
            Block::Divider,
            Block::Hero {
                title: "".into(),
                subtitle: None,
                asset_ref: None,
            },
            Block::Custom {
                source_kind: "".into(),
                payload: serde_json::json!({}),
            },
        ];
        let mut s = std::collections::HashSet::new();
        for b in bs {
            assert!(s.insert(b.kind_slug()));
        }
    }

    #[test]
    fn cms_section_validates_clean_section() {
        let s = section(vec![
            Block::Hero {
                title: "Hi".into(),
                subtitle: None,
                asset_ref: None,
            },
            Block::Paragraph {
                text: "body".into(),
            },
            Block::Image {
                asset_ref: "a-1".into(),
                alt: "an image".into(),
                caption: None,
            },
        ]);
        assert!(s.validate().is_ok());
    }

    #[test]
    fn cms_section_rejects_empty_title() {
        let mut s = section(vec![]);
        s.title = "".into();
        assert!(s.validate().is_err());
    }

    #[test]
    fn cms_section_rejects_non_kebab_slug() {
        let mut s = section(vec![]);
        s.slug = "Hello World".into();
        assert!(s.validate().is_err());
    }

    #[test]
    fn cms_section_rejects_multiple_heroes() {
        let s = section(vec![
            Block::Hero {
                title: "A".into(),
                subtitle: None,
                asset_ref: None,
            },
            Block::Hero {
                title: "B".into(),
                subtitle: None,
                asset_ref: None,
            },
        ]);
        assert!(s.validate().is_err());
    }

    #[test]
    fn cms_section_rejects_image_without_alt() {
        let s = section(vec![Block::Image {
            asset_ref: "a-1".into(),
            alt: "".into(),
            caption: None,
        }]);
        assert!(s.validate().is_err());
    }

    #[test]
    fn cms_section_rejects_out_of_range_heading_level() {
        let s = section(vec![Block::Heading {
            level: 7,
            text: "x".into(),
        }]);
        assert!(s.validate().is_err());
    }

    #[test]
    fn kebab_accepts_typical_slugs() {
        assert!(is_kebab("hello"));
        assert!(is_kebab("hello-world"));
        assert!(is_kebab("a1-b2-c3"));
    }

    #[test]
    fn kebab_rejects_edge_cases() {
        assert!(!is_kebab(""));
        assert!(!is_kebab("-hello"));
        assert!(!is_kebab("hello-"));
        assert!(!is_kebab("hello--world"));
        assert!(!is_kebab("Hello"));
        assert!(!is_kebab("hello world"));
        assert!(!is_kebab("hello_world"));
    }

    #[test]
    fn cms_section_serde_round_trip() {
        let s = section(vec![
            Block::Paragraph {
                text: "Hello.".into(),
            },
            Block::Code {
                text: "fn main() {}".into(),
                language: Some("rust".into()),
            },
            Block::List {
                ordered: true,
                items: vec!["a".into(), "b".into()],
            },
        ]);
        let j = serde_json::to_string(&s).unwrap();
        let back: CmsSection = serde_json::from_str(&j).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn cms_section_rejects_unknown_field() {
        // ahem isn't a known field, deny_unknown_fields catches it.
        let bad = r#"{"id":"x","source":"wordpress","slug":"a","title":"T","blocks":[],"ahem":1}"#;
        let r: Result<CmsSection, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    #[test]
    fn block_serde_round_trips_each_variant() {
        let bs = vec![
            Block::Paragraph { text: "p".into() },
            Block::Heading {
                level: 2,
                text: "h".into(),
            },
            Block::Image {
                asset_ref: "a".into(),
                alt: "x".into(),
                caption: Some("c".into()),
            },
            Block::Code {
                text: "x".into(),
                language: Some("rust".into()),
            },
            Block::Quote {
                text: "q".into(),
                attribution: Some("a".into()),
            },
            Block::Embed {
                url: "https://example.com".into(),
                embed_kind: "youtube".into(),
            },
            Block::List {
                ordered: true,
                items: vec!["1".into(), "2".into()],
            },
            Block::Divider,
            Block::Hero {
                title: "t".into(),
                subtitle: Some("s".into()),
                asset_ref: Some("a".into()),
            },
            Block::Custom {
                source_kind: "webflow-component".into(),
                payload: serde_json::json!({"id":42}),
            },
        ];
        for b in bs {
            let j = serde_json::to_string(&b).unwrap();
            let back: Block = serde_json::from_str(&j).unwrap();
            assert_eq!(b, back);
        }
    }

    #[test]
    fn count_blocks_of_kind_works() {
        let s = section(vec![
            Block::Paragraph { text: "a".into() },
            Block::Paragraph { text: "b".into() },
            Block::Divider,
        ]);
        assert_eq!(s.count_blocks_of_kind("paragraph"), 2);
        assert_eq!(s.count_blocks_of_kind("divider"), 1);
        assert_eq!(s.count_blocks_of_kind("hero"), 0);
    }

    // T97: slug-vs-serde-wire regression guard.
    // ImporterKind has WordPress (camel boundary) + Squarespace
    // (single word with internal capital but no separable
    // boundary) — likely candidates for divergence.
    #[test]
    fn slug_matches_serde_wire_across_all_enums() {
        for v in [
            ImporterKind::WordPress,
            ImporterKind::Webflow,
            ImporterKind::Squarespace,
            ImporterKind::Wix,
            ImporterKind::Ghost,
            ImporterKind::Contentful,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
    }
}
