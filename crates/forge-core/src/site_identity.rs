//! `site_identity` — typed loader for the `[site_identity]`
//! section of `forge.toml`.
//!
//! Task #234 per the variation-architecture spec. Declarative
//! shape stating "this is what this site IS." Consumed by the
//! conformance audit (#235), the voice-profile audit (#241),
//! the token-cascade gate (#242), the mood lock (#243), the
//! vocabulary-coherence audit (#244), the theme-variation
//! requirement (#261), and the differentiation-budget multi-
//! dimensional check (#237).
//!
//! Per AVP-2 + memory `feedback_backward_compat_version_discipline`:
//! the identity carries a spec version so future schema
//! additions don't break existing sites. Per memory
//! `feedback_no_meta_narration`: this module is data, not
//! behavior — phases interpret the declared identity, this
//! module just loads it.
//!
//! ## Layering relative to substrate baselines
//!
//! Site identity is DECLARATIVE — it says what the site claims
//! to be. The baseline substrate defines the universe of
//! valid identities; this struct is the operator's pick from
//! that universe. Layering rules:
//!
//! | Field                    | Substrate default              | Override semantics              |
//! |--------------------------|--------------------------------|---------------------------------|
//! | `voice_profile`          | per-corpus statistical baseline| operator declares target tier   |
//! | `mood`                   | unconstrained                  | operator declares mood lock     |
//! | `density_preference`     | per-page heuristic             | site-wide override of heuristic |
//! | `token_override_budget`  | unbounded                      | caps the budget                 |
//! | `allowed_primitives`     | all                            | whitelist; refuses others       |
//! | `forbidden_primitives`   | none                           | blacklist; refuses listed       |
//! | `content_type_taxonomy`  | inferred from cms/ structure   | operator-declared explicit list |
//! | `theme_variants`         | light + dark                   | operator can declare more       |
//!
//! ## API
//!
//! Call [`SiteIdentity::load`] with the project root. Returns
//! `Option<SiteIdentity>`:
//!
//! * `Some(identity)` — section present + parsed.
//! * `None` — no `forge.toml` OR no `[site_identity]` section
//!   OR malformed TOML. Phases that consume identity MUST be
//!   tolerant of `None`: a site without a declared identity is
//!   not gated by the identity audits.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"` (inherited).
//! * No unwrap/expect in non-test code.
//! * `#[non_exhaustive]` on every public struct so future
//!   fields don't break consumers.
//! * Fail-tolerant load — every error path returns `None`.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Versioned spec marker. Incremented when the identity schema
/// changes in a way that consumers must opt into. Per the
/// backward-compat doctrine, additive changes don't bump the
/// version; breaking changes do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum IdentitySpec {
    /// First-cut schema. All current fields belong to v1.
    #[default]
    V1,
}

/// Voice-profile statistical targets. Consumed by the voice
/// audit (#241) to verify the site's actual content matches its
/// declared voice.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
#[non_exhaustive]
pub struct VoiceProfile {
    /// Target reading-grade tier. One of `"plain"`, `"casual"`,
    /// `"professional"`, `"technical"`, `"academic"`. Consumer
    /// parses into its own enum.
    pub tier: Option<String>,
    /// Maximum acceptable average sentence length in words.
    /// 0 = unset.
    pub max_avg_sentence_words: u32,
    /// Vocabulary tier hash for jargon-density gating. Free-form;
    /// the voice audit resolves the meaning.
    pub vocabulary_tier: Option<String>,
}

/// Mood-lock declaration. Consumed by the mood-lock audit (#243).
/// Sites that declare a mood refuse builds where aggregate
/// aesthetic measurements drift outside the declared mood.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
#[non_exhaustive]
pub struct MoodLock {
    /// Primary mood label. One of `"editorial"`, `"industrial"`,
    /// `"organic"`, `"minimal"`, `"kinetic"`, `"archival"`,
    /// `"playful"`, `"severe"`. Consumer maps this to typography
    /// + spacing + treatment baselines.
    pub primary: Option<String>,
    /// Optional secondary mood for blended sites.
    pub secondary: Option<String>,
    /// Acceptable drift radius around the primary mood (0–100).
    /// 0 = no drift permitted; 100 = mood is descriptive only.
    pub drift_budget: u8,
}

/// Token-override-budget declaration. Consumed by the
/// hierarchical-token-cascade check (#242).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
#[non_exhaustive]
pub struct TokenBudget {
    /// Maximum number of design tokens a single page can override
    /// from the site-wide defaults. Beyond this the page is
    /// considered to be defining a separate visual identity, which
    /// the conformance audit refuses.
    pub max_per_page_overrides: u32,
    /// Maximum total number of distinct token-override values
    /// across the whole site. Caps vocabulary fragmentation.
    pub max_site_distinct_overrides: u32,
}

