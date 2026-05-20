//! Generic augmentable-capability pattern per AVP-Doctrine
//! `DETERMINISTIC_FIRST.md` § "The trait abstraction pattern" +
//! `CAPABILITY_AI_POSTURE.md` (D/A/P inventory) +
//! `CONFIG_SURFACE.md` (3-layer config resolution).
//!
//! Every augmentable capability follows the same shape:
//!
//!   1. A typed contract trait (`Augmentable<I, O>`) that
//!      capability-owners implement for their (Input, Output) pair.
//!   2. A `Deterministic` impl — the baseline that always runs.
//!   3. An `Augmented` impl — the AI/LFI-enriched variant.
//!   4. A `Composite<D, A>` wrapper that runs both when AI is
//!      enabled, returning enriched output, OR runs only the
//!      deterministic side and returns its output untouched.
//!   5. An `Auto<D, A>` wrapper that picks the active mode at
//!      invocation time based on the resolved AI posture
//!      (per `CONFIG_SURFACE.md` 3-layer config).
//!
//! Fail-closed semantics: when the Augmented impl errors or
//! returns nothing useful, the wrapper falls back to the
//! Deterministic output silently (per
//! `[[deterministic-first-lfi-optional]]`). The platform never
//! errors due to AI unavailability; it operates at the baseline.
//!
//! This module provides the generic pattern types. Concrete
//! capabilities (originality scoring, content drift detection,
//! recommendation, etc.) implement `Augmentable` for their own
//! types and use `Auto<D, A>` or `Composite<D, A>` as wiring.
//!
//! Closes `#186 [determ-v3]`.

use serde::{Deserialize, Serialize};

/// Resolved AI mode for a single invocation. Matches the
/// `AiMode` enum sketched in `CONFIG_SURFACE.md` § Operation-level.
/// Computed by the 3-layer config resolver; passed in by calling
/// code per invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AiMode {
    /// Use the deterministic baseline only. Default whenever
    /// config is missing, malformed, or explicitly disabled.
    Deterministic,
    /// Use LFI augmentation when available. Per
    /// `[[deterministic-first-lfi-optional]]`: still falls back
    /// to deterministic if the LFI impl errors.
    Lfi,
    /// Use LLM augmentation when available. Same fail-closed
    /// semantics.
    Llm,
    /// Use whichever augmentation the tenant/platform config
    /// selected. The substrate resolves this to `Deterministic`,
    /// `Lfi`, or `Llm` before dispatch.
    Auto,
    /// Explicitly disabled — alias for `Deterministic` but
    /// signals operator opt-out rather than absence of config.
    Off,
}

impl AiMode {
    /// True if the mode permits AI augmentation. `Deterministic`
    /// and `Off` return false; `Lfi`, `Llm`, `Auto` return true.
    /// Used by `Auto::evaluate` to short-circuit to baseline.
    #[must_use]
    pub fn allows_augmentation(self) -> bool {
        matches!(self, Self::Lfi | Self::Llm | Self::Auto)
    }

    /// Canonical slug for audit-chain entries + JSON serialization.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::Lfi => "lfi",
            Self::Llm => "llm",
            Self::Auto => "auto",
            Self::Off => "off",
        }
    }
}

/// The generic capability contract. Implementers wire a typed
/// `(Input, Output)` pair plus a deterministic `evaluate`. Both
/// the deterministic baseline and the AI-augmented variant
/// satisfy this trait — the wrapper types (`Auto`, `Composite`)
/// dispatch between them.
pub trait Augmentable<I, O>: Send + Sync {
    /// Run the capability on `input`, producing `output`. Per
    /// `[[deterministic-first-lfi-optional]]`: implementations
    /// MUST be deterministic given their input (no time-of-day,
    /// no AI-runtime variance). Augmented impls treat AI
    /// invocation failure as "no enrichment" and return the
    /// shape they'd return without AI — never panic, never error.
    fn evaluate(&self, input: &I) -> O;

    /// Short identifier for audit logs ("originality.deterministic",
    /// "originality.lfi", "originality.llm").
    fn ident(&self) -> &'static str;
}

/// Wrapper that picks between deterministic and augmented impls
/// based on the active `AiMode`. The composition is fail-closed:
/// `Auto::evaluate` only invokes the augmented side when the
/// mode allows; if anything goes wrong inside the augmented impl,
/// the impl is expected to return its baseline-equivalent output
/// (per the trait's contract).
pub struct Auto<I, O, D, A>
where
    D: Augmentable<I, O>,
    A: Augmentable<I, O>,
{
    deterministic: D,
    augmented: A,
    mode: AiMode,
    // `fn() -> *const (I, O)` is unconditionally Send + Sync;
    // wrapping in PhantomData lets the type parameters appear in
    // the struct signature without requiring Send/Sync on the
    // wrapped types themselves.
    _types: std::marker::PhantomData<fn() -> *const (I, O)>,
}

