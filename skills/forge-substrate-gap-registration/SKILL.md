---
name: forge-substrate-gap-registration
description: Register a substrate-capability gap (missing primitive, audit, theme, page kind, page field, doctrine rule, tooling) into the canonical gap registry. Workflow #9 of the paired skill+MCP series (#372). Substrate-reframe doctrine: don't route around gaps; register them.
metadata:
  tags: [forge, substrate, gap, reframe, doctrine]
  related_doctrine_rules: [substrate_only_path, substrate_reframe_2026_05_21, agent_workflows_must_be_paired]
  related_traits: []
  paired_mcp_tool: forge.substrate_gap_registration
  workflow_status: paired
---

# Register a substrate gap

Use this skill the moment an operator (or AI agent) encounters a substrate-capability gap: the substrate doesn't have the primitive / phase / theme / page kind / page field / doctrine rule / tooling needed. The reframe is explicit: **don't route around the gap by hand-authoring outside the substrate path**. Register it so the substrate can grow.

## When to invoke

Recognition signals:
- "The substrate doesn't have a way to..." — that's a gap
- A primitive that almost-but-not-quite fits — gap
- A finding that should fire but no audit phase covers it — gap
- A site needs a theme register the substrate doesn't ship — gap
- A site needs a PageKind that the closed enum doesn't include — gap
- A doctrine question that the current rule set doesn't answer — gap
- A workflow that should be automatable but no tool exists — gap

Anti-signals:
- The gap is "I want to copy a competitor's site" — that's a content-reuse issue (#369), not a substrate gap
- The gap is "this primitive should look slightly different" — that's a primitive modification (#368), not a missing primitive
- The "gap" is actually an existing primitive the operator doesn't know exists — query `forge.add_primitive` (#366) first

## Prerequisites

Before registering, run these checks (in order):

1. **`forge.docs.query`** with relevant tags — the gap may be addressed by existing doc-query entries
2. **`forge.add_primitive`** with the proposed primitive name — surfaces near-duplicates in the existing primitive set
3. **`forge.workflows.list`** — surfaces existing paired workflows
4. **Confirm the gap is substrate-general** per prim-012 — site-specific needs belong in tenant-corpora, not the substrate

If all four return "no, the substrate really doesn't have this", register the gap.

## Procedure

### 1. Call the paired MCP tool

```jsonc
{
  "name": "forge.substrate_gap_registration",
  "arguments": {
    "registry_path": "/home/paul/projects/PlausiDen-Forge/substrate-gap-registry.jsonl",
    "kind": "primitive",
    "observed_in": "tenant-acme",
    "summary": "Need CmsSection::ComicStrip for illustration-heavy editorial",
    "proposed_resolution": "Add ComicStrip per docs/SUBSTRATE_DECORATIVE_AUDIT_2026_05_27.md Tier 2 #4",
    "related_tasks": ["#359"]
  }
}
```

Required:
- `registry_path` — absolute path to the JSONL registry (canonical: `/home/paul/projects/PlausiDen-Forge/substrate-gap-registry.jsonl`)
- `kind` — one of: `primitive` / `audit_phase` / `theme` / `page_kind` / `page_field` / `doctrine_rule` / `tooling`
- `observed_in` — tenant ID or URL where the gap was observed
- `summary` — one-line description
- `proposed_resolution` — proposed substrate change

Optional:
- `related_tasks` — array of task IDs that reference or unblock this gap

### 2. Review the assigned ID

The tool returns the assigned gap ID (sequential, 1-based). Note this ID for cross-reference in commits, doctrine docs, audit findings.

### 3. Update related task tracker

If the gap maps to an unfiled task, file one referencing the gap ID. If it maps to an existing task, append the gap ID to that task's metadata.

## Status lifecycle

Gaps progress through:

| Status | When |
|---|---|
| `open` | Just registered, not triaged |
| `accepted` | Triaged + confirmed as real substrate gap |
| `in_progress` | Implementation underway |
| `shipped` | Substrate change merged + tested |
| `rejected` | Reviewed and declined (out-of-band, duplicate, infeasible) |

Status transitions are appended as new entries with the same `id`; the registry is append-only. The latest revision wins.

## Common pitfalls

### Pitfall 1: Registering site-specific gaps

A tenant wants `AcmeTimelineHero`. That's not a substrate gap — that's tenant-specific naming. Per prim-012, register only substrate-general shapes (`TimelineHero` could be substrate; `AcmeTimelineHero` is not).

### Pitfall 2: Skipping the four prerequisite checks

Registering a gap that already has an answer in `doc_query` wastes everyone's time. Always run the four prerequisites first.

### Pitfall 3: Vague `proposed_resolution`

"Make the substrate handle this better" is not a proposal. Concrete: "Add `CmsSection::ServiceStatus` with `state: ServiceState` field". Tight proposals let triage decide accept/reject quickly.

### Pitfall 4: Registering without filing a related task

A gap that has zero corresponding task entry will not be worked. The registry is the WHAT; tasks are the WHEN. Register both.

## Acceptance criteria

1. ✓ All four prerequisite checks returned "no existing answer"
2. ✓ Gap registered with valid kind + non-empty summary + concrete proposal
3. ✓ Assigned gap ID noted in related task / commit
4. ✓ Registry remains valid JSONL (each line parses as one GapEntry)

## Mapping to substrate

- **MCP tool**: `forge.substrate_gap_registration`
- **Backing module**: `forge-core::gap_registry`
- **Doctrine rules**: `substrate_only_path` (the deeper reason), `substrate_reframe_2026_05_21` (the doctrine source), `agent_workflows_must_be_paired`
- **Related skills**: `forge-add-primitive` (#366), `forge-add-audit-phase` (#367), `extend-doctrine-rules`
- **Registry format**: JSONL; one `GapEntry` per line; append-only
