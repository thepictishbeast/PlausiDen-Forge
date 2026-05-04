//! Forge CLI — drives the phase pipeline.
//!
//! v1: hard-coded phase list; reads `forge.toml` for mode; prints
//! a human-readable terminal report. Future commits add `--watch`,
//! `--debug`, JSON report, parallel phase execution, and the rest
//! of the 22 phases ported from forge.sh.
//!
//! AVP-2 invariants enforced:
//!
//! * `unwrap`/`expect` only on infallible-by-construction paths.
//!   All real I/O goes through `?` + `BuildError`.
//! * The exit code is the gate: 0 if `BuildReport::passed(mode)`,
//!   non-zero otherwise. CI wires straight in.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;
use forge_core::{BuildCtx, BuildMode, BuildReport, Finding, Phase, Severity};
use forge_phases::csp::CspPhase;
use forge_phases::html_semantic::HtmlSemanticPhase;
use forge_phases::loom_sync::LoomSyncPhase;
use forge_phases::tokens::TokensPhase;

#[derive(Parser, Debug)]
#[command(name = "forge", version, about = "PlausiDen-Forge — typed, audited build pipeline.")]
struct Args {
    /// Project root. Defaults to CWD.
    #[arg(long, env = "FORGE_ROOT")]
    root: Option<PathBuf>,

    /// Build mode. Overrides `forge.toml`.
    #[arg(long, value_enum)]
    mode: Option<ModeArg>,

    /// Emit JSON report to this path in addition to terminal.
    #[arg(long)]
    json_report: Option<PathBuf>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum ModeArg {
    Poc,
    Production,
    Static,
    Hybrid,
    Dynamic,
}

impl From<ModeArg> for BuildMode {
    fn from(m: ModeArg) -> Self {
        match m {
            ModeArg::Poc => Self::Poc,
            ModeArg::Production => Self::Production,
            ModeArg::Static => Self::Static,
            ModeArg::Hybrid => Self::Hybrid,
            ModeArg::Dynamic => Self::Dynamic,
        }
    }
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .compact()
        .init();

    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("forge: fatal: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode> {
    let args = Args::parse();

    let root = match args.root {
        Some(p) => p,
        None => std::env::current_dir().context("forge needs a project root and CWD is unreadable")?,
    };
    let static_dir = root.join("static");

    let mode = if let Some(m) = args.mode {
        BuildMode::from(m)
    } else {
        read_mode_from_toml(&root).unwrap_or(BuildMode::Poc)
    };

    let ctx = BuildCtx {
        root: root.clone(),
        static_dir,
        mode,
    };

    // BUG ASSUMPTION: phase order matters. `loom_sync` runs
    // first because downstream phases (tokens, etc.) read
    // skin.css and benefit from knowing it's drift-checked.
    // Tokens / html_semantic / csp are independent — they each
    // walk static/*.html in parallel-safe way (no shared mutable
    // state). Order between them is alphabetic for now;
    // forge-runner crate (queued) will introduce explicit
    // dependency edges.
    let phases: Vec<Box<dyn Phase>> = vec![
        Box::new(LoomSyncPhase),
        Box::new(TokensPhase),
        Box::new(HtmlSemanticPhase),
        Box::new(CspPhase),
    ];

    let mut report = BuildReport {
        mode: format!("{mode:?}").to_lowercase(),
        ..Default::default()
    };
    let started = std::time::Instant::now();

    println!("forge {} mode={}", env!("CARGO_PKG_VERSION"), report.mode);
    for phase in &phases {
        let phase_started = std::time::Instant::now();
        println!("\n== phase: {} ==", phase.name());
        let findings = phase
            .run(&ctx)
            .with_context(|| format!("phase {}", phase.name()))?;
        let elapsed = phase_started.elapsed().as_millis();
        if findings.is_empty() {
            println!("  ok      no findings ({elapsed}ms)");
        } else {
            for f in &findings {
                print_finding(f);
                report.push(f.clone());
            }
        }
    }
    report.duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    println!("\n== summary ==");
    println!("  mode:                {}", report.mode);
    println!("  strict findings:     {}", report.strict_count);
    println!("  suppressible warns:  {}", report.warn_count);
    println!("  duration:            {}ms", report.duration_ms);

    if let Some(p) = args.json_report {
        std::fs::write(
            &p,
            serde_json::to_string_pretty(&report).context("serialize report")?,
        )
        .with_context(|| format!("writing JSON report to {}", p.display()))?;
        println!("  json report:         {}", p.display());
    }

    if report.passed(mode) {
        println!("\nforge build OK");
        Ok(ExitCode::SUCCESS)
    } else {
        println!("\nforge build FAILED — see findings above");
        Ok(ExitCode::from(1))
    }
}

fn print_finding(f: &Finding) {
    // BUG ASSUMPTION: `Severity` is `#[non_exhaustive]`; the `_`
    // arm catches future variants like `Fatal`. Treating an
    // unknown variant as strict is the safe-default choice — we'd
    // rather fail-loud on an unrecognized severity than silently
    // pass it through.
    let label = match f.severity {
        Severity::Strict => "STRICT  ",
        Severity::Warn => "warn    ",
        _ => "STRICT  ",
    };
    if f.path.is_empty() {
        println!("  {}{}: {}", label, f.phase, f.message);
    } else {
        println!("  {}{}: {} — {}", label, f.phase, f.path, f.message);
    }
}

/// Best-effort read of `mode = "..."` from `forge.toml`.
fn read_mode_from_toml(root: &std::path::Path) -> Option<BuildMode> {
    let path = root.join("forge.toml");
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("mode") {
            let rest = rest.trim_start_matches(|c: char| c == '=' || c.is_whitespace());
            let val = rest.trim_matches('"').trim_matches('\'').trim();
            return match val {
                "poc" => Some(BuildMode::Poc),
                "production" => Some(BuildMode::Production),
                "static" => Some(BuildMode::Static),
                "hybrid" => Some(BuildMode::Hybrid),
                "dynamic" => Some(BuildMode::Dynamic),
                _ => None,
            };
        }
    }
    None
}
