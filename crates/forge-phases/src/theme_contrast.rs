//! `theme_contrast` — gate every build on WCAG AA mathematical
//! contrast across every theme. Wraps `loom theme contrast`
//! (T29 in PlausiDen-Loom).
//!
//! T29b. No theme can ship that fails the contrast threshold.
//!
//! ## Doctrine
//!
//! * Composition: ZST phase, Phase trait impl.
//! * Capability discovery: loom binary resolved via LOOM_BIN
//!   env first, then conventional cargo-target paths, then PATH.
//! * Typed JSON-ish output parse — loom theme contrast prints
//!   a tabular layout that's stable enough to grep for "FAIL"
//!   lines. We could move to JSON output in a future loom CLI
//!   release; for now the text format is the contract.
//! * ADT findings, no `unwrap`/`expect`, deny `unsafe_code`,
//!   property-based tests.

use std::path::{Path, PathBuf};
use std::process::Command;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

const PHASE: &str = "theme_contrast";

/// Result classes the phase can produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeContrastFinding {
    /// loom binary not found at any known location. WARN —
    /// devs without it shouldn't be blocked from building.
    LoomMissing {
        /// Paths the resolver looked in before giving up.
        searched: Vec<PathBuf>,
    },
    /// No skin file. WARN.
    SkinMissing {
        /// Paths the resolver looked in before giving up.
        searched: Vec<PathBuf>,
    },
    /// loom theme contrast errored unexpectedly (exit ≥ 2). WARN.
    LoomErrored {
        /// Process exit code returned by loom.
        exit_code: i32,
        /// First ~256 chars of stderr.
        stderr_excerpt: String,
    },
    /// One specific (theme, pair) is below threshold. STRICT.
    PairBelowThreshold {
        /// Theme that failed contrast.
        theme: String,
        /// Token pair that failed (e.g. `fg/bg`).
        pair: String,
        /// Computed contrast ratio, as the string loom emits.
        ratio: String,
    },
}

impl ThemeContrastFinding {
    /// Render to a typed forge finding. Severity is fixed per
    /// variant; consumers can't accidentally upgrade/downgrade.
    pub fn as_finding(&self) -> Finding {
        match self {
            Self::LoomMissing { searched } => {
                let paths = searched
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                Finding::warn(
                    PHASE,
                    "loom-cli",
                    format!(
                        "loom binary not found ({paths}) — \
                         contrast not verified (build cargo run --release -p loom-cli \
                         in PlausiDen-Loom or set LOOM_BIN env)"
                    ),
                )
            }
            Self::SkinMissing { searched } => {
                let paths = searched
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                Finding::warn(
                    PHASE,
                    "static/",
                    format!("no skin css at any known path ({paths}) — contrast not verified"),
                )
            }
            Self::LoomErrored {
                exit_code,
                stderr_excerpt,
            } => Finding::warn(
                PHASE,
                "loom theme contrast",
                format!("loom theme contrast errored (exit {exit_code}): {stderr_excerpt}"),
            ),
            Self::PairBelowThreshold { theme, pair, ratio } => Finding::strict(
                PHASE,
                format!("theme={theme}"),
                format!(
                    "WCAG AA fail: {pair} on {theme} = {ratio} (text will be \
                     unreadable for users with low-vision or in bright sunlight)"
                ),
            ),
        }
    }
}

fn resolve_loom_bin() -> Result<PathBuf, Vec<PathBuf>> {
    if let Ok(env) = std::env::var("LOOM_BIN") {
        let p = PathBuf::from(&env);
        if p.is_file() {
            return Ok(p);
        }
        return Err(vec![p]);
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    // Sibling Loom checkout (canonical `~/projects/PlausiDen-Loom/`
    // layout per the canonical-dir doctrine).
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(parent) = cwd.parent() {
            candidates.push(parent.join("PlausiDen-Loom/target/release/loom"));
            candidates.push(parent.join("PlausiDen-Loom/target/debug/loom"));
        }
    }
    // Local target/ (current dir).
    candidates.push(PathBuf::from("./target/release/loom"));
    candidates.push(PathBuf::from("./target/debug/loom"));
    // PATH fallback.
    if let Ok(path_env) = std::env::var("PATH") {
        for dir in path_env.split(':') {
            candidates.push(PathBuf::from(dir).join("loom"));
        }
    }
    for c in &candidates {
        if c.is_file() {
            return Ok(c.clone());
        }
    }
    Err(candidates)
}

