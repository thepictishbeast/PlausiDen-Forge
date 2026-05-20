//! `site_identity_conformance` — verifies the built site matches
//! its declared `[site_identity]` in `forge.toml`.
//!
//! Task #235 per the variation-architecture spec. First end-to-end
//! consumer of `forge_core::site_identity` (#234). The gate verifies
//! the operator's declaration matches the actual content. Silent
//! when no identity is declared (back-compat); strict findings on
//! drift when it is.
//!
//! ## What it checks
//!
//! Given a declared `SiteIdentity`, the phase verifies:
//!
//! 1. **Voice profile** — actual average sentence length across
//!    body text doesn't exceed the declared
//!    `voice.max_avg_sentence_words` (if non-zero).
//!
//! 2. **Allowed/forbidden primitives** — every section's `kind`
//!    field passes `SiteIdentity::is_primitive_allowed`. Each
//!    violation emits a strict finding.
//!
//! 3. **Content-type coverage** — every `cms/*.json` matches at
//!    least one declared `content_type` pattern when the taxonomy
//!    is declared (non-empty). Unmatched files emit warnings.
//!
//! 4. **Required theme variants** — when `theme_variant` entries
//!    with `required = true` are declared, the corresponding
//!    theme files in `static/css/` are checked. Each missing
//!    required theme emits a strict finding.
//!
//! ## forge.toml
//!
//! No phase-specific config. The phase runs against whatever
//! `[site_identity]` declares. If no identity is declared the
//! phase is silent.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * `#[non_exhaustive]` on the phase struct.
//! * No unwrap/expect in non-test code.
//! * Pure walk over JSON; read-only filesystem.

use std::fs;
use std::path::Path;

use forge_core::site_identity::SiteIdentity;
use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde_json::Value;

/// `site_identity_conformance` phase.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct SiteIdentityConformancePhase;

impl Phase for SiteIdentityConformancePhase {
    fn name(&self) -> &'static str {
        "site_identity_conformance"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let Some(identity) = SiteIdentity::load(&ctx.root) else {
            return Ok(findings);
        };
        if identity.is_default() {
            return Ok(findings);
        }

        let cms_dir = ctx.root.join("cms");
        if cms_dir.is_dir() {
            check_cms(&cms_dir, &identity, &mut findings, self.name())?;
        }

        check_required_themes(&ctx.static_dir, &identity, &mut findings, self.name());

        Ok(findings)
    }
}

