# Layer 2 — Paired skill + MCP workflow infrastructure (2026-05-27)

Per task #363. Establishes the discipline that every workflow
exposed to AI agents is a PAIR: a SKILL.md describing what to do +
an MCP tool that does the heavy lifting. Layer-2 ships the registry
that binds the two and lets agents discover paired workflows
programmatically.

## Why pairing matters

Agents (Claude Code, Codex, Cursor, Gemini, others) succeed at
substrate-shaped tasks when two things are true:

1. They have **procedural knowledge** of what the task requires —
   prerequisites, steps, acceptance criteria, common pitfalls.
2. They have **action surface** to actually do the work — concrete
   tools that perform the operations rather than re-deriving them
   from shell commands.

A SKILL.md alone gives the agent procedure but no concrete action
surface (they end up shelling out to ad-hoc commands). An MCP tool
alone gives action without context (the agent calls it with no
sense of when, why, or how to interpret results). PAIRED, the
skill orients the agent and the MCP tool gives them clean execution.

The "every session re-derives the workflow" failure mode that the
existing `skills/README.md` flags is the SKILL-only failure; the
"untyped MCP coercion" failure that #362 closed is the MCP-only
failure. Layer 2 binds them so neither side can drift independently.

## Pairing convention

For a workflow with slug `<verb>_<noun>`:

| Surface | Naming |
|---|---|
| Workflow slug | `<verb>_<noun>` (snake_case) |
| Skill directory | `skills/forge-<verb>-<noun>/SKILL.md` (kebab-case) |
| MCP tool | `forge.<verb>_<noun>` (dotted + snake_case) |

Example for the brief→site workflow:

- Workflow slug: `build_site_from_brief`
- Skill directory: `skills/forge-build-site-from-brief/SKILL.md`
- MCP tool: `forge.build_site_from_brief`

Two skills that pre-date the convention keep their historical
directory names (`add-loom-primitive`, `add-forge-phase`); the
registry tracks both names so the convention applies only to new
workflows.

## Pairing lifecycle

A workflow is in one of four states (`forge_core::workflow_registry::
PairingStatus`):

| Status | Skill | MCP tool | Notes |
|---|---|---|---|
| `Planned` | absent | absent | Registered as future work |
| `SkillOnly` | shipped | absent | Procedure documented; action surface not wired |
| `McpOnly` | absent | shipped | Action surface live; procedure not documented |
| `Paired` | shipped | shipped | **Only valid steady state** |

`Planned`, `SkillOnly`, and `McpOnly` are transient. CI (#375)
enforces that workflows don't sit in transient states past their
grace window.

## The registry (`forge-core::workflow_registry`)

11 workflows registered in this iteration. The registry is
compile-time static (`&'static [WorkflowEntry]`) so `forge-mcp`
and CI lints can consume it without I/O. Each entry carries:

- `slug` — workflow slug (canonical identifier)
- `summary` — one-line description for agent discovery
- `skill_dir` — skill directory name
- `mcp_tool` — MCP tool name
- `status` — current pairing state
- `task_ref` — task ID tracking the work

Status snapshot at registry creation (2026-05-27):

| Slug | Status | Task |
|---|---|---|
| `build_site_from_brief` | Planned | #364 |
| `modify_site` | Planned | #365 |
| `add_primitive` | SkillOnly | #366 |
| `add_audit_phase` | SkillOnly | #367 |
| `modify_primitive` | Planned | #368 |
| `verify_content_originality` | Planned | #369 |
| `site_fingerprint_check` | Planned | #370 |
| `reference_extraction` | Planned | #371 |
| `substrate_gap_registration` | Planned | #372 |
| `doctrine_violation_explanation` | Planned | #373 |
| `skill_invocation_meta` | Planned | #374 |

## Agent discovery: `forge.workflows.list`

New MCP tool added to forge-mcp surface (typed args per #362
discipline):

```jsonc
// All workflows
{ "name": "forge.workflows.list", "arguments": {} }

// Only Paired (ready-to-use)
{ "name": "forge.workflows.list", "arguments": { "status": "paired" } }

// Look up one by slug
{ "name": "forge.workflows.list",
  "arguments": { "slug": "build_site_from_brief" } }
```

Returns the registry entries directly. Agents reach for this tool
when they need to discover what workflows the substrate exposes,
without having to scan `skills/` directly or guess MCP tool names.

## Compile-time invariants

The registry carries 4 unit tests that pin invariants at compile
time so accidental drift breaks the build:

1. `registry_has_eleven_workflows` — registry size matches the
   #364-#374 task series
2. `every_workflow_has_unique_slug` — no slug duplication
3. `every_workflow_has_unique_mcp_tool` — no MCP-tool duplication
4. `mcp_tool_names_follow_convention` — every `mcp_tool` is
   exactly `forge.<slug>`

These are the seams CI (#375) extends — file-system checks for
`skills/<skill_dir>/SKILL.md` existence and MCP tool presence in
`forge-mcp::tool_list()` belong to the lint, not the in-process
test (the registry can't see the filesystem).

## What this does NOT cover

- **CI enforcement** (#375 owns; will add a lint that asserts every
  `Paired`-status entry has a SKILL.md present + MCP tool registered)
- **Per-workflow implementations** (#364-#374 each own one slug;
  filling in those slugs flips status from `Planned` → `Paired`)
- **Cross-AI surface for Gemini/others** — this iteration targets
  MCP-capable agents. Per `[[priority-architectural-first-and-cross-ai]]`,
  skills work for any agent that can read Markdown; MCP-discovery
  requires MCP support. Non-MCP agents can still consume the SKILL.md
  surface manually.

## Mapping to existing tasks

- **#362** (Layer 1 typed-MCP) — prerequisite; `forge.workflows.list`
  uses the typed-args pattern from day one
- **#364-#374** (Workflow 1-11) — flip status from `Planned` →
  `Paired` as each lands
- **#375** (CI enforcement) — extends invariants 1-4 with filesystem +
  MCP-tool-list checks
- **#386 / #398** (doc-query expansion) — workflow_registry could be
  surfaced via doc_query too; out of scope for this iteration

## Doctrine registration

```
RULE: agent_workflows_must_be_paired
STATEMENT: Every workflow exposed to AI agents MUST have both a
SKILL.md and an MCP tool, registered in
forge-core::workflow_registry, with status `Paired` before the
workflow is documented as ready for agent use.
RATIONALE: Skill-only workflows force agents to re-derive action
surface; MCP-only tools strand agents without procedural context.
Pairing closes both failure modes.
ENFORCEMENT: forge-core::workflow_registry compile-time tests +
CI lint (#375).
APPLIES_TO: crates/forge-core/src/workflow_registry.rs,
crates/forge-mcp/src/main.rs, skills/forge-*/SKILL.md
```
