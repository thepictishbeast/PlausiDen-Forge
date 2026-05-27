---
name: forge-skill-invocation-meta
description: Entry-point meta-skill — given a task description, surface the matching forge workflow(s). Workflow #11 of the paired skill+MCP series (#374). Solves the "which workflow do I invoke?" first-step problem.
metadata:
  tags: [forge, meta, workflow, orientation]
  related_doctrine_rules: [agent_workflows_must_be_paired]
  related_traits: []
  paired_mcp_tool: forge.skill_invocation_meta
  workflow_status: paired
---

# Meta-skill: which workflow applies?

Use this skill when an operator (or AI agent) has a task in mind and doesn't know which `forge.*` workflow to invoke. It's the first step into the workflow series for any non-trivial substrate operation.

## When to invoke

Recognition signals:
- "I need to do X with the substrate" and X doesn't obviously map to one workflow
- Multi-axis tasks that might span several workflows
- A new operator unfamiliar with the workflow inventory
- An AI agent orienting on a new substrate task

Anti-signals:
- The workflow is already known and the operator can name it directly — skip the meta and invoke
- The task is non-substrate (e.g. server config, infra) — out of scope for this skill series

## Prerequisites

None. This skill IS the orientation step; everything else is downstream.

## Procedure

### 1. Call the paired MCP tool with a task description

```jsonc
{
  "name": "forge.skill_invocation_meta",
  "arguments": {
    "task_description": "I want to swap the theme of an existing tenant to editorial"
  }
}
```

Required:
- `task_description` — a freeform sentence or two about what the operator wants to accomplish

### 2. Review the candidate workflows

The tool returns a ranked list of workflow candidates with confidence scores. Each candidate carries:
- `slug` — workflow slug
- `score` — relevance heuristic (higher = better match)
- `skill_dir` — path to the SKILL.md
- `mcp_tool` — paired MCP tool name
- `match_reasons` — which tokens / patterns triggered the match

### 3. Invoke the top candidate(s)

If the top candidate has high confidence (score ≥ 3), proceed to that workflow's SKILL.md directly. If multiple candidates score similarly, read their SKILL.md `When to invoke` sections to disambiguate.

### 4. If no candidates match

The tool returns "no match" when the task description doesn't obviously fit any workflow. Two paths:
- **Refine the task description** and re-call (operator may be using vocabulary the substrate doesn't recognize)
- **Register as a substrate gap** via `forge.substrate_gap_registration` (#372) with `kind: tooling` — the substrate may need a new workflow

## How matching works

The tool uses simple token-based matching against each workflow's summary + slug + skill metadata:

- Words shared between the task description and the workflow summary score 1 each
- Exact slug-token matches score 2
- "Action verbs" (build / modify / add / verify / check / extract / register / explain) trigger specific routes

This is heuristic, not semantic. False positives are expected. The skill's purpose is to surface candidates — final selection is the operator's call.

## Common pitfalls

### Pitfall 1: Trusting the top match unconditionally

The meta returns candidates, not commitments. Always read the candidate's `When to invoke` + `Anti-signals` before proceeding.

### Pitfall 2: Treating "no match" as substrate failure

The substrate has 11 workflows. If your task doesn't match, the more likely cause is task-description vocabulary mismatch than a real gap. Refine first; only register a gap if multiple operators independently report the same miss.

### Pitfall 3: Skipping the meta on "obvious" tasks

Tasks that *seem* to map cleanly to one workflow sometimes have non-obvious gotchas (e.g., "modify the site" is `forge_modify_site` for content edits but `forge_modify_primitive` for substrate-level changes). Run the meta once to surface alternatives.

### Pitfall 4: Calling repeatedly on the same task

The meta is a routing step, not a decision-maker. If you've called it 3+ times for the same task, the bottleneck is task definition, not routing.

## Acceptance criteria

1. ✓ At least one candidate returned (or explicit "no match")
2. ✓ Operator read the top candidate's SKILL.md `When to invoke` before proceeding
3. ✓ Final workflow invocation aligns with the task's actual scope (not just the first match)

## Mapping to substrate

- **MCP tool**: `forge.skill_invocation_meta`
- **Backing**: `forge-core::workflow_registry::WORKFLOW_REGISTRY` (read-only)
- **Doctrine rules**: `agent_workflows_must_be_paired` (this skill IS the agent-facing entry-point)
- **Related skills**: every other `forge-*` skill is downstream of this one
- **Replaces**: ad-hoc "which skill do I read?" guessing
