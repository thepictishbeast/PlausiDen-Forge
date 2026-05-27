---
name: forge-modify-site
description: Apply a scoped modification to an existing tenant site (content change, theme swap, primitive substitution, density change, page add/remove) without rebuilding from scratch. Workflow #2 of the paired skill+MCP series (#365).
metadata:
  tags: [forge, site-modify, workflow, scoped-change]
  related_doctrine_rules: [agent_workflows_must_be_paired, build-001, content-001]
  related_traits: []
  paired_mcp_tool: forge.modify_site
  workflow_status: paired
---

# Modify an existing tenant site

Use this skill when an operator or AI agent needs to apply a SCOPED change to a tenant that already has shipped content. Distinct from `forge-build-site-from-brief` (#364) which builds from zero; this skill assumes `tenant_root/cms/*.json` exists.

## When to invoke

Recognition signals:
- "Change the theme on tenant X to ..." / "Make tenant X's about page denser" / "Add a /pricing page to tenant Y"
- A single concrete modification is requested (one axis: theme, density, content, structure)
- The tenant_root directory already exists and has cms/*.json files

Anti-signals:
- The site doesn't exist yet → use `forge-build-site-from-brief` (#364)
- The change is a full redesign / pixel-reproduction → use `forge-reference-extraction` (#371)
- The needed primitive doesn't exist → use `add-loom-primitive` (#366) first
- The change is purely procedural / content-only with no structural impact → operator can hand-edit cms/*.json + run `forge build` directly (no MCP tool needed)

## Modification kinds the substrate supports

The MCP tool `forge.modify_site` accepts one of these typed modifications per call:

| Kind | Required fields | Effect |
|---|---|---|
| `change_theme` | `theme` | Swap `forge.toml [identity].theme_preference` to the named theme; rebuild |
| `change_density` | `density` | Swap `forge.toml [identity].density_preference` to the named tier; rebuild |
| `change_page_kind` | `page`, `kind` | Update `forge.toml [identity].kind` (or per-page override) to the named PageKind |
| `add_page` | `slug`, `kind`, `sections` | Write `cms/<slug>.json` with the section list; rebuild |
| `remove_page` | `slug` | Delete `cms/<slug>.json`; rebuild |
| `content_edit` | `page`, `section_index`, `field`, `value` | Patch a single field in `cms/<page>.json`; rebuild |

Modifications outside this set are out-of-scope for this workflow and should either:
1. Be decomposed into multiple `forge.modify_site` calls
2. Use `add-loom-primitive` if they require a new primitive
3. Use `extend-doctrine-rules` if they require a new doctrine rule

## Prerequisites

1. **Confirm the tenant_root exists** and contains `cms/*.json` + `forge.toml`. If not, this is a build, not a modify.
2. **Read the current state**: glance at `cms/*.json` and `forge.toml [identity]` to know what's there before changing it.
3. **Choose the modification kind** from the table above. If your change is "I want both a new theme AND a new page", that's two calls, not one.
4. **Decide on dry-run vs real-run**: always dry-run first.

## Procedure

### 1. Call the paired MCP tool with dry_run

```jsonc
{
  "name": "forge.modify_site",
  "arguments": {
    "tenant_root": "/home/paul/projects/PlausiDen-NewTenant",
    "modification_kind": "change_theme",
    "modification_path": "/tmp/modify.toml",
    "dry_run": true
  }
}
```

Where `modification_path` is a TOML file shaped per the modification kind:

```toml
# change_theme example
theme = "editorial"
```

```toml
# add_page example
slug = "pricing"
kind = "marketing_landing"
[[sections]]
kind = "hero_editorial"
heading = "Plans for every team"
```

### 2. Review the dry-run output

The tool reports:
- Which file(s) would change
- The current value(s) being replaced
- The proposed value(s)
- Any audit phases that would be re-run

Verify:
- Only the intended file is touched
- The change doesn't violate `neutralize_defaults` (e.g., changing theme to a SaaS-band theme on a `kind = "editorial"` page would flag)
- For `add_page`: the new page's section list doesn't repeat primitives 3+ times

### 3. Commit the modification

Re-call with `dry_run: false`. The tool:
1. Writes the changed file(s)
2. Runs `forge build --root tenant_root --json`
3. Returns the structured build report

### 4. Handle audit findings

Same as `forge-build-site-from-brief` (#364) step 5. The audit phases that gate the rebuild are unchanged.

## Common pitfalls

### Pitfall 1: Multi-axis modifications in one call

"Change the theme AND the density AND add a page" is three modifications, not one. The MCP tool refuses multi-axis modifications because each axis interacts with the audit phases differently; mixing them obscures which phase flags which change.

### Pitfall 2: Hand-edit drift between modify and rebuild

If the operator hand-edits cms/*.json between dry-run and real-run, the dry-run report is stale. The tool re-validates on real-run, but the operator's mental model can be off. Treat dry-run as advisory; real-run is authoritative.

### Pitfall 3: Modifying primitives via this workflow

Changing a primitive's variant or adding a property is `modify_primitive` (#368), not `modify_site`. This workflow operates at the tenant-instance level; primitive-level changes affect every tenant and need their own workflow.

### Pitfall 4: Forgetting forge.toml is paired with cms/*.json

A `change_theme` modification updates `forge.toml`, not cms/*.json. A `content_edit` modification touches cms/*.json, not forge.toml. The tool routes by `modification_kind` — don't try to inline TOML changes in cms/*.json or vice versa.

## Acceptance criteria

1. ✓ Dry-run shows the correct delta.
2. ✓ Real-run produces the expected file change(s).
3. ✓ `forge build` after the modification exits 0 with no new Strict findings.
4. ✓ Per `keep_docs_tests_logs_current`: if the modification adds doctrine-relevant content, audit-output JSON reflects it.

## Mapping to substrate

- **MCP tool**: `forge.modify_site`
- **Backing modules**: tenant `forge.toml` parser, `forge-core::synthesis::SiteSpec`, every audit phase via `forge build`
- **Doctrine rules**: `build-001`, `content-001`, `agent_workflows_must_be_paired`
- **Related skills**: `forge-build-site-from-brief` (#364) for from-zero builds; `add-loom-primitive` (#366) for primitive surface; `forge-reference-extraction` (#371) for redesigns
