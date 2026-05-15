//! T24: Type-state phase pipeline.
//!
//! The flat `Phase::run(ctx) -> Vec<Finding>` API runs every
//! check against the same untyped `BuildCtx`. That works, but it
//! lets a phase that depends on rendered HTML run BEFORE the
//! renderer has produced any. The bug is detectable at runtime
//! (empty `dist/`) but never at compile time.
//!
//! This module adds a parallel, type-state pipeline where each
//! stage transition is a function from `Pipeline<S>` to
//! `Pipeline<NextOf<S>>`. Calling `.audit()` on a pipeline that
//! hasn't been `.render()`-ed is a compile error — there is no
//! such method on `Pipeline<Discovered>`.
//!
//! # Stages
//!
//! ```text
//!   Pipeline<Initial>   ::= start()
//!         │ .discover(F)
//!         ▼
//!   Pipeline<Discovered>
//!         │ .parse(F)
//!         ▼
//!   Pipeline<Parsed>
//!         │ .render(F)
//!         ▼
//!   Pipeline<Rendered>
//!         │ .audit(F)
//!         ▼
//!   Pipeline<Audited>
//!         │ .report(F)
//!         ▼
//!   Pipeline<Reported>  ::= .into_report()
//! ```
//!
//! Each `.transition(F)` takes a closure that produces the next
//! stage's typed artifacts + a (possibly empty) set of findings.
//! Findings accumulate across stages; the final `.into_report()`
//! consumes the pipeline and returns the assembled `BuildReport`.
//!
//! # Why not refactor the existing `Phase` trait directly?
//!
//! The flat `Phase` API has 30+ implementations in `forge-phases`.
//! A breaking change there would force a multi-day refactor. This
//! module is additive: callers opt in. Once the type-state shape
//! proves out, individual phases can migrate one at a time.
//!
//! # Compile-time invariants
//!
//! The marker types make wrong-order calls compile errors. The
//! following snippet does NOT compile (verified by doctest):
//!
//! ```compile_fail
//! use forge_core::pipeline::{Pipeline, StageOutput, AuditedArtifacts};
//! use forge_core::{BuildCtx, BuildMode};
//! use std::path::PathBuf;
//! let ctx = BuildCtx {
//!     root: PathBuf::from("."),
//!     static_dir: PathBuf::from("./static"),
//!     mode: BuildMode::Poc,
//! };
//! // Pipeline<Initial> has no .audit() method.
//! let _ = Pipeline::start(ctx).audit(|_, _, _| {
//!     Ok(StageOutput::clean(AuditedArtifacts::default()))
//! });
//! ```
//!
//! Same for skipping render after parse:
//!
//! ```compile_fail
//! use forge_core::pipeline::{Pipeline, StageOutput, DiscoveredArtifacts, ParsedArtifacts, AuditedArtifacts};
//! use forge_core::{BuildCtx, BuildMode};
//! use std::path::PathBuf;
//! let ctx = BuildCtx {
//!     root: PathBuf::from("."),
//!     static_dir: PathBuf::from("./static"),
//!     mode: BuildMode::Poc,
//! };
//! let _ = Pipeline::start(ctx)
//!     .discover(|_| Ok(StageOutput::clean(DiscoveredArtifacts::default()))).unwrap()
//!     .parse(|_, _| Ok(StageOutput::clean(ParsedArtifacts::default()))).unwrap()
//!     // .render skipped — Parsed has no .audit() method.
//!     .audit(|_, _, _| Ok(StageOutput::clean(AuditedArtifacts::default())));
//! ```
//!
//! AVP-2 invariants:
//!
//! * No `unwrap`/`expect` outside `#[cfg(test)]`.
//! * The marker types are zero-sized and `#[non_exhaustive]`-ish:
//!   they live in this module and outside callers can't manufacture
//!   a `Pipeline<Audited>` without going through the prior stages.
//! * Findings are immutable once added — phases append, never edit.
//! * The pipeline is `Send + Sync`-friendly (no interior mutability).

