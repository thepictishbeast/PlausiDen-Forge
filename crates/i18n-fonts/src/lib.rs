//! `i18n-fonts` — per-script typed [`FontStack`] + Unicode-range
//! CSS emitter + operator-supplied custom-font registry.
//!
//! Companion to [`i18n_core::Script`] (task #52). For each writing
//! system the platform supports, this crate ships:
//!
//!   1. A typed [`FontStack`] — ordered fallback chain of
//!      `font-family` values guaranteed to render the script's
//!      glyphs on every major operating system without web fonts.
//!   2. A typed [`UnicodeRange`] — the contiguous code-point
//!      blocks the script lives in. Drives CSS `@font-face`
//!      `unicode-range:` declarations so browsers download only
//!      the subset they need per page.
//!   3. A typed [`FontRegistry`] — lets operators upload/install
//!      custom fonts (beyond the built-in script fallbacks) and
//!      have them appear at the head of the resolved chain.
//!
//! Per `super_society_tech_stack`: shipping 4 MB of CJK web-font
//! is the wrong default. System fonts cost zero bandwidth, render
//! correctly offline + on Tor/I2P, and are pre-cached on the
//! user's device. The custom-font path is opt-in: an operator who
//! ships a branded display face still gets it, but the platform
//! never forces a download.
//!
//! ### Public surface
//!
//! - [`FontStack`]              — ordered list of font-families
//! - [`FontStack::for_script`]  — well-known stack per script
//! - [`FontStack::to_css`]      — emits as a `font-family:` value
//! - [`UnicodeRange`]           — typed `(start, end)` codepoint span
//! - [`UnicodeRange::for_script`] — well-known ranges per script
//! - [`UnicodeRange::to_css`]   — emits as `unicode-range:` value
//! - [`CustomFont`]             — operator-uploaded face
//! - [`FontRegistry`]           — built-in stacks + custom fonts
//! - [`emit_font_face`]         — convenience builder for an
//!                                 `@font-face` declaration

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use i18n_core::Script;
use serde::{Deserialize, Serialize};

// ============================================================
// Built-in per-script Unicode ranges (static).
// ============================================================

const LATIN_RANGES: &[UnicodeRange] = &[
    UnicodeRange::new(0x0020, 0x007F), // Basic Latin
    UnicodeRange::new(0x00A0, 0x00FF), // Latin-1 Supplement
    UnicodeRange::new(0x0100, 0x017F), // Latin Extended-A
    UnicodeRange::new(0x0180, 0x024F), // Latin Extended-B
];
const CYRILLIC_RANGES: &[UnicodeRange] = &[
    UnicodeRange::new(0x0400, 0x04FF), // Cyrillic
    UnicodeRange::new(0x0500, 0x052F), // Cyrillic Supplement
    UnicodeRange::new(0x2DE0, 0x2DFF), // Cyrillic Extended-A
];
const ARABIC_RANGES: &[UnicodeRange] = &[
    UnicodeRange::new(0x0600, 0x06FF), // Arabic
    UnicodeRange::new(0x0750, 0x077F), // Arabic Supplement
    UnicodeRange::new(0xFB50, 0xFDFF), // Arabic Presentation Forms-A
    UnicodeRange::new(0xFE70, 0xFEFF), // Arabic Presentation Forms-B
];
const HEBREW_RANGES: &[UnicodeRange] = &[
    UnicodeRange::new(0x0590, 0x05FF), // Hebrew
    UnicodeRange::new(0xFB1D, 0xFB4F), // Hebrew Presentation Forms
];
const HAN_RANGES: &[UnicodeRange] = &[
    UnicodeRange::new(0x4E00, 0x9FFF),   // CJK Unified Ideographs
    UnicodeRange::new(0x3400, 0x4DBF),   // CJK Unified Ideographs Extension A
    UnicodeRange::new(0x20000, 0x2A6DF), // CJK Unified Ideographs Extension B
    UnicodeRange::new(0xF900, 0xFAFF),   // CJK Compatibility Ideographs
];
const HIRAGANA_RANGES: &[UnicodeRange] = &[UnicodeRange::new(0x3040, 0x309F)];
const KATAKANA_RANGES: &[UnicodeRange] = &[
    UnicodeRange::new(0x30A0, 0x30FF),
    UnicodeRange::new(0x31F0, 0x31FF),
];
const HANGUL_RANGES: &[UnicodeRange] = &[
    UnicodeRange::new(0xAC00, 0xD7AF), // Hangul Syllables
    UnicodeRange::new(0x1100, 0x11FF), // Hangul Jamo
    UnicodeRange::new(0x3130, 0x318F), // Hangul Compatibility Jamo
];
const DEVANAGARI_RANGES: &[UnicodeRange] = &[
    UnicodeRange::new(0x0900, 0x097F), // Devanagari
    UnicodeRange::new(0xA8E0, 0xA8FF), // Devanagari Extended
];
const EMPTY_RANGES: &[UnicodeRange] = &[];

