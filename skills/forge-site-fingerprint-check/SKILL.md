---
name: forge-site-fingerprint-check
description: Compute a site's structural fingerprint and check against the registry for near-duplicates. Structural counterpart to forge-verify-content-originality (#369). Workflow #7 of the paired skill+MCP series (#370).
metadata:
  tags: [forge, fingerprint, structure, anti-duplicate, doctrine]
  related_doctrine_rules: [content-001, content-003, agent_workflows_must_be_paired]
  related_traits: []
  paired_mcp_tool: forge.site_fingerprint_check
  workflow_status: paired
---

# Site fingerprint check

Use this skill to detect structural duplication: when two tenants have the same section order, the same primitive distribution, the same density, the same composition rhythm — even if their *content* is different. Two sites with the same structural fingerprint look like the same template applied twice.

## When to invoke

Recognition signals:
- A new tenant has been built and you want to confirm its structure is distinct from existing tenants
- Multiple tenants are sharing themes / primitives heavily and you want to verify they're not collapsing into a single template
- Post-modification check: did `forge.modify_site` produce a result that's now indistinguishable from another tenant?

Anti-signals:
- The tenant is a one-page brief (paulgraham-shaped) — single-page sites have shallow structural fingerprints; the check is less meaningful
- Both tenants legitimately should share structure (e.g., two regional editions of the same publication) — exempt via configuration

## Why structural fingerprints matter

Content originality (#369) catches verbatim text reuse. Structural fingerprint catches the deeper failure: even with original copy, if every tenant follows the same `Hero → FeatureSpotlight 3-up → Testimonial → CTA` pattern at the same density, every output looks like the same SaaS marketing site with different words.

The reframe identified this as **substrate-default-band collapse**: not a single tenant's failure but the substrate's tendency to produce one template repeatedly. The fingerprint registry surveys the fleet for collapse.

## Prerequisites

1. **The tenant cms/*.json must exist** at `tenant_root/cms/`.
2. **Decide on the registry path**. Default: `/home/paul/projects/PlausiDen-Forge/fingerprint-registry.jsonl` (the canonical signed registry). You can use a custom path for ad-hoc comparisons.
3. **Decide the distance threshold**. Default: 4. Lower = stricter (catch even small structural overlap); higher = looser (allow more shared shape).

## Procedure

### 1. Call the paired MCP tool

```jsonc
{
  "name": "forge.site_fingerprint_check",
  "arguments": {
    "tenant_root": "/home/paul/projects/PlausiDen-NewTenant",
    "registry_path": "/home/paul/projects/PlausiDen-Forge/fingerprint-registry.jsonl",
    "distance_threshold": 4
  }
}
```

Required:
- `tenant_root` — absolute path; `tenant_root/cms/` is where the section data is read

Optional:
- `registry_path` — defaults to the canonical fingerprint registry
- `distance_threshold` — defaults to 4

### 2. Review the report

The tool returns:
- `tenant_fingerprint_commitment` — hex commitment of the computed fingerprint
- `total_registry_entries` — entries scanned
- `near_duplicates` — array of `{ entry, distance }` pairs sorted by distance
- `verdict` — `ok` (no near-duplicates), `flag` (near-duplicates exist), `block` (exact match or very close)

### 3. Resolve

For each `block` or close `flag`:
- Identify which structural axis collides (section ordering / primitive distribution / density / composition rhythm / asset distribution)
- Modify the tenant to add distinguishing structure (different section order, different density tier, different theme that uses different primitives)
- Re-run the check

For `flag`-level only: review whether the collision is acceptable for the tenant pair. Some pairs (sibling brands, A/B test variants) legitimately share structure.

## Common pitfalls

### Pitfall 1: Treating all near-duplicates as failures

The registry can legitimately contain the same tenant at multiple build cycles. A new build of `tenant-alpha` matching an old build of `tenant-alpha` is expected, not a failure. The MCP tool dedupes by `tenant_id` when comparing.

### Pitfall 2: Threshold tuning per call

Calling with threshold=10 to make a flag "go away" defeats the purpose. The default 4 is calibrated against the substrate's known structural diversity. Adjust only when there's a specific structural-class reason.

### Pitfall 3: Confusing this with content originality

Content originality (#369) and structural fingerprint (this) catch different failures. A tenant that passes content originality can still fail fingerprint check, and vice versa. Run BOTH before shipping.

### Pitfall 4: Skipping the registry append

After a tenant passes both checks, the operator should append its fingerprint to the registry (via `forge fingerprint append` or equivalent). Otherwise future tenants can't compare against it.

## Acceptance criteria

1. ✓ Tenant fingerprint computed without error
2. ✓ `verdict: ok` against the canonical registry at the chosen threshold
3. ✓ Any `flag`-level near-duplicates reviewed + accepted with reason
4. ✓ Post-ship: tenant fingerprint appended to the canonical registry

## Mapping to substrate

- **MCP tool**: `forge.site_fingerprint_check`
- **Backing modules**: `forge-core::fingerprint::build_from_cms_dir`,
  `forge-core::fingerprint_registry::find_near_duplicates`
- **Doctrine rules**: `content-001`, `content-003`
- **Related skills**: `forge-verify-content-originality` (#369) is the content counterpart; both should run before ship.
- **Future hook**: when `fingerprint_registry` (#376) gains an anti-pattern dictionary, the check will refuse to ship structures matching anti-patterns.