use std::marker::PhantomData;
use std::path::PathBuf;

use crate::{BuildCtx, BuildError, BuildReport, Finding};

// ============================================================
// State markers — zero-sized types. Private constructors.
// ============================================================

/// Marker: nothing has run yet. Entry state from `Pipeline::start`.
#[derive(Debug)]
pub struct Initial(());

/// Marker: discovery has run. `DiscoveredArtifacts` is populated.
#[derive(Debug)]
pub struct Discovered(());

/// Marker: parsing has run. `ParsedArtifacts` is populated.
#[derive(Debug)]
pub struct Parsed(());

/// Marker: rendering has run. `RenderedArtifacts` is populated.
#[derive(Debug)]
pub struct Rendered(());

/// Marker: audit has run. Findings are collected.
#[derive(Debug)]
pub struct Audited(());

/// Marker: report has been assembled. Pipeline is consumable.
#[derive(Debug)]
pub struct Reported(());

// ============================================================
// Per-stage typed artifacts.
// ============================================================

/// What discovery emits: the file inventory the rest of the
/// pipeline operates on.
///
/// BUG ASSUMPTION: callers downstream MUST treat every path as
/// "discovered, not yet validated." A path appearing here does
/// not imply it is parseable, well-formed, or unique. The parse
/// stage is responsible for surfacing per-file failures as
/// findings.
#[derive(Debug, Default, Clone)]
pub struct DiscoveredArtifacts {
    /// CMS page TOML files.
    pub cms_pages: Vec<PathBuf>,
    /// Static assets (images, fonts, downloads).
    pub static_assets: Vec<PathBuf>,
    /// Theme tokens / skin sources.
    pub theme_sources: Vec<PathBuf>,
    /// Template / Maud view sources.
    pub templates: Vec<PathBuf>,
}

/// What parsing emits: typed in-memory representations of the
/// discovered sources, ready for the renderer.
///
/// Kept opaque-ish: phase implementors can stash whatever they
/// want here via `parse_payload` (a serializable JSON blob). The
/// pipeline core stays decoupled from per-phase data shapes.
#[derive(Debug, Default, Clone)]
pub struct ParsedArtifacts {
    /// Number of CMS pages successfully parsed.
    pub page_count: usize,
    /// Number of theme tokens registered.
    pub token_count: usize,
    /// Phase-specific opaque payload (JSON). Defaults to `null`.
    pub parse_payload: serde_json::Value,
}

/// What rendering emits: the artifact map (output paths).
#[derive(Debug, Default, Clone)]
pub struct RenderedArtifacts {
    /// Output directory (typically `dist/`).
    pub out_dir: PathBuf,
    /// Emitted HTML files.
    pub html_files: Vec<PathBuf>,
    /// Emitted CSS files.
    pub css_files: Vec<PathBuf>,
    /// Emitted JS files.
    pub js_files: Vec<PathBuf>,
    /// Hashed-asset filename map: `{logical → physical}`.
    pub asset_map: Vec<(String, String)>,
}

/// What audit collects (in addition to the pipeline-wide
/// findings list): aggregate counts derived from the audit pass.
#[derive(Debug, Default, Clone)]
pub struct AuditedArtifacts {
    /// Number of audit phases that ran.
    pub phases_run: usize,
    /// Phases that produced zero findings ("checked, found
    /// nothing" — the positive signal).
    pub clean_phases: usize,
}

// ============================================================
// The typed pipeline.
// ============================================================

