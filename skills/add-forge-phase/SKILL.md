---
name: add-forge-phase
description: Add a new Forge audit phase implementing the Phase trait. Covers module placement, trait impl, rule citation, regression fixtures, pipeline registration.
metadata:
  tags: [forge, phases, audit, doctrine]
  related_doctrine_rules: [build-002, build-003, build-004, docs-002, docs-005, test-001, test-002, test-004]
  related_traits: []
---

# Add a Forge phase

Use this skill when the substrate needs a new audit / validation step that runs as part of `forge build` (or out-of-pipeline via `forge audit`). A phase emits typed `Finding`s; the runner aggregates them; the build's exit code follows from the strict-finding count.

## When to invoke

Recognition signals:
- A new doctrine rule needs build-time enforcement.
- A class of bug exists in rendered output that no existing phase catches.
- An external standard (WCAG, RFC, regulation) needs to be verified mechanically.
- A capability-request issue (per `docs/CAPABILITY_REQUEST_WORKFLOW.md`) categorized as "Forge phase" was accepted.

Anti-signals (don't add a phase):
- The check belongs at parse time (use serde / clap / schema validation instead).
- The check is intrinsic to a typed surface (extend the `*-core` validate() method).
- The check would only apply to one specific site (file a Loom primitive request instead — primitives generalize, phases enforce).

## Prerequisites

Read first, in this order:
1. `AGENTS.md` Rule 0 + Rule 1 — substrate-only-path + look-before-you-build.
2. `TOOLS.md` — existing Forge subcommand surface, including the doctrine commands.
3. Applicable doctrine rules: `forge doctrine for crates/forge-phases --terse`.
4. `crates/forge-core/src/lib.rs` — the `Phase` trait + `Finding` + `BuildCtx` + `BuildMode` types you'll implement against.
5. A similar existing phase (browse `crates/forge-phases/src/*.rs` for the closest analog).

## Procedure

### 1. File the capability request (if not already)

```bash
gh issue create --template capability-request.yml
```

Substrate layer: "Forge phase". Proposed contract names the input (what files/state the phase reads), the output (which Finding kinds with which severity), and the rule(s) the phase enforces.

### 2. Scaffold the phase

Use the template:

```bash
cp templates/forge-phase/Cargo.toml.tmpl crates/<your-phase>-phase/Cargo.toml
cp templates/forge-phase/src/lib.rs.tmpl crates/<your-phase>-phase/src/lib.rs
```

For simple phases without heavy deps, **prefer extending the existing `forge-phases` crate** by adding a module — don't create a new crate per phase. The template's structure goes into a new `crates/forge-phases/src/<your_phase>.rs` module file.

### 3. Implement the `Phase` trait

```rust
use forge_core::{BuildCtx, BuildError, Finding, Phase};

#[derive(Debug, Clone, Copy, Default)]
pub struct YourPhase;

impl Phase for YourPhase {
    fn name(&self) -> &'static str {
        "your_phase"  // snake_case; matches forge.toml [checks].phases entry
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        // Read inputs from ctx.root / ctx.static_dir.
        // Apply your check.
        // Emit findings with rule citations.
        Ok(findings)
    }
}
```

### 4. Emit findings with rule citations

Per rule docs-005, every finding cites the doctrine rule(s) it enforces. Use the `.citing()` builder:

```rust
findings.push(
    Finding::strict(
        self.name(),
        path.display().to_string(),
        format!("specific human-readable description with values: {x}"),
    )
    .citing(["prim-001", "a11y-001"]),
);
```

`.citing()` chains; the runner formats as `(prim-001, a11y-001)` suffix in the report. Readers can `forge doctrine query --rule prim-001` to read rationale.

### 5. Register the phase

Two registration points:

(a) Module declaration in `crates/forge-phases/src/lib.rs`:
```rust
pub mod your_phase;
```

(b) Pipeline order in the consumer site's `forge.toml`:
```toml
[checks]
phases = [
    "tokens",
    "html_semantic",
    "...",
    "your_phase",  # add at appropriate stage
]
```

Phase order is contractually meaningful (rule build-006). Phases that depend on others' outputs must come after their dependencies.

### 6. Ship a regression fixture

Per rule test-004, every phase ships at least one fixture demonstrating it correctly emits a Finding on input violating the rule.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::BuildMode;
    use std::fs;

    fn ctx(static_dir: &std::path::Path) -> BuildCtx {
        BuildCtx {
            root: static_dir.parent().unwrap().to_path_buf(),
            static_dir: static_dir.to_path_buf(),
            mode: BuildMode::Poc,
        }
    }

    #[test]
    fn detects_violation_class_X() {
        let tmp = tempfile::tempdir().unwrap();
        let static_dir = tmp.path().join("static");
        fs::create_dir_all(&static_dir).unwrap();
        fs::write(static_dir.join("bad.html"), b"<bad>").unwrap();
        let findings = YourPhase.run(&ctx(&static_dir)).expect("runs");
        assert_eq!(findings.len(), 1);
        assert!(findings[0].enforces_rules.contains(&"rule-XXX".to_string()));
    }
}
```

### 7. Property-based test at the input boundary

Per rule test-002, every input-accepting function has a proptest.

```rust
proptest! {
    #[test]
    fn phase_handles_arbitrary_inputs(name in r"[a-z0-9-]{1,32}") {
        // Verify phase doesn't panic on edge-case inputs.
    }
}
```

### 8. Update AGENTS.md + TOOLS.md in the same PR

Per rule docs-007:
- Add the phase to `AGENTS.md` tool inventory if it surfaces via `forge audit --phase X`.
- Add to `TOOLS.md` if you're exposing it as a separately invocable subcommand.

### 9. Verify the chain

```bash
make ci                  # fmt-check + clippy + test
make doctrine-check      # verify your rule citations resolve
make forge-build         # full pipeline runs clean
```

## Common pitfalls

| ❌ Don't | ✅ Do |
|---------|------|
| Emit raw `panic!` / `unwrap()` from `run()` | Return `BuildError` (use `with_context` or `thiserror`); rule build-003 |
| Cite a rule id that doesn't exist | Always verify with `forge doctrine query --rule <id>` first; `forge doctrine check` catches at PR time |
| Skip the regression fixture | Required per rule test-004; CI fails the PR |
| Block on I/O without bound | Phases run as part of the build; long blocking is operator-visible. Wrap network/IPC in timeouts |
| Forget to register in `lib.rs` | The module compiles in isolation but isn't reachable from `forge build` until added to `pub mod` list |
| Add the phase to the pipeline before it's stable | Use `severity = "warn"` for the first few iterations; promote to `strict` after the rule's lifecycle is `stable` |

## Acceptance criteria

- [ ] `cargo test -p forge-phases your_phase::tests` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] Regression fixture exercises the violation case + the clean case
- [ ] Property test at any input boundary
- [ ] Finding emissions cite the doctrine rule(s) the phase enforces
- [ ] `forge doctrine check` reports zero orphan citations
- [ ] AGENTS.md + TOOLS.md updated in same commit (if surfaced)
- [ ] Pipeline entry added to `forge.toml [checks].phases` for the relevant consumer site(s)
- [ ] Module appears in `crates/forge-phases/src/lib.rs` `pub mod` list

## Cross-references

- AVP-2 protocol: `PlausiDen-AVP-Doctrine/AVP2_PROTOCOL.md`
- Substrate Discipline: `PlausiDen-AVP-Doctrine/SUBSTRATE_DISCIPLINE.md`
- Doctrine rules: `forge doctrine for crates/forge-phases`
- Phase template: `templates/forge-phase/`
- Example simple phase: `crates/forge-phases/src/substrate_purity.rs` (task #156)
- Example phase wired to rule citations: `crates/forge-phases/src/phantom_button.rs` (task #177)