fn check_cms(
    cms_dir: &Path,
    identity: &SiteIdentity,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) -> Result<(), BuildError> {
    let mut total_sentences: u64 = 0;
    let mut total_words: u64 = 0;

    let entries = fs::read_dir(cms_dir).map_err(|e| BuildError::Io {
        context: format!("read_dir {}", cms_dir.display()),
        source: e,
    })?;
    let has_taxonomy = !identity.content_type.is_empty();

    for entry in entries {
        let entry = entry.map_err(|e| BuildError::Io {
            context: format!("read_dir entry in {}", cms_dir.display()),
            source: e,
        })?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&path).map_err(|e| BuildError::Io {
            context: format!("read {}", path.display()),
            source: e,
        })?;
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let path_disp = path.display().to_string();

        // Content-type coverage.
        if has_taxonomy {
            let rel = path
                .strip_prefix(cms_dir.parent().unwrap_or(Path::new("")))
                .ok()
                .and_then(|p| p.to_str())
                .map(str::to_owned)
                .unwrap_or_else(|| path_disp.clone());
            // Try both "cms/<name>" and the raw filename as match candidates.
            let alt_rel = format!(
                "cms/{}",
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
            );
            if identity.content_type_for(&rel).is_none()
                && identity.content_type_for(&alt_rel).is_none()
            {
                findings.push(
                    Finding::warn(
                        phase,
                        path_disp.clone(),
                        format!(
                            "site_identity_conformance — `{}` doesn't match any declared content_type pattern; declare a content_type for it or remove the file",
                            path_disp
                        ),
                    )
                    .citing(["ident-001"])
                    .why("the site declares a content-type taxonomy but this page isn't covered by any pattern")
                    .fix("add a [[site_identity.content_type]] entry whose pattern matches this path"),
                );
            }
        }

        let Some(sections) = value.get("sections").and_then(|s| s.as_array()) else {
            continue;
        };

        for (idx, section) in sections.iter().enumerate() {
            let Some(kind) = section.get("kind").and_then(|v| v.as_str()) else {
                continue;
            };
            let where_at = format!("{path_disp}#section-{idx}-{kind}");

            // Primitive allow/deny.
            if !identity.is_primitive_allowed(kind) {
                let reason = if identity.forbidden_primitives.iter().any(|f| f == kind) {
                    "forbidden_primitives"
                } else {
                    "not in allowed_primitives whitelist"
                };
                findings.push(
                    Finding::strict(
                        phase,
                        where_at.clone(),
                        format!(
                            "site_identity_conformance — primitive `{}` violates declared identity ({})",
                            kind, reason
                        ),
                    )
                    .citing(["ident-002"])
                    .why("the site's declared identity refuses this primitive but the CMS uses it")
                    .fix(
                        "either remove the primitive from this section, or amend [site_identity] allowed_primitives / forbidden_primitives",
                    ),
                );
            }

            // Voice profile accumulators.
            for field in &["title", "body", "lede", "subtitle", "message", "summary"] {
                if let Some(text) = section.get(field).and_then(|v| v.as_str()) {
                    let (sentences, words) = count_sentences_words(text);
                    total_sentences = total_sentences.saturating_add(sentences as u64);
                    total_words = total_words.saturating_add(words as u64);
                }
            }
        }
    }

    // Voice profile check (only if declared).
    let max_avg = u64::from(identity.voice.max_avg_sentence_words);
    if max_avg > 0 && total_sentences > 0 {
        let avg = total_words / total_sentences;
        if avg > max_avg {
            findings.push(
                Finding::strict(
                    "site_identity_conformance",
                    cms_dir.display().to_string(),
                    format!(
                        "site_identity_conformance — average sentence length is {avg} words (declared max {max_avg}); voice profile not honored"
                    ),
                )
                .citing(["ident-003"])
                .why("the site declared a voice profile with a sentence-length ceiling, but the actual body text exceeds it on average")
                .fix("shorten sentences in the cms/ body fields, OR raise [site_identity.voice].max_avg_sentence_words to match the actual register"),
            );
        }
    }

    Ok(())
}

fn check_required_themes(
    static_dir: &Path,
    identity: &SiteIdentity,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let required = identity.required_themes();
    if required.is_empty() {
        return;
    }
    // Theme variants are conventionally css files at static/css/<name>.css
    // OR static/<name>.css for flat layouts. Check both.
    for name in required {
        let candidates = [
            static_dir.join("css").join(format!("{name}.css")),
            static_dir.join(format!("{name}.css")),
            static_dir
                .join("themes")
                .join(format!("{name}.css")),
        ];
        let found = candidates.iter().any(|p| p.exists());
        if !found {
            findings.push(
                Finding::strict(
                    phase,
                    static_dir.display().to_string(),
                    format!(
                        "site_identity_conformance — required theme variant `{name}` missing from static/ (looked in css/, themes/, and root)"
                    ),
                )
                .citing(["ident-004"])
                .why("the site declared this theme variant as required but the build didn't produce it")
                .fix(format!("emit static/css/{name}.css OR static/themes/{name}.css during the theme build, OR mark the variant as required=false")),
            );
        }
    }
}

