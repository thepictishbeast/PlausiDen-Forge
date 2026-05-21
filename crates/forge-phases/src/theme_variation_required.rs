//! `theme_variation_required` — substrate default-theme floor.
//!
//! Task #261 per the variation-architecture spec. Anchors the
//! substrate's premium-design commitment from doctrine
//! [[forge-default-themes-a11y]]: every site ships at least two
//! themes (light + dark) by default. Operators opt OUT
//! explicitly via `[theme_policy] minimum = "none"`; opting out
//! is intentional, not the default.
//!
//! Where the site_identity_conformance phase (#235) checks that
//! every `required = true` theme_variant has its corresponding
//! CSS file emitted, this phase enforces a higher contract:
//!
//! 1. Sites with a declared `[site_identity]` MUST also declare
//!    `[[site_identity.theme_variant]]` entries unless they
//!    explicitly opt out.
//! 2. The declared set MUST include both a light-family + a dark-
//!    family variant unless the policy is downgraded.
//! 3. The dark variant SHOULD be AMOLED-true-black (#000000) per
//!    memory [[dark-theme-amoled-true-black]]; this phase emits a
//!    warn (not strict) for non-amoled dark variants.
//!
//! ## forge.toml shape
//!
//! Theme policy lives at top level (sibling of `[site_identity]`)
//! rather than nested under identity — identity is the operator's
//! CLAIM about the site; theme_policy is the substrate's
//! REQUIREMENT contract.
//!
//! ```toml
//! [theme_policy]
//! minimum = "light+dark"   # default; "light+amoled" / "any" / "none"
//! amoled_dark = true       # warn if dark variant isn't named "amoled"
//! ```
//!
//! Theme-family classification:
//!
//! | Variant name(s)                    | Family    |
//! |------------------------------------|-----------|
//! | `light`, `default`, `paper`        | light     |
//! | `dark`, `amoled`, `night`, `midnight` | dark   |
//! | `high_contrast`, `accessible`      | a11y      |
//! | `sepia`, `paperwhite`              | reader    |
//! | (anything else)                    | custom    |
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * `#[non_exhaustive]` on phase struct.
//! * No unwrap/expect in non-test code.
//! * Pure walk over forge.toml.

use std::fs;
use std::path::Path;

use forge_core::site_identity::SiteIdentity;
use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// `theme_variation_required` phase.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct ThemeVariationRequiredPhase;

impl Phase for ThemeVariationRequiredPhase {
    fn name(&self) -> &'static str {
        "theme_variation_required"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let Some(identity) = SiteIdentity::load(&ctx.root) else {
            return Ok(findings);
        };
        if identity.is_default() {
            return Ok(findings);
        }

        let policy = ThemePolicy::load(&ctx.root);

        // Opt-out shortcut.
        if matches!(policy.minimum, MinimumThemeSet::None) {
            return Ok(findings);
        }

        let variants = &identity.theme_variant;
        if variants.is_empty() {
            findings.push(
                Finding::strict(
                    self.name(),
                    ctx.root.join("forge.toml").display().to_string(),
                    "theme_variation_required — [site_identity] is declared but no [[site_identity.theme_variant]] entries are present; the substrate's default-theme floor demands at least one light + one dark variant"
                        .to_owned(),
                )
                .citing(["theme-101"])
                .why("Sites ship multiple themes by default per the substrate's premium-design commitment; declaring an identity without theme variants leaves the AMOLED + light dual-theme floor unmet")
                .fix("add `[[site_identity.theme_variant]]` entries for `light` + `amoled` (recommended), OR set `[theme_policy] minimum = \"none\"` to explicitly opt out"),
            );
            return Ok(findings);
        }

        let mut have_light = false;
        let mut have_dark = false;
        let mut amoled_dark: Option<&str> = None;
        let mut non_amoled_dark: Option<&str> = None;

