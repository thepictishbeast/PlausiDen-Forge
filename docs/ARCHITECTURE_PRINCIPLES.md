# Forge — architecture principles

> Companion to [`FORGE_VISION.md`](./FORGE_VISION.md). Vision says
> *what* Forge does; this doc says *why* the substrate is shaped
> the way it is and what invariants every future addition must
> preserve.

The recurring move across every layer: identify the implicit
assumption that produces a failure mode, make it explicit and
structural. Unbounded plugins → capability sandbox. Unbounded CSS
→ typed primitives. Unbounded AI → constrained grammar. Unbounded
drift between frontend and backend → single manifest with
generated projections. **Bound the surface; derive everything else
from the boundary.**

---

## 1. The WordPress failure mode and the inversion

WordPress's failure is *unbounded extension*. Plugins touch
anything, themes override anything, content is unstructured HTML.
The result is the chaos every WP operator knows.

Forge inverts: **the platform owns presentation and content
structure; extensions get narrow declared capabilities**. Every
architectural principle below is a corollary of that inversion.

| Unbounded WordPress mode | Forge bound | Mechanism |
|---|---|---|
| Plugins can call anything | Plugin = Wasm module with declared `capabilities: ["read:posts","write:comments"]` | Wasmtime sandbox, ambient authority denied by default |
| Themes write arbitrary CSS | Themes bind to design tokens; primitives compose tokens | `loom-lint` rejects raw class strings outside allowlist |
| Content is HTML blobs | Content is typed JSON validated against `CmsSection` schema | `phase_validate_cms` is a build gate |
| Backend and frontend drift | Single capability manifest; UI/backend/AI tools/docs are *projections* of it | Codegen + CI gates verify every projection covers every capability |
| Layout is CSS suggestion | Layout is constraint system the platform owns | Primitives use container queries + intrinsic sizing; `position: absolute` denied to authors |
| AI generates markup | AI generates *site spec* conforming to schema; renderer is identical for AI + human paths | Constrained decoding at token level; primitives are tools |

---

## 2. The capability manifest is the constitution

Every capability — query, mutation, primitive, plugin action,
admin operation, setting — is declared **once** in a typed
manifest. The declaration carries: signature, permissions,
validation rules, UI hints (label, icon, grouping, surface),
audit category, version metadata, AI-callable flag.

**Projections generated from the manifest:**

- Backend handler trait stubs, validation, OpenAPI / GraphQL
  schemas, audit hooks.
- Frontend typed client (no hand-written `fetch`), form
  components per input schema, permission gates, telemetry
  instrumentation.
- Inspector field controls per primitive property schema.
- Command palette entries.
- AI tool schema (the model's tool list IS the manifest's
  ai-callable subset; never more, never less).
- API reference, SDK reference, CLI reference docs.
- Contract tests asserting handler/schema/permission/audit
  invariants hold.

**CI gate: coverage.** For every capability the manifest declares,
assert backend handler exists, frontend has a reachable affordance
*OR* is explicitly marked `api-only` with justification, palette
entry exists, permission policy is defined, audit category
assigned, telemetry event fires, doc page exists. **And** the
inverse: orphan UI (button that calls nothing real) and orphan
capability (endpoint with no UI surface) both fail the build.

The manifest is the platform's *constitution*. Changes are
reviewed, versioned (semver per capability), deprecations are
formal (versioned with sunset windows). Plugins extend the
manifest using the same IDL — their UI affordances appear
automatically in inspector, palette, AI tool list. Plugin can't
ship a backend endpoint with no UI counterpart for the same
structural reason core can't.

---

## 3. Primitives as constraint system

A primitive is a typed component whose contract guarantees:

- **Fluid sizing.** No fixed widths. `min()`, `max()`, `clamp()`,
  `minmax()` in grid. `overflow-wrap: anywhere` + `hyphens: auto`
  on text by default. A primitive that can't shrink to its
  container is a bug in the primitive.
- **No author positioning.** `position: absolute`, negative
  margins, fixed widths, arbitrary z-index — denied. Compose from
  layout primitives (Stack / Cluster / Grid / Sidebar / Switcher
  — the Every Layout taxonomy). Z-index is a closed enum
  (`base`, `sticky`, `overlay`, `modal`, `toast`) managed by the
  platform.
