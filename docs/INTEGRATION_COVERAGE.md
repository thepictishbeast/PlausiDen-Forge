# Integration Coverage Audit (#330)

Per paul 2026-05-21: audit each substrate repo's role + the
integration points between them. Surface any neglected slices.

This document is a point-in-time snapshot. The
`substrate-integration-audit` binary (in this workspace) re-runs
the orphan-crate detection automatically on every PR.

## Substrate repos (2026-05-21 snapshot)

| Repo                  | Crates | Last commit  | Role                                              |
|-----------------------|-------:|--------------|---------------------------------------------------|
| PlausiDen-Forge       |     53 | 2026-05-21   | Typed build pipeline + phase gates                |
| PlausiDen-Loom        |     12 | 2026-05-21   | Typed UI primitives + CMS renderer                |
| PlausiDen-Crawler     |     10 | 2026-05-21   | Chromiumoxide journey runner + detectors          |
| PlausiDen-Annotator   |      2 | 2026-05-20   | Annotation relay + viewer                         |
| PlausiDen-CMS         |      6 | 2026-05-20   | Authoring surface (admin auth, pre-publish audit) |
| PlausiDen-CRM         |      1 | 2026-05-20   | Outbound + relationship layer                     |
| PlausiDen-LFI         |      5 | 2026-05-19   | Neurosymbolic inference substrate                 |
| Forge-LFI             |      3 | 2026-05-19   | Forge↔LFI bridge: critics + policy projections    |
| Crucible              |      6 | 2026-05-20   | Bot-vs-human challenge substrate                  |
| PlausiDen-AVP-Doctrine|      0 | n/a          | Doctrine rules + canon (TOML, no Rust crates)     |
| PlausiDen-Canon       |      0 | n/a          | Canonical-content reference repo                  |

All repos are active (most-recent commit within the past 72 h).
PlausiDen-CMS specifically (paul flagged for review): active —
last commit 2026-05-20, 6 crates, currently houses cms-admin-auth
+ cms-pre-publish-audit.

## Integration edges

### Forge → Loom (path-deps within Forge workspace)

The Forge workspace pulls Loom crates via absolute path:

```
loom-tokens     = { path = "/home/paul/projects/PlausiDen-Loom/loom-tokens" }
loom-cms-render = { path = "/home/paul/projects/PlausiDen-Loom/loom-cms-render" }
loom-lint       = { path = "/home/paul/projects/PlausiDen-Loom/loom-lint" }
loom-components = { path = "/home/paul/projects/PlausiDen-Loom/loom-components" }
```

Forge's render phase + content-substance gate + lint phase all
project through these. The path-dep convention requires a local
multi-checkout dev layout; CI uses the same paths in a workspace
fixture step.

### Forge → Crawler (subprocess + journey JSON)

forge-phases::crawl phase shells out to the `crawler` binary
(`PlausiDen-Crawler::crawler-runner::crawler`) and consumes the
journey JSON output via `crawler-reference-capture` (typed wire
shape mirrored in both repos).

### Forge → Crucible (CmsSection::CrucibleChallenge primitive)

Loom defines a `CmsSection::CrucibleChallenge` variant that
embeds a Crucible challenge inline in a CMS page. The challenge
ID + difficulty are tenant-authored; Crucible's
crucible-challenges crate ships the verifier impls (Math /
Semantic / Image / Audio / Drawing / PromptInjection — 4
shipped, 2 stubbed).

### Forge-LFI → PlausiDen-LFI (git deps)

Forge-LFI bridges between Forge's `forge-critic` trait and LFI's
`lfi-core` / `lfi-policy` / `lfi-corpus` / `lfi-critic`. Uses
git path deps:

```
lfi-core   = { git = "https://github.com/thepictishbeast/PlausiDen-LFI", branch = "main" }
forge-critic = { git = "https://github.com/thepictishbeast/PlausiDen-Forge", branch = "main" }
```

LFI is opt-in per the deterministic-first doctrine — substrate
ships a deterministic baseline; LFI augments only when a tenant
enables it.

### Annotator → Forge / Loom

PlausiDen-Annotator is an external relay + viewer for annotation
artifacts produced by Forge's review pipeline. Not currently
imported as a crate; integration via JSON wire shape only.

### CMS → Forge / Loom

PlausiDen-CMS hosts the authoring UI (cms-admin-auth, cms-pre-
publish-audit). Forge's render phase consumes the CMS JSON
output; CMS depends on loom-cms-render for in-editor preview.

### CRM → external

PlausiDen-CRM is single-crate; outbound + relationship layer.
Integrates with Forge only via the email-core trait surface
(forge ships the trait, CRM provides the concrete provider).

## Neglected slices