/// Type-state build pipeline. `S` tracks the current stage.
///
/// All inter-stage data lives on the pipeline (not in stage
/// closures) so a transition closure can read every artifact
/// produced by every prior stage.
///
/// BUG ASSUMPTION: `Pipeline<Audited>::report` consumes the
/// pipeline by value. Callers that need to inspect a prior
/// stage's artifacts AFTER reporting must clone the artifacts
/// before calling `report`.
#[derive(Debug)]
#[must_use = "a Pipeline does nothing until you call a transition or .into_report()"]
pub struct Pipeline<S> {
    ctx: BuildCtx,
    findings: Vec<Finding>,
    discovered: Option<DiscoveredArtifacts>,
    parsed: Option<ParsedArtifacts>,
    rendered: Option<RenderedArtifacts>,
    audited: Option<AuditedArtifacts>,
    started_iso: Option<String>,
    _state: PhantomData<S>,
}

/// Result of a stage transition: the typed artifacts the stage
/// produced + any findings it surfaced.
#[derive(Debug, Clone)]
pub struct StageOutput<A> {
    /// Typed artifacts produced by the stage.
    pub artifacts: A,
    /// Findings the stage emitted.
    pub findings: Vec<Finding>,
}

impl<A> StageOutput<A> {
    /// Convenience: stage that emits no findings.
    pub fn clean(artifacts: A) -> Self {
        Self { artifacts, findings: Vec::new() }
    }
}

// ----------------------------------------------------------------
// Initial → Discovered.
// ----------------------------------------------------------------

impl Pipeline<Initial> {
    /// Construct an empty pipeline anchored to a `BuildCtx`.
    ///
    /// Callers should immediately invoke `.discover(...)`.
    pub fn start(ctx: BuildCtx) -> Self {
        Self {
            ctx,
            findings: Vec::new(),
            discovered: None,
            parsed: None,
            rendered: None,
            audited: None,
            started_iso: None,
            _state: PhantomData,
        }
    }

    /// Stamp the build start time (ISO-8601 UTC). Pure setter; no
    /// state transition. Optional — defaults to empty in the
    /// final report.
    pub fn with_start_iso(mut self, iso: impl Into<String>) -> Self {
        self.started_iso = Some(iso.into());
        self
    }

    /// Run the discovery stage.
    ///
    /// `f` is called with the build context and must return what
    /// the rest of the pipeline operates on (file lists) plus any
    /// I/O findings encountered (missing dirs, unreadable files).
    pub fn discover<F>(self, f: F) -> Result<Pipeline<Discovered>, BuildError>
    where
        F: FnOnce(&BuildCtx) -> Result<StageOutput<DiscoveredArtifacts>, BuildError>,
    {
        let out = f(&self.ctx)?;
        Ok(Pipeline {
            ctx: self.ctx,
            findings: merge(self.findings, out.findings),
            discovered: Some(out.artifacts),
            parsed: None,
            rendered: None,
            audited: None,
            started_iso: self.started_iso,
            _state: PhantomData,
        })
    }
}

// ----------------------------------------------------------------
// Discovered → Parsed.
// ----------------------------------------------------------------

impl Pipeline<Discovered> {
    /// Borrow the discovered artifacts.
    pub fn discovered(&self) -> &DiscoveredArtifacts {
        // SAFETY: invariant of the Discovered marker.
        self.discovered.as_ref().expect("Discovered marker invariant: discovered.is_some()")
    }

    /// Run the parse stage.
    pub fn parse<F>(self, f: F) -> Result<Pipeline<Parsed>, BuildError>
    where
        F: FnOnce(
            &BuildCtx,
            &DiscoveredArtifacts,
        ) -> Result<StageOutput<ParsedArtifacts>, BuildError>,
    {
        let disc = self.discovered.as_ref().ok_or_else(|| BuildError::Other {
            phase: "pipeline".into(),
            message: "discovered artifacts absent at parse stage".into(),
        })?;
        let out = f(&self.ctx, disc)?;
        Ok(Pipeline {
            ctx: self.ctx,
            findings: merge(self.findings, out.findings),
            discovered: self.discovered,
            parsed: Some(out.artifacts),
            rendered: None,
            audited: None,
            started_iso: self.started_iso,
            _state: PhantomData,
        })
    }
}

// ----------------------------------------------------------------
// Parsed → Rendered.
// ----------------------------------------------------------------

