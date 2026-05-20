//! `substrate_state` — substrate-wide state machine snapshot.
//!
//! Task #286 per the MCP cluster (#284-#288). Aggregates the
//! substrate's declarative state — which gates are active for
//! the current site, which content types are declared, which
//! deprecations are pending — into a single payload Claude
//! queries to orient itself for any action.
//!
//! Where session_context (#285) is *operator-state*-focused
//! (identity, provenance history, recent skill calls), this
//! module is *substrate-state*-focused (gate activation,
//! declared taxonomy, doctrine version).
//!
//! ## What it captures
//!
//! * `active_gates` — every variation-arc phase whose
//!   `[<phase>] enforce = true` flag is set in forge.toml.
//! * `declared_content_types` — slugs from
//!   `[[site_identity.content_type]]`.
//! * `declared_theme_variants` — names from
//!   `[[site_identity.theme_variant]]`.
//! * `enabled_corpora` — which on-disk corpora (page_types,
//!   mcp_tools, skills, reference_baseline) are present.
//! * `doctrine_version` — placeholder for AVP-Doctrine version
//!   tracking (populated when the doctrine ships its own
//!   version stamp).
//! * `pending_deprecations` — corpus deprecation log
//!   (placeholder; future skill emits to it).
//!
//! ## API
//!
//! * [`SubstrateState::snapshot`] — pure compute against root.
//! * [`SubstrateState::is_gate_active`] — lookup helper.
//! * [`SubstrateState::serialize_pretty`] — pretty-JSON for
//!   the CLI / MCP layer.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `#[non_exhaustive]` on every public type.
//! * No unwrap/expect in non-test code.
//! * Pure filesystem reads — no spawn, no network.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// All known variation-arc gates whose `[<name>] enforce = true`
/// flag toggles activation. Mirror of the lib.rs phase list,
/// kept here so this module doesn't take a forge-phases
/// dependency.
const KNOWN_GATES: &[&str] = &[
    "editorial_purity",
    "site_identity_conformance",
    "identity_coherence",
    "voice_profile_audit",
    "mood_lock",
    "composition_lineage",
    "forbidden_patterns",
    "pattern_entropy",
    "primitive_exhaustion",
    "differentiation_budget",
    "reseeding_cadence",
    "pattern_emergence",
    "uniqueness_gate",
    "theme_variation_required",
    "zone_constraints",
    "substrate_self_audit",
];

/// Corpora files the substrate ships. Snapshot reports which
/// are present in the project root's `corpora/` directory.
const KNOWN_CORPORA: &[&str] = &[
    "page_types.json",
    "mcp_tools.json",
    "skills.json",
    "reference_baseline.json",
];

/// Substrate state snapshot.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SubstrateState {
    /// ISO-8601 UTC timestamp.
    pub captured_at: String,
    /// Gates whose `enforce = true` flag is set in forge.toml.
    pub active_gates: Vec<String>,
    /// Gates that are SHIPPED in the substrate but NOT enabled
    /// for this site. Useful for telling the operator "you
    /// could opt into these."
    pub available_but_inactive_gates: Vec<String>,
    /// Content-type slugs declared in
    /// `[[site_identity.content_type]]`.
    pub declared_content_types: Vec<String>,
    /// Theme variant names declared in
    /// `[[site_identity.theme_variant]]`.
    pub declared_theme_variants: Vec<String>,
    /// Corpora files present in `<root>/corpora/`.
    pub enabled_corpora: Vec<String>,
    /// Corpora files the substrate knows about but that AREN'T
    /// present in `<root>/corpora/`.
    pub missing_corpora: Vec<String>,
    /// AVP-Doctrine version stamp. Empty when the doctrine
    /// doesn't ship a version file yet.
    pub doctrine_version: String,
    /// Pending deprecation IDs (e.g. primitive kinds slated for
    /// removal). Empty when none queued.
    pub pending_deprecations: Vec<String>,
    /// Site root the snapshot was taken against.
    pub root: String,
}

