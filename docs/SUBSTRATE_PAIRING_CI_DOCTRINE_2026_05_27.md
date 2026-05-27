# CI gate: skill ↔ workflow pairing (2026-05-27)

Per task #375. Establishes the CI invariants that enforce the
`agent_workflows_must_be_paired` doctrine across substrate. Two
test surfaces back the gate: one filesystem-bound, one in-process.

## What CI now enforces

### 1. Filesystem invariant (forge-core integration test)

File: `crates/forge-core/tests/pairing_invariants.rs`

Three assertions:

- **`every_paired_workflow_has_skill_md_on_disk`** — for every entry
  in `WORKFLOW_REGISTRY` with status `Paired`, the file
  `<workspace>/skills/<skill_dir>/SKILL.md` must exist.
- **`every_skill_only_workflow_has_skill_md_on_disk`** — same for
  `SkillOnly`-status entries. Catches the case where a developer
  marks an entry as `SkillOnly` but ships the registry update
  before the SKILL.md.
- **`skill_dir_naming_convention_matches_slug_or_pre_existing`** —
  enforces that `skill_dir` matches `forge-<slug-with-dashes>` OR
  is in the legacy-accepted set (`add-loom-primitive`, `add-forge-
  phase`). Both legacy names are pinned to prevent silent renames.

### 2. In-process invariant (forge-mcp unit test)

File: `crates/forge-mcp/src/main.rs` (mod `pairing_invariant_tests`)

Two assertions:

- **`every_paired_workflow_has_mcp_tool_registered`** — for every
  `Paired` entry, the `mcp_tool` name appears in `tool_list()`
  output. Catches the case where a developer flips status to
  `Paired` but forgets to add the tool to the dispatch.
- **`no_mcp_only_workflows_register_without_skill`** — asserts
  zero `McpOnly` entries. `McpOnly` is doctrinally transient:
  commits should mark `Planned` (MCP not yet wired) or `Paired`
  (both shipped). `McpOnly` is the no-skill-but-MCP-shipped state
  that the doctrine forbids.

## What CI does NOT yet enforce

- **YAML frontmatter shape** in SKILL.md files. A SKILL.md that
  exists but lacks `name:`, `description:`, `metadata:` passes
  the filesystem check. Future iteration: parse and validate.
- **Skill body completeness** (When to invoke / Procedure / Common
  pitfalls / Acceptance criteria). Future iteration: schema-driven
  parser.
- **Reverse direction**: a SKILL.md on disk that isn't in the
  registry doesn't fail. Future iteration: orphan-skill check.

## How the gate fires

`cargo test --workspace` runs all integration + unit tests. The CI
runner (Makefile target or `cargo test` step) catches these tests
along with the rest. A failing pairing test produces:

```
Paired workflows missing SKILL.md on disk:
workflow 'modify_primitive' (status: Paired) — expected 
/home/paul/projects/PlausiDen-Forge/skills/forge-modify-primitive/SKILL.md
```

…or…

```
Paired workflows missing MCP-tool registration:
workflow 'site_fingerprint_check' (status: Paired) — expected MCP tool 
'forge.site_fingerprint_check' in tool_list()
```

Either failure blocks the PR until the missing artifact lands.

## Why two surfaces

The filesystem test runs against the SKILL.md inventory; the in-
process test runs against the MCP-tool dispatch table. They catch
different drift modes:

| Failure mode | Caught by |
|---|---|
| Registry flipped to Paired; SKILL.md not written | `forge-core` filesystem test |
| Registry flipped to Paired; MCP dispatch not updated | `forge-mcp` in-process test |
| McpOnly entry committed | `forge-mcp` in-process test |
| skill_dir renamed off-convention | `forge-core` naming test |

Together they cover every transition the doctrine cares about.

## CI integration

Currently the tests run via `cargo test -p forge-core` + `cargo
test -p forge-mcp`. The Makefile target `make ci` (when defined)
should include both. CI runners running `cargo test --workspace`
get both automatically.

## Doctrine registration

```
RULE: pairing_invariants_ci_enforced
STATEMENT: Every workflow registered as Paired in
forge-core::workflow_registry MUST have both its SKILL.md present
on disk AND its mcp_tool name registered in forge-mcp::tool_list().
McpOnly is a forbidden steady state.
RATIONALE: The agent-facing workflow surface is load-bearing for
AI-agent productivity per the substrate reframe. Silent drift
between SKILL.md inventory + MCP-tool dispatch produces orphan
tools or orphan skills; both fail the "agents can discover what
the substrate exposes" invariant.
ENFORCEMENT: cargo test --workspace via
- crates/forge-core/tests/pairing_invariants.rs (3 assertions)
- crates/forge-mcp/src/main.rs pairing_invariant_tests (2 assertions)
APPLIES_TO: crates/forge-core/src/workflow_registry.rs,
crates/forge-mcp/src/main.rs, skills/*/SKILL.md
```