/// One theme variant declaration. Consumed by the theme-variation
/// requirement audit (#261).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
#[non_exhaustive]
pub struct ThemeVariant {
    /// Variant name. Conventional values: `"light"`, `"dark"`,
    /// `"amoled"`, `"high_contrast"`, `"sepia"`. Free-form to
    /// allow custom variants.
    pub name: String,
    /// Whether the variant is required (build fails if not
    /// produced) or optional.
    #[serde(default)]
    pub required: bool,
}

/// Content-type entry. Consumed by the page-type-library check
/// (#250) and the composition-zone-constraints audit (#254).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
#[non_exhaustive]
pub struct ContentType {
    /// Slug identifier (e.g. `"homepage"`, `"blog_post"`,
    /// `"product_page"`).
    pub slug: String,
    /// Glob pattern matching cms/ paths that are instances of
    /// this content type (e.g. `"cms/blog/*.json"`).
    pub pattern: String,
    /// Optional human-readable description.
    #[serde(default)]
    pub description: Option<String>,
}

/// The declared identity of a site.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
#[non_exhaustive]
pub struct SiteIdentity {
    /// Schema version. Bumped on breaking changes.
    pub spec: IdentitySpec,
    /// Stable site identifier. Used as the registry's site_id
    /// key (#232). Recommended to be a hostname or slug.
    pub site_id: Option<String>,
    /// Optional tenant identifier. Pairs with the cross-site
    /// uniqueness gate's per-tenant scope.
    pub tenant_id: Option<String>,
    /// Voice-profile declaration.
    pub voice: VoiceProfile,
    /// Mood-lock declaration.
    pub mood: MoodLock,
    /// Site-wide density preference. One of `"sparse"`,
    /// `"comfortable"`, `"dense"`, `"extreme"`. Overrides the
    /// per-page heuristic for the whole site.
    pub density_preference: Option<String>,
    /// Token-budget declaration.
    pub tokens: TokenBudget,
    /// Whitelist of allowed CmsSection kinds (e.g. `"hero_editorial"`).
    /// Empty = no whitelist (all allowed).
    pub allowed_primitives: Vec<String>,
    /// Blacklist of forbidden CmsSection kinds. Refused even
    /// if also whitelisted (forbidden wins).
    pub forbidden_primitives: Vec<String>,
    /// Declared content-type taxonomy. Each entry maps a glob
    /// pattern to a content-type slug.
    pub content_type: Vec<ContentType>,
    /// Declared theme variants. Builds with `required = true`
    /// must produce the variant.
    pub theme_variant: Vec<ThemeVariant>,
}

/// Wrapper used to deserialize the top-level `forge.toml` so we
/// only have to read the `[site_identity]` table, not the others.
#[derive(Debug, Default, Deserialize)]
struct ForgeTomlEnvelope {
    #[serde(default)]
    site_identity: Option<SiteIdentity>,
}

impl SiteIdentity {
    /// Load the `[site_identity]` section from `<root>/forge.toml`.
    ///
    /// Fail-tolerant: ANY error path returns `None`. Phases that
    /// consume identity MUST treat `None` as "no identity declared"
    /// and skip the identity audits.
    #[must_use]
    pub fn load(root: &Path) -> Option<Self> {
        let path = root.join("forge.toml");
        let body = std::fs::read_to_string(&path).ok()?;
        let envelope: ForgeTomlEnvelope = toml::from_str(&body).ok()?;
        envelope.site_identity
    }

    /// `true` iff this identity carries no declarations beyond
    /// default. Useful for early-out in phases.
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == SiteIdentity::default()
    }

    /// Returns the content-type slug whose pattern matches the
    /// given cms-relative path. Patterns are glob-style; this is
    /// a simple match using the `glob_match` helper. Returns
    /// `None` if no pattern matches.
    #[must_use]
    pub fn content_type_for(&self, cms_path: &str) -> Option<&str> {
        for ct in &self.content_type {
            if glob_match(&ct.pattern, cms_path) {
                return Some(&ct.slug);
            }
        }
        None
    }

    /// Returns `true` if the given primitive kind is allowed
    /// under this identity. A primitive is allowed when:
    /// * it is not in `forbidden_primitives`, AND
    /// * `allowed_primitives` is empty OR it is listed there.
    #[must_use]
    pub fn is_primitive_allowed(&self, kind: &str) -> bool {
        if self.forbidden_primitives.iter().any(|f| f == kind) {
            return false;
        }
        if self.allowed_primitives.is_empty() {
            return true;
        }
        self.allowed_primitives.iter().any(|a| a == kind)
    }

    /// Required theme variants (the ones with `required = true`).
    #[must_use]
    pub fn required_themes(&self) -> Vec<&str> {
        self.theme_variant
            .iter()
            .filter(|t| t.required)
            .map(|t| t.name.as_str())
            .collect()
    }
}

