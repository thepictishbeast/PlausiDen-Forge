# PLAUSIDEN_ECOSYSTEM.md

Top-level orientation map for the PlausiDen ecosystem. **Read this when you land in any PlausiDen repo and need to know how it relates to the others** — before reaching for code, search, or commands.

> Companion to the per-repo `AGENTS.md` (which orients *within* a repo) and `forge orient` (which prints affordances + scoped doctrine inside Forge). This document orients *across* repos.

---

## TL;DR — the one-line map

```
   axiom floor        meta-doctrine         substrate                  product / ops
   ──────────────     ─────────────         ────────                   ─────────────
   PlausiDen-Meta  ─→  PlausiDen-AVP-     ─→ PlausiDen-Canon       ─→  PlausiDen-Forge     ─→  (deployed sites)
                       Doctrine              PlausiDen-Loom               (build pipeline)
                                             PlausiDen-CMS
                                             PlausiDen-Crawler
                                             PlausiDen-Annotator
                                             PlausiDen-LFI ─→ Forge-LFI
                                             (each: typed *-core surfaces + tools)
```

Read direction: **lower layers constrain upper layers, never the reverse.** A Forge phase can cite an AVP-Doctrine rule; a doctrine rule never imports forge code.

---

## Repos at a glance

| Repo | Class | Primary purpose | What it owns |
|------|-------|-----------------|--------------|
| **PlausiDen-Meta** | meta-doctrine | Ecosystem priority + axiom floor + governance gate | `AXIOM_FLOOR.md`, priority ordering between sub-doctrines |
| **PlausiDen-AVP-Doctrine** | meta-doctrine | Adversarial validation protocol + agent standing orders + typed doctrine rule database | `doctrine/rules/*.toml` (71 rules across 9 domains), `SUBSTRATE_DISCIPLINE.md`, `DETERMINISTIC_FIRST.md`, AVP-2 36-pass protocol, multi-agent standing orders |
| **PlausiDen-Canon** | infrastructure | Canonical invariants + cross-repo type definitions that *everything* depends on | Root types, ULID + Ed25519 conventions, ISO/IEC standard references |
| **PlausiDen-Loom** | substrate | Typed UI primitives + design tokens + theme system | `loom-tokens` (skin.css generation), `loom-cms-render` (CmsSection enum + render impls), `loom-lint`, `loom-components` |
| **PlausiDen-CMS** | substrate | Typed content schema + content lifecycle types | `CmsPage` + `CmsSection` shapes (mirrored into Loom); content-domain *-core crates |
| **PlausiDen-Crawler** | substrate | chromiumoxide journey runner + runtime DOM detectors | Journey schema + Detector trait + journey runtime; consumer-agnostic (Loom / Forge / third-party sites) |
| **PlausiDen-Annotator** | substrate | Visual + textual annotation layer for Crawler output | Annotation primitives, consumed by Crawler post-runs |
| **PlausiDen-LFI** | substrate | Lattice of Formal Inference — neurosymbolic core (Critic trait, candidate generation seams) | Use-case-agnostic LFI. Default-OFF; opt-in per `[[deterministic-first-lfi-optional]]` |
| **Forge-LFI** | substrate | Forge-tailored downstream of PlausiDen-LFI | Forge-specific Critic impls; bridges LFI ↔ Forge phases |
| **PlausiDen-Forge** | product / pipeline | Site build pipeline + audit framework + doctrine CLI + substrate-bypass register | `forge` binary, `forge-phases` (every audit phase), `forge-cli`, `doctrine-core`, capability-request workflow |

---

## Dependency direction

```
        ╭──────────────────────────────────────────╮
        │ PlausiDen-Meta        (axiom floor)      │
        ╰──────────────────────────────────────────╯
                          │
                          ▼ defines priority + axioms
        ╭──────────────────────────────────────────╮
        │ PlausiDen-AVP-Doctrine                   │
        │   • doctrine/rules/*.toml                │
        │   • SUBSTRATE_DISCIPLINE                 │
        │   • DETERMINISTIC_FIRST                  │
        │   • AVP-2 protocol                       │
        ╰──────────────────────────────────────────╯
                          │
                          ▼ rules cited by code
        ╭──────────────────────────────────────────╮
        │ PlausiDen-Canon                          │
        │   • canonical types                      │
        │   • ISO/IEC + IETF references            │
        ╰──────────────────────────────────────────╯
                          │
       ┌──────────────────┼─────────────────┐
       ▼                  ▼                 ▼
 ╭──────────╮      ╭──────────────╮    ╭──────────────╮
 │  Loom    │      │     CMS      │    │   Crawler    │
 │ tokens / │      │ typed page / │    │ journey /    │
 │ primitives│     │ content      │    │ detectors    │
 ╰──────────╯      ╰──────────────╯    ╰──────────────╯
       │                  │                 │
       ▼                  ▼                 ▼
 ╭─────────────────────────────────────────────╮
 │           PlausiDen-Annotator               │
 │      (annotations layered on Crawler)       │
 ╰─────────────────────────────────────────────╯
       │                  │
       ▼                  ▼
            ╭───────────────────────╮
            │   PlausiDen-Forge     │
            │  build pipeline +     │
            │  audit framework      │
            ╰───────────────────────╯
                       │
                       │ (LFI is opt-in augmentation)
                       ▼
            ╭───────────────────────╮
            │     Forge-LFI         │
            │  Critic impls / AI    │
            ╰───────────────────────╯
                       │
                       ▼
            ╭───────────────────────╮
            │   PlausiDen-LFI       │
            │  neurosymbolic core   │
            ╰───────────────────────╯
```

