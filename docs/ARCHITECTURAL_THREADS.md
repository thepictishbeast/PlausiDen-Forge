# Architectural Threads — task-coverage audit

**Status:** doctrine. Closes task #296. Maps each architectural
arc described in the Forge / Loom / Crawler / LFI design corpus
to its task IDs, shipped artifacts, and remaining work. Updated
each iteration when the substrate evolves.

This doc is the meta-check the advisor flagged for the loop:
many backlog tasks are *enumerative* from design-doc dumps, not
load-bearing deliverables. This document classifies them so the
loop spends time on real work, not on speculative item-by-item
closure.

---

## Thread classification

Each task is one of:

* **shipped** — code merged, tests pass, wired into the build pipeline.
* **in-progress** — partial code or doc, not closed.
* **pending-load-bearing** — required for the arc to deliver its
  promise; should ship.
* **pending-enumerative** — listed in a design-doc dump but
  duplicates or extends a shipped task with diminishing return.
  Candidate for `status=deleted` or consolidation.
* **pending-speculative** — depends on infrastructure not yet
  present (LFI, MCP server, future Crawler axes); park until
  the dependency lands.

---

## Arc 1 — Variation enforcement (#231-#262 conceptual range)

Goal: cross-site uniqueness + within-site consistency + controlled
mutability per `docs/VARIATION_GUARANTEES.md`.

### Shipped
| Task | Subject | Artifact |
|------|---------|----------|
| #231 | SiteFingerprint canonical hash | `forge_core::fingerprint` |
| #232 | Fingerprint registry (Merkle, signed) | `forge_core::fingerprint_registry` |
| #233 | Cross-site uniqueness gate | `forge_phases::uniqueness_gate` |
| #234 | Site identity schema | `forge_core::site_identity` |
| #235 | Site-identity conformance audit | `forge_phases::site_identity_conformance` |
| #236 | Pattern entropy audit | `forge_phases::pattern_entropy` |
| #237 | Differentiation budget multi-axis | `forge_phases::differentiation_budget` |
| #241 | Voice profile statistical audit | `forge_phases::voice_profile_audit` |
| #243 | Aesthetic mood lock | `forge_phases::mood_lock` |
| #244 | Composition lineage / vocabulary coherence | `forge_phases::composition_lineage` |
| #251 | Forbidden composition patterns dictionary | `forge_phases::forbidden_patterns` |
| #258 | Registry-tampering CLI defense | `forge fingerprint verify/list` |
| #259 | Fingerprint-spec versioning + migration | `forge_core::fingerprint_migration` |
| #261 | Theme variation declaration requirement | `forge_phases::theme_variation_required` |
| #262 | VARIATION_GUARANTEES.md doctrine | `docs/VARIATION_GUARANTEES.md` |
| #295 | Substrate-aware error structure spec | `forge_core::diagnostic` + `docs/SUBSTRATE_ERRORS.md` |

11 enforcement gates wired into the build pipeline. The variation
arc is substantially complete — the cross-site uniqueness +
within-site consistency guarantees are machine-checkable today.

### Pending — load-bearing
| Task | Subject | Rationale |
|------|---------|-----------|
| #238 | Identity transition workflow (atomic) | Substrate's controlled-mutability axis (third guarantee in VARIATION_GUARANTEES.md). Requires stateful per-build identity diffing. |
| #239 | Identity rollback + version history | Pairs with #238. |
| #240 | Drift prevention: partial-identity-change refusal | Pairs with #238. |
| #246 | Provenance commitment (every decision signed) | Extends attest module to per-finding signatures. |

### Pending — enumerative (candidates for consolidation)
The variation arc dump enumerated 27 sub-tasks. Several are
sub-divisions of capabilities already shipped in a different
phase. Recommended actions:

| Task | Subject | Action |
|------|---------|--------|
| #242 | Hierarchical token cascade with bounded page-overrides | Subsumed by `[site_identity.tokens]` schema in #234 + existing tokens phase. Consider closing. |
| #245 | Composition genealogy tracking | Subsumed by fingerprint registry (#232) which already records every build's fingerprint with timestamps. Consider closing or scoping to a CLI subcommand. |
| #247 | Exhaustion tracking + auto-rebalance | Speculative; depends on long-term build telemetry. Defer until #246 ships. |
| #248 | Substrate continuous self-audit | Useful but meta. Consider scoping to a small CI lint rather than a full phase. |
| #249 | Forced-variation reseeding cadence | Depends on N-build history; speculative until #246 ships. |
| #250 | Page-type library (50-100 templates) | Out of scope for variation arc — this is a content/template task, not enforcement. |
| #252 | Reference-corpus statistical baseline | Pairs with reference-matching arc (#263-#274) — re-classify there. |
| #253 | Forced primitive variant distribution requirement | Subsumed by composition_lineage (#244) inverse + pattern_entropy (#236). Consider closing. |
| #254 | Composition zone constraints | Speculative; depends on a per-region taxonomy that doesn't exist yet. |
| #255 | Section-type quota enforcement | Subsumed by content_type taxonomy in #234. Consider closing. |
| #256 | Per-page deviation budget | Subsumed by differentiation_budget (#237) with scope flag. |
| #257 | LFI augmentation for HDC semantic similarity | Deferred to LFI Claude. |
| #260 | Quality-floor enforcement independence | Already true architecturally — quality phases (theme_contrast, a11y, etc.) run independent of variation phases. Doctrine note suffices. |

### Recommended task-list cleanup for arc 1

Mark `deleted` (subsumed by shipped work): #242, #245, #253, #255, #256, #260.

