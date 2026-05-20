# TOOL_ADVOCACY.md

How every PlausiDen substrate tool teaches the operator (or AI agent) about the substrate-correct alternative when it detects suboptimal use, errors, or anti-patterns. Tools should not just *report* — they should *advocate* for the canonical path forward.

> Per `[[tool-starvation-anti-pattern]]` doctrine: Claude (and other agents) have strong training-prior toward generic tools (bash / grep / curl / hand-rolled scripts) over substrate-native tools. Every error path is an opportunity to push toward the substrate solution. If `forge audit phantom_button` finds a violation, it doesn't just say "broken" — it names the fix.

> Authored to close `#151 [toolsurface-v7]`. Companion to `AGENTS.md` (Rule 1: look before you build), `TOOLS.md` (canonical command index), and the substrate-discipline CI workflow.

---

## The advocacy template

Every error / finding / warning emitted by a substrate tool follows this template:

```text
[<severity>] <what happened> (at <location>)
   Why: <one-sentence cause>
   Substrate fix: <the substrate-correct command or workflow>
   Doctrine: <cited rule id> — `forge doctrine query --rule <id>` for full statement
   Skill: <relevant skill name> (skills/<name>/SKILL.md)
```

Concrete example (good — `forge audit phantom_button` with advocacy):

```text
[strict] data-backend="cta-signup" has no matching entry in backends.toml (at static/index.html:42)
   Why: rendered HTML references an undeclared backend slug
   Substrate fix: add `[[backend]] id = "cta-signup"` to backends.toml in the SAME commit
   Doctrine: sec-007 — `forge doctrine query --rule sec-007` for full statement
   Skill: author-cms-content (skills/author-cms-content/SKILL.md § Declare nav + footer)
```

Concrete anti-example (current — bare error, no advocacy):

```text
forge: phantom_button violation
```

The bare error leaves the operator to figure out what to do. Advocacy points them at the typed solution.

---

## Why advocate?

Three independent reasons compound:

1. **Tool-starvation prevention.** Claude (and other AI agents) reach for `grep -r data-backend static/` when they don't know `forge audit phantom_button` exists. The error message is the discovery surface — make it pull toward the substrate tool.

2. **Substrate-only-path enforcement.** Per `[[substrate-only-path]]`: every gap is a substrate change. When the substrate refuses an artifact, the next thing the operator sees should be "here's how to extend the substrate," not "here's why you should write a workaround."

3. **Discovery compounds.** A single error that names a doctrine rule + skill + substrate command teaches three affordances at once. Across the substrate's hundreds of error paths, this compounds into a self-documenting tool surface.

---

## Required advocacy elements

| Element | Purpose |
|---------|---------|
| **Severity tag** | `[strict]` / `[warn]` / `[info]` / `[fatal]` — operator triages |
| **Plain-English what** | One sentence: what the tool observed |
| **Location** | `path:line` or `path:json_pointer` so operator finds it instantly |
| **Why** | One-sentence root cause (not just symptom) |
| **Substrate fix** | The substrate-correct command/workflow — exact, not vague ("add a backends.toml entry," not "configure your backends") |
| **Doctrine citation** | Rule id + `forge doctrine query --rule <id>` invocation for the full statement |
| **Skill pointer** | Relevant `skills/<name>/SKILL.md` if a procedure applies |
| **Anti-pattern reminder** | If the operator likely reached for bash/grep, name the substrate alternative explicitly |

Not every element is required for every error — but the more an error costs (strict findings, build failures), the more advocacy belongs.

---

## The `Advocacy` trait

Implementation pattern for new code (Rust):

```rust
/// Every Finding / error / diagnostic surfaces typed advocacy.
pub struct Advocacy {
    pub severity: Severity,
    pub what: String,
    pub location: Option<Location>,
    pub why: String,
    pub substrate_fix: String,
    pub doctrine: Vec<RuleId>,
    pub skill: Option<&'static str>,
    pub anti_pattern: Option<&'static str>,
}

pub trait WithAdvocacy {
    fn advocacy(&self) -> Advocacy;
}

impl WithAdvocacy for Finding {
    fn advocacy(&self) -> Advocacy {
        // Each phase's Finding emission builds the Advocacy struct.
        // The Display impl renders it in the template above.
        // `--json` mode emits the structured shape.
    }
}
```

