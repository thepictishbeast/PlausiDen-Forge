//! Tenant-level visual-style overrides — `[style]` section of
//! `forge.toml`.
//!
//! Per paul 2026-05-21: substrate must NOT ship "entire web
//! content premade". Each tenant's visual identity comes from a
//! per-tenant `[style]` config (palette, fonts, radii, density),
//! not from picking among N pre-styled section primitives. Two
//! tenants composing identical atomic block trees render
//! distinctly because their `[style]` configs differ.
//!
//! Wire format (excerpt of `forge.toml`):
//!
//! ```toml
//! [style.palette]
//! primary       = "#733635"
//! accent        = "#DBA830"
//! ink           = "#291D14"
//! bg            = "#FAF8F5"
//! border        = "#E6E2DA"
//! link          = "#733635"
//! link_hover    = "#4A1F1F"
//! danger        = "#B91C1C"
//!
//! [style.fonts]
//! display = "\"Outfit\", ui-rounded, system-ui, sans-serif"
//! body    = "\"Inter\", ui-sans-serif, system-ui, sans-serif"
//! mono    = "\"JetBrains Mono\", ui-monospace, monospace"
//!
//! [style.radius]
//! sm = "4px"
//! md = "10px"
//! lg = "18px"
//!
//! [style.nav]
//! link_color   = "#733635"
//! link_weight  = "700"
//! link_padding = "0.78rem 1.56rem"
//! ```
//!
//! Generation: [`TenantStyle::to_css_root_block`] emits a CSS
//! `:root { --loom-color-primary: …; … }` block that the render
//! phase injects AFTER the loom-skin.css link so token values
//! override the substrate defaults.
//!
//! Fail-tolerant load: ANY error path (no file, non-UTF8,
//! malformed TOML, missing section, schema mismatch) returns
//! `None` so a tenant without `[style]` gets the substrate
//! baseline rather than a build failure.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Parsed `[style]` section of `forge.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case", deny_unknown_fields)]
pub struct TenantStyle {
    /// `[style.palette]` — semantic color slots keyed by role.
    /// Each value is a CSS color literal (hex / rgb / oklch /
    /// hsl). Substrate-recognised keys: `primary`, `secondary`,
    /// `accent`, `accent_2`, `ink`, `bg`, `muted`, `border`,
    /// `link`, `link_hover`, `focus`, `danger`, `on_primary`,
    /// `on_danger`. Additional keys pass through to CSS vars
    /// (`--loom-color-<key>`) for tenant-specific extensions.
    pub palette: BTreeMap<String, String>,
    /// `[style.fonts]` — font-family stacks keyed by role
    /// (`display`, `body`, `mono`).
    pub fonts: BTreeMap<String, String>,
    /// `[style.radius]` — border-radius scale keyed by step
    /// (`sm`, `md`, `lg`, `xl`).
    pub radius: BTreeMap<String, String>,
    /// `[style.nav]` — primary-nav link styling overrides keyed by
    /// slot. Substrate-recognised keys: `link_color`, `link_weight`,
    /// `link_hover_color`, `link_padding`. Each key passes through to
    /// a CSS var (`--loom-nav-<key>`) consumed by the nav rules in
    /// loom-skin.css with a fallback to the premium muted-nav
    /// baseline, so a tenant only restyles its nav by opting in and
    /// other tenants are unaffected. Additional keys pass through for
    /// tenant-specific extensions.
    pub nav: BTreeMap<String, String>,
    /// `[style.image]` — content-image treatment overrides keyed by
    /// slot. Each key passes through to a CSS var (`--loom-img-<key>`)
    /// consumed by the image / figure / spotlight rules in
    /// loom-skin.css with a fallback to the premium rounded baseline,
    /// so a tenant only restyles its images by opting in and other
    /// tenants are unaffected. Recognised keys: `radius` (corner
    /// radius for all content images — set `0` for squared), `shadow`
    /// (inset card shadow on image-text rows — set `none` to flatten).
    pub image: BTreeMap<String, String>,
    /// `[style.weights]` — font-weight overrides keyed by role. Each
    /// key passes through to a CSS var (`--loom-weight-<key>`) consumed
    /// by the display / heading rules in loom-skin.css with a fallback
    /// to the substrate baseline weight, so a tenant only restyles its
    /// type weights by opting in and other tenants are unaffected.
    /// Recognised keys: `display` (large hero / display type),
    /// `heading` (section + row headings). Additional keys pass through
    /// for tenant-specific extensions.
    pub weights: BTreeMap<String, String>,
    /// `[style.sizes]` — type-size overrides keyed by slot. Each key
    /// passes through to a CSS var (`--loom-size-<key>`) consumed by
    /// the matching rule in loom-skin.css with a fallback to the
    /// substrate clamp, so a tenant only resizes a slot by opting in
    /// and other tenants are unaffected. Values are full CSS length
    /// expressions (`clamp(...)`, `rem`, `px`). Recognised keys:
    /// `hero_title` (image-hero headline). Additional keys pass through
    /// for tenant-specific extensions.
    pub sizes: BTreeMap<String, String>,
    /// `[style.text]` — typographic-treatment overrides keyed by slot.
    /// Covers the type dimensions that are neither color, family, size,
    /// nor weight: `text-transform`, `letter-spacing`, `line-height`,
    /// `font-style`. Each key passes through to a CSS var
    /// (`--loom-text-<key>`) consumed by the matching rule in
    /// loom-skin.css with a fallback to the substrate baseline, so a
    /// tenant only restyles a slot by opting in and other tenants are
    /// unaffected. Recognised keys: `footer_heading_transform`
    /// (footer column-heading case — set `none` for Title-case sans
    /// instead of the mono micro-caps default), `footer_heading_tracking`
    /// (footer column-heading letter-spacing). Additional keys pass
    /// through for tenant-specific extensions.
    pub text: BTreeMap<String, String>,
    /// `[[style.webfonts]]` — self-hosted `@font-face` declarations.
    /// Each entry pins one woff2 (or other format) subset file the
    /// tenant ships under its own `static/` dir, so the families named
    /// in `[style.fonts]` actually load instead of silently falling
    /// back to system-ui. Sovereignty-aligned: fonts are self-hosted
    /// from the tenant origin, never fetched from a third-party CDN.
    /// Emitted into the EXTERNAL `tenant-style.css` (the page-shell CSP
    /// `default-src 'self'` covers same-origin font + style fetches; an
    /// inline `<style>` would be hash-blocked). Additive: a tenant with
    /// no `[[style.webfonts]]` is byte-for-byte unchanged.
    ///
    /// This is the operator-pins-exact-subset-file path; the
    /// script-aware fallback-chain cousin lives in the `i18n-fonts`
    /// crate (`CustomFont` / `FontRegistry`). Kept here as raw CSS
    /// strings because Google-style subsets use comma-lists and single
    /// codepoints that the typed `UnicodeRange` does not model.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub webfonts: Vec<WebFont>,
    /// `[style.density]` — density tier override (`sparse` /
    /// `comfortable` / `dense` / `extreme`). Optional; substrate
    /// default is `comfortable`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density: Option<String>,
}

