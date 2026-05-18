//! Bridge from the workspace-level `phases.toml` schema into the
//! manifest-core [`PhaseDescriptor`] surface.
//!
//! Parallel to [`super::backends_toml`] but for Forge build phases.
//! The phases.toml file declares which phases ship in the Forge
//! pipeline and what their default severity + dependencies look
//! like; the manifest projection feeds that into the rest of the
//! keystone (codegen + coverage gate + admin UI).
//!
//! ### Schema
//!
//! ```toml
//! [meta]
//! schema_version = 1
//!
//! [phases.tokens]
//! summary          = "Loom token surface check"
//! default_severity = "warn"
//! depends_on       = []
//! implements       = "loom-token-coverage" # optional capability ref
//! ```

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{CapabilityId, DefaultSeverity, ManifestError, PhaseDescriptor};

/// Top-level shape of `phases.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct PhasesToml {
    /// Free-form metadata table — uninterpreted.
    #[serde(default)]
    pub meta: BTreeMap<String, toml::Value>,
    /// Map of phase ID → declaration.
    #[serde(default)]
    pub phases: BTreeMap<String, PhaseEntry>,
}

/// One `[phases.NAME]` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct PhaseEntry {
    /// One-line human summary.
    pub summary: String,
    /// Default severity when this phase emits a finding.
    #[serde(default)]
    pub default_severity: DefaultSeverity,
    /// Phase IDs this one depends on (must precede it in the
    /// pipeline). Empty == independent.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Optional capability ID this phase implements.
    #[serde(default)]
    pub implements: Option<String>,
}

impl PhasesToml {
    /// Parse from a raw TOML string.
    pub fn from_toml(s: &str) -> Result<Self, ManifestError> {
        let parsed: Self = toml::from_str(s)?;
        Ok(parsed)
    }

    /// Project every `[phases.NAME]` entry into a
    /// [`PhaseDescriptor`]. Stable iteration order (BTreeMap).
    pub fn to_descriptors(&self) -> Result<Vec<PhaseDescriptor>, ManifestError> {
        let mut out = Vec::with_capacity(self.phases.len());
        for (id, entry) in &self.phases {
            let cap_id = CapabilityId::parse(id)?;
            let implements = match &entry.implements {
                Some(s) => Some(CapabilityId::parse(s)?),
                None => None,
            };
            let mut depends_on = Vec::with_capacity(entry.depends_on.len());
            for dep in &entry.depends_on {
                depends_on.push(CapabilityId::parse(dep)?);
            }
            out.push(PhaseDescriptor {
                id: cap_id,
                implements,
                summary: entry.summary.clone(),
                default_severity: entry.default_severity,
                depends_on,
            });
        }
        Ok(out)
    }