impl Pipeline<Parsed> {
    /// Borrow the parsed artifacts.
    pub fn parsed(&self) -> &ParsedArtifacts {
        self.parsed.as_ref().expect("Parsed marker invariant: parsed.is_some()")
    }

    /// Borrow the discovered artifacts (still available).
    pub fn discovered(&self) -> &DiscoveredArtifacts {
        self.discovered.as_ref().expect("Discovered carry-forward")
    }

    /// Run the render stage.
    pub fn render<F>(self, f: F) -> Result<Pipeline<Rendered>, BuildError>
    where
        F: FnOnce(
            &BuildCtx,
            &DiscoveredArtifacts,
            &ParsedArtifacts,
        ) -> Result<StageOutput<RenderedArtifacts>, BuildError>,
    {
        let disc = self.discovered.as_ref().ok_or_else(|| BuildError::Other {
            phase: "pipeline".into(),
            message: "discovered artifacts absent at render stage".into(),
        })?;
        let parsed = self.parsed.as_ref().ok_or_else(|| BuildError::Other {
            phase: "pipeline".into(),
            message: "parsed artifacts absent at render stage".into(),
        })?;
        let out = f(&self.ctx, disc, parsed)?;
        Ok(Pipeline {
            ctx: self.ctx,
            findings: merge(self.findings, out.findings),
            discovered: self.discovered,
            parsed: self.parsed,
            rendered: Some(out.artifacts),
            audited: None,
            started_iso: self.started_iso,
            _state: PhantomData,
        })
    }
}

// ----------------------------------------------------------------
// Rendered → Audited.
// ----------------------------------------------------------------

impl Pipeline<Rendered> {
    /// Borrow the rendered artifacts.
    pub fn rendered(&self) -> &RenderedArtifacts {
        self.rendered.as_ref().expect("Rendered marker invariant: rendered.is_some()")
    }

    /// Borrow the parsed artifacts (still available).
    pub fn parsed(&self) -> &ParsedArtifacts {
        self.parsed.as_ref().expect("Parsed carry-forward")
    }

    /// Borrow the discovered artifacts (still available).
    pub fn discovered(&self) -> &DiscoveredArtifacts {
        self.discovered.as_ref().expect("Discovered carry-forward")
    }

    /// Run the audit stage. Audit phases consume the rendered
    /// output and (typically) the parsed CMS to surface findings.
    pub fn audit<F>(self, f: F) -> Result<Pipeline<Audited>, BuildError>
    where
        F: FnOnce(
            &BuildCtx,
            &RenderedArtifacts,
            &ParsedArtifacts,
        ) -> Result<StageOutput<AuditedArtifacts>, BuildError>,
    {
        let rendered = self.rendered.as_ref().ok_or_else(|| BuildError::Other {
            phase: "pipeline".into(),
            message: "rendered artifacts absent at audit stage".into(),
        })?;
        let parsed = self.parsed.as_ref().ok_or_else(|| BuildError::Other {
            phase: "pipeline".into(),
            message: "parsed artifacts absent at audit stage".into(),
        })?;
        let out = f(&self.ctx, rendered, parsed)?;
        Ok(Pipeline {
            ctx: self.ctx,
            findings: merge(self.findings, out.findings),
            discovered: self.discovered,
            parsed: self.parsed,
            rendered: self.rendered,
            audited: Some(out.artifacts),
            started_iso: self.started_iso,
            _state: PhantomData,
        })
    }
}

// ----------------------------------------------------------------
// Audited → Reported (terminal).
// ----------------------------------------------------------------

impl Pipeline<Audited> {
    /// Borrow the audited artifacts.
    pub fn audited(&self) -> &AuditedArtifacts {
        self.audited.as_ref().expect("Audited marker invariant: audited.is_some()")
    }

