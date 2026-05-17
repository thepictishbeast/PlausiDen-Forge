//! `i18n-core` — typed i18n substrate every PlausiDen subsystem
//! (CMS, Loom edit-serve, Forge phase output, admin UI) projects
//! through.
//!
//! Per `PLATFORM_ROADMAP.md` §7 + the
//! `super_society_tech_stack` "ready for everyone globally"
//! framing: locale + direction + script + plural rules are
//! first-class platform primitives, not afterthoughts. Every
//! translatable entity carries a typed [`LocaleId`] and every
//! rendering path knows its [`TextDirection`].
//!
//! ### Scope (this iteration)
//!
//! Tight scope. Ships the typed surface + a small but correct
//! ICU MessageFormat subset (plural + select):
//!
//!   * [`LocaleId`]         — BCP 47 validated newtype
//!   * [`TextDirection`]    — closed enum (Ltr / Rtl)
//!   * [`Script`]           — closed enum (Latin / Cyrillic /
//!                            Arabic / Hebrew / Han / Hiragana /
//!                            Katakana / Hangul / Devanagari / Other)
//!   * [`MessageId`]        — kebab-case message identifier
//!   * [`Translatable<T>`]  — wraps any value with its locale
//!   * [`MessageBundle`]    — per-locale messages
//!   * [`PluralCategory`]   — CLDR plural categories
//!   * [`format_plural`]    — applies a plural-form table
//!   * [`format_select`]    — applies a select-form table
//!
//! Full ICU parser (nested arguments, date/number skeletons) is
//! a follow-up. This commit gives downstream consumers a usable
//! substrate they can integrate against today.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Typed BCP 47 locale identifier.
///
/// Validates a minimal subset of BCP 47:
///   * 2-3 letter primary language tag, lowercase
///   * optional `-` + 2-letter or 3-digit region (uppercase or digits)
///   * optional `-` + 4-letter script (TitleCase)
///   * length ≤ 35 chars
///
/// Matches the shape every modern browser + intl library accepts.
/// Tags rejected by [`Self::parse`] cannot represent a coherent
/// locale and would silently fall back to default in CLDR — better
/// to refuse at the boundary.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LocaleId(String);

impl LocaleId {
    /// Maximum tag length per BCP 47 + sanity bound.
    pub const MAX_LEN: usize = 35;

    /// Construct from a string slice.
    pub fn parse(s: impl AsRef<str>) -> Result<Self, I18nError> {
        let s = s.as_ref();
        if s.is_empty() {
            return Err(I18nError::InvalidLocale("empty".into()));
        }
        if s.len() > Self::MAX_LEN {
            return Err(I18nError::InvalidLocale(format!(
                "{s:?} exceeds {} chars",
                Self::MAX_LEN
            )));
        }
        let parts: Vec<&str> = s.split('-').collect();
        if parts.is_empty() || parts[0].len() < 2 || parts[0].len() > 3 {
            return Err(I18nError::InvalidLocale(format!(
                "{s:?} primary subtag must be 2-3 letters"
            )));
        }
        if !parts[0].chars().all(|c| c.is_ascii_lowercase()) {
            return Err(I18nError::InvalidLocale(format!(
                "{s:?} primary subtag must be lowercase ASCII"
            )));
        }
        for subtag in parts.iter().skip(1) {
            let len = subtag.len();
            let is_region = (len == 2 && subtag.chars().all(|c| c.is_ascii_uppercase()))
                || (len == 3 && subtag.chars().all(|c| c.is_ascii_digit()));
            let is_script = len == 4
                && subtag
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_uppercase())
                    .unwrap_or(false)
                && subtag.chars().skip(1).all(|c| c.is_ascii_lowercase());
            let is_variant =
                len >= 5 && len <= 8 && subtag.chars().all(|c| c.is_ascii_alphanumeric());
            if !(is_region || is_script || is_variant) {
                return Err(I18nError::InvalidLocale(format!(
                    "{s:?} subtag {subtag:?} not a valid region/script/variant"
                )));
            }
        }
        Ok(Self(s.to_string()))
    }

    /// Raw string view.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The primary language subtag (e.g. `"en"` from `"en-US"`).
    pub fn language(&self) -> &str {
        self.0.split('-').next().unwrap_or(&self.0)
    }

    /// The region subtag, if present.
    pub fn region(&self) -> Option<&str> {
        self.0.split('-').find(|p| {
            (p.len() == 2 && p.chars().all(|c| c.is_ascii_uppercase()))
                || (p.len() == 3 && p.chars().all(|c| c.is_ascii_digit()))
        })
    }

    /// The script subtag, if present.
    pub fn script(&self) -> Option<&str> {
        self.0.split('-').find(|p| {
            p.len() == 4
                && p.chars()
                    .next()
                    .map(|c| c.is_ascii_uppercase())
                    .unwrap_or(false)
                && p.chars().skip(1).all(|c| c.is_ascii_lowercase())
        })
    }
}

