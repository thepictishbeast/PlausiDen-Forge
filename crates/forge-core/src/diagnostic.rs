//! `diagnostic` — canonical substrate-aware diagnostic shape.
//!
//! Task #295 per the architectural cleanup. Defines the single
//! canonical structure every substrate diagnostic carries,
//! regardless of producer:
//!
//! * `code` — stable kebab-case identifier (e.g. `"var-001"`,
//!   `"ident-002"`, `"io-missing-cms-dir"`).
//! * `message` — human-readable description.
//! * `path` — file or asset the diagnostic is attributed to.
//! * `advocacy` — why + fix + skill + anti-pattern (reuses the
//!   existing [`crate::Advocacy`] shape).
//! * `cited_rules` — AVP-Doctrine rule ids enforced.
//!
//! Producer types (Finding, BuildError, future loom-lint /
//! crawler-detector outputs) all funnel through this struct via
//! the [`SubstrateAwareError`] trait. Consumers (CLI report,
//! JSON serialization, MCP typed-tool output) see one shape.
//!
//! ## Doctrine reference
//!
//! [`docs/SUBSTRATE_ERRORS.md`](../../../docs/SUBSTRATE_ERRORS.md)
//! captures the full doctrine: every diagnostic MUST carry the
//! 5 fields above; phases that emit raw strings without advocacy
//! are non-conformant.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `#[non_exhaustive]` on `Diagnostic` so future fields don't
//!   break consumers.
//! * `unsafe_code = "deny"` (inherited).
//! * No unwrap/expect in non-test code.

use serde::{Deserialize, Serialize};

use crate::{Advocacy, BuildError};

/// The canonical substrate diagnostic. Carries the 5 fields the
/// doctrine requires.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Diagnostic {
    /// Stable kebab-case identifier. Phase implementers MUST
    /// reserve a code namespace (e.g. `"uniqueness."`, `"ident."`,
    /// `"voice."`) and use codes within it. Codes are wire-shape;
    /// renaming one is a breaking change.
    pub code: String,
    /// Human-readable description. Should be precise enough that
    /// a human can fix the underlying issue without rerunning.
    pub message: String,
    /// File or asset path. Empty for project-wide diagnostics.
    pub path: String,
    /// Substrate-fix advocacy (why + fix + skill + anti-pattern).
    #[serde(default, skip_serializing_if = "Advocacy::is_empty")]
    pub advocacy: Advocacy,
    /// AVP-Doctrine rule ids this diagnostic cites.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cited_rules: Vec<String>,
}

impl Diagnostic {
    /// Construct a diagnostic with the minimum required fields.
    /// Use the builder methods to attach advocacy + rules.
    #[must_use]
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            path: path.into(),
            advocacy: Advocacy::default(),
            cited_rules: Vec::new(),
        }
    }

    /// Attach the root-cause line.
    #[must_use]
    pub fn why(mut self, why: impl Into<String>) -> Self {
        self.advocacy.why = why.into();
        self
    }

    /// Attach the substrate-correct fix.
    #[must_use]
    pub fn fix(mut self, fix: impl Into<String>) -> Self {
        self.advocacy.substrate_fix = fix.into();
        self
    }

    /// Attach a skill-playbook pointer.
    #[must_use]
    pub fn skill(mut self, skill: impl Into<String>) -> Self {
        self.advocacy.skill = Some(skill.into());
        self
    }

    /// Attach the anti-pattern (bash/grep alternative to avoid).
    #[must_use]
    pub fn avoid(mut self, anti_pattern: impl Into<String>) -> Self {
        self.advocacy.anti_pattern = Some(anti_pattern.into());
        self
    }

    /// Attach one or more cited AVP-Doctrine rule ids.
    #[must_use]
    pub fn citing<I, S>(mut self, rule_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.cited_rules
            .extend(rule_ids.into_iter().map(Into::into));
        self
    }
}

/// Trait implemented by every substrate error/diagnostic producer.
/// Funnels into the canonical [`Diagnostic`] shape so consumers
/// don't need to special-case producer types.
pub trait SubstrateAwareError {
    /// Convert this producer's payload to a canonical Diagnostic.
    fn to_diagnostic(&self) -> Diagnostic;
}

