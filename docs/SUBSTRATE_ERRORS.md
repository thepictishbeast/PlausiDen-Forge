# Substrate-Aware Error Structure — doctrine

**Status:** load-bearing. Closes task #295. Defines the single canonical
shape every diagnostic in the Forge / Loom / Crawler substrate carries.

---

## The doctrine

Every diagnostic — phase finding, build error, loom-lint warning,
crawler-detector signal — carries five fields:

| Field         | Required | Example                                                                  |
|---------------|----------|--------------------------------------------------------------------------|
| `code`        | yes      | `"var-001"`, `"ident-002"`, `"io.filesystem"`, `"deps.missing"`          |
| `message`     | yes      | `"primitive `hero` violates declared identity"`                          |
| `path`        | sometimes| `"cms/index.json#section-0-hero"` (empty for project-wide diagnostics)  |
| `advocacy`    | yes¹     | `why` + `substrate_fix` + `skill` + `anti_pattern`                       |
| `cited_rules` | sometimes| `["ident-002"]`                                                          |

¹ A diagnostic with empty advocacy is non-conformant per this doctrine.
Producers MAY ship without advocacy during a phased adoption (the
field skips serialization when empty for byte-identical legacy
reports), but new code MUST populate advocacy.

## Code-namespace convention

Codes are kebab-case and namespace-prefixed:

| Namespace        | Meaning                                                          |
|------------------|------------------------------------------------------------------|
| `var-NNN`        | Cross-site variation enforcement (#231-#262 arc)                 |
| `ident-NNN`      | Site identity conformance (#234, #235, #237-#244 arc)            |
| `voice-NNN`      | Voice / vocabulary checks (#241, #244)                           |
| `theme-NNN`      | Theme variation requirements (#261, #243)                        |
| `pattern-NNN`    | Pattern entropy / forbidden patterns (#236, #251)                |
| `purity-NNN`     | Editorial purity gate (`editorial_purity_gate`)                  |
| `density-NNN`    | Density-tier audits                                              |
| `slop-NNN`       | Aesthetic distinctiveness / placeholder-value / scaffold checks  |
| `io.<area>`      | Filesystem / external I/O failures                               |
| `deps.<area>`    | Missing-dependency failures                                      |
| `config.<area>`  | Configuration shape failures                                     |
| `phase.<area>`   | Phase-internal errors not covered above                          |

Once shipped, a code's wire shape is frozen — renaming is a breaking
change per the version-discipline doctrine. New diagnostics get new
codes; deprecated codes stay defined but produce no findings.

## The Diagnostic type

`forge_core::Diagnostic` is the canonical struct. `#[non_exhaustive]`
so future fields (severity, locale, trace_id) don't break consumers.

```rust
use forge_core::Diagnostic;

let d = Diagnostic::new("ident-002", "primitive `hero` violates identity", "cms/index.json#section-0-hero")
    .why("the site's declared identity refuses this primitive but the CMS uses it")
    .fix("remove the primitive OR amend [site_identity] allowed_primitives / forbidden_primitives")
    .skill("identity-resolution")
    .avoid("don't disable the conformance phase to silence the finding")
    .citing(["ident-002"]);
```

## The SubstrateAwareError trait

Producer types implement `SubstrateAwareError` to expose the
canonical shape:

```rust
pub trait SubstrateAwareError {
    fn to_diagnostic(&self) -> Diagnostic;
}
```

Implemented today for:

* `BuildError` — every variant maps to a Diagnostic with sensible
  default advocacy. Phase implementers SHOULD override via wrapper
  types when more specific guidance applies.

Pending impls (additive, non-breaking):

* `Finding` — already carries `Advocacy` via `WithAdvocacy`;
  conversion is mechanical.
* `loom_lint::LintWarning` — to lift loom-lint diagnostics into the
  same shape.
* Crawler detector outputs — same shape so JSON reports unify.

## Why this matters

Before #295, every producer emitted its own ad-hoc shape:

* Finding had `phase` + `message` + `severity` + optional `advocacy`.
* BuildError had per-variant fields with no advocacy contract.
* Crawler detectors emitted free-form text into journey logs.

Consumers (CLI report, MCP tool output, JSON report serialization)
had to special-case each. Operators got inconsistent guidance — some
findings shipped with `why+fix+skill+anti-pattern`, others were one-
line shouts with no remediation hint.

Per `[[tool-starvation-anti-pattern]]` + `[[substrate-only-path]]`:
the substrate's job is to point operators (and AI agents) at the
correct path. A diagnostic without advocacy is the same failure mode
as a missing skill — you know you're stuck, but you don't know the
out.

The Diagnostic struct + SubstrateAwareError trait centralize the
contract. Once every producer adopts, every consumer can render
one shape.

## Migration plan

1. **(now)** Diagnostic type + SubstrateAwareError trait + BuildError
   impl shipped in `forge-core::diagnostic`.
2. **(next)** Add `Finding::to_diagnostic()` as a non-breaking impl;
   existing callers keep using `Finding` directly.
3. **(later)** loom-lint LintWarning + crawler detector outputs adopt
   the same shape.
4. **(eventual)** The CLI report renderer + the MCP typed-tool surface
   consume Diagnostic only — producer-specific code paths retire.

Each step is a separate PR, separately reviewable. The doctrine is
load-bearing but the migration is gradual; no flag day.

## What this doctrine does NOT do

* Doesn't change severity semantics — `Severity::Strict` vs `Warn`
  + `BuildMode::Poc` vs `Production` escalation rules remain in the
  Finding type.
* Doesn't replace `BuildError` — that type still carries phase
  failures upstream; SubstrateAwareError is the adapter layer.
* Doesn't mandate a localized message — `message` stays English-only
  for now; localization is a separate doctrine.
* Doesn't enforce one Diagnostic per producer — phases may emit
  multiple Diagnostics in one Vec.

## Consumer expectations

Code that reads diagnostics MUST:

* Not match on `code` strings except inside the producer's own
  namespace (e.g. `ident_conformance` code may match `"ident-NNN"`
  but MUST NOT match `"var-NNN"` — that's another producer's
  contract).
* Render `advocacy.substrate_fix` prominently when non-empty —
  operators read the fix before the message in most cases.
* Treat `cited_rules` as a doctrine cross-reference — `forge doctrine
  query --rule <id>` resolves to the rationale.

## Future axis adds

* **Severity on Diagnostic** — today severity lives on Finding only.
  When BuildError-derived diagnostics need severity (e.g. config
  errors are strict, missing-dep is warn-if-fallback-exists), a
  `severity` field will be added to Diagnostic. Non-breaking due to
  `#[non_exhaustive]`.
* **Locale field** — for future i18n; English-only message stays as
  the source-of-truth, locale field carries the translation.
* **Trace id** — link the diagnostic to the build run's trace span
  for observability.

This doctrine is the unification surface across every phase, every
producer, every consumer. v1 ships the canonical shape; future
axes refine without breaking.
