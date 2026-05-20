# AI_AUDIT.md

Audit of the PlausiDen-Forge workspace for AI-assuming code, refactored through trait abstractions where present. Per `[[deterministic-first-lfi-optional]]` + AVP-Doctrine `DETERMINISTIC_FIRST.md` § "The trait abstraction pattern": any AI augmentation routes through a typed trait seam; no calling code imports AI dependencies directly.

> Authored to close `#188 [determ-v5]`. Companion to `CAPABILITY_AI_POSTURE.md` (which capabilities have AI augmentation) + `CONFIG_SURFACE.md` (how to control them).

> **Audit verdict (as of HEAD): clean.** Zero AI-assuming code outside the `forge-critic` trait abstraction. The substrate is deterministic-first today; this doc captures the audit method + adds the guardrail to keep it that way.

---

## Audit method

The audit grepped the workspace for:

1. **Direct AI imports**: `use lfi_core::*` / `use llm_core::*` / `use anthropic::*` / `use openai::*` / `use ollama::*` / `use neupsl::*` / `use hdc::*` outside `crates/forge-critic`.

2. **AI environment variables**: `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / `OLLAMA_HOST` / `LFI_*_KEY` references.

3. **Cargo.toml dependencies**: any `[dependencies]` entry for `anthropic` / `openai` / `ollama` / `tokenizers` / `tiktoken` / `lfi-*` outside designated AI-integration crates.

4. **Hardcoded provider URLs**: `api.anthropic.com` / `api.openai.com` / etc. in source.

---

## Findings

```
=== source grep: AI provider imports outside forge-critic ===
(no results)

=== source grep: AI env vars in source ===
(no results)

=== Cargo.toml dependencies on AI integration crates ===
(no results)

=== Hardcoded AI provider URLs ===
(no results)
```

**The workspace is clean.** Every audit class came up empty.

---

## Existing trait abstraction

`forge-critic` implements the canonical Critic seam per `DETERMINISTIC_FIRST.md`:

```text
Proposal ── Critic::evaluate ──► Decision
```

- **`Critic` trait** — the contract. Methods return findings, recommendations, scores.
- **`NoopCritic`** — deterministic default. Always returns `Accept`. Used when AI is not configured.
- **`LlmCritic<P: LlmProvider>`** — AI-augmented variant. Generic over providers — the concrete provider type lives outside `forge-critic`.
- **`LlmProvider` trait** — provider seam. Concrete implementations (Anthropic / Gemini / Ollama / local) live in their own crates, isolated from calling code.
- **LFI-backed Critic** — lives downstream in `Forge-LFI`. Built only with `--features lfi`. Same trait, neurosymbolic implementation.

Properties:
- **Calling code interacts only through `Critic`** — never imports a concrete provider.
- **Default impl is deterministic** — `NoopCritic::evaluate(_) → Accept`. No AI call required.
- **AI augmentation is opt-in** — `LlmCritic` / `LfiCritic` require explicit construction; the runtime configuration determines which Critic is active per the 3-layer config surface (`CONFIG_SURFACE.md`).
- **Provider trait abstracts away differences** — Anthropic API vs OpenAI API vs Ollama local API vs Forge-LFI neurosymbolic eval all satisfy `LlmProvider`.

This is the architectural commitment that lets a sovereignty-conscious tenant disable AI entirely without breaking the platform.

---

## CI guardrail (extension to `determ-baseline.yml`)

The existing `determ-baseline.yml` workflow asserts the *binary* contains no AI symbols. This audit adds a complementary *source-grep* check that fails BEFORE bytes get compiled — protecting against accidental AI-import landing on a PR.

Added to the Scenario A job:

```yaml
- name: assert no AI imports outside forge-critic
  run: |
    set -euo pipefail
    FORBIDDEN_PATTERNS=(
      "use lfi_core"
      "use llm_core"
      "use anthropic::"
      "use openai::"
      "use ollama::"
      "use neupsl::"
      "use hdc::"
      "ANTHROPIC_API_KEY"
      "OPENAI_API_KEY"
      "OLLAMA_HOST"
    )
    fail=0
    for pat in "${FORBIDDEN_PATTERNS[@]}"; do
      # Permit only within forge-critic + tests (provider-trait + stub).
      hits=$(grep -rn "$pat" crates/ 2>/dev/null \
        | grep -v "crates/forge-critic" \
        | grep -v "tests/" || true)
      if [ -n "$hits" ]; then
        echo "::error::AI-assuming code outside forge-critic:"
        echo "$hits"
        fail=1
      fi
    done
    exit $fail
```

A future task lands the matching guardrail for AI-related `[dependencies]` Cargo.toml entries.

---

## Refactor history

No refactors were necessary — the substrate was deterministic-first from inception. The trait abstraction landed in `forge-critic` ahead of any concrete AI provider, so calling code never had a chance to import AI directly.

If a refactor target ever surfaces, the procedure is:

1. **Identify the caller** that imports AI directly.
2. **Move the imported function** into a `LlmProvider` impl (or appropriate trait impl).
3. **Reroute the caller** to consume through `Critic::evaluate` (or whichever trait seam fits).
4. **Verify** with `cargo build --workspace --no-default-features` — must succeed.
5. **Verify** with the CI guardrail — must show zero matches in the new code.

---

## Anti-patterns

| ❌ Don't | ✅ Do |
|---------|------|
| `use anthropic::Client;` directly in `forge-phases/src/foo.rs` | Use `forge_critic::Critic` trait; the concrete provider lives in a `LlmProvider` impl elsewhere |
| `std::env::var("ANTHROPIC_API_KEY")` anywhere outside an `LlmProvider` impl | Provider impls read their own credentials; calling code passes data through, never credentials |
| Add `[dependencies] openai = "..."` to a crate that isn't a provider impl | Provider deps land only in dedicated provider crates (PlausiDen-LFI / future llm-provider-* crates) |
| Conditional `#[cfg(feature = "lfi")] use lfi_core::*;` outside `forge-critic` | Feature-gating belongs to the provider crate; the trait stays unconditional |
| Special-case AI failure in calling code (`if let Err(AiError::Timeout) = ...`) | Calling code only sees `Decision::Accept` / `Decision::Refine` / `Decision::Reject`; AI errors are translated to `Decision` by the Critic impl, per fail-closed semantics in `CONFIG_SURFACE.md` |

---

## Cross-references

- `DETERMINISTIC_FIRST.md` — the architectural doctrine
- `CAPABILITY_AI_POSTURE.md` — D/A/P inventory per capability
- `CONFIG_SURFACE.md` — 3-layer config that drives Critic selection
- `forge-critic` source — Critic trait + LlmProvider trait + NoopCritic + LlmCritic
- `.github/workflows/determ-baseline.yml` — CI guardrail (binary + source)
- `[[deterministic-first-lfi-optional]]` memory — founding directive