impl SubstrateAwareError for BuildError {
    /// Map each BuildError variant to a Diagnostic with sensible
    /// default advocacy. Phase implementers SHOULD override this
    /// when their phase has more specific guidance.
    fn to_diagnostic(&self) -> Diagnostic {
        match self {
            BuildError::Io { context, source } => Diagnostic::new(
                "io.filesystem",
                format!("filesystem I/O failed: {context}: {source}"),
                String::new(),
            )
            .why("a phase tried to read or write a filesystem path that wasn't accessible")
            .fix("verify the path exists, the process has read/write permissions, and there is sufficient disk space")
            .citing(["io-001"]),
            BuildError::MissingDependency { what, hint } => Diagnostic::new(
                "deps.missing",
                format!("missing dependency: {what} ({hint})"),
                String::new(),
            )
            .why("a phase needed an external tool or library that wasn't on PATH or installed")
            .fix(format!("install {what} or set the expected environment variable/path; per hint: {hint}"))
            .avoid("don't disable the phase to suppress the error; fix the missing dependency")
            .citing(["deps-001"]),
            BuildError::Config { phase, message } => Diagnostic::new(
                "config.invalid",
                format!("invalid config in phase {phase}: {message}"),
                String::from("forge.toml"),
            )
            .why("the phase received configuration that doesn't match its expected shape")
            .fix("correct the relevant [section] in forge.toml; consult the phase's module docs for the schema")
            .citing(["config-001"]),
            BuildError::Other { phase, message } => Diagnostic::new(
                "phase.other",
                format!("phase {phase} failed: {message}"),
                String::new(),
            )
            .why("an unclassified failure occurred in this phase")
            .fix("inspect the phase logs and report this with reproducer; phase implementer should split out a typed variant"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_builder_chains_fields() {
        let d = Diagnostic::new("var-001", "msg", "cms/index.json")
            .why("root cause")
            .fix("do this")
            .skill("variation-resolution")
            .avoid("don't grep")
            .citing(["var-001", "ident-002"]);
        assert_eq!(d.code, "var-001");
        assert_eq!(d.message, "msg");
        assert_eq!(d.path, "cms/index.json");
        assert_eq!(d.advocacy.why, "root cause");
        assert_eq!(d.advocacy.substrate_fix, "do this");
        assert_eq!(d.advocacy.skill.as_deref(), Some("variation-resolution"));
        assert!(d.advocacy.anti_pattern.is_some());
        assert_eq!(d.cited_rules.len(), 2);
    }

    #[test]
    fn build_error_io_maps_to_diagnostic() {
        let err = BuildError::Io {
            context: "read cms/".into(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "no such file"),
        };
        let d = err.to_diagnostic();
        assert_eq!(d.code, "io.filesystem");
        assert!(d.message.contains("read cms/"));
        assert!(!d.advocacy.is_empty());
        assert!(d.cited_rules.iter().any(|r| r == "io-001"));
    }

    #[test]
    fn build_error_missing_dependency_maps_to_diagnostic() {
        let err = BuildError::MissingDependency {
            what: "chromium".into(),
            hint: "apt-get install chromium-browser".into(),
        };
        let d = err.to_diagnostic();
        assert_eq!(d.code, "deps.missing");
        assert!(d.message.contains("chromium"));
        assert!(d.advocacy.substrate_fix.contains("install chromium"));
    }

    #[test]
    fn build_error_config_maps_to_diagnostic() {
        let err = BuildError::Config {
            phase: "uniqueness_gate".into(),
            message: "threshold must be >= 0".into(),
        };
        let d = err.to_diagnostic();
        assert_eq!(d.code, "config.invalid");
        assert_eq!(d.path, "forge.toml");
        assert!(d.message.contains("uniqueness_gate"));
    }

    #[test]
    fn build_error_other_maps_to_diagnostic() {
        let err = BuildError::Other {
            phase: "experimental".into(),
            message: "unclassified".into(),
        };
        let d = err.to_diagnostic();
        assert_eq!(d.code, "phase.other");
        assert!(d.message.contains("experimental"));
    }

    #[test]
    fn diagnostic_serializes_without_empty_fields() {
        let d = Diagnostic::new("test", "msg", "");
        let json = serde_json::to_string(&d).expect("serialize");
        // Empty advocacy + empty cited_rules should be skipped.
        assert!(!json.contains("\"advocacy\""));
        assert!(!json.contains("\"cited_rules\""));
        assert!(json.contains("\"code\":\"test\""));
    }
}
