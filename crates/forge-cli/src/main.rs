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
use forge_phases::a11y_landmarks::A11yLandmarksPhase;
use forge_phases::asset_optimization::AssetOptimizationPhase;
use forge_phases::backend_coverage::BackendCoveragePhase;
use forge_phases::contrast::ContrastPhase;
use forge_phases::crawl::CrawlPhase;
use forge_phases::csp::CspPhase;
use forge_phases::csp_devmode::CspDevmodePhase;
use forge_phases::external_assets::ExternalAssetsPhase;
use forge_phases::html_semantic::HtmlSemanticPhase;
use forge_phases::id_strategy::IdStrategyPhase;
use forge_phases::label_consistency::LabelConsistencyPhase;
use forge_phases::link_check::LinkCheckPhase;
use forge_phases::loom_sync::LoomSyncPhase;
use forge_phases::motion::MotionPhase;
use forge_phases::perf_budget::PerfBudgetPhase;
use forge_phases::phantom_button::PhantomButtonPhase;
use forge_phases::self_check::SelfCheckPhase;
use forge_phases::seo::SeoPhase;
use forge_phases::sri::SriPhase;
use forge_phases::theme_consistency::ThemeConsistencyPhase;
use forge_phases::tokens::TokensPhase;
use forge_phases::unbuilt_route::UnbuiltRoutePhase;
use forge_phases::validate_cms::ValidateCmsPhase;

#[derive(Parser, Debug)]
#[command(
    name = "forge",
    version,
    about = "PlausiDen-Forge — typed, audited build pipeline."
)]
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
        None => {
            std::env::current_dir().context("forge needs a project root and CWD is unreadable")?
        }
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
        // T53 (2026-05-06): validate_cms is the entry gate. No
        // downstream phase produces value if CMS input is
        // malformed; failing fast surfaces the bug at the most
        // actionable location.
        Box::new(ValidateCmsPhase),
        Box::new(LoomSyncPhase),
        Box::new(SelfCheckPhase),
        // T51 (2026-05-06): theme_consistency runs early — its
        // findings (e.g. an undefined --loom-color-* reference)
        // tell every downstream phase that depends on themed
        // values that the cascade is broken.
        Box::new(ThemeConsistencyPhase),
        Box::new(TokensPhase),
        Box::new(HtmlSemanticPhase),
        Box::new(CspPhase),
        Box::new(CspDevmodePhase),
        Box::new(ExternalAssetsPhase),
        Box::new(A11yLandmarksPhase),
        Box::new(IdStrategyPhase),
        Box::new(SeoPhase),
        Box::new(PerfBudgetPhase),
        Box::new(AssetOptimizationPhase),
        Box::new(SriPhase),
        Box::new(PhantomButtonPhase),
        Box::new(BackendCoveragePhase),
        Box::new(UnbuiltRoutePhase),
        Box::new(LabelConsistencyPhase),
        Box::new(LinkCheckPhase),
        Box::new(MotionPhase),
        Box::new(ContrastPhase),
        // T52 (2026-05-06): runtime audit runs LAST. Build-
        // infra issues surface earlier; runtime-only regressions
        // (placeholder text in DOM, ARIA drift, axe runtime) get
        // their own rung so the operator can tell them apart.
        Box::new(CrawlPhase),
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
    report.started = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| String::from("?"));

    // T26: Merkle-chain this report to its predecessor. Read the
    // newest existing reports/build-*.json; if found, hash it
    // and store the hash as prev_hash + bump chain_length.
    // Genesis runs (no prior report) yield prev_hash=None,
    // chain_length=1.
    let reports_dir = root.join("reports");
    let prior_report = load_newest_prior_report(&reports_dir);
    forge_core::attest::chain_step(prior_report.as_ref(), &mut report)
        .context("chain_step")?;

    println!("\n== summary ==");
    println!("  mode:                {}", report.mode);
    println!("  strict findings:     {}", report.strict_count);
    println!("  suppressible warns:  {}", report.warn_count);
    println!("  duration:            {}ms", report.duration_ms);
    println!("  chain length:        {}", report.chain_length);
    if let Some(h) = &report.prev_hash {
        println!("  prev hash:           {}…{}", &h[..8], &h[h.len().saturating_sub(8)..]);
    } else {
        println!("  prev hash:           (genesis)");
    }

    // T26: write the chained report to reports/build-<ts>.json
    // AND update reports/latest.json. If reports/ doesn't exist,
    // create it.
    let _ = std::fs::create_dir_all(&reports_dir);
    let ts_compact = report.started.replace([':', '-'], "").replace('.', "");
    let build_path = reports_dir.join(format!("build-{ts_compact}.json"));
    let latest_path = reports_dir.join("latest.json");
    let serialized = serde_json::to_string_pretty(&report).context("serialize report")?;
    if std::fs::write(&build_path, &serialized).is_ok() {
        let _ = std::fs::write(&latest_path, &serialized);
        println!("  chain report:        {}", build_path.display());
    }

    if let Some(p) = args.json_report {
        std::fs::write(&p, &serialized)
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

/// T26: read the newest prior `reports/build-*.json` so we can
/// chain to it. Returns `None` if the dir is empty/missing OR
/// the newest file fails to parse — in either case we treat
/// the current run as genesis.
///
/// REGRESSION-GUARD: sort by *filename* (not mtime) because
/// filenames embed a monotonic timestamp; mtime can be wrong on
/// rsync'd hosts (preserves source mtime not arrival time).
fn load_newest_prior_report(reports_dir: &std::path::Path) -> Option<forge_core::BuildReport> {
    let entries = std::fs::read_dir(reports_dir).ok()?;
    let mut newest: Option<(std::ffi::OsString, std::path::PathBuf)> = None;
    for e in entries.flatten() {
        let name = e.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with("build-") || !name_str.ends_with(".json") {
            continue;
        }
        let path = e.path();
        match &newest {
            None => newest = Some((name, path)),
            Some((bn, _)) if &name > bn => newest = Some((name, path)),
            _ => {}
        }
    }
    let (_, path) = newest?;
    let raw = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
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
