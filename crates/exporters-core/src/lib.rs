//! `exporters-core` — typed export contract.
//!
//! Per `PLATFORM_ROADMAP.md` §12 + the "no walled garden" axis of
//! `super_society_tech_stack`, every tenant can leave
//! at any time with a portable, machine-readable bundle of
//! everything they've authored. This crate defines the
//! cross-format contract; per-format renderers (Markdown w/ YAML
//! frontmatter, JSON, JSON-LD per schema.org Article, portable
//! tarball, ActivityStreams 2.0) plug in via [`Exporter`].
//!
//! ### Why typed
//!
//! Open standards stay open by being typed at the boundary.
//! Markdown front-matter that drifts ("date" vs "published" vs
//! "publishedAt") makes the export "portable" in name only; every
//! consuming tool needs custom mapping. Enforcing a closed
//! [`ExportFormat`] + a typed [`ExportBundle`] + a typed
//! frontmatter projection makes the export bytes-identical
//! across operators.
//!
//! ### Builtin renderers
//!
//! This crate ships the pure-Rust no-dep renderers:
//!   * [`render_markdown`] — Markdown body + YAML frontmatter
//!   * [`render_json`] — canonical JSON document
//!
//! Heavier renderers (tarball with assets, JSON-LD with
//! schema.org context fetch) land in downstream crates and
//! implement [`Exporter`].

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use importers_core::{Block, CmsSection};
use serde::{Deserialize, Serialize};

/// Closed enum of supported export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExportFormat {
    /// Markdown body with YAML frontmatter (CommonMark + YAML 1.2).
    /// Compatible with Jekyll / Hugo / 11ty / Astro / Zola / Eleventy.
    MarkdownYamlFrontmatter,
    /// Canonical JSON of the [`CmsSection`] shape.
    Json,
    /// JSON-LD using schema.org Article context. RFC 7159 +
    /// W3C JSON-LD 1.1.
    JsonLdSchemaOrg,
    /// Portable tarball: site.json + assets/ + sections/*.md.
    /// Reproducible (deterministic ordering + epoch=0
    /// timestamps).
    PortableTarball,
    /// ActivityStreams 2.0 (RFC 7265) — for federation handoff
    /// to ActivityPub / IndieWeb consumers per #79.
    #[serde(rename = "activitystreams-2")]
    ActivityStreams2,
}

impl ExportFormat {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::MarkdownYamlFrontmatter => "markdown-yaml-frontmatter",
            Self::Json => "json",
            Self::JsonLdSchemaOrg => "json-ld-schema-org",
            Self::PortableTarball => "portable-tarball",
            Self::ActivityStreams2 => "activitystreams-2",
        }
    }

    /// IANA media type — what an HTTP server would emit as
    /// `Content-Type` for this format.
    pub fn media_type(&self) -> &'static str {
        match self {
            Self::MarkdownYamlFrontmatter => "text/markdown",
            Self::Json => "application/json",
            Self::JsonLdSchemaOrg => "application/ld+json",
            Self::PortableTarball => "application/x-tar",
            Self::ActivityStreams2 => "application/activity+json",
        }
    }

    /// Conventional filename extension (without leading dot).
    pub fn extension(&self) -> &'static str {
        match self {
            Self::MarkdownYamlFrontmatter => "md",
            Self::Json => "json",
            Self::JsonLdSchemaOrg => "jsonld",
            Self::PortableTarball => "tar",
            Self::ActivityStreams2 => "json",
        }
    }
}

/// Typed frontmatter projection used by the Markdown renderer.
/// Field names match Jekyll/Hugo conventions (`title`, `slug`,
/// `date`, `updated`, `author`, `tags`) so the output drops
/// straight into mainstream static-site generators.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct MarkdownFrontmatter {
    /// Page title.
    pub title: String,
    /// Slug.
    pub slug: String,
    /// Optional published date (RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<time::OffsetDateTime>,
    /// Optional last-updated (RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<time::OffsetDateTime>,
    /// Optional author handle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Tag list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Optional canonical URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical: Option<String>,
}

