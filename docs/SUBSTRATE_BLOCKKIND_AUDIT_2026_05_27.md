# Content-model BlockKind coverage audit — 2026-05-27

Per task #359. Walks the 62 CmsBlock variants against the 5 reference
sites' atomic-block needs and identifies which never-reached blocks
represent real gaps vs which are filler.

## Methodology

Reach numbers carry over from the substrate reachability audit
(`SUBSTRATE_REACHABILITY_AUDIT_2026_05_27.md`). This audit drills into
the 20 CmsBlock variants that the reachability sweep flagged as never
reached:

```
alert, avatar, badge, card, carousel, column, definition_list,
empty_state, figure, grid, iframe, kbd_shortcut, link, list,
row, stat, stepper, table, timeline, video
```

For each, the question is the same: does any of the 5 reference sites
(`SUBSTRATE_REFERENCE_SITES_2026_05_27.md`) need this primitive?

## Per-variant judgement

### Tier 1 — real gap, used by 2+ reference sites or used heavily

| Block | Sites that need it | Verdict |
|---|---|---|
| `Alert` | gov.uk (service-status banner above content), rust-lang (deprecation notices in docs), jvns (TIL callouts) | **Surface** — exists but unreached |
| `Avatar` | jvns (author bio strip), kinfolk (issue contributors) | **Surface** — exists but unreached |
| `Card` | kinfolk (issue thumbnails 4-up), rust-lang (project card grid on landing) | **Surface** — exists but unreached |
| `Figure` | kinfolk (photo + caption, semantic), jvns (illustration + caption) | **Surface** — exists but unreached |
| `Table` | rust-lang (data tables in docs), gov.uk (rate / schedule tables) | **Surface** — exists but unreached |
| `Link` | every site uses inline links inside paragraphs; "never reached as a block" because authors reach for it via prose markdown, not as a structured block | **Accept** — the never-reached status is a measurement artifact; inline links flow through prose tokens, not direct `CmsBlock::Link` |
| `List` | every site has bulleted / numbered lists; same artifact — flowed through prose | **Accept** — measurement artifact, same as Link |

### Tier 2 — niche but high-value for one site

| Block | Site that needs it | Verdict |
|---|---|---|
| `Stepper` | gov.uk (task-flow progress) | **Surface + extend** — exists; needs flow-state integration per compositional Tier 2 #7 |
| `DefinitionList` | gov.uk (definition pairs in service descriptions), rust-lang (term + meaning glossaries) | **Surface** — exists but unreached |
| `KbdShortcut` | rust-lang (docs reference keyboard shortcuts), tool docs generally | **Surface** — exists but unreached |
| `EmptyState` | gov.uk (search no-results, history empty), commerce | **Surface** — exists but unreached |
| `Video` | kinfolk (mood-piece video), tutorial sites | **Surface** — exists but unreached |
| `Iframe` | jvns (embedded interactive demos) | **Caution** — unreached for security reasons; surfacing it needs phantom-button-style backend-vetting + sandboxing doctrine |

### Tier 3 — composition-layer; addressed by compositional audit

| Block | Verdict |
|---|---|
| `Column`, `Row`, `Grid` | **Already in compositional audit Tier 3 #9** — surfacing these is doc-query work, not new substrate |
| `Carousel` | **Surface, but flag** — carousels have known a11y / engagement problems; surface with cautionary doc-query entry indicating preferred alternatives (`MosaicGrid`, `ImageGrid`, content-tabs) |

### Tier 4 — legitimately rare, no reference-site gap

| Block | Verdict |
|---|---|
| `Stat` | **Surface low-priority** — `KvPair` covers most factual surfacing; `Stat` is a numeric-emphasis variant. No reference site forces the need. |
| `Timeline` | **Surface low-priority** — gov.uk service-status timeline is one possible use, but `Steps` / `Stepper` covers it. |
| `Badge` | **Surface low-priority** — kinfolk uses small tags, but inline `<span class="tag">` text in prose covers it. |

## Aggregate

Of 20 never-reached CmsBlock variants:

- **2 are measurement artifacts** (Link, List flow through prose
  tokens; the "never reached as a block" finding is technically true
  but practically misleading)