impl SubstrateState {
    /// Compute a fresh snapshot. Pure filesystem reads.
    #[must_use]
    pub fn snapshot(root: &Path) -> Self {
        let captured_at = current_iso_utc();
        let (active_gates, available_but_inactive_gates) = scan_gates(root);
        let (declared_content_types, declared_theme_variants) = scan_identity(root);
        let (enabled_corpora, missing_corpora) = scan_corpora(root);
        let doctrine_version = scan_doctrine_version(root);
        let pending_deprecations = scan_deprecations(root);

        Self {
            captured_at,
            active_gates,
            available_but_inactive_gates,
            declared_content_types,
            declared_theme_variants,
            enabled_corpora,
            missing_corpora,
            doctrine_version,
            pending_deprecations,
            root: root.display().to_string(),
        }
    }

    /// Returns true if the named gate is in `active_gates`.
    #[must_use]
    pub fn is_gate_active(&self, name: &str) -> bool {
        self.active_gates.iter().any(|g| g == name)
    }

    /// Pretty-printed JSON for the CLI / MCP layer.
    pub fn serialize_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

fn scan_gates(root: &Path) -> (Vec<String>, Vec<String>) {
    let mut active = Vec::new();
    let mut inactive = Vec::new();
    let body = match fs::read_to_string(root.join("forge.toml")) {
        Ok(b) => b,
        Err(_) => {
            // No forge.toml — all known gates are inactive.
            inactive.extend(KNOWN_GATES.iter().map(|s| (*s).to_owned()));
            return (active, inactive);
        }
    };
    let value: toml::Value = match toml::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            inactive.extend(KNOWN_GATES.iter().map(|s| (*s).to_owned()));
            return (active, inactive);
        }
    };
    for gate in KNOWN_GATES {
        let enforce = value
            .get(*gate)
            .and_then(|v| v.get("enforce"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if enforce {
            active.push((*gate).to_owned());
        } else {
            inactive.push((*gate).to_owned());
        }
    }
    (active, inactive)
}

fn scan_identity(root: &Path) -> (Vec<String>, Vec<String>) {
    let mut content_types = Vec::new();
    let mut theme_variants = Vec::new();
    let body = match fs::read_to_string(root.join("forge.toml")) {
        Ok(b) => b,
        Err(_) => return (content_types, theme_variants),
    };
    let Ok(value) = toml::from_str::<toml::Value>(&body) else {
        return (content_types, theme_variants);
    };
    let Some(identity) = value.get("site_identity") else {
        return (content_types, theme_variants);
    };
    if let Some(arr) = identity.get("content_type").and_then(|v| v.as_array()) {
        for entry in arr {
            if let Some(slug) = entry.get("slug").and_then(|v| v.as_str()) {
                content_types.push(slug.to_owned());
            }
        }
    }
    if let Some(arr) = identity.get("theme_variant").and_then(|v| v.as_array()) {
        for entry in arr {
            if let Some(name) = entry.get("name").and_then(|v| v.as_str()) {
                theme_variants.push(name.to_owned());
            }
        }
    }
    (content_types, theme_variants)
}

fn scan_corpora(root: &Path) -> (Vec<String>, Vec<String>) {
    let dir = root.join("corpora");
    let mut enabled = Vec::new();
    let mut missing = Vec::new();
    for name in KNOWN_CORPORA {
        if dir.join(name).is_file() {
            enabled.push((*name).to_owned());
        } else {
            missing.push((*name).to_owned());
        }
    }
    (enabled, missing)
}

fn scan_doctrine_version(root: &Path) -> String {
    // Look for a sibling PlausiDen-AVP-Doctrine repo with a
    // VERSION file. Best-effort; empty when unavailable.
    let candidates = [
        root.parent().map(|p| p.join("PlausiDen-AVP-Doctrine/VERSION")),
        Some(root.join("../PlausiDen-AVP-Doctrine/VERSION")),
    ];
    for candidate in candidates.iter().flatten() {
        if let Ok(s) = fs::read_to_string(candidate) {
            return s.trim().to_owned();
        }
    }
    String::new()
}

fn scan_deprecations(root: &Path) -> Vec<String> {
    // Placeholder: read a JSONL deprecation log when it ships.
    let path = root.join("reports/deprecations.jsonl");
    if !path.is_file() {
        return Vec::new();
    }
    let body = match fs::read_to_string(&path) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for line in body.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(id) = value.get("id").and_then(|v| v.as_str()) {
                out.push(id.to_owned());
            }
        }
    }
    out
}

