//! `ai-pipeline-core` — typed 6-stage AI generation pipeline.
//!
//! Stages:
//!
//! ```text
//!   Brief  →  IA  →  Wireframe  →  Content  →  Tokens  →  Audit
//! ```
//!
//! Each stage takes the prior stage's typed artifact + produces
//! the next. The pipeline is **resumable** + **inspectable** — at
//! every step you can serialize, hand off, replay, or branch.
//!
//! ### Why staged + typed
//!
//! Per `feedback_lfi_as_core_llm_as_peripheral`: LFI is the brain,
//! the LLM is a constrained candidate generator. The Critic gate
//! sits at the end. A typed staged pipeline:
//!
//!   * makes each AI call a SMALL bounded prompt (instead of
//!     "generate me a whole site")
//!   * makes the LFI critic point clear (it runs at Audit)
//!   * makes review hand-off trivial — operator can edit any
//!     stage's typed artifact and continue
//!   * makes failure modes explicit per stage instead of "the
//!     site looks bad and I don't know why"
//!
//! ### LFI scope
//!
//! Per `lfi-out-of-scope-for-this-instance`: this crate ships the
//! typed pipeline + state machine + trait. It does NOT contain
//! LLM or LFI integration code — those live in
//! `lfi-critic` + LLM-backend crates that paul's separate
//! instance owns. The [`Generator`] trait is the seam.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// The six pipeline stages, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    /// Operator's natural-language brief (the original prompt).
    Brief,
    /// Information architecture — page tree, routes, content
    /// outline.
    Ia,
    /// Wireframe — semantic block layout per page (hero, feature
    /// grid, CTA, etc., no copy yet).
    Wireframe,
    /// Copy + body text + alt text per block.
    Content,
    /// Design-system tokens — color palette, type scale, motion
    /// register matched to the brief.
    Tokens,
    /// Final audit — LFI Critic + super-society axes. PASS / FAIL
    /// + structured findings.
    Audit,
}

impl Stage {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Brief => "brief",
            Self::Ia => "ia",
            Self::Wireframe => "wireframe",
            Self::Content => "content",
            Self::Tokens => "tokens",
            Self::Audit => "audit",
        }
    }

    /// All stages in pipeline order.
    pub const ALL: &'static [Stage] = &[
        Self::Brief,
        Self::Ia,
        Self::Wireframe,
        Self::Content,
        Self::Tokens,
        Self::Audit,
    ];

    /// Next stage in the pipeline, or `None` if this is the
    /// final stage.
    pub fn next(&self) -> Option<Stage> {
        match self {
            Self::Brief => Some(Self::Ia),
            Self::Ia => Some(Self::Wireframe),
            Self::Wireframe => Some(Self::Content),
            Self::Content => Some(Self::Tokens),
            Self::Tokens => Some(Self::Audit),
            Self::Audit => None,
        }
    }
}

/// One stage's typed artifact.
///
/// Each variant carries the typed payload that stage emits. The
/// next-stage [`Generator`] reads this + emits the next variant.
/// Serializable so the operator can save/edit/resume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "kebab-case")]
pub enum Artifact {
    /// Brief — free-form natural-language prompt + optional
    /// declared constraints.
    Brief {
        /// The operator's natural-language description of the
        /// site.
        text: String,
        /// Optional declared constraints (locale, region,
        /// network targets, audience).
        #[serde(default)]
        constraints: Vec<String>,
    },
    /// Information architecture — flat list of pages with
    /// `(path, title, summary, children)` shape.
    Ia {
        /// Pages in tree order; consumers reconstruct depth from
        /// path segments.
        pages: Vec<IaPage>,
    },
    /// Wireframe — per-page list of semantic blocks.
    Wireframe {
        /// Wireframe per page, keyed by IaPage.path.
        per_page: Vec<WireframePage>,
    },
    /// Content — copy + alt-text per block.
    Content {
        /// Filled blocks per page, keyed by IaPage.path.
        per_page: Vec<ContentPage>,
    },
    /// Tokens — Loom-compatible token set + 8-axis aesthetic
    /// tuple slug.
    Tokens {
        /// Loom AestheticTuple slug (e.g. "swiss-editorial").
        aesthetic_pack: String,
        /// Operator-overridable token overrides (color / type /
        /// motion overrides applied on top of the pack).
        #[serde(default)]
        overrides: Vec<TokenOverride>,
    },
    /// Audit — final pass/fail + structured findings.
    Audit {
        /// True iff the LFI critic + super-society axes all PASS.
        pass: bool,
        /// One finding per axis or rule that failed.
        findings: Vec<AuditFinding>,
    },
}