impl<I, O, D, A> Auto<I, O, D, A>
where
    D: Augmentable<I, O>,
    A: Augmentable<I, O>,
{
    /// Construct an `Auto` wrapper with the given deterministic +
    /// augmented impls + a runtime-resolved mode.
    pub fn new(deterministic: D, augmented: A, mode: AiMode) -> Self {
        Self {
            deterministic,
            augmented,
            mode,
            _types: std::marker::PhantomData,
        }
    }

    /// Replace the resolved mode (e.g. when the operator-level
    /// override differs from the tenant default). Returns the
    /// updated wrapper for chained construction.
    #[must_use]
    pub fn with_mode(mut self, mode: AiMode) -> Self {
        self.mode = mode;
        self
    }

    /// The active mode for this invocation.
    #[must_use]
    pub fn mode(&self) -> AiMode {
        self.mode
    }
}

impl<I, O, D, A> Augmentable<I, O> for Auto<I, O, D, A>
where
    D: Augmentable<I, O>,
    A: Augmentable<I, O>,
{
    fn evaluate(&self, input: &I) -> O {
        if self.mode.allows_augmentation() {
            self.augmented.evaluate(input)
        } else {
            self.deterministic.evaluate(input)
        }
    }

    fn ident(&self) -> &'static str {
        "auto"
    }
}

/// Wrapper that runs BOTH deterministic + augmented impls and
/// returns the augmented output. Used when the augmented impl's
/// output strictly enriches the deterministic output (e.g. adds
/// semantic-similarity scores alongside structural ones). The
/// deterministic call still runs — its result can be compared
/// against the augmented one at audit time.
///
/// Different from `Auto`: `Composite` always runs both; `Auto`
/// picks one based on mode.
pub struct Composite<I, O, D, A>
where
    D: Augmentable<I, O>,
    A: Augmentable<I, O>,
{
    deterministic: D,
    augmented: A,
    // `fn() -> *const (I, O)` is unconditionally Send + Sync;
    // wrapping in PhantomData lets the type parameters appear in
    // the struct signature without requiring Send/Sync on the
    // wrapped types themselves.
    _types: std::marker::PhantomData<fn() -> *const (I, O)>,
}

impl<I, O, D, A> Composite<I, O, D, A>
where
    D: Augmentable<I, O>,
    A: Augmentable<I, O>,
{
    /// Construct a `Composite` with deterministic + augmented impls.
    pub fn new(deterministic: D, augmented: A) -> Self {
        Self {
            deterministic,
            augmented,
            _types: std::marker::PhantomData,
        }
    }

    /// Run both impls and return both outputs. Callers compare
    /// them; useful for audit + debugging the augmentation layer.
    pub fn evaluate_both(&self, input: &I) -> (O, O) {
        (
            self.deterministic.evaluate(input),
            self.augmented.evaluate(input),
        )
    }
}