        for v in variants {
            match theme_family(&v.name) {
                ThemeFamily::Light => have_light = true,
                ThemeFamily::Dark => {
                    have_dark = true;
                    if v.name.eq_ignore_ascii_case("amoled") {
                        amoled_dark = Some(&v.name);
                    } else {
                        non_amoled_dark = Some(&v.name);
                    }
                }
                _ => {}
            }
        }

        match policy.minimum {
            MinimumThemeSet::Any => {}
            MinimumThemeSet::LightPlusDark | MinimumThemeSet::LightPlusAmoled => {
                if !have_light {
                    findings.push(missing_family_finding(
                        self.name(),
                        &ctx.root,
                        "light",
                        "add `[[site_identity.theme_variant]] name = \"light\" required = true`",
                    ));
                }
                if !have_dark {
                    findings.push(missing_family_finding(
                        self.name(),
                        &ctx.root,
                        "dark",
                        "add `[[site_identity.theme_variant]] name = \"amoled\" required = true` (or any other dark-family variant)",
                    ));
                }
                if matches!(policy.minimum, MinimumThemeSet::LightPlusAmoled)
                    && amoled_dark.is_none()
                    && have_dark
                {
                    findings.push(
                        Finding::strict(
                            "theme_variation_required",
                            ctx.root.join("forge.toml").display().to_string(),
                            format!(
                                "theme_variation_required — `theme_policy.minimum = \"light+amoled\"` requires an AMOLED dark variant; declared `{}` is dark-family but not AMOLED",
                                non_amoled_dark.unwrap_or("?")
                            ),
                        )
                        .citing(["theme-103"])
                        .why("the AMOLED contract requires #000000 backgrounds for battery savings on OLED + maximum contrast")
                        .fix("rename the dark variant to `amoled` OR add a separate `amoled` variant alongside it"),
                    );
                }
            }
            MinimumThemeSet::None => {}
        }

        if policy.amoled_dark && have_dark && amoled_dark.is_none() {
            findings.push(
                Finding::warn(
                    self.name(),
                    ctx.root.join("forge.toml").display().to_string(),
                    format!(
                        "theme_variation_required — `theme_policy.amoled_dark = true` recommends the dark variant be named `amoled` (declared `{}`)",
                        non_amoled_dark.unwrap_or("?")
                    ),
                )
                .citing(["theme-102"])
                .why("AMOLED conventions name the variant `amoled` so consumers recognize the #000000-background contract")
                .fix("rename the dark variant to `amoled`, OR set `theme_policy.amoled_dark = false` to silence this warn"),
            );
        }

        Ok(findings)
    }
}

