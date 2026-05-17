//! `crawl` — invoke the PlausiDen-Crawler against the freshly
//! built static/ tree and surface its findings as forge findings.
//!
//! Rust port of bash `phase_crawl` (T49). One forge run = build
//! + runtime audit, single report.
//!
//! ## Doctrine applied (per supersociety stack)
//!
//! * **Composition** — `CrawlPhase` is a ZST. Phase trait impl.
//! * **Typed JSON parse** — instead of stdout-scraping the
//!   crawler's positive-signal text (fragile, breaks on every
//!   format tweak), the phase reads the latest run directory's
//!   `diff.json` and parses with serde. The crawler's exit code
//!   is the secondary signal.
//! * **ADT findings** — `enum CrawlFinding` exhaustively covers
//!   every result class. Adding a new axis = adding a variant.
//! * **Value objects** — `AxisName(String)` validated against a
//!   curated set of known axis identifiers; unknown names route
//!   to `UnknownAxis` rather than corrupting downstream parsing.
//! * **Capability discovery** — crawler dir resolved by env var
//!   first, then a curated list of conventional sibling paths.
//!   Each candidate is verified before use; a missing dir
//!   degrades to a `Warn` (not `Strict`) so devs without the
//!   sibling repo aren't blocked from building.
//! * **Liveness probe** — dev server probed via raw TCP connect
//!   to 127.0.0.1:8123 before invoking the crawler. Probing
//!   prevents the cascade-of-fake-regressions failure mode that
//!   bit us on 2026-05-05 when /tmp wiped the dev server.
//! * **Deny `unsafe_code`**, no `unwrap`/`expect` in lib code.
//! * **Property-based tests** alongside deterministic ones.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde::Deserialize;

// ============================================================
// Value Objects
// ============================================================

/// Validated crawler axis identifier. Constrained to the
/// 16 known axes (T16 timeframe: console-errors, page-errors,
/// failed-requests, axe-static-a11y, cssHealth, uiOverflow,
/// runtimeContrast, runtimeImages, runtimeFocus, webVitals,
/// cspViolations, ariaDrift, headingOrder, runtimeLandmarks,
/// linkText, placeholderText). Unknown identifiers route to
/// `Unknown` instead of being silently accepted.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AxisName {
    /// One of the 16 known axes — string form preserved for
    /// rendering, but the enum-tagged path proves it was on
    /// the allowlist.
    Known(String),
    /// Anything the crawler may emit that we don't recognise yet.
    Unknown(String),
}

impl AxisName {
    /// Construct from a raw &str. Always succeeds; returns
    /// `Unknown` for axes not on the allowlist.
    #[must_use]
    pub fn classify(s: &str) -> Self {
        // BUG ASSUMPTION: the allowlist drifts as the crawler
        // gains axes. T16 baseline is 16 axes; T58+ may add
        // more. Keep this list synced with PlausiDen-Crawler/
        // src/report.ts CapturedEvent.kind union.
        const KNOWN: &[&str] = &[
            "console-errors",
            "page-errors",
            "failed-requests",
            "axe-static-a11y",
            "cssHealth",
            "uiOverflow",
            "runtimeContrast",
            "runtimeImages",
            "runtimeFocus",
            "webVitals",
            "cspViolations",
            "ariaDrift",
            "headingOrder",
            "runtimeLandmarks",
            "linkText",
            "placeholderText",
        ];
        if KNOWN.contains(&s) {
            Self::Known(s.to_owned())
        } else {
            Self::Unknown(s.to_owned())
        }
    }

    /// Borrow as `&str` for printing.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Known(s) | Self::Unknown(s) => s,
        }
    }
}

// ============================================================
// ADT findings
// ============================================================

/// What the crawl phase concluded. Each variant maps to exactly
/// one `forge_core::Finding` severity at render time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrawlFinding {
    /// Crawler dir not found at any known location. WARN —
    /// devs without the sibling repo shouldn't be blocked.
    CrawlerMissing { searched: Vec<PathBuf> },
    /// Journey JSON file missing. WARN.
    JourneyMissing { path: PathBuf },
    /// Dev server at 127.0.0.1:8123 didn't respond. WARN —
    /// missing server isn't a site regression, it's an operator
    /// concern.
    DevServerDown,
    /// Crawler exited non-zero with no parseable axis breakdown.
    /// STRICT — build cannot trust the runtime state.
    OpaqueFailure { exit_code: i32 },
    /// Crawler errored (exit ≥ 2). WARN — runtime audit could
    /// not complete.
    CrawlerErrored {
        exit_code: i32,
        stderr_excerpt: String,
    },
    /// One specific axis regressed. STRICT.
    AxisRegression { axis: AxisName, new_strict: u32 },
}