impl MarkdownFrontmatter {
    /// Project a [`CmsSection`] into its frontmatter.
    pub fn from_section(s: &CmsSection) -> Self {
        Self {
            title: s.title.clone(),
            slug: s.slug.clone(),
            date: s.published_at,
            updated: s.updated_at,
            author: s.author.clone(),
            tags: s.tags.clone(),
            canonical: s.canonical_path.clone(),
        }
    }
}

/// One exported artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ExportArtifact {
    /// Stable filename, relative to the export root.
    pub path: String,
    /// IANA media type (mirrors `format.media_type()`).
    pub media_type: String,
    /// Format that produced the artifact.
    pub format: ExportFormat,
    /// Output bytes.
    pub bytes: Vec<u8>,
}

/// Aggregate export bundle. One CmsSection may produce >1
/// artifact (e.g. body Markdown + assets/ images for a tarball).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ExportBundle {
    /// Source section id.
    pub source_id: String,
    /// Format used.
    pub format: ExportFormat,
    /// Output artifacts.
    pub artifacts: Vec<ExportArtifact>,
}

/// Typed errors at the export boundary.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// Source section failed its own canonical validation.
    #[error("invalid source section: {0}")]
    InvalidSource(String),
    /// Renderer-specific failure.
    #[error("render: {0}")]
    Render(String),
    /// IO / backend failure.
    #[error("backend: {0}")]
    Backend(String),
}

/// Format-specific exporter plug-in. Impl crates land per format
/// (`exporters-tarball`, `exporters-jsonld`).
pub trait Exporter {
    /// Format this exporter produces.
    fn format(&self) -> ExportFormat;
    /// Render one [`CmsSection`] into an [`ExportBundle`].
    fn export(&self, section: &CmsSection) -> Result<ExportBundle, ExportError>;
}

/// Built-in Markdown + YAML-frontmatter renderer. No deps; pure
/// CommonMark + minimal YAML. Other formats land in dedicated
/// impl crates.
pub fn render_markdown(section: &CmsSection) -> Result<String, ExportError> {
    section
        .validate()
        .map_err(|e| ExportError::InvalidSource(e.to_string()))?;
    let fm = MarkdownFrontmatter::from_section(section);
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&yaml_emit_string("title", &fm.title));
    out.push_str(&yaml_emit_string("slug", &fm.slug));
    if let Some(d) = fm.date {
        out.push_str(&format!("date: {}\n", rfc3339(d)));
    }
    if let Some(u) = fm.updated {
        out.push_str(&format!("updated: {}\n", rfc3339(u)));
    }
    if let Some(a) = &fm.author {
        out.push_str(&yaml_emit_string("author", a));
    }
    if !fm.tags.is_empty() {
        out.push_str("tags:\n");
        for t in &fm.tags {
            out.push_str(&format!("  - {}\n", yaml_scalar(t)));
        }
    }
    if let Some(c) = &fm.canonical {
        out.push_str(&yaml_emit_string("canonical", c));
    }
    out.push_str("---\n\n");
    for b in &section.blocks {
        out.push_str(&render_block_markdown(b));
        out.push('\n');
    }
    Ok(out)
}

/// Built-in canonical-JSON renderer.
pub fn render_json(section: &CmsSection) -> Result<Vec<u8>, ExportError> {
    section
        .validate()
        .map_err(|e| ExportError::InvalidSource(e.to_string()))?;
    serde_json::to_vec_pretty(section).map_err(|e| ExportError::Render(e.to_string()))
}