/// Minimal glob matcher: supports `*` (zero or more any-char)
/// and `?` (exactly one any-char). Implementation is a simple
/// recursive matcher — patterns are short (glob over file paths)
/// so the recursion depth is bounded.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    glob_match_inner(&p, 0, &t, 0)
}

fn glob_match_inner(p: &[char], pi: usize, t: &[char], ti: usize) -> bool {
    if pi == p.len() {
        return ti == t.len();
    }
    match p[pi] {
        '*' => {
            // Skip the star and try every position.
            for split in ti..=t.len() {
                if glob_match_inner(p, pi + 1, t, split) {
                    return true;
                }
            }
            false
        }
        '?' => {
            if ti < t.len() {
                glob_match_inner(p, pi + 1, t, ti + 1)
            } else {
                false
            }
        }
        c => {
            if ti < t.len() && t[ti] == c {
                glob_match_inner(p, pi + 1, t, ti + 1)
            } else {
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let p = env::temp_dir().join(format!(
            "forge-site-identity-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).expect("temp dir creation");
        p
    }

    fn write_forge_toml(dir: &Path, body: &str) {
        fs::write(dir.join("forge.toml"), body).expect("write forge.toml");
    }

    #[test]
    fn load_returns_none_when_no_forge_toml() {
        let dir = temp_dir("no-toml");
        assert!(SiteIdentity::load(&dir).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_returns_none_when_no_site_identity_section() {
        let dir = temp_dir("no-section");
        write_forge_toml(&dir, "[other]\nfoo = 1\n");
        assert!(SiteIdentity::load(&dir).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_returns_some_default_when_section_present_but_empty() {
        let dir = temp_dir("empty-section");
        write_forge_toml(&dir, "[site_identity]\n");
        let id = SiteIdentity::load(&dir).expect("section present");
        assert!(id.is_default());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_voice_round_trip() {
        let dir = temp_dir("voice");
        write_forge_toml(
            &dir,
            r#"
[site_identity]
site_id = "prosperityclub.com"

[site_identity.voice]
tier = "editorial"
max_avg_sentence_words = 22
vocabulary_tier = "professional"
"#,
        );
        let id = SiteIdentity::load(&dir).expect("loads");
        assert_eq!(id.site_id.as_deref(), Some("prosperityclub.com"));
        assert_eq!(id.voice.tier.as_deref(), Some("editorial"));
        assert_eq!(id.voice.max_avg_sentence_words, 22);
        assert_eq!(id.voice.vocabulary_tier.as_deref(), Some("professional"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_mood_round_trip() {
        let dir = temp_dir("mood");
        write_forge_toml(
            &dir,
            r#"
[site_identity.mood]
primary = "editorial"
secondary = "archival"
drift_budget = 12
"#,
        );
        let id = SiteIdentity::load(&dir).expect("loads");
        assert_eq!(id.mood.primary.as_deref(), Some("editorial"));
        assert_eq!(id.mood.secondary.as_deref(), Some("archival"));
        assert_eq!(id.mood.drift_budget, 12);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_tokens_round_trip() {
        let dir = temp_dir("tokens");
        write_forge_toml(
            &dir,
            r#"
[site_identity.tokens]
max_per_page_overrides = 3
max_site_distinct_overrides = 24
"#,
        );
        let id = SiteIdentity::load(&dir).expect("loads");
        assert_eq!(id.tokens.max_per_page_overrides, 3);
        assert_eq!(id.tokens.max_site_distinct_overrides, 24);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_primitives_and_content_types_round_trip() {
        let dir = temp_dir("primitives");
        write_forge_toml(
            &dir,
            r#"
[site_identity]
allowed_primitives = ["hero_editorial", "kv_pair", "pull_quote"]
forbidden_primitives = ["hero", "feature_spotlight"]

[[site_identity.content_type]]
slug = "homepage"
pattern = "cms/index.json"

[[site_identity.content_type]]
slug = "blog_post"
pattern = "cms/blog/*.json"
description = "Long-form editorial content"
"#,
        );
        let id = SiteIdentity::load(&dir).expect("loads");
        assert_eq!(id.allowed_primitives.len(), 3);
        assert!(id.forbidden_primitives.iter().any(|f| f == "hero"));
        assert_eq!(id.content_type.len(), 2);
        assert_eq!(id.content_type_for("cms/index.json"), Some("homepage"));
        assert_eq!(
            id.content_type_for("cms/blog/post-1.json"),
            Some("blog_post")
        );
        assert_eq!(id.content_type_for("cms/marketing/page.json"), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn is_primitive_allowed_respects_whitelist_and_blacklist() {
        let mut id = SiteIdentity::default();
        // Empty whitelist + empty blacklist → all allowed.
        assert!(id.is_primitive_allowed("hero"));
        // Add to blacklist.
        id.forbidden_primitives.push("hero".into());
        assert!(!id.is_primitive_allowed("hero"));
        assert!(id.is_primitive_allowed("hero_editorial"));
        // Add to whitelist (non-empty).
        id.allowed_primitives.push("hero_editorial".into());
        assert!(id.is_primitive_allowed("hero_editorial"));
        assert!(!id.is_primitive_allowed("kv_pair")); // not on whitelist
                                                      // Forbidden wins even if also on whitelist.
        id.allowed_primitives.push("hero".into());
        assert!(!id.is_primitive_allowed("hero"));
    }

    #[test]
    fn load_theme_variants_round_trip() {
        let dir = temp_dir("themes");
        write_forge_toml(
            &dir,
            r#"
[[site_identity.theme_variant]]
name = "light"
required = true

[[site_identity.theme_variant]]
name = "amoled"
required = true

[[site_identity.theme_variant]]
name = "high_contrast"
required = false
"#,
        );
        let id = SiteIdentity::load(&dir).expect("loads");
        assert_eq!(id.theme_variant.len(), 3);
        let req = id.required_themes();
        assert_eq!(req.len(), 2);
        assert!(req.contains(&"light"));
        assert!(req.contains(&"amoled"));
        assert!(!req.contains(&"high_contrast"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_returns_none_on_malformed_toml() {
        let dir = temp_dir("malformed");
        write_forge_toml(&dir, "[site_identity\nbroken");
        assert!(SiteIdentity::load(&dir).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_returns_none_on_unknown_subfield() {
        let dir = temp_dir("unknown");
        write_forge_toml(
            &dir,
            r#"
[site_identity]
surprise_field = "boom"
"#,
        );
        // deny_unknown_fields on SiteIdentity → parse fails → None.
        assert!(SiteIdentity::load(&dir).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn glob_match_handles_common_patterns() {
        assert!(glob_match("cms/*.json", "cms/index.json"));
        assert!(glob_match("cms/blog/*.json", "cms/blog/post.json"));
        assert!(!glob_match("cms/blog/*.json", "cms/marketing/page.json"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("", ""));
        assert!(!glob_match("", "x"));
        assert!(glob_match("a?c", "abc"));
        assert!(!glob_match("a?c", "abbc"));
    }

    #[test]
    fn full_identity_round_trip_all_fields() {
        let dir = temp_dir("full");
        write_forge_toml(
            &dir,
            r#"
[site_identity]
spec = "v1"
site_id = "prosperityclub.com"
tenant_id = "acme"
density_preference = "dense"
allowed_primitives = ["hero_editorial", "kv_pair"]
forbidden_primitives = ["hero"]

[site_identity.voice]
tier = "editorial"
max_avg_sentence_words = 24
vocabulary_tier = "professional"

[site_identity.mood]
primary = "editorial"
drift_budget = 10

[site_identity.tokens]
max_per_page_overrides = 4
max_site_distinct_overrides = 32

[[site_identity.content_type]]
slug = "homepage"
pattern = "cms/index.json"

[[site_identity.theme_variant]]
name = "light"
required = true

[[site_identity.theme_variant]]
name = "amoled"
required = true
"#,
        );
        let id = SiteIdentity::load(&dir).expect("loads");
        assert_eq!(id.spec, IdentitySpec::V1);
        assert_eq!(id.site_id.as_deref(), Some("prosperityclub.com"));
        assert_eq!(id.tenant_id.as_deref(), Some("acme"));
        assert_eq!(id.density_preference.as_deref(), Some("dense"));
        assert_eq!(id.allowed_primitives.len(), 2);
        assert_eq!(id.forbidden_primitives.len(), 1);
        assert_eq!(id.voice.tier.as_deref(), Some("editorial"));
        assert_eq!(id.mood.primary.as_deref(), Some("editorial"));
        assert_eq!(id.tokens.max_per_page_overrides, 4);
        assert_eq!(id.content_type.len(), 1);
        assert_eq!(id.required_themes(), vec!["light", "amoled"]);
        assert!(!id.is_default());
        let _ = fs::remove_dir_all(&dir);
    }
}
