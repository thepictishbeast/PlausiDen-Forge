# AGENTS.md — PlausiDen-Forge

Orientation for any AI agent (Claude or otherwise) working in this repository. Read **before** writing any code or running any script.

> Per [[tool-starvation-anti-pattern]] doctrine: the failure mode that wastes most time is reaching for generic tools (bash, grep, find, curl, hand-rolled scripts) when a platform tool already exists. Stop and check first.

> Cross-repo orientation: see [PLAUSIDEN_ECOSYSTEM.md](./PLAUSIDEN_ECOSYSTEM.md) for how this repo relates to PlausiDen-Loom / Crawler / Annotator / CMS / Canon / Meta / AVP-Doctrine / LFI / Forge-LFI.

> Tool surface for AI clients (Claude / Gemini / other MCP-capable agents): see [mcp/README.md](./mcp/README.md). Tool schemas at `mcp/tools/*.json`, manifest at `mcp/manifest.json`. Cross-AI consumable — no agent-specific extensions.

---

## RULE 0 — The substrate is the only path (LOAD-BEARING)

You are working with PlausiDen substrate. **You do not write code specific to a single site.** Site work = editing CMS content (typed TOML/JSON). Capability work = editing the substrate (Forge / Loom / *-core / CMS schema).

**Forbidden:** hand-authoring CSS, HTML, or site-specific JavaScript. Bash one-liners to manipulate Forge-managed state. "Just for this one case" shortcuts. Substituting libraries for the canonical defaults.

**If you find yourself wanting any of those — STOP.** That's a substrate gap. File a capability request, implement in the appropriate substrate repo, then exercise via CMS content. Do not work around the substrate. Per `[[substrate-only-path]]` doctrine.

**Canonical defaults (do NOT relitigate):**
- HTTP: **Axum**. Async runtime: **Tokio**.
- HTML emission: **Maud** OR Loom typed primitives.
- Database: **PostgreSQL via sqlx** with compile-time query verification.
- Serialization: **serde with `deny_unknown_fields`** at every input boundary.
- Crypto: lift from **PlausiDen-Engine** (erasure/duress/deadman); **Ed25519** signatures; **ML-DSA / ML-KEM** for PQ.
- CLI: **clap** with existing forge-cli + loom-bridge argument-pattern conventions.
- Errors: **anyhow at binary boundaries**, **thiserror within libraries**, `?` everywhere, no `unwrap`/`expect` outside tests.
- Property testing: **proptest** mandatory at input boundaries.
- Logging: **tracing** with structured fields, OTLP-compatible.
- AI invocation: through the **LFI critic trait abstraction** — AND LFI is opt-in augmentation, never load-bearing (per `[[deterministic-first-lfi-optional]]`).

Genuine emergency? Substrate-bypass workflow is heavyweight + visible (operator approval in writing + `// SUBSTRATE-BYPASS(issue-id): reason` comment + tracked backfill issue + appears in audit reports). Not a habit.

---

## RULE 1 — Look before you build (tool selection)

Per `[[tool-starvation-anti-pattern]]`: Claude has strong training-prior toward generic tools (bash/grep/find/curl) vs platform tools. Strong default wins unless pushed against.

Before reaching for bash/grep/find/curl, hand-rolled scripts, or general-purpose libs:

1. **`forge --help`** — see if the subcommand exists.
2. **Scan the "Tool inventory" section below** — every audit, validation, and pipeline step has a typed Forge subcommand. Use it.
3. **Check `crates/` for the typed surface** — every domain (privacy, commerce, federation, email, …) has a `*-core` crate with the canonical types. Don't redefine.
4. **Check `cms-schema.json`** for valid field names and variants. The schema is generated from Rust source; if the schema rejects your field, your field is wrong.
5. **If none of the above** — propose an extension via a separate PR, not by routing around the substrate.

---

## Tool inventory (use these — don't reinvent)

**Build + watch:**
- `forge build` — full audit pipeline. Phase order in `forge.toml`. Strict findings == 0 required.
- `forge build --mode hybrid|dynamic|static` — override build mode.
- `forge build --json-report path.json` — emit machine-readable findings alongside terminal output.
- `forge watch` — inotify-driven re-run on edit.

