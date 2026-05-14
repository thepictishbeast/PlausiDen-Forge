//! Forge — core types.
//!
//! Pure types + a thin trait surface. No I/O. No filesystem
//! access. No phase-specific logic. The intent is that this
//! crate compiles in <2s on cold cache and is trivially
//! testable; phase implementations are in `forge-phases` and
//! the runner is in `forge-cli`.
//!
//! AVP-2 invariants enforced here:
//!
//! * Every public API has a `BUG ASSUMPTION` comment naming what
//!   could go wrong in the marked block.
//! * No `unwrap` / `expect` in non-test code. The crate-level
//!   clippy deny enforces that mechanically.
//! * `Severity` is `#[non_exhaustive]` so adding a new variant
//!   in a future minor is not a breaking change.
//!
//! T26 (2026-05-06): `attest` submodule adds the Merkle-chain
//! math for build-report continuity (pure, no I/O).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod attest;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Severity of a build finding.
///
/// `Strict` fails the build outright. `Warn` is suppressible in
/// PoC mode but escalates to strict in production mode (the bash
/// forge had this same semantics; preserved here for parity).
///
/// BUG ASSUMPTION: a future tier (`Fatal`, beyond strict — meaning
/// "abort the run and skip remaining phases") would add a variant.
/// `#[non_exhaustive]` keeps that addition non-breaking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Fails the build. Default for security / a11y / link-rot.
    #[serde(alias = "STRICT")]
    Strict,
    /// Suppressible in PoC; escalates to strict in production.
    #[serde(alias = "WARN")]
    Warn,
}

impl Severity {
    /// Whether this severity blocks a successful build in the
    /// given mode.
    ///
    /// BUG ASSUMPTION: `BuildMode::Production` upgrades warns
    /// silently. Callers MUST consult `mode_upgraded_severity()`
    /// before reporting a finding label to the user — otherwise
    /// the terminal output disagrees with the gate decision.
    #[must_use]
    pub fn blocks_in(self, mode: BuildMode) -> bool {
        match (self, mode) {
            (Self::Strict, _) => true,
            (Self::Warn, BuildMode::Production) => true,
            (Self::Warn, _) => false,
        }
    }
}

/// Build mode controls warn-vs-strict escalation.
///
/// BUG ASSUMPTION: future modes (`Static`, `Hybrid`, `Dynamic`)
/// might want different escalation rules. Adding here is the
/// place; everywhere else uses [`Severity::blocks_in`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum BuildMode {
    /// Proof-of-concept — warns are advisory.
    Poc,
    /// Production — warns escalate to strict.
    Production,
    /// Static-site generation pipeline.
    Static,
    /// Hybrid (server-side rendered + hydrated client).
    Hybrid,
    /// Dynamic (server-side rendered every request).
    Dynamic,
}

/// A single finding produced by a phase.
///
/// Findings flow up to the runner which collects them into a
/// [`BuildReport`]. The runner decides exit code by walking the
/// findings + applying [`Severity::blocks_in`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Phase that produced this finding (e.g. "tokens", "csp").
    pub phase: String,
    /// File or asset the finding is attributed to. May be empty
    /// for project-wide findings.
    pub path: String,
    /// Human-readable description. Should be precise enough that
    /// a human can fix the underlying issue without rerunning.
    pub message: String,
    /// Severity at the moment of detection (before mode-driven
    /// upgrade).
    pub severity: Severity,
}

impl Finding {
    /// Make a strict finding without ceremony.
    #[must_use]
    pub fn strict(
        phase: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            phase: phase.into(),
            path: path.into(),
            message: message.into(),
            severity: Severity::Strict,
        }
    }

    /// Make a warn finding without ceremony.
    #[must_use]
    pub fn warn(
        phase: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            phase: phase.into(),
            path: path.into(),
            message: message.into(),
            severity: Severity::Warn,
        }
    }
}

/// What every phase needs from the runner: paths + mode.
///
/// BUG ASSUMPTION: a future phase might need direct access to
/// shared mutable state (e.g. a parsed CMS index). Add a field
/// here, NOT a `&mut` parameter to `Phase::run`. `Phase::run`
/// must stay strictly pure-input → pure-output so phases are
/// trivially parallelizable in a future runner.
#[derive(Debug, Clone)]
pub struct BuildCtx {
    /// Project root (where `forge.toml` lives).
    pub root: PathBuf,
    /// Static asset directory (default `<root>/static`).
    pub static_dir: PathBuf,
    /// Mode driving severity escalation.
    pub mode: BuildMode,
}

/// Trait every phase implements.
///
/// `&self` (not `&mut self`) is intentional: phases own no
/// mutable state across runs; everything they need lives in the
/// `BuildCtx`. This invariant is what lets us parallelize the
/// phase pipeline in a future commit without touching any phase
/// implementation.
///
/// BUG ASSUMPTION: long-running phases (Playwright, etc.) MUST
/// honor cooperative cancellation. The trait doesn't expose a
/// cancellation token yet (the v1 runner is single-threaded
/// blocking). When parallelism lands, this trait gains a
/// `should_continue: &AtomicBool` parameter — non-breaking
/// because callers will get a default impl.
pub trait Phase: Send + Sync {
    /// Display name. Surfaces in terminal output + JSON report.
    fn name(&self) -> &'static str;

