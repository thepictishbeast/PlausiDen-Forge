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
// T96 cleanup: discipline gate (T92) requires deny-not-warn so a
// missing doc on a new public item fails CI at PR time, not at
// the next release-prep audit. Pre-existing warn-level violations
// in attest.rs::AttestError struct-variant fields cleaned up
// alongside this flip.
#![deny(missing_docs)]

pub mod attest;
pub mod diagnostic;
pub mod fingerprint;
pub mod fingerprint_migration;
pub mod fingerprint_registry;
pub mod pipeline;
pub mod site_identity;
pub mod tenant_corpus;

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

/// Substrate-correct guidance attached to a finding.
///
/// Per `[[tool-starvation-anti-pattern]]` + `[[substrate-only-path]]`
/// + task #151 (`docs/TOOL_ADVOCACY.md`): every refusal points at the
/// named substrate fix. The Advocacy struct carries the four pieces
/// of advocacy that travel alongside a finding into reports + JSON +
/// terminal output:
///
///   * `why` — one-sentence root cause (not just symptom)
///   * `substrate_fix` — the exact command / file / field that
///     resolves the finding via the substrate-correct path
///   * `skill` — pointer to `skills/<name>/SKILL.md` when a
///     procedure applies
///   * `anti_pattern` — the bash/grep/curl alternative the operator
///     likely reached for; named explicitly so the substrate path
///     is unambiguous
///
/// Per `[[backward-compat-version-discipline]]` additive change
/// classification: populating Advocacy is non-breaking; empty
/// Advocacy skips serialization to keep legacy reports byte-
/// identical.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Advocacy {
    /// One-sentence root cause. Example:
    /// "rendered HTML references an undeclared backend slug".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub why: String,
    /// The substrate-correct command or workflow that resolves the
    /// finding. Be specific: exact command, exact file, exact field.
    /// Example: `add \`[[backend]] id = "cta-signup"\` to
    /// backends.toml in the same commit`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub substrate_fix: String,
    /// Skill playbook reference (slug from `skills/<name>/SKILL.md`),
    /// e.g. `"add-loom-primitive"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    /// Bash/grep/curl alternative the operator likely reached for —
    /// named explicitly so the substrate alternative is unambiguous.
    /// Example: `"don't \`grep -r data-backend static/\` — use
    /// \`forge audit phantom_button\`"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anti_pattern: Option<String>,
}

impl Advocacy {
    /// True when no advocacy field has been populated. Used to skip
    /// serialization of legacy / un-retrofitted findings so JSON
    /// reports stay byte-identical until phases adopt the trait.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.why.is_empty()
            && self.substrate_fix.is_empty()
            && self.skill.is_none()
            && self.anti_pattern.is_none()
    }
}

/// Anything that can carry typed substrate advocacy. Implemented by
/// `Finding` today; other diagnostic types (BuildError variants,
/// loom-lint warnings, crawler journey-step failures) can adopt the
/// trait as the Phase-2 retrofit of task #201 lands.
pub trait WithAdvocacy {
    /// Borrow the attached advocacy. Returns a reference to allow
    /// callers to inspect / render without cloning. May be empty.
    fn advocacy(&self) -> &Advocacy;
}

