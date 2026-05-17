//! Coverage policy + gap detection.
//!
//! A capability is "covered" if every requirement in the active
//! [`CoveragePolicy`] is satisfied. The CI gate (task #33) runs
//! [`PlatformManifest::coverage_report`](crate::PlatformManifest)
//! and refuses to merge if any gap is found.
//!
//! The defaults are intentionally strict — a declared capability
//! with zero handlers + zero UI + zero tests + zero docs is a
//! drift-prone capability and the gate exists to prevent that.
//! Sites that need a softer policy can override the manifest's
//! `coverage` block.

use serde::{Deserialize, Serialize};

use crate::{Capability, CapabilityId, PlatformManifest};

/// What "covered" means for the gate.
///
/// Defaults: every capability must have ≥1 handler, ≥1 UI ref,
/// ≥1 test ref, ≥1 doc ref.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CoveragePolicy {
    /// Minimum handler refs per capability.
    #[serde(default = "default_min_one")]
    pub min_handlers: u32,
    /// Minimum UI refs per capability.
    #[serde(default = "default_min_one")]
    pub min_ui: u32,
    /// Minimum test refs per capability.
    #[serde(default = "default_min_one")]
    pub min_tests: u32,
    /// Minimum doc refs per capability.
    #[serde(default = "default_min_one")]
    pub min_docs: u32,
    /// Capabilities exempt from the gate (kebab-case IDs).
    ///
    /// Use sparingly — every exemption is technical debt the next
    /// audit pass has to justify.
    #[serde(default)]
    pub exempt: Vec<CapabilityId>,
}

fn default_min_one() -> u32 {
    1
}

impl Default for CoveragePolicy {
    fn default() -> Self {
        Self {
            min_handlers: 1,
            min_ui: 1,
            min_tests: 1,
            min_docs: 1,
            exempt: Vec::new(),
        }
    }
}

/// A single coverage gap. The gate refuses if any gaps exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CoverageGap {
    /// Capability that failed the policy.
    pub capability: CapabilityId,
    /// Kind of requirement unmet (`handlers` / `ui` / `tests` / `docs`).
    pub kind: GapKind,
    /// How many references were declared (vs the policy minimum).
    pub declared: u32,
    /// The policy minimum that wasn't met.
    pub required: u32,
}

/// Which dimension of coverage failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GapKind {
    /// Missing handler module references.
    Handlers,
    /// Missing UI module references.
    Ui,
    /// Missing test references.
    Tests,
    /// Missing doc references.
    Docs,
}

/// Output of [`PlatformManifest::coverage_report`]. Empty `gaps`
/// means the gate passes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct CoverageReport {
    /// One entry per (capability, missing-dimension) pair.
    pub gaps: Vec<CoverageGap>,
    /// Total capabilities counted (including exempt ones).
    pub total_capabilities: u32,
    /// Capabilities exempt under the active policy.
    pub exempt_capabilities: u32,
}

impl PlatformManifest {
    /// Compute a coverage report under the manifest's
    /// [`CoveragePolicy`]. The CI gate (task #33) uses this.
    pub fn coverage_report(&self) -> CoverageReport {
        let policy = &self.coverage;
        let exempt: std::collections::HashSet<&CapabilityId> = policy.exempt.iter().collect();
        let mut gaps = Vec::new();
        for cap in &self.capabilities {
            if exempt.contains(&cap.id) {
                continue;
            }
            check_dim(
                cap,
                cap.handlers.len() as u32,
                policy.min_handlers,
                GapKind::Handlers,
                &mut gaps,
            );
            check_dim(
                cap,
                cap.ui.len() as u32,
                policy.min_ui,
                GapKind::Ui,
                &mut gaps,
            );
            check_dim(
                cap,
                cap.tests.len() as u32,
                policy.min_tests,
                GapKind::Tests,
                &mut gaps,
            );
            check_dim(
                cap,
                cap.docs.len() as u32,
                policy.min_docs,
                GapKind::Docs,
                &mut gaps,
            );
        }
        CoverageReport {
            gaps,
            total_capabilities: self.capabilities.len() as u32,
            exempt_capabilities: exempt.len() as u32,
        }
    }
}

fn check_dim(
    cap: &Capability,
    declared: u32,
    required: u32,
    kind: GapKind,
    out: &mut Vec<CoverageGap>,
) {
    if declared < required {
        out.push(CoverageGap {
            capability: cap.id.clone(),
            kind,
            declared,
            required,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{
        Capability, CapabilityId, DocRef, HandlerRef, Ownership, PlatformManifest, TestRef, UiRef,
    };

    fn fully_covered_cap(id: &str) -> Capability {
        Capability {
            id: CapabilityId::parse(id).unwrap(),
            summary: "x".into(),
            ownership: Ownership::Forge,
            handlers: vec![HandlerRef("h".into())],
            ui: vec![UiRef("u".into())],
            tests: vec![TestRef("t".into())],
            docs: vec![DocRef("d".into())],
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn fully_covered_capability_has_no_gaps() {
        let m = PlatformManifest {
            platform: "p".into(),
            capabilities: vec![fully_covered_cap("auth")],
            ..Default::default()
        };
        assert!(m.coverage_report().gaps.is_empty());
    }

    #[test]
    fn empty_capability_reports_four_gaps() {
        let mut cap = fully_covered_cap("auth");
        cap.handlers.clear();
        cap.ui.clear();
        cap.tests.clear();
        cap.docs.clear();
        let m = PlatformManifest {
            platform: "p".into(),
            capabilities: vec![cap],
            ..Default::default()
        };
        let r = m.coverage_report();
        assert_eq!(r.gaps.len(), 4);
        assert_eq!(r.total_capabilities, 1);
        assert_eq!(r.exempt_capabilities, 0);
    }

    #[test]
    fn exempt_capability_skipped() {
        let mut cap = fully_covered_cap("auth");
        cap.handlers.clear();
        let m = PlatformManifest {
            platform: "p".into(),
            capabilities: vec![cap],
            coverage: CoveragePolicy {
                exempt: vec![CapabilityId::parse("auth").unwrap()],
                ..Default::default()
            },
            ..Default::default()
        };
        let r = m.coverage_report();
        assert!(r.gaps.is_empty());
        assert_eq!(r.exempt_capabilities, 1);
    }

    #[test]
    fn policy_default_requires_one_per_dim() {
        let p = CoveragePolicy::default();
        assert_eq!(p.min_handlers, 1);
        assert_eq!(p.min_ui, 1);
        assert_eq!(p.min_tests, 1);
        assert_eq!(p.min_docs, 1);
    }
}
