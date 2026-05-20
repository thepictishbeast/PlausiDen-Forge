# PlausiDen MCP tool definitions

Model Context Protocol (MCP) tool definitions for the PlausiDen ecosystem. **Cross-AI consumable** — any MCP-compatible agent (Claude / Gemini / Cursor / other clients) can mount these as tools.

> Per `[[priority-architectural-first-and-cross-ai]]` doctrine: tool definitions are format-agnostic (JSON), not Claude-specific. No proprietary extensions. The same JSON files declare tools to every MCP client.

---

## What's here

```
mcp/
├── README.md           — this file
├── manifest.json       — MCP server manifest listing every declared tool
└── tools/              — one JSON file per tool, with full inputSchema
    ├── forge_orient.json
    ├── forge_build.json
    ├── forge_doctrine_*.json
    ├── forge_audit_*.json
    ├── forge_verify.json
    ├── forge_bypasses.json
    ├── loom_*.json
    └── crawler_*.json
```

Each `tools/*.json` follows the [MCP tool schema](https://modelcontextprotocol.io/specification/server/tools):

```json
{
  "name": "forge_orient",
  "description": "...what the tool does + when an agent should reach for it...",
  "inputSchema": {
    "type": "object",
    "properties": { ... },
    "required": []
  }
}
```

The `description` is the **affordance signal** that agents read when deciding whether to use the tool. It names: purpose + when-to-invoke + the typed surface — not just the syntax.

---

## How to consume

### From Claude Code

Mount via `~/.claude/mcp.json`:

```json
{
  "mcpServers": {
    "plausiden-forge": {
      "command": "<path-to-mcp-bridge>",
      "args": ["--manifest", "/home/paul/projects/PlausiDen-Forge/mcp/manifest.json"]
    }
  }
}
```

(The MCP bridge binary is a separate substrate piece — see task to follow `[[#199-follow-on]]`. The JSON definitions in this directory are the authoritative declarations regardless of how a bridge is implemented.)

### From Gemini / other agents

Read `mcp/manifest.json` directly; each entry under `tools` points to a `tools/<name>.json` file with the full input schema. Mount per the agent's MCP integration docs.

### Static inspection

```bash
forge orient                  # high-level: what tools exist, what they're for
ls mcp/tools/                 # raw JSON definitions
jq '.name' mcp/tools/*.json   # tool name index
```

---

## Anti-patterns

| ❌ Don't | ✅ Do |
|---------|------|
| Hand-roll a `forge.build()` JSON-RPC wrapper | Read `mcp/tools/forge_build.json` — schema is canonical |
| Add Claude-specific extensions to a tool def | MCP schema only — cross-AI parity |
| Duplicate command help text in tool descriptions | Reference `forge <subcommand> --help` and the AVP-Doctrine rule a tool enforces |
| Add a new MCP tool without adding to `manifest.json` | The manifest is the discovery index — list every tool |

---

## Cross-references

- `AGENTS.md` — top-level orientation for AI agents working in this repo.
- `TOOLS.md` — full canonical CLI command index (the source of truth for the tool surface).
- `forge orient` — single-command session brief (also cross-AI; JSON output).
- `skills/` — task-oriented playbooks; orthogonal to MCP tools (procedures, not declarations).
- AVP-Doctrine rule `docs-008` — structured (JSON) output across platform tools.
