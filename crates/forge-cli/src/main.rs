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
use clap::{Parser, Subcommand};
use forge_core::pipeline::{
    AuditedArtifacts, DiscoveredArtifacts, ParsedArtifacts, Pipeline, RenderedArtifacts,
    StageOutput,
};
use forge_core::{BuildCtx, BuildError, BuildMode, BuildReport, Finding, Phase, Severity};
use forge_phases::a11y_landmarks::A11yLandmarksPhase;
use forge_phases::annotation_review::AnnotationReviewPhase;
use forge_phases::asset_optimization::AssetOptimizationPhase;
use forge_phases::backend_coverage::BackendCoveragePhase;
use forge_phases::carbon_budget::CarbonBudgetPhase;
use forge_phases::contrast::ContrastPhase;
use forge_phases::crawl::CrawlPhase;
use forge_phases::csp::CspPhase;
use forge_phases::csp_devmode::CspDevmodePhase;
use forge_phases::dns_hygiene_lint::DnsHygieneLintPhase;
use forge_phases::dynamic_runtime::DynamicRuntimePhase;
use forge_phases::external_assets::ExternalAssetsPhase;
use forge_phases::html_semantic::HtmlSemanticPhase;
use forge_phases::id_strategy::IdStrategyPhase;
use forge_phases::jurisdiction_compliance::JurisdictionCompliancePhase;
use forge_phases::label_consistency::LabelConsistencyPhase;
use forge_phases::link_check::LinkCheckPhase;
use forge_phases::locale_html_lang::LocaleHtmlLangPhase;
use forge_phases::loom_sync::LoomSyncPhase;
use forge_phases::motion::MotionPhase;
use forge_phases::motion_respects_reduced::MotionRespectsReducedPhase;
use forge_phases::network_target_enforcement::NetworkTargetEnforcementPhase;
use forge_phases::path_consistency::PathConsistencyPhase;
use forge_phases::perf_budget::PerfBudgetPhase;
use forge_phases::phantom_button::PhantomButtonPhase;
use forge_phases::print_stylesheet::PrintStylesheetPhase;
use forge_phases::reader_safety::ReaderSafetyPhase;
use forge_phases::render::RenderPhase;
use forge_phases::required_pages::RequiredPagesPhase;
use forge_phases::self_check::SelfCheckPhase;
use forge_phases::seo::SeoPhase;
use forge_phases::sri::SriPhase;
use forge_phases::structured_data::StructuredDataPhase;
use forge_phases::theme_consistency::ThemeConsistencyPhase;
use forge_phases::theme_contrast::ThemeContrastPhase;
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
    /// Project root. Defaults to CWD. Applies to every subcommand.
    #[arg(long, env = "FORGE_ROOT", global = true)]
    root: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Cmd>,

    /// Build mode (only used by `build`, the default). Overrides
    /// `forge.toml`.
    #[arg(long, value_enum)]
    mode: Option<ModeArg>,

    /// Emit JSON report to this path in addition to terminal.
    /// Only used by `build`.
    #[arg(long)]
    json_report: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Run every phase. Default when no subcommand is given.
    Build,
    /// T44: continuous-build mode. Watches the project root for
    /// changes (cms/*, static/*, forge.toml, backends.toml) and
    /// re-runs the build pipeline on each save with debounce. Exit
    /// with Ctrl-C.
    ///
    /// Mom-class UX: `forge watch` in one terminal, `loom edit-serve`
    /// in another, edit cms/index.json — see findings stream as you
    /// save.
    ///
    /// Debounce: 300 ms (notify can fire many events for one
    /// editor save — vim ~swap files, atomic replaces, etc.).
    Watch {
        /// Debounce window in ms. Defaults to 300.
        #[arg(long, default_value_t = 300)]
        debounce_ms: u64,
        /// Limit to N rebuilds then exit (useful for tests + CI
        /// smoke). 0 means unlimited (the default).
        #[arg(long, default_value_t = 0)]
        max_rebuilds: usize,
    },
    /// Verify the cryptographic build-report chain (T26). Walks
    /// `reports/build-*.json` sorted by filename + asserts that
    /// every report's `prev_hash` matches the SHA-256 of its
    /// predecessor and `chain_length` is contiguous.
    ///
    /// Exit codes:
    ///   0 — clean chain (or empty reports/ — nothing to verify)
    ///   1 — chain divergence (tamper, gap, missing prev, etc.)
    ///   2 — fatal I/O or parse error
    Verify {
        /// Verify the Merkle chain.
        #[arg(long, default_value_t = true)]
        chain: bool,
        /// Also verify Ed25519 signatures on every chain root
        /// (T56). Requires reports/attest-pubkey.b64 to exist.
        #[arg(long, default_value_t = false)]
        signatures: bool,
    },
    /// T56: attestation key management.
    ///
    /// `forge attest init` generates a new Ed25519 keypair and
    /// writes:
    ///   reports/attest-key.b64       — private key (mode 0600)
    ///   reports/attest-pubkey.b64    — public key (world-readable)
    ///
    /// Subsequent `forge build` runs sign every chain root with
    /// the private key. `forge verify --signatures` checks them.
    Attest {
        #[command(subcommand)]
        action: AttestAction,
    },
    /// Adversarial audits run OUTSIDE the build pipeline. Used
    /// for one-off scans + pre-commit hooks; never gated on a
    /// build's success.
    Audit {
        #[command(subcommand)]
        action: AuditAction,
    },
    /// Auto-fix mechanical findings from the latest build report.
    ///
    /// Default mode is `--dry-run`: read the latest `reports/
    /// build-*.json`, identify findings whose fix is unambiguous
    /// (per the in-process fixer registry), print the proposed
    /// diff. Operator reviews + re-runs with `--apply` to write
    /// the changes.
    ///
    /// Current fixers (v1):
    ///   * `required_pages.security_txt` — create
    ///     `static/.well-known/security.txt` from a template.
    ///   * (more fixers planned as separate commits.)
    ///
    /// Exit codes:
    ///   0 — dry-run had no fixable findings, OR --apply succeeded
    ///   1 — dry-run found fixable findings (operator should review)
    ///   2 — fatal (no report found, parse error, etc.)
    Fix {
        /// Actually write the fixes. Default is dry-run (read +
        /// print only).
        #[arg(long, default_value_t = false)]
        apply: bool,
    },
    /// T33: manifest-keystone gate. Loads phases.toml + backends.toml
    /// from the project root, projects through manifest-core, and
    /// asserts internal consistency:
    ///   * every backend ID parses as kebab-case
    ///   * every phase ID parses as kebab-case
    ///   * phase depends_on is acyclic + resolvable (topo-sorts)
    ///   * phase implements references resolve to declared
    ///     capabilities (if a capabilities table is present in
    ///     phases.toml meta)
    ///
    /// Exit codes:
    ///   0 — manifest is internally consistent
    ///   1 — at least one gate violation
    ///   2 — fatal (missing required file, parse error)
    Manifest {
        #[command(subcommand)]
        action: ManifestAction,
    },
    /// T91 (first wiring): privacy-core gate. Loads `privacy.toml`
    /// at the project root, projects through `privacy-core`'s
    /// typed RetentionPolicy + DataCategory + LawfulBasis surface,
    /// and asserts:
    ///   * every DataCategory variant has a RetentionPolicy entry
    ///     (no silently-uncovered data class)
    ///   * no duplicate retention entries per category
    ///   * every retention_days > 0
    ///   * LegalObligation-basis entries are flagged as
    ///     refuses-erasure (informational)
    ///
    /// Exit codes:
    ///   0 — privacy.toml is internally consistent + complete
    ///   1 — at least one gate violation
    ///   2 — fatal (missing privacy.toml, parse error)
    Privacy {
        #[command(subcommand)]
        action: PrivacyAction,
    },
}

