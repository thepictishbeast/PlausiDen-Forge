//! `perf_budget` — per-file size budgets for HTML / CSS / JS.
//!
//! Bash parity: `phase_perf_budget` in forge.sh.
//!
//! Defaults (bytes):
//!   HTML  20 480  (20 KB) per page
//!   CSS   65 536  (64 KB) per stylesheet
//!   JS     8 192  ( 8 KB) per script
//!
//! Severity: `Warn` in PoC, `Strict` in production. The CLI flips
//! that via `BuildMode`; this phase always emits `Warn` and lets
//! `Severity::blocks_in` upgrade in production mode.
//!
//! The total static payload is logged for trend analysis.

use std::fs;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// Per-asset-class scan parameters.
struct AssetClass {
    ext: &'static str,
    budget: u64,
    label: &'static str,
    hint: &'static str,
}

const CLASSES: &[AssetClass] = &[
    AssetClass {
        ext: "html",
        budget: 20 * 1024,
        label: "HTML",
        hint: "audit blocks / split route",
    },
    AssetClass {
        ext: "css",
        budget: 64 * 1024,
        label: "CSS",
        hint: "split into per-route bundles",
    },
    AssetClass {
        ext: "js",
        budget: 8 * 1024,
        label: "JS",
        hint: "code-split or tree-shake",
    },
];

/// `perf_budget` phase.
#[derive(Debug, Default)]
pub struct PerfBudgetPhase;

impl Phase for PerfBudgetPhase {
    fn name(&self) -> &'static str {
        "perf_budget"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let mut total: u64 = 0;
        for class in CLASSES {
            check_class(
                &ctx.static_dir,
                class,
                &mut findings,
                &mut total,
                self.name(),
            )?;
        }
        // Project-wide info (not a finding).
        // BUG ASSUMPTION: total here only includes top-level
        // static/*.html|css|js — not nested directories. Forge
        // doesn't yet generate nested assets; when it does, walk
        // recursively here.
        tracing::info!(target: "forge", "perf_budget total: {} bytes", total);
        Ok(findings)
    }
}

/// Walk one extension class and emit findings for over-budget files.
fn check_class(
    dir: &Path,
    class: &AssetClass,
    findings: &mut Vec<Finding>,
    total: &mut u64,
    phase: &str,
) -> Result<(), BuildError> {
    let AssetClass {
        ext,
        budget,
        label,
        hint,
    } = *class;
    let entries = match fs::read_dir(dir) {
        Ok(it) => it,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(BuildError::Io {
                context: format!("{phase}: read_dir {}", dir.display()),
                source: e,
            });
        }
    };
    for entry in entries {
        let entry = entry.map_err(|e| BuildError::Io {
            context: format!("{phase}: dir entry under {}", dir.display()),
            source: e,
        })?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some(ext) {
            continue;
        }
        let meta = entry.metadata().map_err(|e| BuildError::Io {
            context: format!("{phase}: stat {}", path.display()),
            source: e,
        })?;
        let sz = meta.len();
        *total += sz;
        if sz > budget {
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_owned();
            findings.push(
                Finding::warn(
                    phase,
                    name,
                    format!("{} {} > {} budget — {hint}", iec(sz), label, iec(budget)),
                )
                .why(
                    "every kilobyte over budget is one more TLS round-trip the reader's \
                     browser has to wait for before paint — the budget is the reader-side \
                     latency floor, not an internal lint",
                )
                .fix(format!(
                    "{label} over budget at {}: {hint}",
                    iec(sz)
                ))
                .avoid(
                    "don't raise the budget to silence the warn — every active substrate \
                     uses the same defaults so over-budget assets are a real regression, not \
                     a config mismatch",
                ),
            );
        }
    }
    Ok(())
}

/// Format bytes as IEC (KiB, MiB, etc.) — matches numfmt --to=iec.
fn iec(n: u64) -> String {
    const UNITS: &[&str] = &["B", "K", "M", "G"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{}{}", n, UNITS[0])
    } else if value >= 100.0 {
        format!("{value:.0}{}", UNITS[unit])
    } else if value >= 10.0 {
        format!("{value:.1}{}", UNITS[unit])
    } else {
        format!("{value:.2}{}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perf_budget_findings_carry_advocacy() {
        use std::fs;
        let tmp = std::env::temp_dir().join(format!(
            "forge-perf-budget-advocacy-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("static")).unwrap();
        // Write a JS file 20K — well above the 8K JS budget.
        let big = vec![b'a'; 20 * 1024];
        fs::write(tmp.join("static/big.js"), &big).unwrap();
        let ctx = BuildCtx {
            root: tmp.clone(),
            static_dir: tmp.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = PerfBudgetPhase.run(&ctx).expect("run");
        assert_eq!(findings.len(), 1, "expected one over-budget finding");
        let adv = &findings[0].advocacy;
        assert!(!adv.why.is_empty(), "must carry .why()");
        assert!(!adv.substrate_fix.is_empty(), "must carry .fix()");
        assert!(adv.anti_pattern.is_some(), "must carry .avoid()");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn iec_formats() {
        assert_eq!(iec(0), "0B");
        assert_eq!(iec(1023), "1023B");
        assert_eq!(iec(1024), "1.00K");
        assert_eq!(iec(20_480), "20.0K");
        assert_eq!(iec(65_536), "64.0K");
        assert_eq!(iec(1024 * 1024), "1.00M");
    }
}
