# TOOLS.md — PlausiDen-Forge

Canonical command index for the Forge CLI. Grepable single source of truth. Generated periodically from `forge --help`; PRs that add/remove/change a subcommand update this file in the same commit (per AVP-Doctrine rule `docs-007`).

Run `forge --help` for the latest live surface. Run `forge doctrine query --rule <id>` to read any cited rule's rationale + enforcement.

---

## Build + watch

### `forge build`
Run the full audit pipeline. Phase order in `forge.toml`. Production mode requires strict findings == 0 (rule `build-001`).
```
forge build                    # poc mode (default)
forge build --mode production  # strict, fails on any finding
forge build --mode hybrid      # SPA-augmented static
forge build --mode dynamic     # full runtime
forge build --json-report path # emit machine-readable findings
```

### `forge watch`
Continuous-build mode. inotify-driven; debounce + max_rebuilds args. Exit with Ctrl-C.
```
forge watch
forge watch --debounce-ms 500 --max-rebuilds 100
```

---

## JSON output (for scripts + AI agents)

Every Forge subcommand (and the Loom / Crawler companions) supports `--json` for machine-readable output. Canonical schemas live in [`docs/JSON_OUTPUT.md`](./docs/JSON_OUTPUT.md).

```
forge orient --json                        # session brief
forge doctrine query --json                # rule projection
forge build --json-report path.json        # build report alongside terminal
forge config --json                        # all config gates as one document
```

Every envelope has a `status` field: `ok | warn | fail | fatal | empty`. Exit codes mirror status. Per rule `docs-008`.

---

## MCP tool surface (for AI clients)

Cross-AI consumable Model Context Protocol tool definitions live in `mcp/`:

- **`mcp/manifest.json`** — server manifest indexing every declared tool, with categories + agent guidance.
- **`mcp/tools/*.json`** — one input schema per tool. Read by Claude / Gemini / Cursor / other MCP-capable clients.
- **`mcp/README.md`** — how to mount the surface from your client.
- **`make mcp-list`** — print every declared tool + one-line description.

The MCP surface is a typed projection of the CLI surface; the CLI is the authoritative implementation. Per `[[priority-architectural-first-and-cross-ai]]`: no Claude-specific extensions, identical schemas for every agent.

---

## Orient (session-start meta-tool)

### `forge orient`
Single command that synthesizes everything an AI agent (Claude, Gemini, other) needs at session start: Rule 0 (substrate-only-path), affordance inventory (AGENTS.md / TOOLS.md / Makefile / skills / capability-request workflow), canonical defaults, scoped doctrine rules, skill map, and anti-pattern reminders.

Replaces the aspirational "read AGENTS.md, then TOOLS.md, then doctrine, then…" with one mechanical step. Per `[[tool-starvation-anti-pattern]]` + `[[priority-architectural-first-and-cross-ai]]`.

```
forge orient                          # human-readable session brief
forge orient --json                   # machine-readable for AI tool-use
forge orient --for crates/forge-phases  # scope doctrine rules to a subtree
forge orient --doctrine-dir <path>    # override AVP-Doctrine location
```

JSON output is the cross-AI consumable surface (Claude / Gemini / other agents). No agent-specific extensions; common schema.

---

## Audit (out-of-pipeline, fast feedback)

### `forge audit <action>`
One-off scans, not gated on build success. Use for pre-commit hooks + iteration.
```
forge audit secrets           # credential leak scan (gitleaks-equiv, respects .gitignore)
forge audit secrets --explain # name the rule that matched per path
forge audit mutants           # AVP-2 Tier 6 mutation testing report
forge audit mutants --run     # invoke cargo mutants first (SLOW)
```

Use `forge audit secrets` **instead of** `gitleaks detect` (rule `sec-003`).
Use `forge audit phantom_button` **instead of** `grep -r 'data-backend' static/` (rule `sec-007`).

---

## Verify + attest (cryptographic integrity)

### `forge verify`
Walks `reports/build-*.json`, asserts Merkle chain integrity (rule `build-005`).
```
forge verify                    # chain integrity only
forge verify --signatures       # also verify Ed25519 signatures
```

### `forge attest <action>`
T56 attestation key management.
```
forge attest init                       # generate Ed25519 keypair
forge attest sign reports/build-X.json  # sign a build report
forge attest fingerprint                # show pubkey fingerprint (16-char base64url SHA-256)
```

