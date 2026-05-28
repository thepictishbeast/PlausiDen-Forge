# Substrate reframe — 2026-05-21

This document captures the architectural reframe paul issued on
2026-05-21 after the prosperityclub.com pixel-reproduction loop
failed to converge despite multiple iterations. The reframe
supersedes prior "Claude must execute Forge correctly" framing
and prior "tune the audit gates" framing. The two together are
the canonical statement of where Forge actually stands and what
needs to be true to move it forward.

## Diagnosis: substrate vocabulary, not Claude execution

Forge was built around a small, consumer-shaped vocabulary:
SkillShots-style hero, AMOLED dark themes, the few gradients in
the token system, the few header / footer patterns those sites
need. The token system, the CmsSection / CmsBlock enumeration,
and the primitive count (≈20–30 in active use; estimated 80–120
needed at v1.0) are all calibrated for that band.

When pointed at a site outside that band — prosperityclub.com,
a publication-style insurance / financial education site — the
substrate composes from the only vocabulary it has and produces
something that looks like SkillShots with the labels changed.
The reused gradients appear because that's what the tokens
contain. The reused headers appear because those are the
primitives that exist. The aesthetic drift toward a single
visual register is not a Claude execution failure; it is the
substrate succeeding at what it can currently do.

**Forge in its current state cannot reproduce a site like
prosperityclub.com because Forge's vocabulary is too narrow.**
This is not solvable by better prompting, better skills, better
MCP tools, or better audit gates on top of the current
substrate. The substrate needs primitives, themes, decorative
elements, and compositional patterns it does not yet have.

## Three honest options for mom's site (Prosperity Club)

1. **Build the site outside Forge.** Hand-coded HTML/CSS,
   Webflow, a static-site generator with picked templates,
   whatever produces good results today. Site ships on schedule;
   Forge does not block her timeline; Forge development
   continues in parallel as a longer-term project. **Recommended
   for the immediate need** when the site has any timeline
   pressure.

2. **Hand-author the site inside Forge's repo, marked as legacy.**
   The site lives in Forge's source tree as raw HTML / CSS,
   explicitly marked as "not substrate-built." Forge serves it,
   attests its build report, handles its deployment. Pragmatic
   compromise — uses Forge for build pipeline / attestation /
   deployment without forcing Forge to generate visually
   distinctive sites from a sparse vocabulary. The site is
   marked legacy so it doesn't pollute the registry, doesn't
   inform pattern emergence, doesn't get treated as a reference
   for variation enforcement. Refactors into substrate-native
   form when the substrate can support it.

3. **Wait for substrate vocabulary expansion.** Don't ship the
   site on Forge until the substrate can produce it. Months of
   work in front; viable only if no timeline pressure.

**Paul's stated preference (2026-05-21):** option 1 for the
immediate need; option 3 in parallel as the long-term substrate
work. Don't entangle her business's site needs with the
platform's readiness.

## The five concrete substrate gaps

