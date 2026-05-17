//! `validate_cms` — invoke `loom validate --input cms/` and
//! surface per-file failures as forge findings.
//!
//! Rust port of bash `phase_validate_cms` (T53). Entry-gate of
//! the build pipeline: no other phase produces value if CMS
//! input is malformed.
//!
//! ## Doctrine applied (per supersociety stack)
//!
//! * **Composition** — `ValidateCmsPhase` is a ZST.
//! * **ADT findings** — `enum ValidateCmsFinding` covers every
//!   result class. New failure shapes land as new variants.
//! * **Capability discovery** — `loom` binary resolved via
//!   `LOOM_BIN` env first (operator override), then a curated
//!   list of conventional cargo-target paths, then `PATH`.
//! * **No shell** — `Command::new("loom").arg(...)` direct
//!   invocation. Cms path flows through PathBuf so metachars
//!   never touch a shell.
//! * **Typed output parse** — looks for `^loom validate:` and
//!   `^\s+fail\s` lines specifically; everything else is dim
//!   informational. The bash version had a bug where every
//!   matching line emitted its own strict (3-per-bug); Rust
//!   port collapses to one finding per failed file.
//! * **Deny `unsafe_code`**, no `unwrap`/`expect` in lib code.

use std::path::{Path, PathBuf};
use std::process::Command;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// What this phase concluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidateCmsFinding {
    /// `cms/` directory missing — nothing to validate. Not a
    /// finding at all (silently skipped at run-time).
    NoCmsDir,
    /// `loom` binary couldn't be found at any known location.
    /// STRICT — without it, the gate is open.
    LoomMissing { searched: Vec<PathBuf> },
    /// One CMS file failed validation. STRICT.
    FileInvalid { path: String, message: String },
    /// `loom validate` errored unexpectedly (exit ≥ 2). STRICT.
    LoomErrored {
        exit_code: i32,
        stderr_excerpt: String,
    },
}

impl ValidateCmsFinding {
    /// Render to forge `Finding`. Severity fixed per variant.
    pub fn as_finding(&self) -> Option<Finding> {
        const PHASE: &str = "validate_cms";
        match self {
            Self::NoCmsDir => None, // silent skip
            Self::LoomMissing { searched } => {
                let paths = searched
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                Some(Finding::strict(
                    PHASE,
                    "loom-cli",
                    format!(
                        "loom binary not found (searched: {paths}) — \
                         run `cargo build --release -p loom-cli` in PlausiDen-Loom \
                         or set LOOM_BIN env"
                    ),
                ))
            }
            Self::FileInvalid { path, message } => {
                Some(Finding::strict(PHASE, path.clone(), message.clone()))
            }
            Self::LoomErrored {
                exit_code,
                stderr_excerpt,
            } => Some(Finding::strict(
                PHASE,
                "loom validate",
                format!("loom validate errored (exit {exit_code}): {stderr_excerpt}"),
            )),
        }
    }
}

// ============================================================
// Capability discovery — locate the loom binary.
// ============================================================

/// Resolve the loom binary path. Returns the FIRST candidate
/// that exists and is executable.
fn resolve_loom_bin() -> Result<PathBuf, Vec<PathBuf>> {
    if let Ok(env) = std::env::var("LOOM_BIN") {
        let p = PathBuf::from(&env);
        if is_exec(&p) {
            return Ok(p);
        }
        return Err(vec![p]);
    }
    let candidates = [
        PathBuf::from("/home/user/cargo-target/release/loom"),
        PathBuf::from("/home/user/cargo-target/debug/loom"),
        PathBuf::from("./target/release/loom"),
        PathBuf::from("./target/debug/loom"),
    ];
    for c in &candidates {
        if is_exec(c) {
            return Ok(c.clone());
        }
    }
    // Last resort: PATH lookup.
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(':') {
            let p = Path::new(dir).join("loom");
            if is_exec(&p) {
                return Ok(p);
            }
        }
    }
    Err(candidates.to_vec())
}

fn is_exec(p: &Path) -> bool {
    // SECURITY: stat-only check. We don't open or read the file
    // to test executability — minimum-information path that
    // avoids accidental side effects on / dev nodes etc.
    match std::fs::metadata(p) {
        Ok(md) if md.is_file() => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                md.permissions().mode() & 0o111 != 0
            }
            #[cfg(not(unix))]
            {
                true
            }
        }
        _ => false,
    }
}

// ============================================================
// Phase impl
// ============================================================

/// `validate_cms` phase.
#[derive(Debug, Default)]
pub struct ValidateCmsPhase;

impl Phase for ValidateCmsPhase {
    fn name(&self) -> &'static str {
        "validate_cms"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        tracing::debug!("validate_cms: enter");

        let cms_dir = ctx.root.join("cms");
        if !cms_dir.is_dir() {
            tracing::info!(?cms_dir, "validate_cms: no cms/ — nothing to validate");
            return Ok(vec![]);
        }

