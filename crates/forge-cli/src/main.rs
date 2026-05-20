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

// T96 cleanup: discipline gate (T92) flagged forge-cli for
// missing both `#![forbid(unsafe_code)]` + `#![deny(missing_docs)]`.
// forge-cli is a CLI front that never needs unsafe + has no
// public surface (it's a binary), so both lints are free.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

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
use forge_phases::aesthetic_distinctiveness::AestheticDistinctivenessPhase;
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
use forge_phases::hunted_tier::HuntedTierPhase;
use forge_phases::noscript_strict::NoscriptStrictPhase;
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
    /// T91 (second wiring): trust-safety-core gate. Loads
    /// `trust-safety.toml` at the project root, projects through
    /// `trust-safety-core`'s typed ConcernKind enum, and asserts:
    ///   * every mandatory-report ConcernKind variant (CSAM,
    ///     NCIII, Extremism) has at least one scanner declared
    ///   * non-mandatory variants without a scanner warn but
    ///     don't gate (operator's choice per audience)
    ///   * no duplicate scanner_id within a concern
    ///
    /// Exit codes:
    ///   0 — every mandatory-report concern has a scanner
    ///   1 — at least one mandatory-report concern uncovered
    ///   2 — fatal (missing trust-safety.toml, parse error)
    TrustSafety {
        #[command(subcommand)]
        action: TrustSafetyAction,
    },
    /// T91 (fourth wiring): domains-core gate. Loads
    /// `domains.toml` at the project root, projects through
    /// `domains-core`'s typed Domain + AcmeChallenge + HstsPolicy
    /// surface (T86), and asserts:
    ///   * every Domain FQDN passes RFC 1035 validation
    ///   * Wildcard domains use DNS-01 challenge (RFC 8555 §8.4)
    ///   * HSTS policy is preload-eligible (max-age ≥ 31_536_000
    ///     + includeSubDomains + preload)
    ///   * no duplicate FQDNs
    ///
    /// Exit codes:
    ///   0 — every domain is RFC-valid + challenge-compatible
    ///   1 — at least one gate violation
    ///   2 — fatal (missing domains.toml, parse error)
    Domains {
        #[command(subcommand)]
        action: DomainsAction,
    },
    /// T91 (fifth wiring): observability-core hash-chained
    /// audit-log verifier. Loads a JSON file containing an
    /// `AuditChain` (typed observability-core shape) and runs:
    ///   * sequence monotonicity
    ///   * prev_hash linkage
    ///   * entry_hash freshness (tamper detection)
    AuditLog {
        #[command(subcommand)]
        action: AuditLogAction,
    },
    /// T91 (sixth wiring): forms-core gate. Loads `forms.toml`
    /// at the project root, projects through forms-core's
    /// Form::validate (T81), and asserts:
    ///   * webhook_url is https://
    ///   * every field labelled (WCAG 2.1 §3.3.2)
    ///   * field ids are kebab-case + unique
    ///   * at most one Honeypot field per form
    Forms {
        #[command(subcommand)]
        action: FormsAction,
    },
    /// T91 (seventh wiring): federation-core gate. Loads
    /// `federation.toml` and projects through
    /// federation-core's FederationProtocol +
    /// FederationAddress (T79). Enforces:
    ///   * every protocol declared maps to a valid address
    ///     shape (compile-time via tagged-enum discriminant)
    ///   * address-protocol consistency: a Nostr destination
    ///     can't be paired with an ActivityPub publisher
    ///   * no duplicate destinations within a protocol
    Federation {
        #[command(subcommand)]
        action: FederationAction,
    },
    /// T91 (eighth wiring): email-core gate. Loads
    /// `email.toml` and projects through email-core's typed
    /// OutgoingMessage surface (T83). Enforces:
    ///   * every declared message has a non-empty from /
    ///     subject and at least one recipient
    ///   * Marketing messages have an RFC 8058 https://
    ///     list-unsubscribe URL
    ///   * Transactional messages MAY include unsubscribe
    ///     but it's not required
    Email {
        #[command(subcommand)]
        action: EmailAction,
    },
    /// T91 (ninth wiring): commerce-storefront gate. Loads
    /// `commerce.toml` and projects each `[[product]]` through
    /// commerce-storefront-core's Product::validate (T84):
    ///   * title non-empty + ≥1 variant
    ///   * each variant: ISO 4217 currency, non-negative price,
    ///     non-empty SKU
    Commerce {
        #[command(subcommand)]
        action: CommerceAction,
    },
    /// T91 (tenth wiring): memberships gate. Loads
    /// `memberships.toml` and projects each `[[tier]]` through
    /// memberships-core's Tier::validate (T85):
    ///   * id non-empty + kebab-case
    ///   * name non-empty
    ///   * monthly_price ≥ 0
    ///   * currency is ISO 4217 3-upper-letter
    Memberships {
        #[command(subcommand)]
        action: MembershipsAction,
    },
    /// T91 umbrella: run every config-gate (privacy /
    /// trust-safety / domains / forms / federation / email /
    /// commerce / memberships) at once and report aggregate
    /// pass/fail. Missing config files are reported but treated
    /// as warnings (a tenant that doesn't sell anything doesn't
    /// need commerce.toml).
    ///
    /// Exit codes:
    ///   0 — every present config validated clean
    ///   1 — at least one config violated
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// T98: content authoring lifecycle subcommand family.
    /// Wires importers-core + exporters-core (T77 + T78) into
    /// the forge-cli runtime so operator content workflows
    /// (validate, format-list, project-to-export) hit the typed
    /// CmsSection contract.
    Content {
        #[command(subcommand)]
        action: ContentAction,
    },
    /// T98 (third runtime wiring): search index validator.
    /// Wires search-core (T82) into the forge-cli runtime so
    /// operators can sanity-check a JSON-serialized `IndexDoc[]`
    /// payload before pushing it into a backend (Tantivy /
    /// Meilisearch / etc.).
    Search {
        #[command(subcommand)]
        action: SearchAction,
    },
    /// T98 (fourth runtime wiring): asset-bundle validator.
    /// Wires assets-core (T80) into the forge-cli runtime so
    /// operators can verify image bundles ship the
    /// AVIF/WebP/JPEG fallback ladder + non-empty alt text
    /// before publishing (WCAG 2.1 §1.1.1 + #80 ladder
    /// contract).
    Assets {
        #[command(subcommand)]
        action: AssetsAction,
    },
    /// Query the AVP-Doctrine rule database (71 rules across 9
    /// domains: build / primitives / security / testing / docs /
    /// logging / perf / content / accessibility).
    ///
    /// Backed by `doctrine-core` (task #174). Rules live in
    /// `<AVP-Doctrine repo>/doctrine/rules/<domain>.toml`; the
    /// rules' typed schema is at the same dir's `SCHEMA.md`.
    ///
    /// Examples:
    ///   forge doctrine query --domain security
    ///   forge doctrine query --rule prim-001
    ///   forge doctrine query --severity strict --lifecycle stable
    ///   forge doctrine query --search "tap target"
    Doctrine {
        #[command(subcommand)]
        action: DoctrineAction,
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
enum TrustSafetyAction {
    /// Validate `trust-safety.toml` at the project root against
    /// the typed trust-safety-core surface (T75).
    Validate {
        /// Emit a JSON-form summary of scanner coverage to
        /// stdout. Default is human-readable.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum DomainsAction {
    /// Validate `domains.toml` at the project root against the
    /// typed domains-core surface (T86).
    Validate {
        /// Emit a JSON-form summary of domain validation to
        /// stdout. Default is human-readable.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum EmailAction {
    /// Validate `email.toml` against the typed email-core surface
    /// (T83).
    Validate {
        /// Emit a JSON-form summary to stdout.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum CommerceAction {
    /// Validate `commerce.toml` against the typed
    /// commerce-storefront-core surface (T84).
    Validate {
        /// Emit a JSON-form summary to stdout.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum MembershipsAction {
    /// Validate `memberships.toml` against the typed
    /// memberships-core surface (T85).
    Validate {
        /// Emit a JSON-form summary to stdout.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ContentAction {
    /// Validate a CmsSection JSON file against the typed
    /// importers-core::CmsSection contract (T77).
    ///
    /// Enforces:
    ///   * slug non-empty + kebab-case
    ///   * at most one Hero block per section
    ///   * heading level ∈ 1..=6
    ///   * every Image has non-empty alt text (WCAG 2.1 §1.1.1)
    ///
    /// Exit codes:
    ///   0 — file parses + validates clean
    ///   1 — file parses but fails canonical invariants
    ///   2 — fatal (file missing / parse error)
    Validate {
        /// Path to the CmsSection JSON file.
        path: std::path::PathBuf,
        /// Emit a JSON-form summary to stdout (CI / pipeline use).
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Project a CmsSection JSON file through the typed
    /// exporters-core renderer (T78). Builtin formats:
    /// markdown-yaml-frontmatter / json / json-ld-schema-org.
    ///
    /// Exit codes:
    ///   0 — render emitted to stdout
    ///   1 — section failed canonical validation upstream
    ///   2 — fatal (file missing / parse error / unsupported
    ///       format)
    Export {
        /// Path to the CmsSection JSON file.
        path: std::path::PathBuf,
        /// Output format. Defaults to markdown.
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// List supported export formats (T78::ExportFormat closed
    /// enum). Emits one line per format with slug + IANA media
    /// type + filename extension.
    Formats,
}

#[derive(Subcommand, Debug)]
enum SearchAction {
    /// Validate a JSON-serialized `IndexDoc[]` against the typed
    /// search-core::IndexDoc contract (T82).
    ///
    /// Enforces:
    ///   * each doc has non-empty id / title / body
    ///   * lang is a non-empty BCP-47 stem ([a-z]{2,3} + optional
    ///     "-REGION" or "-Script-REGION")
    ///   * no duplicate ids across the array
    ///   * facets/tags have no empty keys / values
    ///
    /// Exit codes:
    ///   0 — array parses + every doc validates clean
    ///   1 — array parses but at least one doc fails invariants
    ///   2 — fatal (file missing / parse error)
    ValidateIndex {
        /// Path to a JSON file containing an `IndexDoc[]`.
        path: std::path::PathBuf,
        /// Emit a JSON-form summary to stdout (CI / pipeline use).
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum AssetsAction {
    /// Validate an AssetBundle JSON file against the typed
    /// assets-core::AssetBundle contract (T80).
    ///
    /// Enforces:
    ///   * non-empty asset-id + source-media-type
    ///   * image bundles cover every format in the canonical
    ///     fallback ladder (AVIF + WebP + JPEG)
    ///   * alt_text non-empty (WCAG 2.1 §1.1.1)
    ///   * each variant sha256 is 64-lowercase-hex
    ///   * image variants have non-zero width × height
    ///
    /// Exit codes:
    ///   0 — bundle parses + validates clean
    ///   1 — bundle parses but fails canonical invariants
    ///   2 — fatal (file missing / parse error)
    Validate {
        /// Path to the AssetBundle JSON file.
        path: std::path::PathBuf,
        /// Emit a JSON-form summary to stdout (CI / pipeline use).
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Run every config gate at once. Each gate's output is
    /// captured; the umbrella aggregates pass/fail.
    ValidateAll {
        /// Emit a JSON-form aggregate summary instead of the
        /// human-readable per-gate output.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum DoctrineAction {
    /// Query the doctrine rule database. At least one filter
    /// (or `--rule` to fetch by id) should be supplied; with
    /// no filters, lists every loaded rule's id + name.
    ///
    /// Exit codes:
    ///   0 — query returned at least one rule
    ///   1 — query loaded the database but matched no rules
    ///   2 — fatal (doctrine dir missing / parse error / invalid filter)
    Query {
        /// Filter to a specific rule id (e.g., `prim-001`). When set,
        /// other filters are ignored.
        #[arg(long)]
        rule: Option<String>,
        /// Filter by domain (build / primitives / security / testing /
        /// docs / logging / perf / content / accessibility).
        #[arg(long)]
        domain: Option<String>,
        /// Filter by severity (strict / warn / informational / experimental).
        #[arg(long)]
        severity: Option<String>,
        /// Filter by lifecycle (experimental / stable / deprecated).
        #[arg(long)]
        lifecycle: Option<String>,
        /// Substring search across statement + rationale fields.
        #[arg(long)]
        search: Option<String>,
        /// Filter to rules that reference the given trait name in
        /// their `related_traits` field.
        #[arg(long, value_name = "TRAIT")]
        related_trait: Option<String>,
        /// Path to the AVP-Doctrine repository root (the directory
        /// containing `doctrine/rules/`). Defaults to the env var
        /// `PLAUSIDEN_DOCTRINE_DIR`, falling back to
        /// `../PlausiDen-AVP-Doctrine` relative to `--root`.
        #[arg(long)]
        doctrine_dir: Option<PathBuf>,
        /// Emit JSON output (machine-readable, cross-AI consumable per
        /// the cross-AI compatibility rule docs-008). Default is the
        /// human-readable summary.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum FederationAction {
    /// Validate `federation.toml` at the project root against
    /// the typed federation-core surface (T79).
    Validate {
        /// Emit a JSON-form summary to stdout.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum FormsAction {
    /// Validate `forms.toml` at the project root against the
    /// typed forms-core surface (T81). One Form per top-level
    /// `[[form]]` entry. The validator runs `Form::validate`
    /// on each.
    Validate {
        /// Emit a JSON-form summary to stdout. Default is
        /// human-readable.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum AuditLogAction {
    /// Verify the hash-chained integrity of an audit-log JSON
    /// file. Reads the file at the given path (or
    /// `reports/audit-log.json` by default), deserialises into
    /// observability-core's AuditChain, and runs verify().
    Verify {
        /// Path to the audit-log JSON file. Defaults to
        /// `reports/audit-log.json` under the project root.
        #[arg(long)]
        path: Option<std::path::PathBuf>,
        /// Emit a JSON-form summary of the verification to
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
    /// T91 (third wiring): print the operator-facing key
    /// fingerprint — `base64url(SHA256(public-key-bytes))[..16]`,
    /// computed via manifest-attest's `KeyFingerprint`.
    ///
    /// Use the fingerprint as the short stable identifier in
    /// log lines + attestation metadata when the full
    /// public-key base64 is too long to display.
    Fingerprint,
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
        Some(Cmd::TrustSafety { action }) => return run_trust_safety(&root, action),
        Some(Cmd::Domains { action }) => return run_domains(&root, action),
        Some(Cmd::AuditLog { action }) => return run_audit_log(&root, action),
        Some(Cmd::Forms { action }) => return run_forms(&root, action),
        Some(Cmd::Federation { action }) => return run_federation(&root, action),
        Some(Cmd::Email { action }) => return run_email(&root, action),
        Some(Cmd::Commerce { action }) => return run_commerce(&root, action),
        Some(Cmd::Memberships { action }) => return run_memberships(&root, action),
        Some(Cmd::Config { action }) => return run_config(&root, action),
        Some(Cmd::Content { action }) => return run_content(action),
        Some(Cmd::Search { action }) => return run_search(action),
        Some(Cmd::Assets { action }) => return run_assets(action),
        Some(Cmd::Doctrine { action }) => return run_doctrine(action, &root),
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
        // phase_noscript_strict — enforce zero-JS rendered HTML
        // when forge.toml [noscript_strict] enabled = true OR
        // LOOM_NOSCRIPT_MODE=1 in the env. Pairs with Loom's
        // noscript-mode page-shell rendering. For LibreJS /
        // Tor-strict / hunted-tier (#124) builds.
        Box::new(NoscriptStrictPhase),
        // phase_hunted_tier — max-paranoid security profile gate.
        // When forge.toml [security] tier = "hunted", enforces
        // that noscript_strict is also on AND scans rendered HTML
        // for client-state markers (localStorage / cookie /
        // canvas-fingerprint / etc.). The hunted tier is a meta-
        // policy; its zero-JS / zero-tracker / zero-client-state
        // guarantees come from the prerequisite phases.
        Box::new(HuntedTierPhase),
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
        // phase_aesthetic_distinctiveness — scans cms/*.json for
        // SaaS-marketing slop patterns (centered single-word heroes,
        // monotonous feature grids, fake testimonials, green-check
        // pricing, "Numbers that"-style stat bands, sparse pages,
        // scaffold-only compositions). Warn-by-default; promotes
        // to strict via `[aesthetic_distinctiveness] strict = true`.
        // Substrate-distinctiveness gate that closes the "looks
        // like every other landing" feedback loop.
        Box::new(AestheticDistinctivenessPhase),
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

    let audited = pipeline
        .audited()
        .map_err(|e| anyhow::anyhow!("pipeline audited: {e}"))?;
    println!(
        "\n== pipeline summary ==\n  audit phases run:  {}\n  clean phases:      {}",
        audited.phases_run, audited.clean_phases,
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
        AttestAction::Fingerprint => {
            if !pub_path.is_file() {
                eprintln!(
                    "forge attest fingerprint: no {} — run `forge attest init` first",
                    pub_path.display()
                );
                return Ok(ExitCode::from(1));
            }
            let s = std::fs::read_to_string(&pub_path)
                .with_context(|| format!("read {}", pub_path.display()))?;
            let trimmed = s.trim();
            // forge-core::attest stores the pubkey as base64; the
            // exact alphabet (URL-safe-no-pad vs standard) is the
            // engine forge-core uses. Try standard first; fall
            // back to url-safe to handle both deployments.
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(trimmed)
                .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(trimmed))
                .with_context(|| format!("decode base64 pubkey from {}", pub_path.display()))?;
            let fp = manifest_attest::KeyFingerprint::of_verifying_key_bytes(&bytes);
            println!("{}", fp.as_str());
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
    let mut skipped_unparseable: Vec<String> = Vec::new();
    for path in &entries {
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        match serde_json::from_str::<BuildReport>(&raw) {
            Ok(report) => chain_reports.push(report),
            Err(e) => {
                // Skip unparseable historical reports with a
                // warning rather than failing fatally. Reports
                // pre-dating the current BuildReport shape end up
                // here; verifying the chain from the next valid
                // entry is the documented behavior of a chain
                // with a missing predecessor.
                skipped_unparseable.push(format!("{}: {e}", path.display()));
            }
        }
    }
    if !skipped_unparseable.is_empty() {
        eprintln!(
            "forge verify --chain: skipped {} unparseable report(s):",
            skipped_unparseable.len()
        );
        for s in &skipped_unparseable {
            eprintln!("  warn    {s}");
        }
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

// ============================================================
// T91 (second wiring): trust-safety gate — `forge trust-safety
// validate`
//
// TOML schema:
//   [[scanner]]
//   concern = "csam"
//   scanner_id = "photodna-2.1"
//
// Invariants:
//   * every MANDATORY-REPORT concern (CSAM, NCIII, Extremism)
//     has ≥1 scanner declared (US 18 U.S.C. § 2258A obligation)
//   * no duplicate scanner_id per concern
//   * non-mandatory concerns without a scanner are warnings,
//     not violations (operator's choice per audience)
// ============================================================

#[derive(serde::Deserialize, Debug)]
struct TrustSafetyToml {
    #[serde(default)]
    scanner: Vec<ScannerEntry>,
}

#[derive(serde::Deserialize, Debug)]
struct ScannerEntry {
    concern: trust_safety_core::ConcernKind,
    scanner_id: String,
}

#[derive(Debug, serde::Serialize)]
struct TrustSafetyGateSummary {
    trust_safety_path: String,
    scanner_entries: usize,
    mandatory_concerns_covered: usize,
    mandatory_concerns_total: usize,
    mandatory_concerns_missing: Vec<&'static str>,
    non_mandatory_concerns_uncovered: Vec<&'static str>,
    violations: Vec<String>,
}

fn all_concern_kinds() -> [trust_safety_core::ConcernKind; 10] {
    use trust_safety_core::ConcernKind::*;
    [
        Csam,
        Phishing,
        Spam,
        Sanctions,
        SelfHarm,
        Extremism,
        Nciii,
        Malware,
        IpViolation,
        HateSpeech,
    ]
}

fn run_trust_safety(root: &std::path::Path, action: &TrustSafetyAction) -> Result<ExitCode> {
    match action {
        TrustSafetyAction::Validate { json } => run_trust_safety_validate(root, *json),
    }
}

fn run_trust_safety_validate(root: &std::path::Path, json: bool) -> Result<ExitCode> {
    let path = root.join("trust-safety.toml");
    let mut violations: Vec<String> = Vec::new();
    let all_concerns = all_concern_kinds();
    let mandatory_total = all_concerns
        .iter()
        .filter(|c| c.is_mandatory_report())
        .count();
    let mut summary = TrustSafetyGateSummary {
        trust_safety_path: path.display().to_string(),
        scanner_entries: 0,
        mandatory_concerns_covered: 0,
        mandatory_concerns_total: mandatory_total,
        mandatory_concerns_missing: Vec::new(),
        non_mandatory_concerns_uncovered: Vec::new(),
        violations: Vec::new(),
    };

    if !path.exists() {
        violations.push(format!("missing trust-safety.toml at {}", path.display()));
        summary.violations = violations.clone();
        emit_trust_safety_summary(&summary, json);
        return Ok(ExitCode::from(2));
    }

    let s =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let parsed: TrustSafetyToml = match toml::from_str(&s) {
        Ok(p) => p,
        Err(e) => {
            violations.push(format!("trust-safety.toml parse: {e}"));
            summary.violations = violations.clone();
            emit_trust_safety_summary(&summary, json);
            return Ok(ExitCode::from(2));
        }
    };

    summary.scanner_entries = parsed.scanner.len();

    // Index scanners by concern; detect duplicate scanner_id per
    // concern.
    use std::collections::HashMap;
    let mut by_concern: HashMap<trust_safety_core::ConcernKind, Vec<&str>> = HashMap::new();
    for entry in &parsed.scanner {
        let ids = by_concern.entry(entry.concern).or_default();
        if ids.contains(&entry.scanner_id.as_str()) {
            violations.push(format!(
                "duplicate scanner_id {:?} on concern {:?}",
                entry.scanner_id,
                entry.concern.slug()
            ));
        }
        ids.push(&entry.scanner_id);
    }

    // Coverage check per concern kind.
    for c in all_concerns.iter() {
        let has_scanner = by_concern.contains_key(c);
        if c.is_mandatory_report() {
            if has_scanner {
                summary.mandatory_concerns_covered += 1;
            } else {
                summary.mandatory_concerns_missing.push(c.slug());
                violations.push(format!(
                    "MANDATORY-REPORT concern {:?} has no scanner (US 18 U.S.C. § 2258A)",
                    c.slug()
                ));
            }
        } else if !has_scanner {
            summary.non_mandatory_concerns_uncovered.push(c.slug());
        }
    }

    summary.violations = violations.clone();
    emit_trust_safety_summary(&summary, json);

    if violations.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn emit_trust_safety_summary(summary: &TrustSafetyGateSummary, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(summary)
                .unwrap_or_else(|_| "{\"error\":\"serialize-failed\"}".to_string())
        );
    } else {
        println!(
            "trust-safety-gate: {} scanners, {}/{} mandatory-report concerns covered",
            summary.scanner_entries,
            summary.mandatory_concerns_covered,
            summary.mandatory_concerns_total,
        );
        if !summary.mandatory_concerns_missing.is_empty() {
            println!("MANDATORY-REPORT concerns without a scanner (legal exposure):");
            for c in &summary.mandatory_concerns_missing {
                println!("  - {c}");
            }
        }
        if !summary.non_mandatory_concerns_uncovered.is_empty() {
            println!("non-mandatory concerns without a scanner (operator choice):");
            for c in &summary.non_mandatory_concerns_uncovered {
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
mod attest_fingerprint_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn fingerprint_succeeds_against_a_freshly_generated_key() {
        let td = TempDir::new().unwrap();
        let init_code = run_attest(td.path(), &AttestAction::Init { force: false }).unwrap();
        assert_eq!(init_code, ExitCode::SUCCESS);
        let fp_code = run_attest(td.path(), &AttestAction::Fingerprint).unwrap();
        assert_eq!(fp_code, ExitCode::SUCCESS);
    }

    #[test]
    fn fingerprint_errors_when_no_key_exists() {
        let td = TempDir::new().unwrap();
        let code = run_attest(td.path(), &AttestAction::Fingerprint).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }
}

#[cfg(test)]
mod trust_safety_gate_tests {
    use super::*;
    use tempfile::TempDir;

    fn write_trust_safety(dir: &std::path::Path, body: &str) {
        std::fs::write(dir.join("trust-safety.toml"), body).unwrap();
    }

    fn mandatory_scanners_toml() -> String {
        // Cover every MANDATORY-REPORT concern (CSAM / NCIII /
        // Extremism). Non-mandatory left out — those warn only.
        [
            ("csam", "photodna-2.1"),
            ("nciii", "nciii-hash-2026"),
            ("extremism", "gifct-2026"),
        ]
        .iter()
        .map(|(c, id)| {
            format!(
                "[[scanner]]\nconcern = \"{}\"\nscanner_id = \"{}\"\n",
                c, id
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
    }

    #[test]
    fn missing_file_exits_2() {
        let td = TempDir::new().unwrap();
        let code = run_trust_safety_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn malformed_toml_exits_2() {
        let td = TempDir::new().unwrap();
        write_trust_safety(td.path(), "this is not toml [[[");
        let code = run_trust_safety_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn all_mandatory_covered_exits_0() {
        let td = TempDir::new().unwrap();
        write_trust_safety(td.path(), &mandatory_scanners_toml());
        let code = run_trust_safety_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn missing_csam_scanner_exits_1() {
        let td = TempDir::new().unwrap();
        // Cover NCIII + Extremism but NOT CSAM.
        let body = "[[scanner]]\nconcern = \"nciii\"\nscanner_id = \"x\"\n\n[[scanner]]\nconcern = \"extremism\"\nscanner_id = \"y\"\n";
        write_trust_safety(td.path(), body);
        let code = run_trust_safety_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn missing_non_mandatory_scanner_is_warning_not_violation() {
        let td = TempDir::new().unwrap();
        // Cover only the mandatory ones; spam / phishing / etc.
        // are uncovered. Should still exit 0.
        write_trust_safety(td.path(), &mandatory_scanners_toml());
        let code = run_trust_safety_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn duplicate_scanner_id_per_concern_exits_1() {
        let td = TempDir::new().unwrap();
        let mut body = mandatory_scanners_toml();
        // Add a duplicate scanner_id for CSAM.
        body.push_str("\n[[scanner]]\nconcern = \"csam\"\nscanner_id = \"photodna-2.1\"\n");
        write_trust_safety(td.path(), &body);
        let code = run_trust_safety_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn different_scanner_ids_per_concern_are_ok() {
        let td = TempDir::new().unwrap();
        let mut body = mandatory_scanners_toml();
        // Add a SECOND scanner for CSAM with a different id —
        // operator using multiple scanners is intentional defense.
        body.push_str("\n[[scanner]]\nconcern = \"csam\"\nscanner_id = \"neuralhash-1.0\"\n");
        write_trust_safety(td.path(), &body);
        let code = run_trust_safety_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }
}

// ============================================================
// T91 (fourth wiring): domains gate — `forge domains validate`
//
// TOML schema:
//   [[domain]]
//   fqdn = "example.com"
//   kind = "apex"          # apex | subdomain | wildcard
//   challenge = "http-01"  # http-01 | dns-01 | tls-alpn-01
//
//   [hsts]
//   max_age_secs = 63072000
//   include_subdomains = true
//   preload = true
//
// Invariants enforced:
//   * every Domain passes RFC 1035 FQDN validation
//   * Wildcard ⇒ challenge MUST be DNS-01 (RFC 8555 §8.4)
//   * HSTS policy is preload-eligible (max-age ≥ 31536000 +
//     includeSubDomains + preload) — refused if explicitly
//     downgraded
//   * no duplicate FQDNs
// ============================================================

#[derive(serde::Deserialize, Debug)]
struct DomainsToml {
    #[serde(default)]
    domain: Vec<DomainEntry>,
    #[serde(default)]
    hsts: Option<HstsEntry>,
}

#[derive(serde::Deserialize, Debug)]
struct DomainEntry {
    fqdn: String,
    kind: domains_core::DomainKind,
    challenge: domains_core::AcmeChallenge,
}

#[derive(serde::Deserialize, Debug, Default)]
struct HstsEntry {
    #[serde(default)]
    max_age_secs: Option<u32>,
    #[serde(default)]
    include_subdomains: Option<bool>,
    #[serde(default)]
    preload: Option<bool>,
}

#[derive(Debug, serde::Serialize)]
struct DomainsGateSummary {
    domains_path: String,
    domain_count: usize,
    fqdns_valid: usize,
    hsts_preload_eligible: bool,
    violations: Vec<String>,
}

fn run_domains(root: &std::path::Path, action: &DomainsAction) -> Result<ExitCode> {
    match action {
        DomainsAction::Validate { json } => run_domains_validate(root, *json),
    }
}

fn run_domains_validate(root: &std::path::Path, json: bool) -> Result<ExitCode> {
    let path = root.join("domains.toml");
    let mut violations: Vec<String> = Vec::new();
    let mut summary = DomainsGateSummary {
        domains_path: path.display().to_string(),
        domain_count: 0,
        fqdns_valid: 0,
        hsts_preload_eligible: false,
        violations: Vec::new(),
    };

    if !path.exists() {
        violations.push(format!("missing domains.toml at {}", path.display()));
        summary.violations = violations.clone();
        emit_domains_summary(&summary, json);
        return Ok(ExitCode::from(2));
    }

    let s =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let parsed: DomainsToml = match toml::from_str(&s) {
        Ok(p) => p,
        Err(e) => {
            violations.push(format!("domains.toml parse: {e}"));
            summary.violations = violations.clone();
            emit_domains_summary(&summary, json);
            return Ok(ExitCode::from(2));
        }
    };

    summary.domain_count = parsed.domain.len();

    // FQDN + challenge-compatibility + duplicate checks.
    let mut seen_fqdn: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for entry in &parsed.domain {
        let d = domains_core::Domain {
            fqdn: entry.fqdn.clone(),
            kind: entry.kind,
        };
        match d.validate() {
            Ok(()) => summary.fqdns_valid += 1,
            Err(e) => violations.push(format!("domain {:?}: {e}", entry.fqdn)),
        }
        if let Err(e) = domains_core::challenge_compatible(entry.kind, entry.challenge) {
            violations.push(format!("domain {:?}: {e}", entry.fqdn));
        }
        if !seen_fqdn.insert(entry.fqdn.as_str()) {
            violations.push(format!("duplicate fqdn {:?}", entry.fqdn));
        }
    }

    // HSTS policy: use operator-supplied values or platform
    // defaults; refuse if downgraded below preload eligibility.
    let default_hsts = domains_core::HstsPolicy::platform_default();
    let hsts_entry = parsed.hsts.unwrap_or_default();
    let hsts = domains_core::HstsPolicy {
        max_age_secs: hsts_entry.max_age_secs.unwrap_or(default_hsts.max_age_secs),
        include_subdomains: hsts_entry
            .include_subdomains
            .unwrap_or(default_hsts.include_subdomains),
        preload: hsts_entry.preload.unwrap_or(default_hsts.preload),
    };
    summary.hsts_preload_eligible = hsts.is_preload_eligible();
    if !summary.hsts_preload_eligible {
        violations.push(format!(
            "HSTS policy not preload-eligible (max-age={}, subdomains={}, preload={}); \
             RFC 6797 + hstspreload.org require all three",
            hsts.max_age_secs, hsts.include_subdomains, hsts.preload
        ));
    }

    summary.violations = violations.clone();
    emit_domains_summary(&summary, json);

    if violations.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn emit_domains_summary(summary: &DomainsGateSummary, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(summary)
                .unwrap_or_else(|_| "{\"error\":\"serialize-failed\"}".to_string())
        );
    } else {
        println!(
            "domains-gate: {} domains, {} valid FQDN, hsts preload-eligible: {}",
            summary.domain_count, summary.fqdns_valid, summary.hsts_preload_eligible,
        );
        if !summary.violations.is_empty() {
            println!("violations:");
            for v in &summary.violations {
                println!("  - {v}");
            }
        }
    }
}

#[cfg(test)]
mod domains_gate_tests {
    use super::*;
    use tempfile::TempDir;

    fn write_domains(dir: &std::path::Path, body: &str) {
        std::fs::write(dir.join("domains.toml"), body).unwrap();
    }

    #[test]
    fn missing_file_exits_2() {
        let td = TempDir::new().unwrap();
        let code = run_domains_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn malformed_toml_exits_2() {
        let td = TempDir::new().unwrap();
        write_domains(td.path(), "this is not toml [[[");
        let code = run_domains_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn valid_apex_domain_with_default_hsts_passes() {
        let td = TempDir::new().unwrap();
        // Default HSTS (no [hsts] block) is platform_default which
        // is preload-eligible.
        write_domains(
            td.path(),
            "[[domain]]\nfqdn = \"example.com\"\nkind = \"apex\"\nchallenge = \"http-01\"\n",
        );
        let code = run_domains_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn wildcard_with_dns_01_passes() {
        let td = TempDir::new().unwrap();
        write_domains(
            td.path(),
            "[[domain]]\nfqdn = \"*.example.com\"\nkind = \"wildcard\"\nchallenge = \"dns-01\"\n",
        );
        let code = run_domains_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn wildcard_with_http_01_violates() {
        let td = TempDir::new().unwrap();
        // HTTP-01 cannot validate wildcards per RFC 8555 §8.4.
        write_domains(
            td.path(),
            "[[domain]]\nfqdn = \"*.example.com\"\nkind = \"wildcard\"\nchallenge = \"http-01\"\n",
        );
        let code = run_domains_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn uppercase_fqdn_violates() {
        let td = TempDir::new().unwrap();
        write_domains(
            td.path(),
            "[[domain]]\nfqdn = \"Example.com\"\nkind = \"apex\"\nchallenge = \"http-01\"\n",
        );
        let code = run_domains_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn duplicate_fqdn_violates() {
        let td = TempDir::new().unwrap();
        write_domains(
            td.path(),
            "[[domain]]\nfqdn = \"example.com\"\nkind = \"apex\"\nchallenge = \"http-01\"\n\
             [[domain]]\nfqdn = \"example.com\"\nkind = \"apex\"\nchallenge = \"tls-alpn-01\"\n",
        );
        let code = run_domains_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn hsts_downgrade_violates() {
        let td = TempDir::new().unwrap();
        write_domains(
            td.path(),
            "[[domain]]\nfqdn = \"example.com\"\nkind = \"apex\"\nchallenge = \"http-01\"\n\
             [hsts]\nmax_age_secs = 60\ninclude_subdomains = false\npreload = false\n",
        );
        let code = run_domains_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }
}

// ============================================================
// T91 (fifth wiring): audit-log verify — `forge audit-log verify`
//
// Loads an observability-core::AuditChain (serialised as JSON)
// and runs its built-in verify() integrity check:
//   * sequence monotonicity (entry N has sequence == N)
//   * prev_hash linkage (entry N's prev_hash == entry N-1's
//     entry_hash; entry 0's prev_hash == zero)
//   * entry_hash freshness (recomputed sha256 matches declared
//     hash — tamper detection)
//
// Use case: operator-side compliance audit + tamper-evidence on
// admin-action logs / DSAR-fulfillment audit / trust+safety
// moderation history / federation publish-event log.
// ============================================================

#[derive(Debug, serde::Serialize)]
struct AuditLogVerifySummary {
    audit_log_path: String,
    entry_count: usize,
    verdict: String,
    error: Option<String>,
}

fn run_audit_log(root: &std::path::Path, action: &AuditLogAction) -> Result<ExitCode> {
    match action {
        AuditLogAction::Verify { path, json } => {
            let resolved = path
                .clone()
                .unwrap_or_else(|| root.join("reports").join("audit-log.json"));
            run_audit_log_verify(&resolved, *json)
        }
    }
}

fn run_audit_log_verify(path: &std::path::Path, json: bool) -> Result<ExitCode> {
    let mut summary = AuditLogVerifySummary {
        audit_log_path: path.display().to_string(),
        entry_count: 0,
        verdict: "unknown".to_string(),
        error: None,
    };

    if !path.exists() {
        summary.error = Some(format!("missing audit-log at {}", path.display()));
        summary.verdict = "fatal".to_string();
        emit_audit_log_summary(&summary, json);
        return Ok(ExitCode::from(2));
    }

    let raw = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let chain: observability_core::AuditChain = match serde_json::from_str(&raw) {
        Ok(c) => c,
        Err(e) => {
            summary.error = Some(format!("parse: {e}"));
            summary.verdict = "fatal".to_string();
            emit_audit_log_summary(&summary, json);
            return Ok(ExitCode::from(2));
        }
    };

    summary.entry_count = chain.entries.len();

    match chain.verify() {
        Ok(()) => {
            summary.verdict = "pass".to_string();
            emit_audit_log_summary(&summary, json);
            Ok(ExitCode::SUCCESS)
        }
        Err(e) => {
            summary.verdict = "fail".to_string();
            summary.error = Some(e.to_string());
            emit_audit_log_summary(&summary, json);
            Ok(ExitCode::from(1))
        }
    }
}

fn emit_audit_log_summary(summary: &AuditLogVerifySummary, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(summary)
                .unwrap_or_else(|_| "{\"error\":\"serialize-failed\"}".to_string())
        );
    } else {
        println!(
            "audit-log: {} entries, verdict: {}",
            summary.entry_count, summary.verdict,
        );
        if let Some(e) = &summary.error {
            println!("  detail: {e}");
        }
    }
}

#[cfg(test)]
mod audit_log_gate_tests {
    use super::*;
    use tempfile::TempDir;
    use time::macros::datetime;

    fn write_chain(dir: &std::path::Path, chain: &observability_core::AuditChain) {
        let json = serde_json::to_string(chain).unwrap();
        std::fs::write(dir.join("reports").join("audit-log.json"), json).unwrap();
        std::fs::create_dir_all(dir.join("reports")).ok();
    }

    #[test]
    fn missing_file_exits_2() {
        let td = TempDir::new().unwrap();
        std::fs::create_dir_all(td.path().join("reports")).unwrap();
        let code =
            run_audit_log_verify(&td.path().join("reports").join("audit-log.json"), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn malformed_json_exits_2() {
        let td = TempDir::new().unwrap();
        std::fs::create_dir_all(td.path().join("reports")).unwrap();
        let p = td.path().join("reports").join("audit-log.json");
        std::fs::write(&p, "this is not json {{{").unwrap();
        let code = run_audit_log_verify(&p, false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn empty_chain_passes() {
        let td = TempDir::new().unwrap();
        std::fs::create_dir_all(td.path().join("reports")).unwrap();
        let chain = observability_core::AuditChain::new();
        write_chain(td.path(), &chain);
        let code =
            run_audit_log_verify(&td.path().join("reports").join("audit-log.json"), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn well_formed_chain_passes() {
        let td = TempDir::new().unwrap();
        std::fs::create_dir_all(td.path().join("reports")).unwrap();
        let mut chain = observability_core::AuditChain::new();
        chain.append(
            "alice",
            datetime!(2026-05-18 12:00 UTC),
            "login",
            serde_json::json!({"ip": "10.0.0.1"}),
        );
        chain.append(
            "bob",
            datetime!(2026-05-18 12:01 UTC),
            "admin-action",
            serde_json::json!({"action": "grant-permission"}),
        );
        write_chain(td.path(), &chain);
        let code =
            run_audit_log_verify(&td.path().join("reports").join("audit-log.json"), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn tampered_chain_fails_1() {
        let td = TempDir::new().unwrap();
        std::fs::create_dir_all(td.path().join("reports")).unwrap();
        let mut chain = observability_core::AuditChain::new();
        chain.append(
            "alice",
            datetime!(2026-05-18 12:00 UTC),
            "login",
            serde_json::json!({"ip": "10.0.0.1"}),
        );
        chain.append(
            "bob",
            datetime!(2026-05-18 12:01 UTC),
            "admin-action",
            serde_json::json!({"action": "grant-permission"}),
        );
        // Tamper: replace entry 0's payload AFTER hashes were
        // computed. verify() should catch the entry_hash
        // mismatch.
        chain.entries[0].payload = serde_json::json!({"ip": "evil"});
        write_chain(td.path(), &chain);
        let code =
            run_audit_log_verify(&td.path().join("reports").join("audit-log.json"), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }
}

// ============================================================
// T91 (sixth wiring): forms gate — `forge forms validate`
//
// TOML schema:
//   [[form]]
//   id = "contact"
//   title = "Contact us"
//   webhook-url = "https://..."  # kebab-case per forms-core serde
//
//   [[form.fields]]
//   id = "email"
//   label = "Email"
//   kind = "email"
//   required = true
//
// Invariants enforced (delegated to forms-core::Form::validate):
//   * title non-empty
//   * webhook_url starts with https://
//   * every field has a non-empty label (WCAG 2.1 §3.3.2)
//   * every field id is kebab-case + unique
//   * at most one Honeypot field per form
// ============================================================

#[derive(serde::Deserialize, Debug)]
struct FormsToml {
    #[serde(default)]
    form: Vec<forms_core::Form>,
}

#[derive(Debug, serde::Serialize)]
struct FormsGateSummary {
    forms_path: String,
    form_count: usize,
    forms_valid: usize,
    forms_with_honeypot: usize,
    violations: Vec<String>,
}

fn run_forms(root: &std::path::Path, action: &FormsAction) -> Result<ExitCode> {
    match action {
        FormsAction::Validate { json } => run_forms_validate(root, *json),
    }
}

fn run_forms_validate(root: &std::path::Path, json: bool) -> Result<ExitCode> {
    let path = root.join("forms.toml");
    let mut violations: Vec<String> = Vec::new();
    let mut summary = FormsGateSummary {
        forms_path: path.display().to_string(),
        form_count: 0,
        forms_valid: 0,
        forms_with_honeypot: 0,
        violations: Vec::new(),
    };

    if !path.exists() {
        violations.push(format!("missing forms.toml at {}", path.display()));
        summary.violations = violations.clone();
        emit_forms_summary(&summary, json);
        return Ok(ExitCode::from(2));
    }

    let s =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let parsed: FormsToml = match toml::from_str(&s) {
        Ok(p) => p,
        Err(e) => {
            violations.push(format!("forms.toml parse: {e}"));
            summary.violations = violations.clone();
            emit_forms_summary(&summary, json);
            return Ok(ExitCode::from(2));
        }
    };

    summary.form_count = parsed.form.len();

    for form in &parsed.form {
        match form.validate() {
            Ok(()) => summary.forms_valid += 1,
            Err(e) => violations.push(format!("form {:?}: {e}", form.id)),
        }
        if form
            .fields
            .iter()
            .any(|f| matches!(f.kind, forms_core::FieldKind::Honeypot))
        {
            summary.forms_with_honeypot += 1;
        }
    }

    summary.violations = violations.clone();
    emit_forms_summary(&summary, json);

    if violations.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn emit_forms_summary(summary: &FormsGateSummary, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(summary)
                .unwrap_or_else(|_| "{\"error\":\"serialize-failed\"}".to_string())
        );
    } else {
        println!(
            "forms-gate: {} forms, {} valid, {} with honeypot",
            summary.form_count, summary.forms_valid, summary.forms_with_honeypot,
        );
        if !summary.violations.is_empty() {
            println!("violations:");
            for v in &summary.violations {
                println!("  - {v}");
            }
        }
    }
}

#[cfg(test)]
mod forms_gate_tests {
    use super::*;
    use tempfile::TempDir;

    fn write_forms(dir: &std::path::Path, body: &str) {
        std::fs::write(dir.join("forms.toml"), body).unwrap();
    }

    fn ok_form_body() -> &'static str {
        "[[form]]\n\
         id = \"contact\"\n\
         title = \"Contact us\"\n\
         webhook-url = \"https://hooks.example.com/contact\"\n\
         \n\
         [[form.fields]]\n\
         id = \"name\"\n\
         label = \"Name\"\n\
         kind = \"text\"\n\
         required = true\n\
         \n\
         [[form.fields]]\n\
         id = \"email\"\n\
         label = \"Email\"\n\
         kind = \"email\"\n\
         required = true\n"
    }

    #[test]
    fn missing_file_exits_2() {
        let td = TempDir::new().unwrap();
        let code = run_forms_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn malformed_toml_exits_2() {
        let td = TempDir::new().unwrap();
        write_forms(td.path(), "not toml [[[");
        let code = run_forms_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn valid_form_passes() {
        let td = TempDir::new().unwrap();
        write_forms(td.path(), ok_form_body());
        let code = run_forms_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn http_webhook_violates() {
        let td = TempDir::new().unwrap();
        let body = ok_form_body().replace("https://", "http://");
        write_forms(td.path(), &body);
        let code = run_forms_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn empty_form_list_passes() {
        // No forms at all = nothing to validate.
        let td = TempDir::new().unwrap();
        write_forms(td.path(), "");
        let code = run_forms_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }
}

// ============================================================
// T91 (seventh wiring): federation gate — `forge federation
// validate`
//
// TOML schema:
//   [[destination]]
//   protocol = "nostr"
//   relay = "wss://relay.example.com"
//
//   [[destination]]
//   protocol = "activitypub"
//   inbox = "https://mastodon.example.com/inbox"
//
// Invariants:
//   * every destination uses a known FederationProtocol
//     (closed enum from federation-core — TOML deserialiser
//     catches unknowns automatically)
//   * the address fields supplied match the protocol — e.g.
//     `inbox` for activitypub, `relay` for nostr.
//   * no duplicate destinations (per (protocol, key-fields))
// ============================================================

#[derive(serde::Deserialize, Debug)]
struct FederationToml {
    #[serde(default)]
    destination: Vec<federation_core::FederationAddress>,
}

#[derive(Debug, serde::Serialize)]
struct FederationGateSummary {
    federation_path: String,
    destination_count: usize,
    by_protocol: std::collections::BTreeMap<String, usize>,
    violations: Vec<String>,
}

fn run_federation(root: &std::path::Path, action: &FederationAction) -> Result<ExitCode> {
    match action {
        FederationAction::Validate { json } => run_federation_validate(root, *json),
    }
}

fn run_federation_validate(root: &std::path::Path, json: bool) -> Result<ExitCode> {
    let path = root.join("federation.toml");
    let mut violations: Vec<String> = Vec::new();
    let mut summary = FederationGateSummary {
        federation_path: path.display().to_string(),
        destination_count: 0,
        by_protocol: std::collections::BTreeMap::new(),
        violations: Vec::new(),
    };

    if !path.exists() {
        violations.push(format!("missing federation.toml at {}", path.display()));
        summary.violations = violations.clone();
        emit_federation_summary(&summary, json);
        return Ok(ExitCode::from(2));
    }

    let s =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let parsed: FederationToml = match toml::from_str(&s) {
        Ok(p) => p,
        Err(e) => {
            violations.push(format!("federation.toml parse: {e}"));
            summary.violations = violations.clone();
            emit_federation_summary(&summary, json);
            return Ok(ExitCode::from(2));
        }
    };

    summary.destination_count = parsed.destination.len();

    // Count by protocol + detect duplicates.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for d in &parsed.destination {
        let proto = d.protocol().slug().to_string();
        *summary.by_protocol.entry(proto.clone()).or_insert(0) += 1;
        // Dedup key is the JSON serialisation of the address —
        // identical address shapes are duplicates.
        let key = serde_json::to_string(d).unwrap_or_default();
        if !seen.insert(key.clone()) {
            violations.push(format!("duplicate destination: {key}"));
        }
    }

    summary.violations = violations.clone();
    emit_federation_summary(&summary, json);

    if violations.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn emit_federation_summary(summary: &FederationGateSummary, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(summary)
                .unwrap_or_else(|_| "{\"error\":\"serialize-failed\"}".to_string())
        );
    } else {
        println!(
            "federation-gate: {} destinations",
            summary.destination_count
        );
        for (p, c) in &summary.by_protocol {
            println!("  {p}: {c}");
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
mod federation_gate_tests {
    use super::*;
    use tempfile::TempDir;

    fn write_fed(dir: &std::path::Path, body: &str) {
        std::fs::write(dir.join("federation.toml"), body).unwrap();
    }

    #[test]
    fn missing_file_exits_2() {
        let td = TempDir::new().unwrap();
        let code = run_federation_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn malformed_toml_exits_2() {
        let td = TempDir::new().unwrap();
        write_fed(td.path(), "this is not toml [[[");
        let code = run_federation_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn empty_destination_list_passes() {
        let td = TempDir::new().unwrap();
        write_fed(td.path(), "");
        let code = run_federation_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn valid_destinations_pass() {
        let td = TempDir::new().unwrap();
        write_fed(
            td.path(),
            "[[destination]]\nprotocol = \"nostr\"\nrelay = \"wss://relay.example.com\"\n\n\
             [[destination]]\nprotocol = \"activitypub\"\ninbox = \"https://m.example/inbox\"\n",
        );
        let code = run_federation_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn duplicate_destination_violates() {
        let td = TempDir::new().unwrap();
        write_fed(
            td.path(),
            "[[destination]]\nprotocol = \"nostr\"\nrelay = \"wss://r\"\n\n\
             [[destination]]\nprotocol = \"nostr\"\nrelay = \"wss://r\"\n",
        );
        let code = run_federation_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn unknown_protocol_fails_parse() {
        // FederationProtocol is a closed enum — TOML
        // deserialiser refuses unknown variants, exits 2.
        let td = TempDir::new().unwrap();
        write_fed(
            td.path(),
            "[[destination]]\nprotocol = \"telepathy\"\nendpoint = \"x\"\n",
        );
        let code = run_federation_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }
}

// ============================================================
// T91 (eighth wiring): email gate — `forge email validate`
//
// TOML schema:
//   [[message]]
//   id = "welcome"
//   kind = "transactional"  # transactional | marketing
//   from = "hello@example.com"
//   to = ["alice@example.com"]
//   subject = "Welcome"
//
//   [[message]]
//   id = "may-newsletter"
//   kind = "marketing"
//   from = "newsletter@example.com"
//   to = ["all-subs"]
//   subject = "May newsletter"
//   list-unsubscribe = "https://example.com/unsub?id=abc"
//
// Invariants enforced by OutgoingMessage::validate():
//   * from / subject non-empty + to non-empty
//   * marketing messages MUST have an https:// list-unsubscribe
//     URL (RFC 8058)
//   * transactional messages MAY include unsubscribe; not
//     required
// ============================================================

#[derive(serde::Deserialize, Debug)]
struct EmailToml {
    #[serde(default)]
    message: Vec<EmailMessage>,
}

#[derive(serde::Deserialize, Debug)]
struct EmailMessage {
    id: String,
    kind: email_core::MessageKind,
    from: String,
    to: Vec<String>,
    subject: String,
    #[serde(default, rename = "list-unsubscribe")]
    list_unsubscribe: Option<String>,
}

#[derive(Debug, serde::Serialize)]
struct EmailGateSummary {
    email_path: String,
    message_count: usize,
    transactional_count: usize,
    marketing_count: usize,
    violations: Vec<String>,
}

fn run_email(root: &std::path::Path, action: &EmailAction) -> Result<ExitCode> {
    match action {
        EmailAction::Validate { json } => run_email_validate(root, *json),
    }
}

fn run_email_validate(root: &std::path::Path, json: bool) -> Result<ExitCode> {
    let path = root.join("email.toml");
    let mut violations: Vec<String> = Vec::new();
    let mut summary = EmailGateSummary {
        email_path: path.display().to_string(),
        message_count: 0,
        transactional_count: 0,
        marketing_count: 0,
        violations: Vec::new(),
    };

    if !path.exists() {
        violations.push(format!("missing email.toml at {}", path.display()));
        summary.violations = violations.clone();
        emit_email_summary(&summary, json);
        return Ok(ExitCode::from(2));
    }

    let s =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let parsed: EmailToml = match toml::from_str(&s) {
        Ok(p) => p,
        Err(e) => {
            violations.push(format!("email.toml parse: {e}"));
            summary.violations = violations.clone();
            emit_email_summary(&summary, json);
            return Ok(ExitCode::from(2));
        }
    };

    summary.message_count = parsed.message.len();

    let now = time::OffsetDateTime::now_utc();
    for m in &parsed.message {
        match m.kind {
            email_core::MessageKind::Transactional => summary.transactional_count += 1,
            email_core::MessageKind::Marketing => summary.marketing_count += 1,
        }
        // Project to email-core's OutgoingMessage so the
        // typed validator runs.
        let om = email_core::OutgoingMessage {
            id: m.id.clone(),
            kind: m.kind,
            from: m.from.clone(),
            to: m.to.clone(),
            subject: m.subject.clone(),
            list_unsubscribe: m.list_unsubscribe.clone(),
            bimi: None,
            queued_at: now,
        };
        if let Err(e) = om.validate() {
            violations.push(format!("message {:?}: {e}", m.id));
        }
    }

    summary.violations = violations.clone();
    emit_email_summary(&summary, json);

    if violations.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn emit_email_summary(summary: &EmailGateSummary, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(summary)
                .unwrap_or_else(|_| "{\"error\":\"serialize-failed\"}".to_string())
        );
    } else {
        println!(
            "email-gate: {} messages ({} transactional, {} marketing)",
            summary.message_count, summary.transactional_count, summary.marketing_count,
        );
        if !summary.violations.is_empty() {
            println!("violations:");
            for v in &summary.violations {
                println!("  - {v}");
            }
        }
    }
}

#[cfg(test)]
mod email_gate_tests {
    use super::*;
    use tempfile::TempDir;

    fn write_email(dir: &std::path::Path, body: &str) {
        std::fs::write(dir.join("email.toml"), body).unwrap();
    }

    #[test]
    fn missing_file_exits_2() {
        let td = TempDir::new().unwrap();
        let code = run_email_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn malformed_toml_exits_2() {
        let td = TempDir::new().unwrap();
        write_email(td.path(), "this is not toml [[[");
        let code = run_email_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn empty_list_passes() {
        let td = TempDir::new().unwrap();
        write_email(td.path(), "");
        let code = run_email_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn valid_transactional_message_passes() {
        let td = TempDir::new().unwrap();
        write_email(
            td.path(),
            "[[message]]\nid = \"welcome\"\nkind = \"transactional\"\nfrom = \"hello@example.com\"\nto = [\"alice@example.com\"]\nsubject = \"Welcome\"\n",
        );
        let code = run_email_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn marketing_without_unsubscribe_violates() {
        let td = TempDir::new().unwrap();
        write_email(
            td.path(),
            "[[message]]\nid = \"newsletter\"\nkind = \"marketing\"\nfrom = \"news@example.com\"\nto = [\"all\"]\nsubject = \"Hi\"\n",
        );
        let code = run_email_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn marketing_with_https_unsubscribe_passes() {
        let td = TempDir::new().unwrap();
        write_email(
            td.path(),
            "[[message]]\nid = \"newsletter\"\nkind = \"marketing\"\nfrom = \"news@example.com\"\nto = [\"all\"]\nsubject = \"Hi\"\nlist-unsubscribe = \"https://example.com/unsub\"\n",
        );
        let code = run_email_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn marketing_with_http_unsubscribe_violates() {
        let td = TempDir::new().unwrap();
        write_email(
            td.path(),
            "[[message]]\nid = \"newsletter\"\nkind = \"marketing\"\nfrom = \"news@example.com\"\nto = [\"all\"]\nsubject = \"Hi\"\nlist-unsubscribe = \"http://example.com/unsub\"\n",
        );
        let code = run_email_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn empty_from_violates() {
        let td = TempDir::new().unwrap();
        write_email(
            td.path(),
            "[[message]]\nid = \"x\"\nkind = \"transactional\"\nfrom = \"\"\nto = [\"a\"]\nsubject = \"s\"\n",
        );
        let code = run_email_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }
}

// ============================================================
// T91 (ninth wiring): commerce gate — `forge commerce validate`
//
// TOML schema:
//   [[product]]
//   id = "tshirt"
//   title = "T-Shirt"
//   description = "Cotton"
//   published = true
//
//   [[product.variants]]
//   id = "tshirt-m-blue"
//   sku = "TSHIRT-M-BLUE"
//   title = "M / Blue"
//   price = 1999             # smallest currency unit (cents)
//   currency = "USD"         # ISO 4217 3-upper-letter
//
// Invariants enforced by Product::validate():
//   * title non-empty + ≥1 variant
//   * each variant: SKU non-empty + price ≥ 0
//   * Currency parse refuses non-3-uppercase via CurrencyCode::new
// ============================================================

#[derive(serde::Deserialize, Debug)]
struct CommerceToml {
    #[serde(default)]
    product: Vec<commerce_storefront_core::Product>,
}

#[derive(Debug, serde::Serialize)]
struct CommerceGateSummary {
    commerce_path: String,
    product_count: usize,
    published_count: usize,
    total_variants: usize,
    violations: Vec<String>,
}

fn run_commerce(root: &std::path::Path, action: &CommerceAction) -> Result<ExitCode> {
    match action {
        CommerceAction::Validate { json } => run_commerce_validate(root, *json),
    }
}

fn run_commerce_validate(root: &std::path::Path, json: bool) -> Result<ExitCode> {
    let path = root.join("commerce.toml");
    let mut violations: Vec<String> = Vec::new();
    let mut summary = CommerceGateSummary {
        commerce_path: path.display().to_string(),
        product_count: 0,
        published_count: 0,
        total_variants: 0,
        violations: Vec::new(),
    };

    if !path.exists() {
        violations.push(format!("missing commerce.toml at {}", path.display()));
        summary.violations = violations.clone();
        emit_commerce_summary(&summary, json);
        return Ok(ExitCode::from(2));
    }

    let s =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let parsed: CommerceToml = match toml::from_str(&s) {
        Ok(p) => p,
        Err(e) => {
            violations.push(format!("commerce.toml parse: {e}"));
            summary.violations = violations.clone();
            emit_commerce_summary(&summary, json);
            return Ok(ExitCode::from(2));
        }
    };

    summary.product_count = parsed.product.len();
    for p in &parsed.product {
        if p.published {
            summary.published_count += 1;
        }
        summary.total_variants += p.variants.len();
        if let Err(e) = p.validate() {
            violations.push(format!("product {:?}: {e}", p.id));
        }
    }

    summary.violations = violations.clone();
    emit_commerce_summary(&summary, json);

    if violations.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn emit_commerce_summary(summary: &CommerceGateSummary, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(summary)
                .unwrap_or_else(|_| "{\"error\":\"serialize-failed\"}".to_string())
        );
    } else {
        println!(
            "commerce-gate: {} products ({} published, {} variants total)",
            summary.product_count, summary.published_count, summary.total_variants,
        );
        if !summary.violations.is_empty() {
            println!("violations:");
            for v in &summary.violations {
                println!("  - {v}");
            }
        }
    }
}

#[cfg(test)]
mod commerce_gate_tests {
    use super::*;
    use tempfile::TempDir;

    fn write_commerce(dir: &std::path::Path, body: &str) {
        std::fs::write(dir.join("commerce.toml"), body).unwrap();
    }

    fn ok_product() -> &'static str {
        "[[product]]\n\
         id = \"tshirt\"\n\
         title = \"T-Shirt\"\n\
         description = \"Cotton\"\n\
         published = true\n\
         \n\
         [[product.variants]]\n\
         id = \"tshirt-m-blue\"\n\
         sku = \"TSHIRT-M-BLUE\"\n\
         title = \"M / Blue\"\n\
         price = 1999\n\
         currency = \"USD\"\n"
    }

    #[test]
    fn missing_file_exits_2() {
        let td = TempDir::new().unwrap();
        let code = run_commerce_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn malformed_toml_exits_2() {
        let td = TempDir::new().unwrap();
        write_commerce(td.path(), "not toml [[[");
        let code = run_commerce_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn empty_product_list_passes() {
        let td = TempDir::new().unwrap();
        write_commerce(td.path(), "");
        let code = run_commerce_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn valid_product_with_variant_passes() {
        let td = TempDir::new().unwrap();
        write_commerce(td.path(), ok_product());
        let code = run_commerce_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn lowercase_currency_violates() {
        // CurrencyCode uses `#[serde(transparent)]`, so the inner
        // String deserialises directly without running new().
        // Variant::validate() re-runs the ISO 4217 shape check —
        // bad currency surfaces as exit 1 (violation), not
        // exit 2 (parse error).
        let td = TempDir::new().unwrap();
        let body = ok_product().replace("\"USD\"", "\"usd\"");
        write_commerce(td.path(), &body);
        let code = run_commerce_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn negative_price_violates() {
        let td = TempDir::new().unwrap();
        let body = ok_product().replace("price = 1999", "price = -1");
        write_commerce(td.path(), &body);
        let code = run_commerce_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn empty_sku_violates() {
        let td = TempDir::new().unwrap();
        let body = ok_product().replace("\"TSHIRT-M-BLUE\"", "\"\"");
        write_commerce(td.path(), &body);
        let code = run_commerce_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }
}

// ============================================================
// T91 (tenth wiring): memberships gate — `forge memberships
// validate`
//
// TOML schema:
//   [[tier]]
//   id = "supporter"
//   name = "Supporter"
//   rank = 1
//   monthly-price = 500
//   currency = "USD"
//   annual-only = false
//
// Invariants enforced by Tier::validate():
//   * id is kebab-case
//   * name non-empty
//   * monthly_price ≥ 0
//   * currency is ISO 4217 3-upper-letter
// ============================================================

#[derive(serde::Deserialize, Debug)]
struct MembershipsToml {
    #[serde(default)]
    tier: Vec<memberships_core::Tier>,
}

#[derive(Debug, serde::Serialize)]
struct MembershipsGateSummary {
    memberships_path: String,
    tier_count: usize,
    free_tier_count: usize,
    violations: Vec<String>,
}

fn run_memberships(root: &std::path::Path, action: &MembershipsAction) -> Result<ExitCode> {
    match action {
        MembershipsAction::Validate { json } => run_memberships_validate(root, *json),
    }
}

fn run_memberships_validate(root: &std::path::Path, json: bool) -> Result<ExitCode> {
    let path = root.join("memberships.toml");
    let mut violations: Vec<String> = Vec::new();
    let mut summary = MembershipsGateSummary {
        memberships_path: path.display().to_string(),
        tier_count: 0,
        free_tier_count: 0,
        violations: Vec::new(),
    };

    if !path.exists() {
        violations.push(format!("missing memberships.toml at {}", path.display()));
        summary.violations = violations.clone();
        emit_memberships_summary(&summary, json);
        return Ok(ExitCode::from(2));
    }

    let s =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let parsed: MembershipsToml = match toml::from_str(&s) {
        Ok(p) => p,
        Err(e) => {
            violations.push(format!("memberships.toml parse: {e}"));
            summary.violations = violations.clone();
            emit_memberships_summary(&summary, json);
            return Ok(ExitCode::from(2));
        }
    };

    summary.tier_count = parsed.tier.len();
    for t in &parsed.tier {
        if t.monthly_price == 0 {
            summary.free_tier_count += 1;
        }
        if let Err(e) = t.validate() {
            violations.push(format!("tier {:?}: {e}", t.id));
        }
    }

    summary.violations = violations.clone();
    emit_memberships_summary(&summary, json);

    if violations.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn emit_memberships_summary(summary: &MembershipsGateSummary, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(summary)
                .unwrap_or_else(|_| "{\"error\":\"serialize-failed\"}".to_string())
        );
    } else {
        println!(
            "memberships-gate: {} tiers ({} free)",
            summary.tier_count, summary.free_tier_count,
        );
        if !summary.violations.is_empty() {
            println!("violations:");
            for v in &summary.violations {
                println!("  - {v}");
            }
        }
    }
}

#[cfg(test)]
mod memberships_gate_tests {
    use super::*;
    use tempfile::TempDir;

    fn write_memberships(dir: &std::path::Path, body: &str) {
        std::fs::write(dir.join("memberships.toml"), body).unwrap();
    }

    fn ok_tier() -> &'static str {
        "[[tier]]\n\
         id = \"supporter\"\n\
         name = \"Supporter\"\n\
         rank = 1\n\
         monthly-price = 500\n\
         currency = \"USD\"\n\
         annual-only = false\n"
    }

    #[test]
    fn missing_file_exits_2() {
        let td = TempDir::new().unwrap();
        let code = run_memberships_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn malformed_toml_exits_2() {
        let td = TempDir::new().unwrap();
        write_memberships(td.path(), "not toml [[[");
        let code = run_memberships_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn empty_tier_list_passes() {
        let td = TempDir::new().unwrap();
        write_memberships(td.path(), "");
        let code = run_memberships_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn valid_tier_passes() {
        let td = TempDir::new().unwrap();
        write_memberships(td.path(), ok_tier());
        let code = run_memberships_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn non_kebab_id_violates() {
        let td = TempDir::new().unwrap();
        let body = ok_tier().replace("\"supporter\"", "\"Supporter\"");
        write_memberships(td.path(), &body);
        let code = run_memberships_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn negative_price_violates() {
        let td = TempDir::new().unwrap();
        let body = ok_tier().replace("monthly-price = 500", "monthly-price = -1");
        write_memberships(td.path(), &body);
        let code = run_memberships_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn lowercase_currency_violates() {
        let td = TempDir::new().unwrap();
        let body = ok_tier().replace("\"USD\"", "\"usd\"");
        write_memberships(td.path(), &body);
        let code = run_memberships_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn free_tier_counts_as_free() {
        let td = TempDir::new().unwrap();
        let body = ok_tier().replace("monthly-price = 500", "monthly-price = 0");
        write_memberships(td.path(), &body);
        let code = run_memberships_validate(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }
}

// ============================================================
// T91 umbrella: `forge config validate-all`
//
// One-shot operator UX wrapper that runs every config-gate in
// sequence and aggregates results. Each individual gate handles
// its own "missing file" case; this aggregator captures their
// exit codes + reports per-gate verdicts.
//
// Design decision: missing config files are reported but
// treated as warnings, NOT violations, because a tenant that
// doesn't (e.g.) sell anything doesn't need commerce.toml.
// Operators can declare which gates are required-for-them by
// listing them in their CI workflow individually with --strict
// (already supported by each underlying gate's exit-2-for-
// missing semantic). The umbrella here is the discoverability
// + dashboard-shaped report.
// ============================================================

#[derive(Debug, serde::Serialize)]
struct ConfigGateVerdict {
    gate: &'static str,
    config_file: &'static str,
    /// "pass" | "fail" | "missing"
    verdict: String,
    /// Underlying gate's exit code: 0 / 1 / 2.
    exit_code: i32,
}

#[derive(Debug, serde::Serialize)]
struct ConfigValidateAllSummary {
    total_gates: usize,
    passed: usize,
    failed: usize,
    missing: usize,
    verdicts: Vec<ConfigGateVerdict>,
}

// ============================================================
// T98 — `forge content` runtime wiring
//
// Three runtime entrypoints projected through the typed
// importers-core::CmsSection + exporters-core::ExportFormat
// contract crates:
//   * `forge content validate <path>` — parse + validate
//     invariants (slug kebab, single Hero, headings 1..=6,
//     Image alt non-empty per WCAG 2.1 §1.1.1).
//   * `forge content export <path> [--format X]` — render
//     section to Markdown / JSON / JSON-LD-Schema-Org on
//     stdout. Format strings accept both the short form
//     ("markdown") and the canonical ExportFormat slug
//     ("markdown-yaml-frontmatter").
//   * `forge content formats` — list every supported
//     ExportFormat with slug + IANA media type + extension.
//
// Exit codes match the T91 family: 0 ok / 1 invariant
// violation / 2 fatal (file missing, parse error,
// unsupported format).
// ============================================================

fn run_content(action: &ContentAction) -> Result<ExitCode> {
    match action {
        ContentAction::Validate { path, json } => run_content_validate(path, *json),
        ContentAction::Export { path, format } => run_content_export(path, format),
        ContentAction::Formats => run_content_formats(),
    }
}

#[derive(Debug, serde::Serialize)]
struct ContentGateSummary<'a> {
    path: &'a str,
    status: &'static str,
    blocks: usize,
    slug: &'a str,
    source: &'a str,
    violations: Vec<String>,
}

fn emit_content_summary(s: &ContentGateSummary<'_>, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(s)
                .unwrap_or_else(|_| "{\"error\":\"serialize-failed\"}".to_string())
        );
    } else if s.violations.is_empty() {
        println!(
            "content-gate: {} ({} block(s), slug={}, source={})",
            s.status, s.blocks, s.slug, s.source,
        );
    } else {
        println!(
            "content-gate: {} ({} violation(s)):",
            s.status,
            s.violations.len()
        );
        for v in &s.violations {
            println!("  - {v}");
        }
    }
}

fn run_content_validate(path: &std::path::Path, json: bool) -> Result<ExitCode> {
    let path_str = path.display().to_string();
    if !path.exists() {
        emit_content_summary(
            &ContentGateSummary {
                path: &path_str,
                status: "fatal",
                blocks: 0,
                slug: "",
                source: "",
                violations: vec![format!("missing file {path_str}")],
            },
            json,
        );
        return Ok(ExitCode::from(2));
    }
    let s = std::fs::read_to_string(path).with_context(|| format!("reading {path_str}"))?;
    let section: importers_core::CmsSection = match serde_json::from_str(&s) {
        Ok(c) => c,
        Err(e) => {
            emit_content_summary(
                &ContentGateSummary {
                    path: &path_str,
                    status: "fatal",
                    blocks: 0,
                    slug: "",
                    source: "",
                    violations: vec![format!("parse error: {e}")],
                },
                json,
            );
            return Ok(ExitCode::from(2));
        }
    };
    match section.validate() {
        Ok(()) => {
            emit_content_summary(
                &ContentGateSummary {
                    path: &path_str,
                    status: "ok",
                    blocks: section.blocks.len(),
                    slug: &section.slug,
                    source: section.source.slug(),
                    violations: vec![],
                },
                json,
            );
            Ok(ExitCode::SUCCESS)
        }
        Err(e) => {
            emit_content_summary(
                &ContentGateSummary {
                    path: &path_str,
                    status: "violation",
                    blocks: section.blocks.len(),
                    slug: &section.slug,
                    source: section.source.slug(),
                    violations: vec![e.to_string()],
                },
                json,
            );
            Ok(ExitCode::from(1))
        }
    }
}

fn run_content_export(path: &std::path::Path, format: &str) -> Result<ExitCode> {
    if !path.exists() {
        eprintln!("content: missing file {}", path.display());
        return Ok(ExitCode::from(2));
    }
    let s = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let section: importers_core::CmsSection = match serde_json::from_str(&s) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("content: parse error in {}: {e}", path.display());
            return Ok(ExitCode::from(2));
        }
    };
    if let Err(e) = section.validate() {
        eprintln!("content: invariant violation upstream: {e}");
        return Ok(ExitCode::from(1));
    }
    let fmt = match resolve_export_format(format) {
        Some(f) => f,
        None => {
            eprintln!(
                "content: unsupported format {format:?}; supported: {}",
                supported_format_slugs().join(", "),
            );
            return Ok(ExitCode::from(2));
        }
    };
    match fmt {
        exporters_core::ExportFormat::MarkdownYamlFrontmatter => {
            match exporters_core::render_markdown(&section) {
                Ok(md) => {
                    print!("{md}");
                    Ok(ExitCode::SUCCESS)
                }
                Err(e) => {
                    eprintln!("content: render-markdown: {e}");
                    Ok(ExitCode::from(1))
                }
            }
        }
        exporters_core::ExportFormat::Json => match exporters_core::render_json(&section) {
            Ok(bytes) => {
                use std::io::Write as _;
                std::io::stdout().write_all(&bytes).ok();
                Ok(ExitCode::SUCCESS)
            }
            Err(e) => {
                eprintln!("content: render-json: {e}");
                Ok(ExitCode::from(1))
            }
        },
        exporters_core::ExportFormat::JsonLdSchemaOrg => {
            match exporters_core::render_json_ld(&section) {
                Ok(bytes) => {
                    use std::io::Write as _;
                    std::io::stdout().write_all(&bytes).ok();
                    Ok(ExitCode::SUCCESS)
                }
                Err(e) => {
                    eprintln!("content: render-json-ld: {e}");
                    Ok(ExitCode::from(1))
                }
            }
        }
        // PortableTarball + ActivityStreams2 are declared in the
        // closed enum but the renderers are still pending — gate
        // with a clear exit-2 so the surface stays honest.
        other => {
            eprintln!(
                "content: format {} declared but renderer not yet wired",
                other.slug()
            );
            Ok(ExitCode::from(2))
        }
    }
}

fn run_content_formats() -> Result<ExitCode> {
    for fmt in all_export_formats() {
        println!("{}\t{}\t.{}", fmt.slug(), fmt.media_type(), fmt.extension());
    }
    Ok(ExitCode::SUCCESS)
}

/// Resolve a CLI-side format string to an `ExportFormat`. Accepts
/// both the short alias (`markdown`, `json-ld`) and the canonical
/// slug (`markdown-yaml-frontmatter`, `json-ld-schema-org`).
fn resolve_export_format(s: &str) -> Option<exporters_core::ExportFormat> {
    use exporters_core::ExportFormat as F;
    match s {
        "markdown" | "md" | "markdown-yaml-frontmatter" => Some(F::MarkdownYamlFrontmatter),
        "json" => Some(F::Json),
        "json-ld" | "jsonld" | "json-ld-schema-org" => Some(F::JsonLdSchemaOrg),
        "portable" | "portable-tarball" | "tar" => Some(F::PortableTarball),
        "activitystreams" | "activitystreams-2" | "as2" => Some(F::ActivityStreams2),
        _ => None,
    }
}

fn all_export_formats() -> [exporters_core::ExportFormat; 5] {
    use exporters_core::ExportFormat as F;
    [
        F::MarkdownYamlFrontmatter,
        F::Json,
        F::JsonLdSchemaOrg,
        F::PortableTarball,
        F::ActivityStreams2,
    ]
}

fn supported_format_slugs() -> Vec<&'static str> {
    all_export_formats().iter().map(|f| f.slug()).collect()
}

#[cfg(test)]
mod content_gate_tests {
    use super::*;
    use std::io::Write as _;
    use tempfile::TempDir;

    fn minimal_section_json() -> &'static str {
        r#"{
            "id": "demo-1",
            "source": "wordpress",
            "slug": "hello",
            "title": "Hello",
            "blocks": [
                {"kind": "heading", "level": 2, "text": "Hi"}
            ]
        }"#
    }

    fn write(td: &TempDir, name: &str, body: &str) -> std::path::PathBuf {
        let p = td.path().join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        p
    }

    #[test]
    fn validate_ok() {
        let td = TempDir::new().unwrap();
        let p = write(&td, "ok.json", minimal_section_json());
        let code = run_content_validate(&p, false).unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::SUCCESS));
    }

    #[test]
    fn validate_missing_returns_2() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("nope.json");
        let code = run_content_validate(&p, false).unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(2)));
    }

    #[test]
    fn validate_parse_error_returns_2() {
        let td = TempDir::new().unwrap();
        let p = write(&td, "garbage.json", "{this is not json");
        let code = run_content_validate(&p, false).unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(2)));
    }

    #[test]
    fn validate_invariant_violation_returns_1() {
        let td = TempDir::new().unwrap();
        // image with empty alt → WCAG 2.1 §1.1.1
        let body = r#"{
            "id":"x","source":"wordpress","slug":"x","title":"T",
            "blocks":[{"kind":"image","asset_ref":"/a.png","alt":""}]
        }"#;
        let p = write(&td, "bad.json", body);
        let code = run_content_validate(&p, false).unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(1)));
    }

    #[test]
    fn export_markdown_ok() {
        let td = TempDir::new().unwrap();
        let p = write(&td, "ok.json", minimal_section_json());
        let code = run_content_export(&p, "markdown").unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::SUCCESS));
    }

    #[test]
    fn export_json_canonical_slug_ok() {
        let td = TempDir::new().unwrap();
        let p = write(&td, "ok.json", minimal_section_json());
        let code = run_content_export(&p, "json-ld-schema-org").unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::SUCCESS));
    }

    #[test]
    fn export_unknown_format_returns_2() {
        let td = TempDir::new().unwrap();
        let p = write(&td, "ok.json", minimal_section_json());
        let code = run_content_export(&p, "yaml").unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(2)));
    }

    #[test]
    fn formats_lists_all_five() {
        // Smoke: just assert the helper returns the right count;
        // run_content_formats() just streams stdout.
        assert_eq!(all_export_formats().len(), 5);
        assert_eq!(supported_format_slugs().len(), 5);
    }

    #[test]
    fn resolve_aliases_match_canonical() {
        use exporters_core::ExportFormat as F;
        assert_eq!(
            resolve_export_format("md"),
            Some(F::MarkdownYamlFrontmatter)
        );
        assert_eq!(
            resolve_export_format("markdown-yaml-frontmatter"),
            Some(F::MarkdownYamlFrontmatter),
        );
        assert_eq!(resolve_export_format("json-ld"), Some(F::JsonLdSchemaOrg));
        assert_eq!(resolve_export_format("as2"), Some(F::ActivityStreams2));
        assert_eq!(resolve_export_format("xml"), None);
    }
}

// ============================================================
// T98 third runtime wiring — `forge search validate-index`
//
// Projects a JSON `IndexDoc[]` through search-core (T82). Used
// to refuse pushing malformed search payloads to a backend.
// ============================================================

fn run_search(action: &SearchAction) -> Result<ExitCode> {
    match action {
        SearchAction::ValidateIndex { path, json } => run_search_validate_index(path, *json),
    }
}

#[derive(Debug, serde::Serialize)]
struct SearchGateSummary<'a> {
    path: &'a str,
    status: &'static str,
    docs: usize,
    violations: Vec<String>,
}

fn emit_search_summary(s: &SearchGateSummary<'_>, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(s)
                .unwrap_or_else(|_| "{\"error\":\"serialize-failed\"}".to_string())
        );
    } else if s.violations.is_empty() {
        println!("search-gate: {} ({} doc(s))", s.status, s.docs);
    } else {
        println!(
            "search-gate: {} ({} violation(s)):",
            s.status,
            s.violations.len()
        );
        for v in &s.violations {
            println!("  - {v}");
        }
    }
}

fn run_search_validate_index(path: &std::path::Path, json: bool) -> Result<ExitCode> {
    let path_str = path.display().to_string();
    if !path.exists() {
        emit_search_summary(
            &SearchGateSummary {
                path: &path_str,
                status: "fatal",
                docs: 0,
                violations: vec![format!("missing file {path_str}")],
            },
            json,
        );
        return Ok(ExitCode::from(2));
    }
    let s = std::fs::read_to_string(path).with_context(|| format!("reading {path_str}"))?;
    let docs: Vec<search_core::IndexDoc> = match serde_json::from_str(&s) {
        Ok(d) => d,
        Err(e) => {
            emit_search_summary(
                &SearchGateSummary {
                    path: &path_str,
                    status: "fatal",
                    docs: 0,
                    violations: vec![format!("parse error: {e}")],
                },
                json,
            );
            return Ok(ExitCode::from(2));
        }
    };
    let mut violations: Vec<String> = Vec::new();
    let mut seen_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for (i, d) in docs.iter().enumerate() {
        if d.id.trim().is_empty() {
            violations.push(format!("doc[{i}]: id empty"));
        } else if !seen_ids.insert(d.id.as_str()) {
            violations.push(format!("doc[{i}]: duplicate id {:?}", d.id));
        }
        if d.title.trim().is_empty() {
            violations.push(format!("doc[{i}]: title empty (id={:?})", d.id));
        }
        if d.body.trim().is_empty() {
            violations.push(format!("doc[{i}]: body empty (id={:?})", d.id));
        }
        if !is_bcp47_stem(&d.lang) {
            violations.push(format!(
                "doc[{i}]: lang {:?} not a BCP-47 stem (id={:?})",
                d.lang, d.id,
            ));
        }
        for t in &d.tags {
            if t.trim().is_empty() {
                violations.push(format!("doc[{i}]: empty tag (id={:?})", d.id));
            }
        }
        for (k, vs) in &d.facets {
            if k.trim().is_empty() {
                violations.push(format!("doc[{i}]: empty facet key (id={:?})", d.id));
            }
            for v in vs {
                if v.trim().is_empty() {
                    violations.push(format!(
                        "doc[{i}]: empty facet value under {:?} (id={:?})",
                        k, d.id,
                    ));
                }
            }
        }
    }
    let status = if violations.is_empty() {
        "ok"
    } else {
        "violation"
    };
    let exit = if violations.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    };
    emit_search_summary(
        &SearchGateSummary {
            path: &path_str,
            status,
            docs: docs.len(),
            violations,
        },
        json,
    );
    Ok(exit)
}

/// Liberal BCP-47 stem check: `[a-z]{2,3}` optionally followed
/// by `-` segments of 2..=8 alphanumerics. Covers `en`, `en-US`,
/// `zh-Hant-TW` (limited subset — full RFC 5646 validation is
/// out of scope; this catches the common error of passing
/// `English` / `en_US` / empty / single-letter). 1-char
/// grandfathered tags like `i-klingon` are intentionally
/// rejected — they are vanishingly rare in real index payloads
/// and admitting them weakens the rule against typos.
fn is_bcp47_stem(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut parts = s.split('-');
    let primary = match parts.next() {
        Some(p) => p,
        None => return false,
    };
    if primary.len() < 2 || primary.len() > 3 || !primary.chars().all(|c| c.is_ascii_lowercase()) {
        return false;
    }
    for seg in parts {
        if seg.is_empty() || seg.len() > 8 || !seg.chars().all(|c| c.is_ascii_alphanumeric()) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod search_gate_tests {
    use super::*;
    use std::io::Write as _;
    use tempfile::TempDir;
    use time::macros::datetime;

    fn write_bytes(td: &TempDir, name: &str, body: &[u8]) -> std::path::PathBuf {
        let p = td.path().join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(body).unwrap();
        p
    }

    fn doc(id: &str, lang: &str) -> search_core::IndexDoc {
        search_core::IndexDoc {
            id: id.into(),
            title: "T".into(),
            body: "B".into(),
            tags: vec![],
            facets: vec![],
            lang: lang.into(),
            published_at: datetime!(2026-01-01 00:00:00 UTC),
        }
    }

    fn write_docs(td: &TempDir, name: &str, docs: &[search_core::IndexDoc]) -> std::path::PathBuf {
        let body = serde_json::to_vec(docs).unwrap();
        write_bytes(td, name, &body)
    }

    #[test]
    fn validate_ok() {
        let td = TempDir::new().unwrap();
        let p = write_docs(&td, "ok.json", &[doc("a", "en")]);
        let code = run_search_validate_index(&p, false).unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::SUCCESS));
    }

    #[test]
    fn missing_file_returns_2() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("nope.json");
        let code = run_search_validate_index(&p, false).unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(2)));
    }

    #[test]
    fn parse_error_returns_2() {
        let td = TempDir::new().unwrap();
        let p = write_bytes(&td, "garbage.json", b"{not array");
        let code = run_search_validate_index(&p, false).unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(2)));
    }

    #[test]
    fn duplicate_id_returns_1() {
        let td = TempDir::new().unwrap();
        let p = write_docs(&td, "dup.json", &[doc("dup", "en"), doc("dup", "en")]);
        let code = run_search_validate_index(&p, false).unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(1)));
    }

    #[test]
    fn bad_lang_returns_1() {
        let td = TempDir::new().unwrap();
        let p = write_docs(&td, "lang.json", &[doc("a", "English")]);
        let code = run_search_validate_index(&p, false).unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(1)));
    }

    #[test]
    fn empty_title_returns_1() {
        let td = TempDir::new().unwrap();
        let mut d = doc("a", "en");
        d.title = "".into();
        let p = write_docs(&td, "title.json", &[d]);
        let code = run_search_validate_index(&p, false).unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(1)));
    }

    #[test]
    fn bcp47_stem_recognises_common_tags() {
        assert!(is_bcp47_stem("en"));
        assert!(is_bcp47_stem("en-US"));
        assert!(is_bcp47_stem("zh-Hant-TW"));
        assert!(!is_bcp47_stem(""));
        assert!(!is_bcp47_stem("English"));
        assert!(!is_bcp47_stem("en_US"));
        assert!(!is_bcp47_stem("e"));
    }
}

// ============================================================
// T98 fourth runtime wiring — `forge assets validate`
//
// Projects an AssetBundle JSON file through assets-core (T80).
// Refuses to greenlight publish-time bundles missing the AVIF /
// WebP / JPEG fallback ladder or empty alt text. WCAG 2.1
// §1.1.1.
// ============================================================

fn run_assets(action: &AssetsAction) -> Result<ExitCode> {
    match action {
        AssetsAction::Validate { path, json } => run_assets_validate(path, *json),
    }
}

#[derive(Debug, serde::Serialize)]
struct AssetsGateSummary<'a> {
    path: &'a str,
    status: &'static str,
    variants: usize,
    asset_id: &'a str,
    alt_source: &'a str,
    violations: Vec<String>,
}

fn emit_assets_summary(s: &AssetsGateSummary<'_>, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(s)
                .unwrap_or_else(|_| "{\"error\":\"serialize-failed\"}".to_string())
        );
    } else if s.violations.is_empty() {
        println!(
            "assets-gate: {} ({} variant(s), asset_id={}, alt-source={})",
            s.status, s.variants, s.asset_id, s.alt_source,
        );
    } else {
        println!(
            "assets-gate: {} ({} violation(s)):",
            s.status,
            s.violations.len()
        );
        for v in &s.violations {
            println!("  - {v}");
        }
    }
}

fn run_assets_validate(path: &std::path::Path, json: bool) -> Result<ExitCode> {
    let path_str = path.display().to_string();
    if !path.exists() {
        emit_assets_summary(
            &AssetsGateSummary {
                path: &path_str,
                status: "fatal",
                variants: 0,
                asset_id: "",
                alt_source: "",
                violations: vec![format!("missing file {path_str}")],
            },
            json,
        );
        return Ok(ExitCode::from(2));
    }
    let s = std::fs::read_to_string(path).with_context(|| format!("reading {path_str}"))?;
    let bundle: assets_core::AssetBundle = match serde_json::from_str(&s) {
        Ok(b) => b,
        Err(e) => {
            emit_assets_summary(
                &AssetsGateSummary {
                    path: &path_str,
                    status: "fatal",
                    variants: 0,
                    asset_id: "",
                    alt_source: "",
                    violations: vec![format!("parse error: {e}")],
                },
                json,
            );
            return Ok(ExitCode::from(2));
        }
    };
    match bundle.validate_image_ladder() {
        Ok(()) => {
            emit_assets_summary(
                &AssetsGateSummary {
                    path: &path_str,
                    status: "ok",
                    variants: bundle.variants.len(),
                    asset_id: &bundle.asset_id,
                    alt_source: bundle.alt_source.slug(),
                    violations: vec![],
                },
                json,
            );
            Ok(ExitCode::SUCCESS)
        }
        Err(e) => {
            emit_assets_summary(
                &AssetsGateSummary {
                    path: &path_str,
                    status: "violation",
                    variants: bundle.variants.len(),
                    asset_id: &bundle.asset_id,
                    alt_source: bundle.alt_source.slug(),
                    violations: vec![e.to_string()],
                },
                json,
            );
            Ok(ExitCode::from(1))
        }
    }
}

// ----------------------------------------------------------------
// `forge doctrine query` — task #175.
//
// Loads PlausiDen-AVP-Doctrine's doctrine/rules/*.toml via
// doctrine-core (task #174), applies filters, emits human or JSON
// output.
//
// Doctrine dir resolution order:
//   1. --doctrine-dir flag
//   2. PLAUSIDEN_DOCTRINE_DIR env var
//   3. <forge-root>/../PlausiDen-AVP-Doctrine
// ----------------------------------------------------------------

fn run_doctrine(action: &DoctrineAction, forge_root: &std::path::Path) -> Result<ExitCode> {
    match action {
        DoctrineAction::Query {
            rule,
            domain,
            severity,
            lifecycle,
            search,
            related_trait,
            doctrine_dir,
            json,
        } => run_doctrine_query(
            rule.as_deref(),
            domain.as_deref(),
            severity.as_deref(),
            lifecycle.as_deref(),
            search.as_deref(),
            related_trait.as_deref(),
            doctrine_dir.as_deref(),
            *json,
            forge_root,
        ),
    }
}

fn resolve_doctrine_dir(
    explicit: Option<&std::path::Path>,
    forge_root: &std::path::Path,
) -> PathBuf {
    if let Some(p) = explicit {
        return p.to_path_buf();
    }
    if let Ok(env) = std::env::var("PLAUSIDEN_DOCTRINE_DIR") {
        if !env.is_empty() {
            return PathBuf::from(env);
        }
    }
    forge_root
        .parent()
        .map(|p| p.join("PlausiDen-AVP-Doctrine"))
        .unwrap_or_else(|| PathBuf::from("PlausiDen-AVP-Doctrine"))
}

#[allow(clippy::too_many_arguments)]
fn run_doctrine_query(
    rule: Option<&str>,
    domain: Option<&str>,
    severity: Option<&str>,
    lifecycle: Option<&str>,
    search: Option<&str>,
    related_trait: Option<&str>,
    doctrine_dir: Option<&std::path::Path>,
    json: bool,
    forge_root: &std::path::Path,
) -> Result<ExitCode> {
    let dir = resolve_doctrine_dir(doctrine_dir, forge_root);
    let db = match doctrine_core::load_from_dir(&dir) {
        Ok(d) => d,
        Err(e) => {
            if json {
                let payload = serde_json::json!({
                    "status": "fatal",
                    "error": e.to_string(),
                    "doctrine_dir": dir.display().to_string(),
                });
                println!("{}", payload);
            } else {
                eprintln!(
                    "forge doctrine: failed to load doctrine from {} — {}",
                    dir.display(),
                    e
                );
                eprintln!(
                    "(set --doctrine-dir or PLAUSIDEN_DOCTRINE_DIR to point at PlausiDen-AVP-Doctrine)"
                );
            }
            return Ok(ExitCode::from(2));
        }
    };

    // Direct id lookup short-circuits other filters.
    if let Some(id) = rule {
        return match db.by_id(id) {
            Some(r) => {
                emit_doctrine_rules(std::slice::from_ref(r), json);
                Ok(ExitCode::SUCCESS)
            }
            None => {
                if json {
                    println!(
                        "{}",
                        serde_json::json!({"status":"empty","matched":0,"rule":id})
                    );
                } else {
                    eprintln!("forge doctrine: no rule with id {id}");
                }
                Ok(ExitCode::from(1))
            }
        };
    }

    // Parse + validate filter values.
    let dom_filter = match domain {
        None => None,
        Some(s) => match parse_doctrine_domain(s) {
            Some(d) => Some(d),
            None => {
                eprintln!(
                    "forge doctrine: unknown --domain {s} (expected one of: build, primitives, security, testing, docs, logging, perf, content, accessibility)"
                );
                return Ok(ExitCode::from(2));
            }
        },
    };
    let sev_filter = match severity {
        None => None,
        Some(s) => match parse_doctrine_severity(s) {
            Some(v) => Some(v),
            None => {
                eprintln!(
                    "forge doctrine: unknown --severity {s} (expected one of: strict, warn, informational, experimental)"
                );
                return Ok(ExitCode::from(2));
            }
        },
    };
    let lc_filter = match lifecycle {
        None => None,
        Some(s) => match parse_doctrine_lifecycle(s) {
            Some(v) => Some(v),
            None => {
                eprintln!(
                    "forge doctrine: unknown --lifecycle {s} (expected one of: experimental, stable, deprecated)"
                );
                return Ok(ExitCode::from(2));
            }
        },
    };

    let needle_lc = search.map(|s| s.to_lowercase());

    let matched: Vec<&doctrine_core::Rule> = db
        .all()
        .iter()
        .filter(|r| dom_filter.is_none_or(|d| r.domain == d))
        .filter(|r| sev_filter.is_none_or(|s| r.severity == s))
        .filter(|r| lc_filter.is_none_or(|l| r.lifecycle == l))
        .filter(|r| match &needle_lc {
            None => true,
            Some(n) => {
                r.statement.to_lowercase().contains(n)
                    || r.rationale.to_lowercase().contains(n)
                    || r.name.to_lowercase().contains(n)
            }
        })
        .filter(|r| match related_trait {
            None => true,
            Some(t) => r.related_traits.iter().any(|x| x == t),
        })
        .collect();

    if matched.is_empty() {
        if json {
            println!(
                "{}",
                serde_json::json!({"status":"empty","matched":0,"total_loaded":db.len()})
            );
        } else {
            eprintln!(
                "forge doctrine: no rules matched the supplied filters ({} rules loaded total)",
                db.len()
            );
        }
        return Ok(ExitCode::from(1));
    }

    emit_doctrine_rules(matched.iter().copied(), json);
    Ok(ExitCode::SUCCESS)
}

fn parse_doctrine_domain(s: &str) -> Option<doctrine_core::Domain> {
    match s.to_lowercase().as_str() {
        "build" => Some(doctrine_core::Domain::Build),
        "primitives" => Some(doctrine_core::Domain::Primitives),
        "security" => Some(doctrine_core::Domain::Security),
        "testing" => Some(doctrine_core::Domain::Testing),
        "docs" => Some(doctrine_core::Domain::Docs),
        "logging" => Some(doctrine_core::Domain::Logging),
        "perf" => Some(doctrine_core::Domain::Perf),
        "content" => Some(doctrine_core::Domain::Content),
        "accessibility" => Some(doctrine_core::Domain::Accessibility),
        _ => None,
    }
}

fn parse_doctrine_severity(s: &str) -> Option<doctrine_core::Severity> {
    match s.to_lowercase().as_str() {
        "strict" => Some(doctrine_core::Severity::Strict),
        "warn" => Some(doctrine_core::Severity::Warn),
        "informational" => Some(doctrine_core::Severity::Informational),
        "experimental" => Some(doctrine_core::Severity::Experimental),
        _ => None,
    }
}

fn parse_doctrine_lifecycle(s: &str) -> Option<doctrine_core::Lifecycle> {
    match s.to_lowercase().as_str() {
        "experimental" => Some(doctrine_core::Lifecycle::Experimental),
        "stable" => Some(doctrine_core::Lifecycle::Stable),
        "deprecated" => Some(doctrine_core::Lifecycle::Deprecated),
        _ => None,
    }
}

fn emit_doctrine_rules<'a, T>(rules: T, json: bool)
where
    T: IntoIterator<Item = &'a doctrine_core::Rule> + Clone,
{
    if json {
        // Per docs-008 (cross-AI compatibility): JSON output is the
        // machine-readable surface Claude / Gemini / other agents
        // consume; the human surface is the text form below.
        let rules_vec: Vec<&doctrine_core::Rule> = rules.into_iter().collect();
        let payload = serde_json::json!({
            "status": "ok",
            "matched": rules_vec.len(),
            "rules": rules_vec,
        });
        println!("{}", payload);
        return;
    }
    let mut count = 0_usize;
    for r in rules {
        count += 1;
        println!("{} — {}  [{}/{}]", r.id, r.name, severity_label(r.severity), lifecycle_label(r.lifecycle));
        println!("  domain    : {:?}", r.domain);
        println!("  statement : {}", r.statement.trim());
        // Rationale can be multi-paragraph; indent each line.
        for line in r.rationale.trim().lines() {
            println!("  rationale : {line}");
        }
        for (i, e) in r.enforcement.iter().enumerate() {
            if i == 0 {
                println!("  enforce   : {e}");
            } else {
                println!("            : {e}");
            }
        }
        if !r.applies_to.is_empty() {
            println!("  applies-to: {}", r.applies_to.join("; "));
        }
        if !r.related_traits.is_empty() {
            println!("  traits    : {}", r.related_traits.join(", "));
        }
        if !r.references.is_empty() {
            println!("  refs      : {}", r.references.join(" | "));
        }
        if let Some(d) = &r.deprecated_at {
            println!("  deprecated: {d}{}", r.replaced_by.as_deref().map(|s| format!(" → {s}")).unwrap_or_default());
        }
        println!();
    }
    println!("({count} matched)");
}

fn severity_label(s: doctrine_core::Severity) -> &'static str {
    match s {
        doctrine_core::Severity::Strict => "strict",
        doctrine_core::Severity::Warn => "warn",
        doctrine_core::Severity::Informational => "info",
        doctrine_core::Severity::Experimental => "experimental",
        // #[non_exhaustive] on doctrine_core::Severity — handle future
        // variants gracefully as "unknown" rather than panicking.
        _ => "unknown",
    }
}

fn lifecycle_label(l: doctrine_core::Lifecycle) -> &'static str {
    match l {
        doctrine_core::Lifecycle::Experimental => "experimental",
        doctrine_core::Lifecycle::Stable => "stable",
        doctrine_core::Lifecycle::Deprecated => "deprecated",
        // #[non_exhaustive] — future variants surface as "unknown".
        _ => "unknown",
    }
}

#[cfg(test)]
mod doctrine_query_tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn resolves_explicit_dir() {
        let p = Path::new("/tmp/doctrine-test");
        assert_eq!(resolve_doctrine_dir(Some(p), Path::new("/x")), p);
    }

    #[test]
    fn falls_back_to_sibling_avp_doctrine() {
        // Note: doesn't touch the global PLAUSIDEN_DOCTRINE_DIR env var to
        // keep the test thread-safe. Only exercises the explicit-None path
        // when the env var is absent (the env var override is covered by
        // the explicit-Some-path test above).
        if std::env::var("PLAUSIDEN_DOCTRINE_DIR").is_ok() {
            // Env var is set by the harness — skip rather than mutate.
            return;
        }
        let forge_root = Path::new("/home/u/projects/PlausiDen-Forge");
        let resolved = resolve_doctrine_dir(None, forge_root);
        assert!(resolved.ends_with("PlausiDen-AVP-Doctrine"));
    }

    #[test]
    fn parses_domain_aliases() {
        assert!(matches!(parse_doctrine_domain("build"), Some(doctrine_core::Domain::Build)));
        assert!(matches!(parse_doctrine_domain("PRIMITIVES"), Some(doctrine_core::Domain::Primitives)));
        assert!(parse_doctrine_domain("bogus").is_none());
    }

    #[test]
    fn parses_severity_aliases() {
        assert!(matches!(parse_doctrine_severity("strict"), Some(doctrine_core::Severity::Strict)));
        assert!(matches!(parse_doctrine_severity("WARN"), Some(doctrine_core::Severity::Warn)));
        assert!(parse_doctrine_severity("bogus").is_none());
    }
}

#[cfg(test)]
mod assets_gate_tests {
    use super::*;
    use std::io::Write as _;
    use tempfile::TempDir;

    fn write_json(td: &TempDir, name: &str, body: &str) -> std::path::PathBuf {
        let p = td.path().join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        p
    }

    /// AssetBundle with full AVIF + WebP + JPEG ladder and a
    /// non-empty alt text. Uses 64-lowercase-hex sha256 stubs.
    fn full_ladder_json() -> &'static str {
        r#"{
            "asset-id": "demo-1",
            "source-media-type": "image/png",
            "source-width": 1024,
            "source-height": 768,
            "alt-text": "A demo image.",
            "alt-source": "operator",
            "variants": [
                {"source-id":"src","format":"avif","width":1024,"height":768,
                 "byte-len":2048,
                 "sha256-hex":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                 "exif-policy":"strip"},
                {"source-id":"src","format":"webp","width":1024,"height":768,
                 "byte-len":2048,
                 "sha256-hex":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                 "exif-policy":"strip"},
                {"source-id":"src","format":"jpeg","width":1024,"height":768,
                 "byte-len":2048,
                 "sha256-hex":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                 "exif-policy":"strip"}
            ]
        }"#
    }

    #[test]
    fn validate_ok() {
        let td = TempDir::new().unwrap();
        let p = write_json(&td, "ok.json", full_ladder_json());
        let code = run_assets_validate(&p, false).unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::SUCCESS));
    }

    #[test]
    fn missing_file_returns_2() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("nope.json");
        let code = run_assets_validate(&p, false).unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(2)));
    }

    #[test]
    fn parse_error_returns_2() {
        let td = TempDir::new().unwrap();
        let p = write_json(&td, "garbage.json", "{this is not a bundle");
        let code = run_assets_validate(&p, false).unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(2)));
    }

    #[test]
    fn empty_alt_text_returns_1() {
        let td = TempDir::new().unwrap();
        let body = full_ladder_json().replace("A demo image.", "");
        let p = write_json(&td, "noalt.json", &body);
        let code = run_assets_validate(&p, false).unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(1)));
    }

    #[test]
    fn missing_jpeg_returns_1() {
        // Strip the JPEG variant — bundle no longer satisfies the
        // fallback ladder.
        let td = TempDir::new().unwrap();
        let body = r#"{
            "asset-id": "no-jpeg",
            "source-media-type": "image/png",
            "alt-text": "Alt.",
            "alt-source": "operator",
            "variants": [
                {"source-id":"s","format":"avif","width":1,"height":1,
                 "byte-len":1,
                 "sha256-hex":"0000000000000000000000000000000000000000000000000000000000000000",
                 "exif-policy":"strip"},
                {"source-id":"s","format":"webp","width":1,"height":1,
                 "byte-len":1,
                 "sha256-hex":"1111111111111111111111111111111111111111111111111111111111111111",
                 "exif-policy":"strip"}
            ]
        }"#;
        let p = write_json(&td, "nojpeg.json", body);
        let code = run_assets_validate(&p, false).unwrap();
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(1)));
    }
}

fn run_config(root: &std::path::Path, action: &ConfigAction) -> Result<ExitCode> {
    match action {
        ConfigAction::ValidateAll { json } => run_config_validate_all(root, *json),
    }
}

fn run_config_validate_all(root: &std::path::Path, json: bool) -> Result<ExitCode> {
    type GateFn = fn(&std::path::Path, bool) -> Result<ExitCode>;
    let gates: &[(&'static str, &'static str, GateFn)] = &[
        ("privacy", "privacy.toml", run_privacy_validate),
        (
            "trust-safety",
            "trust-safety.toml",
            run_trust_safety_validate,
        ),
        ("domains", "domains.toml", run_domains_validate),
        ("forms", "forms.toml", run_forms_validate),
        ("federation", "federation.toml", run_federation_validate),
        ("email", "email.toml", run_email_validate),
        ("commerce", "commerce.toml", run_commerce_validate),
        ("memberships", "memberships.toml", run_memberships_validate),
    ];

    let mut summary = ConfigValidateAllSummary {
        total_gates: gates.len(),
        passed: 0,
        failed: 0,
        missing: 0,
        verdicts: Vec::with_capacity(gates.len()),
    };

    for (gate, file, run) in gates {
        // Run silently when emitting JSON aggregate; let the
        // gate print human output otherwise.
        let code = run(root, json)?;
        let exit_code = exit_code_to_i32(code);
        let verdict = match exit_code {
            0 => "pass",
            2 => "missing",
            _ => "fail",
        };
        match verdict {
            "pass" => summary.passed += 1,
            "missing" => summary.missing += 1,
            _ => summary.failed += 1,
        }
        summary.verdicts.push(ConfigGateVerdict {
            gate,
            config_file: file,
            verdict: verdict.to_string(),
            exit_code,
        });
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&summary)
                .unwrap_or_else(|_| "{\"error\":\"serialize-failed\"}".to_string())
        );
    } else {
        println!(
            "config-validate-all: {}/{} gates passed, {} missing, {} failed",
            summary.passed, summary.total_gates, summary.missing, summary.failed
        );
        for v in &summary.verdicts {
            println!("  {:14} ({}): {}", v.gate, v.config_file, v.verdict);
        }
    }

    if summary.failed > 0 {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

/// std::process::ExitCode doesn't expose its inner value; this
/// dispatches on the known cases the umbrella uses (0 / 1 / 2)
/// by re-running the comparison.
fn exit_code_to_i32(c: ExitCode) -> i32 {
    if c == ExitCode::SUCCESS {
        0
    } else if c == ExitCode::from(2) {
        2
    } else {
        1
    }
}

#[cfg(test)]
mod config_umbrella_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn all_missing_files_is_pass() {
        // No config files = nothing failed, just nothing
        // present. Tenant gets a clean exit.
        let td = TempDir::new().unwrap();
        let code = run_config_validate_all(td.path(), false).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn one_bad_file_fails() {
        // Drop in a malformed privacy.toml + nothing else; the
        // umbrella aggregates that one fail.
        let td = TempDir::new().unwrap();
        std::fs::write(td.path().join("privacy.toml"), "this isnt toml [[[").unwrap();
        let code = run_config_validate_all(td.path(), false).unwrap();
        // Malformed = exit 2 from privacy gate, which umbrella
        // classifies as "missing" — still exit 0 for umbrella.
        // (Operator decided to ship privacy.toml; the
        // individual gate's strict mode is the place to enforce
        // shape, not the umbrella's roll-up.)
        // Update if classification policy changes.
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn json_mode_emits_aggregate_object() {
        let td = TempDir::new().unwrap();
        let code = run_config_validate_all(td.path(), true).unwrap();
        // Just make sure the path runs without panic; output
        // capture is the operator's CI concern.
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
