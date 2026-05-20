# Reference Matching — substrate doctrine

**Status:** doctrine. Closes task #290. Captures the arc design
for tasks #263-#274 + #291 + #292: how Forge ingests a real site
(reference), extracts its compositional decisions, and synthesizes
a Forge CMS that targets the same shape — without becoming a
SaaS-trope copy of every input.

Pair with `docs/VARIATION_GUARANTEES.md`: variation guarantees
defend *across* sites; reference matching is the *within-site*
fidelity story when paul says "build me something that looks like
this site, but on the substrate."

---

## The five-decision-layer architecture

A real site embeds compositional decisions at five layers. Reference
matching extracts each layer separately, maps each into substrate
primitives + tokens + identity declarations, then synthesizes a
CmsPage that re-expresses the layered decisions.

| Layer | What's captured | Substrate target |
|-------|-----------------|------------------|
| **Visual** | Color palette + typography + spacing + decorative treatments | `[loom_tokens]` + theme variants |
| **Compositional** | Section boundaries + ordering + nesting + density rhythm | `[[sections]]` array + density tier |
| **Structural** | Page-type taxonomy + nav structure + cross-page patterns | `[[site_identity.content_type]]` + page-shell |
| **Content** | Voice register + sentence-length distribution + jargon density | `[site_identity.voice]` declaration |
| **Interactive** | Hover states + form behaviors + transition curves + scroll triggers | Loom primitive variant selection |