- **Reserved space for async.** Every async slot declares
  aspect-ratio or min-height at parse time. CLS = 0 by
  construction.
- **A11y by construction.** Focus ring, keyboard handlers, ARIA
  roles, 44×44 tap targets are properties of the primitive, not
  author concerns. `pointer-events: none` on a region containing
  interactives is rejected by linter.
- **Reader rights preserved.** `user-select: none` denied except
  on the drag-handle primitive. Scroll-lock is a platform
  capability scoped to modal/drawer primitives that restore on
  unmount.
- **Style isolation.** Every plugin/primitive root has
  `contain: layout style` so styles can't leak out and parent
  reflows don't cascade in. Scoped stylesheets at build time;
  Shadow DOM where appropriate.
- **Viewport / safe-area aware.** `100dvh` / `100svh`, not
  `100vh`. `env(safe-area-inset-*)` on all fixed/sticky.
  VirtualKeyboard API respected so inputs aren't covered.

### Vocabulary expansion, not constraint relaxation

The failure mode of "constrained" systems is not too many
constraints — it's too few primitives. Ship **200, not 20**:
Brutalist hero, editorial split, magazine grid, asymmetric
scroll, sticky-marquee, oversized typography hero, tiled gallery,
kinetic type, full-bleed video, draggable canvas, scrollytelling
sequence, parallax stack, broken-grid mosaic. Each preserves the
primitive contract — but the design space they span is enormous.

Add **asymmetric / broken-grid primitives** that intentionally
violate the regular grid within their declared safe overlap
zones: `Offset`, `Overlap` (with declared z-order),
`Broken`, `Scattered`, `Kinetic`, `Marquee`, `Diagonal`. These
let the brutalist / editorial style packs read as "designed by
a human with taste" while the contract still holds.

### Escape hatches are explicit and gated

A small set of opt-in primitives permit near-arbitrary content:
`CustomCanvas` (sandboxed Wasm + WebGL/Canvas, declared bounds),
`ArtisticBlock` (free-positioning within a declared box, runs
through visual regression). Marked clearly in editor. Don't
compose with the safety guarantees of normal primitives. 95% case
stays in the constrained grammar; 5% bespoke-art case has a
declared escape hatch.

### Property-based testing of primitives

For every primitive, fuzz adversarial content: 200-char unbroken
strings, single emoji, RTL Arabic mixed with LTR English,
10,000-word paragraphs, zero-width characters, combining
diacritics, vertical Japanese, 1px / 10000×10000 images, empty
strings, null. Through every breakpoint (320px → 4K), every
density mode, every theme, every browser. Screenshot-diff + assert
no overflow / overlap / clipped text. This catches the long-tail
human QA never finds. **ResizeObserver in dev** logs any child
whose `scrollWidth` exceeds parent's `clientWidth` with component
stack — author sees overflow in the editor before publish.

---

## 4. Tokens as discrete axes — color, type, motion, density, formality

Tokens aren't just palette + spacing. **Aesthetic dimensions as
enums:**

- Density: `tight` / `airy` / `spacious`
- Formality: `editorial` / `technical` / `playful` / `brutalist`
- Motion intensity: `still` / `subtle` / `expressive` / `kinetic`
- Texture: `flat` / `layered` / `grainy` / `glassmorphic`
- Type personality: `humanist` / `geometric` / `mono` /
  `display-serif` / `condensed`
- Grid character: `regular` / `asymmetric` / `broken` / `organic`
- Color mood: `monochromatic` / `analogous` / `complementary` /
  `triadic` / `split-complementary` / `duotone` / `polychrome`
- Color energy: `muted` / `saturated` / `neon` / `pastel`

Each is an enum, not a free value. A site picks a point in this
space; the platform composes a coherent system from it. Tens of
thousands of valid combinations, all guaranteed coherent.

**Typography depth.** Variable font axes as tokens (weight, optical
size, slant, width). Mixed type pairings from a curated table.
Display type primitives (oversized, kinetic, mixed-script, fluid-
clamp headings spanning viewport). OpenType feature tokens
(drop caps, pull quotes, lining vs old-style figures, ligature
sets). Type is 80% of why editorial sites read different from
SaaS templates — make the type system rich.