impl CrawlFinding {
    /// Render to a typed forge finding. Severity is fixed per
    /// variant; consumers can't accidentally upgrade or downgrade.
    pub fn as_finding(&self) -> Finding {
        const PHASE: &str = "crawl";
        match self {
            Self::CrawlerMissing { searched } => {
                let paths = searched
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                Finding::warn(
                    PHASE,
                    "PlausiDen-Crawler",
                    format!(
                        "crawler dir not at any known path ({paths}) — \
                         runtime audit skipped (set CRAWLER_DIR env to override)"
                    ),
                )
            }
            Self::JourneyMissing { path } => Finding::warn(
                PHASE,
                path.display().to_string(),
                format!(
                    "journey file not at {} — runtime audit skipped",
                    path.display()
                ),
            ),
            Self::DevServerDown => Finding::warn(
                PHASE,
                "127.0.0.1:8123",
                "dev server not responding — start it (e.g. `python3 -m http.server 8123 \
                 --directory static`) and re-run; runtime audit skipped",
            ),
            Self::OpaqueFailure { exit_code } => Finding::strict(
                PHASE,
                "runtime",
                format!(
                    "crawler reported FAIL (exit {exit_code}) but no per-axis breakdown \
                     parsed — inspect PlausiDen-Crawler/runs/ for details"
                ),
            ),
            Self::CrawlerErrored {
                exit_code,
                stderr_excerpt,
            } => Finding::warn(
                PHASE,
                "PlausiDen-Crawler",
                format!(
                    "crawler errored (exit {exit_code}) — runtime audit could not \
                     complete: {stderr_excerpt}"
                ),
            ),
            Self::AxisRegression { axis, new_strict } => Finding::strict(
                PHASE,
                axis.as_str(),
                format!(
                    "runtime regression on {} (+{new_strict} new strict)",
                    axis.as_str()
                ),
            ),
        }
    }
}

// ============================================================
// Diff parsing — typed serde deserialize, not text scraping.
// ============================================================

/// Subset of crawler `diff.json` we actually consume. Other
/// fields are ignored via serde's default flatten behaviour.
#[derive(Debug, Deserialize)]
struct CrawlDiff {
    #[serde(default)]
    #[serde(rename = "newConsoleErrors")]
    new_console_errors: Vec<DiffEvent>,
    #[serde(default)]
    #[serde(rename = "newPageErrors")]
    new_page_errors: Vec<DiffEvent>,
    #[serde(default)]
    #[serde(rename = "newFailedRequests")]
    new_failed_requests: Vec<DiffEvent>,
    #[serde(default)]
    #[serde(rename = "newA11yViolations")]
    new_a11y_violations: Vec<DiffEvent>,
    #[serde(default)]
    #[serde(rename = "newCssHealthFindings")]
    new_css_health: Vec<DiffEvent>,
    #[serde(default)]
    #[serde(rename = "newUiOverflowFindings")]
    new_ui_overflow: Vec<DiffEvent>,
    #[serde(default)]
    #[serde(rename = "newRuntimeContrastFindings")]
    new_runtime_contrast: Vec<DiffEvent>,
    #[serde(default)]
    #[serde(rename = "newRuntimeImagesFindings")]
    new_runtime_images: Vec<DiffEvent>,
    #[serde(default)]
    #[serde(rename = "newRuntimeFocusFindings")]
    new_runtime_focus: Vec<DiffEvent>,
    #[serde(default)]
    #[serde(rename = "newWebVitalsFindings")]
    new_web_vitals: Vec<DiffEvent>,
    #[serde(default)]
    #[serde(rename = "newCspViolations")]
    new_csp_violations: Vec<DiffEvent>,
    #[serde(default)]
    #[serde(rename = "newAriaDriftFindings")]
    new_aria_drift: Vec<DiffEvent>,
    #[serde(default)]
    #[serde(rename = "newHeadingOrderFindings")]
    new_heading_order: Vec<DiffEvent>,
    #[serde(default)]
    #[serde(rename = "newRuntimeLandmarksFindings")]
    new_runtime_landmarks: Vec<DiffEvent>,
    #[serde(default)]
    #[serde(rename = "newLinkTextFindings")]
    new_link_text: Vec<DiffEvent>,
    #[serde(default)]
    #[serde(rename = "newPlaceholderTextFindings")]
    new_placeholder_text: Vec<DiffEvent>,
}

