# Reference extraction pipeline — current state + remaining gaps

Per task #361. The "deterministic URL → substrate spec" pipeline is
**substantially built** (~5500 LoC across Forge + Crawler). This
document audits what is wired end-to-end and where the remaining gaps
sit before the pipeline can be invoked by an operator from a single
URL.

## End-to-end pipeline diagram

```
+--------+   chromiumoxide     +------------------+
|  URL   | ------------------> | CaptureManifest  |
+--------+   (Crawler runner)  | + per-viewport   |
                               |   ReferenceCapture|
                               | + screenshot     |
                               | + html dump      |
                               | + computed-styles|
                               +--------+---------+
                                        |
                                        | (forge-core consumes)
                                        v
   +--------------------+----------------+----------------+
   | extractors::palette  typography  spacing  motion    |
   | extractors::sections structural  voice    interactive|
   +----------------------+------------+------------------+
                                        |
                                        v
                          +-----------------------------+
                          | reference_mapping::         |
                          |   ExtractedSignals          |
                          |   → map_to_spec()           |
                          +-----------------------------+
                                        |
                                        v
                          +-----------------------------+
                          | synthesis::SiteSpec         |
                          | (cms/<page-slug>.json files)|
                          +-----------------------------+
                                        |
                                        v
                          +-----------------------------+
                          | forge build (existing path) |
                          | dist/<site>/...html         |
                          +-----------------------------+
```

## Surface area already shipped

### Crawler-side (PlausiDen-Crawler::crates/crawler-reference-capture)

- `CaptureSpec` enum (versioned wire shape)
- `ReferenceCapture` struct: url, viewport_px, screenshot_path,
  html_path, computed_styles_path, network_summary
- `NetworkSummary`: fonts_loaded, image_count, video_count,
  script_count, third_party_origins, total_bytes
- `CaptureManifest`: site_slug + Vec<ReferenceCapture>
- `CaptureError` (Io / Json / SpecMismatch / BadTimestamp)
- 442 LoC; v0.1.0; AGPL-3.0-or-later

### Forge-side capture mirror (forge-core::reference_capture)

- Mirrors `CaptureSpec` / `ReferenceCapture` / `NetworkSummary` /
  `CaptureManifest` / `CaptureError` so the substrate can deserialize
  what the Crawler writes
- 412 LoC

### Forge-side per-axis extractors (forge-core::extractors)

- `extractors::palette` (405 LoC) → `Vec<PaletteEntry>`
- `extractors::typography` (413 LoC) → `TypographyResult`
- `extractors::spacing` (348 LoC) → `SpacingResult`
- `extractors::motion` (474 LoC) → `MotionResult`
- `extractors::sections` (503 LoC) → `Vec<PatternClassification>`
- `extractors::structural` (386 LoC) → `StructuralResult`
- `extractors::voice` (397 LoC) → `VoiceResult`
- `extractors::interactive` (391 LoC) → `InteractiveResult`

Each extractor is a pure function taking `&ComputedStylesDump` and
returning its typed result.

### Forge-side aggregation + mapping (forge-core::reference_mapping)

- `ExtractedSignals` struct aggregating all 8 axes
- `map_to_spec(site_id, tenant_id, signals) → SiteSpec` — pure function
- Heuristics for mood / density / per-section translation
- 337 LoC

### Forge-side multi-reference blending (forge-core::reference_composition)

- `WeightedReference` (path + weight)
- `BlendedSignals`
- `compose_multi(site_id, tenant_id, refs) → SiteSpec` — blends
  multiple reference captures with weights
- 504 LoC

## What is NOT wired

### 1. `chromiumoxide` runner (Crawler-side)

The `crawler-reference-capture` crate is the wire contract only —
its doc-comment is explicit: "*runner does the IO*". The actual
chromiumoxide runner that:
- Navigates to a URL
- Waits for layout / network idle
- Captures screenshot at multiple viewports
- Dumps post-render HTML
- Dumps computed-styles per element

…is referenced as the Crawler runner that calls into the contract.
The runner exists in `crawler-runner` but its reference-capture
mode needs verification — currently this audit cannot confirm
end-to-end runner-side capture has been tested at the chromiumoxide
layer for the 5 reference sites.

### 2. `forge-cli` ingestion subcommand