/// A single finding produced by a phase.
///
/// Findings flow up to the runner which collects them into a
/// [`BuildReport`]. The runner decides exit code by walking the
/// findings + applying [`Severity::blocks_in`].
///
/// `enforces_rules` (task #177) lets a finding cite the doctrine
/// rule ids it enforces, so consumers can trace finding → doctrine
/// → rationale via `forge doctrine query --rule <id>`. Optional;
/// rules-aware phases populate it, legacy phases leave it empty
/// during migration.
///
/// `advocacy` (task #201) lets a finding carry the substrate-correct
/// fix alongside the diagnosis — see `docs/TOOL_ADVOCACY.md` for
/// the template + the chained `.why()` / `.fix()` / `.skill()` /
/// `.avoid()` builder methods.
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
    /// AVP-Doctrine rule ids this finding cites (e.g. `["prim-001",
    /// "a11y-004"]`). Empty for findings that don't (yet) map to
    /// codified rules. Surfaced in reports as "(rule-XXX)" so
    /// consumers can run `forge doctrine query --rule <id>` to read
    /// the rule's rationale + enforcement contract.
    ///
    /// Skipped from JSON when empty so legacy reports stay
    /// byte-identical (per [[backward-compat-version-discipline]]
    /// additive change classification).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enforces_rules: Vec<String>,
    /// Substrate-correct guidance the operator (or AI agent) follows
    /// to resolve the finding. Populated via the chained
    /// `.why()` / `.fix()` / `.skill()` / `.avoid()` builders.
    /// Skipped from JSON when empty (additive change classification).
    #[serde(default, skip_serializing_if = "Advocacy::is_empty")]
    pub advocacy: Advocacy,
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
            enforces_rules: Vec::new(),
            advocacy: Advocacy::default(),
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
            enforces_rules: Vec::new(),
            advocacy: Advocacy::default(),
        }
    }

    /// Attach one or more AVP-Doctrine rule ids to this finding so
    /// consumers can trace it back to the codified rationale.
    /// Returns `self` for chained construction:
    /// `Finding::strict(...).citing(["prim-001"])`.
    #[must_use]
    pub fn citing<I, S>(mut self, rule_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.enforces_rules
            .extend(rule_ids.into_iter().map(Into::into));
        self
    }

    /// Attach the one-sentence root-cause explanation to this
    /// finding's advocacy. Per `docs/TOOL_ADVOCACY.md`:
    /// `Finding::strict(...).why("rendered HTML references an
    /// undeclared backend slug")`.
    #[must_use]
    pub fn why(mut self, why: impl Into<String>) -> Self {
        self.advocacy.why = why.into();
        self
    }

    /// Attach the substrate-correct fix to this finding's advocacy.
    /// Be specific: exact command, exact file, exact field.
    /// `Finding::strict(...).fix("add \`[[backend]] id = \"cta-
    /// signup\"\` to backends.toml in the same commit")`.
    #[must_use]
    pub fn fix(mut self, substrate_fix: impl Into<String>) -> Self {
        self.advocacy.substrate_fix = substrate_fix.into();
        self
    }

    /// Attach a skill-playbook pointer to this finding's advocacy.
    /// The slug should match a `skills/<slug>/SKILL.md` file.
    #[must_use]
    pub fn skill(mut self, skill: impl Into<String>) -> Self {
        self.advocacy.skill = Some(skill.into());
        self
    }

    /// Attach the anti-pattern (bash/grep/curl alternative) the
    /// operator likely reached for to this finding's advocacy.
    /// `Finding::strict(...).avoid("don't \`grep -r data-backend
    /// static/\` — use \`forge audit phantom_button\`")`.
    #[must_use]
    pub fn avoid(mut self, anti_pattern: impl Into<String>) -> Self {
        self.advocacy.anti_pattern = Some(anti_pattern.into());
        self
    }
}

impl WithAdvocacy for Finding {
    fn advocacy(&self) -> &Advocacy {
        &self.advocacy
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

    /// Declare which engine version range this phase supports.
    ///
    /// Per `[[backward-compat-version-discipline]]` doctrine + AVP-
    /// Doctrine `VERSION_DISCIPLINE.md` § Pin-by-default: every
    /// substrate component declares its supported engine range so
    /// operators know whether a phase is safe to run against their
    /// pinned `forge.toml` `[platform].forge_version`.
    ///
    /// Returns a version-range expression as a `&'static str`.
    /// Format: `">=X.Y.Z[,<A.B.C]"` (subset of `cargo` version
    /// requirements; comma-separated `>=` / `<` / `=` predicates).
    ///
    /// Default: `">=0.1.0,<2.0.0"` — compatible with every 0.x and
    /// 1.x engine. Phases that depend on engine-specific surfaces
    /// (new API in 1.5+, removed in 2.x) override to narrow the
    /// range.
    ///
    /// Closes task #143 (backcompat-v7).
    fn supported_engines(&self) -> &'static str {
        ">=0.1.0,<2.0.0"
    }
}