impl Artifact {
    /// Which stage this artifact represents.
    pub fn stage(&self) -> Stage {
        match self {
            Self::Brief { .. } => Stage::Brief,
            Self::Ia { .. } => Stage::Ia,
            Self::Wireframe { .. } => Stage::Wireframe,
            Self::Content { .. } => Stage::Content,
            Self::Tokens { .. } => Stage::Tokens,
            Self::Audit { .. } => Stage::Audit,
        }
    }
}

/// One page in the information architecture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct IaPage {
    /// URL path of the page (e.g. `"/"`, `"/about"`).
    pub path: String,
    /// Page title.
    pub title: String,
    /// One-line summary of what the page is for.
    pub summary: String,
}

/// One semantic block in a wireframe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct WireframeBlock {
    /// Block kind (e.g. `"hero"`, `"feature-grid"`, `"cta"`).
    pub kind: String,
    /// Operator-readable hint for what the block displays.
    pub intent: String,
}

/// Wireframe for one page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct WireframePage {
    /// IaPage.path the wireframe corresponds to.
    pub path: String,
    /// Ordered blocks.
    pub blocks: Vec<WireframeBlock>,
}

/// Filled content for one block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ContentBlock {
    /// Block kind (mirrors WireframeBlock.kind).
    pub kind: String,
    /// Copy text for the block (markdown allowed).
    pub copy: String,
    /// Alt text for any image asset the block references.
    #[serde(default)]
    pub alt_text: String,
}

/// Filled content for one page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ContentPage {
    /// IaPage.path the content corresponds to.
    pub path: String,
    /// Filled blocks, in wireframe order.
    pub blocks: Vec<ContentBlock>,
}

/// One operator-supplied or AI-emitted token override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TokenOverride {
    /// Token name (e.g. `"--loom-color-accent"`).
    pub token: String,
    /// New value.
    pub value: String,
}

/// One audit finding (a fail on the super-society axes or LFI
/// critic).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AuditFinding {
    /// Rule slug (kebab-case).
    pub rule: String,
    /// Severity — `"info"`, `"warn"`, `"strict"`.
    pub severity: String,
    /// Operator-readable description.
    pub message: String,
    /// Optional pointer (page path, block id, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
}

/// The generator seam. Implementations live downstream:
///
///   * `ai-pipeline-llm`         — LLM-driven candidate generator
///   * `lfi-critic`              — LFI Critic gate at Audit stage
///   * a manual-edit "operator" generator that just echoes a
///     human-supplied artifact
///
/// This crate is async-runtime-agnostic — implementers choose
/// their own runtime. Returns `Box<dyn std::error::Error>` so
/// every backend can surface its own error type.
pub trait Generator {
    /// Produce the artifact for [`Stage::next`] given the
    /// current one. Returning `Ok(None)` means the pipeline
    /// terminates here cleanly (e.g. Audit pass).
    fn generate_next(
        &self,
        current: &Artifact,
    ) -> Result<Option<Artifact>, Box<dyn std::error::Error + Send + Sync>>;
}

/// Resumable pipeline state. Serialize → save → resume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Pipeline {
    /// Artifacts produced so far, in pipeline order.
    pub artifacts: Vec<Artifact>,
}

impl Pipeline {
    /// Start a new pipeline from an operator brief.
    pub fn from_brief(text: impl Into<String>, constraints: Vec<String>) -> Self {
        Self {
            artifacts: vec![Artifact::Brief {
                text: text.into(),
                constraints,
            }],
        }
    }