No `forge reference ingest <capture-dir>` / `forge reference extract`
subcommand exists. The library functions are reachable from Rust but
no operator-facing CLI entry exists. This is the single highest-
leverage missing piece: with it, an operator can take a Crawler
output directory and produce a `SiteSpec` deterministically.

### 3. `forge-mcp` reference tool

No `forge.reference.extract` MCP tool exists. AI agents can't call
the pipeline through MCP. Same fix as #371 (workflow 8) in the
backlog — actually #371 is the dedicated task for this gap.

### 4. SiteSpec → cms/*.json emit

`SiteSpec` is the in-memory shape. The synthesis backend (mentioned
in `reference_mapping.rs:5`) writes `cms/<page-slug>.json` files,
but the wiring from `map_to_spec()` output → on-disk cms files
through the existing forge build pipeline needs verification at
the CLI level.

### 5. Per-site captures of the 5 reference sites

No `fixtures/reference-captures/<site-slug>/` directories exist yet
in the Forge repo for paulgraham, jvns, rust-lang, kinfolk, gov.uk.
The pipeline cannot be tested end-to-end without input data.

## Gap-closure roadmap

In dependency order:

1. **Crawler reference-capture runner verification** — confirm
   chromiumoxide runner produces a complete `CaptureManifest` for
   one reference URL (paulgraham as smallest baseline). Estimated:
   1-2 days if the runner exists, 1-2 weeks if it needs writing.

2. **`forge reference ingest <capture-dir>`** CLI subcommand —
   loads `CaptureManifest`, runs all 8 extractors, calls
   `map_to_spec()`, prints or writes the `SiteSpec`. Pure Rust
   wiring, no new types. Estimated: 1 day.

3. **`forge reference build <capture-dir>`** CLI subcommand —
   ingestion + writes cms/*.json + runs the existing forge build
   pipeline. Closes the URL-to-built-site loop. Estimated: 2-3 days.

4. **5-site reference captures** — actually capture paulgraham, jvns,
   rust-lang, kinfolk, gov.uk through the runner and snapshot the
   `CaptureManifest`s into `fixtures/reference-captures/`. Estimated:
   1 day after step 1 lands.

5. **`forge.reference.extract` MCP tool** — owned by #371; thin
   wrapper over the CLI. Estimated: half-day.

6. **End-to-end snapshot tests** — given a captured `paulgraham/`
   directory, the pipeline produces a stable `SiteSpec`. Estimated:
   1 day.

Total to deterministic URL → built site: **~2-3 weeks**.

## What this audit changes

The reframe task #361 was scoped as "build the pipeline". The audit
reveals the pipeline is ~80% built — what remains is **integration
wiring**, not new architecture. This recategorizes #361:

- From "build the pipeline" (multi-week design + implementation)
- To "wire the existing pipeline through CLI + MCP" (~1 week of
  integration + 2 weeks of runner verification + capture acquisition)

The substrate-side library is essentially complete. The gap is
operator-facing surface (CLI / MCP) and input data (captures).

## Mapping to existing tasks

- **#371** (forge_reference_extraction + paired skill / workflow 8) —
  owns the MCP-tool gap (gap #3)
- **#380** (exemplar libraries) — depends on captures from gap #4;
  exemplars are extracted from real reference sites
- **#398** (doc-query expansion) — orthogonal, no dependency

## Honest scope note

This audit does NOT verify the runner end-to-end. The
`crawler-runner` crate may or may not have working reference-capture
mode against chromiumoxide; confirming requires actually running
it against a live URL with a screenshot artifact landing on disk.
That verification is gap #1 and is the leading edge of remaining
work.

The audit also doesn't measure extractor *quality* — whether the
typography extractor actually produces useful results against
kinfolk's serif display type, or whether the section classifier
correctly identifies kinfolk's photographic-divider sections. Those
are extractor-output quality questions that surface only with real
captures and ground-truth comparison.

## Conclusion

The deterministic URL → substrate-spec pipeline exists as Rust
library functions across 11 modules and ~5500 LoC. The remaining
work is operator-facing surface (CLI + MCP subcommands) and
end-to-end verification with real captures. No new architecture is
needed; the integration is bounded and on the order of 1-3 weeks
to fully close.
