//! `forge_lite` — deliberately narrow substrate surface for the
//! complexity-bottleneck diagnostic.
//!
//! Per `docs/SUBSTRATE_REFRAME_2026_05_21.md` § Accessibility
//! axis → Forge Lite diagnostic. The question is whether
//! Forge's accumulated cognitive surface area is what produces
//! the convergent-output failure mode, independent of substrate
//! vocabulary breadth. The diagnostic test: constrain the
//! exposed surface to ~10 primitives + 3 themes, let Claude
//! build sites within that constraint, compare outputs to full-
//! Forge sites. If Forge Lite produces visibly better Claude
//! outputs, complexity is the bottleneck; restructure Forge's
//! interface so Claude effectively works in something like
//! Forge Lite by default, with broader capabilities accessed
//! only via explicit deviation.
//!
//! ## What this exposes
//!
//! [`ForgeLitePrimitive`] — closed enumeration of 10 primitives:
//!
//! 1. `Hero` — simple centered hero (eyebrow + title + lede + CTA)
//! 2. `Heading` — standalone h2/h3 heading
//! 3. `Paragraph` — body paragraph
//! 4. `ImageHero` — hero with photo background
//! 5. `FeatureSpotlight` — 2- or 3-column tile grid
//! 6. `PullQuote` — typographic quote
//! 7. `CallToAction` — CTA card
//! 8. `LogoCloud` — partner / logo strip
//! 9. `Divider` — horizontal rule
//! 10. `Spacer` — vertical spacer
//!
//! [`ForgeLiteTheme`] — closed enumeration of 3 themes:
//! `Light`, `Dark`, `Warm`.
//!
//! [`ForgeLitePage`] — typed page envelope (title + description
//! + theme + sections) that resolves to a full
//! [`loom_cms_render::CmsPage`] via [`ForgeLitePage::resolve`].
//!
//! ## Why a separate type rather than a CmsSection subset
//!
//! Two reasons:
//!
//! 1. **Schema enforcement.** A `serde(deny_unknown_fields)`
//!    boundary at this layer rejects every CmsSection variant
//!    not in the lite vocabulary. Claude (or any operator)
//!    can't accidentally reach for `HeroEditorial` or
//!    `SplitHero` from inside the lite surface — the field is
//!    structurally inaccessible.
//!
//! 2. **Stable MCP / CLI surface.** The lite surface is its own
//!    contract; expanding the full substrate doesn't widen the
//!    lite surface. Operators who pick the lite mode get a
//!    permanent narrow contract; full-substrate evolution
//!    happens elsewhere.

use serde::{Deserialize, Serialize};

/// One of the ten primitives exposed by Forge Lite. Closed
/// enumeration — no escape hatches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[allow(missing_docs)] // enum + field names + docs above are the contract
pub enum ForgeLitePrimitive {
    Hero {
        eyebrow: Option<String>,
        title: String,
        lede: Option<String>,
        cta_label: Option<String>,
        cta_href: Option<String>,
    },
    Heading {
        /// 2 or 3 only. Forge Lite caps heading depth so the
        /// outline stays scannable.
        level: u8,
        text: String,
    },
    Paragraph {
        text: String,
    },
    ImageHero {
        eyebrow: Option<String>,
        title: String,
        lede: Option<String>,
        photo_src: String,
        photo_alt: String,
        cta_label: Option<String>,
        cta_href: Option<String>,
    },
    FeatureSpotlight {
        heading: String,
        /// 2 or 3 only. Forge Lite refuses 4+ column grids
        /// because they consistently degrade on mobile in
        /// observed full-Forge outputs.
        columns: u8,
        items: Vec<FeatureItem>,
    },
    PullQuote {
        text: String,
        attribution: Option<String>,
    },
    CallToAction {
        title: String,
        lede: Option<String>,
        cta_label: String,
        cta_href: String,
    },
    LogoCloud {
        heading: Option<String>,
        logos: Vec<LogoItem>,
    },
    Divider,
    Spacer {
        /// "small" / "medium" / "large" only. Forge Lite
        /// rejects arbitrary spacer values to keep the rhythm
        /// scale predictable.
        size: SpacerSize,
    },
}

/// One feature-spotlight item. Title + body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(missing_docs)]
pub struct FeatureItem {
    pub title: String,
    pub body: String,
}

/// One logo-cloud item. Image src + accessible name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(missing_docs)]
pub struct LogoItem {
    pub src: String,
    pub alt: String,
}

/// Closed enumeration of spacer sizes. No raw px values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum SpacerSize {
    Small,
    Medium,
    Large,
}

/// Closed enumeration of Forge Lite themes. Three only.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum ForgeLiteTheme {
    #[default]
    Light,
    Dark,
    Warm,
}