---

## Doctrine (rule database)

### `forge doctrine query [filters]`
Query the AVP-Doctrine 71-rule database (build / primitives / security / testing / docs / logging / perf / content / accessibility).
```
forge doctrine query --rule prim-001
forge doctrine query --domain security
forge doctrine query --severity strict --lifecycle stable
forge doctrine query --search "tap target"
forge doctrine query --related-trait Sensitive
forge doctrine query --domain perf --json
```

### `forge doctrine check`
PR gate: load rules + scan workspace for `.citing([...])` literals, fail on orphan rule references.
```
forge doctrine check
forge doctrine check --source-dir crates/forge-phases --json
```

### `forge doctrine exceptions`
Inline `// DOCTRINE-EXCEPTION: rule-XXX — reason, see ADR-YYY` register. Verifies each cited rule exists + has justification + reference.
```
forge doctrine exceptions
forge doctrine exceptions --json
```

### `forge doctrine for <path>`
Surface rules applicable to a specific path. Markdown-list (`--terse`) form is embeddable in per-directory AGENTS.md.
```
forge doctrine for crates/forge-phases
forge doctrine for crates/loom-cms-render --terse
forge doctrine for /path/to/site/cms --json
```

### `forge doctrine render`
Render the full doctrine database as a single Markdown document for `docs.plausiden.com/doctrine`.
```
forge doctrine render --out docs/doctrine.md
forge doctrine render | pandoc -o doctrine.html
```

### `forge doctrine lifecycle`
Audit by lifecycle state. Surfaces experimental rules (promotion candidates), deprecated rules (sunset + replacement), health summary.
```
forge doctrine lifecycle
forge doctrine lifecycle --json
```

Doctrine resolution order for every action: `--doctrine-dir` → `$PLAUSIDEN_DOCTRINE_DIR` → `<forge-root>/../PlausiDen-AVP-Doctrine`.

---

## Config gates (typed-surface validation)

Each gate loads its TOML at the project root, projects through the typed `*-core` crate, fails on schema violations. Use individually during iteration or `forge config` to run all at once.

### Individual gates
```
forge manifest      # phases.toml + backends.toml: kebab-case, acyclic, capability resolution (rule build-006)
forge privacy       # privacy.toml: RetentionPolicy ∀ DataCategory; days>0; LawfulBasis (sec-001)
forge trust-safety  # trust-safety.toml: CSAM/NCIII/Extremism scanners (sec-006)
forge domains       # domains.toml: RFC 1035 FQDN, RFC 8555 wildcards, HSTS preload (sec-010)
forge audit-log <path.json>  # observability-core AuditChain integrity (sec-006)
forge forms         # forms.toml: https webhook, WCAG labels (rule a11y-001)
forge federation    # federation.toml: protocol/address consistency
forge email         # email.toml: RFC 8058 list-unsubscribe for marketing
forge commerce      # commerce.toml: ISO 4217 currency, prices, SKUs
forge memberships   # memberships.toml: tier id kebab, currency, price
```

### Umbrella
```
forge config        # run every config gate at once, aggregate pass/fail
```

---

## Content + asset workflows

### `forge content <action>`
CMS section authoring lifecycle (importers-core + exporters-core).
```
forge content validate path.json [--json]    # CmsSection contract checks (rule prim-006, a11y-001)
forge content format-list                    # list export formats
forge content export path.json --format X    # project to typed export format
```

### `forge search <action>`
Search index validator (search-core).
```
forge search validate-index path.json [--json]
```

### `forge assets <action>`
Asset-bundle validator (assets-core). AVIF/WebP/JPEG ladder + WCAG 2.1 §1.1.1 alt text (rule perf-005).
```
forge assets validate path.json [--json]
```

---

## Fix

### `forge fix`
Auto-apply mechanical findings from latest build report.

---

## Codegen (complete-Rust-stack runtime generator)

### `forge codegen --dry-run`
Walks `cms/*.json` + `backends.toml`, prints the planned generated-crate file set + stage summary. Use to preview before writing.

### `forge codegen --out <DIR>`
Generates a complete Cargo crate that turns every CmsPage into a typed `async fn render_<slug>() -> Html<String>` handler. axum + tokio + serde + loom-cms-render stack. Five stages (handler-scaffold, router-assembly, crate-manifest, persistence-layer, smoke-tests). Generated crate is self-verifying — `cargo test` on the output runs one smoke test per page proving every route returns 200 + non-empty body.

