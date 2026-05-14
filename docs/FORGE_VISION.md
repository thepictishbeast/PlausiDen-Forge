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
   editor on first visit [in-flight T64b].
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
| Bundled portfolio + blog templates | queued (T48b) |
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
| Dual-theme contrast audit (both palettes) | queued (T68) |
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
| `forge replay` — replay a build report | partial (forge-replay scaffolded) |
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
| Interactive query-string tour mode | queued (T64b) |
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

## 6. Roadmap from now to "done"

### Sprint 1 — operationalise the directives (this week)

- [shipped] T48c v1 + v2 — dual-theme + a11y baseline in page-shell
- [shipped] T66 — `phase_dual_theme` parity gate
- [shipped] T67 — `phase_semantic_html` extended for `<div role>`
- [shipped] T70 — `phase_render` (Forge generates content in-process)
- [shipped] T71 — this doc
- [in-flight] T615 — GUI site/app builder (rolling)
- [queued] T68 — extend `phase_theme_contrast` to dual theme
- [queued] T48b — portfolio + blog bundled templates
- [queued] T64b — interactive query-string tour mode

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
- **`forge replay`** — replay a build report into a future
  audit run (the scaffolded `forge-replay` crate completed).
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
