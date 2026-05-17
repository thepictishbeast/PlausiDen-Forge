# Forge — vision document

> "If Forge was already built and did everything we wanted, what
> would this doc say?"

This is that doc. It describes Forge as it should be — the
finished product, not the current snapshot. Sections marked
**[shipped]** are working today on `main`. **[in-flight]** is
mid-build. **[queued]** has a task ID. **[concept]** is here
because it's been requested or implied by an owner directive
and a developer should design it.

Where this doc and the code disagree, the code wins this week
and the doc wins next week. The roadmap section maps capabilities
to the work that closes the gap.

---

## 0. Companion documents

This vision says **what** Forge does and **where it's going**.
Three companion docs in the same `docs/` directory carry the
deeper layers:

| Doc | What it covers |
|---|---|
| [`ARCHITECTURE_PRINCIPLES.md`](./ARCHITECTURE_PRINCIPLES.md) | **Why** the substrate is shaped the way it is. WordPress-inversion principle. Capability manifest as constitution. Primitives as constraint system. Token axes + style packs. AI as bounded neurosymbolic search. Editor UX (canvas-dominant, ⌘K + AI bar). Quality gates as continuous gradient. Strangler-fig retrofit. Substrate-flexible / product-opinionated stance. |
| [`SITE_OPERATIONS.md`](./SITE_OPERATIONS.md) | **What every published site needs to operate in the real world.** Required pages by site type. Live linkage between practice + policy. Site-success operational layer (search verification, business directory, DNS hygiene, email deliverability, registrar best practices). Multi-network publishing (Tor / I2P / Lokinet / IPFS / Gemini). Threat-model tiers + security rating dashboard. Mainstream vs sovereign per-dimension duality. |
| [`ENGINEERING_DISCIPLINES.md`](./ENGINEERING_DISCIPLINES.md) | **What every engineer working on Forge needs to know.** Caching + invalidation with surrogate keys. Concurrency + database choices. Time + clocks + ordering. Background jobs. Webhook delivery semantics. State machines. Cryptography in concrete (Argon2id, XChaCha20-Poly1305, Ed25519, ML-KEM-768). Secrets management. Multi-region complexity. Failure modes from system dynamics. DB migration safety. Incident handling. Open-source strategy. Costs + funding implications. Decision-making framework. Honest revisions to earlier doctrine. |

Companion docs cross-reference each other and back to this
vision. **Read them as one connected design**, not as separate
artifacts — the principles inform the operations inform the
disciplines inform the vision.

---

## 1. What Forge IS

Forge is the **build pipeline** for the PlausiDen ecosystem. It
takes a typed CMS source (JSON edited by Loom or hand-authored)
and produces a signed, attested, accessibility-audited, dual-
themed static site bundle ready for atomic deploy.

It is **not**:

- a frontend framework (Loom is the typed component layer)
- a runtime server (the build output is plain HTML/CSS — any
  static host serves it)
- a JavaScript bundler (zero JS by default; opt-in only when
  the page genuinely needs interactivity)
- a CMS (Loom is the CMS; Forge consumes its output)

Forge's contract: feed it a project root with `cms/*.json` and
`forge.toml` and it returns a verified bundle plus a build
report listing every finding, severity-ranked.

## The meta-mission: making AI-built UI reliable

Every PlausiDen tool — Loom, CMS, Forge, Crawler, Annotator,
Oxidizer — exists for one common reason: **AI agents building
GUI / frontend / UX work need a reliability substrate that
humans don't.** A human dev opens DevTools, eyeballs the layout,
fixes the colour. An AI agent doesn't open DevTools — so without
typed primitives, schema-validated content, mathematical contrast
verification, runtime audit, ecosystem-wide doctrine enforcement,
and human-review capture, regressions ship silently every
iteration.

Forge's specific contribution: **the build-time gate.** Every
audit phase Forge runs is a check the agent gets BEFORE Mom sees
the broken page. WCAG contrast, CSP strictness, link-target
validity, semantic HTML, dual-theme parity, schema-validated
CMS, signed manifest — all enforced before the bundle reaches
the deploy step. An agent edit that violates any of them is
held at the build, not in production.

The six PlausiDen tools cover six different timeslices of the
agent-driven UI work:

| Tool | Where it operates | What it gives the agent |
|---|---|---|
| **Loom** ([vision](https://github.com/thepictishbeast/PlausiDen-Loom/blob/main/docs/LOOM_VISION.md)) | Edit-time | Typed primitives + schema-validated CMS so edits can't silently corrupt data |
| **CMS** ([vision](https://github.com/thepictishbeast/PlausiDen-CMS/blob/main/docs/CMS_VISION.md)) | Storage + multi-tenant | Per-tenant isolation + signed audit log so agent ops are forensically attributable |
| **Forge** (this doc) | Build-time | Audit gates (a11y / contrast / CSP / semantic HTML / theme parity) so regressions are held |
| **Crawler** ([vision](https://github.com/thepictishbeast/PlausiDen-Crawler/blob/master/docs/CRAWLER_VISION.md)) | Runtime / post-deploy | Typed Findings from real browser execution — agent's runtime oracle |
| **Annotator** ([vision](https://github.com/thepictishbeast/PlausiDen-Annotator/blob/master/docs/ANNOTATOR_VISION.md)) | Human-in-the-loop | Captured human review as JSON the agent can act on |
| **Oxidizer** (vision in `PlausiDen-Oxidizer/docs/`) | Doctrine-time | Ecosystem conformance gate — agent can't introduce non-Rust / non-supersociety regressions |

Forge integrates with all five siblings:

- **Loom** → Forge's `phase_render` calls `loom_cms_render::page_shell`
  in-process [shipped T70 + T70b].
- **CMS** → publish event hands rendered bundle to Forge for audit
  before write [queued].
- **Crawler** → Forge's `phase_crawl` invokes Crawler runtime audit
  pre-deploy [partial].
- **Annotator** → `phase_annotation_review` consumes Annotator session
  JSON as findings [shipped 2026-05-17] — reads `[review] session_dir`
  from `forge.toml`, walks `*.json` sessions, maps operator tags
  (`a11y` / `contrast` / `bug` → Strict; `alignment` / `copy` / `perf`
  / `suggestion` / `other` → Warn) to typed Findings clustered per
  page from `session.meta.url`. Closes task #13. Silent skip when
  unconfigured.
- **Oxidizer** → `phase_oxidizer_conformance` ensures Forge itself
  passes Rust-first + supersociety doctrine on every build [concept].

## 2. The supersociety stack Forge uses

Tech-of-tomorrow choices, layered for defence in depth:

- **Memory-safe core** — Rust everywhere. The deprecated bash
  `forge.sh` is the parity reference until T54 deletes it.
- **Type-safe rendering** — Forge calls `loom_cms_render::render_page`
  IN-PROCESS via a Cargo dep [shipped T70]. No subprocess shell-out,
  no JSON-roundtrip-through-CLI escape risk. CmsPage is the
  contract; serde with `deny_unknown_fields` is the gate.
- **Capability-based filesystem writes** — `WriteCapability::for_dir`
  canonicalises a confine-root and refuses any write that escapes.
  Atomic temp-file + rename for every output.
- **Cryptographic provenance** — every bundle carries an Ed25519
  signature over its manifest [shipped T56 in Forge / T47c in Loom];
  the trust anchor is OUT-OF-BAND; bundle-local pubkeys are
  convenience metadata only and must match the configured anchor
  (constant-time compare via `subtle`).
- **Merkle-chained build reports** — every report carries the
  SHA-256 of the previous report; an attacker who tampers with
  one report has to forge every subsequent one [shipped T26].
- **Strict CSP** — every Loom-rendered page uses `style-src 'self'`
  + sha256-pinned inline blocks, never `unsafe-inline` / `unsafe-
  hashes`. Inline overlay JS in the editor preview is hashed too
  [shipped T62 step 9, step 10].
- **WCAG 2.1 AA / ISO/IEC 40500 by default** — every site
  Loom generates ships dual theme + skip link + focus-visible +
  reduced-motion + semantic landmarks WITHOUT integrator wiring
  [shipped T48c v1, v2].
- **Property-based + mutation testing** — proptest on every
  parser; `forge audit mutants` runs the AVP-2 Tier 6 mutation
  campaign [shipped T58].
- **Reproducible builds** — same inputs → bit-identical bundle
  (content-addressed bundle dirs prove this).
- **Privacy-preserving uploads** — JPEG / PNG metadata strip
  before content-addressed write [shipped T62 step 7]; GIF/WebP
  pending [queued T62 step 7b].

## 3. Personas

### 3.1 Mom — non-technical client (the gold standard)

Mom runs a small bakery. She doesn't know HTML. She wants a
website she can update on her own.

What Mom does:

1. `loom site init mybakery --template basic` — gets a complete
   site. (T48b ships portfolio + blog templates.)
2. `loom edit-serve` opens a browser editor.
3. **She clicks any text in the live preview and types over it.**
   Hit Enter to save. [shipped — T62 step 10]
4. She uploads a photo from her iPhone. **GPS / EXIF stripped
   automatically** before storage. Her home address never leaks.
5. **An interactive in-browser tour** walks her through the
   editor on first visit [shipped T64b — query-string-driven
   tour via `?tour=N` URL param; `parse_tour_query()` +
   contextual overlay in `loom edit-serve`].
6. She clicks "Publish". Atomic deploy ships a signed bundle.
7. She breaks something? **`loom deploy rollback` flips back in
   one command.** [shipped T47]

What Mom never has to think about:

- Path traversal, CSRF, XSS, mixed-content warnings, CSP, cookies.
- Typing JSON, editing CSS, picking colours that pass contrast.
- Whether her site works in dark mode — it just does.
- Whether her site is accessible — it just is.

What Mom can ALSO do (when she asks):

- **WebAuthn passkey login** — no password, just her phone or
  YubiKey [queued T43d].
- **Multiple sites under one account** — bakery, knitting club,
  family newsletter, all isolated [queued T45].

### 3.2 The technical client — wants control

A small-business owner who CAN write Markdown but not Rust. Wants
to pick a different colour, swap fonts, add a contact form
endpoint without learning a framework.

What they get:

- **Loom design tokens are JSON.** Edit `tokens-dark.json` and the
  whole dark mode re-skins. Light/dark parity gate (T66) prevents
  drift.
- **`forge.toml` controls build mode.** `mode = "production"`
  fails on warn-severity, not just strict.
- **Custom backends declared in `backends.toml`.** Forge audits
  that every `data-backend="X"` has a matching declaration.
- **Bundled component variants** they can compose without
  writing CSS — Hero, Group, Banner, CardFeed, Sidebar, Composer
  (typed enums; no string blindness).
- **Live editor with click-to-edit.** They get the same surface
  Mom does — but the form pane on the left exposes every typed
  field for fine-grained control.
- **Theme switcher** — zero-JS form-POST switches data-theme
  and the cookie sticks [queued T37].
- **A configurable per-tenant Claude Code SSH bridge** — sandboxed
  agent runs inside their tenant's workspace, can't see other
  tenants [queued T46].

### 3.3 The developer — contributor or forker

Someone who wants to extend Forge with a new phase, or fork it
for their own purposes.

What they get:

- **Phase trait** — implement `name()` + `run(&BuildCtx) ->
  Result<Vec<Finding>, BuildError>` and you have a new phase.
  Register it in `lib.rs`. ~50 lines of boilerplate.
- **20+ ported phases** as worked examples (a11y_landmarks,
  contrast, csp, html_semantic, link_check, perf_budget, sri,
  tokens, …).
- **Crate boundary discipline** — `forge-core` is types only
  (no I/O), `forge-phases` is implementations, `forge-cli` is
  the binary. New phases land in `forge-phases`.
- **Inline annotation grammar** — `BUG ASSUMPTION:` /
  `AVP-PASS-N:` / `SECURITY:` / `REGRESSION-GUARD:` /
  `SHIP-DECISION:` / `SCHEMA:` are all machine-grepable.
  Future-them can audit the lineage of any line.
- **`cargo mutants` / proptest / `cargo audit` / `cargo geiger`**
  all wired up via `forge audit` subcommands.
- **`forge verify --chain --signatures`** lets them prove a
  given bundle came from a particular trust anchor.
- **TLA+ specification of the phase pipeline invariants**
  [queued T27] — formal model of what "the build is valid"
  means, with refinement proofs that the Rust code satisfies it.

What developers want next:

- **Type-state phase pipeline** [queued T24] — the phase order
  becomes a compile-time guarantee. Trying to run `phase_render`
  after `phase_attest` is a compile error.
- **`forge-watch`** — inotify-driven re-run on edit [queued, no
  task ID; design covered in `Cargo.toml` future-crates list].
- **`forge-html`** — fast read-only HTML parser wrapper around
  `lol_html` so phases that need real parse trees stop hand-
  rolling substring scans [queued T67 follow-up].
- **`forge-css`** — `lightningcss` wrapper for the CSS-touching
  phases (theme_consistency, contrast, dual_theme).
- **`forge-report`** — JSON + terminal renderers separated from
  CLI so SaaS deployments can render reports server-side.
- **Dynamic frontend mode** [queued T12] — opt-in escape hatch
  for sites that DO need JS, with the same security guarantees.

### 3.4 Claude Code (and other autonomous agents)

A future where Claude instances are the primary content authors
working at scale.

What an agent gets:

- **Stable JSON contract** — `cms/<slug>.json` is the addressable
  surface. Read, mutate, write — the typed schema makes drift
  impossible.
- **`loom site init`** is one command. **`loom deploy publish`**
  is one command. **`loom deploy rollback`** is one command.
  Each is idempotent and has a single deterministic output.
- **`loom edit-serve --port N`** so the agent can run multiple
  isolated editor instances in parallel.
- **Inline-edit POST** is shaped for programmatic use too —
  `application/x-www-form-urlencoded` body, JSON-friendly
  response, auth via cookie OR API key (key auth queued).
- **Forge build report is JSON** [shipped]. An agent reads
  findings programmatically, decides whether to fix or escalate.
- **Sandboxed Claude SSH bridge** [queued T46] — the eventual
  vision is each tenant's editor exposes a per-tenant Claude
  Code session running inside the tenant's workspace, with no
  outbound access except through approved channels.
- **Multi-instance parallelism** [queued T45] — N tenants
  each with isolated SQLite + workspace + agent. The orchestrator
  spawns one Claude per tenant per task.
- **Annotator integration** [queued, see PlausiDen-Annotator
  directive] — agents can replay an annotated browser session
  to understand what a human reviewer flagged.
- **Cross-repo contribution protocol** — when an agent in
  Forge spots a fix that applies to Loom, it follows the
  CROSSFIX flow (commit with `AVP-CROSSFIX from <source>:
  <description>`, run AVP-2 Tiers 1–3 in the sibling repo,
  return).

## 4. Capability map

### 4.1 Content authoring

| Capability | Status |
|---|---|
| Typed CMS (`CmsPage` + `CmsSection` enums) | shipped |
| Typed editor forms per kind | shipped |
| Click-to-edit inline editing in live preview | shipped (T62 step 10) |
| Section reorder / delete / add | shipped |
| Bundled site templates (`basic`) | shipped (T48 + T48c v1) |
| Bundled portfolio + blog templates | ✅ shipped (T48b) — `loom site init --template portfolio` / `--template blog` (run `loom site templates` for the list) |
| Compound-field inline editing (group.body[N], cards) | queued (T62 step 10b) |
| Markdown import → CmsSection | queued (T63b — extend importer) |
| WordPress export → CmsSection | concept |
| Notion export → CmsSection | concept |

### 4.2 Image handling

| Capability | Status |
|---|---|
| Multipart upload, magic-byte sniff (JPEG/PNG/GIF/WebP) | shipped |
| Content-addressed storage with `Cache-Control: immutable` | shipped |
| EXIF / GPS / metadata strip on JPEG + PNG | shipped (T62 step 7) |
| EXIF strip on GIF + WebP | queued (T62 step 7b) |
| In-browser image picker for editor | queued (T62 step 8) |
| Responsive `<picture>` with WebP/AVIF fallback | concept |
| Auto-resize at deploy time | concept |

### 4.3 Theming + accessibility

| Capability | Status |
|---|---|
| Light + dark themes by default | shipped (T48c v1, v2) |
| `prefers-color-scheme` honoured + `<meta name="color-scheme">` | shipped |
| `prefers-reduced-motion` honoured | shipped |
| WCAG 2.1 AA contrast verified at compile + runtime | shipped (T29, T29b) |
| Semantic HTML enforced (`<div role="banner">` blocked) | shipped (T67) |
| Dual-theme presence enforced | shipped (T66) |
| Dual-theme contrast audit (both palettes) | ✅ shipped (T68) — `phase_theme_contrast` delegates to `loom theme contrast` which enumerates per-theme; current dogfood walks 12 themes (auto / dark / default / forest / hc-dark / hc-light / light / ocean / rose / sepia / violet / warm), all clear @ 4.5:1 WCAG AA |
| Zero-JS theme/density/font switcher (form-POST cookie) | queued (T37) |
| Keyboard-only navigation audit | concept |

### 4.4 Build pipeline

| Capability | Status |
|---|---|
| In-process content generation via Loom | shipped (T70) |
| 20+ audit phases (a11y, contrast, csp, sri, seo, perf…) | shipped |
| Merkle-chained build reports | shipped (T26) |
| Ed25519-signed reports | shipped (T56) |
| Trust-anchor-required signature verification | shipped (T47c v2 in Loom) |
| Secret-leak pre-commit gate | shipped (T56b) |
| `forge audit mutants` (cargo-mutants integration) | shipped (T58) |
| Visual-regression diffing across themes/breakpoints | queued (T33) |
| Inotify-driven re-run on edit | queued (no T# yet) |
| Type-state phase pipeline (compile-time order) | queued (T24) |
| TLA+ spec for invariants | queued (T27) |
| Dynamic frontend mode | queued (T12) |

### 4.5 Deploy

| Capability | Status |
|---|---|
| Local atomic deploy (symlink swap) | shipped (T47) |
| Content-addressed bundle dirs (`publish-<sha>`) | shipped |
| Rollback (single-command flip) | shipped |
| Ed25519-signed manifests | shipped (T47c) |
| Bundle pubkey deposit for cross-verification | shipped |
| SSH/rsync transport for remote deploys | queued (T47b) |
| Hetzner / cloud-storage transport plugins | concept |
| Multi-region propagation | concept |
| `loom attest export` (QR + fingerprint sharing) | queued (T47e) |

### 4.6 Auth

| Capability | Status |
|---|---|
| Argon2id passwords + HMAC-SHA256 cookies | shipped (T43) |
| `SameSite=Strict` + `HttpOnly` + `Secure` cookies | shipped |
| Constant-time secret comparison via `subtle` | shipped |
| WebAuthn / passkey login | queued (T43d) |
| API-key auth for agent integrations | concept |
| Multi-tenant isolation | queued (T45) |
| Sandboxed per-tenant Claude SSH bridge | queued (T46) |

### 4.7 Privacy + opsec

| Capability | Status |
|---|---|
| Image metadata strip | shipped + queued (7b GIF/WebP) |
| Error scrubbing (no PII / paths leaked) | shipped (per error site) |
| Secrets never committed (gitignore + `forge audit secrets`) | shipped (T56b) |
| Reproducible builds | shipped (content-addressed) |
| TLS 1.3 only for outbound | doctrine; concept |
| Tor / I2P / onion-service deploy target | concept |
| At-rest secret encryption with separate key | doctrine; partial |

### 4.8 Developer ergonomics

| Capability | Status |
|---|---|
| `loom new`, `loom site init`, `loom edit-serve`, `loom deploy` | shipped |
| `loom lint`, `loom audit`, `loom cms-render`, `loom import` | shipped |
| `forge build`, `forge verify`, `forge audit secrets/mutants` | shipped |
| Inline annotation grammar (`AVP-PASS-N:` etc.) | shipped (doctrine) |
| `forge-watch` — inotify re-run | queued |
| `forge serve` — local preview server | partial (forge-serve crate scaffolded) |
| `forge replay` — trend + churn + slow-URL analysis over build-report history | ✅ shipped (`forge-replay` binary; `--last N` for trend table; findings churn vs previous; slow-URL hotspots from forge-serve log) |
| Cross-repo CROSSFIX commits | doctrine — happens organically |

### 4.9 Documentation

| Capability | Status |
|---|---|
| `docs/USAGE.md` (Loom — Mom-friendly walkthrough) | shipped |
| `docs/DESIGN.md` (Loom — design rationale) | shipped |
| `docs/FORGE_VISION.md` (this doc) | shipped (T71) |
| ISO standards adoption doc | queued (T69) |
| Per-phase `--help` with full doctrine | partial |
| In-GUI tutorial (Loom editor) | shipped (T64) |
| Interactive query-string tour mode | ✅ shipped (T64b) — `?tour=N` URL param activates step-by-step in-context overlay in `loom edit-serve`; `parse_tour_query()` validates step range; tests cover valid + out-of-range + garbage inputs |
| Architecture decision records (ADRs) | concept |

## 5. Architecture (when fully built)

```
┌─────────────────────────────────────────────────────────────┐
│                  PlausiDen ecosystem                         │
│                                                              │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │   Loom       │    │   Forge      │    │   Crawler    │  │
│  │              │    │              │    │              │  │
│  │  - typed CMS │───▶│ - render     │◀───│ - audit      │  │
│  │  - editor    │    │ - audit      │    │ - findings   │  │
│  │  - components│    │ - sign       │    │              │  │
│  │  - tokens    │    │ - deploy     │    │              │  │
│  └──────────────┘    └──────────────┘    └──────────────┘  │
│         │                   │                                │
│         └───────────┬───────┘                                │
│                     ▼                                        │
│            ┌──────────────────┐                              │
│            │  Site bundle     │                              │
│            │  (signed)        │                              │
│            │  publish-<sha>/  │                              │
│            └──────────────────┘                              │
└─────────────────────────────────────────────────────────────┘
        │
        ▼ atomic symlink swap
┌──────────────────┐
│  /var/www/<site> │
│  current ──────▶ publish-<sha>
└──────────────────┘
```

Per-tenant view (the multi-tenant future):

```
┌────────── tenant A ──────────┐  ┌────────── tenant B ──────────┐
│  cms-A/                       │  │  cms-B/                       │
│  static-A/                    │  │  static-B/                    │
│  auth-A/  (per-tenant secret) │  │  auth-B/                      │
│  sandbox-A/  (claude ssh)     │  │  sandbox-B/                   │
└──────────────────────────────┘  └──────────────────────────────┘
            │                                  │
            └──────────────┬───────────────────┘
                           ▼
                ┌──────────────────────┐
                │   Loom + Forge       │
                │   (shared binaries,  │
                │    isolated state)   │
                └──────────────────────┘
```

Six-tool federation view (full vision):

```
                    ┌──────────────────┐
                    │   Oxidizer       │  meta-conformance gate
                    │   ecosystem-wide │
                    │   doctrine audit │
                    └────────┬─────────┘
                             │ phase_oxidizer_conformance
                             ▼
   ┌─────────────────────────────────────────────────────────┐
   │                                                          │
   │   ┌────────┐    ┌────────┐    ┌────────┐    ┌────────┐ │
   │   │  Loom  │───▶│  CMS   │───▶│ Forge  │───▶│Crawler │ │
   │   │ typed  │    │multi-  │    │ build  │    │runtime │ │
   │   │ render │    │tenant  │    │ + sign │    │ audit  │ │
   │   │+ editor│    │+ audit │    │+ deploy│    │+ recon │ │
   │   └────────┘    └────────┘    └────────┘    └────────┘ │
   │       ▲                            │              │     │
   │       │                            ▼              ▼     │
   │       │                    ┌──────────────────────────┐ │
   │       │                    │ Annotator                │ │
   │       └────────────────────┤ human-review session JSON│ │
   │                            └──────────────────────────┘ │
   │                                                          │
   └──────────────────────────────────────────────────────────┘
                                │
                                ▼ feedback loop closes
                       ┌──────────────────┐
                       │  Agent / human   │
                       │  iteration       │
                       └──────────────────┘
```

## 5b. What Forge can do when each sister vision lands

Sibling capabilities Forge inherits as they ship — each unlocks
new phases or first-class capabilities IN Forge.

### When **Loom** ships its full vision (LOOM_VISION.md)

| Sibling capability | New Forge ability |
|---|---|
| Type-state Section landmarks (Loom T36) | `phase_landmark_compile_check` — refuses any CmsPage that statically violates landmark rules; promotes runtime to compile-time |
| `loom-audit` visual-regression crate (parity with Forge T33) | Runtime + build-time visual diff cross-check — Forge can flag drift between Loom-rendered preview and Forge-built output |
| Multi-tenant per-tenant workspace (Loom T45) | `forge build --tenant N` — per-tenant build isolation under the same binary |
| Claude Code SSH bridge (Loom T46) | Forge phases can spawn a sandboxed agent inside the build to auto-fix findings before the report finalizes |
| WebAuthn passkey auth (Loom T43d) | Forge build reports get hardware-key-signed by the human operator at attest time |
| Loom-as-PWA + CRDT collab | Forge `phase_collab_drift` checks N concurrent editors haven't produced divergent CmsPage states |
| Voice-to-CMS / on-device LLM | Forge phase consuming the LLM-suggestion provenance for transparency-log inclusion |

### When **CMS** ships its full vision (CMS_VISION.md)

| Sibling capability | New Forge ability |
|---|---|
| Multi-site SQLite + Postgres adapter | `forge build --site <name>` consumes typed `Page` direct from CMS storage adapter |
| Per-site workflow (draft → review → schedule → publish) | `phase_pre_publish_audit` runs at workflow stage, blocks publish on strict findings |
| Per-tenant capability tokens | Forge runs scoped to one tenant's content, can't accidentally cross-build |
| Append-only signed audit log | Every Forge build event lands as a CMS audit-log entry signed by the build operator |
| Time-locked publish | Forge generates the time-locked envelope; CMS executes when the timer fires |
| C2PA content provenance | Forge embeds the C2PA manifest in every rendered image at build time |
| Webhook outbound on publish | Forge build-completion fires the configured webhook with the signed report |
| Tor onion-service publish target | Forge `phase_onion_deploy` writes the bundle to a per-site `.onion` mirror |

### When **Crawler** ships its full vision (CRAWLER_VISION.md)

| Sibling capability | New Forge ability |
|---|---|
| Cross-browser matrix (Chromium + Firefox + Safari TP) | `phase_crawl_cross_browser` — every deploy verified across all three before signing |
| Cross-device matrix (mobile + tablet + desktop) | `phase_crawl_responsive` — verifies every page across every viewport |
| Pixel-hash visual diff vs baseline | `phase_visual_drift` first-class (replaces the current Forge T33 stub) |
| `crawler auto-record` from human clicks | Forge auto-generates per-tenant journeys from operator interaction history |
| `crawler shrink-finding` | Forge `--explain` mode bisects journey to minimal repro for any strict finding |
| `crawler-replay` (network-log replay) | Forge re-runs the audit against a captured network log offline |
| OSINT mode (`PlausiDen-Recon` fork after Crawler T73) | `phase_competitor_audit` — compare Forge-built output vs reference site, flag missing capabilities |

### When **Annotator** ships its full vision (ANNOTATOR_VISION.md)

| Sibling capability | New Forge ability |
|---|---|
| Crawler `annotate` step kind | `phase_annotation_review` — every flagged element becomes a typed Forge Finding |
| Rust `annotator-session` crate | Forge consumes Session JSON natively, no manual parsing |
| `annotator-replay` (agent walks flagged elements) | Forge phase can reject a build if agent-proposed fixes don't resolve human-flagged elements |
| Hardware-key signed comments | Forge surfaces commenter identity in the build report |
| Multi-agent review consensus | Forge ranks findings by N-agent consensus weight |
| Diff renderer (two sessions of same page) | Forge `phase_review_drift` — flags pages where two reviewers disagreed |

### When **Oxidizer** ships its full vision (OXIDIZER_VISION.md)

| Sibling capability | New Forge ability |
|---|---|
| `check_rust_only` + Rust-purity catalog | `phase_oxidizer_conformance` blocks Forge build if the source repo violates Rust-first |
| Supersociety-stack baseline checks | Build report carries Oxidizer conformance score per dep |
| Auto-fix engine | Forge `--fix` mode applies Oxidizer's fixes pre-build |
| Per-fork conformance baseline | Forge respects per-fork waivers when computing severity |
| Cross-repo conformance graph | Forge build-time link-check verifies every cross-repo dep is on a conformant version |
| Hardware-attested Oxidizer runs | Forge build report inherits hardware-attestation chain |
| Cross-Oxidizer federation (peer cross-signing) | Forge build reports gain federated trust; agents can verify deploys against multiple peer Oxidizers |

## 5c. Background-infrastructure adjacencies (the other 9)

The 5 sister tools above are immediate user-facing functionality.
The PlausiDen ecosystem also has 9+ background-infrastructure
repos that Forge does NOT depend on today but reasonably could,
and probably should as each matures. Listed for completeness so
future planners know the adjacency exists — none of these are
in scope for current sprints, all are queued at "concept" tier.

| Repo | What it is | When Forge would integrate |
|---|---|---|
| **PlausiDen-AVP-Doctrine** | The validation protocol every PlausiDen artifact is graded against — standing orders, gates, annotations, FOSS-absorption protocol, cross-repo contribution protocol, ship-decision rules. The doctrine repo is the source of truth for what Forge's audit phases enforce. | Forge `phase_doctrine_conformance` reads doctrine TOMLs from this repo via path/git dep, generates audit phases procedurally so a doctrine update in PlausiDen-AVP-Doctrine auto-rolls into every Forge build. Today: doctrine is duplicated in `~/.claude/CLAUDE.md` + scattered through Forge phase docstrings. |
| **PlausiDen-Audits** | TOOL_REGISTRY of every external tool considered for absorption (cargo-audit, cargo-deny, cargo-mutants, axe-core, …) with verdicts (`adopted` / `adopted-as-dep` / `deferred` / `reference-only` / `rejected`). Same shape as PlausiDen-Crawler's CRAWLER_REGISTRY. | Forge phases that wrap external tools record their choice rationale here. `forge audit registry` cross-checks every wrapped tool against the catalog. Re-evaluation gate: agents can't re-evaluate a `rejected` tool without new evidence + signed waiver. |
| **PlausiDen-Canon** | Tier-1 canonical invariant substrate — tokens, primitives, components, contracts that every UI surface conforms to. Five-layer model (Tokens / Primitives / Components / Compositions / Patterns). Sibling to Loom but at a higher abstraction (Canon = ecosystem-wide; Loom = render layer for Canon-conformant content). | Forge `phase_canon_conformance` checks every rendered output uses Canon-blessed tokens / primitives. Loom-rendered content gets free Canon conformance because Loom-tokens is Canon-derived; hand-written HTML in `static/` gets audited against Canon directly. |
| **PlausiDen-Tests** | Generic testing-framework + test-harness substrate. Bidirectional flow: patterns flow Generic↔Specific between Testing-Framework and project test suites. | Forge phases that need property-test infrastructure consume the test-harness; `forge audit tests` checks every Forge phase has a test-to-public-fn ratio ≥ 4 per AVP-2 doctrine. |
| **PlausiDen-Obs** | Observability substrate — structured tracing, signed audit-event format, doctrine-guarding tests pinning the schema. | Forge's `tracing` output emits Obs-compatible structured events; build reports land in the Obs event stream signed with the build operator's key. |
| **PlausiDen-Meta** | Cross-repo coordination + priority gate (PRIORITY.md). Tier-promotion rules (build-ahead-of-trigger vs wait-for-trigger). Governance for the whole PlausiDen ecosystem. | Forge `phase_priority_check` enforces that no work proceeds on a Tier-2 repo if a Tier-1 dependency is still missing. Forge build-graph metadata lands in Meta for cross-repo planning. |
| **PlausiDen-Sentinel** | Live-system runtime sentinel (Kali-workstation hardening, intrusion detection, …). | Forge can register build-success / deploy events with Sentinel for runtime cross-correlation. Sentinel can trigger Forge re-builds on monitored events (signed-cert rotation, dep-CVE notification). |
| **PlausiDen-Harvest** | Harvest candidate evaluation — when Forge phases produce findings that suggest a new component / pattern / tool worth absorbing, the candidate goes through Harvest's protocol. | Forge findings of class `harvest_candidate` route to Harvest automatically; Harvest's verdict feeds back as a Forge `SHIP-DECISION` waiver if rejected. |
| **sacredvote-crypto** | Post-quantum-forward primitives (ML-KEM / ML-DSA). Source-of-truth for any PlausiDen crypto crate that needs PQ-readiness. | Forge gains dual-sign manifests (Ed25519 + ML-DSA) when this crate stabilises. The Sacred.Vote-class technical-client tier gets cryptographic forward-secrecy by default. |

These adjacencies are real but lower-priority than the immediate
5-tool federation. They show up in Forge's roadmap once the
five-tool integration loop closes and the federation is stable
enough to absorb meta-layer dependencies without re-litigating
every architectural decision.

## 5d. Ecosystem dependency topology (consolidated)

Visions for every load-bearing repo are now in `docs/<NAME>_VISION.md`
under each repo. This is the consolidated edge list across the
14 PlausiDen-* repos that touch Forge's transitive dep cone.

```
                    ┌──────────────────┐
                    │  PlausiDen-Meta  │  Tier-0 — root governance
                    │  (constitution)  │  (consumes nothing)
                    └────────┬─────────┘
                             │ advisory
                             ▼
   ┌────────────────────────────────────────────────────┐
   │                                                     │
   ▼                                                     ▼
┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│ AVP-Doctrine     │    │ Canon            │    │ Harvest          │
│ Tier-1           │    │ Tier-1           │    │ Tier-3 root      │
│ (constitution)   │    │ (5-layer subst.) │    │ (protocol)       │
│ consumes: nothing│    │ consumes: nothing│    │ consumes: nothing│
└────────┬─────────┘    └────────┬─────────┘    └────────┬─────────┘
         │                       │                        │
         ├───────────┬───────────┼────────────┬──────────┘
         │           │           │            │
         ▼           ▼           ▼            ▼
   ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐
   │ Audits  │ │ Tests   │ │ Obs     │ │ Oxidizer│
   │ Tier-1  │ │ Tier-1  │ │ Tier-0  │ │ NEW     │
   └─────────┘ └─────────┘ └─────────┘ └─────────┘
        │           │           │           │
        └───────────┴───────────┴───────────┘
                          │
                          ▼ enforced by
                   ┌─────────────┐
                   │   Loom      │  ◀── (loom-cms-render)
                   │   CMS       │      Cargo path-dep
                   │   Forge     │  ◀── this repo
                   │   Crawler   │
                   │   Annotator │
                   │   Sentinel  │
                   │   Engine    │
                   └─────────────┘
```

### Direct deps Forge has TODAY (Cargo-level)

| Dep | Type | Status |
|---|---|---|
| `loom-cms-render` | path dep | shipped (T70) |
| `forge-core` (in-workspace) | path dep | shipped |
| `forge-phases` (in-workspace) | path dep | shipped |
| External: serde, clap, toml, time, tracing, sha2, base64, ed25519-dalek, thiserror, anyhow, proptest | crates.io | shipped |

### Conceptual integration points (the 5-tool federation, queued)

| Dep | Integration | Status |
|---|---|---|
| Loom (`loom-cms-render`) | `phase_render` calls `page_shell` in-process | ✅ shipped |
| CMS | `phase_pre_publish_audit` consumes `cms-core::Page` | concept |
| Crawler | `phase_crawl` invokes Crawler runtime audit | partial (subprocess) |
| Annotator | `phase_annotation_review` consumes `annotator-session` JSON | ✅ shipped 2026-05-17 |
| Oxidizer | `phase_oxidizer_conformance` calls `oxidizer-cli` / `oxidizer-core` | concept |

### Background-infrastructure adjacencies (the meta-layer, concept)

| Dep | Integration | Vision |
|---|---|---|
| AVP-Doctrine | doctrine TOMLs feed phase generation | `PlausiDen-AVP-Doctrine/docs/AVP_DOCTRINE_VISION.md` |
| Audits | `phase_audits_catalog_check` consumes audit.toml registry | `PlausiDen-Audits/docs/AUDITS_VISION.md` |
| Canon | `phase_canon_conformance` against 5-layer substrate | `PlausiDen-Canon/docs/CANON_VISION.md` |
| Tests | property + mutation + fuzz harnesses | `PlausiDen-Tests/docs/TESTS_VISION.md` |
| Obs | structured-tracing emission + signed audit events | `PlausiDen-Obs/docs/OBS_VISION.md` |
| Meta | `phase_priority_check` enforces tier-promotion | `PlausiDen-Meta/docs/META_VISION.md` |
| Sentinel | runtime defence on the host Forge runs on | `PlausiDen-Sentinel/docs/SENTINEL_VISION.md` |
| Harvest | `harvest.toml` for upstream-doctrine candidates | `PlausiDen-Harvest/docs/HARVEST_VISION.md` |
| sacredvote-crypto | post-quantum dual-sign manifests (ML-DSA) | external — vision deferred |

### Engine + adjacent (orthogonal — Forge does NOT consume Engine directly)

Engine is the synthetic-data generation library powering
PlausiDen's plausible-deniability mission. Forge is a build
tool — Engine is a runtime concern. They share the AVP-2
substrate via Tests + Obs but don't directly cross at the
Cargo level.

### Total ecosystem awareness for Forge

- **1 Cargo-level dep** (loom-cms-render) — the only hard edge today
- **5 user-facing siblings** — vision-doc-tracked integrations
- **9 meta-layer adjacencies** — all visioned now (AVP-Doctrine, Audits, Canon, Tests, Obs, Meta, Sentinel, Harvest, Engine)
- **= 15 PlausiDen-* repos** Forge transitively touches when fully built

Each of those 15 has its own `docs/<NAME>_VISION.md` companion
to this Forge vision. The federation discipline lets each repo
evolve independently; the typed contracts + shared doctrine
keep them composable.

## 5b. What Forge becomes when all of this is true

This section describes the platform Forge **becomes** once the
substrate from
[`ARCHITECTURE_PRINCIPLES.md`](./ARCHITECTURE_PRINCIPLES.md), the
operations layer from
[`SITE_OPERATIONS.md`](./SITE_OPERATIONS.md), and the engineering
disciplines from
[`ENGINEERING_DISCIPLINES.md`](./ENGINEERING_DISCIPLINES.md) are
all in place. The capabilities below are **emergent from
substrate composition**, not features that need to be specified
independently. Each falls out for free once the substrate is
honest.

### 5b.1 The unifying principle that makes everything composable

> Identify the implicit assumption that produces the failure
> mode. Make it explicit and structural. Derive everything else
> from the boundary.

Every capability below is a corollary of this single principle.
Listing them is enumerating consequences, not adding features.

### 5b.2 What the capability manifest unlocks

Once the manifest is the source of truth and every projection
(backend handlers, frontend client, UI affordances, AI tools,
docs, contract tests, telemetry, permissions, audit hooks,
billing meters) is generated from it:

- **Drift between frontend and backend is structurally
  impossible.** Orphan UI (button that calls nothing) and orphan
  capability (endpoint with no UI surface) both fail the build.
- **Adding a new capability is mechanical.** Declare it in the
  manifest; implement the handler. Everything else is generated.
  ~30 lines of code, ~5 minutes of human review for the
  permission scope and AI-callability flag.
- **AI's tool list cannot drift from reality.** The model
  cannot hallucinate a capability the platform doesn't have.
  The platform cannot ship a capability the AI doesn't know
  about.
- **Documentation cannot rot.** API reference, SDK reference,
  CLI reference, AI tool reference, contract test suite — all
  generated. The gap between "code that exists" and "code in the
  generated docs" is zero by construction.
- **Permissions cannot drift.** Hand-written
  `if (user.role === 'admin')` checks don't exist; the manifest
  is the only place permissions are expressed.
- **Plugins inherit the same discipline.** Plugin manifest
  extends the same IDL. Plugin can't ship a backend endpoint
  with no UI counterpart or vice versa for the same structural
  reason core can't.

### 5b.3 What the primitive system unlocks

Once primitives are typed components with overflow + a11y +
performance contracts, and the token system has discrete axes
(density, formality, motion, texture, type personality, grid
character, color mood, color energy):

- **AI-generated sites are correct by construction.** The model
  emits a site spec; the renderer turns spec → HTML. The
  renderer is the same for AI and human paths. The model cannot
  produce broken layouts because the primitive system makes
  broken layouts unreachable.
- **Layout failures become impossible classes, not bug
  categories.** Overflow, overlap, broken responsiveness,
  contrast violations, missing focus indicators — none of these
  are bug categories Forge ships with, because the primitive
  system makes them structurally absent.
- **Visual range is genuinely large without slop.** 200 primitives,
  8+ aesthetic dimensions as discrete enums, dozens of curated
  style packs — tens of thousands of valid combinations, every
  one of them coherent and guaranteed-correct.
- **Cross-browser / cross-device / cross-locale testing
  collapses to primitive testing.** A small kernel of
  primitives means a small surface. Property-based testing per
  primitive against adversarial content + every breakpoint +
  RTL + every theme catches issues no human QA would find.

### 5b.4 What the multi-network publishing capability unlocks

Once Tor / I2P / Lokinet / IPFS / Gemini are typed deployment
targets with their own constraint sets:

- **A site can declare its network reach as a property,** not a
  separate deployment story. `[networks] clearnet=true, tor=true,
  i2p=optional` in `forge.toml` and the build pipeline produces
  appropriately-constrained bundles per target.
- **The primitive system enforces no-clearnet-leakage on
  Tor-mode sites at the primitive layer.** External fonts,
  scripts, embeds, fingerprintable resources — denied at the
  primitive level, not as an afterthought CSP rule.
- **The security rating dashboard is a typed projection of the
  manifest + primitive analysis.** No separate "is this site
  safe" subsystem; the answer falls out of capability
  composition.
- **Plausible deniability integrates** rather than reimplements.
  Forge is the publishing surface; PlausiDen-Engine handles the
  obfuscation layer. The seams are typed.

### 5b.5 What the dual mainstream/sovereign architecture unlocks

Once the values configuration panel lets users mix per-dimension
(hosting, analytics, auth, AI, embeds, payments, email):

- **Forge serves multiple audiences with one substrate.** The
  small bakery and the dissident publisher both get a CMS that
  fits their threat model, without two product lines.
- **Substrate discipline benefits the mainstream case too.** A
  CMS that *could* run a Tor site safely produces a CMS where
  every mainstream site has better default security, cleaner
  extension model, less performance bloat, more honest defaults.
- **The values declaration is an audit-grade artifact.** Per-
  dimension declared choices become the basis for the
  Privacy Policy, the Sub-processor list, the Accessibility
  Statement, the trust center. Practice and policy stay bound
  at the source.

### 5b.6 What the AI-as-bounded-search architecture unlocks

Once AI generates site specs constrained by the schema, composes
via typed primitive tools, runs through staged pipelines with
critics, and operates under per-tenant cost + safety budgets:

- **An AI agent can spawn a fresh tenant, build a complete
  accessible site from a brief, fix every fixable finding, and
  deliver a signed bundle, in under 5 minutes end-to-end.**
  (Acceptance criterion §7.3.)
- **AI output passes the same gates as human output.** No
  separate quality story for AI-generated content. The gates
  don't care what produced the input.
- **AI behavior is bounded by the capability manifest.** The
  AI's tools are exactly the manifest's `ai-callable` capabilities.
  Prompt injection attempts that try to escape the grammar are
  rejected at the structural layer, not the prompt layer.
- **The corpus + critic loop compounds over time.** Sites that
  perform well in production feed back into the corpus as
  positive examples; bounces and drop-offs feed back as
  negative. The system improves at its own job.

### 5b.7 What the operational layer unlocks

Once required pages, site-success automation, contextual
reminders, and template freshness all flow from declared site
type + jurisdiction:

- **Mom never sees a stack trace.** (Acceptance criterion §7.1.)
- **First-publish-to-customer-visible-search-listing is days,
  not months.** Forge handles Search Console submission,
  business directory listings, social profile claiming, DNS
  hygiene, email authentication automatically.
- **Compliance documents reflect reality.** Cookie Policy
  generated from actually-running scripts. Privacy Policy
  references PII-tagged schema fields. Sub-processor list
  reflects actual integrations. Drift between practice and
  policy is structurally impossible.
- **Legal regression has a CI gate.** EU regulation drops →
  cookie banner template updates → every site sees a "recommended
  update" notification with diff and one-click apply. Sites stay
  legal as the law changes, without operator effort.

### 5b.8 What the engineering disciplines unlock

Once caching uses surrogate keys, jobs have priority lanes,
webhooks have exactly-once-with-receipt, state machines are
typed, idempotency keys are mandatory, secrets are rotated, and
incident response is drilled:

- **Failure modes that hit other platforms during scaling don't
  hit Forge.** Cache stampedes, retry storms, notification loops,
  thundering herds — patterns from *Release It!* applied
  deliberately rather than discovered painfully.
- **The system can be reasoned about.** Transaction isolation
  is explicit per operation. Time is HLC-ordered. Background
  jobs have observability per job. State transitions are typed.
- **Database migrations are safe by default.** Expand-contract
  pattern is the only way to ALTER. The CI linter rejects
  unsafe single-shot migrations on hot tables.
- **Security incidents have a known response.** Forensic logs
  in immutable storage. Tabletop drills quarterly. The first
  serious incident isn't a learning experience because the
  lessons were already learned in the drill.

### 5b.9 The continuous-gradient stance

> World-class isn't a state — it's a gradient maintained against
> entropy.

Every quality dimension (security, UI, UX, SEO, a11y,
performance, audit) has the same structural recipe: explicit
measurable criteria, automated measurement on every change,
manual audit on cadence, adversarial testing by parties with
incentive to find failures, public accountability, protected
capacity.

The substrate above makes this measurement + enforcement
infrastructure **cheap**. On unbounded WordPress-style extension,
every gate is impossible to enforce because the system has no
internal model of what it should be doing. On the substrate
Forge designs, every gate is **mechanical** because the system
knows its own contract.

### 5b.10 The honest revisions

[`ENGINEERING_DISCIPLINES.md §22`](./ENGINEERING_DISCIPLINES.md)
captures nuance the earlier doctrine understated:

- **Capability manifest needs explicit ownership** — manifests
  rot without it. Closer to language-standards-committee work
  than typical engineering.
- **World-class costs continuous resources** — visual regression,
  pen tests, bug bounties, localization, a11y user testing all
  have ongoing budget implications. Treating "world-class" as
  achievable without explicit budget is the failure mode that
  turns architecture documents into wishes.
- **Mainstream vs sovereign is per-dimension, not binary.** Most
  real deployments mix (Stripe for payments + Hetzner for
  hosting + Cloudflare for CDN + self-hosted analytics + OpenAI
  for AI + passkeys for auth). The values declaration is
  per-dimension; profile presets are starting points the user
  mixes from.
- **AI integration is pluggable, not hosted-model-coupled.** The
  capability manifest declares AI providers as pluggable; self-
  hosted fallback for critical paths means the platform survives
  provider disputes or outages.
- **The plugin sandbox is Wasm-based, not "solved."** WASI
  Preview 2 + component model are recent. Architectural
  commitment is right; implementation will hit edges that don't
  have great answers yet. Honest framing.

### 5b.11 The meta-observation

The architecture is coherent, ambitious, recognizably valuable.
Remaining gaps are increasingly specialized — "things experts in
narrow domains would add" rather than "things a complete picture
is missing."

The useful question at this point isn't *"what else?"* but
**"what now?"** The architecture has been thought through.
Building it requires choices about sequencing, team, funding,
audience, and which compromises to accept. Those are not
engineering questions; the answer to them is the next layer of
work.

The architecture is sound. The remaining questions are about
what to do with it.

---

## 6. Roadmap from now to "done"

### Sprint 1 — operationalise the directives (this week)

- [shipped] T48c v1 + v2 — dual-theme + a11y baseline in page-shell
- [shipped] T66 — `phase_dual_theme` parity gate
- [shipped] T67 — `phase_semantic_html` extended for `<div role>`
- [shipped] T70 — `phase_render` (Forge generates content in-process)
- [shipped] T71 — this doc
- [in-flight] T615 — GUI site/app builder (rolling)
- [shipped] T68 — `phase_theme_contrast` enumerates per-theme via
  `loom theme contrast`; current dogfood walks 12 themes,
  WCAG AA gate enforced
- [shipped] T48b — `loom site init --template portfolio` /
  `--template blog` (see `loom site templates` for the bundled
  list)
- [shipped] T64b — `?tour=N` query-string-driven step-by-step
  overlay in `loom edit-serve`; `parse_tour_query()` validates
  step range
- [shipped 2026-05-17] `phase_annotation_review` — closes the
  Annotator↔Forge integration; reads `[review] session_dir`
  from `forge.toml`, surfaces operator-flagged elements as
  typed `Finding`s with severity per tag

### Sprint 2 — close the supersociety stack

- T36 — type-state Section heading + landmark contracts
- T37 — zero-JS theme/density/font switcher (form-POST)
- T70b — move full a11y page-shell from `loom-cli` into
  `loom-cms-render` (Forge inherits without duplication)
- T70c — flip `static/` to canonical output (delete `_render/`)
- T54 — delete bash `forge.sh` (parity validated)
- T39 — `phase_loom_lint` integrated as Forge phase
- T40 — extend `loom-lint` for raw `ms`/`s` outside `:root`
- T38 — tokenize the 33 spacing literals T32 surfaced
- T34 — component state-matrix fixtures + crawler coverage
- T62 step 7b — GIF + WebP metadata strip
- T62 step 8 — image picker UI
- T62 step 10b — compound-field inline edit
- T47b — SSH/rsync transport for `loom deploy`
- T47e — `loom attest export` (QR + fingerprint)
- T43d — WebAuthn passkey auth
- T69 — ISO standards adoption doc

### Sprint 3 — multi-tenant + agent farm

- T45 — multi-tenant: per-tenant SQLite + workspace isolation
- T46 — Claude Code SSH bridge (sandboxed per-tenant agent)
- T44 — `phase_edit_loop` + auto-rebuild on save
- T12 — dynamic frontend mode (opt-in JS)
- T33 — `phase_visual_diff` (4 themes × 3 viewports)
- T27 — TLA+ spec for phase pipeline invariants
- T24 — type-state phase pipeline

### Sprint 4 — capabilities not yet ticketed

These are owner-implied or persona-derived:

- **Markdown / WordPress / Notion → CmsSection importers** (T63
  extensions) — the technical-client persona expects to migrate
  in, not start from scratch.
- **`forge-watch`** crate — inotify-driven re-run on edit.
- **`forge-html`** crate — `lol_html` wrapper for parser-needing
  phases.
- **`forge-css`** crate — `lightningcss` wrapper for CSS-touching
  phases.
- **`forge-report`** crate — JSON + terminal renderers separated
  from CLI for SaaS deployment.
- **`forge serve`** — local preview server with hot reload (the
  scaffolded `forge-serve` crate completed).
- **`forge replay`** — trend + churn + slow-URL triage over the
  build-report chain history. **Shipped** as the `forge-replay`
  binary (cargo run --release -p forge-replay -- --last 10).
  Read-only triage helper, not a gate.
- **Annotator integration** — Forge phase consumes Annotator
  session JSON, surfaces the human-flagged elements as findings.
- **API-key auth** for agentic + CI integrations.
- **Tor onion-service deploy target** — the privacy-maximal
  publish destination.
- **Cloud-storage / Hetzner / Cloudflare R2 transports** for
  `loom deploy`.
- **Reproducible-build attestation** — sigstore-style transparency
  log of every signed bundle.
- **Component state-matrix renderer** — every variant of every
  primitive rendered into an inspection grid (developer aid).
- **Visual regression budget per page** — the existing visual
  diff phase, gated by a per-page tolerance.
- **`forge fix`** — auto-fix every fixable finding (where the
  fix is unambiguous).
- **Cross-tenant search** with mTLS — the agent farm needs a
  way to coordinate.
- **`loom doctor`** — health-check command Mom can run when
  something feels off, surfaces the misconfiguration in plain
  English.

## 7. Acceptance criteria for "done"

Forge is **done** when:

1. Mom can build, edit, theme, audit, and publish a complete
   accessible site without ever seeing a stack trace.
2. A developer can fork the repo, add a new phase in <100 lines,
   and have it integrated into the build pipeline + the CSP +
   the a11y audit + the CI report.
3. A Claude Code agent can spawn a fresh tenant, populate it with
   pages from a specification, run the build, fix every
   fixable finding, and deliver a signed bundle, in <5 minutes
   end-to-end.
4. Every line of code carries an `AVP-PASS-N` annotation
   somewhere in its blame history.
5. `cargo mutants` survival rate is <5%.
6. Every phase has property-based tests with ≥10k cases.
7. The TLA+ model has been refined to the Rust code with a
   tool-checkable correspondence.
8. Public-facing sites pass WCAG 2.2 AAA in both light and dark.
9. Build outputs are bit-identical across machines (reproducible).
10. The threat model from `~/.claude/CLAUDE.md` (state-actor
    adversary, full breach, unlimited time) holds against the
    deployed system.

The verdict is always **STILL BROKEN** — shipping is risk
acceptance, not a declaration of correctness. The loop resumes
on the next commit.
