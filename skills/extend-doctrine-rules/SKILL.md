---
name: extend-doctrine-rules
description: Add a new rule to the AVP-Doctrine database. Covers schema, triple format, domain placement, lifecycle, citation downstream.
metadata:
  tags: [doctrine, governance, avp-2]
  related_doctrine_rules: [docs-003, docs-004]
  related_traits: []
---

# Extend the AVP-Doctrine rule database

Use this skill when a class of substrate concern needs to become codified doctrine — surfaced via `forge doctrine query`, enforced via the `Finding.citing()` chain, audited via `forge doctrine check`.

## When to invoke

Recognition signals:
- Repeated PR-review feedback covers the same kind of mistake → codify the standard as a rule.
- A new substrate capability introduces a new discipline (e.g. "every primitive must respect prefers-reduced-motion").
- An incident retrospective surfaces a missing rule.
- Paul (or doctrine review) explicitly asks for a rule on topic X.

Anti-signals:
- The standard is intrinsic to a typed surface (extend `*-core` validate() instead).
- The rule would apply to one specific case (capability-request issue is the right mechanism, not a rule).

## Prerequisites

1. Read `doctrine/rules/SCHEMA.md` in `PlausiDen-AVP-Doctrine` — the canonical rule shape.
2. Browse existing rules in the relevant domain (`forge doctrine query --domain <domain>`).
3. Identify the **enforcement mechanism** — without it, the rule is a wish, not a rule.

## Procedure

### 1. Choose the domain

Existing domains: `build` / `primitives` / `security` / `testing` / `docs` / `logging` / `perf` / `content` / `accessibility`.

If the new rule fits an existing domain, add it there. If it genuinely doesn't (rare), propose a new domain — that's a doctrine-schema change requiring its own ADR.

### 2. Pick the next id in sequence

In `doctrine/rules/<domain>.toml`, find the highest existing id (`prim-012`, `sec-010`, etc.) and use the next number. Ids are stable; once published, they don't change.

### 3. Author the triple

Append a `[[rule]]` table:

```toml
[[rule]]
id        = "<domain>-NNN"           # globally unique kebab + 3-digit numeric
name      = "Short label, Title Case"
domain    = "<domain>"                # must match file's [meta].domain
statement = "Precise one-sentence rule."
rationale = """
Multi-line block. Why this rule exists. What goes wrong without it.
Past incident references if applicable.
"""
enforcement = [
  "forge phase: <phase_name> — what it checks",
  "loom-lint refuses <pattern>",
  "crawler runtime: <axis_name> — what it verifies",
]
applies_to    = ["<crate or path>", "<entity class>"]
severity      = "strict"               # strict | warn | informational | experimental
lifecycle     = "experimental"         # experimental until trial period proves it
related_traits = ["<TraitName>"]       # cross-reference to trait system (optional)
references    = ["WCAG X.X.X", "RFC NNNN"]  # external authoritative references
```

The triple is NON-NEGOTIABLE: statement + rationale + enforcement. Per rule docs-003, incomplete rules fail parse.

### 4. Lifecycle choice

- `experimental` — being trialed; behaves like `warn` until promoted. Default for new rules.
- `stable` — binding doctrine; strict enforcement per severity. Promote via PR review.
- `deprecated` — being removed; requires `deprecated_at` + optional `replaced_by`.

### 5. Verify

```bash
cd PlausiDen-Forge
forge doctrine query --rule <your-rule-id>   # should now resolve
forge doctrine lifecycle                     # see your rule in the experimental list
forge doctrine check                         # ensures no orphan citations elsewhere
```

If the rule has corresponding Forge phase enforcement that doesn't exist yet, file a follow-up capability request to implement it.

### 6. Wire the citation downstream

When the rule has a Forge phase that emits findings on violation, wire `Finding.citing([your-rule-id])` in the phase's emission sites. Per rule docs-005, findings cite the rules they enforce.

Smoke test: trigger a violation, check the report shows `(your-rule-id)` suffix.

### 7. Commit

Per rule docs-007, AGENTS.md + TOOLS.md updates land in the same commit if the rule changes Claude-discoverable behavior.

Run `forge doctrine render --out docs/doctrine.md` to regenerate the rendered doctrine doc.

## Common pitfalls

| ❌ Don't | ✅ Do |
|---------|------|
| Author a rule without naming the enforcement mechanism | Without enforcement it's a wish (rule docs-003) |
| Author a rule without rationale | Future contributors can't judge edge cases (rule docs-003) |
| Promote experimental → stable without a trial period | Run the rule as experimental first; promote when enforcement is reliable |
| Mark a deprecated rule without `deprecated_at` or `replaced_by` | Parser rejects (`MissingDeprecatedAt` error) |
| Cite a rule id from code that doesn't exist | `forge doctrine check` will fail the build |
| Skip the cross-reference if a trait applies | `related_traits` field surfaces rules to consumers of those traits |
| Add a rule that overlaps with an existing rule | Refine the existing one instead; doctrine is canonical |

## Acceptance criteria

- [ ] New rule has complete triple (statement + rationale + enforcement)
- [ ] `domain` field matches the file's `[meta].domain`
- [ ] id follows the `<domain>-NNN` pattern, unique globally
- [ ] `severity` + `lifecycle` declared
- [ ] `applies_to` names specific paths / crates / entity classes
- [ ] `forge doctrine query --rule <id>` resolves
- [ ] `forge doctrine check` reports zero orphans (verify any new citations also resolve)
- [ ] `forge doctrine render` regenerated docs/doctrine.md if applicable
- [ ] Enforcement mechanism implemented OR follow-up capability request filed
- [ ] If `lifecycle = stable`: enforcement is proven via fixture / regression test

## Cross-references

- Schema: `PlausiDen-AVP-Doctrine/doctrine/rules/SCHEMA.md`
- Existing rules: `PlausiDen-AVP-Doctrine/doctrine/rules/*.toml`
- Doctrine parser: `crates/doctrine-core/src/lib.rs`
- Doctrine CLI: `crates/forge-cli/src/main.rs` (search for `DoctrineAction`)
- AVP-2 protocol: `PlausiDen-AVP-Doctrine/AVP2_PROTOCOL.md`
- Backward-compat discipline: rules are versioned + lifecycle-managed per `[[backward-compat-version-discipline]]`