/// Built-in JSON-LD (schema.org Article) renderer.
pub fn render_json_ld(section: &CmsSection) -> Result<Vec<u8>, ExportError> {
    section
        .validate()
        .map_err(|e| ExportError::InvalidSource(e.to_string()))?;
    let body = section
        .blocks
        .iter()
        .map(render_block_markdown)
        .collect::<Vec<_>>()
        .join("\n");
    let mut obj = serde_json::Map::new();
    obj.insert(
        "@context".into(),
        serde_json::Value::String("https://schema.org".into()),
    );
    obj.insert("@type".into(), serde_json::Value::String("Article".into()));
    obj.insert(
        "headline".into(),
        serde_json::Value::String(section.title.clone()),
    );
    obj.insert("articleBody".into(), serde_json::Value::String(body));
    if let Some(p) = section.published_at {
        obj.insert(
            "datePublished".into(),
            serde_json::Value::String(rfc3339(p)),
        );
    }
    if let Some(u) = section.updated_at {
        obj.insert("dateModified".into(), serde_json::Value::String(rfc3339(u)));
    }
    if let Some(a) = &section.author {
        let mut author = serde_json::Map::new();
        author.insert("@type".into(), serde_json::Value::String("Person".into()));
        author.insert("name".into(), serde_json::Value::String(a.clone()));
        obj.insert("author".into(), serde_json::Value::Object(author));
    }
    if !section.tags.is_empty() {
        obj.insert(
            "keywords".into(),
            serde_json::Value::Array(
                section
                    .tags
                    .iter()
                    .map(|t| serde_json::Value::String(t.clone()))
                    .collect(),
            ),
        );
    }
    if let Some(c) = &section.canonical_path {
        obj.insert(
            "mainEntityOfPage".into(),
            serde_json::Value::String(c.clone()),
        );
    }
    serde_json::to_vec_pretty(&serde_json::Value::Object(obj))
        .map_err(|e| ExportError::Render(e.to_string()))
}

fn render_block_markdown(b: &Block) -> String {
    match b {
        Block::Paragraph { text } => format!("{}\n", text),
        Block::Heading { level, text } => {
            let lvl = (*level).clamp(1, 6) as usize;
            format!("{} {}\n", "#".repeat(lvl), text)
        }
        Block::Image {
            asset_ref,
            alt,
            caption,
        } => match caption {
            Some(c) => format!("![{}]({})\n\n*{}*\n", alt, asset_ref, c),
            None => format!("![{}]({})\n", alt, asset_ref),
        },
        Block::Code { text, language } => {
            let lang = language.as_deref().unwrap_or("");
            format!("```{}\n{}\n```\n", lang, text)
        }
        Block::Quote { text, attribution } => match attribution {
            Some(a) => format!("> {}\n>\n> — {}\n", text, a),
            None => format!("> {}\n", text),
        },
        Block::Embed { url, embed_kind } => format!("<!-- embed: {} -->\n{}\n", embed_kind, url),
        Block::List { ordered, items } => {
            let mut s = String::new();
            for (i, it) in items.iter().enumerate() {
                if *ordered {
                    s.push_str(&format!("{}. {}\n", i + 1, it));
                } else {
                    s.push_str(&format!("- {}\n", it));
                }
            }
            s
        }
        Block::Divider => "---\n".to_string(),
        Block::Hero {
            title, subtitle, ..
        } => match subtitle {
            Some(sub) => format!("# {}\n\n*{}*\n", title, sub),
            None => format!("# {}\n", title),
        },
        Block::Custom { source_kind, .. } => {
            format!(
                "<!-- custom block dropped: source_kind={} -->\n",
                source_kind
            )
        }
    }
}

fn yaml_emit_string(k: &str, v: &str) -> String {
    format!("{}: {}\n", k, yaml_scalar(v))
}

/// Minimal YAML 1.2 plain-scalar emitter. Quotes any string with
/// `:`, `#`, `'`, `"`, leading/trailing whitespace, or starting
/// with YAML-reserved characters.
fn yaml_scalar(s: &str) -> String {
    let needs_quote = s.is_empty()
        || s.starts_with(' ')
        || s.ends_with(' ')
        || s.chars()
            .any(|c| matches!(c, ':' | '#' | '\'' | '"' | '\n' | '\r' | '\t'))
        || s.starts_with(|c: char| {
            matches!(
                c,
                '-' | '?'
                    | '!'
                    | '&'
                    | '*'
                    | '@'
                    | '`'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | ','
                    | '>'
                    | '|'
                    | '%'
            )
        });
    if needs_quote {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{}\"", escaped)
    } else {
        s.to_string()
    }
}