**Color as a system.** Generated from a base hue via palette
theory rules. Application tokens beyond fills: `gradient`,
`mesh-gradient`, `grain-overlay`, `duotone-image-treatment`. Tens
of valid palettes per brand, harmonic by construction.

**AMOLED dark theme** is `bg_base = #000000` so OLED pixels turn
off (battery + contrast). Elevation grays only where depth
genuinely helps. Default unless explicit override. See
`feedback_dark_theme_amoled_true_black`.

---

## 5. Style packs as first-class artifacts

A **style pack** is a coherent bundle of token settings + primitive
preferences + motion language + asset treatments. "Swiss
editorial," "90s zine," "Y2K chrome," "Bauhaus poster,"
"post-digital brutalist," "Memphis revival," "Japanese minimal,"
"Dieter Rams." Curated by human designers, stored as data, signed,
versioned.

Packs ship with **grammars**, not just tokens. Swiss pack
constrains to a strict baseline grid, two type sizes, asymmetric
layouts, generous whitespace, no shadows. Brutalist pack permits
violated grids, raw HTML aesthetics, system fonts, harsh
contrast, overlapping elements *within designated overlap
primitives*. "Controlled chaos" is itself a primitive.

The AI picks a pack (or blends two with declared weights) rather
than inventing aesthetics from scratch. This is how you get range
without slop — the inventiveness was front-loaded into curation.

**Designer-authored marketplace.** Outside designers contribute
packs. Same review process as plugins: signed, versioned,
screenshot-tested, conforms to contract. Range expands with
marketplace; quality stays gated.

---

## 6. AI generation as bounded neurosymbolic search

The AI's job is **taste-matching within a quality floor**, not
inventing quality from scratch. The platform defines the valid
output space; the model navigates it.

- **AI generates a site spec, never markup or CSS.** Output is a
  typed document conforming to the same content schema human
  authors fill in. Renderer is identical for AI and human paths.
- **Constrained decoding at the token level.** Outlines /
  llguidance / JSON-Schema-enforced sampling so the model
  *cannot* emit invalid output. Same principle as LFI's symbolic
  substrate — symbolic layer fixes valid combinations; neural
  proposer navigates.
- **Tools = design primitives.** Each layout primitive (`Hero`,
  `FeatureGrid`, `Pricing`, `Testimonial`, `Stack`, `Cluster`) is
  a typed function exposed to the model. Composition happens via
  tool calls, never code generation.
- **Multi-stage pipeline.** Brief → IA → Wireframe (primitive
  composition per page) → Content (copy, image briefs) → Token
  overrides → Audit. Different models per stage; cheap-fast for
  IA, careful for copy, vision for visual critique. One-shot
  generation produces the slop everyone associates with AI sites;
  staged with verification produces quality.
- **RAG over a curated corpus.** Validated patterns scored on
  Lighthouse, a11y, conversion, design review. Model retrieves
  and adapts. Corpus stratified by style pack so retrieval is
  filtered by chosen pack — brutalist sites retrieve brutalist
  exemplars, not generic SaaS layouts.
- **Critic loops with targeted repair.** Each stage's output
  scored against explicit rubrics: clarity, hierarchy, SEO
  completeness, conversion patterns, voice consistency,
  accessibility. Vision-model critic renders the page and audits
  actual pixels. Failures trigger targeted revision of the
  offending block, not full regeneration.
- **SEO is platform-emitted from the content model.** Titles,
  metas, OG/Twitter cards, JSON-LD, canonical, hreflang,
  sitemaps, robots.txt, breadcrumbs — deterministic from typed
  content. AI writes copy; platform emits markup.
- **Quality gates identical to human-authored sites.** Lighthouse
  budgets, visual regression vs corpus, axe-core a11y, JS/CSS/HTML
  KB caps, CLS/LCP. AI output that fails gates routes back through
  repair or rejected. Gate doesn't care what produced the input.
- **Anti-bloat as a budget.** Hard caps per page: max N
  primitives, max nesting depth, max KB per route, max distinct
  fonts/colors. Each primitive accounted. Adding more than the
  brief warrants reduces budget score; critic flags. This is the
  lever that prevents AI from defaulting to every-feature-on-every-
  page maximalism.