impl std::fmt::Display for LocaleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Closed enum of writing-direction values. Used to drive
/// `<html dir="…">` + CSS `direction:` declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TextDirection {
    /// Left-to-right (default for most locales).
    #[default]
    Ltr,
    /// Right-to-left (Arabic / Hebrew / Persian / Urdu / etc.)
    Rtl,
}

impl TextDirection {
    /// Resolve the default direction for a [`LocaleId`]. Returns
    /// Rtl for the well-known RTL languages, Ltr otherwise.
    pub fn for_locale(locale: &LocaleId) -> Self {
        match locale.language() {
            "ar" | "he" | "fa" | "ur" | "yi" | "ps" | "sd" | "ku" | "ckb" => Self::Rtl,
            _ => Self::Ltr,
        }
    }

    /// Stable slug for `<html dir>`.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Ltr => "ltr",
            Self::Rtl => "rtl",
        }
    }
}

/// Coarse script family. Used by font-subsetting (#53) +
/// `loom-font-stack` selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Script {
    /// Latin alphabet — most European + Vietnamese + Turkish.
    #[default]
    Latin,
    /// Cyrillic — Russian, Ukrainian, Bulgarian, Serbian, Mongolian.
    Cyrillic,
    /// Arabic — Arabic, Persian, Urdu, Pashto.
    Arabic,
    /// Hebrew.
    Hebrew,
    /// Han ideographs — Chinese (Simplified + Traditional),
    /// Japanese kanji.
    Han,
    /// Japanese hiragana.
    Hiragana,
    /// Japanese katakana.
    Katakana,
    /// Korean hangul.
    Hangul,
    /// Devanagari — Hindi, Marathi, Nepali, Sanskrit.
    Devanagari,
    /// Anything else (Greek, Thai, Tibetan, Ethiopic, etc.) —
    /// add specific variants here as the platform encounters them.
    Other,
}

impl Script {
    /// Resolve script from a locale's language tag. Falls back to
    /// [`Script::Latin`] when the script tag is absent + the
    /// language doesn't map to a well-known script.
    pub fn for_locale(locale: &LocaleId) -> Self {
        // Explicit script subtag takes precedence.
        if let Some(s) = locale.script() {
            return match s {
                "Latn" => Self::Latin,
                "Cyrl" => Self::Cyrillic,
                "Arab" => Self::Arabic,
                "Hebr" => Self::Hebrew,
                "Hans" | "Hant" | "Hani" => Self::Han,
                "Hira" => Self::Hiragana,
                "Kana" => Self::Katakana,
                "Hang" | "Kore" => Self::Hangul,
                "Deva" => Self::Devanagari,
                _ => Self::Other,
            };
        }
        match locale.language() {
            "zh" | "ja" => Self::Han, // ja has kana mix; primary is Han
            "ko" => Self::Hangul,
            "ar" | "fa" | "ur" | "ps" => Self::Arabic,
            "he" | "yi" => Self::Hebrew,
            "hi" | "mr" | "ne" | "sa" => Self::Devanagari,
            "ru" | "uk" | "bg" | "sr" | "mk" | "mn" => Self::Cyrillic,
            _ => Self::Latin,
        }
    }
}

/// Typed message identifier — kebab-case + dotted-namespace allowed.
/// Example: `"home.hero.title"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageId(String);

impl MessageId {
    /// Construct from a string slice. Allows `[a-z0-9.-]`, must
    /// start with `[a-z]`, length ≤ 128.
    pub fn parse(s: impl AsRef<str>) -> Result<Self, I18nError> {
        let s = s.as_ref();
        if s.is_empty() || s.len() > 128 {
            return Err(I18nError::InvalidMessageId(format!("{s:?} length")));
        }
        if !s
            .chars()
            .next()
            .map(|c| c.is_ascii_lowercase())
            .unwrap_or(false)
        {
            return Err(I18nError::InvalidMessageId(format!(
                "{s:?} must start with [a-z]"
            )));
        }
        for c in s.chars() {
            if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.') {
                return Err(I18nError::InvalidMessageId(format!(
                    "{s:?} contains {c:?} not in [a-z0-9.-]"
                )));
            }
        }
        Ok(Self(s.to_string()))
    }

    /// Raw string view.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Locale-tagged value wrapper. Every translatable entity at the