Defer (depend on #246 first): #247, #249.

Re-classify to other arcs: #252 (reference), #250 (templates), #257 (LFI).

---

## Arc 2 — Reference matching (#263-#274, #290-#292)

Goal: ingest a real site, extract its compositional decisions,
synthesize a Forge CMS that targets the same shape.

### Pending — load-bearing
Nothing shipped on this arc yet. All tasks block on a working
Crawler-side capture + extraction pipeline (#263), which itself
depends on the Crawler being able to render headless at multiple
viewports + emit a structured journey.

| Task | Subject | Blocker |
|------|---------|---------|
| #263 | Reference-site crawl + capture pipeline | none — Crawler can do this today |
| #264-#272 | Per-axis extraction (palette / typography / spacing / motion / sections / patterns / structural / voice / interactive) | All block on #263 |
| #273 | Reference → substrate mapping engine | Blocks on extraction outputs |
| #274 | Multi-reference composition engine | Blocks on #273 |
| #290 | REFERENCE_MATCHING.md doctrine | Document while building #263+ |
| #291 | Site synthesis pipeline | Blocks on #273 |
| #292 | Operator UX preview/confirm | Blocks on #291 |

Recommendation: pick #263 + #290 as the next concrete pair when
the variation arc clears. The reference arc is a multi-iteration
build, not a single-iteration task close.

---

## Arc 3 — Long-term compounding + Claude integration (#275-#288)

Goal: capture substrate gaps, evolve substrate from data,
provide MCP typed-tool surface to Claude.

### Pending — load-bearing
| Task | Subject | Notes |
|------|---------|-------|
| #284 | MCP typed-tool surface | paul flagged as "highest leverage" |
| #285 | Structured context loading + caching for Claude sessions | Pairs with #284 |
| #286 | Substrate state machine + Claude state-query interface | Pairs with #284 |
| #287 | Skill-driven workflow library | Pairs with #284 |
| #288 | Structured progress reporting via MCP tools | Pairs with #284 |

The MCP arc is a five-task cluster; ship as a single unit when
addressed.

### Pending — enumerative
| Task | Subject | Action |
|------|---------|--------|
| #275 | Substrate-gap registry | Subsumed by GitHub issues + task list itself. Close. |
| #276 | Pattern emergence detector | Speculative. Defer. |
| #277 | Configuration accretion log | Subsumed by git history. Close. |
| #278 | Skill execution telemetry | Depends on MCP arc landing. Defer. |
| #279 | Generality assessment process | Captured in PR review checklist (informal). Close or scope to a CI lint. |
| #280 | Security review pipeline | Captured in existing audits (cargo audit / hunted_tier / cscli). Close. |
| #281 | Composability check for primitive proposals | Captured in trait_consistency phase. Close. |
| #282 | Deprecation safety net | Captured in backward-compat doctrine. Close. |
| #283 | Reusability test for code contributions | Captured in PR discipline. Close. |
| #289 | VARIATION_AND_CONTRIBUTION.md doctrine | Redundant with VARIATION_GUARANTEES.md. Close. |
| #293 | LFI augmentations for reference matching | Deferred to LFI Claude. |
| #294 | Rust-everywhere discipline enforcement | Already enforced via Cargo.toml `unsafe_code = "deny"` + clippy + the doctrine memo. Close. |

### Recommended task-list cleanup for arc 3

Mark `deleted` (subsumed): #275, #277, #279, #280, #281, #282, #283, #289, #294.

---

## Arc 4 — Real-website validation (#207, #208, #218-#222)

These are operational tasks, not architectural. Pixel-reproduction
loops against real sites. Per memory `[[pixel-reproduction-needs-
live-infra]]`: multi-hour per site, requires Crawler+Forge+
headless-Firefox+diff loop. Not single-cron-fire work.

| Task | Subject |
|------|---------|
| #207 | #15 real-website validation |
| #208 | #93 real-website validation rotation |
| #218 | prosperityclub.com pixel reproduction |
| #219 | plausiden.com pixel reproduction |
| #220 | sacred.vote Forge-static approximation |
| #221 | Stripe pixel reproduction |
| #222 | Linear/Vercel/GitHub/Notion/Anthropic/Render/Fly rotation |

Recommendation: park in the task list as known follow-up work;
batch the rotation when the reference-matching arc lands.

---

## Arc 5 — Substrate-shape deepening (#210, #213)

Carryover tasks from the substrate-deepening sprint.

| Task | Subject | Action |
|------|---------|--------|
| #210 | Complete-Rust-stack codegen (#101) | Multi-iteration build. Park. |
| #213 | Variant explosion to 300-500 primitives (#104) | Multi-iteration build. Park. |

---

## Cleanup recommendations summary

Tasks to `status=deleted` (subsumed by shipped work):
* Arc 1: #242, #245, #253, #255, #256, #260
* Arc 3: #275, #277, #279, #280, #281, #282, #283, #289, #294

Tasks to defer (depend on un-shipped infrastructure):
* Arc 1: #247, #249, #257
* Arc 3: #276, #278, #293

Tasks to re-classify:
* #252 → reference arc
* #250 → content/template arc (new — not under variation)

Tasks to keep as next-iteration candidates:
* Arc 1 controlled-mutability: #238, #239, #240, #246, #248
* Arc 2 reference-matching: #263, #290 (paired)
* Arc 3 MCP: #284-#288 (cluster, ship as a unit)
* Arc 4: pick one pixel reproduction when #284+ unblocks
  better diff tooling

---

## Why this audit matters

Per advisor catch + paul's "tasks they are piling up": the backlog
grew faster than completions because design-doc dumps enumerated
sub-tasks at the wrong granularity. This document right-sizes
the backlog so each remaining task corresponds to a real
deliverable, not a doc-bullet that should be closed by reference
to existing shipped code.

After applying the cleanup recommendations, the active task count
drops from ~50 to ~15 — a tractable backlog that maps to actual
remaining substrate work.