- **Brand transfer through tokens, never CSS.** Vision/text model
  analyzes brand input (logo, references, voice samples,
  competitor sites) and emits *token overrides within the closed
  scale*. Primary color picks from allowed palette; type pairing
  from allowed combinations. Finite set selection, no invention.
- **Provenance on every block.** Each generated block records the
  model, prompt, retrieval sources, critic scores, alternatives
  considered. `TracedDerivation` (model chose this primitive
  because the critic preferred it over N alternatives for reason X)
  vs `ReconstructedRationalization` (post-hoc explanation).
- **Critic models tuned for aesthetics, not just correctness.**
  Score *interestingness*, *coherence*, *brand fit*, *originality*
  (low cosine similarity to corpus in style space). Pipeline
  optimizes for good *and* not generic. Sites scoring high on
  correctness + low on originality get pushed back for variation.

### AI safety as platform property

- **Prompt injection defense.** User content reaching the model
  is the attack vector. Defense in layers: input sanitization
  (strip system-prompt patterns), structural separation (user
  content is data, not instructions, enforced by prompt
  architecture + model-level instruction hierarchy), output
  validation (constrain to manifest-legal operations, reject
  outside grammar), capability isolation (AI tools have same
  capability scoping as plugins).
- **Output safety.** AI-generated content goes through same
  content-policy checks as user-generated. Image generation runs
  CSAM detection + brand/IP infringement. Red-team the AI
  specifically with documented evaluation suite gating model +
  prompt changes.
- **Cost containment.** Per-tenant AI budgets at API gateway.
  Runaway costs bounded automatically. Per-session, per-page,
  per-asset cost surfaced to user. Fallback to cheaper models on
  budget exhaustion.
- **Evaluation harness.** Every prompt change + model change runs
  benchmark suite — generated sites scored on rubrics above.
  Regressions block deployment. Without evals, AI changes are
  vibes.

---

## 7. Editor UX — power and calm are decoupled, not opposed

The visible chrome stays minimal while latent capability stays
vast. **Most capability is summoned, not displayed.**

- **Canvas dominates** (70–80%). One inspector that morphs by
  selection. Minimal top bar. Layers / assets / components /
  history / comments / AI: summoned, not docked. Persistent
  panels are the disease that turns admin UIs into cockpits.
- **One inspector, polymorphic.** Right-hand panel; content
  changes by selection. Hero selected → hero properties. Text
  selected → typography. Nothing selected → page settings.
  Breadcrumbs in panel header (Page > Section > Block > Element).
- **Disclose by intent, not feature.** Group UI by cognitive
  mode — writing, structuring, styling, motion, publish — not
  feature taxonomy. Mode detected from action context.
- **Direct manipulation as default.** Edit text in place. Drag
  spacing/padding handles. Color swatches next to the element.
  Floating contextual toolbars attach to selection and vanish
  when deselected. These don't live in chrome; they live next
  to what they affect.
- **⌘K is the universal action surface.** Every action reachable
  by name with fuzzy match. The 80% of features used 5% of the
  time aren't buttons — they're palette entries. Linear / Raycast
  / VS Code / Figma converged here because it's the only pattern
  that scales capability without scaling clutter.