The `Advocacy` struct becomes a first-class part of every emission. JSON output (per `docs/JSON_OUTPUT.md`) includes it as the `advocacy` field.

---

## Audit — current state

A sweep across emission sites in `forge-cli`, `forge-phases`, `forge-core`, `loom-cli`, and the Crawler runtime classifies each into one of:

- **✓ Good advocacy** — emits substrate-correct command + cites doctrine + names skill where applicable
- **◐ Partial** — names the problem clearly but doesn't point at the typed solution
- **✗ Bare error** — just reports the symptom

### High-volume / high-leverage error paths

| Emission site | Current state | Required advocacy elements |
|---------------|---------------|----------------------------|
| `forge audit phantom_button` finding | ◐ | + substrate fix + skill pointer |
| `forge audit secrets` finding (with `--explain`) | ✓ | already cites detector rule; skill pointer optional |
| `forge audit external_assets` finding | ◐ | + substrate fix (loom-tokens skin.css path) |
| `forge doctrine check` orphan citation | ◐ | + substrate fix (file capability request OR fix the typo) |
| `forge doctrine for <path>` empty result | ◐ | + advocacy "use `forge doctrine query --domain <name>` instead" |
| `forge bypasses` orphan tag | ◐ | + substrate fix (add register entry OR remove tag) |
| `forge bypasses` register-without-tag | ◐ | + substrate fix (close out the bypass OR add the SUBSTRATE-BYPASS comment) |
| `forge manifest` cycle in phase dependencies | ◐ | + skill pointer to add-forge-phase |
| `forge privacy` uncovered DataCategory | ◐ | + substrate fix (declare retention policy in privacy.toml) |
| `forge contrast` finding | ◐ | + substrate fix (loom-tokens color edit; cite a11y-003) |
| `forge label_consistency` finding | ◐ | + substrate fix (rule sec-004 explanation) |
| `forge substrate_purity` finding (hand-coded artifact) | ✓ | already names canonical fix per phase doc |
| `forge build` Cargo build failure | ✗ | + skill pointer + check `forge.toml` |
| `forge build` strict-mode-fail | ◐ | + advocacy: "fix the finding, don't disable strict mode" |
| `forge content validate` schema mismatch | ◐ | + substrate fix (cms-schema.json reference) |
| `forge assets` missing format in ladder | ◐ | + substrate fix (loom-bridge emit-schema command) |
| `forge config` missing required field | ◐ | + substrate fix (typed-config gate doc reference) |
| `forge orient` doctrine load failure | ✓ | already emits structured warning + path |
| `loom validate` unknown variant | ◐ | + skill pointer to add-loom-primitive |
| `crawler --journey ...` step failure | ✗ | + advocacy: "viewport / selector / network mismatch" |

**Tally**: ~3 ✓, ~17 ◐, ~3 ✗ across the high-leverage error paths. The ◐ class is the easy win — text already names the problem; just add the substrate-fix line + doctrine citation.

---

## Implementation arc

| Step | Status | Deliverable |
|------|--------|-------------|
| **Done — this doc** | ✓ | Advocacy template + Advocacy trait sketch + audit of current state |
| Phase 1 | ✓ landed at 9ee5851 | Advocacy struct + WithAdvocacy trait + Finding builders + 7 unit tests; phantom_button retrofit + print_finding renderer |
| Phase 2 | ✓ closed — 20 forge-phases carry Advocacy: a11y_landmarks / asset_optimization / backend_coverage / contrast / csp / external_assets / html_semantic / id_strategy / label_consistency / link_check / loom_lint / phantom_button / semver_enforcement / sri / substrate_purity / theme_consistency / tokens / trait_consistency / trait_implications / validate_cms. The originally-targeted 17 ◐ emission sites are met; subsequent phases retrofitted by the same pattern as they landed. | Pattern is self-sustaining: new Findings ship Advocacy by convention. |
| Phase 3 | pending | Refactor the 3 ✗ emission sites |
| Phase 4 | pending | JSON output emits `advocacy` field per `docs/JSON_OUTPUT.md`; consumer schemas updated (automatic via serde; needs schema doc + MCP tool description update) |
| Phase 5 | pending | Loom + Crawler emission sites adopt the same trait (cross-repo) |
| CI guardrail | pending | A lint that refuses new `Finding::new(...)` without an `Advocacy` (after Phase 2 majority lands) |

