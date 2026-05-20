//! `tenant_corpus` — typed loader for the `[tenant_corpus]`
//! section of `forge.toml`.
//!
//! Implements the doctrine in
//! [`docs/PER_TENANT_CORPORA.md`](../../../docs/PER_TENANT_CORPORA.md):
//! substrate corpora are ADDITIVE; tenants extend via
//! `forge.toml [tenant_corpus]` without modifying the curated
//! baseline.
//!
//! ## API
//!
//! Call [`TenantCorpus::load`] with the project root.
//! Returns `Option<TenantCorpus>`:
//!
//! * `Some(corpus)` — `[tenant_corpus]` section present + parsed.
//! * `None` — no `forge.toml` OR no `[tenant_corpus]` section OR
//!   malformed TOML.
//!
//! Phases that consume corpora MUST be tolerant of `None` — the
//! substrate baseline is the floor; tenants extend only.
//!
//! ## Layering semantics
//!
//! See the doctrine doc for full details. Summary:
//!
//! | Field                        | Layering   |
//! |------------------------------|------------|
//! | `extra_jargon`               | additive   |
//! | `suppress_jargon`            | subtractive|
//! | `extra_scaffold_defaults`    | additive   |
//! | `extra_vague_link_phrases`   | additive   |
//! | `extra_body_leak_markers`    | additive   |
//! | `density_override`           | per-pattern replace |
//! | `reference_site`             | additive   |
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"` (inherited).
//! * No unwrap/expect in non-test code.
//! * `#[non_exhaustive]` on the top-level struct so adding
//!   fields in a future minor isn't a breaking change.
//! * `load()` is fail-tolerant — IO errors, parse errors, and
//!   structural mismatches all return `None`.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// One scaffold-default override: `(field, value)` pair to flag
/// as a placeholder when found unchanged in cms/*.json.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScaffoldDefaultEntry {
    /// CMS JSON field name (e.g. `"title"`, `"brand"`).
    pub field: String,
    /// Literal value to match (case-sensitive).
    pub value: String,
}

/// One density-tier override for a page-pattern.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DensityOverrideEntry {
    /// Glob pattern over the cms/ relative path
    /// (e.g. `"cms/blog/*.json"`).
    pub pattern: String,
    /// Override tier. One of `"sparse"`, `"comfortable"`,
    /// `"dense"`, `"extreme"`. Parsing into the typed enum is
    /// the consumer's responsibility; this struct just carries
    /// the string verbatim.
    pub tier: String,
}

/// One reference-site entry for the pixel-reproduction rotation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceSiteEntry {
    /// Live URL of the reference site.
    pub url: String,
    /// Declared density tier (`"sparse"` / `"comfortable"` /
    /// `"dense"` / `"extreme"`).
    pub tier: String,
    /// Optional human-readable note about why the site is in
    /// the corpus.
    #[serde(default)]
    pub note: Option<String>,
}

/// Parsed `[tenant_corpus]` section of `forge.toml`.
///
/// All fields default to empty. Every field uses serde-default
/// so partial corpora (e.g. only `extra_jargon` set, nothing
/// else) deserialize cleanly.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
#[non_exhaustive]
pub struct TenantCorpus {
    /// Additional jargon phrases the operator wants flagged.
    /// Layered onto `forge_phases::aesthetic_distinctiveness::JARGON_PHRASES`.
    pub extra_jargon: Vec<String>,
    /// Baseline jargon phrases to remove for this tenant. Each
    /// entry must match a baseline phrase exactly; non-matches
    /// emit a warn-finding (typo guard).
    pub suppress_jargon: Vec<String>,
    /// Additional scaffold-default `(field, value)` pairs.
    /// Layered onto `forge_phases::placeholder_value_audit::SCAFFOLD_DEFAULTS`.
    pub extra_scaffold_defaults: Vec<ScaffoldDefaultEntry>,
    /// Additional vague link-text phrases. Layered onto the
    /// crawler-detectors `link_text_distinguishable::VAGUE_PHRASES`.
    pub extra_vague_link_phrases: Vec<String>,
    /// Additional body-leak markers the hunted_tier phase
    /// scans for. Layered onto `BODY_LEAK_MARKERS`.
    pub extra_body_leak_markers: Vec<String>,
    /// Per-page-pattern density-tier overrides. Replaces (not
    /// adds to) the substrate's heuristic classification when
    /// the pattern matches a cms/*.json path.
    pub density_override: Vec<DensityOverrideEntry>,
    /// Additional reference sites for the pixel-reproduction
    /// rotation.
    pub reference_site: Vec<ReferenceSiteEntry>,
}