**Audit (out-of-pipeline, fast feedback):**
- `forge audit --phase <name>` — run one phase in isolation. Use this when iterating on a single concern, not the whole pipeline.
- `forge audit secrets` — credential leak scan (gitleaks-equivalent, respects `.gitignore`). **Use this instead of `gitleaks` or hand-rolled grep patterns.**
- `forge audit phantom_button` — `data-backend=` references not declared in `backends.toml`. **Use this instead of `grep -r data-backend static/`.**
- `forge audit external_assets` — third-party fetches. **Use this instead of `grep src=https`.**

**Verify + attest (cryptographic integrity):**
- `forge verify` — walks `reports/build-*.json`, asserts Merkle chain integrity.
- `forge attest init` — generate Ed25519 attestation key.
- `forge attest sign` — sign a build report.

**Config gates (typed surface validation):**
- `forge manifest` — phases.toml + backends.toml consistency (kebab-case, acyclic, capability resolution).
- `forge privacy` — privacy.toml: every DataCategory has retention, no duplicates, days > 0.
- `forge trust-safety` — trust-safety.toml: mandatory-report ConcernKinds have scanners.
- `forge domains` — domains.toml: RFC 1035 FQDN, RFC 8555 DNS-01 for wildcards, HSTS preload-eligibility.
- `forge audit-log <path.json>` — hash-chained AuditChain verifier (monotonic, prev_hash linkage, tamper detection).
- `forge forms` — forms.toml: webhook https, WCAG labels, kebab-case unique ids, ≤1 honeypot per form.
- `forge federation` — federation.toml: typed protocol↔address consistency (no Nostr-to-ActivityPub mismatches).
- `forge email` — email.toml: marketing messages require RFC 8058 list-unsubscribe URL.
- `forge commerce` — commerce.toml: ISO 4217 currencies, non-negative prices, non-empty SKUs.
- `forge memberships` — memberships.toml: tier id kebab-case, monthly_price ≥ 0, ISO 4217 currency.
- `forge config` — run every config gate at once.

**Content + asset workflows:**
- `forge content validate` — CmsSection contract checks (importers-core + exporters-core).
- `forge content format-list` — list available importer/exporter formats.
- `forge content project-to-export <fmt>` — typed projection from CMS to export format.
- `forge search <path.json>` — IndexDoc[] validation before pushing to Tantivy/Meilisearch.
- `forge assets <bundle>` — AVIF/WebP/JPEG ladder + WCAG 2.1 §1.1.1 alt-text validation.

**Fix:**
- `forge fix` — auto-apply mechanical fixes from latest build report.

