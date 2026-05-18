# Explanation: why typed everything

> **Diátaxis tier: Explanation** — understanding-oriented. The
> "why" behind PlausiDen's typed-surface approach. Not a tutorial,
> not a recipe, not a reference. Read this when you want to
> understand the design choices.

## The short version

Every platform boundary — config files, CLI arguments, deploy
adapters, AI generation stages, audit log entries, status page
state — uses a typed Rust shape with `deny_unknown_fields` serde.
There is no "configuration via convention" anywhere.

## Why not just YAML + duck typing?

It's tempting. Many platforms ship config in YAML or JSON with
loose schemas, validated lazily at use-time. The objections:

### 1. Typos don't fail loud

A YAML config with a misspelled field gets silently ignored. The
intent ("use the strict severity") disappears at parse time. The
gate runs as if the field weren't there. The operator only
discovers the bug when prod traffic exhibits the wrong behaviour
weeks later.

`deny_unknown_fields` flips this. The parser refuses anything not
in the schema; the build fails immediately with the typo's exact
location.

### 2. Refactor safety is structural, not textual

When PlausiDen adds a new capability (say, a new `BlockKind`
variant for the CMS), every `match` in the codebase that handles
all variants becomes a compile-time check. Rust's exhaustive-match
won't let stale handlers silently miss the new kind.

This is the same property as a closed-enum vs an open string slot.
PlausiDen uses closed enums everywhere a fixed set exists:
`Severity`, `BlockKind`, `DeployTarget`, `BrowserEngine`, `DrTier`,
`StatusLevel`, etc.

### 3. Cross-tier comparison is honest

The deploy-security-rating dashboard (task #43) is a pure
projection over each deploy adapter's typed `SecurityProfile`. It
can't lie because there's no string-typed `"high"` it can swap in
for the actual Strong/Medium/Low anonymity rating.

The status page (task #68) computes its `Ok` / `Degraded` /
`PartialOutage` / `MajorOutage` level deterministically from
typed `SliMeasurement` inputs. The operator can't fudge the
output without changing the inputs, which the audit log captures.

### 4. The boundary is the audit point

In the manifest-attest layer (#64), every artifact's signature
records the typed `AttestableKind` it covers. A signature over a
build report can't be replayed as a signature over a manifest —
the kind discriminator is part of what gets signed.

If `AttestableKind` were a free-form string, an attacker could
forge a "signed build report" by replaying a manifest signature
with a renamed kind. Closed enum + typed signing prevents this
class of attack at the schema layer.

## What the trade-off costs

Typed everything has real costs:

* **More upfront design work.** Operators authoring a new adapter
  have to write the typed config struct AND the serde derives AND
  the test cases.
* **Schema migrations are explicit.** When the shape changes, every
  consumer needs to be updated. There's no graceful soft-rollout
  where new fields drift in.
* **Operators who want freedom can't get it.** A site that wanted
  to attach arbitrary YAML to a Page can't — the typed
  `Page::meta` field is a `BTreeMap<String, String>`, not
  `serde_json::Value`. Operators with specific structured needs
  add them to the typed surface explicitly.

PlausiDen accepts those costs because the alternative — silent
drift between layers, untestable cross-component invariants — is
worse for a platform that wants to make verifiable guarantees
about reliability, security, and accessibility.

## Where to learn more

* The discipline doctrine: [feedback_iso_standards](https://github.com/thepictishbeast/PlausiDen-Forge/blob/main/docs/ENGINEERING_DISCIPLINES.md)
* The keystone manifest layer: [Reference: manifest.toml](../reference/manifest-toml.md)
* The supply-chain gate: `.github/workflows/supply-chain.yml`
  (deny.toml at workspace root)
* The protocol-boundary fuzz tests: `.github/workflows/protocol-fuzz.yml`