/// Rough sentence + word count for a body-text field. Sentences
/// are split on `.`, `!`, `?`. Words are non-empty whitespace-
/// separated tokens.
fn count_sentences_words(text: &str) -> (u32, u32) {
    let sentences = text
        .split(|c: char| c == '.' || c == '!' || c == '?')
        .filter(|s| !s.trim().is_empty())
        .count();
    let words = text
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .count();
    (
        u32::try_from(sentences).unwrap_or(u32::MAX),
        u32::try_from(words).unwrap_or(u32::MAX),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "forge-ident-conf-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("cms")).unwrap();
        fs::create_dir_all(p.join("static").join("css")).unwrap();
        p
    }

    fn write_cms(root: &Path, name: &str, body: &str) {
        fs::write(root.join("cms").join(name), body).unwrap();
    }

    fn ctx_for(root: &Path) -> BuildCtx {
        BuildCtx {
            root: root.to_path_buf(),
            static_dir: root.join("static"),
            mode: forge_core::BuildMode::Poc,
        }
    }

    #[test]
    fn phase_is_silent_when_no_identity_declared() {
        let root = temp_root("no-id");
        write_cms(&root, "index.json", r#"{"sections":[]}"#);
        let findings = SiteIdentityConformancePhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_refuses_forbidden_primitive() {
        let root = temp_root("forbidden");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]
forbidden_primitives = ["hero"]
"#,
        )
        .unwrap();
        write_cms(
            &root,
            "index.json",
            r#"{"sections":[{"kind":"hero","title":"X"}]}"#,
        );
        let findings = SiteIdentityConformancePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("`hero` violates declared identity")),
            "expected forbidden-primitive finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_refuses_non_whitelisted_primitive() {
        let root = temp_root("not-whitelisted");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]
allowed_primitives = ["hero_editorial", "kv_pair"]
"#,
        )
        .unwrap();
        write_cms(
            &root,
            "index.json",
            r#"{"sections":[
              {"kind":"hero_editorial","title":"OK"},
              {"kind":"feature_spotlight","columns":3}
            ]}"#,
        );
        let findings = SiteIdentityConformancePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("`feature_spotlight`")),
            "expected feature_spotlight finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_unmatched_content_type_when_taxonomy_declared() {
        let root = temp_root("taxonomy");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]
[[site_identity.content_type]]
slug = "homepage"
pattern = "cms/index.json"
"#,
        )
        .unwrap();
        write_cms(&root, "index.json", r#"{"sections":[]}"#);
        write_cms(&root, "about.json", r#"{"sections":[]}"#);
        let findings = SiteIdentityConformancePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("about.json")
                    && f.message
                        .contains("doesn't match any declared content_type")),
            "expected unmatched-content-type finding for about.json; got: {findings:#?}"
        );
        // index.json should NOT be flagged.
        assert!(!findings
            .iter()
            .any(|f| f.message.contains("index.json")
                && f.message.contains("doesn't match")));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_voice_violation() {
        let root = temp_root("voice");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity.voice]
max_avg_sentence_words = 8
"#,
        )
        .unwrap();
        write_cms(
            &root,
            "index.json",
            r#"{"sections":[{"kind":"paragraph","body":"This is a very long sentence with way more than eight words to test the voice profile check works correctly."}]}"#,
        );
        let findings = SiteIdentityConformancePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("voice profile not honored")),
            "expected voice-profile finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_missing_required_theme() {
        let root = temp_root("missing-theme");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]
[[site_identity.theme_variant]]
name = "amoled"
required = true

[[site_identity.theme_variant]]
name = "light"
required = true
"#,
        )
        .unwrap();
        // Provide light.css but not amoled.css.
        fs::write(root.join("static").join("css").join("light.css"), "/* */").unwrap();
        let findings = SiteIdentityConformancePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("`amoled` missing")),
            "expected missing-amoled finding; got: {findings:#?}"
        );
        // light.css IS present — no finding for it.
        assert!(!findings
            .iter()
            .any(|f| f.message.contains("`light` missing")));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_required_theme_present() {
        let root = temp_root("theme-present");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]
[[site_identity.theme_variant]]
name = "dark"
required = true
"#,
        )
        .unwrap();
        fs::write(root.join("static").join("css").join("dark.css"), "/* */").unwrap();
        let findings = SiteIdentityConformancePhase.run(&ctx_for(&root)).unwrap();
        assert!(!findings.iter().any(|f| f.message.contains("missing")));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn count_sentences_words_basic() {
        let (s, w) = count_sentences_words("Hello world. Foo bar baz!");
        assert_eq!(s, 2);
        assert_eq!(w, 5);
    }
}