**Loom (sister repo, drives this one's CSS + primitives):**
- `loom site init --template <kind>` — scaffold buildable site.
- `loom edit serve` — admin CMS editor (cookie auth).
- `loom validate` — CMS JSON typed against CmsPage schema.
- `loom deploy hetzner` — atomic remote deploy.
- `loom sync --regenerate` — regenerate skin.css from token changes.

**Crawler (also sister; runtime audit + visual regression):**
- `crawler --journey <file>` — chromiumoxide journey runner. Use for screenshots at viewports, runtime-DOM detector axes, visual diff.

---

## Anti-patterns — do NOT do these

These are real failure modes from past Claude sessions:

- ❌ `grep -r 'data-backend' static/` → use `forge audit phantom_button`.
- ❌ `find . -name '*.json' -path '*cms*'` → cms files are at `cms/*.json`; if you need to enumerate them, Forge's loader already does it.
- ❌ `gitleaks detect` → use `forge audit secrets`.
- ❌ `curl https://prosperityclub.com/` to fetch a site → use Crawler journey with `goto` + `screenshot` steps. Crawler captures runtime state (post-JS), respects viewport, lands in `runs/`.
- ❌ Hand-rolled bash `for f in *' (1).*'; do mv …` → if you're scripting file manipulation as part of Forge work, check whether there's a typed import phase first.
- ❌ Writing raw CSS in a site repo → all styles flow through `loom-tokens` (skin.css) + `loom-components`. Site-specific CSS doesn't go in sites; primitives + variants get added to Loom.
- ❌ Editing `static/loom-skin.css` directly → it's a build artifact regenerated by `forge build` from `loom_tokens::SKIN_CSS`. Edits go in `PlausiDen-Loom/loom-tokens/src/skin.css`. (Note: until task #144 is closed, this requires commit-push-repin cycle; see task for path-dep override workaround.)
- ❌ `cargo run -p forge-cli -- build` from a non-Forge cwd → `forge` always uses `--root` defaulting to CWD. Run from the site root.
- ❌ Bypassing strict findings with `mode = "production"` flips OFF → fix the finding instead. The gate exists for a reason.
- ❌ Direct push to `main` on LFI repos (PlausiDen-LFI, Forge-LFI) — PR-only flow per memory.

---

## Crate map

| Crate | Owns |
|-------|------|
| `forge-core` | Types: `Phase` trait, `Finding`, `Severity`, `BuildCtx`, `BuildReport`, `BuildError`. Zero I/O, pure types. |
| `forge-phases` | Every concrete phase impl. One module per phase (tokens, html_semantic, csp, seo, perf_budget, sri, loom_sync, render, …). |
| `forge-cli` | The `forge` binary. argv parsing, `forge.toml` loading, pipeline orchestration. |
| `manifest-core` | Typed projection of phases.toml + backends.toml. T33 gate. |
| `privacy-core` | RetentionPolicy + DataCategory + LawfulBasis. T91. |
| `trust-safety-core` | ConcernKind enum (CSAM/NCIII/Extremism + non-mandatory). T91. |
| `domains-core` | Domain FQDN + AcmeChallenge + HstsPolicy. T86. |
| `forms-core` | Form::validate with WCAG + honeypot rules. T81. |
| `federation-core` | FederationProtocol + FederationAddress typed-enum pairs. T79. |
| `email-core` | OutgoingMessage with RFC 8058 list-unsubscribe rules. T83. |
| `commerce-storefront-core` | Product::validate (ISO 4217, prices, SKUs). T84. |
| `memberships-core` | Tier::validate (kebab-case, currency, price). T85. |
| `observability-core` | AuditChain hash-chain verifier. T91. |
| `importers-core` / `exporters-core` | Content lifecycle. T77+T78. |
| `assets-core` | Image bundle ladder + WCAG alt-text. T80. |
| `search-core` | IndexDoc[] validation. T82. |
| `forge-critic` | Critic-trait seam for AI-graded findings. |

External dependencies (git, see Loom→Forge dev loop in task #144):
- `loom-tokens`, `loom-cms-render`, `loom-lint`, `loom-components` from `github.com/thepictishbeast/PlausiDen-Loom`.

---

## Doctrine references

- [DESIGN.md](./DESIGN.md) — architectural reasoning.
- [FORGE_ROADMAP.md](./FORGE_ROADMAP.md) — what's planned, what's experimental, what's stable.
- [DEPRECATION.md](./DEPRECATION.md) — sunset schedule.
- [SECURITY.md](./SECURITY.md) — security model.
- [docs/](./docs/) — companion docs (vision, architecture principles, site operations, engineering disciplines, commercialization, platform roadmap).
- AVP-2 doctrine — out-of-tree; see PlausiDen-AVP-Doctrine repo for the 36-pass validation rules.

---

## First steps when starting work in this repo

1. **`forge orient`** — single-command session brief: Rule 0 + affordances + canonical defaults + scoped doctrine rules + anti-patterns + skill map. Pass `--for <path>` to scope to a subtree, `--json` for machine consumption (cross-AI). This replaces "read AGENTS.md, then TOOLS.md, then doctrine, then…" with one mechanical step.
2. **Run `forge --help`** to see the full subcommand surface live (more accurate than this doc).
3. **Check `forge.toml`** for build mode + suppressed/strict gates.
4. **Run `forge build`** to confirm green baseline before changing anything.
5. **State the goal** in one sentence — does it match a Forge subcommand? a forge-phases module? a typed `*-core` API? Reach for the platform-provided thing first.

If you are about to invoke bash/grep/find/curl/awk/sed on substrate-managed state, stop and re-read RULE 0.