fn rfc3339(t: time::OffsetDateTime) -> String {
    t.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use importers_core::{Block, ImporterKind};
    use time::macros::datetime;

    fn sample() -> CmsSection {
        CmsSection {
            id: "id-1".into(),
            source: ImporterKind::WordPress,
            source_id: Some("post-42".into()),
            slug: "hello-world".into(),
            title: "Hello: World".into(),
            canonical_path: Some("/hello-world".into()),
            published_at: Some(datetime!(2026-05-18 12:00:00 UTC)),
            updated_at: None,
            author: Some("alice".into()),
            tags: vec!["greeting".into(), "intro".into()],
            blocks: vec![
                Block::Hero {
                    title: "Welcome".into(),
                    subtitle: Some("Lede line.".into()),
                    asset_ref: None,
                },
                Block::Paragraph {
                    text: "Body paragraph.".into(),
                },
                Block::Heading {
                    level: 2,
                    text: "Section".into(),
                },
                Block::Code {
                    text: "fn main() {}".into(),
                    language: Some("rust".into()),
                },
                Block::List {
                    ordered: false,
                    items: vec!["a".into(), "b".into()],
                },
                Block::Image {
                    asset_ref: "/img.png".into(),
                    alt: "an image".into(),
                    caption: Some("a caption".into()),
                },
                Block::Quote {
                    text: "to be".into(),
                    attribution: Some("Hamlet".into()),
                },
                Block::Divider,
                Block::Embed {
                    url: "https://youtube.com/x".into(),
                    embed_kind: "youtube".into(),
                },
            ],
        }
    }

    #[test]
    fn format_slugs_distinct() {
        let fs = [
            ExportFormat::MarkdownYamlFrontmatter,
            ExportFormat::Json,
            ExportFormat::JsonLdSchemaOrg,
            ExportFormat::PortableTarball,
            ExportFormat::ActivityStreams2,
        ];
        let mut s = std::collections::HashSet::new();
        for f in fs {
            assert!(s.insert(f.slug()));
            assert!(!f.media_type().is_empty());
            assert!(!f.extension().is_empty());
        }
    }

    #[test]
    fn format_media_types_are_iana() {
        assert_eq!(
            ExportFormat::MarkdownYamlFrontmatter.media_type(),
            "text/markdown"
        );
        assert_eq!(ExportFormat::Json.media_type(), "application/json");
        assert_eq!(
            ExportFormat::JsonLdSchemaOrg.media_type(),
            "application/ld+json"
        );
        assert_eq!(
            ExportFormat::PortableTarball.media_type(),
            "application/x-tar"
        );
        assert_eq!(
            ExportFormat::ActivityStreams2.media_type(),
            "application/activity+json"
        );
    }

    #[test]
    fn markdown_renders_frontmatter_then_body() {
        let s = sample();
        let md = render_markdown(&s).unwrap();
        assert!(md.starts_with("---\n"));
        // Frontmatter keys.
        assert!(md.contains("title: \"Hello: World\"")); // colon forces quoting
        assert!(md.contains("slug: hello-world"));
        assert!(md.contains("date: 2026-05-18T12:00:00Z"));
        assert!(md.contains("author: alice"));
        assert!(md.contains("tags:\n  - greeting\n  - intro"));
        assert!(md.contains("canonical: /hello-world"));
        // Closing fence.
        assert!(md.contains("\n---\n\n"));
        // Body renders each block.
        assert!(md.contains("# Welcome"));
        assert!(md.contains("*Lede line.*"));
        assert!(md.contains("Body paragraph."));
        assert!(md.contains("## Section"));
        assert!(md.contains("```rust\nfn main() {}\n```"));
        assert!(md.contains("- a\n- b"));
        assert!(md.contains("![an image](/img.png)"));
        assert!(md.contains("> to be"));
        assert!(md.contains("> — Hamlet"));
        assert!(md.contains("<!-- embed: youtube -->"));
    }

    #[test]
    fn markdown_refuses_invalid_section() {
        let mut s = sample();
        s.slug = "Not Kebab".into();
        let r = render_markdown(&s);
        assert!(matches!(r, Err(ExportError::InvalidSource(_))));
    }

    #[test]
    fn json_renders_valid_canonical_section() {
        let s = sample();
        let bytes = render_json(&s).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["slug"], "hello-world");
        assert_eq!(v["title"], "Hello: World");
    }

    #[test]
    fn json_ld_uses_schema_org_context() {
        let s = sample();
        let bytes = render_json_ld(&s).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["@context"], "https://schema.org");
        assert_eq!(v["@type"], "Article");
        assert_eq!(v["headline"], "Hello: World");
        assert_eq!(v["author"]["@type"], "Person");
        assert_eq!(v["author"]["name"], "alice");
        assert_eq!(v["keywords"][0], "greeting");
        assert_eq!(v["mainEntityOfPage"], "/hello-world");
        assert!(v["articleBody"]
            .as_str()
            .unwrap()
            .contains("Body paragraph."));
    }

    #[test]
    fn frontmatter_project_from_section() {
        let s = sample();
        let fm = MarkdownFrontmatter::from_section(&s);
        assert_eq!(fm.title, s.title);
        assert_eq!(fm.slug, s.slug);
        assert_eq!(fm.tags, s.tags);
        assert_eq!(fm.canonical, s.canonical_path);
        assert_eq!(fm.author, s.author);
        assert_eq!(fm.date, s.published_at);
    }

    #[test]
    fn yaml_scalar_quotes_when_needed() {
        assert_eq!(yaml_scalar("alice"), "alice");
        assert_eq!(yaml_scalar("Hello: World"), "\"Hello: World\"");
        assert_eq!(yaml_scalar(""), "\"\"");
        assert_eq!(yaml_scalar("- dash-leader"), "\"- dash-leader\"");
        assert_eq!(yaml_scalar("trailing "), "\"trailing \"");
        assert_eq!(yaml_scalar("a\"b"), "\"a\\\"b\"");
    }

    #[test]
    fn export_bundle_serde_round_trips() {
        let bundle = ExportBundle {
            source_id: "s1".into(),
            format: ExportFormat::Json,
            artifacts: vec![ExportArtifact {
                path: "section.json".into(),
                media_type: "application/json".into(),
                format: ExportFormat::Json,
                bytes: b"{}".to_vec(),
            }],
        };
        let j = serde_json::to_string(&bundle).unwrap();
        let back: ExportBundle = serde_json::from_str(&j).unwrap();
        assert_eq!(bundle, back);
    }

    #[test]
    fn export_artifact_rejects_unknown_field() {
        let bad = r#"{"path":"x","media-type":"y","format":"json","bytes":[],"ahem":1}"#;
        let r: Result<ExportArtifact, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    #[test]
    fn rfc3339_emits_well_known_format() {
        let d = datetime!(2026-05-18 12:00:00 UTC);
        assert_eq!(rfc3339(d), "2026-05-18T12:00:00Z");
    }

    // Regression-guard: serde rename_all="kebab-case" wouldn't
    // hyphenate `ActivityStreams2` correctly (no boundary between
    // `s` and `2`). The per-variant `#[serde(rename)]` makes the
    // wire format match slug(); this test enforces it.
    #[test]
    fn format_serde_wire_matches_slug() {
        for f in [
            ExportFormat::MarkdownYamlFrontmatter,
            ExportFormat::Json,
            ExportFormat::JsonLdSchemaOrg,
            ExportFormat::PortableTarball,
            ExportFormat::ActivityStreams2,
        ] {
            let wire = serde_json::to_string(&f).unwrap();
            let stripped = wire.trim_matches('"');
            assert_eq!(stripped, f.slug(), "wire vs slug for {:?}", f);
        }
    }
}