#[derive(Debug, Deserialize)]
struct DiffEvent {
    #[serde(default)]
    severity: Option<String>,
}

impl DiffEvent {
    fn is_strict(&self) -> bool {
        // Some axes (console-errors, page-errors, etc.) don't
        // carry a severity field — every event is implicitly
        // strict. Only css-health / ui-overflow / runtime-* /
        // web-vitals carry an explicit severity.
        match &self.severity {
            None => true,
            Some(s) => s == "strict",
        }
    }
}

/// Walk the crawler's diff and emit one `AxisRegression` per
/// axis with at least one strict event.
fn diff_to_findings(diff: &CrawlDiff) -> Vec<CrawlFinding> {
    let pairs: &[(&str, &Vec<DiffEvent>)] = &[
        ("console-errors", &diff.new_console_errors),
        ("page-errors", &diff.new_page_errors),
        ("failed-requests", &diff.new_failed_requests),
        ("axe-static-a11y", &diff.new_a11y_violations),
        ("cssHealth", &diff.new_css_health),
        ("uiOverflow", &diff.new_ui_overflow),
        ("runtimeContrast", &diff.new_runtime_contrast),
        ("runtimeImages", &diff.new_runtime_images),
        ("runtimeFocus", &diff.new_runtime_focus),
        ("webVitals", &diff.new_web_vitals),
        ("cspViolations", &diff.new_csp_violations),
        ("ariaDrift", &diff.new_aria_drift),
        ("headingOrder", &diff.new_heading_order),
        ("runtimeLandmarks", &diff.new_runtime_landmarks),
        ("linkText", &diff.new_link_text),
        ("placeholderText", &diff.new_placeholder_text),
    ];
    let mut out = Vec::new();
    for (name, events) in pairs {
        let strict_count =
            u32::try_from(events.iter().filter(|e| e.is_strict()).count()).unwrap_or(u32::MAX);
        if strict_count > 0 {
            out.push(CrawlFinding::AxisRegression {
                axis: AxisName::classify(name),
                new_strict: strict_count,
            });
        }
    }
    out
}

// ============================================================
// Capability discovery — crawler dir + journey file
// ============================================================

fn resolve_crawler_dir() -> Result<PathBuf, Vec<PathBuf>> {
    if let Ok(env) = std::env::var("CRAWLER_DIR") {
        let p = PathBuf::from(&env);
        if p.is_dir() {
            return Ok(p);
        }
        return Err(vec![p]);
    }
    let candidates = [
        PathBuf::from("/home/user/Development/PlausiDen/PlausiDen-Crawler"),
        PathBuf::from("../PlausiDen-Crawler"),
        PathBuf::from("../../PlausiDen-Crawler"),
    ];
    for c in &candidates {
        if c.is_dir() {
            return Ok(c.clone());
        }
    }
    Err(candidates.to_vec())
}

fn resolve_journey_path(crawler_dir: &Path) -> PathBuf {
    let rel = std::env::var("CRAWLER_JOURNEY")
        .unwrap_or_else(|_| "journeys/skillshots-poc.json".to_owned());
    crawler_dir.join(rel)
}

/// TCP-probe 127.0.0.1:8123 with a 2-second connect timeout.
/// SECURITY: connect-only, no data sent — minimum surface.
fn dev_server_alive() -> bool {
    let addr = "127.0.0.1:8123";
    let socket_addrs = match addr.parse::<std::net::SocketAddr>() {
        Ok(a) => vec![a],
        Err(_) => return false,
    };
    for sa in socket_addrs {
        if std::net::TcpStream::connect_timeout(&sa, Duration::from_secs(2)).is_ok() {
            return true;
        }
    }
    false
}

// ============================================================
// Phase impl
// ============================================================

/// `crawl` phase. ZST — composition over inheritance.
#[derive(Debug, Default)]
pub struct CrawlPhase;