/// Parse a comma-separated cargo-style version-range expression
/// into a vector of `(predicate, version_string)` pairs.
///
/// Accepts predicates `>=`, `<`, `=`, `<=`, `>`. Returns `None` if
/// any predicate is malformed or any version string fails the
/// `MAJOR.MINOR.PATCH` semver-2.0.0 check (per
/// `crates/forge-phases/src/semver_enforcement.rs` parser).
///
/// Companion to `Phase::supported_engines`. Used by `forge orient`
/// / pipeline gates to decide whether a phase is compatible with
/// the current engine version.
#[must_use]
pub fn parse_version_range(range: &str) -> Option<Vec<(VersionPredicate, String)>> {
    let mut out = Vec::new();
    for part in range.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (pred, ver_str) = if let Some(rest) = part.strip_prefix(">=") {
            (VersionPredicate::AtLeast, rest.trim())
        } else if let Some(rest) = part.strip_prefix("<=") {
            (VersionPredicate::AtMost, rest.trim())
        } else if let Some(rest) = part.strip_prefix("<") {
            (VersionPredicate::LessThan, rest.trim())
        } else if let Some(rest) = part.strip_prefix(">") {
            (VersionPredicate::GreaterThan, rest.trim())
        } else if let Some(rest) = part.strip_prefix('=') {
            (VersionPredicate::Equal, rest.trim())
        } else {
            return None;
        };
        if !is_canonical_semver(ver_str) {
            return None;
        }
        out.push((pred, ver_str.to_owned()));
    }
    Some(out)
}

/// Comparison predicates supported in a `Phase::supported_engines`
/// range expression. See [`parse_version_range`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionPredicate {
    /// `>=`
    AtLeast,
    /// `<=`
    AtMost,
    /// `>`
    GreaterThan,
    /// `<`
    LessThan,
    /// `=`
    Equal,
}