Options:
- `--crate-name <NAME>` override (default: project root basename, kebab-cased)
- `--dry-run` print plan only, don't write

See `crates/forge-codegen/src/lib.rs` for the stage shapes.

---

## Pixel reproduction (Forge #218 / rotation work)

These targets live in the Makefile, not `forge` itself, because they wrap multiple binaries (Crawler + ImageMagick).

### `make pixel-rep`
Capture live URL + local Forge mirror at 390/768/1280px via Crawler's `--capture-reference`. Outputs land under `../PlausiDen-Crawler/runs/<slug>/` (live) and `<slug>-forge/` (mirror).

Overrides:
- `PIXEL_REP_SLUG=stripe`
- `PIXEL_REP_SITE_URL=https://stripe.com/`
- `PIXEL_REP_FORGE_PATH=/stripe.html` (default `/`)

End-to-end cycle on prosperity-club: ~17 seconds.

### `make pixel-rep-diff SLUG=<slug>`
Compact file-size delta table for a captured slug.

### `make pixel-rep-visual-diff SLUG=<slug>`
ImageMagick `compare -metric AE -fuzz 5%` against the captured pair. Emits diff PNGs with red-overlay marking changed pixels + per-viewport pixel-diff count + % of live area.

### `make pixel-rep-rotation`
Walks every `<slug>+<slug>-forge` pair under `runs/` and prints a compact per-site pixel-diff summary table. Auto-updates as new captures land.

Per-site analysis docs live in `docs/PIXEL_REP_<SLUG>.md`. Rotation summary doc: `docs/PIXEL_REP_ROTATION.md`.

---

## Global flags

Apply to every subcommand:
```
--root <ROOT>            # project root (default CWD)
--mode <MODE>            # build mode override [poc|production|static|hybrid|dynamic]
--json-report <PATH>     # JSON report alongside terminal output (build subcommand only)
```

Environment variables:
- `FORGE_ROOT` — default project root
- `PLAUSIDEN_DOCTRINE_DIR` — default AVP-Doctrine repo path
- `RUSTFLAGS="-D warnings"` recommended per AVP-2 doctrine

---

## Anti-patterns — DO NOT do these

These are real failure modes from past sessions. Each lists the generic invocation people reach for, the platform tool to use instead, and the doctrine rule the misuse violates.

| ❌ Don't reach for | ✅ Use instead | Rule |
|--------------------|----------------|------|
| `grep -r 'data-backend' static/` | `forge audit phantom_button` (when wired) or `forge build` | sec-007 |
| `find . -name '*.json' -path '*cms*'` | Forge cms loader (already enumerates) | — |
| `gitleaks detect` | `forge audit secrets` | sec-003 |
| `curl https://prosperityclub.com/` to fetch + diff a site | `crawler --journey <file>` with `goto` + `screenshot` steps | — |
| Hand-rolled CSS in a site repo | Loom tokens / primitives; extend Loom as a separate PR | prim-006 |
| Editing `static/loom-skin.css` directly | Edit `PlausiDen-Loom/loom-tokens/src/skin.css`; `forge build` regenerates | — |
| Bypassing strict findings with poc mode for production ship | Fix the finding (rule build-001 + build-007) | build-001 |
| Direct push to main on LFI repos | PR-only flow per memory `feedback_lfi_out_of_scope_for_this_instance` | — |
| Writing raw class strings outside loom-components | Use typed primitives; new variant = separate PR | prim-006 |
| `==` / `.eq()` on secrets / MACs / tokens | `subtle::ConstantTimeEq` | sec-009 |
| Embedding secrets in source / config | Secrets manager (env / age-encrypted / HSM) | sec-003 |

---

## Adding new tools

When adding / removing / modifying a Forge subcommand:

1. Update the relevant `Cmd` / action enum in `crates/forge-cli/src/main.rs`.
2. Update `AGENTS.md` Tool inventory section in the same commit (rule docs-007).
3. Update this file (`TOOLS.md`) — categorize by purpose, include canonical invocation example.
4. If the new tool enforces or relies on a doctrine rule, cite it in the description.
5. `forge build` must stay green.
