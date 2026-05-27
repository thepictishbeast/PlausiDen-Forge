---
name: forge-verify-content-originality
description: Verify a tenant's content is original — not verbatim-reused from reference corpora, exemplar libraries, or other tenants. Anti-reuse gate. Workflow #6 of the paired skill+MCP series (#369).
metadata:
  tags: [forge, anti-reuse, originality, content, doctrine]
  related_doctrine_rules: [content-001, content-003, agent_workflows_must_be_paired]
  related_traits: []
  paired_mcp_tool: forge.verify_content_originality
  workflow_status: paired
---

# Verify content originality

Use this skill before shipping any tenant content to confirm that strings, headings, body copy, and CTAs are not verbatim copies of:
- Other tenants' content in the substrate
- Reference corpora (kinfolk, gov.uk, exemplar libraries)
- Prior-generation content from the same tenant being recycled without acknowledgment

## When to invoke

Recognition signals:
- A new tenant's cms/*.json has been authored (whether by operator, brief synthesis, or reference extraction)
- An existing tenant is making a substantial content refresh
- Suspicion that LFI / LLM candidate-generation pulled from training data verbatim

Anti-signals:
- Boilerplate strings that legitimately repeat (legal disclaimers, copyright notices, ARIA labels, ISO date formats) — these are stable substrate text, not reuse
- Single-word matches — semantic overlap is expected; the gate is for substantive multi-word reuse

## Why this matters

Verbatim reuse creates four failure modes:

1. **Legal**: copying language from one site to another without license breaks intellectual-property doctrine
2. **Brand homogeneity**: the substrate's output looks "samey" if 50 tenants share 20 phrases
3. **Trust**: visitors who recognize text from another site lose trust in the originating brand
4. **Discoverability**: search ranking penalizes duplicate content

The substrate gate refuses to ship content that matches reference corpora beyond a configurable threshold.

## Prerequisites

1. **The tenant cms/*.json must exist** and be readable.
2. **A reference corpus to compare against** must be available — typically `/home/paul/projects/PlausiDen-Forge/corpora/` for substrate-shipped baselines, plus any sibling tenant cms/ directories you want to check against.
3. **Decide the match threshold**: default is 6-word shingles. Lower for stricter checks, higher for loose checks.

## Procedure

### 1. Call the paired MCP tool

```jsonc
{
  "name": "forge.verify_content_originality",
  "arguments": {
    "tenant_root": "/home/paul/projects/PlausiDen-NewTenant",
    "corpus_roots": [
      "/home/paul/projects/PlausiDen-Forge/corpora",
      "/home/paul/projects/PlausiDen-OtherTenant/cms"
    ],
    "min_ngram_words": 6
  }
}
```

Required:
- `tenant_root` — absolute path to the tenant under check (cms/*.json files are read)
- `corpus_roots` — array of absolute paths to compare against
- `min_ngram_words` — default 6; configurable per check

### 2. Review the overlap report

The tool returns:
- `total_tenant_strings`: count of text fields scanned in the tenant
- `total_corpus_strings`: count of text fields scanned in the corpus
- `overlaps`: list of `{ phrase, tenant_file, corpus_file, ngram_count }`
- `verdict`: `ok` (no overlaps above threshold), `flag` (small overlap, soft Warn), `block` (substantial overlap, hard reject)

### 3. Resolve overlaps

For each `block`-verdict overlap:
- Confirm the overlap is substantive content reuse, not boilerplate
- Either re-author the tenant phrase OR explicitly license the corpus phrase (add an attribution note)
- Re-run the verification

For `flag`-verdict overlaps:
- Decide per-content whether the overlap is acceptable
- If acceptable, mark as `[strict.exempt]` in the tenant's forge.toml with reason

## Common pitfalls

### Pitfall 1: Boilerplate flagged as reuse

ISO date formats, ARIA labels, legal disclaimers, copyright lines, and standard CTAs (`Sign up`, `Learn more`) appear identically across many tenants because the substrate provides them. Adjust `min_ngram_words` upward, or accept the flags as boilerplate without blocking ship.

### Pitfall 2: Internal style guide phrases

If the operator runs multiple PlausiDen-* tenants and they share house-style language ("plausibly deniable", "sovereign-first"), those will trigger overlaps. Solution: add them to a tenant-specific `[exempt.house_style]` allowlist with justification.

### Pitfall 3: Skipping corpus for "small" sites

A 3-page brief tenant feels too small to bother checking. Run anyway; sometimes the smallest tenants are most likely to copy verbatim because their content was generated quickly.

### Pitfall 4: Treating all overlaps as equal

A 6-word legal-fine-print phrase shared across tenants is benign. A 6-word hero-headline shared across tenants is bad. The verdict groups by source file kind to help; review each cluster separately.

## Acceptance criteria

1. ✓ `verdict: ok` for all tenant strings against the corpus_roots provided
2. ✓ Any `flag`-level overlaps reviewed + accepted with reason logged
3. ✓ Re-running the check after content edits remains stable (overlaps don't reappear)
4. ✓ Tenant `forge.toml` `[exempt]` entries (if any) carry reasons

## Mapping to substrate

- **MCP tool**: `forge.verify_content_originality`
- **Doctrine rules**: `content-001`, `content-003`
- **Related skills**: `forge-build-site-from-brief` (#364) — typically the originality check follows the build; `forge-site-fingerprint-check` (#370) — structural fingerprint counterpart to this content fingerprint
- **Future hook**: when the fingerprint registry (#376) lands, this workflow could feed corpus fingerprints rather than re-scanning every time