/// `MAJOR.MINOR.PATCH` (with optional `-pre` / `+build`) semver
/// 2.0.0 acceptance check. Companion to
/// `forge-phases::semver_enforcement::parse_semver` — same shape,
/// re-implemented here so forge-core doesn't depend on
/// forge-phases.
fn is_canonical_semver(s: &str) -> bool {
    let core = s.split('-').next().unwrap_or(s);
    let core = core.split('+').next().unwrap_or(core);
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Compare two canonical `MAJOR.MINOR.PATCH` semver strings.
/// Pre-release / build metadata are ignored for ordering — the
/// pre-release ordering rules of semver 2.0.0 are out of scope
/// for the substrate's engine-range check (engines don't ship
/// pre-releases). Returns `None` if either input fails the
/// canonical-form check.
#[must_use]
pub fn semver_cmp(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    if !is_canonical_semver(a) || !is_canonical_semver(b) {
        return None;
    }
    let a_core = a.split('-').next().unwrap_or(a).split('+').next().unwrap_or(a);
    let b_core = b.split('-').next().unwrap_or(b).split('+').next().unwrap_or(b);
    let a_parts: Vec<u64> = a_core.split('.').filter_map(|p| p.parse().ok()).collect();
    let b_parts: Vec<u64> = b_core.split('.').filter_map(|p| p.parse().ok()).collect();
    Some(a_parts.cmp(&b_parts))
}

/// True if `target_version` satisfies the comma-separated
/// `range` (e.g. `">=0.1.0,<2.0.0"`). Returns `None` if either
/// is malformed (caller decides whether to treat that as
/// fail-closed or fail-open per `[[deterministic-first-lfi-optional]]`).
///
/// Closes task #143 (backcompat-v7): `forge orient` + the pipeline
/// runner call this to decide whether to surface a compatibility
/// warning at session start.
#[must_use]
pub fn engine_satisfies(target_version: &str, range: &str) -> Option<bool> {
    if !is_canonical_semver(target_version) {
        return None;
    }
    let predicates = parse_version_range(range)?;
    for (pred, ver) in &predicates {
        let order = semver_cmp(target_version, ver)?;
        let ok = match pred {
            VersionPredicate::AtLeast => order != std::cmp::Ordering::Less,
            VersionPredicate::AtMost => order != std::cmp::Ordering::Greater,
            VersionPredicate::GreaterThan => order == std::cmp::Ordering::Greater,
            VersionPredicate::LessThan => order == std::cmp::Ordering::Less,
            VersionPredicate::Equal => order == std::cmp::Ordering::Equal,
        };
        if !ok {
            return Some(false);
        }
    }
    Some(true)
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

    // -----------------------------------------------------------------
    // Engine version-range tests — task #143
    // -----------------------------------------------------------------

    #[test]
    fn parse_version_range_canonical() {
        let r = parse_version_range(">=0.1.0,<2.0.0").expect("parse");
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].0, VersionPredicate::AtLeast);
        assert_eq!(r[0].1, "0.1.0");
        assert_eq!(r[1].0, VersionPredicate::LessThan);
        assert_eq!(r[1].1, "2.0.0");
    }

    #[test]
    fn parse_version_range_rejects_malformed() {
        // Floating pointer is not semver — reject.
        assert!(parse_version_range(">=latest").is_none());
        // Unknown predicate.
        assert!(parse_version_range("~=1.0.0").is_none());
        // Missing predicate.
        assert!(parse_version_range("0.1.0").is_none());
        // Non-canonical semver.
        assert!(parse_version_range(">=1.0").is_none());
        assert!(parse_version_range(">=v1.0.0").is_none());
    }

    #[test]
    fn engine_satisfies_in_range() {
        assert_eq!(engine_satisfies("1.0.0", ">=0.1.0,<2.0.0"), Some(true));
        assert_eq!(engine_satisfies("0.1.0", ">=0.1.0,<2.0.0"), Some(true));
        assert_eq!(engine_satisfies("1.99.99", ">=0.1.0,<2.0.0"), Some(true));
    }

    #[test]
    fn engine_satisfies_out_of_range() {
        assert_eq!(engine_satisfies("2.0.0", ">=0.1.0,<2.0.0"), Some(false));
        assert_eq!(engine_satisfies("0.0.9", ">=0.1.0,<2.0.0"), Some(false));
        assert_eq!(engine_satisfies("3.5.1", ">=0.1.0,<2.0.0"), Some(false));
    }

    #[test]
    fn engine_satisfies_returns_none_on_invalid_inputs() {
        // Invalid target version.
        assert!(engine_satisfies("latest", ">=0.1.0,<2.0.0").is_none());
        // Invalid range.
        assert!(engine_satisfies("1.0.0", "~=1.0.0").is_none());
    }

    #[test]
    fn semver_cmp_canonical() {
        use std::cmp::Ordering;
        assert_eq!(semver_cmp("1.0.0", "1.0.0"), Some(Ordering::Equal));
        assert_eq!(semver_cmp("1.0.0", "0.9.9"), Some(Ordering::Greater));
        assert_eq!(semver_cmp("0.9.9", "1.0.0"), Some(Ordering::Less));
        assert_eq!(semver_cmp("1.10.0", "1.9.0"), Some(Ordering::Greater));
        // Numeric, not lexicographic.
        assert_eq!(semver_cmp("2.0.0", "10.0.0"), Some(Ordering::Less));
    }

    #[test]
    fn default_supported_engines_is_load_bearing_v0_and_v1() {
        // A phase with the default impl should report the canonical
        // engine range. The default returns ">=0.1.0,<2.0.0" — assert
        // it's a contract callers can rely on without instantiating
        // every phase.
        struct DummyPhase;
        impl Phase for DummyPhase {
            fn name(&self) -> &'static str {
                "dummy"
            }
            fn run(&self, _ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
                Ok(vec![])
            }
        }
        assert_eq!(DummyPhase.supported_engines(), ">=0.1.0,<2.0.0");
        assert_eq!(
            engine_satisfies("1.0.0", DummyPhase.supported_engines()),
            Some(true)
        );
        assert_eq!(
            engine_satisfies("2.0.0", DummyPhase.supported_engines()),
            Some(false)
        );
    }

    #[test]
    fn phase_can_narrow_supported_engines() {
        // A phase that only supports 1.x can override the default.
        struct OnlyV1;
        impl Phase for OnlyV1 {
            fn name(&self) -> &'static str {
                "v1_only"
            }
            fn run(&self, _ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
                Ok(vec![])
            }
            fn supported_engines(&self) -> &'static str {
                ">=1.0.0,<2.0.0"
            }
        }
        let phase = OnlyV1;
        assert_eq!(
            engine_satisfies("0.9.9", phase.supported_engines()),
            Some(false)
        );
        assert_eq!(
            engine_satisfies("1.0.0", phase.supported_engines()),
            Some(true)
        );
        assert_eq!(
            engine_satisfies("2.0.0", phase.supported_engines()),
            Some(false)
        );
    }

    // -----------------------------------------------------------------
    // Advocacy tests — task #201 phase 1
    // -----------------------------------------------------------------

    #[test]
    fn advocacy_default_is_empty() {
        let a = Advocacy::default();
        assert!(a.is_empty());
    }

    #[test]
    fn advocacy_is_not_empty_after_any_field_populated() {
        let mut a = Advocacy::default();
        a.why = "x".into();
        assert!(!a.is_empty());

        let mut a = Advocacy::default();
        a.substrate_fix = "x".into();
        assert!(!a.is_empty());

        let mut a = Advocacy::default();
        a.skill = Some("x".into());
        assert!(!a.is_empty());

        let mut a = Advocacy::default();
        a.anti_pattern = Some("x".into());
        assert!(!a.is_empty());
    }

    #[test]
    fn finding_advocacy_builders_populate_fields() {
        let f = Finding::strict("phantom_button", "static/index.html", "msg")
            .citing(["sec-007"])
            .why("rendered HTML references an undeclared backend slug")
            .fix("add `[[backend]] id = \"cta-signup\"` to backends.toml")
            .skill("author-cms-content")
            .avoid("don't `grep -r data-backend static/` — use `forge audit phantom_button`");

        assert_eq!(f.enforces_rules, vec!["sec-007".to_string()]);
        assert_eq!(
            f.advocacy.why,
            "rendered HTML references an undeclared backend slug"
        );
        assert!(f.advocacy.substrate_fix.starts_with("add `[[backend]]"));
        assert_eq!(f.advocacy.skill.as_deref(), Some("author-cms-content"));
        assert!(f.advocacy.anti_pattern.as_deref().unwrap().contains("grep"));
        assert!(!f.advocacy.is_empty());
    }

    #[test]
    fn finding_with_advocacy_trait_exposes_borrow() {
        let f = Finding::strict("p", "p", "m").why("root cause");
        let a: &Advocacy = f.advocacy();
        assert_eq!(a.why, "root cause");
    }

    #[test]
    fn finding_empty_advocacy_round_trips_unchanged() {
        // Legacy finding (no advocacy) should serialize without an
        // "advocacy" field (additive change per backward-compat doctrine).
        let f = Finding::strict("p", "path", "msg");
        let json = serde_json::to_string(&f).expect("serialize");
        assert!(
            !json.contains("\"advocacy\""),
            "empty advocacy should be skipped to keep legacy reports byte-identical: {json}"
        );
    }

    #[test]
    fn finding_populated_advocacy_serializes() {
        let f = Finding::strict("p", "path", "msg").why("cause").fix("do x");
        let json = serde_json::to_string(&f).expect("serialize");
        assert!(json.contains("\"advocacy\""));
        assert!(json.contains("\"why\":\"cause\""));
        assert!(json.contains("\"substrate_fix\":\"do x\""));

        // Round-trip: deserialize back + assert content matches.
        let back: Finding = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.advocacy.why, "cause");
        assert_eq!(back.advocacy.substrate_fix, "do x");
        assert!(back.advocacy.skill.is_none());
        assert!(back.advocacy.anti_pattern.is_none());
    }
}