Each phase is a small bounded task; total ~10-15 PRs across the substrate.

---

## Templates for common cases

### Substrate-bypass attempted

```text
[fatal] hand-authored CSS detected in site/static/* — substrate-purity violation
   Why: the substrate-only-path doctrine forbids hand-authored HTML/CSS/JS in site repos
   Substrate fix: extend loom-tokens skin.css (per skill add-loom-primitive § skin.css conventions)
                  OR add a new primitive variant (CmsSection enum + render emission)
                  OR if genuinely emergent: declare via the bypass workflow
                    1. open the capability request: gh issue create --template capability-request.yml
                    2. document the bypass in bypass-register.toml
                    3. add `// SUBSTRATE-BYPASS(<issue-id>): <reason>` in source
   Doctrine: build-007, prim-006 — `forge doctrine query --rule build-007`
   Skill:    add-loom-primitive (skills/add-loom-primitive/SKILL.md)
   Anti-pattern: don't `curl + cp` into static/. Don't `cat <<HTML > static/foo.html`. Don't edit static/loom-skin.css (build artifact).
```

### Tool-starvation attempted (caught by lint)

```text
[warn] crates/forge-phases/src/foo.rs:42 — `std::process::Command::new("grep")` detected
   Why: substrate has typed APIs for cross-reference checks; subprocess grep masks the typed surface
   Substrate fix: use `forge_core::scan::pattern(...)` (typed scan API) OR a Forge phase
                  if this is a repeatable check that should land in the audit pipeline
   Doctrine: build-001, docs-008 — `forge doctrine query --rule build-001`
   Skill:    add-forge-phase (skills/add-forge-phase/SKILL.md)
   Anti-pattern: see AGENTS.md § Anti-patterns; never reach for grep when forge phase exists
```

### Operator opens build without forge orient

The `forge build` first emit (when no `.forge-oriented` marker exists in the working session — to be added) could include:

```text
[info] First operation in this session detected (no recent `forge orient` activity logged).
   Suggestion: run `forge orient` first for affordance inventory + scoped doctrine.
   Doctrine: tool-starvation-anti-pattern (memory) — orienting reduces wrong-tool friction.
   Skill:    skills/README.md
```

(This advocacy is *additive* — it doesn't gate the build, just nudges.)

---

## Cross-AI consumption

The `Advocacy` struct serializes cleanly to JSON. AI agents (Claude / Gemini / other MCP-capable clients) consume:

```jsonc
"findings": [{
   "severity": "strict",
   "phase": "phantom_button",
   "code": "phantom-1",
   "message": "...",
   "file": "static/index.html",
   "line": 42,
   "enforces_rules": ["sec-007"],
   "advocacy": {
     "what": "...",
     "why": "...",
     "substrate_fix": "...",
     "doctrine": ["sec-007"],
     "skill": "author-cms-content",
     "anti_pattern": "don't use `grep -r data-backend static/`"
   }
}]
```

An AI agent reading this JSON sees not just the failure, but the substrate-correct next action. The agent's behavior shifts from "diagnose the problem" to "apply the named fix" — making the substrate-only-path the path of least resistance.

---

## Anti-patterns

| ❌ Don't | ✅ Do |
|---------|------|
| Emit bare errors ("validation failed") | Cite the failed rule + the substrate fix command |
| Use generic advice ("check your config") | Name the exact file + the exact field + the exact value |
| Make the operator search for the skill | Point at it by path: `skills/<name>/SKILL.md` |
| Reference removed/renamed doctrine rules | Reference current rule ids; if uncertain, run `forge doctrine query --rule <id>` first |
| Include emoji / decorative output without a function | Severity tag is functional; everything else costs scan time |
| Repeat the same advocacy text in every finding of the same class | Render once + reference; if a Finding has 50 instances of the same violation, batch the advocacy at the top |
| Skip JSON-output Advocacy fields | Required for AI consumption |

---

## See also

- `AGENTS.md` § Anti-patterns — explicit list of bad-tool→good-tool pairs.
- `TOOLS.md` § Anti-patterns table — same mapping in human form.
- `docs/JSON_OUTPUT.md` — Advocacy serialization in the JSON envelope.
- `[[tool-starvation-anti-pattern]]` memory — the founding directive.
- `[[substrate-only-path]]` memory — Rule 0.
- AVP-Doctrine rule `docs-005` — Findings cite the rules they enforce.