impl<I, O, D, A> Augmentable<I, O> for Composite<I, O, D, A>
where
    D: Augmentable<I, O>,
    A: Augmentable<I, O>,
{
    /// `Composite::evaluate` returns the augmented output (since
    /// `Composite` is used precisely when augmented enriches the
    /// deterministic side). Use `evaluate_both` to get both
    /// outputs separately.
    fn evaluate(&self, input: &I) -> O {
        // Still call deterministic for side-effects (audit hooks,
        // counters in implementations that wire them). The
        // returned value is the augmented output.
        let _baseline = self.deterministic.evaluate(input);
        self.augmented.evaluate(input)
    }

    fn ident(&self) -> &'static str {
        "composite"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Reference capability for testing the pattern ------------
    //
    // Per AVP-Doctrine `CAPABILITY_AI_POSTURE.md` § CMS authoring:
    // "Originality / similarity scoring — A (augmentable)."
    // The deterministic impl counts unique word tokens (structural
    // similarity proxy). The augmented impl returns a richer
    // score that includes a semantic similarity component (here:
    // a stub that adds a fixed bonus).
    // -------------------------------------------------------------

    /// Stub input for the reference capability — a string buffer.
    #[derive(Debug, Clone)]
    struct CorpusEntry(String);

    /// Stub output — a similarity score plus a label for which
    /// engine produced it.
    #[derive(Debug, Clone, PartialEq)]
    struct OriginalityScore {
        unique_token_ratio: f32,
        engine: &'static str,
    }

    /// Deterministic baseline.
    struct DeterministicOriginality;
    impl Augmentable<CorpusEntry, OriginalityScore> for DeterministicOriginality {
        fn evaluate(&self, input: &CorpusEntry) -> OriginalityScore {
            let tokens: Vec<&str> = input.0.split_whitespace().collect();
            let total = tokens.len().max(1) as f32;
            let unique: std::collections::HashSet<_> = tokens.iter().copied().collect();
            OriginalityScore {
                unique_token_ratio: unique.len() as f32 / total,
                engine: "deterministic",
            }
        }
        fn ident(&self) -> &'static str {
            "originality.deterministic"
        }
    }

    /// Augmented stub. Returns the same structural shape PLUS a
    /// fixed semantic-similarity adjustment.
    struct LfiOriginality;
    impl Augmentable<CorpusEntry, OriginalityScore> for LfiOriginality {
        fn evaluate(&self, input: &CorpusEntry) -> OriginalityScore {
            let baseline = DeterministicOriginality.evaluate(input);
            OriginalityScore {
                unique_token_ratio: (baseline.unique_token_ratio + 0.1).min(1.0),
                engine: "lfi",
            }
        }
        fn ident(&self) -> &'static str {
            "originality.lfi"
        }
    }

    fn entry(s: &str) -> CorpusEntry {
        CorpusEntry(s.to_string())
    }

    #[test]
    fn ai_mode_slug_canonical() {
        assert_eq!(AiMode::Deterministic.slug(), "deterministic");
        assert_eq!(AiMode::Lfi.slug(), "lfi");
        assert_eq!(AiMode::Llm.slug(), "llm");
        assert_eq!(AiMode::Auto.slug(), "auto");
        assert_eq!(AiMode::Off.slug(), "off");
    }

    #[test]
    fn ai_mode_allows_augmentation_per_spec() {
        assert!(!AiMode::Deterministic.allows_augmentation());
        assert!(!AiMode::Off.allows_augmentation());
        assert!(AiMode::Lfi.allows_augmentation());
        assert!(AiMode::Llm.allows_augmentation());
        assert!(AiMode::Auto.allows_augmentation());
    }

    #[test]
    fn auto_in_deterministic_mode_uses_baseline() {
        let auto = Auto::new(
            DeterministicOriginality,
            LfiOriginality,
            AiMode::Deterministic,
        );
        let r = auto.evaluate(&entry("a b c d a"));
        assert_eq!(r.engine, "deterministic");
    }

    #[test]
    fn auto_in_lfi_mode_uses_augmented() {
        let auto = Auto::new(
            DeterministicOriginality,
            LfiOriginality,
            AiMode::Lfi,
        );
        let r = auto.evaluate(&entry("a b c d a"));
        assert_eq!(r.engine, "lfi");
    }

    #[test]
    fn auto_in_off_mode_uses_baseline_fail_closed() {
        let auto = Auto::new(
            DeterministicOriginality,
            LfiOriginality,
            AiMode::Off,
        );
        let r = auto.evaluate(&entry("a b c d a"));
        // Off is alias for Deterministic — never invokes LFI even
        // though it's provided.
        assert_eq!(r.engine, "deterministic");
    }

    #[test]
    fn auto_with_mode_chain_replaces_mode() {
        let auto = Auto::new(
            DeterministicOriginality,
            LfiOriginality,
            AiMode::Deterministic,
        );
        assert_eq!(auto.mode(), AiMode::Deterministic);
        let auto = auto.with_mode(AiMode::Lfi);
        assert_eq!(auto.mode(), AiMode::Lfi);
        let r = auto.evaluate(&entry("a b c d a"));
        assert_eq!(r.engine, "lfi");
    }

    #[test]
    fn composite_runs_both_and_returns_augmented() {
        let comp = Composite::new(DeterministicOriginality, LfiOriginality);
        let r = comp.evaluate(&entry("a b c d a"));
        // Composite always picks the augmented side as the
        // returned value (the deterministic side ran too, just
        // discarded by `evaluate`).
        assert_eq!(r.engine, "lfi");
    }

    #[test]
    fn composite_evaluate_both_returns_both_outputs() {
        let comp = Composite::new(DeterministicOriginality, LfiOriginality);
        let (det, aug) = comp.evaluate_both(&entry("a b c d a"));
        assert_eq!(det.engine, "deterministic");
        assert_eq!(aug.engine, "lfi");
        // Augmented adds 0.1 to the ratio per the stub impl.
        assert!((aug.unique_token_ratio - (det.unique_token_ratio + 0.1)).abs() < 1e-6);
    }

    #[test]
    fn augmentable_trait_dyn_dispatchable() {
        // The trait is generic in I,O but works under dyn for
        // monomorphized I,O — useful when calling code wants a
        // boxed capability per resolved config.
        let baseline = DeterministicOriginality;
        let _: &dyn Augmentable<CorpusEntry, OriginalityScore> = &baseline;
    }
}
