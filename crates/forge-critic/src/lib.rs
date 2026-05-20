//! `forge-critic` — the Critic seam for Forge's AI integration.
//!
//! Every AI proposal flows through:
//!
//! ```text
//!     Proposal ── Critic::evaluate ──► Decision
//! ```
//!
//! before reaching the commit boundary. There is no code path
//! where a raw LLM output reaches `commit()` — that's a type
//! signature, not a code-review policy.
//!
//! ## Implementations
//!
//! - [`NoopCritic`] — always returns `Accept`. Used for
//!   pipelines that don't have AI generation at all (operator-
//!   authored content flows through the same shape).
//! - [`LlmCritic`] — uses an [`LlmProvider`] (Claude, Gemini,
//!   GPT, local Llama, client-provided) as the evaluator.
//!   Bring-your-own-key, bring-your-own-model.
//! - **LFI-backed Critic** — lives in `Forge-LFI`. Built only
//!   with `cargo build --features lfi`. Same trait, neurosymbolic
//!   substrate underneath.
//!
//! ## Two-Forge architecture
//!
//! One codebase, swappable evaluator:
//!
//! ```bash
//! cargo build -p forge-cli                      # LlmCritic / NoopCritic
//! cargo build -p forge-cli --features lfi       # LfiCritic active
//! ```
//!
//! Migration day = zero LOC change in consumers.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod augmentable;

use serde::{Deserialize, Serialize};

/// Probabilistic strength clamped to [0.0, 1.0].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Strength(f32);

impl Strength {
    /// Construct from f32, clamped to [0.0, 1.0].
    pub fn new(v: f32) -> Self {
        Self(v.clamp(0.0, 1.0))
    }
    /// Full strength.
    pub const FULL: Self = Self(1.0);
    /// No strength.
    pub const NONE: Self = Self(0.0);
    /// Inner f32.
    pub fn get(&self) -> f32 {
        self.0
    }
}

/// Source that produced this proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProposalSource {
    /// Operator-authored.
    #[default]
    Operator,
    /// LLM-generated candidate.
    Llm,
    /// Pipeline-stage output.
    Pipeline,
    /// Imported from another source.
    Imported,
}

/// Evaluation context.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct ProposalContext {
    /// Tenant identifier.
    pub tenant_id: Option<String>,
    /// UI surface ("landing", "blog-post", etc.).
    pub surface: Option<String>,
    /// Origin.
    pub source: ProposalSource,
}

/// A proposal awaiting Critic evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Proposal {
    /// Kebab-case kind (e.g. `cms-section`, `meta-description`,
    /// `alt-text`, `design-tokens`).
    pub kind: String,
    /// Opaque payload — downstream Critic deserializes its own
    /// typed schema.
    pub payload: serde_json::Value,
    /// Context.
    #[serde(default)]
    pub context: ProposalContext,
}

/// Rule identifier (kebab-case slug).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuleId(String);

impl RuleId {
    /// Construct from any kebab-case slug.
    pub fn new(slug: impl Into<String>) -> Self {
        Self(slug.into())
    }
    /// Slug as &str.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A single violated rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Violation {
    /// Rule identifier.
    pub rule: RuleId,
    /// How strongly the rule fired.
    pub strength: Strength,
    /// Operator-facing explanation.
    pub explanation: String,
}

/// The Critic's typed verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Decision {
    /// Proposal accepted.
    Accept {
        /// Aggregate confidence.
        confidence: Strength,
        /// Rules that fired in favor.
        traced_rules_fired: Vec<(RuleId, Strength)>,
    },
    /// Proposal rejected; do not commit.
    Reject {
        /// One or more violations.
        violations: Vec<Violation>,
    },
    /// Resubmit with targeted regeneration guidance.
    Refine {
        /// Plain-language guidance for the proposer.
        targeted_regeneration_guidance: String,
        /// Rules that motivated the refinement.
        violated_rules: Vec<Violation>,
    },
}

impl Decision {
    /// Is this an Accept?
    pub fn is_accept(&self) -> bool {
        matches!(self, Decision::Accept { .. })
    }
}

/// The seam. Every commit path holds a `&dyn Critic` and calls
/// `.evaluate()` before persisting a proposal.
pub trait Critic: Send + Sync {
    /// Evaluate a proposal.
    fn evaluate(&self, proposal: &Proposal) -> Decision;
    /// Short identifier for audit logs.
    fn ident(&self) -> &'static str;
}

/// No-op Critic — always Accept. Pipelines with no AI still
/// dispatch through the seam so the call shape stays uniform.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopCritic;

impl Critic for NoopCritic {
    fn evaluate(&self, _proposal: &Proposal) -> Decision {
        Decision::Accept {
            confidence: Strength::FULL,
            traced_rules_fired: vec![],
        }
    }
    fn ident(&self) -> &'static str {
        "noop"
    }
}

/// Pluggable LLM provider. Async-runtime-agnostic — implementers
/// pick their own runtime (tokio, async-std, blocking client).
pub trait LlmProvider: Send + Sync {
    /// Run a prompt + return the completion text.
    ///
    /// The provider is responsible for retries, timeouts, and
    /// rate limiting; the Critic above just consumes the
    /// completion + parses a Decision.
    fn complete(&self, prompt: &str) -> Result<String, LlmError>;