fn resolve_skin(static_dir: &Path) -> Result<PathBuf, Vec<PathBuf>> {
    let candidates = ["loom.css", "loom-skin.css"];
    let mut tried = Vec::new();
    for name in candidates {
        let p = static_dir.join(name);
        if p.is_file() {
            return Ok(p);
        }
        tried.push(p);
    }
    Err(tried)
}

/// Parse loom theme contrast output for FAIL lines.
///
/// Format expected (stable text contract — see loom-cli T29):
///
/// ```text
///   theme           pair                          ratio   status
///   --------------  ----------------------------  ------  ------
///   default         ink-on-canvas                 21.00   ok
///   sepia           primary-fg-on-primary          5.28   ok
///   broken-theme    ink-on-canvas                  3.10   FAIL
/// ```
///
/// REGRESSION-GUARD: the loom CLI may add a `--json` flag in a
/// future release. When that lands, switch this parser to typed
/// JSON. Until then, the text format is the contract; the parser
/// must tolerate variable whitespace + future trailing columns.
pub fn parse_contrast_output(text: &str) -> Vec<ThemeContrastFinding> {
    let mut hits = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.ends_with("FAIL") {
            continue;
        }
        // Split on whitespace into columns.
        let cols: Vec<&str> = trimmed.split_whitespace().collect();
        if cols.len() < 4 {
            continue;
        }
        // theme=cols[0], pair=cols[1], ratio=cols[len-2], status=cols[len-1]
        let theme = cols[0].to_owned();
        let pair = cols[1].to_owned();
        let ratio = cols[cols.len() - 2].to_owned();
        hits.push(ThemeContrastFinding::PairBelowThreshold { theme, pair, ratio });
    }
    hits
}

/// Forge phase that runs the loom theme-contrast audit and
/// projects its output into typed [`ThemeContrastFinding`]s.
#[derive(Debug, Default)]
pub struct ThemeContrastPhase;

impl Phase for ThemeContrastPhase {
    fn name(&self) -> &'static str {
        PHASE
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        tracing::debug!("theme_contrast: enter");

        let loom_bin = match resolve_loom_bin() {
            Ok(p) => p,
            Err(searched) => {
                tracing::warn!(?searched, "theme_contrast: loom missing");
                return Ok(vec![
                    ThemeContrastFinding::LoomMissing { searched }.as_finding()
                ]);
            }
        };
        let skin = match resolve_skin(&ctx.static_dir) {
            Ok(p) => p,
            Err(searched) => {
                tracing::warn!(?searched, "theme_contrast: skin missing");
                return Ok(vec![
                    ThemeContrastFinding::SkinMissing { searched }.as_finding()
                ]);
            }
        };

        // Configurable threshold via env. Default 4.5 (WCAG AA
        // normal text). Operators wanting AAA can set
        // FORGE_CONTRAST_MIN_RATIO=7.0.
        let min_ratio = std::env::var("FORGE_CONTRAST_MIN_RATIO")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(4.5);

        tracing::info!(
            ?loom_bin,
            ?skin,
            min_ratio,
            "theme_contrast: invoking loom theme contrast"
        );

        // SECURITY: arg-vec, no shell.
        let output = Command::new(&loom_bin)
            .args(["theme", "contrast", "--skin"])
            .arg(&skin)
            .args(["--min-ratio", &min_ratio.to_string()])
            .output()
            .map_err(|source| BuildError::Io {
                context: format!("invoke {} theme contrast", loom_bin.display()),
                source,
            })?;