    /// The most recently produced artifact.
    pub fn current(&self) -> Option<&Artifact> {
        self.artifacts.last()
    }

    /// The stage we're currently waiting to enter (the next
    /// stage after the latest produced artifact). `None` once
    /// Audit is produced.
    pub fn next_stage(&self) -> Option<Stage> {
        self.current().and_then(|a| a.stage().next())
    }

    /// Append the next artifact to the pipeline. Refuses if the
    /// new artifact's stage doesn't match the expected next stage.
    pub fn advance(&mut self, artifact: Artifact) -> Result<(), PipelineError> {
        let expected = self.next_stage().ok_or(PipelineError::AlreadyComplete)?;
        let got = artifact.stage();
        if got != expected {
            return Err(PipelineError::StageMismatch { expected, got });
        }
        self.artifacts.push(artifact);
        Ok(())
    }

    /// Drive the pipeline forward by invoking `generator` to
    /// produce each subsequent stage's artifact. Stops when:
    ///   * the pipeline reaches Audit (or generator returns None)
    ///   * generator errors
    pub fn run_to_completion(&mut self, generator: &dyn Generator) -> Result<(), PipelineError> {
        loop {
            let next_stage = match self.next_stage() {
                Some(s) => s,
                None => return Ok(()), // complete
            };
            let current = self
                .current()
                .ok_or_else(|| PipelineError::StageMismatch {
                    expected: next_stage,
                    got: next_stage, // empty-pipeline case
                })?
                .clone();
            let produced = generator
                .generate_next(&current)
                .map_err(|e| PipelineError::Generator(e.to_string()))?;
            match produced {
                Some(a) => self.advance(a)?,
                None => return Ok(()),
            }
        }
    }

    /// Whether the pipeline has reached Audit + the Audit
    /// passed.
    pub fn is_passing(&self) -> bool {
        matches!(
            self.artifacts.last(),
            Some(Artifact::Audit { pass: true, .. })
        )
    }
}