/// One self-hosted `@font-face` declaration (`[[style.webfonts]]`).
///
/// All fields are raw CSS strings so the operator can mirror a
/// Google-Fonts-style subset export verbatim (variable weight ranges
/// like `"300 700"`, comma-list `unicode-range` values). `src` is the
/// same-origin path to the font file the tenant ships in `static/`
/// (e.g. `/fonts/outfit-latin.woff2`); the renderer wraps it as
/// `url(<src>) format("<format>")`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct WebFont {
    /// CSS `font-family` name to register (must match the name used in
    /// `[style.fonts]`, e.g. `"Outfit"`).
    pub family: String,
    /// Same-origin path to the font file under the tenant `static/`
    /// dir (e.g. `/fonts/outfit-latin.woff2`).
    pub src: String,
    /// Font container format for the `format(...)` hint. Defaults to
    /// `woff2`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// `font-weight` — single (`"700"`) or variable range (`"300 700"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<String>,
    /// `font-style` (e.g. `"normal"`, `"italic"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// `font-display` strategy. Defaults to `swap`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    /// Raw `unicode-range` value (verbatim CSS, e.g.
    /// `"U+0000-00FF, U+0131, U+2000-206F"`). Omit for a full-coverage
    /// face.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unicode_range: Option<String>,
}