fn current_iso_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Re-use session_context's formatter via a one-line indirection.
    crate::session_context_fmt_indirect(secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "forge-substrate-state-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn snapshot_with_empty_root_lists_all_gates_inactive() {
        let root = temp_root("empty");
        let state = SubstrateState::snapshot(&root);
        assert!(state.active_gates.is_empty());
        assert_eq!(
            state.available_but_inactive_gates.len(),
            KNOWN_GATES.len()
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn snapshot_detects_active_gates_from_forge_toml() {
        let root = temp_root("active-gates");
        fs::write(
            root.join("forge.toml"),
            r#"
[editorial_purity]
enforce = true

[pattern_entropy]
enforce = true

[mood_lock]
enforce = false
"#,
        )
        .unwrap();
        let state = SubstrateState::snapshot(&root);
        assert!(state.is_gate_active("editorial_purity"));
        assert!(state.is_gate_active("pattern_entropy"));
        assert!(!state.is_gate_active("mood_lock"));
        assert!(!state.is_gate_active("zone_constraints"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn snapshot_picks_up_declared_content_types() {
        let root = temp_root("content-types");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]

[[site_identity.content_type]]
slug = "homepage"
pattern = "cms/index.json"

[[site_identity.content_type]]
slug = "blog_post"
pattern = "cms/blog/*.json"
"#,
        )
        .unwrap();
        let state = SubstrateState::snapshot(&root);
        assert_eq!(state.declared_content_types, vec!["homepage", "blog_post"]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn snapshot_picks_up_declared_theme_variants() {
        let root = temp_root("themes");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]

[[site_identity.theme_variant]]
name = "light"

[[site_identity.theme_variant]]
name = "amoled"
"#,
        )
        .unwrap();
        let state = SubstrateState::snapshot(&root);
        assert_eq!(state.declared_theme_variants, vec!["light", "amoled"]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn snapshot_detects_corpora_presence() {
        let root = temp_root("corpora");
        fs::create_dir_all(root.join("corpora")).unwrap();
        fs::write(root.join("corpora/page_types.json"), "{}").unwrap();
        fs::write(root.join("corpora/skills.json"), "{}").unwrap();
        let state = SubstrateState::snapshot(&root);
        assert!(state.enabled_corpora.contains(&"page_types.json".to_owned()));
        assert!(state.enabled_corpora.contains(&"skills.json".to_owned()));
        assert!(state.missing_corpora.contains(&"mcp_tools.json".to_owned()));
        assert!(state.missing_corpora.contains(&"reference_baseline.json".to_owned()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn snapshot_reads_pending_deprecations() {
        let root = temp_root("deprecations");
        fs::create_dir_all(root.join("reports")).unwrap();
        fs::write(
            root.join("reports/deprecations.jsonl"),
            r#"{"id":"primitive.legacy_card"}
{"id":"phase.old_audit"}
"#,
        )
        .unwrap();
        let state = SubstrateState::snapshot(&root);
        assert_eq!(state.pending_deprecations.len(), 2);
        assert!(state.pending_deprecations.contains(&"primitive.legacy_card".to_owned()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn serialize_pretty_emits_valid_json() {
        let root = temp_root("serialize");
        let state = SubstrateState::snapshot(&root);
        let json = state.serialize_pretty().unwrap();
        assert!(json.contains("active_gates"));
        assert!(json.contains("captured_at"));
        // Should round-trip.
        let back: SubstrateState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.root, state.root);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn is_gate_active_is_case_sensitive() {
        let root = temp_root("case");
        fs::write(
            root.join("forge.toml"),
            "[editorial_purity]\nenforce = true\n",
        )
        .unwrap();
        let state = SubstrateState::snapshot(&root);
        assert!(state.is_gate_active("editorial_purity"));
        assert!(!state.is_gate_active("Editorial_Purity"));
        let _ = fs::remove_dir_all(&root);
    }
}