/// API boundary should be either [`Translatable<String>`] or a
/// reference to a [`MessageId`] looked up in a [`MessageBundle`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Translatable<T> {
    /// Locale this value is in.
    pub locale: LocaleId,
    /// The localized value itself.
    pub value: T,
}

/// Per-locale message dictionary.
///
/// No `Default` — every bundle MUST declare its locale explicitly,
/// because "default locale" silently picks English in CLDR and
/// hides bugs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct MessageBundle {
    /// Locale these messages are in.
    pub locale: LocaleId,
    /// `MessageId` → localized template string. Templates may
    /// contain `{argname}` placeholders + plural/select forms.
    #[serde(default)]
    pub messages: BTreeMap<String, String>,
}

impl MessageBundle {
    /// Look up `id` in the bundle.
    pub fn get(&self, id: &MessageId) -> Option<&str> {
        self.messages.get(id.as_str()).map(String::as_str)
    }
}

/// CLDR plural categories. Locales select among these per the
/// CLDR plural-rule for the language. The platform uses the
/// English fallback rules here (one/other); fuller per-locale
/// rules can be plugged in by a downstream crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluralCategory {
    /// `zero`
    Zero,
    /// `one` — singular
    One,
    /// `two`
    Two,
    /// `few`
    Few,
    /// `many`
    Many,
    /// `other` — fallback / plural for English
    Other,
}

/// Select a plural form from `forms` for `n`, using the CLDR
/// fallback hierarchy: zero → one → two → few → many → other.
/// Returns `forms["other"]` if no more-specific category matches.
///
/// Per-locale rules aren't applied here — that's a follow-up. The
/// English-like fallback (`n==1` → one, else other) is the
/// out-of-the-box behaviour.
pub fn format_plural<'a>(n: i64, forms: &'a BTreeMap<String, String>) -> Option<&'a str> {
    let category = if n == 1 {
        PluralCategory::One
    } else {
        PluralCategory::Other
    };
    let key = plural_slug(category);
    forms
        .get(key)
        .or_else(|| forms.get("other"))
        .map(String::as_str)
}

fn plural_slug(c: PluralCategory) -> &'static str {
    match c {
        PluralCategory::Zero => "zero",
        PluralCategory::One => "one",
        PluralCategory::Two => "two",
        PluralCategory::Few => "few",
        PluralCategory::Many => "many",
        PluralCategory::Other => "other",
    }
}

/// Select a `select`-style form from `forms` for `key`. Falls back
/// to `forms["other"]` if missing. Used for gender + status enums.
pub fn format_select<'a>(key: &str, forms: &'a BTreeMap<String, String>) -> Option<&'a str> {
    forms
        .get(key)
        .or_else(|| forms.get("other"))
        .map(String::as_str)
}

/// Errors at the i18n boundary.
#[derive(Debug, thiserror::Error)]
pub enum I18nError {
    /// BCP 47 locale tag failed validation.
    #[error("invalid locale: {0}")]
    InvalidLocale(String),
    /// MessageId failed validation.
    #[error("invalid message id: {0}")]
    InvalidMessageId(String),
    /// Message not present in the active bundle.
    #[error("message not found: {0}")]
    MessageNotFound(MessageId),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_parses_basic_codes() {
        assert!(LocaleId::parse("en").is_ok());
        assert!(LocaleId::parse("en-US").is_ok());
        assert!(LocaleId::parse("zh-Hans").is_ok());
        assert!(LocaleId::parse("zh-Hans-CN").is_ok());
        assert!(LocaleId::parse("ar").is_ok());
        assert!(LocaleId::parse("ar-EG").is_ok());
        assert!(LocaleId::parse("ko").is_ok());
        assert!(LocaleId::parse("ru-RU").is_ok());
        assert!(LocaleId::parse("he-IL").is_ok());
        assert!(LocaleId::parse("hi-IN").is_ok());
        assert!(LocaleId::parse("fr-150").is_ok()); // 3-digit region
    }

