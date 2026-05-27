---
name: forge-build-site-from-brief
description: Build a complete tenant site from a written brief — parses the brief, scaffolds SiteSpec, emits cms/*.json, runs forge build, surfaces audit findings. Workflow #1 of the paired skill+MCP infrastructure (#363/#364).
metadata:
  tags: [forge, site-build, workflow, brief, synthesis]
  related_doctrine_rules: [agent_workflows_must_be_paired, build-001, content-001, content-002]
  related_traits: []
  paired_mcp_tool: forge.build_site_from_brief
  workflow_status: paired
---

# Build a site from a written brief

Use this skill when an operator or AI agent needs to take a brief describing a site and produce a complete, building, audited tenant site. This is the canonical orchestration path for "build me a site that does X."

## When to invoke

Recognition signals:
- "Build a site for ..." or "Create a tenant for ..."
- A brief document exists (Markdown / TOML / plain text) describing the goal of the site.
- The site does not yet exist in `/home/paul/projects/PlausiDen-<TenantName>/`.

Anti-signals:
- The site already exists and a SCOPED change is needed → use `forge-modify-site` skill (#365) instead.
- The brief describes a pixel-reproduction of a live URL → use `forge-reference-extraction` skill (#371) which calls into the deterministic URL→SiteSpec pipeline.
- The brief describes a NEW primitive needed by the site → use `add-loom-primitive` skill (#366) FIRST, then return here.

## Prerequisites

Before invoking the paired MCP tool:

1. **Read the brief end-to-end.** Note the operator's goal, audience, page kind (marketing landing / brief / portfolio / editorial / docs / civic / commerce), tone, and any explicit constraints (theme, density, copy length, primitives to feature or avoid).

2. **Read `/home/paul/projects/PlausiDen-Forge/docs/SUBSTRATE_REFERENCE_SITES_2026_05_27.md`** to recall which observed-band shape the brief most closely resembles. Out-of-band briefs (e.g., gov.uk-style civic IA) should be flagged per `feedback_dont_pixel_reproduce_outside_band`.

3. **Run `forge.doctrine.for` with `path: "crates/forge-core/src/synthesis"`** to load applicable content / build doctrine before generating.

4. **Decide the site slug.** Use kebab-case matching the eventual `/home/paul/projects/PlausiDen-<TenantName>/` directory.

## Procedure

### 1. Inspect the brief shape

Briefs come in three shapes the substrate handles natively:

| Shape | Format | Handling |
|---|---|---|
| Structured | TOML or JSON with explicit `pages`, `theme`, `density`, `kind` | Direct → SiteSpec |
| Semi-structured | Markdown with H1/H2 sections per page + prose | Section detection → SiteSpec |
| Free-text | Plain paragraphs | LFI / LLM candidate generation, then SiteSpec |

Free-text briefs require deterministic-first + LFI-optional resolution per the `deterministic_first_lfi_optional` doctrine. Default behavior: the MCP tool refuses free-text briefs and emits a structured-brief template the operator fills.

### 2. Call the paired MCP tool

```jsonc
{
  "name": "forge.build_site_from_brief",
  "arguments": {
    "brief_path": "/path/to/brief.toml",
    "tenant_root": "/home/paul/projects/PlausiDen-NewTenant",
    "site_id": "newtenant",
    "tenant_id": "newtenant",
    "dry_run": true
  }
}
```

Required:
- `brief_path` — absolute path to the brief file
- `tenant_root` — absolute path where the tenant repo will live
- `site_id` — kebab-case site identifier
- `tenant_id` — kebab-case tenant identifier (often same as site_id)

Optional:
- `dry_run` (default `true`): when true, prints the planned SiteSpec without writing anything. When false, writes `tenant_root/cms/*.json`, then runs `forge build` and returns the structured report.

### 3. Review the dry-run output

The MCP tool returns the planned SiteSpec summary. Check:

- **Page count + kinds** match the brief's intent.
- **No primitive is repeated more than 3 times** per page (variation-arc audit will flag).
- **No section is the SaaS-default `FeatureSpotlight::Decorated`** if PageKind is brief/editorial/civic (per `neutralize_defaults` survey).
- **No tenant-specific content** is named in a way that would lock the substrate to one tenant.

If anything is off, edit the brief and re-run dry-run. Iterate until the SiteSpec matches the brief's intent.

### 4. Commit the build

Re-call the MCP tool with `dry_run: false`. The tool will:

1. Write `tenant_root/cms/<page-slug>.json` files.
2. Write `tenant_root/forge.toml` + `tenant_root/phases.toml` if absent.
3. Run `forge build --root tenant_root` and surface the structured report.

### 5. Handle audit findings

The build will surface findings from every phase. Common categories:

- **Strict findings**: block the build until resolved. Edit the brief or the cms/*.json files and re-run.
- **Warn findings on the long tail**: surfacing alternatives that the brief didn't ask for. Decide per the brief's spirit whether to expand the SiteSpec.
- **Findings on band drift**: PageKind doesn't match default values used (per `neutralize_defaults` doctrine). Either change the brief's `kind` or override the defaults explicitly per-section.

## Common pitfalls

### Pitfall 1: Free-text briefs masked as structured

If the brief contains placeholder prose ("Lorem ipsum"), the MCP tool produces a SiteSpec full of empty `content_substance` findings. Substrate's response is the right one: writing placeholder pages doesn't pass the build. Either supply real content in the brief, or commit to LFI candidate generation as a separate explicit step.

### Pitfall 2: Pixel-reproduction shaped as brief

When the brief is "build it like kinfolk.com", the operator wants reference-extraction (#371), not from-brief synthesis. The brief workflow synthesizes structure from text; reference-extraction extracts structure from a live URL. Don't conflate them.

### Pitfall 3: Substrate-vocabulary mismatch

If the brief asks for a primitive the substrate doesn't have ("we need a multi-step booking calendar"), do NOT route around by hand-authoring. Per `[[substrate_only_path]]` doctrine, propose adding the primitive (via `add-loom-primitive` skill #366), then return to this workflow.

### Pitfall 4: Tenant-name pollution

Substrate stays generic. Brief content can name the tenant ("Acme Co. is..."), but the SiteSpec slug / cms/*.json filenames stay structural ("about", "pricing", "contact" — not "acme-about"). The MCP tool enforces kebab-case structural slugs.

## Acceptance criteria

The workflow is complete when:

1. ✓ Dry-run output matches the brief's intent (operator-verified).
2. ✓ Real-run produces cms/*.json files passing strict audit phases.
3. ✓ `forge build` exits 0 with no Strict findings.
4. ✓ Site renders in `loom edit serve` at the tenant root (when applicable for that PageKind).
5. ✓ Per `keep_docs_tests_logs_current` doctrine: every commit carries doc + test + audit-output updates inline.

## Mapping to substrate

- **MCP tool**: `forge.build_site_from_brief` (registered in `forge-core::workflow_registry`)
- **Backing module**: `forge-core::synthesis::SiteSpec` (existing), `reference_mapping::map_to_spec()` (for URL briefs that overlap with #371)
- **Audit phases**: every Forge build phase runs against the generated cms/*.json — variation-arc, content-substance, default-band-drift (when shipped per #360), aesthetic_distinctiveness, etc.
- **Doctrine rules**: `build-001`, `content-001`, `content-002`, `agent_workflows_must_be_paired`

## Output token economy

Per `[[tool-starvation-anti-pattern]]` doctrine: this workflow consumes one MCP call per build cycle. The structured SiteSpec + build report come back as JSON, not CLI text — avoiding the "parse the build log" failure mode.