(Why "more audit gates" doesn't help when these gaps exist.)

- **Primitive vocabulary is too narrow.** ≈20–30 active vs. ~80–
  120 needed at v1.0; needs to be five to ten times its current
  size before the composition space is large enough to express
  the target sites.
- **Themes are too few and too narrow.** Light + dark + dark-
  AMOLED ≈ 3 themes; real sites span magazines, dashboards,
  portfolios, e-commerce, government, editorial, brutalist,
  minimal, maximalist, etc. Needs dozens of themes, each
  genuinely distinct, each with rich token sets.
- **Decorative primitives barely exist.** Background patterns,
  accent shapes, illustrative elements, decorative dividers,
  ambient motion, atmospheric depth — sites without these look
  like wireframes regardless of what else is right.
- **Compositional patterns are too rigid.** Vertical-stack-only.
  No overlapping sections, asymmetric layouts, broken grids,
  diagonal cuts, layered elements.
- **Content model is shaped for the existing sites.** CmsSection
  BlockKinds match what SkillShots and plausiden.com needed; the
  schema needs to permit sites that don't look like SkillShots.
- **Defaults are too uniform.** Default state should be neutral;
  the chosen theme + tokens define what the site looks like.

## Layered defense — seven layers for Claude-Forge reliability

Independent of the substrate-vocabulary work, Claude's use of
Forge has its own failure modes. Paul's framing: a reliable
Claude-Forge interaction is a layered defense where multiple
mechanisms catch different failure modes. No single layer is
sufficient; the combination is robust.

### Layer 1 — Constrain inputs (type-enforced MCP)

- Forced selection from enumerated options at MCP boundary
- Forced selection from filtered options (context-aware
  filtering)
- Forced parameter validation through types
- Forced sequencing through state machines (skipping steps is
  structurally impossible — successor operations are inaccessible
  until predecessor states reached)

**Catches:** Claude inventing unsupported variants, wrong
parameters, skipped sequencing.

### Layer 2 — Structure execution (paired skill + MCP workflows)

Each skill is a markdown document carrying doctrine, examples,
pitfalls, mental models. Each skill is **paired** with an
MCP-callable workflow tool that executes its procedure with
correct sequencing, flag selection, intermediate verification,
and error handling baked in. Claude reads the skill (understands),
invokes the workflow (executes deterministically), interprets
the structured result.

Pairing mechanics:
- Skill frontmatter declares paired MCP tool
- MCP tool docs reference paired skill
- skill-invocation workflow loads both
- CI verifies every skill has a paired tool and vice versa

First ten workflows worth building (highest leverage):

1. `forge_build_site_from_brief` + skill
2. `forge_modify_site` + skill
3. `forge_add_primitive` + skill (refuses to complete until every
   required artifact — Rust code, tests, docs, audit phases — is
   present and verified)
4. `forge_add_audit_phase` + skill (proptest target + integration
   test + docs required)
5. `forge_modify_primitive` + skill (visual regression across all
   themes; trait conformance; downstream impact)
6. `verify_content_originality` + skill (anti-reuse — the
   anti-pattern dictionary + fingerprint check baked in)
7. `forge_site_fingerprint_check` + skill (uniqueness verification
   against the registry)
8. `forge_reference_extraction` + skill (deterministic mapping
   from reference URL to substrate vocabulary; surfaces unmappable
   patterns as substrate gaps)
9. `forge_substrate_gap_registration` + skill (captures gaps as
   structured registry entries rather than chat mentions that
   evaporate)
10. `forge_doctrine_violation_explanation` + skill (turns generic
    audit findings into actionable structured remediation with
    rationale)

Internal pattern every workflow follows:

1. Validate typed inputs; reject with structured error
2. Load relevant context (substrate version, doctrine,
   artifacts)
3. Execute declared phases in sequence with progress reporting
4. Verify post-conditions after each phase; failure aborts with
   structured error
5. Run final verification suite (objective achieved, not just
   phases ran)
6. Return structured outcome (success / partial / failure) + per-
   phase results + findings + suggested next actions + skill
   references

**Catches:** skipped audit phases, skipped intermediate
verification, wrong flags / commands, misinterpreted errors,
forgotten platform-wide rules, reused site-specific content,
working on the wrong layer (site-repo vs substrate-repo).

### Layer 3 — Enforce diversity (fingerprint registry + anti-pattern)

- Fingerprint every produced site (structural signature + content
  silhouette + token usage + primitive sequence + decorative
  selection + gradient / decoration specifics)
- Mandatory uniqueness check at ship boundary; matches above
  threshold fail the workflow
- Anti-pattern dictionary curated from specific observed failures
  (this exact gradient, this exact header structure, etc.) —
  outputs matching anti-patterns refuse to ship
- Structured deviation suggestions when matches found

**Catches:** reused gradients, reused headers / footers, reused
composition signatures, exact-replica patterns from prior sites.
The substrate refuses to produce them; Claude has to differ;
either Claude finds variation in the substrate (if it exists)
or the gap surfaces explicitly (substrate roadmap signal).

### Layer 4 — Surface alternatives (multi-pass generation)

- Forced diversity in candidate generation (3 candidates per
  decision, substantively different)
- Cross-temperature generation (cheaper than independent runs;
  produces meaningful variation)
- Constraint-shuffled generation (emphasize sovereignty first vs.
  density first vs. variation first vs. brand consistency first)
- Adversarial pair generation (one biased toward consistency,
  one toward variation; select from contrast)
- Substrate-initiated choice prompting (substrate prompts
  operator at generation start + at deviation points instead of
  proceeding silently)

**Catches:** path-of-least-resistance generation, default-pattern
generation, operator-passive generation.

### Layer 5 — Enable correction (inline operator override)

- Inline correction primitives (highlight + specify + apply, no
  full re-generation)
- Corrections pinned to site identity (this site never uses that
  gradient again)
- Cross-site correction propagation (operator decides scope —
  this-site / tenant / substrate)
- Correction analytics (operator corrections are feedback data —
  most-corrected patterns are substrate / Claude problems to fix)

**Catches:** whatever upstream layers missed; provides surgical
correction without losing what's right.

### Layer 6 — Learn from outcomes (ratings + surveillance)

- Output ratings (operator scores quality, distinctness, fit-to-
  brief, aesthetic appeal, completeness with structured numeric
  scores + optional comments)
- Rating-aware generation (past ratings visible as context; low-
  rated patterns explicitly avoided)
- Cohort-level rating analysis (aggregate patterns → anti-pattern
  library + exemplar library)
- Per-operator preference profiles (rating history reveals
  preferences; future generation considers them)
- Self-similarity surveillance (substrate scans its own output;
  flags drift before operators complain)
- Vocabulary utilization surveillance (underused primitives /
  variants / themes surface — substrate trim or Claude orient)
- Failure pattern detection (aggregate audit findings → substrate
  gap or Claude misuse)
- Operator complaint correlation (complaints categorized +
  correlated with output patterns)

**Catches:** systemic drift, accumulated substrate / Claude
problems unnoticed until severe, lack of ground truth about
what's good.

### Layer 7 — Vocabulary expansion (substrate work)

When upstream layers surface "the substrate can't express what
we need," that's substrate roadmap. Expansion happens based on
what enforcement surfaced. This is the months-to-years body of
work that closes the actual capability gap. See section above
on the five concrete substrate gaps.

## Demonstration-based learning (cross-cutting)

Independent of the seven layers, structured exemplar libraries
help Claude pattern-match correctness from concrete examples
rather than abstract documentation:

- **Exemplar library** — vetted "correct" interactions: brief,
  generated content, build outputs, audit results, "why this is
  good"
- **Anti-exemplar library** — captured failures: brief, what
  Claude generated, why it was wrong, what the correct version
  would look like
- **Reference comparison** — sites Forge aspires to (outside the
  consumer band); loaded as context during generation
- **Contrast pairs** — good vs. bad example pairs for specific
  failure modes; makes the difference concrete and learnable

## Resource-constrained generation (cross-cutting)

- Token budgets per task (efficient primitive use forced)
- Operation count limits (planning over exploration; hitting
  limit signals bad plan or substrate gap)
- Time-boxed generation (shippable-quality within bounds)
- Component count caps (economy of expression)

## Pre-generation planning (cross-cutting)

- Mandatory structured plan before any generation (what's being
  built, what primitives, what content shape, what identity,
  what variation strategy)
- Plan reviewed against rules + by operator before generation
  proceeds
- Plan-vs-execution audit (deviations flagged)
- Plan reuse detection (plans themselves fingerprinted)
- Plan branching for variation (substrate suggests deviation
  points when plan too similar to past plans)

## The brick library (cross-cutting)

Larger pre-built compositions between primitives and templates.
Each brick is hand-designed and audited. Many bricks per page-
section type. Site generation primarily selects bricks rather
than composing primitives. Fewer choices, each more impactful,
each pre-vetted. Brick attribution records which sites have used
each brick; over-relied-on bricks get attention.

## The path forward

For mom's site (Prosperity Club): option 1. Don't make her site
hostage to the platform's readiness.

For Forge: stop testing it on requirements it can't yet satisfy.
Pick five real reference sites outside the current consumer
band. Catalog gaps per site. Aggregate gap list = substrate
roadmap. Build incrementally; each new primitive 2–4 weeks of
careful work with all disciplines. Periodically attempt the
reference sites again; fidelity should improve measurably with
each expansion. Three of five reproducible at reasonable
fidelity = starting to be commercially viable. All five = ready
for the broader commercial trajectory.

For Claude reliability: build the highest-leverage layers first.
For the specific reuse failures observed, the four highest-
leverage layers are:

1. Fingerprint registry with mandatory uniqueness check
2. Constrained input enforcement at MCP boundaries
3. Skill-workflow pairing (with mandatory uniqueness check baked
   into the build workflow)
4. Anti-pattern dictionary curated from specific failures

Build these together; verify they prevent the failures; iterate
based on what new failures surface. The other layers get built
as the workflow surface and substrate vocabulary mature.

## What this supersedes

This document supersedes:

- The "PC pixel-reproduction loop" approach (tasks #339–#345,
  deleted 2026-05-21). Pixel-reproducing a publication site on
  current Forge vocabulary is not achievable; chasing it
  produces frustration without convergence.
- The "tune audit gates" framing. Audit gates operate over the
  substrate's existing vocabulary; gates can't manufacture
  primitives that don't exist.
- The implicit assumption that Claude execution improvements
  alone close the visual fidelity gap. They don't; substrate
  vocabulary expansion is the actual bottleneck.

It does not supersede:

- The Forge architectural principles (manifest-first, audit-
  enforced, deterministic baseline) — those remain correct.
- The substrate-only-path doctrine for sites that Forge IS able
  to express — for those sites, no hand-coding inside
  substrate repos.
- LFI-as-core-LLM-as-peripheral — still the brain / candidate-
  generator split.
- Per-tenant corpora additive doctrine — substrate stays
  generic; tenants extend, never modify.

## Forge-specific fixes (independent of Claude)

Forge has its own problems independent of who is driving it. Even
a perfect Claude would produce convergent outputs through current
Forge because Forge structurally permits and defaults to reuse.
The fixes below are substrate work Forge needs regardless of how
its driver behaves.

### Forge problems that produce convergent outputs

- **No concept of "previous sites" as a constraint.** Each build
  is independent; site A's build doesn't see site B's. Nothing
  prevents structural identity across sites because the
  substrate never compares them.
- **Defaults are aggressive.** Single canonical defaults dominate
  because they're path-of-least-resistance. The default gradient
  appears on every site that doesn't explicitly override.
- **No variation requirement.** Audit gates check correctness
  (a11y, perf, structure) but not variation. A site passes every
  audit and still mirrors another site.
- **Primitive set rewards reuse.** Few variants per primitive →
  many sites land on the same variant by combinatorial necessity.
- **Theme system has too few themes.** Light + dark + AMOLED are
  aesthetic siblings, not aesthetic alternatives.
- **Content model presumes the originating sites.** Other site
  types force-fit into existing BlockKinds.
- **Doesn't measure its own output diversity.** No telemetry
  tracking cross-site distance.
- **Audit phases run per-build, not cross-build.** Reuse failures
  live in the cross-build dimension; that dimension is
  structurally invisible.

### The Forge-specific fixes

**The cross-build registry as core substrate.** Every site Forge
builds gets its full structural signature stored in a persistent
queryable registry — primitive sequence, variant choices, token
usage, content silhouette, theme selection, decorative element
usage. The registry is part of Forge itself. Every new build
queries the registry as part of audit. Checks: exact-match
detection (refuses identical builds), near-match detection
(refuses near-duplicates above threshold), gradient-reuse
detection, header / footer reuse detection, composition-pattern
reuse detection. Checks are non-optional; skipping requires
explicit configuration override, logged.

**Default fragmentation.** Instead of one default gradient, Forge
ships a pool of 20–30 gradients spanning aesthetic range.
Selection is deterministic on site identity (brand / tenant /
content type) but considers what other sites have used. Same
pattern for default header, default footer, default button style,
default spacing rhythm. Pool is hand-curated. Defaults become a
force for variation, not convergence.

**The variation audit phase.** New Forge phase explicitly checks
variation across the registry. Computes new site's structural
signature; queries registry for similar signatures; emits strict
findings when too close; findings include specific suggested
deviations to pass. Non-optional. Runs on every build. Strict
findings block ship.

**Primitive vocabulary expansion (substrate priority).** Not
because Claude needs more options but because the substrate
needs more options to refuse reuse from. With 10 Hero variants,
2 sites have ~10% chance of variant collision; with 100, ~1%.
Substrate vocabulary breadth directly determines how easy or
hard it is for sites to differ.

**Theme system expansion.** Dozens of themes, each genuinely
distinct. Editorial, brutalist, magazine, technical, playful,
minimal, dense, atmospheric, photographic-led, illustration-led,
etc. Coherent aesthetic per theme with its own primitive
preferences, token sets, decorative treatments. Themes are
versioned, audited, tested across all primitives. Adding a
theme is a substantial substrate event reviewed for quality.

**Content model breadth.** Forge's CMS schema expands beyond
originating shapes. New BlockKinds for:

- Editorial: PullQuote, Sidebar, Footnote, DropCap,
  ResponsiveAside, MarginNote
- Portfolio: Gallery, Slideshow, ImageWithCaption,
  ProjectShowcase
- Documentation: CodeBlock-with-annotations, ParameterTable,
  ApiReference
- Commerce: ProductGallery, VariantSelection, ReviewBlock
- Civic: BallotChoice, OfficialStatement, CivicAction

Schema covers many site types, not just the originating consumer
band.

**Diversity surveillance system.** Forge continuously analyzes
its own outputs. Metrics: aggregate primitive / variant / theme /
token-override usage distribution across all sites; mean
cross-site signature distance; distribution of cross-site
signature distances. Outliers + trends surface as substrate-level
findings. Convergence on a subset of primitives → either Claude
isn't reaching for breadth or substrate's selection logic is
biased; investigation drives correction. Drift detection catches
problems before catastrophic.

**Cross-build audit infrastructure.** General capability to
verify properties across builds: tenant-level diversity
("no two sites in this tenant's portfolio share more than X%"),
within-site distribution caps, platform-level vocabulary
utilization floors, gradient uniqueness across recently-built
sites. Phases live in Forge, have access to build history,
enforce cross-build properties.

### Sequencing for Forge-specific fixes (by impact)

1. **Cross-build registry + uniqueness check.** Highest leverage —
   the structural addition that makes reuse impossible.
2. **Variation audit phase.** Lands alongside the registry; uses
   it. Threshold calibration empirical.
3. **Default fragmentation.** Curate the pools; implement
   deterministic identity-aware selection.
4. **Diversity surveillance system.** Incremental — each metric a
   small substrate addition.
5. **Primitive + theme + content-model expansion.** Continuous;
   each new artifact is design-led work, 2–4 weeks per primitive
   or BlockKind, longer per theme.

### Verification that Forge is fixed

Concrete tests:

- Build 5 sites with completely different briefs through Forge.
  Without operator intervention or Claude special prompting, are
  the 5 outputs visibly distinct? If they look like 5 versions of
  the same site, Forge isn't fixed.
- Build the same site (same brief, same operator) twice on
  different days, no shared cache / session state. Are outputs
  distinct? Identical = variation enforcement isn't engaging.
- Variation surveillance metrics: mean cross-site signature
  distance above threshold; distribution lacks a long tail toward
  zero; aggregate primitive usage spans most of the available
  primitives.
- Try to produce a site that uses the same gradient as a recent
  site. Forge should refuse with a gradient-uniqueness finding.
  Try identical hero. Refusal. Try identical composition. Refusal.

### Persistence diagnostic checklist

If symptoms persist after deploying the fixes:

1. Are the fixes actually deployed and active on the failing path?
   (Registry has entries? Variation phase in default phase list?
   Defaults pool populated and selected from? Claude's interaction
   goes through audit pipeline?)
2. Are thresholds calibrated strictly enough? (Lenient thresholds
   pass obvious reuse.)
3. Is the substrate's vocabulary broad enough for variation
   enforcement to operate over? (10 Hero variants caps variation
   enforcement at 10 distinct uses.)
4. Is Claude actually engaging with the substrate's variation
   mechanisms? (Skill/MCP coverage needed to ensure interactions
   go through gated workflows.)
5. Is operator iteration creating effective convergence? (Operator
   rejects varied outputs until familiar appears → operator-
   discipline problem masquerading as substrate problem.)

### Architectural last resort

If symptoms persist after all the simpler fixes (would be
surprising), the substrate's fundamental shape may encode
convergence:

- **Template-based generation pattern.** All outputs share
  template-level structure. Move to compositional-build (sites
  assembled from primitives without intermediate templates).
  Major architectural change.
- **Deterministic-default pattern.** Same inputs → same outputs;
  similar briefs → similar outputs. Add controlled stochasticity
  within bounds. Major architectural change.
- **Single-pipeline pattern.** One canonical build pipeline →
  sites converge. Multiple pipelines for different site types
  (editorial vs. SaaS-marketing vs. portfolio) → intrinsic
  structural variation.

Consider only after the simpler fixes have been deployed and
verified.

## Accessibility axis: substrate too complex for Claude's working context

Distinct from the capability axis (vocabulary breadth). Forge has
accumulated significant cognitive surface area: typed pipeline,
27+ audit phases, trait system, manifest layer, multiple crates
with typed interfaces, doctrine rules, cross-build registry
concepts, orientation declarations, version discipline, deploy
targets, cryptographic attestation, cross-cutting concerns
(a11y, sovereignty, performance). Holding all of this in mind
while making decisions about a specific task is genuinely hard.

When the substrate's intrinsic complexity exceeds what Claude can
hold while still reasoning effectively, the symptoms are exactly
the reuse pattern: Claude reaches for what it remembers reliably
(defaults, recent patterns, things it just did) rather than what
it should reach for in principle. The reuse isn't laziness — it
is cognitive load management. Claude defaults to what's familiar
because the substrate is too rich to navigate exhaustively per
choice.

### Diagnostic test: Forge Lite

Build a deliberately narrow surface of Forge — a "Forge Lite"
mode that exposes only a fraction of the substrate's capabilities
through a much simpler interface. Constrain to ~10 primitives,
3 themes, small set of typed operations. Let Claude work within
Forge Lite. Compare outputs to current Forge.

- Forge Lite produces visibly better Claude outputs → diagnosis
  confirmed; restructure Forge's interface so Claude effectively
  works in something like Forge Lite by default, with broader
  capabilities accessed only when explicitly needed.
- Forge Lite produces same symptoms → diagnosis needs revision;
  problem is elsewhere.

### Architectural patterns that reduce cognitive load

**Progressive disclosure interface.** Claude doesn't see all of
Forge's surface area at once. MCP tools available in a session
are scoped to the current task. Building a site = site-building
tools available; modifying a primitive = primitive-modification
tools. Tools irrelevant to current work aren't presented. The
substrate's surface area is large; the working surface is small.
Inverse of "give Claude everything"; the substrate decides what's
relevant based on declared scope.

**Opinionated workflow guidance.** Substrate presents the
canonical way to do each common thing rather than presenting
many ways and asking Claude to pick. One right way to build a
marketing homepage; the workflow does that thing. Variations
require explicit deviation requests with reasoning, not Claude
considering all paths at every step. Compresses Claude's
decision space.

**Structured representation over text.** Documentation as typed
queries, not loaded prose. Claude needs to know what primitives
exist → query, get typed list. Claude needs to know what rules
apply → query, get structured rules. Information is available
on-demand in structured form rather than loaded upfront as
documentation surface. Dramatically reduces context consumption.

**Composition over enumeration.** Instead of 100 Hero variants
as enumerated options, Hero with 5 orthogonal properties × 5
values each: `Hero(layout: centered/split/asymmetric/full-bleed/
stacked, emphasis: text-led/visual-led/balanced, density: tight/
normal/loose, decoration: none/subtle/prominent/atmospheric,
motion: still/subtle/expressive)`. Combinatorial space is huge;
cognitive surface is small. Claude picks 5 values, not 1 of 500.
Trait system applied to reduce enumeration burden.

**Skill-driven workflow execution.** The paired skill + MCP
combination applied specifically to complexity management.
Skills carry procedural knowledge; workflows execute procedures;
Claude orchestrates without holding the full procedural
specification in context.

**Explicit hand-off pattern.** Tasks too complex for one session
support explicit hand-off — Claude completes a phase, saves
structured state, the next session picks up from that state.
Each session works on a bounded subset; cumulative work spans
the complexity across sessions without any single session
overwhelmed.

### Specific things to build (accessibility-axis)

- **Scoped session pattern.** Each Claude session starts with a
  declared scope. MCP tool surface, doc context, visible substrate
  state filtered to scope. Claude doesn't see irrelevant state.
- **Progressive query interface.** Docs are queryable, not loaded.
  Requires manifest layer + typed catalogs + structured doctrine
  to be machine-queryable.
- **Decision compression through composition.** Property-based
  composition replacing enumerated picks for primitives + themes
  + tokens.
- **Default-heavy interaction with explicit deviation.** Most
  decisions have canonical defaults applied automatically;
  deviation requires explicit request with reasoning. Claude's
  reasoning focuses on what to deviate, not what to choose at
  every default-able point.
- **Opinionated workflow surface.** One canonical workflow per
  common task. Variations summoned, not browsed.
- **Structured progress representation.** Session progress tracked
  as structured state (what phase, what artifacts, what gates
  passing, what next). Claude queries the progress structure
  rather than re-deriving from session transcript.
- **Recovery + resume capability.** Sessions that hit complexity
  limits save state and resume. State captures done / remaining /
  choices. Resumption loads structured state without re-loading
  full task context.

### Measurable effects

- Token consumption per task should decrease as the interface
  tightens.
- Time-to-correct-output should decrease as defaults handle more
  decisions.
- Quality variance should decrease as cognitive load drops within
  Claude's reliable working range.

### Two orthogonal axes

- **Capability axis** (vocabulary breadth): what Forge CAN
  produce. Substrate vocabulary expansion (tasks #355-#360).
- **Accessibility axis** (interface tightness): what Claude can
  EFFECTIVELY USE. The patterns above.

A platform can be highly capable but inaccessible (rich substrate
nobody can extract richness from). A platform can be highly
accessible but limited (everything easy to use; nothing
sophisticated possible). Goal is both: capable substrate,
accessible interface. Both must happen in parallel; neither alone
is sufficient.

Faster-payoff guess: the accessibility work probably produces
visible improvement in Claude's outputs over the existing
substrate capabilities faster than vocabulary expansion does,
because each new primitive helps only its specific cases whereas
interface tightening helps every interaction.

### AI-capability last resort

Honest acknowledgment: if Forge is reorganized for accessibility,
vocabulary expanded, variation enforcement built, cross-build
registry deployed, skill-and-MCP combination in place, AND
Claude still produces convergent unaesthetic outputs — the
diagnosis becomes about current-generation AI's fundamental
capability for aesthetic compositional work where judgment about
beauty / distinctness / appropriateness matters.

Mitigations if this turns out to be the binding constraint:

- More aggressive human-in-the-loop workflows (AI proposes,
  human selects)
- More constrained problem domains where AI's strengths apply
  (technical sites, documentation sites, structured-content
  sites) while design-led sites remain human-led
- Accepting that some sites will be hand-built indefinitely
  while AI-assisted production focuses on contexts where AI
  succeeds reliably

The platform's success doesn't depend on any single fix working;
it depends on the layered defense being sufficient across the
actual AI capability landscape. The platform-works-without-AI
commitment from prior doctrine protects against this risk: the
deterministic substrate produces sites regardless of AI
capability; accessibility improvements help any user; the
platform stays viable even in the worst case where AI proves
limited for this specific kind of work.