    /// Provider identifier ("anthropic-claude-opus-4-7",
    /// "openai-gpt-5", "google-gemini-2-flash", "local-llama-3.1-70b").
    fn ident(&self) -> &'static str;
}

/// LLM call error.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    /// Provider returned non-OK.
    #[error("provider error: {0}")]
    Provider(String),
    /// Output didn't parse as a Decision.
    #[error("decision parse: {0}")]
    DecisionParse(String),
    /// Network / IO.
    #[error("transport: {0}")]
    Transport(String),
}

/// LLM-backed Critic. Calls the provider with a structured
/// prompt + parses the response back to a typed Decision.
pub struct LlmCritic<P: LlmProvider> {
    /// The underlying LLM provider.
    pub provider: P,
    /// System prompt that primes the model to return JSON
    /// matching the [`Decision`] shape.
    pub system_prompt: String,
}

impl<P: LlmProvider> LlmCritic<P> {
    /// Construct with the default system prompt.
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            system_prompt: Self::default_system_prompt(),
        }
    }

    /// The default system prompt. Instructs the model to return
    /// a JSON object matching `Decision`'s wire format.
    pub fn default_system_prompt() -> String {
        r#"You are a critic evaluating a proposal against quality + safety rules.
Respond ONLY with a JSON object matching this schema:

  {"kind": "accept", "confidence": 0.0..1.0, "traced_rules_fired": [["rule-slug", 0.5], ...]}
  {"kind": "reject", "violations": [{"rule": "slug", "strength": 0.7, "explanation": "..."}, ...]}
  {"kind": "refine", "targeted_regeneration_guidance": "...", "violated_rules": [...]}

No prose. No markdown fences. JSON only."#
            .into()
    }
}

impl<P: LlmProvider> Critic for LlmCritic<P> {
    fn evaluate(&self, proposal: &Proposal) -> Decision {
        let prompt = format!(
            "{}\n\nProposal:\n{}",
            self.system_prompt,
            serde_json::to_string_pretty(proposal).unwrap_or_default()
        );
        match self.provider.complete(&prompt) {
            Ok(text) => match serde_json::from_str::<Decision>(text.trim()) {
                Ok(d) => d,
                // Conservative fallback: when the model returns
                // un-parseable output, treat it as Refine with
                // diagnostic guidance rather than guessing.
                Err(e) => Decision::Refine {
                    targeted_regeneration_guidance: format!(
                        "LLM output did not match Decision schema: {e}. Raw output truncated: {}",
                        text.chars().take(200).collect::<String>()
                    ),
                    violated_rules: vec![Violation {
                        rule: RuleId::new("llm-decision-parse"),
                        strength: Strength::FULL,
                        explanation: "LLM did not produce parseable Decision JSON".into(),
                    }],
                },
            },
            Err(e) => Decision::Reject {
                violations: vec![Violation {
                    rule: RuleId::new("llm-provider-error"),
                    strength: Strength::FULL,
                    explanation: e.to_string(),
                }],
            },
        }
    }
    fn ident(&self) -> &'static str {
        // Provider's ident drives auditability; "llm" prefix
        // tells the audit log this came from the LLM path.
        "llm"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> Proposal {
        Proposal {
            kind: "cms-section".into(),
            payload: serde_json::json!({"hero": "demo"}),
            context: Default::default(),
        }
    }

    #[test]
    fn strength_clamps() {
        assert_eq!(Strength::new(-1.0).get(), 0.0);
        assert_eq!(Strength::new(99.0).get(), 1.0);
    }

    #[test]
    fn noop_always_accepts() {
        let c = NoopCritic;
        assert!(c.evaluate(&p()).is_accept());
        assert_eq!(c.ident(), "noop");
    }

    #[test]
    fn dyn_dispatch_works() {
        let critics: Vec<Box<dyn Critic>> = vec![Box::new(NoopCritic)];
        for c in &critics {
            assert!(c.evaluate(&p()).is_accept());
        }
    }

    /// Test LlmProvider with a canned response.
    struct StubProvider {
        response: String,
    }
    impl LlmProvider for StubProvider {
        fn complete(&self, _: &str) -> Result<String, LlmError> {
            Ok(self.response.clone())
        }
        fn ident(&self) -> &'static str {
            "stub"
        }
    }

    #[test]
    fn llm_critic_accepts_parsed_accept() {
        let c = LlmCritic::new(StubProvider {
            response: r#"{"kind":"accept","confidence":0.9,"traced_rules_fired":[]}"#.into(),
        });
        assert!(c.evaluate(&p()).is_accept());
    }

    #[test]
    fn llm_critic_refines_on_parse_error() {
        let c = LlmCritic::new(StubProvider {
            response: "blah blah not json".into(),
        });
        let d = c.evaluate(&p());
        assert!(matches!(d, Decision::Refine { .. }));
    }

    #[test]
    fn llm_critic_rejects_on_provider_error() {
        struct ErrProvider;
        impl LlmProvider for ErrProvider {
            fn complete(&self, _: &str) -> Result<String, LlmError> {
                Err(LlmError::Transport("test".into()))
            }
            fn ident(&self) -> &'static str {
                "err"
            }
        }
        let c = LlmCritic::new(ErrProvider);
        let d = c.evaluate(&p());
        assert!(matches!(d, Decision::Reject { .. }));
    }
}