Rules of motion:
1. **Lower layer cannot import upper layer.** Loom does not depend on Forge. Canon does not depend on Loom.
2. **AVP-Doctrine rules are projection-only** — `forge doctrine` reads them; doctrine code does not call forge code.
3. **LFI is opt-in.** Every Forge feature has a deterministic baseline; LFI is augmentation behind a feature flag per `[[deterministic-first-lfi-optional]]`.

---

## Which repo for which question?

| If you want to… | Look in |
|------------------|---------|
| Add a new UI primitive (e.g., a new hero variant) | **Loom** (`loom-cms-render` + `loom-tokens`) |
| Add a new content section type | **CMS** schema → mirror into **Loom** CmsSection variant |
| Add a new audit phase (token contrast / dead-link / etc.) | **Forge** (`forge-phases`) |
| Add a new doctrine rule | **AVP-Doctrine** (`doctrine/rules/<domain>.toml`) |
| Add a new runtime detector (DOM-time check) | **Crawler** (Detector trait impl) |
| Add an annotation layer on Crawler output | **Annotator** |
| Capture an axiom that constrains *the platform itself* | **Meta** (`AXIOM_FLOOR.md`) |
| Define a canonical invariant or shared type | **Canon** |
| Make Claude / Gemini / other agents use a new tool | **Forge** (`forge orient` + AGENTS.md + TOOLS.md + skills/) |
| Add LFI-augmented critic | **PlausiDen-LFI** (generic) then **Forge-LFI** (Forge bridge) |

---

## Doctrine roots

Every load-bearing decision in the ecosystem traces back to AVP-Doctrine. The three top-level doctrines you should be familiar with:

1. **`SUBSTRATE_DISCIPLINE.md`** — hand-coding HTML/CSS/JS in site repos is forbidden; every gap is a substrate change. Enforced by `forge` phase `substrate_purity` + capability-request workflow + bypass register.

2. **`DETERMINISTIC_FIRST.md`** — deterministic baseline before LFI/LLM augmentation; LFI is opt-in not load-bearing. Every capability has a deterministic implementation; LFI augmentation is feature-flagged.

3. **Rule database** — 71 rules across 9 domains (`build` / `primitives` / `security` / `testing` / `docs` / `logging` / `perf` / `content` / `accessibility`). Query via `forge doctrine query` / `forge doctrine for <path>`.

---

## Canonical defaults (across every repo)

These hold across the entire ecosystem (Forge, Loom, Crawler, CMS, Canon, LFI):

- HTTP service → **axum + tokio + tower**
- Async runtime → **tokio** (multi-thread)
- Database → **sqlx** (compile-time-checked)
- HTML emission → **maud** (typed compile-time) OR Loom primitives
- Serialization → **serde** + `deny_unknown_fields` at every input boundary
- Crypto → **ed25519-dalek** + **ML-DSA** (post-quantum) where dual-stack
- CLI → **clap** (derive)
- Errors → **anyhow** (binaries) / **thiserror** (library boundary)
- Property tests → **proptest**
- Logging → **tracing** + structured JSON
- `forbid(unsafe_code)` on Forge crates; no `unwrap`/`expect` outside tests

If a repo deviates, that deviation is a doctrine bug — file it.

---

## Cross-repo dev loop

Loom is consumed by Forge via git dependency. For local iteration on Loom from Forge, the Forge `Cargo.toml` carries a `[patch."https://github.com/.../PlausiDen-Loom.git"]` block pointing to the local path (see task #144 for the chronic gate).

Crawler is invoked from Forge phases via subprocess; no Cargo dep.

LFI is opt-in: Forge phases that *can* be LFI-augmented declare a `Critic` trait seam; the default impl is deterministic, and Forge-LFI provides the augmented impl.

---

## Multi-Claude coordination

Multiple Claude Code instances may operate across the ecosystem concurrently. The standing-orders protocol is in **PlausiDen-AVP-Doctrine/standing-orders/** — read those when running in a multi-agent context. Solo Claude on a single repo does not need them (per `[[standing-orders-not-canonical]]`).

LFI repos are **PR-only** from this instance (per `[[lfi-out-of-scope-for-this-instance]]`); the dedicated LFI Claude merges. Branch + commit + push + open PR. Do not direct-push to main on PlausiDen-LFI / Forge-LFI.

---

## See also

- `PlausiDen-Forge/AGENTS.md` — orientation when working in Forge specifically.
- `PlausiDen-Forge/TOOLS.md` — canonical Forge CLI command index.
- `PlausiDen-AVP-Doctrine/SUBSTRATE_DISCIPLINE.md` — Rule 0.
- `PlausiDen-AVP-Doctrine/DETERMINISTIC_FIRST.md` — LFI architectural posture.
- `PlausiDen-AVP-Doctrine/doctrine/rules/SCHEMA.md` — rule schema reference.
- `PlausiDen-Forge/skills/README.md` — high-frequency task playbooks.
- `forge orient` — live, machine-readable session brief (cross-AI).
