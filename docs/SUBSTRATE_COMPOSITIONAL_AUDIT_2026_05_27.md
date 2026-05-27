# Compositional primitive coverage audit — 2026-05-27

Per task #357. Walks Forge's existing compositional / layout
primitive surface and compares against what the 5 reference
sites need beyond vertical-stack section composition.

## Existing compositional surface in Forge

### Section-level composition (CmsSection variants)

| Primitive | Shape | Reach |
|---|---|---|
| `Compose` | heading + Vec<CmsBlock> (free composition of atomic blocks) | reached (forge_lite uses it for Divider + Spacer) |
| `SplitHero` | text + visual side-by-side, 2-col | reached |
| `Group` | heading + body, framed as `<section>` | never reached |
| `Sidebar` | aside-column shape | never reached |
| `Columns` | N-column container (items: Vec<String>) | never reached |
| `GridLayout` | N×M grid container | never reached |
| `Stack` | vertical stack with gap | never reached |
| `Cluster` | horizontal-wrapping cluster | never reached |
| `MosaicGrid` | irregular gallery layout | never reached |
| `ImageGrid` | uniform grid of images | never reached |
| `AsideNote` | inline call-out aside | never reached |
| `VerticalNav` | side-rail nav | never reached |
| `AnchorList` | TOC-style anchor list | never reached |

### Block-level composition (CmsBlock variants)

| Primitive | Shape | Reach |
|---|---|---|
| `Container` | wrapping container with padding | unverified |
| `Row` | horizontal row of children | never reached |
| `Column` | vertical column of children | never reached |
| `Grid` | grid container | never reached |
| `Card` | bounded card frame | never reached |
| `AspectRatio` | aspect-ratio-locked wrapper | 2 occurrences |
| `Sheet` | bottom-sheet overlay | 3 occurrences |
| `Drawer` | side drawer overlay | never reached |
| `Dialog` | modal dialog | 3 occurrences |

### Page-shell composition (ChromeKind variants)

| Variant | Shape |
|---|---|
| `PageShell` (default) | sticky top-bar header + cream body backdrop |
| `FloatingPill` | floating capsule nav with glass-morphism |
| `Minimal` | no full header chrome; first section carries it |

## Per-site compositional needs

### paulgraham.com — vertical-stack only

| Need | Coverage |
|---|---|
| Single-column vertical stack | ✓ default composition |
| No chrome (no nav, no footer) | ✗ ChromeKind has no `None` variant |

Single gap: no-chrome page-shell. Same gap surfaced in
`docs/SUBSTRATE_GAP_CATALOG_2026_05_27.md` Tier 1 #4.

### jvns.ca — illustrative + code editorial

| Need | Coverage |
|---|---|
| Vertical stack of posts | ✓ |
| Code + side-explanation side-by-side | partial — `Compose` + `Row` (never reached); no first-class "code with annotation column" primitive |
| Multi-panel comic strip (4-6 horizontally-arranged panels) | ✗ Closest is `MosaicGrid` (never reached); not designed for comics |
| Date-stamped post-list (date column + title column) | ✗ Approximate with `Columns`+`Stack` but loses semantic shape |

### rust-lang.org — documentation hub with sidebar TOC

| Need | Coverage |
|---|---|
| Three-column layout: left TOC + center content + right secondary nav | ✗ No primitive — `Sidebar` is single-aside only; doesn't compose with center+right |
| Sticky-position TOC (scrolls with content but pins at top) | ✗ No primitive — would need a new ChromeKind or Sidebar variant |
| Tabs across docs vs blog vs governance | partial — `NavTabs` exists but never reached; no first-class "documentation tab strip" |
| Breadcrumbs above content | partial — `Breadcrumb` block reaches 2; no integration into page-shell |

### kinfolk.com — magazine asymmetric layouts

| Need | Coverage |
|---|---|
| Full-bleed photo as section divider (no chrome above/below) | ✗ Same gap as decorative audit Tier 1 #1 |
| Asymmetric 2-col (60/40, 70/30, 30/70) | ✗ `Columns` is equal-width only; no asymmetric variant |
| Pull-quote in side margin (marginalia) | ✗ `marginalia` CmsSection exists but never reached; no first-class "quote in margin" comp |
| Issue grid (4-up of photo + title + author) | ✗ approximate with `FeatureSpotlight` columns=4 but loses semantic shape; `MosaicGrid` exists |
| Generous-margin reading column | ✗ `ContentWidth::Narrow` exists but Prose-measure variant missing per #348 |

### gov.uk — task flows + form layouts