    #[test]
    fn locale_rejects_invalid_tags() {
        assert!(LocaleId::parse("").is_err());
        assert!(LocaleId::parse("EN").is_err()); // uppercase primary
        assert!(LocaleId::parse("english").is_err()); // too long primary
        assert!(LocaleId::parse("en-us").is_err()); // lowercase region
        assert!(LocaleId::parse("en-USA").is_err()); // 3-letter alphabetic region
        assert!(LocaleId::parse("en_US").is_err()); // wrong separator
        assert!(LocaleId::parse(&"a".repeat(36)).is_err()); // too long total
    }

    #[test]
    fn locale_extracts_subtags() {
        let l = LocaleId::parse("zh-Hans-CN").unwrap();
        assert_eq!(l.language(), "zh");
        assert_eq!(l.script(), Some("Hans"));
        assert_eq!(l.region(), Some("CN"));

        let l = LocaleId::parse("en").unwrap();
        assert_eq!(l.language(), "en");
        assert_eq!(l.script(), None);
        assert_eq!(l.region(), None);
    }

    #[test]
    fn text_direction_resolves_for_known_rtl() {
        let rtl_locales = ["ar", "ar-EG", "he", "he-IL", "fa", "ur-PK", "yi"];
        for s in rtl_locales {
            let l = LocaleId::parse(s).unwrap();
            assert_eq!(TextDirection::for_locale(&l), TextDirection::Rtl, "{s}");
        }
        for s in ["en", "fr", "de", "zh-Hans", "ja", "ko"] {
            let l = LocaleId::parse(s).unwrap();
            assert_eq!(TextDirection::for_locale(&l), TextDirection::Ltr, "{s}");
        }
    }

    #[test]
    fn script_resolves_for_major_languages() {
        for (s, expected) in [
            ("en", Script::Latin),
            ("zh-Hans", Script::Han),
            ("ja", Script::Han),
            ("ko", Script::Hangul),
            ("ar", Script::Arabic),
            ("he", Script::Hebrew),
            ("hi", Script::Devanagari),
            ("ru", Script::Cyrillic),
        ] {
            let l = LocaleId::parse(s).unwrap();
            assert_eq!(Script::for_locale(&l), expected, "{s}");
        }
    }

    #[test]
    fn message_id_validates_shape() {
        assert!(MessageId::parse("home").is_ok());
        assert!(MessageId::parse("home.hero.title").is_ok());
        assert!(MessageId::parse("user-profile").is_ok());

        assert!(MessageId::parse("").is_err());
        assert!(MessageId::parse(".leading-dot").is_err());
        assert!(MessageId::parse("Capital").is_err());
        assert!(MessageId::parse("has space").is_err());
        assert!(MessageId::parse("has_underscore").is_err());
    }

    #[test]
    fn translatable_serde_round_trip() {
        let t: Translatable<String> = Translatable {
            locale: LocaleId::parse("ja-JP").unwrap(),
            value: "こんにちは".into(),
        };
        let s = serde_json::to_string(&t).unwrap();
        let back: Translatable<String> = serde_json::from_str(&s).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn bundle_lookup() {
        let mut messages = BTreeMap::new();
        messages.insert("home.hero.title".into(), "Hello, world".into());
        let bundle = MessageBundle {
            locale: LocaleId::parse("en-US").unwrap(),
            messages,
        };
        let id = MessageId::parse("home.hero.title").unwrap();
        assert_eq!(bundle.get(&id), Some("Hello, world"));
        let miss = MessageId::parse("nope").unwrap();
        assert_eq!(bundle.get(&miss), None);
    }

    #[test]
    fn plural_picks_one_then_falls_back_to_other() {
        let mut forms = BTreeMap::new();
        forms.insert("one".into(), "1 file".into());
        forms.insert("other".into(), "{n} files".into());
        assert_eq!(format_plural(1, &forms), Some("1 file"));
        assert_eq!(format_plural(0, &forms), Some("{n} files"));
        assert_eq!(format_plural(2, &forms), Some("{n} files"));
        assert_eq!(format_plural(99, &forms), Some("{n} files"));
    }

    #[test]
    fn plural_falls_back_when_one_missing() {
        let mut forms = BTreeMap::new();
        forms.insert("other".into(), "{n} files".into());
        assert_eq!(format_plural(1, &forms), Some("{n} files"));
    }

    #[test]
    fn select_picks_matching_then_falls_back_to_other() {
        let mut forms = BTreeMap::new();
        forms.insert("masculine".into(), "Il".into());
        forms.insert("feminine".into(), "Elle".into());
        forms.insert("other".into(), "They".into());
        assert_eq!(format_select("masculine", &forms), Some("Il"));
        assert_eq!(format_select("neuter", &forms), Some("They"));
    }
}
