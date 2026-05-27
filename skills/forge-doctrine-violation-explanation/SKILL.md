---
name: forge-doctrine-violation-explanation
description: When an audit phase flags a doctrine violation, explain the rule, its rationale, and concrete remediation. Workflow #10 of the paired skill+MCP series (#373).
metadata:
  tags: [forge, doctrine, audit, remediation]
  related_doctrine_rules: [agent_workflows_must_be_paired]
  related_traits: []
  paired_mcp_tool: forge.doctrine_violation_explanation
  workflow_status: paired
---

# Explain a doctrine violation

Use this skill when an audit phase fires a finding and the operator/agent doesn't understand why the rule exists or how to fix the underlying issue. The tool decouples "the rule was violated" (the finding) from "here's why + here's how to comply" (the explanation).

## When to invoke

Recognition signals:
- A `forge build` finding cites a doctrine rule the operator/agent doesn't recognize
- The audit explains *what* failed but not *why* the rule exists
- The operator is tempted to add an `[strict.exempt]` entry without understanding what they're exempting

Anti-signals:
- The finding is mechanical (a typo, a missing file) — fix it directly; doctrine explanation isn't needed
- The rule is well-known to the operator — skip the lookup
- The intent is to register a NEW rule — use `extend-doctrine-rules` skill instead

## Prerequisites

1. **Have the rule ID handy.** Audit findings cite the rule ID as e.g. `prim-012` or `build-001`.
2. **(Optional) Have the violating path.** Helps the explanation surface which scope the rule applies to.

## Procedure

### 1. Call the paired MCP tool

```jsonc
{
  "name": "forge.doctrine_violation_explanation",
  "arguments": {
    "rule_id": "prim-012",
    "violating_path": "crates/loom-cms-render/src/lib.rs"
  }
}
```

Required:
- `rule_id` — the rule slug cited by the audit finding

Optional:
- `violating_path` — path that triggered the finding; threaded into the response for context

### 2. Read the explanation

The tool returns:
- **Rule statement** — the canonical text
- **Rationale** — why the rule exists (often a past incident or design constraint)
- **Remediation category** — one of: `mechanical_fix` (rename/add field), `content_change` (edit copy), `structural_redesign` (rework section), `escalate` (substrate gap, register via #372)
- **Concrete remediation steps** — what to actually do

### 3. Apply remediation

Per the category:
- `mechanical_fix`: edit the cited file and re-run `forge build`
- `content_change`: edit the cited cms/*.json content; re-run
- `structural_redesign`: usually means a different primitive or composition is needed; consider `forge.modify_site` (#365) or `forge.modify_primitive` (#368)
- `escalate`: the rule reveals a substrate gap; call `forge.substrate_gap_registration` (#372)

## Why this skill exists

Two failure modes it closes:

1. **Cargo-cult exemption**: operators see a finding, don't understand the rule, add `[strict.exempt]` to silence it, and ship code that violates the original spirit. The explanation surfaces the rule's *why*; with that, exemption becomes a deliberate choice rather than a reflex.

2. **Substrate-gap masking**: some "violations" reveal that the substrate's rule set is incomplete. Surfacing the remediation category lets the operator distinguish "I should fix my code" from "the substrate's doctrine should grow".

## Common pitfalls

### Pitfall 1: Treating rationale as bureaucracy

The rationale is the load-bearing part. Skipping it produces code that satisfies the letter of the rule and breaks its spirit. Read every rationale line.

### Pitfall 2: Auto-applying remediation

Some remediations require operator judgment ("restructure the section list"). Don't auto-apply structural changes; surface the recommendation and let the operator decide.

### Pitfall 3: Repeated exemptions for the same rule

If the same rule is being exempted across 5+ tenants, that's a signal the rule itself needs revision (or the substrate has a gap). After 5 exemptions, file a `forge.substrate_gap_registration` (#372) with `kind: doctrine_rule`.

## Acceptance criteria

1. ✓ Rule lookup returned a non-empty statement + rationale
2. ✓ Remediation category identified
3. ✓ Operator applied the per-category remediation (or registered as gap if `escalate`)
4. ✓ Re-running `forge build` confirms the finding cleared

## Mapping to substrate

- **MCP tool**: `forge.doctrine_violation_explanation`
- **Backing**: forge subcommand `forge doctrine for <path>` (existing); future iteration will query the doctrine database directly via `forge-core::doctrine`
- **Related skills**: `forge-substrate-gap-registration` (#372), `extend-doctrine-rules`, every audit-driven workflow
- **Doctrine rules**: `agent_workflows_must_be_paired`