impl Phase for CrawlPhase {
    fn name(&self) -> &'static str {
        "crawl"
    }

    fn run(&self, _ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        tracing::debug!("crawl: enter");

        // 1. Locate crawler.
        let crawler_dir = match resolve_crawler_dir() {
            Ok(d) => d,
            Err(searched) => {
                tracing::warn!(?searched, "crawl: crawler dir not found");
                return Ok(vec![CrawlFinding::CrawlerMissing { searched }.as_finding()]);
            }
        };

        // 2. Locate journey.
        let journey = resolve_journey_path(&crawler_dir);
        if !journey.is_file() {
            tracing::warn!(path = ?journey, "crawl: journey not found");
            return Ok(vec![
                CrawlFinding::JourneyMissing { path: journey }.as_finding()
            ]);
        }

        // 3. Probe dev server.
        if !dev_server_alive() {
            tracing::warn!("crawl: dev server 127.0.0.1:8123 unreachable");
            return Ok(vec![CrawlFinding::DevServerDown.as_finding()]);
        }

        // 4. Run the crawler.
        // SECURITY: command + args are static; the only operator-
        // controlled values (CRAWLER_DIR, CRAWLER_JOURNEY) flow
        // through the resolved PathBuf so shell metachars never
        // touch a shell. We invoke `npm` directly, no shell.
        tracing::info!(?crawler_dir, ?journey, "crawl: invoking crawler");
        let journey_arg = journey
            .strip_prefix(&crawler_dir)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| journey.clone());
        let output = Command::new("npm")
            .args(["run", "audit", "--", "--journey"])
            .arg(&journey_arg)
            .current_dir(&crawler_dir)
            .output()
            .map_err(|source| BuildError::Io {
                context: format!("npm run audit in {}", crawler_dir.display()),
                source,
            })?;

        let exit_code = output.status.code().unwrap_or(-1);
        tracing::debug!(exit_code, "crawl: crawler exited");

        // Crawler exit ≥2 = errored, not regressed.
        if exit_code >= 2 {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let excerpt = stderr.chars().take(200).collect::<String>();
            return Ok(vec![CrawlFinding::CrawlerErrored {
                exit_code,
                stderr_excerpt: excerpt,
            }
            .as_finding()]);
        }

        // Crawler exit 0 = no regressions; succeed silently.
        if exit_code == 0 {
            return Ok(vec![]);
        }

        // Crawler exit 1 = regressions. Parse the latest run's
        // diff.json for typed per-axis counts.
        let runs_dir = crawler_dir.join("runs");
        let latest = newest_run_dir(&runs_dir);
        let Some(latest) = latest else {
            return Ok(vec![CrawlFinding::OpaqueFailure { exit_code }.as_finding()]);
        };
        let diff_path = latest.join("diff.json");
        let Ok(raw) = std::fs::read_to_string(&diff_path) else {
            return Ok(vec![CrawlFinding::OpaqueFailure { exit_code }.as_finding()]);
        };
        let Ok(diff) = serde_json::from_str::<CrawlDiff>(&raw) else {
            tracing::warn!(path = ?diff_path, "crawl: diff.json parse failed");
            return Ok(vec![CrawlFinding::OpaqueFailure { exit_code }.as_finding()]);
        };

        let parsed = diff_to_findings(&diff);
        let findings: Vec<Finding> = parsed.iter().map(CrawlFinding::as_finding).collect();
        if findings.is_empty() {
            // Crawler said FAIL but diff parsed clean — surface
            // an opaque failure rather than swallow.
            return Ok(vec![CrawlFinding::OpaqueFailure { exit_code }.as_finding()]);
        }
        Ok(findings)
    }
}