impl WebFont {
    /// Emit a complete `@font-face { … }` declaration. Values are the
    /// operator's own trusted config (not end-user input), emitted
    /// verbatim — same trust model as [`TenantStyle::to_css_root_block`].
    #[must_use]
    pub fn to_font_face(&self) -> String {
        let format = self.format.as_deref().unwrap_or("woff2");
        let display = self.display.as_deref().unwrap_or("swap");
        let mut out = format!(
            "@font-face {{ font-family: \"{}\"; src: url({}) format(\"{}\"); font-display: {};",
            self.family, self.src, format, display
        );
        if let Some(w) = &self.weight {
            out.push_str(&format!(" font-weight: {w};"));
        }
        if let Some(s) = &self.style {
            out.push_str(&format!(" font-style: {s};"));
        }
        if let Some(r) = &self.unicode_range {
            out.push_str(&format!(" unicode-range: {r};"));
        }
        out.push_str(" }");
        out
    }
}

/// Wrapper used to deserialize the top-level `forge.toml` so
/// we only need to pull out the `[style]` table.
#[derive(Debug, Default, Deserialize)]
struct ForgeTomlEnvelope {
    #[serde(default)]
    style: Option<TenantStyle>,
}

impl TenantStyle {
    /// Load the `[style]` section from `<root>/forge.toml`.
    ///
    /// Fail-tolerant: ANY error returns `None`.
    #[must_use]
    pub fn load(root: &Path) -> Option<Self> {
        let path = root.join("forge.toml");
        let body = std::fs::read_to_string(&path).ok()?;
        let envelope: ForgeTomlEnvelope = toml::from_str(&body).ok()?;
        envelope.style
    }

    /// True when this tenant declares NO style overrides. The
    /// render phase can skip the CSS-injection step entirely.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.palette.is_empty()
            && self.fonts.is_empty()
            && self.radius.is_empty()
            && self.nav.is_empty()
            && self.image.is_empty()
            && self.weights.is_empty()
            && self.sizes.is_empty()
            && self.text.is_empty()
            && self.webfonts.is_empty()
            && self.density.is_none()
    }

    /// Emit the tenant's `@font-face` declarations (one per
    /// `[[style.webfonts]]` entry), newline-joined. Empty string when
    /// the tenant declares no webfonts.
    #[must_use]
    pub fn to_font_face_css(&self) -> String {
        if self.webfonts.is_empty() {
            return String::new();
        }
        let mut out = String::new();
        for wf in &self.webfonts {
            out.push_str(&wf.to_font_face());
            out.push('\n');
        }
        out
    }

    /// Full body of the external `tenant-style.css`: the `@font-face`
    /// declarations FIRST (so the families are registered before the
    /// `:root` token block names them), then the `:root { … }`
    /// overrides. This is the single source the render phase both
    /// SRI-hashes and writes to disk — they must be byte-identical or
    /// the browser rejects the stylesheet on integrity mismatch.
    #[must_use]
    pub fn to_external_css(&self) -> String {
        let mut out = self.to_font_face_css();
        out.push_str(&self.to_css_root_block());
        out
    }

    /// Generate a CSS `:root { … }` block from this tenant's
    /// declared overrides. Returns an empty string when
    /// [`Self::is_empty`].
    ///
    /// Variable naming convention:
    /// * palette keys map to `--loom-color-<key>` (kebab-case)
    /// * font keys map to `--loom-font-<key>`
    /// * radius keys map to `--loom-radius-<key>`
    /// * nav keys map to `--loom-nav-<key>`
    /// * image keys map to `--loom-img-<key>`
    /// * weight keys map to `--loom-weight-<key>`
    /// * size keys map to `--loom-size-<key>`
    /// * text keys map to `--loom-text-<key>`
    /// * density maps to `--loom-density` (single value)
    ///
    /// Values are emitted verbatim — the operator-side TOML is
    /// trusted (this is the operator's own config, not user
    /// input). No HTML or CSS escaping is performed; tenants
    /// authoring `palette.primary = "expression(alert(1))"`
    /// produce broken CSS but cannot escape the `<style>` block
    /// in a meaningful injection sense (the value is inside a
    /// `:root { … }` declaration block and the renderer wraps
    /// the whole block in a single `<style>` tag).
    #[must_use]
    pub fn to_css_root_block(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        let mut out = String::from(":root, :root[data-theme] {\n");
        for (k, v) in &self.palette {
            out.push_str(&format!("  --loom-color-{}: {} !important;\n", to_kebab(k), v));
        }
        for (k, v) in &self.fonts {
            out.push_str(&format!("  --loom-font-{}: {} !important;\n", to_kebab(k), v));
        }
        for (k, v) in &self.radius {
            out.push_str(&format!("  --loom-radius-{}: {} !important;\n", to_kebab(k), v));
        }
        for (k, v) in &self.nav {
            out.push_str(&format!("  --loom-nav-{}: {} !important;\n", to_kebab(k), v));
        }
        for (k, v) in &self.image {
            out.push_str(&format!("  --loom-img-{}: {} !important;\n", to_kebab(k), v));
        }
        for (k, v) in &self.weights {
            out.push_str(&format!("  --loom-weight-{}: {} !important;\n", to_kebab(k), v));
        }
        for (k, v) in &self.sizes {
            out.push_str(&format!("  --loom-size-{}: {} !important;\n", to_kebab(k), v));
        }
        for (k, v) in &self.text {
            out.push_str(&format!("  --loom-text-{}: {} !important;\n", to_kebab(k), v));
        }
        if let Some(d) = &self.density {
            out.push_str(&format!("  --loom-density: {d} !important;\n"));
        }
        out.push_str("}\n");
        out
    }

    /// Generate a complete `<style>…</style>` snippet ready for
    /// injection into a page-shell head. Same conditions as
    /// [`Self::to_css_root_block`]; returns empty string when
    /// there's nothing to declare.
    #[must_use]
    pub fn to_style_tag(&self) -> String {
        let block = self.to_css_root_block();
        if block.is_empty() {
            String::new()
        } else {
            format!("<style data-loom-tenant-style>\n{block}</style>\n")
        }
    }

    /// Generate an external-stylesheet `<link>` tag that references
    /// `/tenant-style.css`. Use this in place of [`Self::to_style_tag`]
    /// when the page-shell CSP `style-src` whitelist does not include
    /// the inline-style hash (which happens whenever tenant overrides
    /// are dynamic — the hash would have to be recomputed per render).
    /// `'self'` covers same-origin stylesheets without a hash. The
    /// caller is responsible for writing the CSS body to that path.
    #[must_use]
    pub fn to_link_tag(&self) -> String {
        if self.is_empty() {
            String::new()
        } else {
            "<link rel=\"stylesheet\" href=\"/tenant-style.css\" data-loom-tenant-style>\n"
                .to_owned()
        }
    }
}