    /// Topologically sort phases by their declared dependencies.
    ///
    /// Returns an error if a cycle is present. Phases with no
    /// dependencies appear in BTreeMap order (alphabetical by ID),
    /// which gives Forge a deterministic baseline pipeline order
    /// when nothing constrains it.
    pub fn topo_sort(&self) -> Result<Vec<PhaseDescriptor>, ManifestError> {
        let phases = self.to_descriptors()?;
        let by_id: BTreeMap<&CapabilityId, &PhaseDescriptor> =
            phases.iter().map(|p| (&p.id, p)).collect();
        // Verify every depends_on resolves.
        for p in &phases {
            for dep in &p.depends_on {
                if !by_id.contains_key(&dep) {
                    return Err(ManifestError::PhaseImplementsUnknown {
                        phase: p.id.clone(),
                        capability: dep.clone(),
                    });
                }
            }
        }
        // Kahn's algorithm.
        let mut in_deg: BTreeMap<&CapabilityId, usize> =
            phases.iter().map(|p| (&p.id, p.depends_on.len())).collect();
        let mut ready: std::collections::VecDeque<&CapabilityId> = in_deg
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut out_ids = Vec::with_capacity(phases.len());
        while let Some(id) = ready.pop_front() {
            out_ids.push(id.clone());
            for p in &phases {
                if p.depends_on.iter().any(|d| d == id) {
                    if let Some(d) = in_deg.get_mut(&p.id) {
                        *d = d.saturating_sub(1);
                        if *d == 0 {
                            ready.push_back(&p.id);
                        }
                    }
                }
            }
        }
        if out_ids.len() != phases.len() {
            // Cycle — name a remaining node so the operator can find it.
            let cyclic = phases
                .iter()
                .find(|p| !out_ids.iter().any(|id| id == &p.id))
                .map(|p| p.id.clone())
                .unwrap_or_else(|| CapabilityId::parse("unknown").unwrap());
            return Err(ManifestError::InvalidCapabilityId(format!(
                "cycle detected in phases.toml depends_on involving {cyclic}"
            )));
        }
        // Re-emit in order.
        let mut sorted = Vec::with_capacity(phases.len());
        for id in out_ids {
            if let Some(p) = phases.iter().find(|p| p.id == id) {
                sorted.push(p.clone());
            }
        }
        Ok(sorted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[meta]
schema_version = 1

[phases.tokens]
summary          = "Loom token surface check"
default_severity = "warn"
depends_on       = []

[phases.theme-contrast]
summary          = "WCAG AA contrast across themes"
default_severity = "strict"
depends_on       = ["tokens"]
implements       = "wcag-aa-contrast"

[phases.seo]
summary          = "SEO meta / open graph audit"
default_severity = "warn"
depends_on       = []
"#;

    #[test]
    fn parses_phases_and_meta() {
        let p = PhasesToml::from_toml(SAMPLE).unwrap();
        assert_eq!(p.phases.len(), 3);
        assert_eq!(p.phases["tokens"].summary, "Loom token surface check");
    }

    #[test]
    fn projects_to_phase_descriptors() {
        let p = PhasesToml::from_toml(SAMPLE).unwrap();
        let descs = p.to_descriptors().unwrap();
        let theme = descs
            .iter()
            .find(|d| d.id.as_str() == "theme-contrast")
            .unwrap();
        assert_eq!(theme.default_severity, DefaultSeverity::Strict);
        assert_eq!(theme.depends_on.len(), 1);
        assert_eq!(theme.depends_on[0].as_str(), "tokens");
        assert_eq!(
            theme.implements.as_ref().unwrap().as_str(),
            "wcag-aa-contrast"
        );
    }

    #[test]
    fn topo_sort_respects_dependencies() {
        let p = PhasesToml::from_toml(SAMPLE).unwrap();
        let sorted = p.topo_sort().unwrap();
        let pos = |id: &str| sorted.iter().position(|p| p.id.as_str() == id).unwrap();
        // tokens must come before theme-contrast.
        assert!(pos("tokens") < pos("theme-contrast"));
    }

    #[test]
    fn topo_sort_detects_cycle() {
        let cyclic = r#"
[phases.a]
summary    = "a"
depends_on = ["b"]

[phases.b]
summary    = "b"
depends_on = ["a"]
"#;
        let p = PhasesToml::from_toml(cyclic).unwrap();
        let err = p.topo_sort().unwrap_err();
        assert!(format!("{err}").contains("cycle"));
    }

    #[test]
    fn topo_sort_detects_unknown_dependency() {
        let bad = r#"
[phases.a]
summary    = "a"
depends_on = ["does-not-exist"]
"#;
        let p = PhasesToml::from_toml(bad).unwrap();
        let err = p.topo_sort().unwrap_err();
        assert!(matches!(err, ManifestError::PhaseImplementsUnknown { .. }));
    }

    #[test]
    fn rejects_unknown_fields_in_entry() {
        let bad = r#"
[phases.a]
summary   = "a"
ahem_typo = 1
"#;
        let err = PhasesToml::from_toml(bad).unwrap_err();
        assert!(matches!(err, ManifestError::Toml(_)));
    }

    #[test]
    fn projects_the_repo_phases_toml() {
        // The actual workspace-root file. Skip cleanly if it isn't
        // present (partial clone).
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../phases.toml");
        let s = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => return,
        };
        let p = PhasesToml::from_toml(&s).expect("repo phases.toml parses");
        let descs = p.to_descriptors().expect("repo phases.toml projects");
        assert!(
            descs.len() >= 30,
            "expected ≥ 30 phases in repo file, got {}",
            descs.len()
        );
        // Every depends_on must be acyclic + resolvable.
        let _sorted = p.topo_sort().expect("repo phases.toml topo-sorts");
        for d in &descs {
            assert!(!d.id.as_str().is_empty());
            assert!(!d.summary.is_empty(), "{} has empty summary", d.id);
        }
    }

    #[test]
    fn default_severity_is_warn() {
        let bare = r#"
[phases.a]
summary = "a"
"#;
        let p = PhasesToml::from_toml(bare).unwrap();
        let descs = p.to_descriptors().unwrap();
        assert_eq!(descs[0].default_severity, DefaultSeverity::Warn);
    }
}