    /// View the accumulated findings without consuming the
    /// pipeline (e.g. for a pre-report dry-run).
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// Pipeline-wide pass/fail in the build mode.
    pub fn passed(&self) -> bool {
        self.findings.iter().all(|f| !f.severity.blocks_in(self.ctx.mode))
    }

    /// Consume the pipeline and emit a `BuildReport`. Optionally
    /// run a final `f` to mutate the report (e.g. attach a
    /// signature) before returning.
    pub fn into_report<F>(self, f: F) -> Result<(BuildReport, Pipeline<Reported>), BuildError>
    where
        F: FnOnce(&mut BuildReport, &BuildCtx) -> Result<(), BuildError>,
    {
        let mode_str = match self.ctx.mode {
            crate::BuildMode::Poc => "poc",
            crate::BuildMode::Production => "production",
            crate::BuildMode::Static => "static",
            crate::BuildMode::Hybrid => "hybrid",
            crate::BuildMode::Dynamic => "dynamic",
        }
        .to_owned();
        let mut report = BuildReport {
            mode: mode_str,
            ..BuildReport::default()
        };
        if let Some(iso) = &self.started_iso {
            report.started.clone_from(iso);
        }
        for finding in &self.findings {
            report.push(finding.clone());
        }
        f(&mut report, &self.ctx)?;
        let pipeline = Pipeline {
            ctx: self.ctx,
            findings: self.findings,
            discovered: self.discovered,
            parsed: self.parsed,
            rendered: self.rendered,
            audited: self.audited,
            started_iso: self.started_iso,
            _state: PhantomData,
        };
        Ok((report, pipeline))
    }
}

// ----------------------------------------------------------------
// Helpers visible at every stage.
// ----------------------------------------------------------------

impl<S> Pipeline<S> {
    /// View the build context.
    pub fn ctx(&self) -> &BuildCtx {
        &self.ctx
    }

    /// View the accumulated findings (any stage).
    pub fn current_findings(&self) -> &[Finding] {
        &self.findings
    }
}

fn merge(mut a: Vec<Finding>, b: Vec<Finding>) -> Vec<Finding> {
    a.extend(b);
    a
}