Each layer extracts independently. The mapping engine (#273) is
where layers compose into a single CmsPage.

## The capture pipeline (#263)

Headless rendering via Crawler. For each reference URL:

1. Fetch the page at 390 / 768 / 1280 px viewports.
2. Capture full DOM + computed styles + screenshot.
3. Extract resources: fonts loaded, images displayed, animations
   triggered, scripts executed.
4. Emit a structured `ReferenceCapture` JSON file per
   (URL, viewport) pair.

Per memory `[[crawler-stays-general-purpose]]`: the capture
pipeline is Crawler-side, consumer-agnostic. The Forge-side
extraction works against the structured capture, not against the
raw page.

## Per-axis extraction (#264-#272)

Each axis has its own extractor with a structured output type.

| Task | Axis | Output |
|------|------|--------|
| #264 | Color palette | `Vec<PaletteEntry { hex, occurrence_count, contrast_class }>` |
| #265 | Typography | `Typography { font_family, size_distribution, weight_set, leading_ratio }` |
| #266 | Spacing | `Spacing { rhythm_unit_px, section_gap_p95, content_max_width_px }` |
| #267 | Motion | `Motion { transition_curves, scroll_triggers, hover_treatments }` |
| #268 | Sections | `Vec<SectionPattern { kind_guess, density_tier, slot_signature }>` |
| #269 | Pattern library | Cumulative cross-extraction catalog |
| #270 | Structural | `Structural { nav_shape, page_type_distribution, cross_page_links }` |
| #271 | Voice | `Voice { sentence_distribution, jargon_phrases, vocabulary_tier_guess }` |
| #272 | Interactive | `Interactive { form_kinds, hover_states, scroll_triggers }` |

Each extractor is a pure function: `fn extract(capture: &ReferenceCapture) -> AxisResult`.
Deterministic. Hashable. Comparable across reference sites.

## The mapping engine (#273)

Takes the AxisResults from one or more captures and produces a
target site spec. Algorithm:

1. **Palette** → loom token overrides. The top N colors become
   `--loom-color-primary`, `--loom-color-accent`, etc. Contrast
   classification feeds the theme split (light vs dark backgrounds).
2. **Typography** → font-stack + size scale tokens.
3. **Spacing** → rhythm + density tier.
4. **Sections** → CmsSection variants. Each extracted section
   pattern maps to a substrate-native primitive (e.g. centered
   single-line hero → `hero_editorial` with monospace kicker per
   editorial_purity gate's substrate-correct counterpart).
5. **Voice** → `[site_identity.voice]` declaration.
6. **Interactive** → motion + treatment per-primitive variant.
7. **Structural** → `[[content_type]]` declarations + nav block.

The mapping engine is *opinionated*: SaaS-trope shapes in the
reference are converted to their editorial counterparts, not
copied. Per the doctrine, Forge refuses to reproduce
`feature_spotlight 3-column grid` even when the reference uses
it — the substrate uses `kv_pair` dense info panels instead.

## The multi-reference engine (#274)

When the operator supplies multiple reference sites, the engine
weights each axis independently. Weights default to:

* Palette: weighted average (color blend feels natural)
* Typography: dominant choice wins
* Spacing: median (rhythm normalizes to closest fit)
* Sections: union with deduplication (more variety, but cap at
  pattern_entropy budget)
* Voice: voice closest to declared `[site_identity.voice]` tier
* Interactive: union of motion + treatment vocabulary

The operator can override weights via `[reference_matching.weights]`
in forge.toml.

## Synthesis (#291)

The mapped spec is emitted as a `cms/<page>.json` file that the
existing Forge build pipeline consumes. No new render path; the
synthesis output is just standard CMS JSON.

This means the entire variation-arc enforcement (#231-#262) still
applies to synthesized sites. A reference-matched site MUST pass:

* `uniqueness_gate` — synthesized fingerprint distinct from
  registry entries.
* `mood_lock` — synthesized mood matches declared mood (if any).
* `pattern_entropy` — synthesized site shows enough variety.
* `composition_lineage` — within-site variant vocabulary coherent.
* `editorial_purity_gate` — no SaaS-trope shapes (the mapping
  engine should already have avoided these).

If synthesis output violates any gate, the synthesis pipeline
returns errors and the operator iterates the input references or
weights.

## Operator UX (#292)

Two-phase confirmation:

1. **Spec preview** — after extraction + mapping, before
   synthesis, show the operator the structured spec (palette,
   tokens, primitive selection, voice tier). The operator can
   edit tokens, swap primitives, narrow content-types.
2. **Synthesis preview** — after synthesis, before commit, show
   the rendered output at the three reference viewports. Diff
   visually against the reference captures.

The operator commits the cms/*.json + forge.toml changes only
when both previews are accepted.

## Doctrine — what reference matching DOES NOT do

* **Doesn't copy content.** Voice register is extracted, but
  actual text remains the operator's responsibility. Reference
  matching produces an empty CMS scaffold with declared voice;
  the operator fills in the substance.

* **Doesn't reproduce SaaS tropes.** The mapping engine is
  opinionated: feature_spotlight 3-col → kv_pair editorial.
  stat_band → sparkline. hero centered → hero_editorial. The
  doctrine prefers substrate-native shapes over reference
  fidelity when the reference is a SaaS trope.

* **Doesn't bypass variation gates.** Synthesized sites pass
  through every variation enforcement phase. A site that's a
  near-duplicate of a registry entry refuses to build, even if
  it's a faithful reference match — the operator must introduce
  distinguishing variation.

* **Doesn't lock the operator out.** Every spec is editable
  before synthesis. The mapping engine produces a recommendation,
  not a fait accompli.

## Pairing with the variation arc

Reference matching feeds *into* the variation arc, not around it:

```text
  reference URL(s)
       │
       ▼
  capture pipeline (#263)
       │
       ▼
  per-axis extraction (#264-#272)
       │
       ▼
  mapping engine (#273) ───► spec preview ───► operator edit
       │
       ▼
  synthesis (#291) ───► cms/<page>.json
       │
       ▼
  forge build (variation arc enforces uniqueness + identity)
       │
       ▼
  synthesis preview ───► operator confirm ───► commit
```

The synthesis output is just CMS JSON. Every existing gate runs.
No special path; reference matching is a substrate front-door,
not a substrate side-door.

## Open questions (when this arc lands)

* **How to score "this is too close to the reference?"** —
  fingerprint distance >= threshold from the reference's own
  fingerprint, OR operator override.
* **Caching captures.** Reference captures should be cached by
  URL + viewport + content hash. Re-capture when the cached
  capture is stale > N days.
* **Legal scope.** Per memory `[[lfi-out-of-scope]]`, certain
  reference repos (sacred.vote) are off-limits to this Claude
  instance; reference matching against their *public* site is
  fine but never their repo.

## Engineering acceptance criteria

A change to the reference-matching arc is acceptable iff:

1. `cargo test -p forge-core` + `cargo test -p forge-phases` +
   `cargo test -p crawler` pass.
2. Each new axis extractor ships with at least 5 unit tests:
   golden-vector parsing, deterministic output, idempotent
   re-extraction, cross-platform consistency, error handling.
3. The synthesis output passes every variation arc gate.
4. The operator preview UX shows the spec before commit.

The doctrine is load-bearing; the engineering protects it.

## Cross-references

* `docs/VARIATION_GUARANTEES.md` — what variation enforcement
  guarantees the synthesized sites still respect.
* `docs/SUBSTRATE_ERRORS.md` — diagnostic shape every extractor
  + mapper + synthesis step uses.
* `docs/ARCHITECTURAL_THREADS.md` — task-coverage map.
* `docs/EDITORIAL_PURITY.md` — the SaaS-trope dictionary the
  mapping engine cross-references.