// ============================================================
// FontStack — ordered fallback chain.
// ============================================================

/// Ordered fallback chain of font-family values.
///
/// First entry is the preferred face, subsequent entries are
/// fallbacks. Last entry SHOULD be a generic family
/// (`"sans-serif"`, `"serif"`, `"system-ui"`) so the chain always
/// resolves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FontStack(pub Vec<String>);

impl FontStack {
    /// Return a system-installed fallback chain for the given
    /// script. Chains chosen to cover macOS + Windows + Linux +
    /// Android + iOS without web fonts.
    pub fn for_script(script: Script) -> Self {
        let chain: &[&str] = match script {
            Script::Latin => &[
                "-apple-system",
                "BlinkMacSystemFont",
                "Segoe UI",
                "Roboto",
                "Helvetica Neue",
                "Arial",
                "system-ui",
                "sans-serif",
            ],
            Script::Cyrillic => &[
                "Segoe UI",
                "Helvetica Neue",
                "Roboto",
                "PT Sans",
                "DejaVu Sans",
                "Liberation Sans",
                "system-ui",
                "sans-serif",
            ],
            Script::Arabic => &[
                "SF Arabic",
                "Geeza Pro",
                "Tahoma",
                "Segoe UI",
                "Noto Sans Arabic",
                "Amiri",
                "system-ui",
                "sans-serif",
            ],
            Script::Hebrew => &[
                "SF Hebrew",
                "Arial Hebrew",
                "Tahoma",
                "Segoe UI",
                "Noto Sans Hebrew",
                "system-ui",
                "sans-serif",
            ],
            Script::Han => &[
                "PingFang SC",
                "Hiragino Sans",
                "Microsoft YaHei",
                "Noto Sans CJK SC",
                "Source Han Sans SC",
                "WenQuanYi Micro Hei",
                "system-ui",
                "sans-serif",
            ],
            Script::Hiragana | Script::Katakana => &[
                "Hiragino Sans",
                "Yu Gothic",
                "Meiryo",
                "Noto Sans CJK JP",
                "Source Han Sans JP",
                "system-ui",
                "sans-serif",
            ],
            Script::Hangul => &[
                "Apple SD Gothic Neo",
                "Malgun Gothic",
                "Noto Sans CJK KR",
                "Source Han Sans KR",
                "system-ui",
                "sans-serif",
            ],
            Script::Devanagari => &[
                "Kohinoor Devanagari",
                "Nirmala UI",
                "Mangal",
                "Noto Sans Devanagari",
                "Sanskrit Text",
                "system-ui",
                "sans-serif",
            ],
            Script::Other => &["system-ui", "sans-serif"],
        };
        Self(chain.iter().map(|s| (*s).to_string()).collect())
    }