| Need | Coverage |
|---|---|
| Task list with step indicators | ✗ `Steps` exists but never reached; no integrated progress tracker |
| Form + help-panel side-by-side | ✗ No primitive |
| Multi-step form with persistent progress sidebar | ✗ Form is single-section; no flow-state model |
| Service-status banner above main content | ✗ Can use `AnnouncementBar` but doesn't render in-flow |
| Breadcrumbs for civic IA | ✗ Same gap as rust-lang |

## Aggregate compositional gap list

### Tier 1 — affects 3+ sites

1. **No-chrome page-shell variant** (paulgraham, jvns, gov.uk task-flow steps)
   `ChromeKind::None` — single substrate change. Already on Tier 1 of `SUBSTRATE_GAP_CATALOG_2026_05_27.md` #4.

2. **Documentation-style sidebar-TOC composition** (rust-lang primarily; pattern reused for kinfolk issue-TOC + gov.uk task-progress-sidebar)
   New page-shell variant: `ChromeKind::DocsShell { toc_position: Left | Right }`. Substantial — page-shell layout change. Substrate gain: 3-column responsive (TOC | content | optional secondary) with sticky scroll behavior.

3. **Breadcrumb integration into page-shell** (rust-lang + gov.uk + commerce)
   `CmsPage.breadcrumbs: Vec<Crumb>` field; page-shell renders above first section. The `Breadcrumb` block exists; needs page-shell-level surface. Bounded.

### Tier 2 — affects 2 sites or one heavily

4. **Asymmetric N-column composition** (kinfolk + jvns code-with-explanation)
   Extend `Columns` to accept ratio specs: `Columns { ratios: Vec<u8>, items: Vec<...> }`. Or new variant `AsymmetricColumns`. Bounded.

5. **Comic-strip primitive** (jvns)
   Already on decorative audit Tier 2 #4. Compositional + decorative gap.

6. **Mid-flow full-bleed photo section** (kinfolk)
   Already on decorative audit Tier 1 #1. Single-substrate variant.

7. **Multi-step form with flow state** (gov.uk)
   Substantial — new `FormFlow` CmsSection with steps, progress, back-button-aware navigation, save-and-resume. Substantial substrate work.

8. **Form + help-panel side-by-side composition** (gov.uk)
   Could be expressed via the asymmetric-columns work (#4) + form variant. Lower priority.

### Tier 3 — surfacing existing primitives via doc-query

9. **Compositional primitives that exist but aren't reached**
   - `Group` — heading + body composition
   - `Stack` / `Cluster` — gap-based layout
   - `Columns` / `GridLayout` — equal-width layouts
   - `Sidebar` — single-aside composition
   - `MosaicGrid` / `ImageGrid` — image layouts
   - `AsideNote` — inline call-out
   - `VerticalNav` / `AnchorList` — TOC-style nav

   All shipped, never reached. Surfacing via doc-query expansion (per #355 recommendation + #398) addresses this.

## Why so much existing composition is unreached

The reachability audit (#355) found 64% of CmsSection variants
never reached. The compositional cluster overlaps that — most
layout primitives are never reached because:

1. **`Compose` covers the basic case.** When you need a block-tree
   layout, `Compose` works for most situations. Operators don't
   reach for `Stack` / `Cluster` / `Columns` because they don't
   know they exist + `Compose` is the path of least resistance.

2. **Documentation gap.** None of these primitives has a
   doc-query entry surfacing it for typical use cases.

3. **No worked examples.** A test fixture exercising `Columns`
   or `Sidebar` end-to-end would make them discoverable; none
   exists.

This is a **discoverability problem**, not a primitive-count
problem. The substrate is adequately compositional; the surface
to reach the composition is missing.

## Recommended next moves

**Substrate code work (bounded):**
- Tier 1 #1 (`ChromeKind::None`): ~half-day
- Tier 1 #3 (breadcrumb integration): ~1 day
- Tier 2 #4 (asymmetric columns): ~1 day

**Substantial substrate work:**
- Tier 1 #2 (DocsShell): ~1 week
- Tier 2 #7 (FormFlow): ~2-3 weeks

**Documentation work (orthogonal, ships in parallel):**
- Tier 3 (doc-query entries for the 13 unreached compositional
  CmsSection variants): ~half-day to draft each entry

## Mapping to existing tasks

- **#359** (BlockKind coverage) — owns Tier 1 #1, #3 + Tier 2 #4, #5, #7
- **#398** (doc-query expansion) — owns Tier 3
- **#360** (neutralize defaults) — implicit; surfacing compositional
  alternatives reduces the SaaS-vertical-stack default

## Honest scope note

This audit doesn't address every compositional pattern in
existence — only what the 5 reference sites observably use that
the substrate doesn't express well. Other compositional shapes
(dashboards, data tables, kanban boards, calendar grids) aren't
in the reference set; they'd surface as gaps if those site
types entered the reference set.
