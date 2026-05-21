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
    /// `[style.density]` — density tier override (`sparse` /
    /// `comfortable` / `dense` / `extreme`). Optional; substrate
    /// default is `comfortable`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density: Option<String>,
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
            && self.density.is_none()
    }

    /// Generate a CSS `:root { … }` block from this tenant's
    /// declared overrides. Returns an empty string when
    /// [`Self::is_empty`].
    ///
    /// Variable naming convention:
    /// * palette keys map to `--loom-color-<key>` (kebab-case)
    /// * font keys map to `--loom-font-<key>`
    /// * radius keys map to `--loom-radius-<key>`
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
        let mut out = String::from(":root {\n");
        for (k, v) in &self.palette {
            out.push_str(&format!("  --loom-color-{}: {};\n", to_kebab(k), v));
        }
        for (k, v) in &self.fonts {
            out.push_str(&format!("  --loom-font-{}: {};\n", to_kebab(k), v));
        }
        for (k, v) in &self.radius {
            out.push_str(&format!("  --loom-radius-{}: {};\n", to_kebab(k), v));
        }
        if let Some(d) = &self.density {
            out.push_str(&format!("  --loom-density: {d};\n"));
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
        assert!(css.contains("--loom-color-primary: #733635;"));
        assert!(css.contains("--loom-color-link-hover: #4A1F1F;"));
    }

    #[test]
    fn fonts_emit_loom_font_vars() {
        let mut style = TenantStyle::default();
        style.fonts.insert(
            "display".to_owned(),
            "\"Outfit\", system-ui, sans-serif".to_owned(),
        );
        let css = style.to_css_root_block();
        assert!(css.contains("--loom-font-display: \"Outfit\", system-ui, sans-serif;"));
    }

    #[test]
    fn radius_emit_loom_radius_vars() {
        let mut style = TenantStyle::default();
        style.radius.insert("md".to_owned(), "10px".to_owned());
        let css = style.to_css_root_block();
        assert!(css.contains("--loom-radius-md: 10px;"));
    }

    #[test]
    fn density_emits_single_var() {
        let style = TenantStyle {
            density: Some("dense".to_owned()),
            ..Default::default()
        };
        let css = style.to_css_root_block();
        assert!(css.contains("--loom-density: dense;"));
    }

    #[test]
    fn style_tag_wraps_block() {
        let mut style = TenantStyle::default();
        style.palette.insert("ink".to_owned(), "#111".to_owned());
        let tag = style.to_style_tag();
        assert!(tag.starts_with("<style data-loom-tenant-style>"));
        assert!(tag.contains("--loom-color-ink: #111;"));
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