/// Wrapper used to deserialize the top-level `forge.toml` so
/// we only have to read the `[tenant_corpus]` table, not the
/// other sections.
#[derive(Debug, Default, Deserialize)]
struct ForgeTomlEnvelope {
    #[serde(default)]
    tenant_corpus: Option<TenantCorpus>,
}

impl TenantCorpus {
    /// Load the `[tenant_corpus]` section from `<root>/forge.toml`.
    ///
    /// Fail-tolerant per the doctrine: ANY error path (no file,
    /// non-UTF8, malformed TOML, missing section, schema mismatch)
    /// returns `None` rather than propagating. Phases that consume
    /// corpora MUST treat `None` as "no tenant extensions" and
    /// fall through to the substrate baseline.
    #[must_use]
    pub fn load(root: &Path) -> Option<Self> {
        let path = root.join("forge.toml");
        let body = std::fs::read_to_string(&path).ok()?;
        // Deserialize the whole document just enough to pull out
        // the [tenant_corpus] section, then drop everything else.
        let envelope: ForgeTomlEnvelope = toml::from_str(&body).ok()?;
        envelope.tenant_corpus
    }

    /// `true` iff this corpus carries no entries at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.extra_jargon.is_empty()
            && self.suppress_jargon.is_empty()
            && self.extra_scaffold_defaults.is_empty()
            && self.extra_vague_link_phrases.is_empty()
            && self.extra_body_leak_markers.is_empty()
            && self.density_override.is_empty()
            && self.reference_site.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let p = env::temp_dir().join(format!(
            "forge-tenant-corpus-{}-{}",
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
        assert!(TenantCorpus::load(&dir).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_returns_none_when_no_tenant_corpus_section() {
        let dir = temp_dir("no-section");
        write_forge_toml(&dir, "[other]\nfoo = 1\n");
        assert!(TenantCorpus::load(&dir).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_returns_some_empty_when_section_present_but_empty() {
        // `[tenant_corpus]` heading with no fields → all-default struct.
        let dir = temp_dir("empty-section");
        write_forge_toml(&dir, "[tenant_corpus]\n");
        let corpus = TenantCorpus::load(&dir).expect("section present");
        assert!(corpus.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_extra_jargon_round_trip() {
        let dir = temp_dir("extra-jargon");
        write_forge_toml(
            &dir,
            r#"
[tenant_corpus]
extra_jargon = ["synergy with our framework", "the Acme advantage"]
"#,
        );
        let corpus = TenantCorpus::load(&dir).expect("section present");
        assert_eq!(corpus.extra_jargon.len(), 2);
        assert_eq!(corpus.extra_jargon[0], "synergy with our framework");
        assert_eq!(corpus.extra_jargon[1], "the Acme advantage");
        assert!(corpus.suppress_jargon.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_suppress_jargon_round_trip() {
        let dir = temp_dir("suppress");
        write_forge_toml(
            &dir,
            r#"
[tenant_corpus]
suppress_jargon = ["transform your"]
"#,
        );
        let corpus = TenantCorpus::load(&dir).expect("section present");
        assert_eq!(corpus.suppress_jargon, vec!["transform your".to_owned()]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_scaffold_defaults_round_trip() {
        let dir = temp_dir("scaffold");
        write_forge_toml(
            &dir,
            r#"
[tenant_corpus]
extra_scaffold_defaults = [
  { field = "title", value = "Acme Internal — Untitled Project" },
  { field = "brand", value = "Acme — Replace Me" },
]
"#,
        );
        let corpus = TenantCorpus::load(&dir).expect("section present");
        assert_eq!(corpus.extra_scaffold_defaults.len(), 2);
        assert_eq!(corpus.extra_scaffold_defaults[0].field, "title");
        assert_eq!(
            corpus.extra_scaffold_defaults[0].value,
            "Acme Internal — Untitled Project"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_density_override_round_trip() {
        let dir = temp_dir("density");
        write_forge_toml(
            &dir,
            r#"
[tenant_corpus]
[[tenant_corpus.density_override]]
pattern = "cms/blog/*.json"
tier = "dense"

[[tenant_corpus.density_override]]
pattern = "cms/index.json"
tier = "sparse"
"#,
        );
        let corpus = TenantCorpus::load(&dir).expect("section present");
        assert_eq!(corpus.density_override.len(), 2);
        assert_eq!(corpus.density_override[0].pattern, "cms/blog/*.json");
        assert_eq!(corpus.density_override[0].tier, "dense");
        assert_eq!(corpus.density_override[1].pattern, "cms/index.json");
        assert_eq!(corpus.density_override[1].tier, "sparse");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_reference_site_round_trip() {
        let dir = temp_dir("ref-site");
        write_forge_toml(
            &dir,
            r#"
[tenant_corpus]
[[tenant_corpus.reference_site]]
url = "https://competitor.example"
tier = "comfortable"
note = "Competitor we want to match on type-density signals"

[[tenant_corpus.reference_site]]
url = "https://acme-design.example/landing-2025"
tier = "sparse"
"#,
        );
        let corpus = TenantCorpus::load(&dir).expect("section present");
        assert_eq!(corpus.reference_site.len(), 2);
        assert_eq!(corpus.reference_site[0].url, "https://competitor.example");
        assert_eq!(corpus.reference_site[0].tier, "comfortable");
        assert!(corpus.reference_site[0].note.is_some());
        assert!(corpus.reference_site[1].note.is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_returns_none_on_malformed_toml() {
        let dir = temp_dir("malformed");
        write_forge_toml(&dir, "[tenant_corpus\nthis is not valid toml = ");
        assert!(TenantCorpus::load(&dir).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_returns_none_on_unknown_subfield() {
        // ScaffoldDefaultEntry uses deny_unknown_fields. An entry
        // with an extra field fails to deserialize; the whole
        // load() returns None per the fail-tolerant contract.
        let dir = temp_dir("unknown-field");
        write_forge_toml(
            &dir,
            r#"
[tenant_corpus]
extra_scaffold_defaults = [
  { field = "title", value = "X", surprise = "boom" },
]
"#,
        );
        // Deny-unknown-fields makes this fail-to-parse; load() is
        // tolerant + returns None.
        assert!(TenantCorpus::load(&dir).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn is_empty_distinguishes_empty_from_populated() {
        let mut c = TenantCorpus::default();
        assert!(c.is_empty());
        c.extra_jargon.push("synergy".into());
        assert!(!c.is_empty());
    }

    #[test]
    fn full_corpus_round_trip_all_fields() {
        let dir = temp_dir("full");
        write_forge_toml(
            &dir,
            r#"
[tenant_corpus]
extra_jargon = ["a", "b"]
suppress_jargon = ["transform your"]
extra_vague_link_phrases = ["go here", "this link"]
extra_body_leak_markers = ["indexedDB.open"]

[[tenant_corpus.extra_scaffold_defaults]]
field = "title"
value = "Internal Placeholder"

[[tenant_corpus.density_override]]
pattern = "cms/*.json"
tier = "comfortable"

[[tenant_corpus.reference_site]]
url = "https://ref.example"
tier = "dense"
"#,
        );
        let corpus = TenantCorpus::load(&dir).expect("loads");
        assert_eq!(corpus.extra_jargon.len(), 2);
        assert_eq!(corpus.suppress_jargon.len(), 1);
        assert_eq!(corpus.extra_vague_link_phrases.len(), 2);
        assert_eq!(corpus.extra_body_leak_markers.len(), 1);
        assert_eq!(corpus.extra_scaffold_defaults.len(), 1);
        assert_eq!(corpus.density_override.len(), 1);
        assert_eq!(corpus.reference_site.len(), 1);
        assert!(!corpus.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