    /// Run the phase. Must be deterministic given a fixed
    /// filesystem state — same input → same findings, every time.
    ///
    /// BUG ASSUMPTION: the returned `Vec` is the COMPLETE finding
    /// set for this phase. Returning `Ok(vec![])` on an
    /// I/O-error path silently swallows the failure. Phases MUST
    /// surface I/O errors via `BuildError::Io` (an `Err` return)
    /// rather than emit zero findings.
    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError>;
}

/// The errors a phase can return upward.
///
/// BUG ASSUMPTION: when adding new error variants, pin them
/// `#[non_exhaustive]` on the enum so existing match arms in
/// downstream code don't compile-fail in a minor bump.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BuildError {
    /// Filesystem operation failed in a phase.
    #[error("io: {context}: {source}")]
    Io {
        /// Where in the phase the I/O happened — keep concrete.
        context: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// A phase needed an external dependency we couldn't run.
    #[error("missing dependency: {what} ({hint})")]
    MissingDependency {
        /// What the phase needed (e.g. "openssl").
        what: String,
        /// Suggestion for the operator to fix it.
        hint: String,
    },
    /// A phase was passed an invalid configuration.
    #[error("config error in phase {phase}: {message}")]
    Config {
        /// Phase that received the bad config.
        phase: String,
        /// What was wrong with it.
        message: String,
    },
    /// Anything else, with context.
    #[error("phase {phase} failed: {message}")]
    Other {
        /// Phase that failed.
        phase: String,
        /// Description.
        message: String,
    },
}

/// Build report — what the runner accumulates and the CLI emits.
///
/// Serializable so `reports/build-<ts>.json` round-trips for
/// log replay (T38) and trend analysis.
///
/// `#[serde(default)]` on `duration_ms` so bash-era reports
/// (which never emitted that field) deserialize cleanly and
/// can be replayed alongside Rust-era reports.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BuildReport {
    /// Build mode at run time.
    pub mode: String,
    /// All findings, in phase-order.
    #[serde(default)]
    pub findings: Vec<Finding>,
    /// Strict-finding count (excluding warns).
    #[serde(default)]
    pub strict_count: usize,
    /// Warn-finding count.
    #[serde(default)]
    pub warn_count: usize,
    /// Total wall time across all phases (ms). Bash-era reports
    /// don't have this — defaults to 0 on those.
    #[serde(default)]
    pub duration_ms: u64,
    /// T26: SHA-256 of the canonical-serialized previous build
    /// report. `None` for the genesis report; `Some(hex)` for
    /// every subsequent build. The chain makes the build log
    /// tamper-evident — any mutation, deletion, or out-of-order
    /// insertion breaks the hash chain at the next verification.
    ///
    /// Format: lowercase hex of 32 bytes (64 chars). NOT base64
    /// because hex round-trips through every shell tool without
    /// quoting hazards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_hash: Option<String>,
    /// T26: depth of this report in the Merkle chain (1-indexed).
    /// Operator can spot-check `length == N` against the count
    /// of `reports/build-*.json` files.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub chain_length: u64,
    /// T26: ISO-8601 UTC timestamp when this report was emitted.
    /// Part of the hashed payload — backdating a report breaks
    /// the chain.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub started: String,
    /// T56: Ed25519 signature over the canonical-serialized
    /// report bytes (with this field omitted). Base64 standard
    /// encoding (44 chars). `None` when no signing key is
    /// configured. Verifier checks signature against the public
    /// key in `attest-pubkey.pem`; mismatch = forgery.
    ///
    /// REGRESSION-GUARD: signature is computed AFTER prev_hash
    /// + chain_length + every other field is set; the bytes
    /// hashed for signing OMIT this field (otherwise the hash
    /// would depend on its own value — circular).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

fn is_zero_u64(n: &u64) -> bool {
    *n == 0
}

impl BuildReport {
    /// Add a finding and update the counters.
    pub fn push(&mut self, finding: Finding) {
        match finding.severity {
            Severity::Strict => self.strict_count += 1,
            Severity::Warn => self.warn_count += 1,
        }
        self.findings.push(finding);
    }

    /// Did the build pass the gate in the given mode?
    #[must_use]
    pub fn passed(&self, mode: BuildMode) -> bool {
        self.findings.iter().all(|f| !f.severity.blocks_in(mode))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_strict_always_blocks() {
        assert!(Severity::Strict.blocks_in(BuildMode::Poc));
        assert!(Severity::Strict.blocks_in(BuildMode::Production));
    }

    #[test]
    fn severity_warn_blocks_only_in_production() {
        assert!(!Severity::Warn.blocks_in(BuildMode::Poc));
        assert!(Severity::Warn.blocks_in(BuildMode::Production));
    }

    #[test]
    fn report_push_increments_counts() {
        let mut r = BuildReport::default();
        r.push(Finding::strict("p", "path", "msg"));
        r.push(Finding::warn("p", "path", "msg"));
        r.push(Finding::strict("p", "path", "msg"));
        assert_eq!(r.strict_count, 2);
        assert_eq!(r.warn_count, 1);
        assert_eq!(r.findings.len(), 3);
    }

    #[test]
    fn report_passes_in_poc_when_only_warns() {
        let mut r = BuildReport::default();
        r.push(Finding::warn("p", "path", "msg"));
        assert!(r.passed(BuildMode::Poc));
        assert!(!r.passed(BuildMode::Production));
    }

    #[test]
    fn report_fails_on_any_strict() {
        let mut r = BuildReport::default();
        r.push(Finding::strict("p", "path", "msg"));
        assert!(!r.passed(BuildMode::Poc));
        assert!(!r.passed(BuildMode::Production));
    }
}
