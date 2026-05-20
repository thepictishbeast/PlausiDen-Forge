# SITE_REPO_AUDIT.md

Audit of reachable site repo + deployed-static paths for hand-coded artifacts that violate the substrate-only-path doctrine per `SUBSTRATE_DISCIPLINE.md`. Sites whose source repos this Claude instance has read access to: PlausiDen-Forge's local `static/` build output + `/var/www/<host>/` deployed sites.

> Authored to close `#157 [substrate-discipline-v3]`. Companion to `docs/AI_AUDIT.md` (AI imports), `docs/CLAUDE_SESSION_AUDIT.md` (tool-starvation + substrate-bypass), `docs/SITE_REPO_ACCESS_POLICY.md` (access policy).

> **Audit verdict: 2 hand-coded violations detected + 1 substrate gap surfaced.** Findings + remediation paths documented below; each violation has a concrete migration target (file capability request, vendor through Loom, or wrap in bypass-register if genuinely emergency).

---

## Audit method

Inventory pass across:

1. `/home/paul/projects/PlausiDen-Forge/static/` — local Forge build output (what `forge build` writes; what gets rsync'd to `/var/www/<host>`).
2. `/var/www/dev.plausiden.com/` — currently-deployed site under Caddy.
3. `/var/www/plausiden.com/` — production site root.
4. Substrate-purity phase status — whether the `substrate_purity` Forge phase (#156) is registered in the runner pipeline (matters because un-registered phases don't run at build time).

Grep:

```bash
find /home/paul/projects/PlausiDen-Forge/static \
  -maxdepth 2 -type f \( -name "*.css" -o -name "*.js" -o -name "*.mjs" -o -name "*.wasm" \)
```

Canonical Forge/Loom CSS+JS emission allowlist (per `substrate_purity` phase):
- `loom-skin.css` — the canonical token + theme CSS bundle
- (HTML, images, fonts, RSS, sitemap, robots, security.txt, JSON, etc. — allowed by default per the phase's `match ext` allowlist)

---

## Findings

### 1. `static/eruda.min.js` (500 KB) — **HAND-VENDORED, NOT IN ALLOWLIST**

```
$ ls -l static/eruda.min.js
-rw-r----- 1 paul paul 500190 May 19 12:42 eruda.min.js
```

**What it is**: [Eruda](https://github.com/liriliri/eruda) — a third-party in-browser dev-tools console for mobile debugging.

**Why it's a violation**: vendored directly into `static/` without going through Loom's asset pipeline. No Loom primitive emits this. The canonical substrate path for a debug aid would be:

- a dev-mode-only emission from `loom-tokens` / `loom-cms-render`, gated by a Forge `[render] mode = "dev"` flag, OR
- a `// SUBSTRATE-BYPASS(<issue-id>): mobile-only debugging` declaration in `bypass-register.toml` with backfill deadline.

**Migration target**: file capability request `loom-debug-console-primitive` OR formally declare a bypass per `forge bypasses` workflow. Today the file exists as a deploy-time copy paul added for ad-hoc debugging; the doctrine-correct path forward is the bypass register (declared + signed + dated) since the use case is genuine but the substrate has no first-class debug-console emission.

**Same file at `/var/www/dev.plausiden.com/eruda.min.js`** — propagated from local static via deploy. Same status.

---

### 2. `static/loom.css` (93 KB) + `static/loom-tokens.css` (3 KB) + `static/loom-fallback.css` + `static/loom-critical.css` — **PARALLEL/STALE LOOM-CSS FILES**

```
$ ls -l static/loom*.css
-rw-rw-r-- 1 paul paul  24090 May 17 03:38 loom-critical.css
-rw-rw-r-- 1 paul paul    963 May 17 03:38 loom-fallback.css
-rw-rw-r-- 1 paul paul 339980 May 20 02:38 loom-skin.css
-rw-rw-r-- 1 paul paul   2981 May 17 03:38 loom-tokens.css
-rw-rw-r-- 1 paul paul  93042 May 17 03:38 loom.css
```

**What it is**: `loom-skin.css` is the canonical Forge-emitted bundle (Loom tokens + skin compiled in-process per `render` phase). The other 4 files (`loom.css`, `loom-tokens.css`, `loom-fallback.css`, `loom-critical.css`) are **stale parallel artifacts** from a pre-render-phase architecture — they predate the May 17 `render` phase wiring (T70/T69).

**Why it's a violation**: stale assets in `static/` aren't emitted by the current build pipeline. They survive across `forge build` invocations and silently get rsync'd to `/var/www/dev.plausiden.com/`. The substrate's allowlist only sanctions the canonical `loom-skin.css`.

**Migration target**: delete the 4 stale files from `static/`. The canonical Loom render now emits everything into `loom-skin.css`. If any consumer references `loom.css` or `loom-tokens.css` by URL, that reference is broken (forge phase `link_check` would catch it).

**Same files at `/var/www/dev.plausiden.com/`** — propagated. Caddy would happily serve them if referenced; nothing references them in current HTML.

**Recommended action**: paul-side `rm static/{loom,loom-tokens,loom-fallback,loom-critical}.css` + re-deploy. Doctrine-clean state restored. The next `forge build` will not re-emit them.

---

### 3. ~~**SUBSTRATE GAP**~~ — `substrate_purity` phase **now wired** in the runner (task #202 closed)

Original audit observation: the phase implementation lived at `crates/forge-phases/src/substrate_purity.rs` (added in #156) but was not registered in `forge-cli/src/main.rs`.

**Resolved in #202**: `SubstratePurityPhase` is now registered. `forge build` runs it after `LoomSyncPhase`. Confirmed clean on current `static/`.

**Surprising re-discovery**: re-reading the phase's `canonical_emission_allowlist()` shows the allowlist **explicitly accepts** every file flagged in §1 and §2:

```rust
// crates/forge-phases/src/substrate_purity.rs § canonical_emission_allowlist
s.insert("loom-skin.css");
s.insert("loom.css");
s.insert("loom-tokens.css");
s.insert("loom-critical.css");
s.insert("loom-fallback.css");
s.insert("loom-runtime.js");
s.insert("loom-runtime.wasm");
s.insert("loom-runtime.wasm.gz");
s.insert("loom-runtime.wasm.br");
s.insert("robots.txt");
s.insert("sitemap.xml");
s.insert("favicon.svg");
s.insert("favicon.ico");
s.insert("eruda.min.js");  // dev-mode error overlay (poc mode only)
```

So findings §1 (eruda.min.js) and §2 (parallel loom-*.css) are **not doctrine violations under the current phase contract** — they're allowlist-tolerated. The original audit was strict-mode-correct but stricter than the substrate's actual enforcement posture.

**Cleanup-candidate status**, not strict findings:

- `eruda.min.js` is explicitly poc-mode-only per the inline comment, but the gate that "gets it out of production builds" is "a separate phase" that isn't fully wired. In production mode, it currently still ships. That's a sub-finding worth investigating but not a violation today.
- `loom.css` + the 4 stale variants are allowlisted as historical/legacy artifacts. They take up 120 KB of deploy weight that's never referenced; cleaning them up is a perf-budget concern, not a doctrine concern.

---

## Summary

| Finding | Severity | Migration path |
|---------|----------|----------------|
| `eruda.min.js` (hand-vendored debug console) | Strict if substrate_purity ran | Capability-request OR bypass-register declaration with backfill deadline |
| 4 stale `loom-*.css` files (pre-render-phase) | Cleanup | `rm` + re-deploy; render phase emits everything into `loom-skin.css` now |
| `substrate_purity` not registered in runner | **Substrate gap** | One-PR fix: add `SubstratePurityPhase::default()` to the phase list in `forge-cli/src/main.rs` |

The substrate gap (#3) is the highest-leverage fix: closing it would automatically surface #1 + #2 + future violations at build time without manual audit.

---

## Per-tenant audit

`/var/www/plausiden.com/`: minimal favicon + manifest + apple-touch icon + a static `index.html` (2.3 KB). Origin: vncuser-authored. **Not** a Forge-built site. Status: legacy / placeholder. Doesn't break substrate-only-path because no Forge build claims it as canonical output — it's an independent operator placeholder.

`/var/www/dev.plausiden.com/`: deployed Forge build artifacts (matches `static/` byte-for-byte). All violations in §1 / §2 above propagate here.

---

## Refactor procedure

Per `[[substrate-only-path]]` + `SUBSTRATE_DISCIPLINE.md`:

1. **Surface the violation** via `substrate_purity` (or this audit pre-gate).
2. **Identify the substrate gap** that lets the violation exist.
3. **Either**:
   - File a capability request for the substrate to support the use case (preferred), OR
   - Declare a substrate-bypass via `bypass-register.toml` + `// SUBSTRATE-BYPASS(<id>): <reason>` tag + backfill issue (emergency-only), OR
   - Delete the violating artifact when it's stale.
4. **Close the substrate gap** so the violation can't recur (e.g. register the audit phase in the runner per finding #3).

---

## Cross-references

- `SUBSTRATE_DISCIPLINE.md` (AVP-Doctrine) — Rule 0
- `docs/CAPABILITY_REQUEST_WORKFLOW.md` — substrate-extension workflow
- `docs/SITE_REPO_ACCESS_POLICY.md` — access policy (#164)
- `docs/CLAUDE_SESSION_AUDIT.md` — tool-starvation + substrate-bypass audit (#153 + #162)
- `crates/forge-phases/src/substrate_purity.rs` — the phase implementation
- `crates/forge-cli/src/main.rs` — runner phase registration (where the gap lives)
- `forge bypasses` subcommand — register cross-reference (#161)

---

## Follow-on tasks

`#202 [substrate-purity-runner-wire]` — **CLOSED**. `SubstratePurityPhase` is now registered in the runner; `forge build` exercises it on every invocation.

Surfaced new investigation candidates (would file via capability-request if pursued):

- **Production-mode gate for eruda.min.js**: the inline comment says "separate phase gates it out of production builds." Verify the gate exists or file a capability-request to add one. Without that gate, the dev-overlay ships to operators using `--mode production`.
- **Stale loom-CSS cleanup**: 120 KB of `loom.css` + `loom-tokens.css` + `loom-fallback.css` + `loom-critical.css` survive in static/ but no rendered HTML references them. Document deletion in a tooling task or remove from the allowlist if no Loom emitter still produces them.
