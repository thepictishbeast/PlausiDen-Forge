//! Tenant-level placeholder substitution tables — sibling JSON
//! files at the tenant root:
//!
//! - `variables.json`  — `{{ VAR }}` placeholders → values
//! - `palette.json`    — `{{ PALETTE.fg }}` placeholders → values
//! - `assets-map.json` — `@asset-slug` references → resolved URL
//!
//! Per paul 2026-05-21 (#322): "you should be using variables and
//! such to replace generic place holders for when you use the
//! content for a specific site... it might pull the variables
//! from a json".
//!
//! Wire format for each file is a flat JSON object:
//!
//! ```json
//! { "BRAND_NAME": "PlausiDen", "YEAR": "2026" }
//! ```
//!
//! At render time, `forge-phases::render` projects this struct
//! through `loom_cms_render::apply_variables` before emitting
//! HTML — the placeholder syntax is the same as the typed
//! `loom_variables::TenantVariables` shape this struct mirrors,
//! so a future merge of the two layers is one rename away.
//!
//! Fail-tolerant load: ANY error path (no file, non-UTF8,
//! malformed JSON, type mismatch) returns `None` for that
//! sub-table; if ALL three sub-tables are missing, [`load`]
//! returns `None` overall so the render phase can skip the
//! substitution pass entirely.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Three sibling substitution tables. Any may be empty.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TenantVariables {
    /// `{{ KEY }}` → value. Loaded from `variables.json`.
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    /// `{{ PALETTE.foo }}` → value. Loaded from `palette.json`.
    #[serde(default)]
    pub palette: BTreeMap<String, String>,
    /// `@asset-slug` → resolved URL. Loaded from `assets-map.json`.
    #[serde(default)]
    pub assets: BTreeMap<String, String>,
}

impl TenantVariables {
    /// Load the three sibling JSON files at the tenant root,
    /// merging the present ones into a single [`TenantVariables`].
    ///
    /// Returns `None` when all three files are absent or
    /// unreadable. A missing or malformed individual file is
    /// silently dropped — the substitute pass downstream
    /// preserves unresolved placeholders verbatim, and the
    /// authoring audit catches them.
    #[must_use]
    pub fn load(root: &Path) -> Option<Self> {
        let variables = load_json_map(&root.join("variables.json"));
        let palette = load_json_map(&root.join("palette.json"));
        let assets = load_json_map(&root.join("assets-map.json"));
        if variables.is_none() && palette.is_none() && assets.is_none() {
            return None;
        }
        Some(Self {
            variables: variables.unwrap_or_default(),
            palette: palette.unwrap_or_default(),
            assets: assets.unwrap_or_default(),
        })
    }

    /// True when every table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.variables.is_empty() && self.palette.is_empty() && self.assets.is_empty()
    }

    /// Number of placeholder + asset entries across all three
    /// tables. Used by the orient surface to report tenant
    /// variable-coverage.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.variables.len() + self.palette.len() + self.assets.len()
    }
}

/// Read a JSON file as a flat string-to-string map. Returns
/// `None` on any error (missing, malformed, wrong shape).
fn load_json_map(path: &Path) -> Option<BTreeMap<String, String>> {
    let body = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<BTreeMap<String, String>>(&body).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tempdir(test_name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("forge-tv-{test_name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).expect("mkdir");
        d
    }

    #[test]
    fn load_returns_none_when_no_files_present() {
        let root = tempdir("no_files");
        assert!(TenantVariables::load(&root).is_none());
    }

    #[test]
    fn load_reads_variables_json_when_present() {
        let root = tempdir("vars_only");
        fs::write(
            root.join("variables.json"),
            r#"{"BRAND_NAME":"PlausiDen","YEAR":"2026"}"#,
        )
        .unwrap();
        let tv = TenantVariables::load(&root).expect("loads");
        assert_eq!(tv.variables.get("BRAND_NAME").map(String::as_str), Some("PlausiDen"));
        assert_eq!(tv.variables.get("YEAR").map(String::as_str), Some("2026"));
        assert!(tv.palette.is_empty());
        assert!(tv.assets.is_empty());
    }

    #[test]
    fn load_merges_three_sibling_files() {
        let root = tempdir("three_files");
        fs::write(root.join("variables.json"), r#"{"FOO":"bar"}"#).unwrap();
        fs::write(root.join("palette.json"), r##"{"accent":"#3a7afe"}"##).unwrap();
        fs::write(
            root.join("assets-map.json"),
            r#"{"logo":"/static/logo.svg"}"#,
        )
        .unwrap();
        let tv = TenantVariables::load(&root).expect("loads");
        assert_eq!(tv.variables.get("FOO").map(String::as_str), Some("bar"));
        assert_eq!(tv.palette.get("accent").map(String::as_str), Some("#3a7afe"));
        assert_eq!(
            tv.assets.get("logo").map(String::as_str),
            Some("/static/logo.svg")
        );
        assert_eq!(tv.entry_count(), 3);
    }

    #[test]
    fn load_drops_malformed_file_keeps_others() {
        let root = tempdir("malformed");
        fs::write(root.join("variables.json"), r#"{"OK":"yes"}"#).unwrap();
        fs::write(root.join("palette.json"), r#"this is not json"#).unwrap();
        let tv = TenantVariables::load(&root).expect("loads");
        assert!(!tv.variables.is_empty());
        assert!(tv.palette.is_empty()); // dropped on parse error
    }

    #[test]
    fn is_empty_matches_no_entries() {
        let tv = TenantVariables::default();
        assert!(tv.is_empty());
        assert_eq!(tv.entry_count(), 0);
    }

    #[test]
    fn rejects_non_string_values_via_silent_drop() {
        // The map shape is BTreeMap<String, String>; a JSON value
        // of int / null / array fails to deserialize so the file
        // is treated as malformed → empty map.
        let root = tempdir("non_string");
        fs::write(root.join("variables.json"), r#"{"K":42}"#).unwrap();
        let tv = TenantVariables::load(&root);
        assert!(tv.is_none() || tv.unwrap().variables.is_empty());
    }
}