- **11 are real reach gaps in shipped primitives** (Alert, Avatar,
  Card, Figure, Table, Stepper, DefinitionList, KbdShortcut,
  EmptyState, Video, Iframe) — addressed by doc-query expansion
- **3 are composition-layer** (Column, Row, Grid) — addressed by
  compositional audit Tier 3
- **1 needs cautionary doc-query** (Carousel)
- **3 are legitimately low-priority surface** (Stat, Timeline, Badge)

Net: **0 reference-site needs require new BlockKind variants.** The
content-model vocabulary is adequately wide for the 5 reference sites.
The bottleneck is discoverability, identical to the reachability
audit's central finding.

## Required NEW BlockKinds outside the existing 62

The reference sites surfaced one BlockKind-shaped gap that the
existing primitives don't cleanly cover:

### `CodeAnnotation` — inline annotation on a specific line of `Code`

jvns.ca uses code blocks with prose annotations pointing at specific
lines. The existing `Code` block is line-flat — no anchor for
annotations. Two design options:

1. **Composition route**: `Compose` + `Code` + `AnchorList` ties
   anchor IDs to line numbers manually. Authors hand-roll, no
   semantic surface.
2. **Substrate route**: `Code { annotations: Vec<Annotation> }`
   where `Annotation { line: u32, text: String }`. Render emits the
   annotations as ARIA-described margin notes.

Option 2 is the right substrate move. Bounded — one field added to
existing `Code` block, render extension, doc-query entry.

### `ServiceStatus` — civic service-status banner with timeline

gov.uk uses a service-status banner above main content with current
state + history. Could be expressed via `Alert` + composition, but
loses semantic surface. Optional new variant; deprioritized — `Alert`
with structured `tone` already covers the surface need.

## Page-level gaps (not BlockKind)

These don't fit BlockKind but surfaced during the audit:

- **`CmsPage.published_at: Option<IsoDate>`** — paulgraham, jvns
  date-stamp posts (already noted in decorative audit Tier 2 #7)
- **`CmsPage.author: Option<Author>`** — jvns author identity
  (already noted in decorative audit Tier 2 #6)
- **`CmsPage.breadcrumbs: Vec<Crumb>`** — rust-lang, gov.uk
  (already noted in compositional audit Tier 1 #3)

These three page-level fields are bounded substrate work and are the
highest-leverage moves to land in `CmsPage` itself.

## Mapping to existing tasks

- **#398** (doc-query expansion) — surfaces the 11 Tier-1 / Tier-2
  unreached BlockKinds via new canonical_index entries
- **#356** (decorative audit) — owns Avatar surface + page-level
  author / published_at fields
- **#357** (compositional audit) — owns Column / Row / Grid surface
  + breadcrumb integration
- **#359** (this audit) — produces the 1 BlockKind extension
  recommendation (`Code.annotations`) + the 3 `CmsPage` field
  recommendations; design-led, not in scope for this audit doc

## Honest scope note

This audit measures REFERENCE-SITE-NEED gaps, not theoretical content-
model completeness. A larger reference set (commerce, dashboards,
data-heavy SaaS, video-first) would surface different gaps:

- Commerce: `ProductBadge`, `PriceLockup`, `InventoryStatus` (none in
  reference set)
- Dashboards: `MetricCard`, `TimeseriesChart`, `DataTable` (extensions
  of `Table`) — not in reference set
- Video-first: `VideoPlayer`, `VideoChapters`, `Subtitles` — not in
  reference set

Per the substrate reframe (2026-05-21), the substrate should ship the
vocabulary that observed-band sites need; speculative coverage for
hypothetical verticals would re-introduce the over-engineering the
reframe rejected. The 5-site reference frame is the operative
constraint until paul expands it.

## Conclusion

The content-model layer is **adequately wide**. No new variants are
required to cover the 5 reference sites. The single substantive
extension is `Code.annotations` for jvns-style code editorial. The
three `CmsPage` field additions (author, published_at, breadcrumbs)
are the higher-leverage moves and are tracked in the decorative +
compositional audits.

The work that closes the reference-site BlockKind gap is documentation
(doc-query expansion) and surface (worked examples), not new typed
variants.
