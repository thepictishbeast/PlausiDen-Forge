# forge-mcp

Model Context Protocol server for the Forge substrate. Exposes
Forge subcommands as JSON-RPC tools so MCP-aware clients
(Claude Code, Codex, Cursor, …) can call `forge orient`,
`forge build`, `forge audit … --explain`, `forge doctrine for
<path>`, `forge synthesis preview`, `forge codegen` without
re-parsing CLI text on every invocation.

Per paul 2026-05-21 directive: "skills and MCPs that allow you
to work even more closely with forge and get all the
functionality and potential out of it, it should be designed
in a way that saves as many tokens as possible."

## Install

```sh
cargo install --path crates/forge-mcp --bin forge-mcp
```

## Use from Claude Code

Add to `~/.claude/mcp-servers.json`:

```json
{
  "mcpServers": {
    "forge": {
      "command": "forge-mcp"
    }
  }
}
```

Tools surface as `forge.orient`, `forge.build`, etc. — schemas
deferred per the MCP spec until invoked, so the listing is
cheap on Claude's side.

## Tool surface

### Shipped

- `forge.orient { root?: string }` — session brief. Shells out
  to `forge orient --root <root>`.
- `forge.build { root?, json? }` — run every phase + return the
  build report. Pass `json: true` to request structured output
  when the underlying `forge build` supports it.
- `forge.doctrine.for { path, root?, terse? }` — surface
  doctrine rules applicable to a path. Defaults to terse output
  (rule ids + names only) for token efficiency.
- `forge.authoring { root? }` — scan a tenant's `cms/*.json`
  for empty / below-floor content fields. Returns a structured
  TODO list of sections that still need content.
- `forge.config { root? }` — umbrella config-gate runner
  (privacy / trust-safety / domains / forms / federation /
  email / commerce / memberships). Missing config files are
  warnings, not failures.
- `forge.fix { root? }` — auto-fix mechanical findings from
  the latest build report. Idempotent.

### Planned

- `forge.synthesis.preview { spec_path }` — preview the
  `SiteSpec` that would generate from a given spec.
- `forge.codegen { root?, target? }` — emit an axum + tokio +
  sqlx crate from `cms/*.json`.
- `forge.tenant_style.preview { root? }` — render the tenant's
  `[style]` config as the `<style>` snippet that injects into
  the page-shell head.

## Why an MCP, not just skills?

Per claude-tools META.md doctrine (PR #3 there):

> Skill = workflow / convention / when-to-use. MCP = callable
> typed operation. The substrate ships dozens of `forge`
> subcommands — exposing them as one MCP server with deferred-
> schema-loaded tools costs ~0 context tokens until invoked.

## Implementation notes

- Stdio JSON-RPC 2.0 per the MCP spec.
- v0.1.0 shells out to the installed `forge` binary. Future
  iters call `forge-core` / `forge-phases` directly to skip
  subprocess overhead + return structured JSON instead of
  CLI text (more token-efficient).
- Notification requests (`id == null` + method starts with
  `notifications/`) receive no response per spec.
