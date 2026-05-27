---
name: forge-reference-extraction
description: Run the deterministic URL → SiteSpec pipeline against a captured reference site. Workflow #8 of the paired skill+MCP series (#371). Consumes a Crawler-emitted CaptureManifest; emits a SiteSpec via forge-core's 8-axis extractor + reference_mapping::map_to_spec.
metadata:
  tags: [forge, reference, extraction, pipeline, synthesis]
  related_doctrine_rules: [agent_workflows_must_be_paired, content-001]
  related_traits: []
  paired_mcp_tool: forge.reference_extraction
  workflow_status: paired
---

# Reference extraction

Use this skill to convert a captured reference site (paulgraham.com, jvns.ca, rust-lang.org, kinfolk.com, gov.uk, or any other observed-band URL) into a substrate SiteSpec that can drive a `forge build`. Distinct from `forge-build-site-from-brief` (#364): briefs synthesize structure from text; this workflow extracts structure from a live URL via the Crawler runner.

## When to invoke

Recognition signals:
- The operator wants to study how an existing site is structured and produce a buildable substrate spec from it
- A reference-site capture has been emitted by the Crawler runner and lives at `runs/<slug>/manifest.json`
- The brief is "build it like <url>" rather than free-text content

Anti-signals:
- The URL is out-of-band (e.g. dashboards, complex web apps that the substrate's vocabulary doesn't express); see `[[dont-pixel-reproduce-outside-band]]`
- The capture doesn't exist yet → run `crawler-runner` reference-capture mode FIRST against the URL; this skill consumes captures, doesn't make them
- The intent is content-only (the structure should be substrate-default; only the prose differs) → use `forge-build-site-from-brief` (#364)

## Prerequisites

1. **A CaptureManifest exists** at `<capture_dir>/manifest.json` per `crawler-reference-capture::CaptureSpec` v1
2. **The capture includes computed-styles dumps** for every viewport (`computed_styles_path` set on each ReferenceCapture)
3. **A tenant_id + site_id** chosen (kebab-case)
4. **(For full extraction)** the chromiumoxide runner has been verified end-to-end against the target URL — confirm the screenshot artifacts and computed-styles dumps look plausible before extracting

## Procedure

### 1. Verify the capture

The MCP tool first validates the CaptureManifest. Call:

```jsonc
{
  "name": "forge.reference_extraction",
  "arguments": {
    "capture_dir": "/home/paul/projects/PlausiDen-Crawler/runs/paulgraham",
    "site_id": "paulgraham-shape",
    "tenant_id": "paulgraham-shape"
  }
}
```

Required:
- `capture_dir` — absolute path; manifest.json must exist inside
- `site_id`, `tenant_id` — kebab-case identifiers for the emitted SiteSpec

The tool returns the captures found (count, URLs, viewports) and any spec-mismatch / readability errors.

### 2. Review the capture summary

Before running extraction, verify:
- Capture count ≥ 1 viewport (typically 3-4 for desktop/tablet/mobile)
- Computed-styles paths resolve
- network_summary fields populated (fonts_loaded, image_count, etc.)

### 3. Run the per-axis extractors

Currently the MCP tool returns the capture summary; full per-axis extraction (palette, typography, spacing, motion, structural, voice, sections, interactive) runs through `forge-core::extractors::*` once the Crawler runner is verified end-to-end. Per the audit at `docs/SUBSTRATE_REFERENCE_PIPELINE_AUDIT_2026_05_27.md`, this is the integration boundary; the library functions exist.

For now, drive extraction directly through `forge-core` until the chromiumoxide runner verification lands. After verification, this MCP tool will surface `ExtractedSignals` then call `map_to_spec` and return the `SiteSpec`.

### 4. Emit the SiteSpec

The SiteSpec returned by `map_to_spec` is consumed by:
- `forge synthesis preview` — review before writing
- `forge.build_site_from_brief` (#364) — actual build invocation with the spec

## Common pitfalls

### Pitfall 1: Pixel-reproducing out-of-band sites

The reframe is explicit: out-of-band sites (dashboards, complex SPAs, video-first) should NOT be driven through this pipeline. The substrate vocabulary doesn't express them; extraction produces low-fidelity results. Stop at recognition; redirect to a non-Forge build path.

### Pitfall 2: Capture from one viewport

A single-viewport capture extracts a single density read. Run the Crawler at desktop + tablet + mobile minimum to give the spacing extractor enough data to classify the density tier.

### Pitfall 3: Treating extracted spec as authoritative content

`map_to_spec` produces a STRUCTURAL spec — section ordering, density, theme palette, primitive distribution. It does NOT extract verbatim copy. Tenants using this workflow still need to author their own content. The structure is the starting point.

### Pitfall 4: Skipping content-originality check post-extraction

If the operator hand-fills cms/*.json with content reflecting the source site's voice closely, the result may verbatim-overlap the source. Always run `forge.verify_content_originality` (#369) after this workflow with the source corpus in `corpus_roots`.

## Acceptance criteria

1. ✓ CaptureManifest loaded without error
2. ✓ ≥ 1 viewport captured with computed_styles_path resolvable
3. ✓ (When extraction lands) SiteSpec produced via map_to_spec
4. ✓ Originality check (#369) passes against the source corpus
5. ✓ Fingerprint check (#370) acceptable distance from the source

## Mapping to substrate

- **MCP tool**: `forge.reference_extraction`
- **Backing modules**: `forge-core::reference_capture::CaptureManifest::read`,
  `forge-core::extractors::*` (8 axes), `forge-core::reference_mapping::map_to_spec`
- **Doctrine rules**: `content-001`, `agent_workflows_must_be_paired`, `dont-pixel-reproduce-outside-band`
- **Related skills**: `forge-build-site-from-brief` (#364), `forge-verify-content-originality` (#369), `forge-site-fingerprint-check` (#370)
- **Pipeline audit**: `docs/SUBSTRATE_REFERENCE_PIPELINE_AUDIT_2026_05_27.md`
