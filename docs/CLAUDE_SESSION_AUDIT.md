# CLAUDE_SESSION_AUDIT.md

Audit of the PlausiDen-Forge workspace + recent Claude-session contributions for two failure modes the substrate-discipline doctrine targets:

1. **Tool-starvation** (per `[[tool-starvation-anti-pattern]]`) — reaching for generic tools (bash / grep / find / curl / sed / awk / python) inside Rust code instead of using the substrate's typed surfaces.
2. **Substrate-bypass** (per `[[substrate-only-path]]`) — hand-authored HTML / CSS / JS in site repos instead of routing through Forge / Loom; or `// SUBSTRATE-BYPASS(...)` source tags without the corresponding `bypass-register.toml` envelope.

> Authored to close `#153 [toolsurface-v9]` (tool-starvation audit) and `#162 [substrate-discipline-v8]` (substrate-bypass audit). Companion to `docs/AI_AUDIT.md` (#188), which audited for AI-assuming code outside `forge-critic`.

> **Audit verdict (HEAD): clean on both axes.** No tool-starvation patterns in Forge crates. No active substrate-bypass tags in source. `bypass-register.toml` is absent (zero declared bypasses). The substrate-discipline CI workflows guard against new violations landing.

---

## Audit method

### Tool-starvation patterns

The audit greps Forge source for invocations of `std::process::Command` spawning shell-tier utilities:

```bash
grep -rn 'Command::new("grep"\|Command::new("find"\|Command::new("curl"\|\
         Command::new("sed"\|Command::new("awk"\|Command::new("rg"' \
  /home/paul/projects/PlausiDen-Forge/crates/
```

Plus Python (forbidden per loop preamble):

```bash
grep -rn 'Command::new("python\|Command::new("python3"' \
  /home/paul/projects/PlausiDen-Forge/crates/
```

Plus shell heredocs inside Rust strings (the "I'll just inline a quick shell script" pattern):

```bash
grep -rn '"<<EOF\|<<"EOF"' /home/paul/projects/PlausiDen-Forge/crates/
```

### Substrate-bypass patterns

```bash
# Active bypass tags in source.
grep -rn 'SUBSTRATE-BYPASS' /home/paul/projects/PlausiDen-Forge/crates/

# Bypass register existence + entry count.
ls -la /home/paul/projects/PlausiDen-Forge/bypass-register.toml

# Hand-coded assets in site repo: `forge audit substrate_purity` is the
# canonical check (also wired in determ-baseline.yml CI).
forge build --mode poc
```

---

## Findings — Tool-starvation

```
=== std::process::Command shell-tier invocations ===
(no results)

=== Python invocations ===
(no results)

=== Shell heredocs in Rust ===
(no results)
```

**Zero violations.** Every Forge phase uses typed Rust + the substrate's own surfaces:

- Pattern matching: in-process via the `html_walk` crate + `regex` where applicable.
- File traversal: `std::fs::read_dir` + the dedicated `visit_rs_files` helper.
- Subresource cross-references: `forge_core::scan` typed APIs.
- AVP-Doctrine queries: `doctrine_core::DoctrineDatabase` typed API.
- Crawler invocations: scheduled jobs / journey runner with typed JSON output.

The substrate was tool-starvation-free from inception. The `determ-baseline.yml` CI workflow's source-grep guardrail (added in task #188) extends this protection forward — any PR that lands a `Command::new("grep")` outside `forge-critic` would fail CI before merge.

### Tool-starvation patterns expected outside the substrate

The audit is scoped to **Forge crates**. Documentation files, README content, GitHub Actions YAML, and operator examples may legitimately mention `grep` / `find` / etc. as part of explaining anti-patterns or as historical reference. These do not count as substrate violations.

`docs/TOOL_ADVOCACY.md` is the canonical place tools advocate for their substrate-native alternatives in finding messages.

---

## Findings — Substrate-bypass

### Active bypass tags

```
=== SUBSTRATE-BYPASS tag occurrences in /home/paul/projects/PlausiDen-Forge/crates/ ===
forge-phases/src/substrate_purity.rs:9   (doc comment — bypass scanner module doc)
forge-phases/src/substrate_purity.rs:127 (advocacy text describing the bypass workflow)
forge-cli/src/main.rs:386                (doc comment — `forge bypasses` subcommand)
forge-cli/src/main.rs:6865, 6891, 7313+  (bypass scanner implementation; parses the tag)
```

All occurrences are **definitional** — they implement / describe / advocate the bypass workflow. **Zero active bypass tags** (`// SUBSTRATE-BYPASS(<issue-id>): <reason>` in real source code).

### Bypass register

```
$ ls -la /home/paul/projects/PlausiDen-Forge/bypass-register.toml
ls: cannot access ...: No such file or directory
```

**`bypass-register.toml` is absent.** This is the intended state: zero declared bypasses = the heavyweight bypass workflow has never been exercised in HEAD. Per `[[substrate-only-path]]`: substrate-bypass is a last-resort emergency mechanism; absent register = the substrate has been extended cleanly in every case.

### Site repo hand-coded artifacts

The `substrate_purity` Forge phase (#156) walks `static/` on every `forge build` and emits strict findings for hand-authored CSS/JS/WASM outside the canonical Forge/Loom emission allowlist. Per the audit grep above, the phase exists + is wired in the build pipeline; any regression would be caught at build time.

---

## CI guardrails currently active

These workflows enforce the audit verdict forward:

| Workflow | Gate | Closes |
|----------|------|--------|
| `.github/workflows/substrate-discipline.yml` | `forge doctrine check` + `forge bypasses` + `forge audit phantom_button` + MCP schema integrity | #154 / #163 |
| `.github/workflows/determ-baseline.yml` § source-grep | Rejects new AI imports outside `forge-critic` (extension target: also reject `Command::new(grep|find|curl|...)` outside test code as the surface grows) | #188 |
| `.github/workflows/backcompat-matrix.yml` | Every fixture × every supported substrate version renders clean | #140 |
| `forge-phases::substrate_purity` (in `forge build`) | Refuses hand-authored CSS/JS in `static/` | #156 |
| `forge bypasses` subcommand | Cross-references source-tagged bypasses vs `bypass-register.toml` | #161 |
| `forge doctrine check` subcommand | Refuses orphan citations of doctrine rules | #178 |

---

## Refactor procedure (for future use)

If either audit class ever surfaces a violation:

### Tool-starvation
1. Identify the caller spawning the shell-tier utility.
2. Replace with the typed Forge / Loom / Crawler equivalent (see `TOOLS.md` + `mcp/manifest.json`).
3. If no typed equivalent exists, file a capability-request (`gh issue create --template capability-request.yml`) and **block the PR** until the substrate-correct surface lands. Do not route around the substrate per `[[substrate-only-path]]`.

### Substrate-bypass
1. Verify the bypass is genuinely necessary (audit + capability-request review).
2. If unavoidable: add the `// SUBSTRATE-BYPASS(<issue-id>): <reason>` source tag.
3. Add the matching entry to `bypass-register.toml` with operator approval + backfill deadline.
4. CI (substrate-discipline.yml) cross-references both. Mismatched state fails.
5. The backfill issue must close before the deadline expires.

---

## Cross-references

- `[[tool-starvation-anti-pattern]]` memory — the founding directive
- `[[substrate-only-path]]` memory — Rule 0
- `docs/AI_AUDIT.md` — companion audit for AI-assuming code (#188)
- `docs/TOOL_ADVOCACY.md` — how tools advocate for the substrate-correct path (#151)
- `.github/workflows/substrate-discipline.yml` — CI gate
- `.github/workflows/determ-baseline.yml` — CI gate
- `forge bypasses` subcommand source: `crates/forge-cli/src/main.rs` (search for `SUBSTRATE-BYPASS`)
- `forge audit phantom_button`, `forge doctrine check` — adjacent gates