        let exit = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout);

        if exit == 0 {
            tracing::debug!("theme_contrast: all pairs ≥ {min_ratio}:1");
            return Ok(vec![]);
        }
        if exit >= 2 {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(vec![ThemeContrastFinding::LoomErrored {
                exit_code: exit,
                stderr_excerpt: stderr.chars().take(200).collect(),
            }
            .as_finding()]);
        }

        // Exit 1: at least one pair below threshold. Parse the
        // FAIL lines.
        let parsed = parse_contrast_output(&stdout);
        if parsed.is_empty() {
            // Loom said FAIL but we couldn't parse rows — surface
            // a generic strict so the build doesn't silently pass.
            return Ok(vec![Finding::strict(
                PHASE,
                "loom theme contrast",
                format!(
                    "loom theme contrast reported FAIL (exit {exit}) but no per-pair \
                     breakdown parsed — inspect output manually"
                ),
            )]);
        }
        Ok(parsed
            .iter()
            .map(ThemeContrastFinding::as_finding)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::Severity;

    const FAIL_OUTPUT: &str = "  theme           pair                          ratio   status
  --------------  ----------------------------  ------  ------
  default         ink-on-canvas                 21.00   ok
  hc-light        ink-on-canvas                 21.00   ok
  broken          primary-fg-on-primary          3.10   FAIL
  default         primary-fg-on-primary          9.10   ok
  worse           ink-muted-on-canvas            2.50   FAIL

loom theme contrast: 2 pair(s) below 4.5:1 — themes WILL ship with unreadable text
";

    const PASS_OUTPUT: &str = "  theme           pair                          ratio   status
  --------------  ----------------------------  ------  ------
  default         ink-on-canvas                 21.00   ok

loom theme contrast: 1 theme(s) checked, ALL pairs ≥ 4.5:1 (WCAG OK)
";

    #[test]
    fn parser_extracts_fail_lines() {
        let hits = parse_contrast_output(FAIL_OUTPUT);
        assert_eq!(hits.len(), 2);
        assert!(matches!(
            &hits[0],
            ThemeContrastFinding::PairBelowThreshold { theme, pair, ratio }
                if theme == "broken" && pair == "primary-fg-on-primary" && ratio == "3.10"
        ));
        assert!(matches!(
            &hits[1],
            ThemeContrastFinding::PairBelowThreshold { theme, .. }
                if theme == "worse"
        ));
    }

    #[test]
    fn parser_returns_empty_on_clean_output() {
        let hits = parse_contrast_output(PASS_OUTPUT);
        assert!(hits.is_empty());
    }

    #[test]
    fn parser_ignores_summary_text() {
        // Summary lines never end with literal "FAIL" — they end
        // with "unreadable text" or similar. Ensure the parser
        // doesn't false-positive on those.
        let hits = parse_contrast_output(FAIL_OUTPUT);
        assert_eq!(hits.len(), 2, "summary line must not produce a finding");
    }

    #[test]
    fn pair_below_threshold_finding_is_strict() {
        let f = ThemeContrastFinding::PairBelowThreshold {
            theme: "broken".into(),
            pair: "ink-on-canvas".into(),
            ratio: "2.10".into(),
        }
        .as_finding();
        assert_eq!(f.severity, Severity::Strict);
        assert!(f.message.contains("broken"));
        assert!(f.message.contains("2.10"));
    }

    #[test]
    fn loom_missing_finding_is_warn() {
        let f = ThemeContrastFinding::LoomMissing {
            searched: vec![PathBuf::from("/nope")],
        }
        .as_finding();
        assert_eq!(f.severity, Severity::Warn);
    }

    #[test]
    fn skin_missing_finding_is_warn() {
        let f = ThemeContrastFinding::SkinMissing {
            searched: vec![PathBuf::from("/static/loom.css")],
        }
        .as_finding();
        assert_eq!(f.severity, Severity::Warn);
    }

    #[test]
    fn loom_errored_finding_is_warn() {
        let f = ThemeContrastFinding::LoomErrored {
            exit_code: 2,
            stderr_excerpt: "boom".into(),
        }
        .as_finding();
        assert_eq!(f.severity, Severity::Warn);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Output parser must not panic on arbitrary input.
        #[test]
        fn parser_does_not_panic(input in ".{0,2000}") {
            let _ = parse_contrast_output(&input);
        }

        /// Any line ending with FAIL that has at least 4 cols
        /// produces exactly one finding.
        #[test]
        fn fail_lines_produce_findings(
            theme in "[a-z][a-z0-9-]{0,12}",
            pair in "[a-z][a-z0-9-]{0,28}",
            ratio in "[0-9]{1,2}\\.[0-9]{2}",
        ) {
            let line = format!("  {theme}    {pair}    {ratio}    FAIL\n");
            let hits = parse_contrast_output(&line);
            prop_assert_eq!(hits.len(), 1);
        }
    }
}