fn missing_family_finding(
    phase: &'static str,
    root: &Path,
    family: &str,
    suggestion: &str,
) -> Finding {
    Finding::strict(
        phase,
        root.join("forge.toml").display().to_string(),
        format!(
            "theme_variation_required — declared theme variants do not include a `{family}`-family variant"
        ),
    )
    .citing(["theme-101"])
    .why(format!(
        "the substrate's default-theme floor demands both a light and a dark variant; the {family}-family slot is empty"
    ))
    .fix(suggestion.to_owned())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MinimumThemeSet {
    LightPlusDark,
    LightPlusAmoled,
    Any,
    None,
}

#[derive(Debug, Clone, Copy)]
struct ThemePolicy {
    minimum: MinimumThemeSet,
    amoled_dark: bool,
}

impl ThemePolicy {
    fn load(root: &Path) -> Self {
        let default = Self {
            minimum: MinimumThemeSet::LightPlusDark,
            amoled_dark: true,
        };
        let Some(body) = fs::read_to_string(root.join("forge.toml")).ok() else {
            return default;
        };
        let Ok(value) = toml::from_str::<toml::Value>(&body) else {
            return default;
        };
        let Some(policy) = value.get("theme_policy").and_then(|v| v.as_table()) else {
            return default;
        };
        let minimum = match policy.get("minimum").and_then(|v| v.as_str()) {
            Some("light+amoled") => MinimumThemeSet::LightPlusAmoled,
            Some("any") => MinimumThemeSet::Any,
            Some("none") => MinimumThemeSet::None,
            _ => MinimumThemeSet::LightPlusDark,
        };
        let amoled_dark = policy
            .get("amoled_dark")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        Self {
            minimum,
            amoled_dark,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeFamily {
    Light,
    Dark,
    A11y,
    Reader,
    Custom,
}

fn theme_family(name: &str) -> ThemeFamily {
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "light" | "default" | "paper" | "paperwhite_light" => ThemeFamily::Light,
        "dark" | "amoled" | "night" | "midnight" | "ink" => ThemeFamily::Dark,
        "high_contrast" | "accessible" | "a11y" => ThemeFamily::A11y,
        "sepia" | "paperwhite" | "reader" => ThemeFamily::Reader,
        _ => ThemeFamily::Custom,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("forge-theme-req-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn ctx_for(root: &Path) -> BuildCtx {
        BuildCtx {
            root: root.to_path_buf(),
            static_dir: root.join("static"),
            mode: forge_core::BuildMode::Poc,
        }
    }

    #[test]
    fn phase_silent_when_no_identity_declared() {
        let root = temp_root("no-identity");
        let findings = ThemeVariationRequiredPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_under_policy_none() {
        let root = temp_root("opt-out");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]
site_id = "x"

[theme_policy]
minimum = "none"
"#,
        )
        .unwrap();
        let findings = ThemeVariationRequiredPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_missing_theme_variants_with_identity() {
        let root = temp_root("none-declared");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]
site_id = "x"
"#,
        )
        .unwrap();
        let findings = ThemeVariationRequiredPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f
                .message
                .contains("no [[site_identity.theme_variant]] entries are present")),
            "expected missing-variants finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_missing_dark_family() {
        let root = temp_root("light-only");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]
site_id = "x"

[[site_identity.theme_variant]]
name = "light"
required = true
"#,
        )
        .unwrap();
        let findings = ThemeVariationRequiredPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("`dark`-family variant")),
            "expected dark-missing finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_with_light_plus_amoled() {
        let root = temp_root("complete");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]
site_id = "x"

[[site_identity.theme_variant]]
name = "light"
required = true

[[site_identity.theme_variant]]
name = "amoled"
required = true
"#,
        )
        .unwrap();
        let findings = ThemeVariationRequiredPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.is_empty(),
            "complete light+amoled should pass; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_warns_when_dark_is_not_amoled() {
        let root = temp_root("dark-not-amoled");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]
site_id = "x"

[[site_identity.theme_variant]]
name = "light"
required = true

[[site_identity.theme_variant]]
name = "dark"
required = true
"#,
        )
        .unwrap();
        let findings = ThemeVariationRequiredPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f
                .message
                .contains("recommends the dark variant be named `amoled`")),
            "expected amoled-naming warn; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_strict_under_light_plus_amoled_when_dark_not_amoled() {
        let root = temp_root("amoled-required");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]
site_id = "x"

[theme_policy]
minimum = "light+amoled"

[[site_identity.theme_variant]]
name = "light"
required = true

[[site_identity.theme_variant]]
name = "dark"
required = true
"#,
        )
        .unwrap();
        let findings = ThemeVariationRequiredPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("requires an AMOLED dark variant")),
            "expected amoled-required strict; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn theme_family_classification() {
        assert_eq!(theme_family("light"), ThemeFamily::Light);
        assert_eq!(theme_family("Default"), ThemeFamily::Light);
        assert_eq!(theme_family("AMOLED"), ThemeFamily::Dark);
        assert_eq!(theme_family("dark"), ThemeFamily::Dark);
        assert_eq!(theme_family("sepia"), ThemeFamily::Reader);
        assert_eq!(theme_family("high_contrast"), ThemeFamily::A11y);
        assert_eq!(theme_family("brand_blue"), ThemeFamily::Custom);
    }
}