    /// Emit as a CSS `font-family:` value (quoted where the family
    /// name contains whitespace; never quoted for generic keywords).
    pub fn to_css(&self) -> String {
        self.0
            .iter()
            .map(|name| {
                if needs_quotes(name) {
                    format!("\"{}\"", name)
                } else {
                    name.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn needs_quotes(name: &str) -> bool {
    let is_generic = matches!(
        name,
        "serif" | "sans-serif" | "monospace" | "cursive" | "fantasy" | "system-ui"
    );
    if is_generic {
        return false;
    }
    name.contains(char::is_whitespace)
}

// ============================================================
// UnicodeRange — codepoint span for @font-face subsetting.
// ============================================================

/// One contiguous Unicode codepoint range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnicodeRange {
    /// First codepoint in the range (inclusive).
    pub start: u32,
    /// Last codepoint in the range (inclusive).
    pub end: u32,
}

impl UnicodeRange {
    /// Build from start/end values.
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// Emit as a CSS `unicode-range:` token (`U+XXXX-YYYY`).
    pub fn to_css(&self) -> String {
        format!("U+{:04X}-{:04X}", self.start, self.end)
    }

    /// Return the canonical set of ranges covering a script.
    pub fn for_script(script: Script) -> &'static [UnicodeRange] {
        match script {
            Script::Latin => LATIN_RANGES,
            Script::Cyrillic => CYRILLIC_RANGES,
            Script::Arabic => ARABIC_RANGES,
            Script::Hebrew => HEBREW_RANGES,
            Script::Han => HAN_RANGES,
            Script::Hiragana => HIRAGANA_RANGES,
            Script::Katakana => KATAKANA_RANGES,
            Script::Hangul => HANGUL_RANGES,
            Script::Devanagari => DEVANAGARI_RANGES,
            Script::Other => EMPTY_RANGES,
        }
    }
}

// ============================================================
// CustomFont + FontRegistry — operator-uploaded fonts.
// ============================================================

/// Operator-supplied font face. Lives outside the built-in
/// script-fallback chains. The operator uploads or installs a
/// font (managed by the asset pipeline / CMS), and this crate
/// records the metadata the renderer needs to emit `@font-face`
/// for it and prepend it to the resolved [`FontStack`].
///
/// `src_css` is the full CSS `src:` value (e.g.
/// `url(/fonts/brand.woff2) format("woff2")`) — already validated
/// by the consumer; this crate doesn't fetch bytes itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CustomFont {
    /// CSS `font-family` name to register. Quoted automatically
    /// if it contains whitespace.
    pub family: String,
    /// Full CSS `src:` value the renderer will emit.
    pub src_css: String,
    /// Script the operator is augmenting. Determines the
    /// `unicode-range:` declaration emitted alongside the face.
    /// Use [`Script::Other`] for a global custom face that's not
    /// script-specific (e.g. a branded display face).
    pub script: Script,
    /// Optional `font-weight` (e.g. `"400"`, `"600"`,
    /// `"100 900"` for a variable font). `None` → CSS default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<String>,
    /// Optional `font-style` (e.g. `"normal"`, `"italic"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// `font-display:` strategy. Defaults to `swap` (visible
    /// fallback text first, swap when the custom font loads).
    /// Other values: `block`, `fallback`, `optional`, `auto`.
    #[serde(default = "default_display")]
    pub display: String,
    /// Optional explicit Unicode ranges. When set, overrides the
    /// per-script defaults — useful when the operator subsetted
    /// the font to specific codepoint blocks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unicode_range: Option<Vec<UnicodeRange>>,
}

fn default_display() -> String {
    "swap".to_string()
}

impl CustomFont {
    /// Emit a complete `@font-face` declaration for this font.
    pub fn to_font_face(&self) -> String {
        let ranges: &[UnicodeRange] = match &self.unicode_range {
            Some(v) => v.as_slice(),
            None => UnicodeRange::for_script(self.script),
        };
        let urange = if ranges.is_empty() {
            String::new()
        } else {
            let css = ranges
                .iter()
                .map(UnicodeRange::to_css)
                .collect::<Vec<_>>()
                .join(", ");
            format!(" unicode-range: {css};")
        };
        let weight = self
            .weight
            .as_ref()
            .map(|w| format!(" font-weight: {w};"))
            .unwrap_or_default();
        let style = self
            .style
            .as_ref()
            .map(|s| format!(" font-style: {s};"))
            .unwrap_or_default();
        format!(
            "@font-face {{ font-family: \"{}\"; src: {}; font-display: {};{}{}{} }}",
            self.family, self.src_css, self.display, weight, style, urange
        )
    }
}

/// Resolved bundle: every custom font + the per-script chains a
/// page might need. Consumers (Forge build pipeline, Loom
/// edit-serve admin UI) read [`FontRegistry::resolve_stack`] +
/// [`FontRegistry::all_font_faces`] and inject the result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FontRegistry {
    /// Operator-uploaded fonts. Order matters — first matching
    /// face wins at resolve time.
    #[serde(default)]
    pub custom: Vec<CustomFont>,
}

impl FontRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an operator-uploaded custom font.
    /// Reserved for ergonomic call sites: `reg.register(font)`
    /// instead of `reg.custom.push(font)`.
    pub fn register(&mut self, font: CustomFont) {
        self.custom.push(font);
    }

    /// Resolve the effective [`FontStack`] for a given script —
    /// operator-uploaded fonts (for this script OR for
    /// [`Script::Other`]) first, then the built-in fallback chain.
    pub fn resolve_stack(&self, script: Script) -> FontStack {
        let mut chain: Vec<String> = self
            .custom
            .iter()
            .filter(|f| f.script == script || f.script == Script::Other)
            .map(|f| f.family.clone())
            .collect();
        chain.extend(FontStack::for_script(script).0);
        FontStack(chain)
    }