/// Find the most-recently-modified subdirectory of `runs/`.
fn newest_run_dir(runs_dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(runs_dir).ok()?;
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let mtime = e.metadata().ok().and_then(|m| m.modified().ok())?;
        match &best {
            None => best = Some((mtime, p)),
            Some((bt, _)) if mtime > *bt => best = Some((mtime, p)),
            _ => {}
        }
    }
    best.map(|(_, p)| p)
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::Severity;

    #[test]
    fn axis_name_classifies_known_axes() {
        for known in [
            "console-errors",
            "axe-static-a11y",
            "cssHealth",
            "placeholderText",
        ] {
            assert!(matches!(AxisName::classify(known), AxisName::Known(_)));
        }
    }

    #[test]
    fn axis_name_routes_unknown_to_unknown_variant() {
        assert!(matches!(
            AxisName::classify("madeUpAxis"),
            AxisName::Unknown(_)
        ));
        // Empty string and shell metachars all classified, never panic.
        assert!(matches!(AxisName::classify(""), AxisName::Unknown(_)));
        assert!(matches!(
            AxisName::classify("a;rm -rf /"),
            AxisName::Unknown(_)
        ));
    }

    #[test]
    fn dev_server_down_finding_is_warn() {
        let f = CrawlFinding::DevServerDown.as_finding();
        assert_eq!(f.severity, Severity::Warn);
    }

    #[test]
    fn axis_regression_finding_is_strict() {
        let f = CrawlFinding::AxisRegression {
            axis: AxisName::classify("placeholderText"),
            new_strict: 1,
        }
        .as_finding();
        assert_eq!(f.severity, Severity::Strict);
        assert!(f.message.contains("placeholderText"));
        assert!(f.message.contains("+1"));
    }

    #[test]
    fn crawler_errored_finding_is_warn() {
        let f = CrawlFinding::CrawlerErrored {
            exit_code: 2,
            stderr_excerpt: "boom".into(),
        }
        .as_finding();
        assert_eq!(f.severity, Severity::Warn);
    }

    #[test]
    fn opaque_failure_finding_is_strict() {
        let f = CrawlFinding::OpaqueFailure { exit_code: 1 }.as_finding();
        assert_eq!(f.severity, Severity::Strict);
    }

    #[test]
    fn empty_diff_yields_no_findings() {
        let diff: CrawlDiff = serde_json::from_str("{}").expect("empty diff");
        let findings = diff_to_findings(&diff);
        assert!(findings.is_empty());
    }

    #[test]
    fn diff_with_strict_console_error_yields_axis_regression() {
        let raw = r#"{
            "newConsoleErrors": [{"text":"oops"}]
        }"#;
        let diff: CrawlDiff = serde_json::from_str(raw).expect("parse");
        let findings = diff_to_findings(&diff);
        assert_eq!(findings.len(), 1);
        assert!(matches!(
            &findings[0],
            CrawlFinding::AxisRegression { axis, new_strict: 1 }
                if axis.as_str() == "console-errors"
        ));
    }

    #[test]
    fn diff_with_warn_severity_does_not_emit_axis_regression() {
        let raw = r#"{
            "newCssHealthFindings": [{"severity":"warn"}]
        }"#;
        let diff: CrawlDiff = serde_json::from_str(raw).expect("parse");
        let findings = diff_to_findings(&diff);
        assert!(
            findings.is_empty(),
            "warn-severity must not regress: {findings:?}"
        );
    }

    #[test]
    fn diff_with_mixed_severity_counts_only_strict() {
        let raw = r#"{
            "newCssHealthFindings": [
                {"severity":"strict"},
                {"severity":"warn"},
                {"severity":"strict"}
            ]
        }"#;
        let diff: CrawlDiff = serde_json::from_str(raw).expect("parse");
        let findings = diff_to_findings(&diff);
        assert_eq!(findings.len(), 1);
        assert!(matches!(
            &findings[0],
            CrawlFinding::AxisRegression { new_strict: 2, .. }
        ));
    }

    #[test]
    fn unknown_diff_fields_do_not_crash() {
        // Crawler may add new event-bucket fields; serde must
        // tolerate them via #[serde(default)] on every field we
        // care about + ignore the rest.
        let raw = r#"{
            "newConsoleErrors": [],
            "newSomeFutureField": [{"weird":true}],
            "extraField": 42
        }"#;
        let diff: CrawlDiff = serde_json::from_str(raw).expect("parse");
        let findings = diff_to_findings(&diff);
        assert!(findings.is_empty());
    }
}

// ============================================================
// Property-based tests (AVP-2 Tier 6)
// ============================================================

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// AxisName classification must not panic on arbitrary
        /// bytes — operator-controlled axis names from a future
        /// crawler version must always classify into Known or
        /// Unknown.
        #[test]
        fn axis_classify_never_panics(s in ".{0,200}") {
            let _ = AxisName::classify(&s);
        }

        /// The `as_finding` renderer must not panic on any
        /// CrawlFinding instance reachable from the constructor
        /// surface.
        #[test]
        fn finding_render_never_panics(
            axis in "[a-zA-Z][a-zA-Z0-9-]{0,40}",
            new_strict in 0u32..=10_000u32,
        ) {
            let f = CrawlFinding::AxisRegression {
                axis: AxisName::classify(&axis),
                new_strict,
            };
            let _ = f.as_finding();
        }

        /// Diff parser must not panic on arbitrary JSON-shaped
        /// strings. A panic here = a malformed crawler report
        /// crashes forge instead of reporting a clean error.
        #[test]
        fn diff_parser_does_not_panic_on_arbitrary_input(input in "\\{.{0,2000}\\}") {
            let _ = serde_json::from_str::<CrawlDiff>(&input);
        }
    }
}