#[derive(Subcommand, Debug)]
enum AuditAction {
    /// T56b: scan paths for filenames that match dangerous
    /// patterns (private keys, certificates, dotenv, password
    /// stores). With no paths, reads `git diff --cached
    /// --name-only` so a pre-commit hook can `forge audit
    /// secrets` and refuse the commit on any match.
    ///
    /// Exit codes:
    ///   0 — no matches
    ///   1 — at least one secret-shaped path found
    ///   2 — fatal (git unavailable, etc.)
    Secrets {
        /// Explicit paths to scan. Defaults to git's staged
        /// changes when omitted.
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,
        /// Print the rule that matched alongside the path.
        #[arg(long, default_value_t = false)]
        explain: bool,
    },
    /// T58: mutation testing per AVP-2 Tier 6. Without
    /// `--run`, reads the most recent `mutants.out/outcomes.json`
    /// and reports survivor count vs threshold. With `--run`,
    /// invokes `cargo mutants` first (SLOW — re-runs the test
    /// suite per mutation; expect minutes-to-hours).
    ///
    /// Doctrine: a public function whose mutations all SURVIVE
    /// the test suite is a function the tests don't actually
    /// constrain. AVP-2 mandates < 5% survival on critical
    /// paths. forge core, forge-phases, attest, capability —
    /// all critical.
    ///
    /// Exit codes:
    ///   0 — survival rate ≤ threshold (or no outcomes yet)
    ///   1 — survival rate above threshold
    ///   2 — fatal (cargo-mutants missing, JSON unparseable)
    Mutants {
        /// Actually invoke `cargo mutants -p <crate>` first.
        /// Default false: just read existing mutants.out/.
        #[arg(long, default_value_t = false)]
        run: bool,
        /// Crate to test under `--run`. Defaults to forge-core.
        #[arg(long, default_value = "forge-core")]
        crate_name: String,
        /// Maximum acceptable survival rate as a percent.
        /// AVP-2 default is 5.0. Lower is stricter.
        #[arg(long, default_value_t = 5.0)]
        threshold: f64,
    },
    /// Install a pre-commit hook that runs `forge audit secrets`
    /// against staged changes + refuses commits that introduce
    /// secret-shaped filenames.
    ///
    /// Writes `.githooks/pre-commit` (chmod 0755), prints the
    /// `git config core.hooksPath .githooks` command the operator
    /// needs to run to activate it, and reports success.
    ///
    /// Exit codes:
    ///   0 — hook written (or already up-to-date)
    ///   1 — hook present but content differs (use --force to overwrite)
    ///   2 — fatal (can't write to .githooks/, etc.)
    InitHook {
        /// Overwrite an existing `.githooks/pre-commit` with a
        /// different body. Without this, the command refuses to
        /// replace any non-identical existing hook.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ManifestAction {
    /// Validate phases.toml + backends.toml at the project root.
    /// Reports parsing + projection + topo-sort errors and exits
    /// non-zero on the first gate violation.
    Validate {
        /// Emit a JSON-form summary of phases/backends counts +
        /// detected gaps to stdout. Default is human-readable.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum PrivacyAction {
    /// Validate `privacy.toml` at the project root against the
    /// typed privacy-core surface (T76).
    Validate {
        /// Emit a JSON-form summary of retention coverage to
        /// stdout. Default is human-readable.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum AttestAction {
    /// Generate a fresh Ed25519 keypair and persist it under
    /// `reports/`. Refuses to overwrite an existing key without
    /// `--force`.
    Init {
        /// Overwrite an existing keypair. Use only if you've
        /// rotated keys deliberately + accepted the
        /// chain-of-trust break.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Print the public key in base64 form. Stable identifier
    /// for an external auditor pinning trust to this forge
    /// instance.
    Pubkey,
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

    let root = match &args.root {
        Some(p) => p.clone(),
        None => {
            std::env::current_dir().context("forge needs a project root and CWD is unreadable")?
        }
    };

    // T55: subcommand router. `Build` is the default for backwards
    // compat with `forge` (no subcommand).
    match args.command.as_ref() {
        Some(Cmd::Verify { chain, signatures }) => return run_verify(&root, *chain, *signatures),
        Some(Cmd::Attest { action }) => return run_attest(&root, action),
        Some(Cmd::Audit { action }) => return run_audit(&root, action),
        Some(Cmd::Fix { apply }) => return run_fix(&root, *apply),
        Some(Cmd::Manifest { action }) => return run_manifest(&root, action),
        Some(Cmd::Privacy { action }) => return run_privacy(&root, action),
        Some(Cmd::Watch {
            debounce_ms,
            max_rebuilds,
        }) => return run_watch(&root, *debounce_ms, *max_rebuilds),
        Some(Cmd::Build) | None => {}
    }

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
        // T70c (2026-05-14): regenerate static HTML from cms/*.json
        // BEFORE every lint phase runs. Without this, edits to
        // cms/ or to loom-cms-render's page_shell don't show up
        // in static/ until the operator runs `loom cms-render`
        // manually — a friction surfaced by repeated dogfood loops.
        // The phase opt-out for legacy sites: forge.toml
        //   [render]
        //   write_canonical = false   # default; writes _render/
        // skips overwriting static/.
        Box::new(RenderPhase),
        Box::new(SelfCheckPhase),
        // T51 (2026-05-06): theme_consistency runs early — its
        // findings (e.g. an undefined --loom-color-* reference)
        // tell every downstream phase that depends on themed
        // values that the cascade is broken.
        Box::new(ThemeConsistencyPhase),
        // T29b (2026-05-06): WCAG AA mathematical contrast gate
        // on every theme. Pairs with ThemeConsistencyPhase: that
        // checks token presence; this checks token VALUES against
        // the contrast threshold. Together they ensure no theme
        // ships unreadable.
        Box::new(ThemeContrastPhase),
        // T57 (2026-05-06): cms.path → static/<file>.html
        // consistency. Catches typo'd routes (e.g. /compose vs
        // /compose.html) at build time, before the crawler hits
        // a runtime 404.
        Box::new(PathConsistencyPhase),
        Box::new(TokensPhase),
        Box::new(HtmlSemanticPhase),
        Box::new(CspPhase),
        Box::new(CspDevmodePhase),
        Box::new(ExternalAssetsPhase),
        Box::new(A11yLandmarksPhase),
        Box::new(IdStrategyPhase),
        Box::new(SeoPhase),
        // phase_structured_data — Schema.org JSON-LD per page +
        // @context/@type validation. Silent skip when
        // [structured_data] missing from forge.toml.
        Box::new(StructuredDataPhase),
        Box::new(PerfBudgetPhase),
        // phase_carbon_budget — per-page byte total tracked against
        // [carbon_budget] kb_per_page in forge.toml + Sustainable
        // Web Design g-CO2 estimate. Silent skip when unconfigured.
        Box::new(CarbonBudgetPhase),
        Box::new(AssetOptimizationPhase),
        Box::new(SriPhase),
        Box::new(PhantomButtonPhase),
        Box::new(BackendCoveragePhase),
        // phase_required_pages — site-type + jurisdiction-aware
        // required-pages doctrine from SITE_OPERATIONS.md §1.
        // Silent skip when [required_pages] missing from forge.toml;
        // sites that haven't opted into the contract aren't gated.
        Box::new(RequiredPagesPhase),
        // phase_locale_html_lang — every <html lang="..."> matches
        // the site's declared locale set. WCAG 2.1 SC 3.1.1 Level A
        // baseline. Silent skip when [locale] missing from
        // forge.toml.
        Box::new(LocaleHtmlLangPhase),
        // phase_jurisdiction_compliance — runtime compliance
        // markers (cookie banner / CCPA / LGPD / age gate) per
        // declared jurisdictions in forge.toml.
        Box::new(JurisdictionCompliancePhase),
        Box::new(UnbuiltRoutePhase),
        Box::new(LabelConsistencyPhase),
        Box::new(LinkCheckPhase),
        Box::new(MotionPhase),
        // phase_motion_respects_reduced — every CSS animation /
        // transition / scroll-behavior must be guarded by
        // @media (prefers-reduced-motion). WCAG 2.1 SC 2.3.3.
        // Silent skip when [motion_respects_reduced] missing.
        Box::new(MotionRespectsReducedPhase),
        // phase_print_stylesheet — verify print CSS exists +
        // optionally meets minimum quality (link-URL expansion,
        // background normalization). Silent skip when
        // [print_stylesheet] missing.
        Box::new(PrintStylesheetPhase),
        // phase_network_target_enforcement — sites declaring a
        // non-clearnet deploy target (Tor/I2P/IPFS/Gemini/Lokinet)
        // must not contain clearnet-URL references. Silent skip
        // for clearnet-only or no-[networks] sites.
        Box::new(NetworkTargetEnforcementPhase),
        // phase_reader_safety — Tor-mode reader-side checks
        // (no inline script / no @font-face / no recaptcha /
        // cookie+localStorage warnings / etc). Auto-fires when
        // [networks].targets includes tor/i2p/lokinet; opt-in for
        // clearnet via explicit [reader_safety] section.
        Box::new(ReaderSafetyPhase),
        // phase_dns_hygiene_lint — emit Warn-level DNS-record
        // checklist per declared [dns_hygiene] features. Cannot
        // verify external DNS state; surfaces what operator
        // needs to add at registrar.
        Box::new(DnsHygieneLintPhase),
        Box::new(ContrastPhase),
        // T432 (closes #432): emit SPA client runtime + inject
        // <script> tag into every page WHEN mode is Dynamic or
        // Hybrid. No-op in Poc/Production/Static so legacy SSG
        // workflows are byte-identical. Runs after all linting
        // and audit phases so they see the pre-injection HTML,
        // but before CrawlPhase so the runtime gets exercised
        // by the headless browser audit.
        Box::new(DynamicRuntimePhase),
        // Annotator↔Forge integration (closes task #13): consume
        // operator-flagged elements from annotator-relay sessions
        // and surface them as Findings. Runs after runtime audit
        // so axe/runtime regressions get their own rung first;
        // operator-flagged signals layer on top. Silent skip if
        // [review] session_dir is unconfigured or absent.
        Box::new(AnnotationReviewPhase),
        // T52 (2026-05-06): runtime audit runs LAST. Build-
        // infra issues surface earlier; runtime-only regressions
        // (placeholder text in DOM, ARIA drift, axe runtime) get
        // their own rung so the operator can tell them apart.
        Box::new(CrawlPhase),
    ];

    let started = std::time::Instant::now();
    let started_iso = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| String::from("?"));

    // T24 cycle 2 (advances #576): opt-in type-state pipeline.
    // FORGE_PIPELINE=1 routes the same phases through
    // forge_core::pipeline so the parse → render → audit shape
    // actually carries traffic. Default path stays the flat loop
    // for zero-risk regression on prod builds. Once the pipeline
    // path has soaked, the flat loop will be removed.
    let use_pipeline = std::env::var("FORGE_PIPELINE").ok().as_deref() == Some("1");
    let mode_str = format!("{mode:?}").to_lowercase();
    println!(
        "forge {} mode={}{}",
        env!("CARGO_PKG_VERSION"),
        mode_str,
        if use_pipeline { " [pipeline]" } else { "" }
    );

    let mut report = if use_pipeline {
        run_phases_through_pipeline(&ctx, &phases, started_iso.clone())
            .context("type-state pipeline run")?
    } else {
        let mut report = BuildReport {
            mode: mode_str,
            ..Default::default()
        };
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
        report
    };
    report.duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    report.started = started_iso;

    // T26: Merkle-chain this report to its predecessor. Read the
    // newest existing reports/build-*.json; if found, hash it
    // and store the hash as prev_hash + bump chain_length.
    // Genesis runs (no prior report) yield prev_hash=None,
    // chain_length=1.
    let reports_dir = root.join("reports");
    let prior_report = load_newest_prior_report(&reports_dir);
    forge_core::attest::chain_step(prior_report.as_ref(), &mut report).context("chain_step")?;

    // T56: if a signing key exists at reports/attest-key.b64,
    // sign the chain root. Missing key = unsigned build (silent
    // skip — operator hasn't run `forge attest init` yet).
    let key_path = reports_dir.join("attest-key.b64");
    if key_path.is_file() {
        match std::fs::read_to_string(&key_path)
            .ok()
            .and_then(|s| forge_core::attest::signing_key_from_base64(s.trim()))
        {
            Some(key) => {
                forge_core::attest::sign_report(&mut report, &key).context("sign chain root")?;
            }
            None => {
                eprintln!(
                    "  warn  attest key at {} unreadable — build unsigned",
                    key_path.display()
                );
            }
        }
    }

    println!("\n== summary ==");
    println!("  mode:                {}", report.mode);
    println!("  strict findings:     {}", report.strict_count);
    println!("  suppressible warns:  {}", report.warn_count);
    println!("  duration:            {}ms", report.duration_ms);
    println!("  chain length:        {}", report.chain_length);
    if let Some(h) = &report.prev_hash {
        println!(
            "  prev hash:           {}…{}",
            &h[..8],
            &h[h.len().saturating_sub(8)..]
        );
    } else {
        println!("  prev hash:           (genesis)");
    }
    if let Some(s) = &report.signature {
        println!(
            "  signature:           {}…{} (Ed25519)",
            &s[..8],
            &s[s.len().saturating_sub(8)..]
        );
    } else {
        println!("  signature:           (unsigned — run `forge attest init` to enable)");
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

/// T24 cycle 2 (advances #576): drive the existing flat phase
/// list through the type-state pipeline.
///
/// Phase classification (preserves declared order within each
/// bucket — the documented order in the flat list is sacrosanct):
///
///   parse stage  → ValidateCmsPhase, LoomSyncPhase
///   render stage → RenderPhase, SelfCheckPhase
///   audit stage  → every other phase
///
/// The classification matches each phase's documented purpose
/// (validate_cms is the entry gate; loom_sync drift-checks
/// skin.css; render emits HTML; self_check verifies the emit;
/// the rest audit the rendered output). Future commits migrate
/// individual phases to typed stage closures with their own
/// per-stage artifacts.
fn run_phases_through_pipeline(
    ctx: &BuildCtx,
    phases: &[Box<dyn Phase>],
    started_iso: String,
) -> Result<BuildReport> {
    fn is_parse(name: &str) -> bool {
        matches!(name, "validate_cms" | "loom_sync")
    }
    fn is_render(name: &str) -> bool {
        matches!(name, "render" | "self_check")
    }

    let pipeline = Pipeline::start(ctx.clone())
        .with_start_iso(started_iso)
        .discover(|_| {
            // No real discovery yet — phases that walk static/
            // do their own walking. Future commit: emit the
            // walked file list here so phases can read from
            // typed inventory instead of restating the walk.
            Ok(StageOutput::clean(DiscoveredArtifacts::default()))
        })
        .map_err(|e| anyhow::anyhow!("pipeline discover: {e}"))?
        .parse(|ctx, _| {
            println!("\n== pipeline stage: parse ==");
            let mut findings = Vec::new();
            for phase in phases.iter().filter(|p| is_parse(p.name())) {
                let started = std::time::Instant::now();
                println!("  -- phase: {}", phase.name());
                let f = phase.run(ctx).map_err(|e| BuildError::Other {
                    phase: phase.name().to_owned(),
                    message: format!("{e}"),
                })?;
                report_inline(&f, started);
                findings.extend(f);
            }
            Ok(StageOutput {
                artifacts: ParsedArtifacts::default(),
                findings,
            })
        })
        .map_err(|e| anyhow::anyhow!("pipeline parse: {e}"))?
        .render(|ctx, _, _| {
            println!("\n== pipeline stage: render ==");
            let mut findings = Vec::new();
            for phase in phases.iter().filter(|p| is_render(p.name())) {
                let started = std::time::Instant::now();
                println!("  -- phase: {}", phase.name());
                let f = phase.run(ctx).map_err(|e| BuildError::Other {
                    phase: phase.name().to_owned(),
                    message: format!("{e}"),
                })?;
                report_inline(&f, started);
                findings.extend(f);
            }
            Ok(StageOutput {
                artifacts: RenderedArtifacts::default(),
                findings,
            })
        })
        .map_err(|e| anyhow::anyhow!("pipeline render: {e}"))?
        .audit(|ctx, _, _| {
            println!("\n== pipeline stage: audit ==");
            let mut findings = Vec::new();
            let mut phases_run = 0usize;
            let mut clean_phases = 0usize;
            for phase in phases
                .iter()
                .filter(|p| !is_parse(p.name()) && !is_render(p.name()))
            {
                let started = std::time::Instant::now();
                println!("  -- phase: {}", phase.name());
                let f = phase.run(ctx).map_err(|e| BuildError::Other {
                    phase: phase.name().to_owned(),
                    message: format!("{e}"),
                })?;
                report_inline(&f, started);
                phases_run += 1;
                if f.is_empty() {
                    clean_phases += 1;
                }
                findings.extend(f);
            }
            Ok(StageOutput {
                artifacts: AuditedArtifacts {
                    phases_run,
                    clean_phases,
                },
                findings,
            })
        })
        .map_err(|e| anyhow::anyhow!("pipeline audit: {e}"))?;

    println!(
        "\n== pipeline summary ==\n  audit phases run:  {}\n  clean phases:      {}",
        pipeline.audited().phases_run,
        pipeline.audited().clean_phases,
    );
    let (report, _) = pipeline
        .into_report(|_, _| Ok(()))
        .map_err(|e| anyhow::anyhow!("pipeline report: {e}"))?;
    Ok(report)
}

fn report_inline(findings: &[Finding], started: std::time::Instant) {
    let elapsed = started.elapsed().as_millis();
    if findings.is_empty() {
        println!("    ok      no findings ({elapsed}ms)");
    } else {
        for f in findings {
            print_finding(f);
        }
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

/// T44: continuous-build watch mode.
///
/// Watches `cms/`, `static/`, `forge.toml`, and `backends.toml`
/// under `root`. On any debounced change, re-execs `forge` (the
/// current binary, with no `watch` subcommand) so the existing
/// `run()` build pipeline runs unchanged.
///
/// Why subprocess re-exec instead of in-process loop:
///   * Phase order, BuildCtx setup, Merkle-chain logic all live
///     in `run()` — re-using them as a function would mean
///     factoring out `run_build()` from `run()`. Subprocess is
///     a smaller change with the same operator semantics.
///   * Each rebuild gets a fresh process — no in-memory state
///     leaks across rebuilds, no risk of phases interfering
///     across iterations.
///
/// Debounce: `notify` can fire many events for one editor save
/// (vim swap files, atomic-rename, fsync flushes). We coalesce
/// any burst of events within `debounce_ms` into a single
/// rebuild.
///
/// Exit: Ctrl-C terminates the loop. `--max-rebuilds N` exits
/// cleanly after N rebuilds (used for tests + CI smoke).
///
/// AVP-PASS-T44: 2026-05-14.
fn run_watch(root: &std::path::Path, debounce_ms: u64, max_rebuilds: usize) -> Result<ExitCode> {
    use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::{channel, RecvTimeoutError};
    use std::time::{Duration, Instant};

    // T44 BUG-GUARD: notify emits events with ABSOLUTE paths.
    // is_relevant_event strip_prefix's against the watch root,
    // so we must canonicalise BEFORE handing to either notify or
    // the filter — otherwise a relative `.` root never matches
    // the absolute event paths and every event is "irrelevant."
    let root_canon = root
        .canonicalize()
        .with_context(|| format!("canonicalize root {}", root.display()))?;
    let root = root_canon.as_path();

    eprintln!("forge watch:");
    eprintln!("  ok    watching {}", root.display());
    eprintln!("  ok    debounce {debounce_ms}ms");
    if max_rebuilds > 0 {
        eprintln!("  info  capped at {max_rebuilds} rebuilds");
    }
    eprintln!("  info  Ctrl-C to exit");
    eprintln!();

    // Initial build runs immediately so the operator sees current
    // state without having to save first.
    let mut rebuild_count = 0usize;
    rebuild_once(root, "initial", &mut rebuild_count)?;
    if max_rebuilds > 0 && rebuild_count >= max_rebuilds {
        eprintln!("forge watch: hit max-rebuilds; exiting cleanly");
        return Ok(ExitCode::SUCCESS);
    }

    let (tx, rx) = channel::<notify::Result<Event>>();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
        // The notify thread sends events; we coalesce in main.
        let _ = tx.send(res);
    })
    .context("failed to create file-watcher")?;

    // Watch the project root recursively. Subdirs we care about
    // (`cms/`, `static/`) are auto-included; `target/` and dot-
    // dirs are filtered in the event-handler below to keep the
    // operator from triggering rebuilds via cargo activity.
    watcher
        .watch(root, RecursiveMode::Recursive)
        .context("failed to watch project root")?;

    let debounce = Duration::from_millis(debounce_ms);
    loop {
        // Block until first event.
        let first = match rx.recv() {
            Ok(r) => r,
            Err(_) => {
                eprintln!("forge watch: watcher channel closed; exiting");
                return Ok(ExitCode::SUCCESS);
            }
        };
        if !is_relevant_event(&first, root) {
            continue;
        }

        // Coalesce burst within debounce window.
        let burst_started = Instant::now();
        let mut latest_path: Option<String> = first.ok().and_then(event_path_label);
        loop {
            let remaining = debounce
                .checked_sub(burst_started.elapsed())
                .unwrap_or_default();
            if remaining.is_zero() {
                break;
            }
            match rx.recv_timeout(remaining) {
                Ok(r) => {
                    if is_relevant_event(&r, root) {
                        if let Some(label) = r.ok().and_then(event_path_label) {
                            latest_path = Some(label);
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    eprintln!("forge watch: watcher channel disconnected; exiting");
                    return Ok(ExitCode::SUCCESS);
                }
            }
        }

        let trigger = latest_path.unwrap_or_else(|| "<unknown>".to_owned());
        rebuild_once(root, &trigger, &mut rebuild_count)?;
        if max_rebuilds > 0 && rebuild_count >= max_rebuilds {
            eprintln!("forge watch: hit max-rebuilds; exiting cleanly");
            return Ok(ExitCode::SUCCESS);
        }
    }
}

/// Filter for filesystem events that should trigger a rebuild.
/// Excludes target/, hidden dot-dirs, and reports/ to avoid
/// rebuild-storm from cargo + the build's own outputs.
fn is_relevant_event(ev: &notify::Result<notify::Event>, root: &std::path::Path) -> bool {
    let Ok(event) = ev else {
        return false;
    };
    for path in &event.paths {
        let rel = match path.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let s = rel.to_string_lossy();
        if s.starts_with("target/")
            || s.starts_with(".git/")
            || s.starts_with("reports/")
            || s.starts_with("node_modules/")
            || s.contains("/.")
            || s.starts_with('.')
        {
            continue;
        }
        // At least one path is interesting.
        return true;
    }
    false
}

fn event_path_label(ev: notify::Event) -> Option<String> {
    ev.paths.first().map(|p| p.display().to_string())
}

/// Re-exec the current binary as `forge build` (no watch flag)
/// so we hit the existing build pipeline. Inherits stdio so
/// findings stream live to the operator's terminal.
fn rebuild_once(root: &std::path::Path, trigger: &str, counter: &mut usize) -> Result<()> {
    *counter += 1;
    let n = *counter;
    eprintln!("---- forge watch rebuild #{n} (trigger: {trigger}) ----");
    let exe = std::env::current_exe().context("locate own binary for re-exec")?;
    let status = std::process::Command::new(exe)
        .arg("--root")
        .arg(root)
        .arg("build")
        .status()
        .context("forge build subprocess failed to spawn")?;
    eprintln!(
        "---- forge watch rebuild #{n} done ({}) ----",
        status
            .code()
            .map(|c| format!("exit {c}"))
            .unwrap_or_else(|| "signaled".to_owned())
    );
    Ok(())
}

/// T56: attestation key management.
fn run_attest(root: &std::path::Path, action: &AttestAction) -> Result<ExitCode> {
    let reports_dir = root.join("reports");
    let _ = std::fs::create_dir_all(&reports_dir);
    let key_path = reports_dir.join("attest-key.b64");
    let pub_path = reports_dir.join("attest-pubkey.b64");
    match action {
        AttestAction::Init { force } => {
            if key_path.exists() && !force {
                eprintln!(
                    "forge attest init: {} already exists; pass --force to overwrite \
                     (chain-of-trust will break for any verifier pinned to the old key)",
                    key_path.display()
                );
                return Ok(ExitCode::from(1));
            }
            let key = forge_core::attest::generate_keypair();
            let priv_b64 = forge_core::attest::signing_key_to_base64(&key);
            let pub_b64 = forge_core::attest::pubkey_to_base64(&key.verifying_key());
            std::fs::write(&key_path, &priv_b64)
                .with_context(|| format!("write {}", key_path.display()))?;
            // Restrict private key to owner-only on Unix.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&key_path)?.permissions();
                perms.set_mode(0o600);
                let _ = std::fs::set_permissions(&key_path, perms);
            }
            std::fs::write(&pub_path, &pub_b64)
                .with_context(|| format!("write {}", pub_path.display()))?;
            println!("forge attest init:");
            println!("  ok    private key → {} (mode 0600)", key_path.display());
            println!("  ok    public  key → {}", pub_path.display());
            println!("  pubkey: {pub_b64}");
            Ok(ExitCode::SUCCESS)
        }
        AttestAction::Pubkey => {
            if !pub_path.is_file() {
                eprintln!(
                    "forge attest pubkey: no {} — run `forge attest init` first",
                    pub_path.display()
                );
                return Ok(ExitCode::from(1));
            }
            let s = std::fs::read_to_string(&pub_path)
                .with_context(|| format!("read {}", pub_path.display()))?;
            print!("{}", s.trim());
            println!();
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// T55: walk `reports/build-*.json` ordered by filename
/// (filenames embed monotonic ts), verify the Merkle chain.
/// T56: also verify signatures when `signatures = true`.
// ============================================================
// T56b: secret-pattern scanner.
// ============================================================
//
// Patterns chosen from real-world incident classes:
//   * private keys (Ed25519 / RSA / ECDSA)
//   * x509 certs / pkcs12 keystores
//   * SSH private keys
//   * dotenv / config files known to carry credentials
//   * password store databases
//
// Each rule is `(name, glob)` where glob is a simple suffix /
// basename match (no full glob crate dep). Rules are matched
// against the BASENAME so paths under any subdir are caught.
//
// REGRESSION-GUARD (added 2026-05-06 after T56 incident):
// `attest-key.b64` was the file I committed. The matching rule
// `*-key.b64` is on this list. DO NOT remove without an
// explicit justification + a replacement gate.

const SECRET_RULES: &[(&str, fn(&str) -> bool)] = &[
    ("ed25519-priv-key", |n| n.ends_with("-key.b64")),
    ("pem-keystore", |n| {
        n.ends_with(".pem") || n.ends_with(".p12") || n.ends_with(".pfx")
    }),
    ("ssh-private-key", |n| {
        n == "id_rsa"
            || n == "id_ed25519"
            || n == "id_ecdsa"
            || n == "id_dsa"
            || n.starts_with("id_rsa.")
            || n.starts_with("id_ed25519.")
            || n.starts_with("id_ecdsa.")
    }),
    ("dotenv", |n| n == ".env" || n.starts_with(".env.")),
    ("password-store", |n| {
        n.ends_with(".kdbx") || n.ends_with(".kdb")
    }),
    ("aws-credentials", |n| {
        n == "credentials" || n == "credentials.json"
    }),
    ("gcp-service-account", |n| {
        n.contains("service-account") && n.ends_with(".json")
    }),
    ("git-credentials", |n| {
        n == ".git-credentials" || n == ".netrc"
    }),
    ("private-key-suffix", |n| {
        n.ends_with(".key") || n.ends_with(".priv") || n.ends_with("-private.b64")
    }),
];

/// Scan a list of file paths for secret-shaped basenames.
/// Returns the matches (path, rule-name) so caller can report.
fn scan_paths_for_secrets<P: AsRef<std::path::Path>>(
    paths: &[P],
) -> Vec<(std::path::PathBuf, &'static str)> {
    let mut hits = Vec::new();
    for p in paths {
        let path = p.as_ref();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        for (rule_name, predicate) in SECRET_RULES {
            if predicate(name) {
                hits.push((path.to_path_buf(), *rule_name));
                break; // one rule per path is enough
            }
        }
    }
    hits
}

/// Read `git diff --cached --name-only` from `cwd`. Returns
/// empty Vec if git is unavailable / not a repo / no staged
/// changes.
fn git_staged_paths(cwd: &std::path::Path) -> Vec<std::path::PathBuf> {
    let out = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(cwd)
        .output();
    let Ok(o) = out else { return Vec::new() };
    if !o.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(std::path::PathBuf::from)
        .collect()
}

// ============================================================
// T58: cargo-mutants integration.
// ============================================================
//
// AVP-2 Tier 6: mutation testing < 5% survival on critical
// paths. cargo-mutants writes results to `mutants.out/` —
// notably `mutants.out/outcomes.json` with per-mutant outcomes.
//
// JSON shape (cargo-mutants 27.x):
//   { "outcomes": [
//       { "outcome": "Caught" | "MissedSurvived" | "Unviable" | "Timeout", ... },
//       ...
//     ],
//     ...
//   }
//
// Survival rate = MissedSurvived / (MissedSurvived + Caught).
// (Unviable + Timeout aren't counted — they're scaffolding
// failures, not mutation outcomes.)

#[derive(serde::Deserialize, Debug, Default)]
struct MutantsOutcomes {
    #[serde(default)]
    outcomes: Vec<MutantOutcome>,
}

#[derive(serde::Deserialize, Debug)]
struct MutantOutcome {
    /// Either a string or an object — cargo-mutants 27.x has
    /// {"summary": "Caught"} OR a bare string. We accept either.
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    outcome: Option<String>,
}

impl MutantOutcome {
    fn category(&self) -> Option<&str> {
        self.summary.as_deref().or(self.outcome.as_deref())
    }
}

#[derive(Debug, Default, PartialEq)]
struct MutantsSummary {
    caught: usize,
    survived: usize,
    unviable: usize,
    timeout: usize,
    other: usize,
}

impl MutantsSummary {
    fn from_outcomes(o: &MutantsOutcomes) -> Self {
        let mut s = Self::default();
        for m in &o.outcomes {
            match m.category() {
                Some("Caught") => s.caught += 1,
                Some("MissedSurvived") | Some("Survived") => s.survived += 1,
                Some("Unviable") => s.unviable += 1,
                Some("Timeout") => s.timeout += 1,
                _ => s.other += 1,
            }
        }
        s
    }

    fn survival_rate(&self) -> f64 {
        let total = self.caught + self.survived;
        if total == 0 {
            0.0
        } else {
            (self.survived as f64) * 100.0 / (total as f64)
        }
    }
}

fn run_audit_mutants(
    root: &std::path::Path,
    run: bool,
    crate_name: &str,
    threshold: f64,
) -> Result<ExitCode> {
    if run {
        // Optional cargo-mutants invocation. SECURITY: arg-vec
        // only, no shell.
        let status = std::process::Command::new("cargo")
            .args(["mutants", "-p", crate_name])
            .current_dir(root)
            .status();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                eprintln!(
                    "forge audit mutants: cargo mutants exited {s} (continuing to parse outcomes)"
                );
            }
            Err(e) => {
                eprintln!("forge audit mutants: cargo mutants failed: {e}");
                return Ok(ExitCode::from(2));
            }
        }
    }
    let outcomes_path = root.join("mutants.out").join("outcomes.json");
    if !outcomes_path.is_file() {
        if run {
            eprintln!(
                "forge audit mutants: cargo mutants ran but {} missing — \
                 the version may write outcomes to a different path",
                outcomes_path.display()
            );
            return Ok(ExitCode::from(2));
        }
        println!(
            "forge audit mutants: no {} yet — run with --run to generate",
            outcomes_path.display()
        );
        return Ok(ExitCode::SUCCESS);
    }
    let raw = std::fs::read_to_string(&outcomes_path)
        .with_context(|| format!("read {}", outcomes_path.display()))?;
    let parsed: MutantsOutcomes =
        serde_json::from_str(&raw).with_context(|| format!("parse {}", outcomes_path.display()))?;
    let summary = MutantsSummary::from_outcomes(&parsed);
    let rate = summary.survival_rate();

    println!("forge audit mutants:");
    println!("  caught     {}", summary.caught);
    println!("  survived   {}", summary.survived);
    println!("  unviable   {}", summary.unviable);
    println!("  timeout    {}", summary.timeout);
    if summary.other > 0 {
        println!("  other      {}", summary.other);
    }
    println!(
        "  survival rate: {:.1}%  (threshold: {:.1}%)",
        rate, threshold
    );

    if rate <= threshold {
        println!("  ok      survival ≤ threshold (AVP-2 Tier 6 met)");
        Ok(ExitCode::SUCCESS)
    } else {
        eprintln!(
            "  FAIL    survival {rate:.1}% > threshold {threshold:.1}% — \
             tests do not constrain {} survived mutations",
            summary.survived
        );
        Ok(ExitCode::from(1))
    }
}

fn run_audit(root: &std::path::Path, action: &AuditAction) -> Result<ExitCode> {
    match action {
        AuditAction::Mutants {
            run,
            crate_name,
            threshold,
        } => run_audit_mutants(root, *run, crate_name, *threshold),
        AuditAction::InitHook { force } => cmd_audit_init_hook(root, *force),
        AuditAction::Secrets { paths, explain } => {
            let scan_targets: Vec<std::path::PathBuf> = if paths.is_empty() {
                git_staged_paths(root)
            } else {
                paths.clone()
            };
            if scan_targets.is_empty() {
                println!(
                    "forge audit secrets: nothing staged + no paths supplied — nothing to scan"
                );
                return Ok(ExitCode::SUCCESS);
            }
            let hits = scan_paths_for_secrets(&scan_targets);
            if hits.is_empty() {
                println!(
                    "forge audit secrets: scanned {} path(s), no secret-shaped names matched",
                    scan_targets.len()
                );
                return Ok(ExitCode::SUCCESS);
            }
            eprintln!(
                "forge audit secrets: {} secret-shaped path(s) found — refuse to commit",
                hits.len()
            );
            for (path, rule) in &hits {
                if *explain {
                    eprintln!("  SECRET  [{rule}]  {}", path.display());
                } else {
                    eprintln!("  SECRET  {}", path.display());
                }
            }
            eprintln!(
                "\nIf this is a false positive, rename the file or add it to a gitignore'd \
                 directory. NEVER --force past this gate."
            );
            Ok(ExitCode::from(1))
        }
    }
}

/// Pre-commit hook script body. Invokes `forge audit secrets`
/// against the staged-paths set; non-zero exit refuses the
/// commit. Stable + idempotent — same body on every install so
/// re-running `forge audit init-hook` is a no-op unless the
/// canonical body has been edited deliberately.
const PRECOMMIT_HOOK_BODY: &str = "#!/usr/bin/env bash
# pre-commit hook installed by `forge audit init-hook`.
# Refuses commits that introduce filenames matching dangerous
# secret-shaped patterns (private keys, certs, dotenv, password
# stores). If `forge` is missing, the hook fails open with a
# warning rather than blocking — operator can re-install via
# `forge audit init-hook --force` once forge is rebuilt.
set -euo pipefail
if ! command -v forge >/dev/null 2>&1; then
    echo 'pre-commit: forge binary not on PATH — secret scan skipped' >&2
    exit 0
fi
exec forge audit secrets --explain
";

fn cmd_audit_init_hook(root: &std::path::Path, force: bool) -> Result<ExitCode> {
    let hooks_dir = root.join(".githooks");
    let hook_path = hooks_dir.join("pre-commit");

    std::fs::create_dir_all(&hooks_dir)
        .with_context(|| format!("create_dir_all {}", hooks_dir.display()))?;

    if hook_path.exists() {
        let existing = std::fs::read_to_string(&hook_path)
            .with_context(|| format!("read existing {}", hook_path.display()))?;
        if existing == PRECOMMIT_HOOK_BODY {
            println!("forge audit init-hook: .githooks/pre-commit already up-to-date — no change.");
            return Ok(ExitCode::SUCCESS);
        }
        if !force {
            eprintln!(
                "forge audit init-hook: .githooks/pre-commit exists with different content. \
                 Use --force to overwrite (review the existing hook first; the operator may \
                 have customized it)."
            );
            return Ok(ExitCode::from(1));
        }
    }

    std::fs::write(&hook_path, PRECOMMIT_HOOK_BODY)
        .with_context(|| format!("write {}", hook_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_path)
            .with_context(|| format!("metadata {}", hook_path.display()))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms)
            .with_context(|| format!("chmod 0755 {}", hook_path.display()))?;
    }

    println!(
        "forge audit init-hook: wrote {} ({} bytes, mode 0755).

To activate (one-time per clone):
    git config core.hooksPath .githooks

To verify before committing real changes:
    touch test-private-key.pem
    git add test-private-key.pem
    git commit -m 'should be rejected'
    # → 'forge audit secrets: 1 secret-shaped path(s) found — refuse to commit'
    git restore --staged test-private-key.pem
    rm test-private-key.pem

To re-install if the canonical body changes in a future Forge
release:
    forge audit init-hook --force",
        hook_path.display(),
        PRECOMMIT_HOOK_BODY.len(),
    );
    Ok(ExitCode::SUCCESS)
}

/// `forge fix` — auto-fix mechanical findings from the latest
/// build report. v1 ships ONE fixer (security_txt) and the
/// framework for adding more. Dry-run by default; `--apply`
/// writes.
fn run_fix(root: &std::path::Path, apply: bool) -> Result<ExitCode> {
    // Locate the latest build report.
    let reports_dir = root.join("reports");
    let mut reports: Vec<std::path::PathBuf> = std::fs::read_dir(&reports_dir)
        .with_context(|| format!("read_dir {}", reports_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().and_then(|s| s.to_str()) == Some("json")
                && p.file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.starts_with("build-"))
                    .unwrap_or(false)
        })
        .collect();
    if reports.is_empty() {
        eprintln!(
            "forge fix: no reports/build-*.json found — run `forge build` first then re-run."
        );
        return Ok(ExitCode::from(2));
    }
    reports.sort();
    let latest = reports.last().expect("non-empty");
    let body =
        std::fs::read_to_string(latest).with_context(|| format!("read {}", latest.display()))?;
    let report: serde_json::Value =
        serde_json::from_str(&body).with_context(|| format!("parse {}", latest.display()))?;
    let findings = report
        .get("findings")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Build the fix-plan: pairs (Finding, FixAction).
    let mut planned: Vec<FixAction> = Vec::new();
    for finding in &findings {
        let phase = finding.get("phase").and_then(|v| v.as_str()).unwrap_or("");
        let path = finding.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let message = finding
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Fixer: required_pages.security_txt — create the file.
        if phase == "required_pages" && path == "security_txt" {
            planned.push(FixAction::CreateSecurityTxt);
        }
        let _ = message;
    }

    if planned.is_empty() {
        println!(
            "forge fix: no auto-fixable findings in {} — nothing to do.",
            latest.file_name().unwrap_or_default().to_string_lossy()
        );
        return Ok(ExitCode::SUCCESS);
    }

    println!(
        "forge fix: {} auto-fixable finding(s) in {} (mode={}):",
        planned.len(),
        latest.file_name().unwrap_or_default().to_string_lossy(),
        if apply { "apply" } else { "dry-run" }
    );
    let mut applied = 0usize;
    for action in &planned {
        let summary = action.summary();
        if apply {
            match action.apply(root) {
                Ok(()) => {
                    println!("  [applied]  {summary}");
                    applied += 1;
                }
                Err(e) => {
                    eprintln!("  [failed]   {summary}: {e}");
                }
            }
        } else {
            println!("  [proposed] {summary}");
        }
    }

    if apply {
        println!(
            "\nforge fix: applied {applied} of {} fix(es). Re-run `forge build` to verify.",
            planned.len()
        );
        Ok(ExitCode::SUCCESS)
    } else {
        println!("\nDry-run only — re-run with `--apply` to write the fixes.");
        // Exit 1 so CI pipelines piping `forge fix` see "stuff to
        // fix" without the operator needing a separate flag.
        Ok(ExitCode::from(1))
    }
}

/// Per-fix mechanical-edit action. New fixers add a variant +
/// implement `summary` and `apply`. Keep each fixer SAFE: must
/// be idempotent + must never overwrite operator-authored
/// content silently.
#[derive(Debug, Clone)]
enum FixAction {
    /// Create `static/.well-known/security.txt` from a template
    /// if the file is missing. Idempotent — if it exists, this
    /// action skips silently.
    CreateSecurityTxt,
}

impl FixAction {
    fn summary(&self) -> String {
        match self {
            Self::CreateSecurityTxt => {
                "required_pages.security_txt — create static/.well-known/security.txt".to_owned()
            }
        }
    }

    fn apply(&self, root: &std::path::Path) -> Result<()> {
        match self {
            Self::CreateSecurityTxt => apply_create_security_txt(root),
        }
    }
}

/// Canonical security.txt body. Per RFC 9116, every Contact line
/// MUST exist; Expires SHOULD be within 1 year. Operator edits
/// the file after generation to put real values.
const SECURITY_TXT_TEMPLATE: &str = "# RFC 9116 vulnerability-disclosure declaration.
# Edit Contact + Expires + Preferred-Languages to match your
# operator setup. Re-generate via `forge fix` if removed.

Contact: mailto:security@example.com
Expires: 2026-12-31T23:59:59Z
Preferred-Languages: en

# Optional: PGP-Encryption key fingerprint
# Encryption: https://example.com/.well-known/openpgpkey

# Optional: link to scope + policy
# Policy: https://example.com/security-policy

# Optional: where to thank disclosers
# Acknowledgments: https://example.com/security-acknowledgments
";

fn apply_create_security_txt(root: &std::path::Path) -> Result<()> {
    let well_known = root.join("static").join(".well-known");
    let target = well_known.join("security.txt");
    if target.exists() {
        // Idempotent: don't overwrite. Operator may have customized.
        return Ok(());
    }
    std::fs::create_dir_all(&well_known)
        .with_context(|| format!("create_dir_all {}", well_known.display()))?;
    std::fs::write(&target, SECURITY_TXT_TEMPLATE)
        .with_context(|| format!("write {}", target.display()))?;
    Ok(())
}

fn run_verify(root: &std::path::Path, chain: bool, signatures: bool) -> Result<ExitCode> {
    if !chain {
        // Currently chain is the only verify mode; reject other
        // shapes politely so future flags surface their own help.
        eprintln!("forge verify: pass --chain (currently the only mode)");
        return Ok(ExitCode::from(2));
    }
    let reports_dir = root.join("reports");
    if !reports_dir.is_dir() {
        println!(
            "forge verify --chain: no reports/ directory at {} — nothing to verify",
            reports_dir.display()
        );
        return Ok(ExitCode::SUCCESS);
    }

    // Collect + sort by filename (filenames embed monotonic ts).
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(&reports_dir)
        .with_context(|| format!("read_dir {}", reports_dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.starts_with("build-") && name.ends_with(".json")
        })
        .collect();
    entries.sort();

    if entries.is_empty() {
        println!("forge verify --chain: no build-*.json reports — nothing to verify");
        return Ok(ExitCode::SUCCESS);
    }

    let mut chain_reports: Vec<BuildReport> = Vec::with_capacity(entries.len());
    for path in &entries {
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let report: BuildReport =
            serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
        chain_reports.push(report);
    }

    println!(
        "forge verify --chain: walking {} report(s) in {}",
        chain_reports.len(),
        reports_dir.display()
    );
    match forge_core::attest::verify_chain(&chain_reports) {
        Ok(()) => {
            let last = chain_reports.last();
            let head_len = last.map(|r| r.chain_length).unwrap_or(0);
            let head_started = last.map(|r| r.started.as_str()).unwrap_or("?");
            println!(
                "  ok      chain intact — head chain_length={head_len} started={head_started}"
            );

            // T56: optional signature verification.
            if signatures {
                let pub_path = reports_dir.join("attest-pubkey.b64");
                if !pub_path.is_file() {
                    eprintln!(
                        "  FAIL    --signatures requested but {} missing — run `forge attest init`",
                        pub_path.display()
                    );
                    return Ok(ExitCode::from(1));
                }
                let pub_b64 = std::fs::read_to_string(&pub_path)
                    .with_context(|| format!("read {}", pub_path.display()))?;
                let pubkey = match forge_core::attest::pubkey_from_base64(pub_b64.trim()) {
                    Some(p) => p,
                    None => {
                        eprintln!(
                            "  FAIL    {} is not a valid base64 ed25519 pubkey",
                            pub_path.display()
                        );
                        return Ok(ExitCode::from(2));
                    }
                };
                let mut signed = 0usize;
                let mut unsigned = 0usize;
                for (idx, r) in chain_reports.iter().enumerate() {
                    if r.signature.is_none() {
                        unsigned += 1;
                        continue;
                    }
                    if let Err(e) = forge_core::attest::verify_report(r, &pubkey) {
                        eprintln!("  FAIL    signature mismatch at index {idx}: {e}");
                        if let Some(p) = entries.get(idx) {
                            eprintln!("  bad     {}", p.display());
                        }
                        return Ok(ExitCode::from(1));
                    }
                    signed += 1;
                }
                println!(
                    "  ok      {signed} signature(s) verified, {unsigned} unsigned (genesis-era)"
                );
            }

            Ok(ExitCode::SUCCESS)
        }
        Err(e) => {
            // T55: print the typed ChainError verbatim. Each
            // variant carries the at_index + expected/actual so
            // the operator can immediately bisect to the bad file.
            eprintln!("  FAIL    chain divergence: {e}");
            // Surface which file the divergence is at if available.
            if let Some(idx) = chain_error_index(&e) {
                if let Some(p) = entries.get(idx) {
                    eprintln!("  bad     {}", p.display());
                }
            }
            Ok(ExitCode::from(1))
        }
    }
}

/// Extract the index of the offending file from a ChainError.
///
/// Subtle: ChainError::Broken { at_index } reports the SUCCESSOR
/// — that's where the broken `prev_hash` was *detected*. But the
/// actual tampered file is the PREDECESSOR (its bytes no longer
/// hash to what the successor recorded). We return at_index - 1
/// for Broken so the operator opens the right file.
///
/// SequenceGap + MissingPrev are detected at the successor and
/// the successor IS the bad file (its chain_length / prev_hash
/// is forged), so they return at_index unchanged.
fn chain_error_index(e: &forge_core::attest::ChainError) -> Option<usize> {
    use forge_core::attest::ChainError;
    match e {
        ChainError::Broken { at_index, .. } => at_index.checked_sub(1),
        ChainError::SequenceGap { at_index, .. } | ChainError::MissingPrev { at_index, .. } => {
            Some(*at_index)
        }
        ChainError::GenesisHasPrev | ChainError::Serialize(_) => None,
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

// ====================================================================
// T33: manifest-keystone gate.
//
// Loads phases.toml + backends.toml from the project root, projects
// through manifest-core, and asserts internal consistency. Designed
// for use in a CI workflow (.github/workflows/manifest-gate.yml).
// ====================================================================

#[derive(Debug, serde::Serialize)]
struct ManifestGateSummary {
    backends_path: String,
    phases_path: String,
    backends_declared: usize,
    backends_stub: usize,
    phases_declared: usize,
    phases_topo_ok: bool,
    violations: Vec<String>,
}

fn run_manifest(root: &std::path::Path, action: &ManifestAction) -> Result<ExitCode> {
    match action {
        ManifestAction::Validate { json } => run_manifest_validate(root, *json),
    }
}

fn run_manifest_validate(root: &std::path::Path, json: bool) -> Result<ExitCode> {
    let backends_path = root.join("backends.toml");
    let phases_path = root.join("phases.toml");
    let mut violations: Vec<String> = Vec::new();
    let mut summary = ManifestGateSummary {
        backends_path: backends_path.display().to_string(),
        phases_path: phases_path.display().to_string(),
        backends_declared: 0,
        backends_stub: 0,
        phases_declared: 0,
        phases_topo_ok: false,
        violations: Vec::new(),
    };

    // --- backends.toml ---
    if !backends_path.exists() {
        violations.push(format!(
            "missing backends.toml at {}",
            backends_path.display()
        ));
    } else {
        let s = std::fs::read_to_string(&backends_path)
            .with_context(|| format!("reading {}", backends_path.display()))?;
        match manifest_core::projections::backends_toml::BackendsToml::from_toml(&s) {
            Ok(bt) => match bt.to_descriptors() {
                Ok(descs) => {
                    summary.backends_declared = descs.len();
                    summary.backends_stub = bt.stub_ids().len();
                }
                Err(e) => violations.push(format!("backends.toml projection: {e}")),
            },
            Err(e) => violations.push(format!("backends.toml parse: {e}")),
        }
    }

    // --- phases.toml ---
    if !phases_path.exists() {
        violations.push(format!("missing phases.toml at {}", phases_path.display()));
    } else {
        let s = std::fs::read_to_string(&phases_path)
            .with_context(|| format!("reading {}", phases_path.display()))?;
        match manifest_core::projections::phases_toml::PhasesToml::from_toml(&s) {
            Ok(pt) => {
                match pt.to_descriptors() {
                    Ok(descs) => summary.phases_declared = descs.len(),
                    Err(e) => violations.push(format!("phases.toml projection: {e}")),
                }
                match pt.topo_sort() {
                    Ok(_) => summary.phases_topo_ok = true,
                    Err(e) => violations.push(format!("phases.toml topo-sort: {e}")),
                }
            }
            Err(e) => violations.push(format!("phases.toml parse: {e}")),
        }
    }

    summary.violations = violations.clone();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&summary)
                .unwrap_or_else(|_| "{\"error\":\"serialize-failed\"}".to_string())
        );
    } else {
        println!(
            "manifest-gate: {} backends ({} stub), {} phases, topo {}",
            summary.backends_declared,
            summary.backends_stub,
            summary.phases_declared,
            if summary.phases_topo_ok { "ok" } else { "FAIL" },
        );
        if !violations.is_empty() {
            println!("violations:");
            for v in &violations {
                println!("  - {v}");
            }
        }
    }

    if violations.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

// ============================================================
// T91 (first wiring): privacy gate — `forge privacy validate`
//
// Loads `privacy.toml` at the project root + projects through
// privacy-core (T76). The TOML schema mirrors privacy-core's
// RetentionPolicy + LawfulBasis enums:
//
//   [[retention]]
//   category = "account"            # DataCategory variant
//   retention_days = 2555           # u32, must be > 0
//   basis = "legal-obligation"      # LawfulBasis variant
//   note = "tax-retention 7 years"  # optional operator note
//
// Validation rules:
//   * every DataCategory enum variant has ≥1 retention entry
//   * no duplicate entries per category
//   * retention_days > 0
//   * LegalObligation-basis entries flagged as
//     refuses-erasure (informational, not a violation)
// ============================================================

#[derive(serde::Deserialize, Debug)]
struct PrivacyToml {
    #[serde(default)]
    retention: Vec<RetentionEntry>,
}

#[derive(serde::Deserialize, Debug)]
struct RetentionEntry {
    category: privacy_core::DataCategory,
    retention_days: u32,
    basis: privacy_core::LawfulBasis,
    // Operator-supplied audit-trail context. Preserved through
    // TOML so operator tooling round-trips it; the validator
    // itself doesn't consume the field.
    #[serde(default)]
    #[allow(dead_code)]
    note: Option<String>,
}

#[derive(Debug, serde::Serialize)]
struct PrivacyGateSummary {
    privacy_path: String,
    retention_entries: usize,
    categories_covered: usize,
    categories_total: usize,
    categories_missing: Vec<&'static str>,
    legal_obligation_categories: Vec<&'static str>,
    violations: Vec<String>,
}

fn all_data_categories() -> [privacy_core::DataCategory; 9] {
    use privacy_core::DataCategory::*;
    [
        Account,
        Content,
        AuditLog,
        Telemetry,
        Payment,
        SupportTicket,
        Marketing,
        Auth,
        Backup,
    ]
}

fn run_privacy(root: &std::path::Path, action: &PrivacyAction) -> Result<ExitCode> {
    match action {
        PrivacyAction::Validate { json } => run_privacy_validate(root, *json),
    }
}

fn run_privacy_validate(root: &std::path::Path, json: bool) -> Result<ExitCode> {
    let privacy_path = root.join("privacy.toml");
    let mut violations: Vec<String> = Vec::new();
    let all_categories = all_data_categories();
    let mut summary = PrivacyGateSummary {
        privacy_path: privacy_path.display().to_string(),
        retention_entries: 0,
        categories_covered: 0,
        categories_total: all_categories.len(),
        categories_missing: Vec::new(),
        legal_obligation_categories: Vec::new(),
        violations: Vec::new(),
    };

    if !privacy_path.exists() {
        violations.push(format!(
            "missing privacy.toml at {}",
            privacy_path.display()
        ));
        summary.violations = violations.clone();
        emit_privacy_summary(&summary, json);
        return Ok(ExitCode::from(2));
    }

    let s = std::fs::read_to_string(&privacy_path)
        .with_context(|| format!("reading {}", privacy_path.display()))?;
    let parsed: PrivacyToml = match toml::from_str(&s) {
        Ok(p) => p,
        Err(e) => {
            violations.push(format!("privacy.toml parse: {e}"));
            summary.violations = violations.clone();
            emit_privacy_summary(&summary, json);
            return Ok(ExitCode::from(2));
        }
    };

    summary.retention_entries = parsed.retention.len();

    // Detect duplicates per category + zero/negative retention.
    let mut seen: std::collections::HashSet<privacy_core::DataCategory> =
        std::collections::HashSet::new();
    for r in &parsed.retention {
        if !seen.insert(r.category) {
            violations.push(format!(
                "duplicate retention entry for category {:?}",
                r.category.slug()
            ));
        }
        if r.retention_days == 0 {
            violations.push(format!(
                "retention_days = 0 for category {:?}",
                r.category.slug()
            ));
        }
        if matches!(r.basis, privacy_core::LawfulBasis::LegalObligation) {
            summary.legal_obligation_categories.push(r.category.slug());
        }
    }

    // Detect uncovered categories.
    for c in all_categories.iter() {
        if !seen.contains(c) {
            summary.categories_missing.push(c.slug());
            violations.push(format!("no retention policy for category {:?}", c.slug()));
        }
    }
    summary.categories_covered = all_categories.len() - summary.categories_missing.len();

    summary.violations = violations.clone();
    emit_privacy_summary(&summary, json);

    if violations.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn emit_privacy_summary(summary: &PrivacyGateSummary, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(summary)
                .unwrap_or_else(|_| "{\"error\":\"serialize-failed\"}".to_string())
        );
    } else {
        println!(
            "privacy-gate: {} entries, {}/{} DataCategory variants covered",
            summary.retention_entries, summary.categories_covered, summary.categories_total,
        );
        if !summary.categories_missing.is_empty() {
            println!("missing retention policies:");
            for c in &summary.categories_missing {
                println!("  - {c}");
            }
        }
        if !summary.legal_obligation_categories.is_empty() {
            println!("legal-obligation categories (refuse-erasure):");
            for c in &summary.legal_obligation_categories {
                println!("  - {c}");
            }
        }
        if !summary.violations.is_empty() {
            println!("violations:");
            for v in &summary.violations {
                println!("  - {v}");
            }
        }
    }
}

#[cfg(test)]
mod privacy_gate_tests {
    use super::*;
    use tempfile::TempDir;

    fn write_privacy(dir: &std::path::Path, body: &str) {
        std::fs::write(dir.join("privacy.toml"), body).unwrap();
    }

    fn all_categories_toml() -> String {
        // Cover every DataCategory variant — used to assert
        // the happy path validates clean.
        all_data_categories()
            .iter()
            .map(|c| {
                format!(
                    "[[retention]]\ncategory = \"{}\"\nretention_days = 30\nbasis = \"contract\"\n",
                    c.slug()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn missing_privacy_toml_exits_2() {
        let td = TempDir::new().unwrap();
        let code = run_privacy_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn malformed_privacy_toml_exits_2() {
        let td = TempDir::new().unwrap();
        write_privacy(td.path(), "this is not valid toml [[[");
        let code = run_privacy_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn complete_coverage_exits_0() {
        let td = TempDir::new().unwrap();
        write_privacy(td.path(), &all_categories_toml());
        let code = run_privacy_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn missing_category_exits_1() {
        let td = TempDir::new().unwrap();
        // Cover only Account; the other 8 categories are missing.
        write_privacy(
            td.path(),
            "[[retention]]\ncategory = \"account\"\nretention_days = 30\nbasis = \"contract\"\n",
        );
        let code = run_privacy_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn duplicate_category_exits_1() {
        let td = TempDir::new().unwrap();
        let mut body = all_categories_toml();
        body.push_str(
            "\n[[retention]]\ncategory = \"account\"\nretention_days = 60\nbasis = \"consent\"\n",
        );
        write_privacy(td.path(), &body);
        let code = run_privacy_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn zero_retention_days_exits_1() {
        let td = TempDir::new().unwrap();
        // Cover every category, but with retention_days = 0 on one.
        let mut body = String::new();
        for (i, c) in all_data_categories().iter().enumerate() {
            let days = if i == 0 { 0 } else { 30 };
            body.push_str(&format!(
                "[[retention]]\ncategory = \"{}\"\nretention_days = {}\nbasis = \"contract\"\n\n",
                c.slug(),
                days
            ));
        }
        write_privacy(td.path(), &body);
        let code = run_privacy_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn legal_obligation_basis_is_informational_not_violation() {
        let td = TempDir::new().unwrap();
        // Cover every category; one of them uses legal-obligation.
        let mut body = String::new();
        for (i, c) in all_data_categories().iter().enumerate() {
            let basis = if i == 0 {
                "legal-obligation"
            } else {
                "contract"
            };
            body.push_str(&format!(
                "[[retention]]\ncategory = \"{}\"\nretention_days = 30\nbasis = \"{}\"\n\n",
                c.slug(),
                basis
            ));
        }
        write_privacy(td.path(), &body);
        let code = run_privacy_validate(td.path(), false).unwrap();
        // Legal obligation alone is NOT a violation — coverage
        // is complete + no zeroes + no dupes.
        assert_eq!(code, ExitCode::SUCCESS);
    }
}

#[cfg(test)]
mod manifest_gate_tests {
    use super::*;

    fn write_files(dir: &std::path::Path, phases: &str, backends: &str) {
        std::fs::write(dir.join("phases.toml"), phases).unwrap();
        std::fs::write(dir.join("backends.toml"), backends).unwrap();
    }

    #[test]
    fn passes_on_clean_files() {
        let tmp = tempfile::tempdir().unwrap();
        write_files(
            tmp.path(),
            r#"
[phases.a]
summary = "a"
"#,
            r#"
[backends.x]
method = "GET"
path   = "/x"
purpose = "x"
impl_files = ["src/x.rs"]
"#,
        );
        let r = run_manifest_validate(tmp.path(), false).unwrap();
        assert_eq!(r, ExitCode::SUCCESS);
    }

    #[test]
    fn fails_on_phases_cycle() {
        let tmp = tempfile::tempdir().unwrap();
        write_files(
            tmp.path(),
            r#"
[phases.a]
summary    = "a"
depends_on = ["b"]
[phases.b]
summary    = "b"
depends_on = ["a"]
"#,
            r#"
[backends.x]
method = "GET"
path   = "/x"
purpose = "x"
impl_files = ["src/x.rs"]
"#,
        );
        let r = run_manifest_validate(tmp.path(), false).unwrap();
        assert_eq!(r, ExitCode::from(1));
    }

    #[test]
    fn fails_on_missing_backends_toml() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("phases.toml"),
            r#"
[phases.a]
summary = "a"
"#,
        )
        .unwrap();
        let r = run_manifest_validate(tmp.path(), false).unwrap();
        assert_eq!(r, ExitCode::from(1));
    }

    #[test]
    fn fails_on_invalid_kebab_case_id() {
        let tmp = tempfile::tempdir().unwrap();
        write_files(
            tmp.path(),
            r#"
[phases.A]
summary = "a"
"#,
            r#"
[backends.x]
method = "GET"
path   = "/x"
purpose = "x"
impl_files = []
"#,
        );
        let r = run_manifest_validate(tmp.path(), false).unwrap();
        assert_eq!(r, ExitCode::from(1));
    }
}

#[cfg(test)]
mod mutants_summary_tests {
    use super::*;

    #[test]
    fn empty_outcomes_yield_zero_rate() {
        let o = MutantsOutcomes::default();
        let s = MutantsSummary::from_outcomes(&o);
        assert_eq!(s.survival_rate(), 0.0);
    }

    #[test]
    fn caught_only_yields_zero_rate() {
        let raw = r#"{"outcomes":[{"summary":"Caught"},{"summary":"Caught"}]}"#;
        let o: MutantsOutcomes = serde_json::from_str(raw).expect("parse");
        let s = MutantsSummary::from_outcomes(&o);
        assert_eq!(s.caught, 2);
        assert_eq!(s.survived, 0);
        assert_eq!(s.survival_rate(), 0.0);
    }

    #[test]
    fn one_survived_three_caught_yields_25_percent() {
        let raw = r#"{"outcomes":[
            {"summary":"Caught"},
            {"summary":"Caught"},
            {"summary":"Caught"},
            {"summary":"MissedSurvived"}
        ]}"#;
        let o: MutantsOutcomes = serde_json::from_str(raw).expect("parse");
        let s = MutantsSummary::from_outcomes(&o);
        assert_eq!(s.caught, 3);
        assert_eq!(s.survived, 1);
        assert!((s.survival_rate() - 25.0).abs() < 1e-9);
    }

    #[test]
    fn unviable_and_timeout_excluded_from_rate() {
        let raw = r#"{"outcomes":[
            {"summary":"Caught"},
            {"summary":"Unviable"},
            {"summary":"Timeout"},
            {"summary":"MissedSurvived"}
        ]}"#;
        let o: MutantsOutcomes = serde_json::from_str(raw).expect("parse");
        let s = MutantsSummary::from_outcomes(&o);
        assert_eq!(s.unviable, 1);
        assert_eq!(s.timeout, 1);
        // Rate = 1 / (1+1) = 50%, NOT 1/4 = 25%.
        assert!((s.survival_rate() - 50.0).abs() < 1e-9);
    }

    #[test]
    fn alternative_field_name_outcome_works() {
        // cargo-mutants version skew: some emit `summary`, some
        // `outcome`. We accept either.
        let raw = r#"{"outcomes":[{"outcome":"Caught"}]}"#;
        let o: MutantsOutcomes = serde_json::from_str(raw).expect("parse");
        let s = MutantsSummary::from_outcomes(&o);
        assert_eq!(s.caught, 1);
    }

    #[test]
    fn unknown_category_routes_to_other() {
        let raw = r#"{"outcomes":[{"summary":"Mystery"}]}"#;
        let o: MutantsOutcomes = serde_json::from_str(raw).expect("parse");
        let s = MutantsSummary::from_outcomes(&o);
        assert_eq!(s.other, 1);
        assert_eq!(s.survival_rate(), 0.0);
    }
}

#[cfg(test)]
mod secret_scan_tests {
    use super::*;
    use std::path::PathBuf;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn flags_attest_key_b64() {
        let hits = scan_paths_for_secrets(&[p("reports/attest-key.b64")]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].1, "ed25519-priv-key");
    }

    #[test]
    fn flags_dotenv() {
        for f in [".env", ".env.local", ".env.production"] {
            let hits = scan_paths_for_secrets(&[p(f)]);
            assert_eq!(hits.len(), 1, "should flag {f}");
            assert_eq!(hits[0].1, "dotenv");
        }
    }

    #[test]
    fn flags_pem_and_p12() {
        for f in ["server.pem", "ca.p12", "keystore.pfx"] {
            let hits = scan_paths_for_secrets(&[p(f)]);
            assert_eq!(hits.len(), 1, "should flag {f}");
            assert_eq!(hits[0].1, "pem-keystore");
        }
    }

    #[test]
    fn flags_ssh_keys() {
        for f in ["id_rsa", "id_ed25519", "id_ecdsa", "id_rsa.bak"] {
            let hits = scan_paths_for_secrets(&[p(f)]);
            assert_eq!(hits.len(), 1, "should flag {f}");
            assert_eq!(hits[0].1, "ssh-private-key");
        }
    }

    #[test]
    fn flags_password_store() {
        let hits = scan_paths_for_secrets(&[p("vault.kdbx")]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].1, "password-store");
    }

    #[test]
    fn does_not_flag_safe_files() {
        for f in [
            "src/main.rs",
            "Cargo.toml",
            "README.md",
            "reports/attest-pubkey.b64",
            "config.toml",
            "static/index.html",
            "id_card.svg", // contains "id_" but not the SSH pattern
        ] {
            let hits = scan_paths_for_secrets(&[p(f)]);
            assert!(hits.is_empty(), "should NOT flag {f}: {hits:?}");
        }
    }

    #[test]
    fn pubkey_b64_is_safe() {
        // The public key is the trust anchor; it MUST be
        // committable. Negative-control test prevents an
        // overzealous future tightening of the rule.
        let hits = scan_paths_for_secrets(&[p("reports/attest-pubkey.b64")]);
        assert!(hits.is_empty(), "pubkey must NOT be flagged");
    }

    #[test]
    fn matches_basename_under_subdirs() {
        let hits = scan_paths_for_secrets(&[p("deeply/nested/dir/secret.pem")]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].1, "pem-keystore");
    }

    #[test]
    fn each_path_emits_at_most_one_hit() {
        // A path that matches multiple rules (rare) should still
        // emit exactly one finding (first match wins).
        let hits = scan_paths_for_secrets(&[p("evil.pem")]);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn flags_aws_credentials() {
        let hits = scan_paths_for_secrets(&[p("credentials"), p("creds/credentials.json")]);
        assert_eq!(hits.len(), 2);
    }
}