    /// Emit one `@font-face` block per registered custom font,
    /// joined by newlines. Drop this into a stylesheet head.
    pub fn all_font_faces(&self) -> String {
        self.custom
            .iter()
            .map(CustomFont::to_font_face)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ============================================================
// Free-standing helper.
// ============================================================

/// Convenience: emit a `@font-face` block with the right
/// unicode-range subset for the script.
///
/// Caller provides `family` (the in-CSS face name) + `src` (a
/// `url(...) format(...)` string for the font binary). For more
/// control, use [`CustomFont`] + [`FontRegistry`] which carry
/// weight / style / display / explicit range overrides.
pub fn emit_font_face(family: &str, src: &str, script: Script) -> String {
    let ranges = UnicodeRange::for_script(script);
    if ranges.is_empty() {
        format!("@font-face {{ font-family: \"{family}\"; src: {src}; font-display: swap; }}")
    } else {
        let urange_css = ranges
            .iter()
            .map(UnicodeRange::to_css)
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "@font-face {{ font-family: \"{family}\"; src: {src}; font-display: swap; unicode-range: {urange_css}; }}"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use i18n_core::LocaleId;

    #[test]
    fn latin_stack_starts_with_apple_system() {
        let s = FontStack::for_script(Script::Latin);
        assert_eq!(s.0[0], "-apple-system");
        assert!(s.0.last().unwrap().contains("sans-serif"));
    }

    #[test]
    fn han_stack_includes_cjk_fallbacks() {
        let s = FontStack::for_script(Script::Han);
        let chain = s.0.join(",");
        assert!(chain.contains("PingFang"));
        assert!(chain.contains("Hiragino"));
        assert!(chain.contains("YaHei"));
        assert!(chain.contains("Noto Sans CJK"));
    }

    #[test]
    fn arabic_stack_includes_arabic_fallbacks() {
        let s = FontStack::for_script(Script::Arabic);
        let chain = s.0.join(",");
        assert!(chain.contains("SF Arabic") || chain.contains("Geeza"));
        assert!(chain.contains("Noto Sans Arabic"));
    }

    #[test]
    fn hangul_stack_includes_korean_fallbacks() {
        let s = FontStack::for_script(Script::Hangul);
        let chain = s.0.join(",");
        assert!(chain.contains("Apple SD Gothic Neo") || chain.contains("Malgun Gothic"));
        assert!(chain.contains("Noto Sans CJK KR"));
    }

    #[test]
    fn devanagari_stack_includes_devanagari_fallbacks() {
        let s = FontStack::for_script(Script::Devanagari);
        let chain = s.0.join(",");
        assert!(
            chain.contains("Kohinoor") || chain.contains("Nirmala") || chain.contains("Mangal")
        );
        assert!(chain.contains("Noto Sans Devanagari"));
    }

    #[test]
    fn to_css_quotes_whitespace_families_but_not_generic_keywords() {
        let s = FontStack(vec![
            "Helvetica Neue".into(),
            "Arial".into(),
            "sans-serif".into(),
        ]);
        let css = s.to_css();
        assert!(css.contains("\"Helvetica Neue\""));
        assert!(css.contains("Arial"));
        assert!(css.contains("sans-serif"));
        assert!(!css.contains("\"sans-serif\""));
    }

    #[test]
    fn unicode_range_covers_known_blocks() {
        let r = UnicodeRange::for_script(Script::Han);
        assert!(r.iter().any(|u| u.start == 0x4E00 && u.end == 0x9FFF));
    }

    #[test]
    fn unicode_range_emits_4_hex_digits() {
        let r = UnicodeRange::new(0x4E00, 0x9FFF);
        assert_eq!(r.to_css(), "U+4E00-9FFF");
    }

    #[test]
    fn emit_font_face_includes_unicode_range() {
        let css = emit_font_face("MyHan", "url(my.woff2) format(\"woff2\")", Script::Han);
        assert!(css.contains("@font-face"));
        assert!(css.contains("MyHan"));
        assert!(css.contains("font-display: swap"));
        assert!(css.contains("unicode-range"));
        assert!(css.contains("U+4E00-9FFF"));
    }

    #[test]
    fn emit_font_face_for_other_script_omits_unicode_range() {
        let css = emit_font_face("X", "url(x.woff2)", Script::Other);
        assert!(!css.contains("unicode-range"));
    }

    #[test]
    fn locale_to_script_to_stack_chains_correctly() {
        let l = LocaleId::parse("zh-Hans").unwrap();
        let stack = FontStack::for_script(Script::for_locale(&l));
        let chain = stack.0.join(",");
        assert!(chain.contains("PingFang") || chain.contains("YaHei"));
    }

    // ----- CustomFont + FontRegistry -----

    #[test]
    fn custom_font_emits_font_face_with_subset() {
        let f = CustomFont {
            family: "BrandHan".into(),
            src_css: r#"url(/fonts/brand.woff2) format("woff2")"#.into(),
            script: Script::Han,
            weight: Some("400 700".into()),
            style: None,
            display: "swap".into(),
            unicode_range: None,
        };
        let css = f.to_font_face();
        assert!(css.contains("BrandHan"));
        assert!(css.contains("font-weight: 400 700"));
        assert!(css.contains("font-display: swap"));
        assert!(css.contains("unicode-range"));
        assert!(css.contains("U+4E00-9FFF"));
    }

    #[test]
    fn custom_font_explicit_range_overrides_script_default() {
        let f = CustomFont {
            family: "Tiny".into(),
            src_css: "url(/x.woff2)".into(),
            script: Script::Han,
            weight: None,
            style: None,
            display: "swap".into(),
            unicode_range: Some(vec![UnicodeRange::new(0x4E00, 0x4E10)]),
        };
        let css = f.to_font_face();
        assert!(css.contains("U+4E00-4E10"));
        assert!(!css.contains("U+9FFF"));
    }

    #[test]
    fn registry_resolve_prepends_custom_then_falls_back() {
        let mut reg = FontRegistry::new();
        reg.register(CustomFont {
            family: "BrandHan".into(),
            src_css: "url(/h.woff2)".into(),
            script: Script::Han,
            weight: None,
            style: None,
            display: "swap".into(),
            unicode_range: None,
        });
        let stack = reg.resolve_stack(Script::Han);
        assert_eq!(stack.0[0], "BrandHan");
        // Built-in fallbacks still present after the custom face.
        assert!(stack.0.iter().any(|f| f.contains("PingFang")));
        assert!(stack.0.last().unwrap().contains("sans-serif"));
    }

    #[test]
    fn registry_global_custom_appears_on_every_script() {
        let mut reg = FontRegistry::new();
        reg.register(CustomFont {
            family: "BrandDisplay".into(),
            src_css: "url(/b.woff2)".into(),
            script: Script::Other, // global
            weight: None,
            style: None,
            display: "swap".into(),
            unicode_range: None,
        });
        // Show up on Latin AND Han + Arabic.
        for s in [Script::Latin, Script::Han, Script::Arabic] {
            let stack = reg.resolve_stack(s);
            assert_eq!(stack.0[0], "BrandDisplay", "script={s:?}");
        }
    }

    #[test]
    fn registry_does_not_pollute_unrelated_scripts() {
        let mut reg = FontRegistry::new();
        reg.register(CustomFont {
            family: "ArabicOnly".into(),
            src_css: "url(/a.woff2)".into(),
            script: Script::Arabic,
            weight: None,
            style: None,
            display: "swap".into(),
            unicode_range: None,
        });
        let han = reg.resolve_stack(Script::Han);
        assert!(!han.0.contains(&"ArabicOnly".to_string()));
        let ar = reg.resolve_stack(Script::Arabic);
        assert_eq!(ar.0[0], "ArabicOnly");
    }

    #[test]
    fn registry_all_font_faces_emits_one_per_custom() {
        let mut reg = FontRegistry::new();
        reg.register(CustomFont {
            family: "A".into(),
            src_css: "url(/a.woff2)".into(),
            script: Script::Latin,
            weight: None,
            style: None,
            display: "swap".into(),
            unicode_range: None,
        });
        reg.register(CustomFont {
            family: "B".into(),
            src_css: "url(/b.woff2)".into(),
            script: Script::Han,
            weight: None,
            style: None,
            display: "swap".into(),
            unicode_range: None,
        });
        let css = reg.all_font_faces();
        assert_eq!(css.matches("@font-face").count(), 2);
        assert!(css.contains("\"A\""));
        assert!(css.contains("\"B\""));
    }

    #[test]
    fn registry_serde_round_trips() {
        let mut reg = FontRegistry::new();
        reg.register(CustomFont {
            family: "X".into(),
            src_css: "url(/x.woff2)".into(),
            script: Script::Cyrillic,
            weight: Some("400".into()),
            style: Some("italic".into()),
            display: "block".into(),
            unicode_range: None,
        });
        let s = serde_json::to_string(&reg).unwrap();
        let back: FontRegistry = serde_json::from_str(&s).unwrap();
        assert_eq!(reg, back);
    }

    #[test]
    fn custom_font_rejects_unknown_field() {
        let bad = r#"{"family":"X","src-css":"url(x)","script":"latin","display":"swap","ahem":1}"#;
        let r: Result<CustomFont, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }
}
