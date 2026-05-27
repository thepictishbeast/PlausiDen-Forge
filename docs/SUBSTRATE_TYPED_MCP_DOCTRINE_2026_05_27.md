# Layer 1 — Type-enforced MCP at substrate boundary (2026-05-27)

Per task #362. Establishes the discipline that every tool exposed
through `forge-mcp` parses its arguments through a typed Rust struct
with `#[serde(deny_unknown_fields)]` before any side effect occurs.

## Why this matters

The MCP boundary is where AI agents (Claude Code, Codex, Cursor,
Gemini, others) reach into the substrate. Errors at this boundary
fail in one of three ways:

1. **Silent typo absorption**: argument named `roots` instead of
   `root` gets ignored, tool defaults `.`, build runs against the
   wrong directory, agent reports success against the wrong tenant.

2. **Type confusion**: argument `json: "true"` (string) instead of
   `json: true` (bool) gets coerced or skipped depending on
   parsing code, tool produces inconsistent results.

3. **Schema/code drift**: hand-written `inputSchema` in `tool_list()`
   claims one shape, hand-rolled `args.get("...").and_then(.as_str())`
   accepts a different shape, callers see schema-promised behaviour
   diverge from actual behaviour.

All three failure modes are silent: the build appears to succeed,
the tool returns content, the agent moves on. Substrate-wide bias
toward consumer-band SaaS marketing (per the 2026-05-21 reframe)
gets reinforced as the wrong inputs silently shape outputs.

## The rule

> Every `tool_forge_*` function MUST parse its `Value` argument
> through a typed struct in `typed_args` as its first statement.
> Parse failures MUST return a structured `isError` MCP response
> carrying the serde error message.

## Implementation pattern

Each tool has one struct in `crates/forge-mcp/src/typed_args.rs`:

```rust
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BuildArgs {
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub json: bool,
}
```

Each tool function is:

```rust
async fn tool_forge_build(args: Value) -> Value {
    let parsed: BuildArgs = match parse_args("build", args) {
        Ok(p) => p,
        Err(err_value) => return err_value,
    };
    let root = parsed.root.as_deref().unwrap_or(".");
    // ... use parsed fields, never touch raw `args` again
}
```

The `parse_args::<T>(tool, args)` helper returns either the parsed
struct or an MCP-shaped `isError` response. Tools never see
malformed input.

## Acceptance criteria

A tool is type-enforced when ALL of the following hold:

1. There exists `pub(crate) struct {Tool}Args` in `typed_args.rs`.
2. The struct has `#[serde(deny_unknown_fields)]`.
3. Required fields are non-`Option` (deserialize fails when absent).
4. Optional fields have `#[serde(default)]` and either `Option<T>` or
   a `bool`/numeric with explicit default.
5. The tool function's first statement is `let parsed: ... = match
   parse_args(...) { ... }`.
6. The tool function NEVER calls `args.get(...)` after the parse step.
7. A test in `typed_args::tests` covers: empty-ok case, required-field-
   missing case, unknown-field-rejected case.

## Current coverage

10 forge.* tools refactored in this iteration:

- `forge.orient` ✓
- `forge.build` ✓
- `forge.doctrine.for` ✓
- `forge.authoring` ✓
- `forge.config` ✓
- `forge.fix` ✓
- `forge.synthesis.preview` ✓
- `forge.codegen` ✓
- `forge.manifest.validate` ✓
- `forge.docs.query` ✓

11/11 unit tests pass. 4/4 existing scope_filter_tests still pass.

## What this does NOT cover

- **Output schemas**: tools return free-form `Value` content arrays;
  output is not type-checked. Future Layer-1 extension is to define
  output types too and serialize through them, so callers can
  deserialize results into typed shapes.

- **Inter-tool consistency**: if two tools both take `root`, they
  both have their own struct field. A shared `WithRoot` mixin could
  enforce one shape, but the current explicit duplication is fine
  while the tool count stays small.

- **Future schema generation**: schemars-derived `inputSchema` would
  eliminate the hand-maintained JSON in `tool_list()`. Not done in
  this iteration — schemars adds compile cost and the current
  JSON shape is in sync. Revisit when the count grows past 20 tools.

- **Crawler-MCP, Loom-MCP**: this discipline currently applies to
  `forge-mcp`. The same pattern should propagate to any other MCP
  server the substrate ships.

## Mapping to existing tasks

- **#363** (Layer 2: Paired skill + MCP workflow) — depends on this
  Layer 1 surface; skills can now trust that arguments they pass are
  validated at the boundary
- **#364-#374** (Workflow tools 1-11) — each new workflow MCP tool
  added under these tasks MUST follow the typed-args pattern from day
  one; the acceptance criteria above is the gate
- **#375** (CI: enforce skill ↔ workflow pairing) — should also lint
  for new MCP tools missing typed_args struct + tests

## Doctrine registration

This file establishes a doctrine rule that should be registered in
the canonical doctrine surface (`forge-core::doctrine`):

```
RULE: mcp_tools_typed_args_required
STATEMENT: Every forge-mcp tool function MUST parse its arguments
through a typed struct with deny_unknown_fields before any side
effect. Hand-rolled args.get(...).and_then(.as_str()) is forbidden
post-parse.
RATIONALE: Silent argument absorption shapes the substrate's outputs
against the wrong inputs. Hard parse failure at the boundary keeps
agent calls correct.
ENFORCEMENT: code review + future CI lint checking for the pattern.
APPLIES_TO: crates/forge-mcp/src/main.rs
```