        let loom_bin = match resolve_loom_bin() {
            Ok(p) => p,
            Err(searched) => {
                tracing::error!(?searched, "validate_cms: loom binary not found");
                return Ok(ValidateCmsFinding::LoomMissing { searched }
                    .as_finding()
                    .into_iter()
                    .collect());
            }
        };

        tracing::info!(?loom_bin, ?cms_dir, "validate_cms: invoking loom validate");

        // SECURITY: Command::new + arg-vec — no shell parsing.
        // Cms path flows as a PathBuf, never spliced into a string.
        let output = Command::new(&loom_bin)
            .arg("validate")
            .arg("--input")
            .arg(&cms_dir)
            .output()
            .map_err(|source| BuildError::Io {
                context: format!("invoke {} validate", loom_bin.display()),
                source,
            })?;

        let exit = output.status.code().unwrap_or(-1);
        tracing::debug!(exit, "validate_cms: loom validate exited");

        if exit == 0 {
            // Surface the summary line dim-printed; no findings.
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.starts_with("loom validate:") {
                    tracing::info!(target: "forge_phases::validate_cms::summary", "{line}");
                    break;
                }
            }
            return Ok(vec![]);
        }
        if exit >= 2 {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let excerpt = stderr.chars().take(200).collect::<String>();
            return Ok(ValidateCmsFinding::LoomErrored {
                exit_code: exit,
                stderr_excerpt: excerpt,
            }
            .as_finding()
            .into_iter()
            .collect());
        }

        // Exit 1 — at least one file failed. Parse `  fail …` lines.
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");
        let parsed = parse_loom_validate_output(&combined);
        Ok(parsed.into_iter().filter_map(|p| p.as_finding()).collect())
    }
}

/// Pure parser: walks loom-validate output, returns one
/// `FileInvalid` per `  fail ` line. Suppresses the per-run
/// summary line (the bash version emitted it as a 3rd finding).
///
/// REGRESSION-GUARD: bash version emitted ONE finding per matching
/// line (fail OR summary OR tail), so a single bad file appeared
/// as 3 strict findings. The Rust port emits ONE per actual file
/// failure.
fn parse_loom_validate_output(text: &str) -> Vec<ValidateCmsFinding> {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("fail") else {
            continue;
        };
        // Expected shape:
        //   fail   cms/about.json: <message>
        let rest = rest.trim_start();
        let (path, msg) = rest.split_once(':').unwrap_or((rest, ""));
        out.push(ValidateCmsFinding::FileInvalid {
            path: path.trim().to_owned(),
            message: msg.trim().to_owned(),
        });
    }
    out
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::Severity;

    #[test]
    fn no_cms_dir_finding_renders_to_none() {
        assert!(ValidateCmsFinding::NoCmsDir.as_finding().is_none());
    }

    #[test]
    fn loom_missing_finding_is_strict() {
        let f = ValidateCmsFinding::LoomMissing {
            searched: vec![PathBuf::from("/nope")],
        }
        .as_finding()
        .expect("renders");
        assert_eq!(f.severity, Severity::Strict);
    }

    #[test]
    fn file_invalid_finding_is_strict() {
        let f = ValidateCmsFinding::FileInvalid {
            path: "cms/about.json".into(),
            message: "schema violation: missing field 'title'".into(),
        }
        .as_finding()
        .expect("renders");
        assert_eq!(f.severity, Severity::Strict);
        assert_eq!(f.path, "cms/about.json");
    }

    #[test]
    fn parser_extracts_one_finding_per_fail_line() {
        let raw = "  ok      cms/index.json\n  fail   cms/about.json: missing title\n  fail   cms/contact.json: bad path\nloom validate: 3 file(s), 1 ok, 2 failed";
        let parsed = parse_loom_validate_output(raw);
        assert_eq!(parsed.len(), 2);
        assert!(matches!(
            &parsed[0],
            ValidateCmsFinding::FileInvalid { path, .. } if path == "cms/about.json"
        ));
    }

    #[test]
    fn parser_does_not_double_count_on_summary() {
        // The bash bug: a single bad file produced 3 strict findings
        // because parser matched fail line + summary line + tail.
        let raw = "  fail   cms/about.json: missing title\nloom validate: 1 file(s), 0 ok, 1 failed\nloom validate: at least one file failed";
        let parsed = parse_loom_validate_output(raw);
        assert_eq!(parsed.len(), 1, "summary lines must not become findings");
    }

    #[test]
    fn parser_handles_empty_input() {
        assert!(parse_loom_validate_output("").is_empty());
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Parser must not panic on arbitrary loom-validate output.
        #[test]
        fn parser_never_panics(input in ".{0,2000}") {
            let _ = parse_loom_validate_output(&input);
        }

        /// All produced FileInvalid findings render successfully.
        #[test]
        fn produced_findings_always_render(
            path in "[a-z/.]{1,40}",
            msg in ".{0,200}",
        ) {
            let f = ValidateCmsFinding::FileInvalid { path, message: msg };
            prop_assert!(f.as_finding().is_some());
        }
    }
}