- **AI bar as the natural-language equivalent.** Persistent thin
  input, one key away, accepting freeform intent ("make this
  section feel more editorial," "try three variants of this
  hero"). Resolves to platform-legal operations with diff preview
  before commit.
- **Three zoom levels as peer surfaces.** Site map (pages + link
  graph), page (structure + content), element (properties).
  Pinch / scroll / keyboard navigates between them seamlessly.
- **Constraints as guard rails, not walls.** Drop zones highlight
  during drag; invalid zones don't accept. Brand-legal palette
  surfaces first; full picker is one click further. Typographic
  limit shown live during heading entry. Path of least resistance
  = correct path.
- **Spatial consistency is non-negotiable.** Once a user learns
  where the inspector lives, those locations don't move. Adaptive
  UIs that relocate tools destroy muscle memory (Office's ribbon
  being the canonical disaster).
- **Density modes for novice / expert.** Two modes:
  *guided* (more affordances, hints inline, AI suggestions
  surfaced) and *expert* (chrome minimized, shortcuts dominant,
  palette-first). New users start guided; expert users start
  expert. Same capabilities, different presentation density.
- **Templates and AI scaffolding replace the blank canvas.** New
  site begins with template/style-pack selection or AI brief
  intake producing a draft. First action is *edit something
  good*, not *create from nothing*.
- **Branching version history.** Every meaningful change is a
  snapshot. Branch and try variants without fear; merge or
  revert visually. Treating undo as the only safety mechanism
  is 1990s thinking.
- **Inline comments and review.** Stakeholder feedback happens
  in the editor, not via screenshots in Slack or PDFs over
  email. Pin to elements, assign, resolve, thread.
- **Authoring accessibility itself.** Screen reader support in
  the editor, keyboard-first authoring flows, dictation, dyslexia-
  friendly modes. Most CMS admin interfaces are a11y nightmares.
  Authors with disabilities exist.

---

## 8. Quality gates as continuous gradient, not state

World-class isn't a state — it's a gradient maintained against
entropy. Every dimension below has the same recipe: explicit
measurable criteria, automated measurement on every change,
manual audit on a cadence, adversarial testing by parties with
incentive to find failures, public accountability, protected
capacity for the work.

| Dimension | Continuous check | Gate |
|---|---|---|
| Security | Semgrep / CodeQL on every PR; ZAP / Burp in CI; quarterly external pentest; annual red team | Block on critical findings; track MTTR on others |
| UI quality | Visual regression per primitive × theme × breakpoint × LTR/RTL × accessibility-zoom; design review for new patterns | Block on diff regressions not approved |
| UX | Session replay weekly; user interviews monthly; SUS on critical flows; activation cohort tracked | Drops surface immediately |
| A11y | axe-core in CI; Lighthouse a11y gated; manual screen-reader + keyboard for high-traffic pages; paid disabled-user testing | Block on WCAG 2.2 AA regression |
| Performance | Core Web Vitals as hard budgets (LCP <2.5s, INP <200ms, CLS <0.1); per-route KB caps; RUM in production | Block on budget regression |
| SEO | Lighthouse SEO gated; crawl-self with Screaming Frog; Search Console + Bing Webmaster monitored; structured data validated | Block on indexation drop |
| Audit / compliance | SOC 2 Type II + ISO 27001 surveillance; internal audit reviews; evidence as continuous byproduct (not after-the-fact assembly) | External attestation annually |

**Error budget.** Every service has SLO/SLI. Burn rate gates
feature releases — exhausted budget freezes that service's
feature work until budget recovers. The only mechanism that
consistently forces reliability work to compete with feature
work on equal footing.

**Continuous adversarial testing.** Static analysis (Semgrep,
CodeQL), dynamic (ZAP, Burp, fuzz), pen testing quarterly,
red team annually, bug bounty once baseline is workable,
continuous attack-surface monitoring (Shodan, CT logs, DNS).

**Telemetry coverage.** Every capability emits structured
invocation event. Dashboard shows: reachable but never invoked
(dead capabilities for removal), invoked but only via API
(candidates for UI promotion), UI affordances buried so deep
nobody finds them. The manifest becomes a *living catalogue*.

---

## 9. Retrofit migration — the strangler fig discipline

Forge as designed is greenfield in many places. The architecture
above doesn't refactor onto a live codebase in one pass. The
pattern that works:

1. **Inventory before changing.** Static + dynamic analysis to
   map every endpoint, route, hook, table. Cross-reference with
   production traffic. Output is an honest catalogue with usage
   frequency, ownership, test coverage. Becomes the seed of the
   capability manifest, retroactively.
2. **Dead-code elimination first.** 20-40% of any mature CMS is
   code nobody invokes. Mark zero-traffic for deprecation, gate
   behind flags set to off, ship, wait, delete. Reduce surface
   area before architectural work.
3. **Manifest as parallel artifact, not replacement.** Create
   the capability manifest as a new file. CI check in *warning
   mode* tracks coverage. Flip checks to *blocking* one at a
   time as capabilities migrate.
4. **Smallest vertical slice end-to-end.** Pick one capability,
   migrate through new pipeline completely: declare in manifest,
   regenerate scaffolding, route UI through new inspector, add
   contract test, wire telemetry, generate docs. First slice
   takes disproportionately long; tenth is mechanical.
5. **Strangler facade.** Thin router/proxy in front of old +
   new. Routes by manifest declaration. Capability-by-capability
   migration with instant rollback. Client doesn't know the
   difference.
6. **Adapter layer for design system.** New primitives + tokens
   as parallel library. Legacy components wrapped in adapters
   presenting new primitive interface externally. Gradually
   rewrite adapter internals to drop legacy dep. Same pattern
   for plugins — compatibility shim, v2 sandbox alongside v1.
7. **Quarantine the unknowns.** Undocumented, owner-unknown,
   test-coverage-zero, possibly load-bearing modules — wrap in
   characterization test harness capturing current behavior as
   regression baseline. Don't touch first. Trying to clean up
   the scariest module first is how these migrations fail.
8. **Gates in warning mode first, blocking later.** Visual
   regression, a11y, perf budgets, coverage, contract tests —
   all running warning-mode against entire existing codebase.
   Initial reports horrifying; that's fine. *Establish
   measurement infrastructure before enforcement.* Flip
   individual checks to blocking on **new code only**
   (delta-based enforcement) before applying to legacy.
9. **Documentation as forcing function.** Generated docs only
   cover migrated capabilities. Gap between "code that exists"
   and "code in generated docs" is a visible, public, shrinking
   number.
10. **Protected migration capacity.** 20-30% of every team's
    capacity, tracked separately, defended at planning level.
    "We'll migrate as we go" = never. Without protected
    capacity, migration becomes background noise that loses to
    every shipping deadline.
11. **Accept partial migration as victory.** 85% migrated with
    remaining 15% cleanly quarantined is a victory; treating it
    as failure is project-management mistake. Some code is too
    entangled, low-traffic, or owned-by-no-one to justify
    migration.

The transition state is permanent-ish — multi-year migrations
spend most of their existence half-old / half-new. The
**architecture of the transition** matters as much as the
destination. Adapters, facades, parallel manifests, dual gates
need to be deliberately designed, not improvised.

---

## 10. Substrate-flexible, product-opinionated

The substrate above serves multiple audiences — solopreneur,
agency, in-house marketing team, developer, enterprise. Each
wants different things. Solopreneurs want zero-setup + beautiful
templates. Agencies want client management, white-label, billing,
multi-site dashboards. In-house teams want workflow, governance,
audit trails. Developers want APIs, custom code, version control.
Enterprise wants SSO, SCIM, compliance, SLAs.

**The substrate can serve any of these. The *product* layer above
the substrate has to commit.** Pricing, templates, onboarding,
marketing, feature priorities — those follow from who the
customer is. Trying to be everything to everyone is how WordPress
accumulated 20 years of inconsistency.

**Make the substrate flexible; make the product opinionated.** That
divide determines whether the platform reaches escape velocity in
any market or stalls trying to serve all of them adequately and
none of them excellently.

Forge's chosen audience: **AI agents building UI**, plus the
operators who run sites the agents produce. Other audiences are
secondary. Substrate doesn't preclude them; product doesn't
prioritize them.

---

## 11. The unifying principle

Across every layer the same architectural move repeats:

> Identify the implicit assumption that produces the failure
> mode. Make it explicit and structural. Derive everything else
> from the boundary.

| Layer | Implicit assumption | Made explicit as |
|---|---|---|
| Plugins | "Anyone can do anything in PHP" | Capability manifest + Wasm sandbox |
| Themes | "Themes can override anything" | Token binding + primitive composition |
| Content | "Content is HTML" | Typed CmsSection schemas |
| Layout | "CSS is a suggestion" | Constraint system the platform owns |
| AI | "LLM emits anything" | Constrained decoding over typed grammar |
| Frontend ↔ backend | "Discipline keeps them in sync" | Single manifest with generated projections |
| Quality | "Excellence is a state" | Continuous gradient maintained against entropy |
| Organization | "Excellence is enforced by code review" | Excellence is enforced by the compiler + CI |

Same insight that makes LFI's symbolic substrate work: bound the
valid combinations, let neural search navigate the bounded space,
never let the neural component define what's valid.

This principle is the durable architectural commitment. Specific
primitives, capabilities, and gates will change with time; this
principle does not.