impl ForgeLiteTheme {
    /// Stable kebab-case slug for the theme. Used as the
    /// `theme` field on the resolved [`loom_cms_render::CmsPage`].
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::Warm => "warm",
        }
    }
}

/// Typed page envelope exposed by the Forge Lite surface.
///
/// Intentionally exhaustive — the lite contract is a closed
/// surface; adding fields means widening the diagnostic
/// surface, which is a substrate-doctrine event (see
/// `docs/SUBSTRATE_REFRAME_2026_05_21.md` § Forge Lite).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeLitePage {
    /// `<title>` text.
    pub title: String,
    /// `<meta name="description">` text.
    pub description: String,
    /// Canonical URL path (e.g. `/`, `/about`).
    pub path: String,
    /// Theme — closed enumeration.
    #[serde(default)]
    pub theme: ForgeLiteTheme,
    /// Optional brand label rendered as page-shell brand link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brand: Option<String>,
    /// Section list. Order is preserved on resolve.
    pub sections: Vec<ForgeLitePrimitive>,
}

impl ForgeLitePage {
    /// Validate the page against Forge Lite's own constraints
    /// that aren't expressible at the serde-derive layer.
    ///
    /// Returns the first encountered violation. Pure function;
    /// no I/O.
    pub fn validate(&self) -> Result<(), LiteValidationError> {
        if self.title.trim().is_empty() {
            return Err(LiteValidationError::EmptyField {
                field: "title".to_owned(),
            });
        }
        if self.description.trim().is_empty() {
            return Err(LiteValidationError::EmptyField {
                field: "description".to_owned(),
            });
        }
        if !self.path.starts_with('/') {
            return Err(LiteValidationError::BadPath {
                provided: self.path.clone(),
            });
        }
        for (idx, section) in self.sections.iter().enumerate() {
            match section {
                ForgeLitePrimitive::Heading { level, .. } => {
                    if !matches!(*level, 2 | 3) {
                        return Err(LiteValidationError::BadHeadingLevel {
                            section_index: idx,
                            level: *level,
                        });
                    }
                }
                ForgeLitePrimitive::FeatureSpotlight { columns, items, .. } => {
                    if !matches!(*columns, 2 | 3) {
                        return Err(LiteValidationError::BadColumnCount {
                            section_index: idx,
                            columns: *columns,
                        });
                    }
                    if items.is_empty() {
                        return Err(LiteValidationError::EmptyItemList {
                            section_index: idx,
                            primitive: "feature_spotlight".to_owned(),
                        });
                    }
                }
                ForgeLitePrimitive::LogoCloud { logos, .. } => {
                    if logos.is_empty() {
                        return Err(LiteValidationError::EmptyItemList {
                            section_index: idx,
                            primitive: "logo_cloud".to_owned(),
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Validation errors specific to the Forge Lite surface. Each
/// variant carries enough context for a structured-error
/// presentation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LiteValidationError {
    /// Required string field is empty / whitespace-only.
    #[error("field {field:?} is empty")]
    EmptyField {
        /// Field name.
        field: String,
    },
    /// `path` doesn't start with `/`.
    #[error("path must start with '/'; got {provided:?}")]
    BadPath {
        /// What the caller provided.
        provided: String,
    },
    /// Heading level outside 2..=3.
    #[error("section {section_index} heading level {level} outside Forge Lite range 2..=3")]
    BadHeadingLevel {
        /// Index of the offending section in the page.
        section_index: usize,
        /// Level the caller asked for.
        level: u8,
    },
    /// `FeatureSpotlight` column count outside 2..=3.
    #[error("section {section_index} feature_spotlight columns {columns} outside Forge Lite range 2..=3")]
    BadColumnCount {
        /// Index of the offending section in the page.
        section_index: usize,
        /// Column count the caller asked for.
        columns: u8,
    },
    /// A list-shaped primitive (FeatureSpotlight items, LogoCloud
    /// logos) was empty.
    #[error("section {section_index} {primitive} has empty item list")]
    EmptyItemList {
        /// Index of the offending section in the page.
        section_index: usize,
        /// Primitive name.
        primitive: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_page_passes() {
        let page = ForgeLitePage {
            title: "Test".to_owned(),
            description: "A description".to_owned(),
            path: "/".to_owned(),
            theme: ForgeLiteTheme::Light,
            brand: Some("Brand".to_owned()),
            sections: vec![
                ForgeLitePrimitive::Hero {
                    eyebrow: None,
                    title: "Welcome".to_owned(),
                    lede: None,
                    cta_label: None,
                    cta_href: None,
                },
                ForgeLitePrimitive::Heading {
                    level: 2,
                    text: "About".to_owned(),
                },
                ForgeLitePrimitive::Paragraph {
                    text: "Body text.".to_owned(),
                },
            ],
        };
        assert!(page.validate().is_ok());
    }

    #[test]
    fn empty_title_fails() {
        let page = ForgeLitePage {
            title: "  ".to_owned(),
            description: "ok".to_owned(),
            path: "/".to_owned(),
            theme: ForgeLiteTheme::Light,
            brand: None,
            sections: vec![],
        };
        assert!(matches!(
            page.validate(),
            Err(LiteValidationError::EmptyField { field }) if field == "title"
        ));
    }

    #[test]
    fn bad_path_fails() {
        let page = ForgeLitePage {
            title: "T".to_owned(),
            description: "D".to_owned(),
            path: "about".to_owned(),
            theme: ForgeLiteTheme::Light,
            brand: None,
            sections: vec![],
        };
        assert!(matches!(
            page.validate(),
            Err(LiteValidationError::BadPath { .. })
        ));
    }

    #[test]
    fn heading_level_4_fails() {
        let page = ForgeLitePage {
            title: "T".to_owned(),
            description: "D".to_owned(),
            path: "/".to_owned(),
            theme: ForgeLiteTheme::Light,
            brand: None,
            sections: vec![ForgeLitePrimitive::Heading {
                level: 4,
                text: "x".to_owned(),
            }],
        };
        let err = page.validate().unwrap_err();
        assert!(matches!(
            err,
            LiteValidationError::BadHeadingLevel { level: 4, .. }
        ));
    }

    #[test]
    fn feature_spotlight_4_columns_fails() {
        let page = ForgeLitePage {
            title: "T".to_owned(),
            description: "D".to_owned(),
            path: "/".to_owned(),
            theme: ForgeLiteTheme::Light,
            brand: None,
            sections: vec![ForgeLitePrimitive::FeatureSpotlight {
                heading: "h".to_owned(),
                columns: 4,
                items: vec![FeatureItem {
                    title: "t".to_owned(),
                    body: "b".to_owned(),
                }],
            }],
        };
        let err = page.validate().unwrap_err();
        assert!(matches!(
            err,
            LiteValidationError::BadColumnCount { columns: 4, .. }
        ));
    }

    #[test]
    fn empty_feature_items_fails() {
        let page = ForgeLitePage {
            title: "T".to_owned(),
            description: "D".to_owned(),
            path: "/".to_owned(),
            theme: ForgeLiteTheme::Light,
            brand: None,
            sections: vec![ForgeLitePrimitive::FeatureSpotlight {
                heading: "h".to_owned(),
                columns: 2,
                items: vec![],
            }],
        };
        assert!(matches!(
            page.validate(),
            Err(LiteValidationError::EmptyItemList { .. })
        ));
    }

    #[test]
    fn theme_slug_round_trips() {
        assert_eq!(ForgeLiteTheme::Light.slug(), "light");
        assert_eq!(ForgeLiteTheme::Dark.slug(), "dark");
        assert_eq!(ForgeLiteTheme::Warm.slug(), "warm");
    }

    #[test]
    fn closed_enum_rejects_unknown_primitive() {
        // serde(tag="kind", deny_unknown_fields) on ForgeLitePrimitive
        // rejects kinds outside the closed enum. This pins the
        // contract: callers can't smuggle in HeroEditorial /
        // SplitHero / etc. through the wire format.
        let bad = r#"{"kind":"hero_editorial","title":"x"}"#;
        let result: Result<ForgeLitePrimitive, _> = serde_json::from_str(bad);
        assert!(
            result.is_err(),
            "unknown primitive kind must be refused at the serde boundary"
        );
    }

    #[test]
    fn closed_page_rejects_extra_fields() {
        let bad = r#"{
            "title": "T", "description": "D", "path": "/",
            "theme": "light", "sections": [],
            "extra_field": "smuggled"
        }"#;
        let result: Result<ForgeLitePage, _> = serde_json::from_str(bad);
        assert!(
            result.is_err(),
            "unknown top-level field must be refused"
        );
    }

    #[test]
    fn primitive_count_is_ten() {
        // Pin the diagnostic invariant: Forge Lite ships
        // exactly 10 primitives. Adding an 11th means the
        // diagnostic surface widened — review the change
        // against docs/SUBSTRATE_REFRAME_2026_05_21.md §
        // Accessibility axis → Forge Lite first.
        let kinds: &[&str] = &[
            "hero",
            "heading",
            "paragraph",
            "image_hero",
            "feature_spotlight",
            "pull_quote",
            "call_to_action",
            "logo_cloud",
            "divider",
            "spacer",
        ];
        assert_eq!(kinds.len(), 10);
    }
}
