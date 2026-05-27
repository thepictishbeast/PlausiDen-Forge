---
name: forge-modify-primitive
description: Modify an existing primitive — add a variant, field, decoration, or property — without breaking back-compat. Workflow #5 of the paired skill+MCP series (#368). Classifies every change as invisible / additive / auto-migration / operator-action per backward_compat_version_discipline doctrine.
metadata:
  tags: [loom, primitives, backward-compat, doctrine, modify]
  related_doctrine_rules: [prim-001, prim-002, prim-003, prim-004, prim-005, prim-006, prim-007, prim-008, prim-009, prim-010, prim-011, prim-012, backward_compat_version_discipline]
  related_traits: []
  paired_mcp_tool: forge.modify_primitive
  workflow_status: paired
---

# Modify an existing primitive

Use this skill when an existing primitive needs an EXTENSION (new variant, new field, new decoration) and you want to avoid breaking tenant content already written against the current shape. Distinct from `add-loom-primitive` (#366) which creates a new primitive from scratch.

## When to invoke

Recognition signals:
- An existing primitive (`CmsSection::FeatureSpotlight`, `CmsBlock::Card`, `HeroBackground`, etc.) needs one more shape and a current tenant or reference site demonstrates the need.
- A new theme would benefit from a per-primitive decoration variant.
- A field is being added to capture a hint the renderer already accepts implicitly.

Anti-signals:
- The shape is brand-new and unrelated to existing primitives → use `add-loom-primitive` (#366).
- The change would remove a variant or rename a field → that's a breaking change; follow the migration-friendly path per `[[backward-compat-version-discipline]]` (signed migration registry), not this skill.
- The change is site-specific (only one tenant would use it) → rule prim-012; reject as out-of-scope for substrate.

## The 4 change categories

Per `backward_compat_version_discipline` doctrine, every primitive modification falls into one of four categories:

| Category | What it means | Tenant impact |
|---|---|---|
| `invisible` | Internal refactor, no wire-shape change | None — tenants don't notice |
| `additive` | New variant, new optional field with `#[serde(default)]`, new enum case | Backward-compatible — existing TOML still parses |
| `auto_migration` | Renamed field / variant where a signed migration map can rewrite tenant content | Auto-handled at build time |
| `operator_action` | Required field added / variant removed / shape change | Tenants must edit their content |

The MCP tool `forge.modify_primitive` classifies the proposed change and routes accordingly.

## Prerequisites

1. **Read `[[backward-compat-version-discipline]]` doctrine.** This skill is its operational arm.
2. **Identify the primitive** by exact CmsSection/CmsBlock/sub-enum name (e.g. `FeatureSpotlight`, `Card`, `HeroBackground`).
3. **Read its current definition** in `crates/loom-cms-render/src/lib.rs` + its render impl.
4. **Run `forge.add_primitive`** with the proposed extension's name — the duplicate-check guard surfaces whether the change really is an extension or a new primitive in disguise.

## Procedure

### 1. Call the paired MCP tool

```jsonc
{
  "name": "forge.modify_primitive",
  "arguments": {
    "primitive_name": "FeatureSpotlightDecoration",
    "change_kind": "additive",
    "change_summary": "Add `Brutalist` variant alongside Decorated/Editorial/Minimal"
  }
}
```

Required:
- `primitive_name` — the exact Rust type name being modified
- `change_kind` — one of: `invisible`, `additive`, `auto_migration`, `operator_action`
- `change_summary` — one-line description

The tool validates the classification + surfaces the substrate-side discipline for that change kind.

### 2. Implement the change per category

**For `additive`:**
1. Add the new variant / field with `#[serde(default)]`.
2. Extend the render impl to handle it.
3. Add a snapshot test pinning the new render output.
4. Add a `doc_query` entry surfacing the new variant.

**For `auto_migration`:**
1. Add the new shape alongside the old (`#[serde(alias = "old_name")]`).
2. Register a migration entry in the signed migration registry.
3. Update `doc_query` to point at the new name; mark the old as deprecated.
4. Plan a future cycle for old-name removal.

**For `operator_action`:**
1. Add the new required field / removed variant in a feature-flagged module.
2. Emit a `forge build` Warn finding in the current cycle.
3. Plan promotion to Strict + breaking-change release notes.

**For `invisible`:**
- Refactor freely. Run the full test suite to confirm no behavior change.

### 3. Run the build pipeline

`forge build --root <tenant_root> --json` against a tenant that uses the primitive. The audit phases enforce:

- No regression in `aesthetic_distinctiveness`
- `variation-arc` budgets remain valid
- New variants get a `doc_query` entry per `tool-starvation-anti-pattern`

### 4. Update doctrine + memory if the change shifts a default

If the change adds a new "preferred" default per `neutralize_defaults` (#360), update the per-PageKind dispatch table in the dispatch doctrine doc.

## Common pitfalls

### Pitfall 1: Misclassifying an additive change as invisible

If the change touches the wire shape AT ALL, it's not invisible. Anything that affects `serde::Serialize` output of `CmsPage` is at minimum additive.

### Pitfall 2: Forgetting `#[serde(default)]` on new fields

New struct field without `#[serde(default)]` makes the entire primitive fail to parse for existing tenants. The MCP tool flags this case as `operator_action`, not `additive`.

### Pitfall 3: Renaming via `auto_migration` without registering the migration

Just adding `#[serde(alias = "old")]` works in-process but loses the historical record of what was renamed when. The signed migration registry is the canonical store.

### Pitfall 4: Modifying primitives for one tenant's benefit

Prim-012 still applies. If only one tenant needs the new shape, the answer is per-tenant corpora (per `[[per-tenant-corpora-doctrine]]`), not primitive modification.

## Acceptance criteria

1. ✓ Change kind classification matches the actual wire-shape impact.
2. ✓ Existing tenant content still builds (additive + invisible + auto_migration).
3. ✓ Snapshot tests cover the new shape.
4. ✓ `doc_query` entry surfaces the new variant.
5. ✓ Audit phases (variation_arc, aesthetic_distinctiveness) accept the change.

## Mapping to substrate

- **MCP tool**: `forge.modify_primitive`
- **Doctrine rules**: `backward_compat_version_discipline`, `prim-012`, `prim-001..011`
- **Related skills**: `add-loom-primitive` (#366) for entirely new primitives; `forge-doctrine-violation-explanation` (#373) when audit phases flag the change