/// Errors at the pipeline level.
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    /// Pipeline is already at Audit; can't advance further.
    #[error("pipeline already complete")]
    AlreadyComplete,
    /// Caller tried to append an artifact for a different stage
    /// than was expected.
    #[error("stage mismatch: expected {expected:?}, got {got:?}")]
    StageMismatch {
        /// Stage the pipeline expected next.
        expected: Stage,
        /// Stage the caller tried to append.
        got: Stage,
    },
    /// Generator implementation surfaced an error.
    #[error("generator: {0}")]
    Generator(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Echo generator that mechanically produces the next stage's
    /// empty artifact. Useful for tests + the operator-manual-edit
    /// path.
    struct EchoGenerator;
    impl Generator for EchoGenerator {
        fn generate_next(
            &self,
            current: &Artifact,
        ) -> Result<Option<Artifact>, Box<dyn std::error::Error + Send + Sync>> {
            let next = match current.stage().next() {
                Some(s) => s,
                None => return Ok(None),
            };
            let a = match next {
                Stage::Brief => unreachable!("Brief has no predecessor"),
                Stage::Ia => Artifact::Ia { pages: vec![] },
                Stage::Wireframe => Artifact::Wireframe { per_page: vec![] },
                Stage::Content => Artifact::Content { per_page: vec![] },
                Stage::Tokens => Artifact::Tokens {
                    aesthetic_pack: "swiss-editorial".into(),
                    overrides: vec![],
                },
                Stage::Audit => Artifact::Audit {
                    pass: true,
                    findings: vec![],
                },
            };
            Ok(Some(a))
        }
    }

    #[test]
    fn stage_order_is_correct() {
        assert_eq!(Stage::Brief.next(), Some(Stage::Ia));
        assert_eq!(Stage::Ia.next(), Some(Stage::Wireframe));
        assert_eq!(Stage::Wireframe.next(), Some(Stage::Content));
        assert_eq!(Stage::Content.next(), Some(Stage::Tokens));
        assert_eq!(Stage::Tokens.next(), Some(Stage::Audit));
        assert_eq!(Stage::Audit.next(), None);
    }

    #[test]
    fn stage_all_has_six_entries_in_order() {
        assert_eq!(Stage::ALL.len(), 6);
        assert_eq!(Stage::ALL[0], Stage::Brief);
        assert_eq!(Stage::ALL[5], Stage::Audit);
    }

    #[test]
    fn pipeline_from_brief_starts_at_brief() {
        let p = Pipeline::from_brief("a sock store", vec!["region:US".into()]);
        assert_eq!(p.artifacts.len(), 1);
        assert_eq!(p.current().unwrap().stage(), Stage::Brief);
        assert_eq!(p.next_stage(), Some(Stage::Ia));
    }

    #[test]
    fn advance_refuses_stage_mismatch() {
        let mut p = Pipeline::from_brief("x", vec![]);
        let err = p
            .advance(Artifact::Wireframe { per_page: vec![] })
            .unwrap_err();
        assert!(matches!(err, PipelineError::StageMismatch { .. }));
    }

    #[test]
    fn advance_accepts_correct_next_stage() {
        let mut p = Pipeline::from_brief("x", vec![]);
        p.advance(Artifact::Ia { pages: vec![] }).unwrap();
        assert_eq!(p.current().unwrap().stage(), Stage::Ia);
        assert_eq!(p.next_stage(), Some(Stage::Wireframe));
    }

    #[test]
    fn run_to_completion_drives_to_audit() {
        let mut p = Pipeline::from_brief("x", vec![]);
        p.run_to_completion(&EchoGenerator).unwrap();
        assert_eq!(p.artifacts.len(), 6);
        assert_eq!(p.current().unwrap().stage(), Stage::Audit);
        assert!(p.is_passing());
        assert!(p.next_stage().is_none());
    }

    #[test]
    fn advance_past_audit_errors() {
        let mut p = Pipeline::from_brief("x", vec![]);
        p.run_to_completion(&EchoGenerator).unwrap();
        let err = p
            .advance(Artifact::Audit {
                pass: false,
                findings: vec![],
            })
            .unwrap_err();
        assert!(matches!(err, PipelineError::AlreadyComplete));
    }

    #[test]
    fn artifact_serde_round_trips() {
        let a = Artifact::Tokens {
            aesthetic_pack: "brutalist".into(),
            overrides: vec![TokenOverride {
                token: "--loom-color-accent".into(),
                value: "hsl(180 100% 50%)".into(),
            }],
        };
        let s = serde_json::to_string(&a).unwrap();
        let back: Artifact = serde_json::from_str(&s).unwrap();
        assert_eq!(a, back);
    }

    #[test]
    fn artifact_serde_carries_stage_tag() {
        let a = Artifact::Brief {
            text: "hi".into(),
            constraints: vec![],
        };
        let s = serde_json::to_string(&a).unwrap();
        assert!(s.contains("\"stage\":\"brief\""));
    }

    #[test]
    fn pipeline_is_passing_only_when_audit_pass_true() {
        let mut p = Pipeline::from_brief("x", vec![]);
        p.run_to_completion(&EchoGenerator).unwrap();
        assert!(p.is_passing());

        // Replace the final audit with a failing one.
        p.artifacts.pop();
        p.artifacts.push(Artifact::Audit {
            pass: false,
            findings: vec![AuditFinding {
                rule: "wcag-aa-contrast".into(),
                severity: "strict".into(),
                message: "low contrast".into(),
                locator: Some("/".into()),
            }],
        });
        assert!(!p.is_passing());
    }

    #[test]
    fn pipeline_rejects_unknown_field() {
        let bad = r#"{"artifacts":[],"ahem":1}"#;
        let r: Result<Pipeline, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    // T97: slug-vs-serde-wire regression guard.
    #[test]
    fn slug_matches_serde_wire_across_all_enums() {
        for v in [
            Stage::Brief,
            Stage::Ia,
            Stage::Wireframe,
            Stage::Content,
            Stage::Tokens,
            Stage::Audit,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            assert_eq!(wire.trim_matches('"'), v.slug(), "{:?}", v);
        }
    }
}
