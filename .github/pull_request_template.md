<!--
  PlausiDen-Forge PR template.
  Per AVP-Doctrine rule docs-007 (substrate updates AGENTS.md +
  TOOLS.md in the same commit) and per the
  `[[tool-starvation-anti-pattern]]` + `[[substrate-only-path]]`
  doctrines, this template surfaces two reviewable disciplines
  every PR is expected to pass.

  Closes tasks #154 (tools-first PR review) + #163 (substrate-
  bypass rejection) when merged.
-->

## Summary

<!-- One-paragraph description of what changed and why. Reference the issue / capability-request / doctrine rule motivating the change. -->

## Substrate discipline checklist

Per `[[substrate-only-path]]` doctrine. Each ☐ below blocks merge.

- [ ] **No hand-authored HTML / CSS / JS in site repos.** All site output flows through Forge phases + Loom primitives. If you needed CSS, you extended `loom-tokens/src/skin.css`; if you needed a new component, you added a Loom primitive (per `add-loom-primitive` skill).
- [ ] **`forge build --mode production` is strict-clean** (zero strict findings).
- [ ] **`forge doctrine check` passes** (every `Finding.citing(...)` reference resolves).
- [ ] **`forge audit phantom_button` passes** if rendering changed (every `data-backend` references an entry in `backends.toml`).
- [ ] **`forge bypasses` shows no orphan register entries / no untracked tags** if you touched the bypass register.
- [ ] **If a substrate-bypass was unavoidable**, a `// SUBSTRATE-BYPASS(<issue-id>): <reason>` comment is present in code AND `bypass-register.toml` has the matching entry with operator approval + backfill deadline.

If a checkbox cannot be ticked: file a capability-request issue rather than landing the workaround (per `docs/CAPABILITY_REQUEST_WORKFLOW.md`).

## Tool-surface discipline checklist

Per `[[tool-starvation-anti-pattern]]`. Each ☐ below blocks merge.

- [ ] **New CLI surface** — if you added a `forge` subcommand or flag, you also updated `TOOLS.md` + `AGENTS.md` + `Makefile` `make help` table + (if cross-AI consumable) `mcp/tools/<name>.json` + `mcp/manifest.json`. Same commit.
- [ ] **Anti-patterns updated** — if your change deprecates an older approach, the `Anti-patterns` table in `TOOLS.md` and `AGENTS.md` reflects the new canonical tool.
- [ ] **Cross-AI parity** — any MCP schema changes preserve format-agnostic shape (no Claude-specific extensions per `[[priority-architectural-first-and-cross-ai]]`).
- [ ] **JSON output** — any new CLI invocation supports `--json` for machine consumption (per rule `docs-008`).

## Doctrine discipline checklist

- [ ] **Doctrine cited** — every new `Finding` in a Forge phase uses `.citing([...rule-ids...])` to ground itself in AVP-Doctrine.
- [ ] **New doctrine rule** — if a new class of violation was introduced, a corresponding rule is in `PlausiDen-AVP-Doctrine/doctrine/rules/<domain>.toml` (statement + rationale + enforcement triple) — see `extend-doctrine-rules` skill.
- [ ] **Lifecycle declared** — new rules are `experimental` until a trial period proves enforcement is reliable.

## Test plan

- [ ] `cargo test -p <touched-crates>` passes
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `cargo fmt --all -- --check` clean
- [ ] Property tests at any new input boundary (per rule `test-002`)
- [ ] Regression fixtures added for the bug class being fixed (per rule `test-004`)

## Reviewer hints

- **Substrate-bypass posture**: if you see `// SUBSTRATE-BYPASS(...)` in the diff, verify the register entry, the issue link, and the backfill deadline. Default to rejecting bypasses; ask "can this be a Loom variant?" first.
- **Tool-starvation posture**: if you see `std::process::Command::new("grep"|"find"|"curl")` or shell heredocs in Rust code, ask "is there a `forge` / `loom` / `crawler` subcommand that does this?"
- **Cross-AI parity posture**: MCP tool schemas should read identically to Claude, Gemini, Cursor, other clients. No `x-claude-*` extensions.

## Cross-references

- `AGENTS.md` (Rule 0 + Rule 1 + Tool inventory)
- `TOOLS.md` (canonical command index)
- `PLAUSIDEN_ECOSYSTEM.md` (cross-repo orientation)
- `docs/CAPABILITY_REQUEST_WORKFLOW.md` (substrate-extension workflow)
- `docs/RECOMMENDED_LOOP_PREAMBLE.md` (durable-loop policy)
- `mcp/README.md` (cross-AI tool surface)
- Doctrine: `forge doctrine for <path>` to surface rules applicable to your change