/// Convert a snake_case or camelCase key to kebab-case for CSS
/// variable naming.
fn to_kebab(k: &str) -> String {
    let mut out = String::with_capacity(k.len());
    let mut prev_upper = false;
    for (i, c) in k.chars().enumerate() {
        if c == '_' {
            out.push('-');
            prev_upper = false;
            continue;
        }
        if c.is_ascii_uppercase() {
            if i > 0 && !prev_upper {
                out.push('-');
            }
            for low in c.to_lowercase() {
                out.push(low);
            }
            prev_upper = true;
        } else {
            out.push(c);
            prev_upper = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(label: &str) -> std::path::PathBuf {
        let pid = std::process::id();
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        std::env::temp_dir().join(format!("forge-tenant-style-{label}-{pid}-{n}"))
    }

    #[test]
    fn load_returns_none_when_forge_toml_missing() {
        let root = tmpdir("missing");
        std::fs::create_dir_all(&root).unwrap();
        assert!(TenantStyle::load(&root).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn load_returns_none_when_no_style_section() {
        let root = tmpdir("no-style");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("forge.toml"), "[forge]\nmode = \"poc\"\n").unwrap();
        assert!(TenantStyle::load(&root).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn load_parses_style_palette() {
        let root = tmpdir("palette");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("forge.toml"),
            r##"
[style.palette]
primary = "#733635"
accent  = "#DBA830"
"##,
        )
        .unwrap();
        let style = TenantStyle::load(&root).expect("loads");
        assert_eq!(style.palette.get("primary").unwrap(), "#733635");
        assert_eq!(style.palette.get("accent").unwrap(), "#DBA830");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn empty_style_emits_empty_css() {
        let style = TenantStyle::default();
        assert!(style.is_empty());
        assert!(style.to_css_root_block().is_empty());
        assert!(style.to_style_tag().is_empty());
    }

    #[test]
    fn palette_emits_loom_color_vars() {
        let mut style = TenantStyle::default();
        style
            .palette
            .insert("primary".to_owned(), "#733635".to_owned());
        style
            .palette
            .insert("link_hover".to_owned(), "#4A1F1F".to_owned());
        let css = style.to_css_root_block();
        assert!(css.contains("--loom-color-primary: #733635 !important;"));
        assert!(css.contains("--loom-color-link-hover: #4A1F1F !important;"));
    }

    #[test]
    fn fonts_emit_loom_font_vars() {
        let mut style = TenantStyle::default();
        style.fonts.insert(
            "display".to_owned(),
            "\"Outfit\", system-ui, sans-serif".to_owned(),
        );
        let css = style.to_css_root_block();
        assert!(css.contains("--loom-font-display: \"Outfit\", system-ui, sans-serif !important;"));
    }

    #[test]
    fn radius_emit_loom_radius_vars() {
        let mut style = TenantStyle::default();
        style.radius.insert("md".to_owned(), "10px".to_owned());
        let css = style.to_css_root_block();
        assert!(css.contains("--loom-radius-md: 10px !important;"));
    }

    #[test]
    fn nav_emits_loom_nav_vars() {
        let mut style = TenantStyle::default();
        style
            .nav
            .insert("link_color".to_owned(), "#922030".to_owned());
        style.nav.insert("link_weight".to_owned(), "700".to_owned());
        style
            .nav
            .insert("link_padding".to_owned(), "0.78rem 1.56rem".to_owned());
        let css = style.to_css_root_block();
        assert!(css.contains("--loom-nav-link-color: #922030 !important;"));
        assert!(css.contains("--loom-nav-link-weight: 700 !important;"));
        assert!(css.contains("--loom-nav-link-padding: 0.78rem 1.56rem !important;"));
    }

    #[test]
    fn image_emits_loom_img_vars() {
        let mut style = TenantStyle::default();
        style.image.insert("radius".to_owned(), "0".to_owned());
        style.image.insert("shadow".to_owned(), "none".to_owned());
        let css = style.to_css_root_block();
        assert!(css.contains("--loom-img-radius: 0 !important;"));
        assert!(css.contains("--loom-img-shadow: none !important;"));
    }

    #[test]
    fn weights_emit_loom_weight_vars() {
        let mut style = TenantStyle::default();
        style.weights.insert("display".to_owned(), "700".to_owned());
        style.weights.insert("heading".to_owned(), "600".to_owned());
        let css = style.to_css_root_block();
        assert!(css.contains("--loom-weight-display: 700 !important;"));
        assert!(css.contains("--loom-weight-heading: 600 !important;"));
    }

    #[test]
    fn sizes_emit_loom_size_vars() {
        let mut style = TenantStyle::default();
        style
            .sizes
            .insert("hero_title".to_owned(), "clamp(2.5rem, 4.8vw, 4.25rem)".to_owned());
        let css = style.to_css_root_block();
        assert!(css.contains("--loom-size-hero-title: clamp(2.5rem, 4.8vw, 4.25rem) !important;"));
    }

    #[test]
    fn text_emit_loom_text_vars() {
        let mut style = TenantStyle::default();
        style
            .text
            .insert("footer_heading_transform".to_owned(), "none".to_owned());
        style
            .text
            .insert("footer_heading_tracking".to_owned(), "normal".to_owned());
        let css = style.to_css_root_block();
        assert!(css.contains("--loom-text-footer-heading-transform: none !important;"));
        assert!(css.contains("--loom-text-footer-heading-tracking: normal !important;"));
    }

    #[test]
    fn webfont_emits_font_face_with_all_fields() {
        let wf = WebFont {
            family: "Outfit".to_owned(),
            src: "/fonts/outfit-latin.woff2".to_owned(),
            format: None,
            weight: Some("300 700".to_owned()),
            style: Some("normal".to_owned()),
            display: None,
            unicode_range: Some("U+0000-00FF, U+0131".to_owned()),
        };
        let css = wf.to_font_face();
        assert!(css.starts_with("@font-face {"));
        assert!(css.contains("font-family: \"Outfit\";"));
        assert!(css.contains("src: url(/fonts/outfit-latin.woff2) format(\"woff2\");"));
        assert!(css.contains("font-display: swap;"));
        assert!(css.contains("font-weight: 300 700;"));
        assert!(css.contains("font-style: normal;"));
        assert!(css.contains("unicode-range: U+0000-00FF, U+0131;"));
        assert!(css.trim_end().ends_with('}'));
    }

    #[test]
    fn webfont_minimal_omits_optional_fields() {
        let wf = WebFont {
            family: "Body".to_owned(),
            src: "/fonts/body.woff2".to_owned(),
            format: None,
            weight: None,
            style: None,
            display: None,
            unicode_range: None,
        };
        let css = wf.to_font_face();
        assert!(!css.contains("font-weight"));
        assert!(!css.contains("font-style"));
        assert!(!css.contains("unicode-range"));
        assert!(css.contains("font-display: swap;"));
    }

    #[test]
    fn webfonts_make_style_non_empty_and_emit_before_root() {
        let style = TenantStyle {
            webfonts: vec![WebFont {
                family: "Outfit".to_owned(),
                src: "/fonts/outfit-latin.woff2".to_owned(),
                format: None,
                weight: Some("300 700".to_owned()),
                style: None,
                display: None,
                unicode_range: None,
            }],
            ..Default::default()
        };
        assert!(!style.is_empty());
        let ext = style.to_external_css();
        // @font-face precedes the (here empty) :root block.
        assert!(ext.contains("@font-face"));
        assert!(ext.contains("font-family: \"Outfit\";"));
    }

    #[test]
    fn external_css_font_face_precedes_root_block() {
        let mut style = TenantStyle::default();
        style.palette.insert("ink".to_owned(), "#111".to_owned());
        style.webfonts.push(WebFont {
            family: "Outfit".to_owned(),
            src: "/fonts/outfit-latin.woff2".to_owned(),
            format: None,
            weight: None,
            style: None,
            display: None,
            unicode_range: None,
        });
        let ext = style.to_external_css();
        let face_at = ext.find("@font-face").expect("has @font-face");
        let root_at = ext.find(":root").expect("has :root");
        assert!(face_at < root_at, "@font-face must precede :root");
    }

    #[test]
    fn webfont_unknown_field_rejected_by_deny_unknown() {
        // deny_unknown_fields guards typos in the operator's TOML.
        let json = r#"{"family":"X","src":"/x.woff2","wieght":"700"}"#;
        assert!(serde_json::from_str::<WebFont>(json).is_err());
    }

    #[test]
    fn density_emits_single_var() {
        let style = TenantStyle {
            density: Some("dense".to_owned()),
            ..Default::default()
        };
        let css = style.to_css_root_block();
        assert!(css.contains("--loom-density: dense !important;"));
    }

    #[test]
    fn style_tag_wraps_block() {
        let mut style = TenantStyle::default();
        style.palette.insert("ink".to_owned(), "#111".to_owned());
        let tag = style.to_style_tag();
        assert!(tag.starts_with("<style data-loom-tenant-style>"));
        assert!(tag.contains("--loom-color-ink: #111 !important;"));
        assert!(tag.trim_end().ends_with("</style>"));
    }

    #[test]
    fn snake_case_key_emits_kebab_variable() {
        assert_eq!(to_kebab("primary"), "primary");
        assert_eq!(to_kebab("link_hover"), "link-hover");
        assert_eq!(to_kebab("on_primary"), "on-primary");
        assert_eq!(to_kebab("accent_2"), "accent-2");
    }

    #[test]
    fn round_trip_through_serde_json() {
        // toml crate is parse-only in this workspace; round-trip via
        // serde_json (already a workspace dep) instead.
        let mut style = TenantStyle::default();
        style
            .palette
            .insert("primary".to_owned(), "#733635".to_owned());
        style.density = Some("comfortable".to_owned());
        let json_str = serde_json::to_string(&style).expect("ser");
        let back: TenantStyle = serde_json::from_str(&json_str).expect("de");
        assert_eq!(back, style);
    }

    #[test]
    fn malformed_toml_returns_none() {
        let root = tmpdir("malformed");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("forge.toml"), "[[[ not toml").unwrap();
        assert!(TenantStyle::load(&root).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