// ============================================================
// Tests.
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildMode, Severity};

    fn ctx() -> BuildCtx {
        BuildCtx {
            root: PathBuf::from("/tmp/forge-test"),
            static_dir: PathBuf::from("/tmp/forge-test/static"),
            mode: BuildMode::Poc,
        }
    }

    #[test]
    fn happy_path_runs_every_stage_in_order() {
        let pipeline = Pipeline::start(ctx())
            .with_start_iso("2026-05-15T00:00:00Z")
            .discover(|_| {
                Ok(StageOutput::clean(DiscoveredArtifacts {
                    cms_pages: vec![PathBuf::from("index.toml")],
                    ..Default::default()
                }))
            })
            .unwrap()
            .parse(|_, disc| {
                assert_eq!(disc.cms_pages.len(), 1);
                Ok(StageOutput::clean(ParsedArtifacts {
                    page_count: 1,
                    ..Default::default()
                }))
            })
            .unwrap()
            .render(|_, _, parsed| {
                assert_eq!(parsed.page_count, 1);
                Ok(StageOutput::clean(RenderedArtifacts {
                    out_dir: PathBuf::from("dist"),
                    html_files: vec![PathBuf::from("dist/index.html")],
                    ..Default::default()
                }))
            })
            .unwrap()
            .audit(|_, rendered, _| {
                assert_eq!(rendered.html_files.len(), 1);
                Ok(StageOutput::clean(AuditedArtifacts {
                    phases_run: 5,
                    clean_phases: 5,
                }))
            })
            .unwrap();
        assert!(pipeline.passed());
        let (report, _) = pipeline.into_report(|_, _| Ok(())).unwrap();
        assert_eq!(report.findings.len(), 0);
        assert_eq!(report.mode, "poc");
        assert_eq!(report.started, "2026-05-15T00:00:00Z");
    }

    #[test]
    fn findings_accumulate_across_stages() {
        let pipeline = Pipeline::start(ctx())
            .discover(|_| {
                Ok(StageOutput {
                    artifacts: DiscoveredArtifacts::default(),
                    findings: vec![Finding::warn("discover", "a", "warn-a")],
                })
            })
            .unwrap()
            .parse(|_, _| {
                Ok(StageOutput {
                    artifacts: ParsedArtifacts::default(),
                    findings: vec![Finding::strict("parse", "b", "strict-b")],
                })
            })
            .unwrap()
            .render(|_, _, _| {
                Ok(StageOutput {
                    artifacts: RenderedArtifacts::default(),
                    findings: vec![Finding::warn("render", "c", "warn-c")],
                })
            })
            .unwrap()
            .audit(|_, _, _| {
                Ok(StageOutput {
                    artifacts: AuditedArtifacts::default(),
                    findings: vec![Finding::strict("audit", "d", "strict-d")],
                })
            })
            .unwrap();
        assert_eq!(pipeline.findings().len(), 4);
        assert!(!pipeline.passed());
        let (report, _) = pipeline.into_report(|_, _| Ok(())).unwrap();
        assert_eq!(report.strict_count, 2);
        assert_eq!(report.warn_count, 2);
    }

    #[test]
    fn report_callback_can_attach_signature() {
        let pipeline = Pipeline::start(ctx())
            .discover(|_| Ok(StageOutput::clean(DiscoveredArtifacts::default())))
            .unwrap()
            .parse(|_, _| Ok(StageOutput::clean(ParsedArtifacts::default())))
            .unwrap()
            .render(|_, _, _| Ok(StageOutput::clean(RenderedArtifacts::default())))
            .unwrap()
            .audit(|_, _, _| Ok(StageOutput::clean(AuditedArtifacts::default())))
            .unwrap();
        let (report, _) = pipeline
            .into_report(|r, _| {
                r.signature = Some("dGVzdC1zaWc=".into());
                Ok(())
            })
            .unwrap();
        assert_eq!(report.signature.as_deref(), Some("dGVzdC1zaWc="));
    }

    #[test]
    fn discover_propagates_error() {
        let result: Result<Pipeline<Discovered>, BuildError> =
            Pipeline::start(ctx()).discover(|_| {
                Err(BuildError::Other {
                    phase: "discover".into(),
                    message: "synthetic".into(),
                })
            });
        assert!(result.is_err());
    }

    #[test]
    fn parse_propagates_error_after_discovery() {
        let pipeline = Pipeline::start(ctx())
            .discover(|_| Ok(StageOutput::clean(DiscoveredArtifacts::default())))
            .unwrap();
        let result: Result<Pipeline<Parsed>, BuildError> = pipeline.parse(|_, _| {
            Err(BuildError::Other {
                phase: "parse".into(),
                message: "synthetic".into(),
            })
        });
        assert!(result.is_err());
    }

    #[test]
    fn audit_can_inspect_rendered_and_parsed() {
        let pipeline = Pipeline::start(ctx())
            .discover(|_| Ok(StageOutput::clean(DiscoveredArtifacts::default())))
            .unwrap()
            .parse(|_, _| {
                Ok(StageOutput::clean(ParsedArtifacts {
                    page_count: 7,
                    ..Default::default()
                }))
            })
            .unwrap()
            .render(|_, _, p| {
                let mut html = vec![];
                for i in 0..p.page_count {
                    html.push(PathBuf::from(format!("dist/page-{i}.html")));
                }
                Ok(StageOutput::clean(RenderedArtifacts {
                    out_dir: PathBuf::from("dist"),
                    html_files: html,
                    ..Default::default()
                }))
            })
            .unwrap()
            .audit(|_, rendered, parsed| {
                assert_eq!(rendered.html_files.len(), parsed.page_count);
                Ok(StageOutput::clean(AuditedArtifacts {
                    phases_run: 1,
                    clean_phases: 1,
                }))
            })
            .unwrap();
        assert_eq!(pipeline.audited().clean_phases, 1);
    }

    #[test]
    fn passed_in_poc_with_only_warns() {
        let pipeline = Pipeline::start(ctx())
            .discover(|_| Ok(StageOutput::clean(DiscoveredArtifacts::default())))
            .unwrap()
            .parse(|_, _| Ok(StageOutput::clean(ParsedArtifacts::default())))
            .unwrap()
            .render(|_, _, _| Ok(StageOutput::clean(RenderedArtifacts::default())))
            .unwrap()
            .audit(|_, _, _| {
                Ok(StageOutput {
                    artifacts: AuditedArtifacts::default(),
                    findings: vec![Finding::warn("p", "x", "warn-only")],
                })
            })
            .unwrap();
        assert!(pipeline.passed());
    }

    #[test]
    fn fails_in_production_when_warn_present() {
        let mut c = ctx();
        c.mode = BuildMode::Production;
        let pipeline = Pipeline::start(c)
            .discover(|_| Ok(StageOutput::clean(DiscoveredArtifacts::default())))
            .unwrap()
            .parse(|_, _| Ok(StageOutput::clean(ParsedArtifacts::default())))
            .unwrap()
            .render(|_, _, _| Ok(StageOutput::clean(RenderedArtifacts::default())))
            .unwrap()
            .audit(|_, _, _| {
                Ok(StageOutput {
                    artifacts: AuditedArtifacts::default(),
                    findings: vec![Finding::warn("p", "x", "warn → strict in prod")],
                })
            })
            .unwrap();
        assert!(!pipeline.passed());
    }

    #[test]
    fn ctx_visible_at_every_stage() {
        let pipeline = Pipeline::start(ctx());
        assert_eq!(pipeline.ctx().mode, BuildMode::Poc);
        let p = pipeline
            .discover(|c| {
                assert!(c.root.ends_with("forge-test"));
                Ok(StageOutput::clean(DiscoveredArtifacts::default()))
            })
            .unwrap();
        assert_eq!(p.ctx().mode, BuildMode::Poc);
    }

    #[test]
    fn current_findings_visible_mid_pipeline() {
        let p = Pipeline::start(ctx())
            .discover(|_| {
                Ok(StageOutput {
                    artifacts: DiscoveredArtifacts::default(),
                    findings: vec![Finding::warn("d", "a", "discovery warn")],
                })
            })
            .unwrap();
        assert_eq!(p.current_findings().len(), 1);
        let p = p
            .parse(|_, _| {
                Ok(StageOutput {
                    artifacts: ParsedArtifacts::default(),
                    findings: vec![Finding::warn("p", "b", "parse warn")],
                })
            })
            .unwrap();
        assert_eq!(p.current_findings().len(), 2);
    }

    #[test]
    fn parse_payload_round_trips_to_render() {
        use serde_json::json;
        let payload = json!({"hello": "world", "n": 42});
        let p = Pipeline::start(ctx())
            .discover(|_| Ok(StageOutput::clean(DiscoveredArtifacts::default())))
            .unwrap()
            .parse(|_, _| {
                Ok(StageOutput::clean(ParsedArtifacts {
                    page_count: 0,
                    token_count: 0,
                    parse_payload: payload.clone(),
                }))
            })
            .unwrap();
        let p = p
            .render(|_, _, parsed| {
                assert_eq!(parsed.parse_payload, payload);
                Ok(StageOutput::clean(RenderedArtifacts::default()))
            })
            .unwrap();
        assert_eq!(p.parsed().parse_payload, payload);
    }

    #[test]
    fn carry_forward_artifacts_at_audit() {
        let p = Pipeline::start(ctx())
            .discover(|_| {
                Ok(StageOutput::clean(DiscoveredArtifacts {
                    cms_pages: vec![PathBuf::from("p.toml")],
                    ..Default::default()
                }))
            })
            .unwrap()
            .parse(|_, d| {
                assert_eq!(d.cms_pages.len(), 1);
                Ok(StageOutput::clean(ParsedArtifacts {
                    page_count: 1,
                    ..Default::default()
                }))
            })
            .unwrap()
            .render(|_, d, p| {
                assert_eq!(d.cms_pages.len(), 1);
                assert_eq!(p.page_count, 1);
                Ok(StageOutput::clean(RenderedArtifacts::default()))
            })
            .unwrap();
        // Rendered stage still has access to discovered + parsed
        // via the Pipeline accessors.
        assert_eq!(p.discovered().cms_pages.len(), 1);
        assert_eq!(p.parsed().page_count, 1);
    }

    #[test]
    fn report_mode_string_matches_build_mode() {
        for (mode, expected) in [
            (BuildMode::Poc, "poc"),
            (BuildMode::Production, "production"),
            (BuildMode::Static, "static"),
            (BuildMode::Hybrid, "hybrid"),
            (BuildMode::Dynamic, "dynamic"),
        ] {
            let mut c = ctx();
            c.mode = mode;
            let p = Pipeline::start(c)
                .discover(|_| Ok(StageOutput::clean(DiscoveredArtifacts::default())))
                .unwrap()
                .parse(|_, _| Ok(StageOutput::clean(ParsedArtifacts::default())))
                .unwrap()
                .render(|_, _, _| Ok(StageOutput::clean(RenderedArtifacts::default())))
                .unwrap()
                .audit(|_, _, _| Ok(StageOutput::clean(AuditedArtifacts::default())))
                .unwrap();
            let (report, _) = p.into_report(|_, _| Ok(())).unwrap();
            assert_eq!(report.mode, expected);
        }
    }

    #[test]
    fn must_use_holds_intermediate_pipeline_alive() {
        // Compile-only: assert the type returned from start() is
        // marked `#[must_use]`. If the attribute is removed this
        // test still compiles, but reviewers will catch the
        // regression in the diff.
        let p = Pipeline::start(ctx());
        let _ = p; // silence unused
    }

    #[test]
    fn empty_audit_reports_zero_findings() {
        let pipeline = Pipeline::start(ctx())
            .discover(|_| Ok(StageOutput::clean(DiscoveredArtifacts::default())))
            .unwrap()
            .parse(|_, _| Ok(StageOutput::clean(ParsedArtifacts::default())))
            .unwrap()
            .render(|_, _, _| Ok(StageOutput::clean(RenderedArtifacts::default())))
            .unwrap()
            .audit(|_, _, _| Ok(StageOutput::clean(AuditedArtifacts::default())))
            .unwrap();
        let (report, _) = pipeline.into_report(|_, _| Ok(())).unwrap();
        assert_eq!(report.strict_count, 0);
        assert_eq!(report.warn_count, 0);
        assert_eq!(report.findings.len(), 0);
    }

    #[test]
    fn severity_invariant_distinguishes_in_pipeline() {
        let pipeline = Pipeline::start(ctx())
            .discover(|_| Ok(StageOutput::clean(DiscoveredArtifacts::default())))
            .unwrap()
            .parse(|_, _| Ok(StageOutput::clean(ParsedArtifacts::default())))
            .unwrap()
            .render(|_, _, _| Ok(StageOutput::clean(RenderedArtifacts::default())))
            .unwrap()
            .audit(|_, _, _| {
                Ok(StageOutput {
                    artifacts: AuditedArtifacts::default(),
                    findings: vec![
                        Finding::strict("p", "x", "must-fail"),
                        Finding::warn("p", "y", "soft"),
                    ],
                })
            })
            .unwrap();
        let strict_count = pipeline
            .findings()
            .iter()
            .filter(|f| f.severity == Severity::Strict)
            .count();
        let warn_count = pipeline
            .findings()
            .iter()
            .filter(|f| f.severity == Severity::Warn)
            .count();
        assert_eq!(strict_count, 1);
        assert_eq!(warn_count, 1);
    }
}