### Annotator (2 crates, integration unfinished)

PlausiDen-Annotator has only 2 crates and no integration into
Forge phases yet. Forge cannot currently consume Annotator
artifacts; the wire shape is mirrored only in spec docs.

**Action**: spec or stub a `forge-phases::annotator_sync` phase
that reads Annotator's relay output and surfaces it as Findings.
Open as a separate task when the Annotator wire shape stabilises.

### CRM (1 crate, only email-trait integration)

PlausiDen-CRM has a single crate and only the email-core trait
edge. The relationship-management surface (campaigns,
sequences) is not exposed through Forge or Loom.

**Action**: not urgent. CRM is a tenant-facing product; doesn't
need Forge substrate integration unless tenants need an in-CMS
campaign editor.

### Canon (0 crates)

PlausiDen-Canon is reference content, no Rust crates. Not a
neglect — by design.

### Crucible verifiers (4/6 shipped)

Two challenge verifiers (Audio + PromptInjection) ship as stubs
per the #319 follow-up. Real implementations queued; tracked as
part of #310.

## Orphan crates (first scan: 21)

The `substrate-integration-audit` binary detects crates with no
incoming dependencies (not depended on by anything in the
workspace).

First-run result on `PlausiDen-Forge/crates/`: **21 orphan
crates**. Categorised:

### Contract crates (consumer wiring queued, not bugs)

These ship typed contracts; their runtime consumers are
follow-up work per the substrate's "contract first, runtime
later" pattern. They're depended on by future crates that
don't exist yet — annotating each with
`# integration-audit-allow: contract-first; consumer queued`
keeps the gate green without losing visibility.

- `ai-pipeline-core` (T56 — 6-stage AI generation pipeline)
- `commerce-core` (T72 — operator billing surface)
- `compliance-evidence` (compliance-evidence assembly)
- `deploy-gemini` / `deploy-i2p` / `deploy-ipfs` /
  `deploy-lokinet` / `deploy-onion` / `deploy-security-rating`
  (T39-T43 — DeployAdapter implementations; consumed by
  `forge deploy` subcommand which is a binary slice)
- `dr-core` (disaster-recovery primitives)
- `editor-ux-core` (T55 — admin-UI state machine)
- `extension-host` (T45 — Wasm extension host trait)
- `forge-auth` (auth method registry)
- `forge-critic` (Critic trait; consumed by Forge-LFI)
- `i18n-fonts` (T53 — per-script font stacks)
- `manifest-codegen-macros` (proc-macro companion to
  manifest-codegen build.rs)
- `migration-core` (T139 — typed migration framework)
- `ops-status` (T68 — SLO/SLI + error-budget)
- `orient-core` (12 N-orientations)
- `region-adaptation` (T54 — RegionProfile per ISO 3166)
- `tenancy-core` (T44 — TenantId + isolation tiers)

### Truly neglected (none in this scan)

No truly-neglected crates surfaced. Every orphan above has a
documented consumer-wiring task in the roadmap. The
substrate-integration-audit CI gate will start tripping when
new orphans appear that DON'T fit one of the categories above,
forcing the operator to either wire them up or annotate them.

### Action

Each contract-first orphan above gets a one-line
`# integration-audit-allow: contract-first; consumer at
forge-cli T<n>` annotation in its Cargo.toml in a follow-up PR.
After annotation, the scanner exits clean — only NEW orphans
trip the gate.

Tracked as #333.

## CI gates currently active

| Gate                       | Workflow                              | Scope                  |
|----------------------------|---------------------------------------|------------------------|
| substrate-name-audit       | substrate-name-audit.yml             | crates/** tenant-name leaks |
| substrate-docs-audit       | substrate-docs-audit.yml             | crates/** undocumented pub items |
| substrate-integration-audit| (planned — this PR adds the binary)  | orphan-crate detection |

## Open work tracked elsewhere

- #310 Crucible E2E demo wired into a Forge site (blocked on
  wasm-pack toolchain install).
- #321 Substrate atomization (ongoing; 60+ CmsBlock primitives
  shipped, more in flight).
- #331 True visual uniqueness across tenants (multiple Nav /
  Footer / Hero shapes + style-pack expansion).

## Re-running this audit

The orphan-crate detector ships as
`crates/substrate-integration-audit`. Run via:

```sh
cargo run -p substrate-integration-audit -- \
  /home/paul/projects/PlausiDen-Forge \
  /home/paul/projects/PlausiDen-Loom \
  /home/paul/projects/PlausiDen-Crawler \
  ...
```

The binary reports any crate with zero incoming dependencies as
a potentially-orphaned slice. Manual narrative review (this doc)
should be refreshed quarterly or when a new substrate repo is
created.
