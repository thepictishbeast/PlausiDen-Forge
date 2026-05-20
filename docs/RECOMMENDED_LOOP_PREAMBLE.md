# RECOMMENDED_LOOP_PREAMBLE.md

Proposed update to the durable cron-loop preamble paul installed at `2026-05-19 ~12:33 UTC`. Captures the architectural-first + cross-AI-tooling directives that emerged after the loop was originally authored.

> **Status:** authored by Claude per task #198. Paul installs via CronDelete + CronCreate on the active loop.

---

## What's different from the current preamble

The current preamble already covers task-completion priority, editorial-content replacement, pixel-by-pixel reproductions, Crawler detectors, hygiene, culprit-hunting, substrate-deepening, and LFI work. It does **not** explicitly carry two doctrines that paul has since stated and that are now load-bearing across the ecosystem:

1. **Architectural-first** — per memory `[[priority-architectural-first-and-cross-ai]]`: "Do architectural / standards / rules work FIRST regardless of length. Don't avoid long or time-consuming architectural tasks."

2. **Cross-AI tooling** — per same memory: "Build Skills + MCP for Claude + Gemini + other AI; heavily cater to Claude + Gemini. Schemas are format-agnostic — no agent-specific extensions."

Two additional reinforcements are warranted given recent session experience:

3. **Substrate-only-path Rule 0** — per memory `[[substrate-only-path]]`: hand-coding HTML/CSS/JS in site repos is forbidden. Every gap is a substrate change.

4. **Deterministic-first, LFI-optional** — per memory `[[deterministic-first-lfi-optional]]`: every capability has a deterministic baseline; LFI is opt-in augmentation, never load-bearing. **This supersedes** the earlier "LFI is the brain" framing currently in PRIORITY 8.

---

## Proposed preamble (full text, ready to install)

```text
FINISH THE TASKS. Run TaskList first. Drive open tasks to completion before reaching for new work. Per paul (2026-05-19): "and finish your tasks they are piling up" — the backlog has grown faster than completions. Close one substantive open task per iteration whenever feasible. Mark complete via TaskUpdate; delete with status=deleted if obsolete.

SCHEDULED CONTINUOUS-IMPROVEMENT LOOP for PlausiDen-Forge / Loom / Crawler / LFI. Created 2026-05-19 ~12:33 UTC by paul. Fires every minute, durable (survives restarts), expires automatically after 7 days; ask paul before re-scheduling.

ARCHITECTURAL-FIRST. Per memory [[priority-architectural-first-and-cross-ai]]: do architectural / standards / rules / discipline / cross-AI-tooling work FIRST regardless of length. Don't avoid long or time-consuming architectural tasks. Specifically: substrate discipline (doctrine + bypass register + capability-request workflow), tool surface (AGENTS.md / TOOLS.md / make help / skills / MCP / forge orient), trait DAG, doctrine rule extensions, N-orientation manifest, backward-compat version discipline. These compound. Site rebuilds compound less per hour spent.

CROSS-AI TOOLING. Schemas are format-agnostic — Claude, Gemini, Cursor, other MCP-capable agents read identical JSON. No agent-specific extensions. mcp/manifest.json + mcp/tools/*.json are the canonical surface. Anything I author for Claude should also work for Gemini.

SUBSTRATE-ONLY-PATH (LOAD-BEARING). Per memory [[substrate-only-path]]: hand-coding HTML / CSS / JS in site repos is forbidden. Every gap is a substrate change: file a capability-request, extend Loom (primitive / variant), extend Forge (phase), or extend doctrine. The substrate_purity phase enforces this. Canonical defaults (do NOT relitigate): axum / tokio / sqlx / maud / serde-deny_unknown_fields / Ed25519+ML-DSA / clap / anyhow-thiserror / proptest / tracing. Forbid unsafe_code, no unwrap/expect outside tests.

DETERMINISTIC-FIRST. Per memory [[deterministic-first-lfi-optional]]: every capability has a deterministic baseline; LFI is opt-in augmentation, never load-bearing. Don't write code that assumes AI availability. Critic trait abstraction is the seam. Supersedes earlier "LFI is the brain" framing.

Per iteration, work the priority stack top-down. Finish what you can in the time you have. Leave durable state (commits, files, memory, tasks) consistent before stopping.

PRIORITY 1 — Architectural / standards / discipline work. The toolsurface arc (#145-154), substrate-discipline arc (#156-165), doctrine arc, trait arc (#166-172), backward-compat arc (#137-143), N-orientation arc (#183, #190-197), deterministic-first arc (#185-189), MCP arc (#199), session-start orient (#152). These compound across every future iteration. Close them. Don't worry about length.

PRIORITY 2 — Complete remaining tasks. Run TaskList. Including LFI tasks #34-37 (PR-only flow per memory feedback_lfi_out_of_scope_for_this_instance — branch + commit + push + open PR, never direct main push). Plus #15 / #93 (real-website validation), #100 (RFC 3339 timestamps), #101 (complete-Rust-stack codegen), #102 (theme-toggle WASM/CSS port), #103 (substrate de-consumer-shaping audit), #104 (variant explosion to 300-500 primitives), #105 (slot composition), #106 (decorative/editorial/compositional primitive tiers), #107 (polish-token enum), #109 (reference corpus + density tiers), #112-#124 (real-site rebuilds + new Crawler axes + Tor / TUI / app-UI / noscript / account-primitives / hunted-tier work), plus any newly-created tasks. Mark complete via TaskUpdate. Don't claim done if tests fail.

PRIORITY 3 — Assume Forge-generated content is ugly, boring, reused, SkillShots-shape. Search cms/*.json + Loom primitives for SaaS-trope shapes: centered single-line heroes, 3-column feature_spotlight grids, fake testimonial cards with avatars, green-checkmark pricing with "most popular" badge, gradient-clipped marketing text, "Numbers that compose"-style stat bands. Replace with editorial / asymmetric / dense / kinetic compositions using existing primitives.

PRIORITY 4 — Pixel-by-pixel real-site reproductions via Forge. Rotate through:
  • prosperityclub.com
  • plausiden.com (Forge-static matches the prod Rust app)
  • sacred.vote / sacredvote.org
  • Stripe / Linear / Vercel / GitHub / Notion / Anthropic / Render / Fly

For each: Crawler screenshots at 390 / 768 / 1280. Build Forge CMS targeting the composition. Deploy to /var/www/dev.plausiden.com (rsync, chown caddy:caddy). Screenshot via firefox-esr --headless. Diff visually. Missing primitives / themes / variants → ADD them to Loom (generic, never site-specific). Repeat until match. Substrate-only-path applies: NO byte-mirror, NO hand-authored HTML/CSS in /var/www.

PRIORITY 5 — Improve Crawler detectors. Cover: broken text (character-per-line column collapse, mid-word gradient clipping, RTL/LTR mix), hidden elements, low contrast (auto-flag <WCAG AA), overflow clipping, off-screen content, FOUC, fonts missing, layout-thrash, image desert. Add Detector trait impls.

PRIORITY 6 — Hygiene every iteration: `sudo -u paul cargo test` across PlausiDen-Forge, PlausiDen-Loom, Crawler, PlausiDen-LFI, Forge-LFI, Crucible. `sudo -u paul ./target/release/forge build` and ensure strict findings == 0. cargo audit / cargo deny / clippy if available. Commit fixes as paul with descriptive bodies. Push to GitHub. Bump pinning between repos.

PRIORITY 7 — Find new culprits. Pick one Loom primitive, audit its CSS, look for hardcoded SkillShots-flavor (centered, gradient, rounded-card, drop-shadow, neon). Make it variant-aware or split substrate+theme. File the work via TaskCreate.

PRIORITY 8 — Substrate-deepening per memory feedback_consumer_shaped_substrate: variant explosion, slot composition, density audit phase, slop dictionary, per-tenant corpora, decorative + editorial + compositional-relationship primitive tiers.

PRIORITY 9 — LFI work. Tasks #34-37: lfi-core / lfi-policy / lfi-corpus / lfi-critic. Per [[deterministic-first-lfi-optional]]: LFI is opt-in augmentation, never load-bearing. Per [[manifest-layer-is-the-keystone]]: project capabilities through manifest, don't rewrite. Per [[super-society-tech-stack]]: fast + reliable + robust + secure + anonymous + private SIMULTANEOUSLY. Repos at /home/paul/projects/PlausiDen-LFI and /home/paul/projects/Forge-LFI. PR-only push flow (the dedicated LFI Claude merges).

SESSION START. Call `forge orient` (or read mcp/tools/forge_orient.json) first. It synthesizes Rule 0 + affordances + canonical defaults + scoped doctrine + skill map + anti-patterns. Replaces "read AGENTS.md, then TOOLS.md, then doctrine, then ..." with one mechanical step.

When all priorities pass — assume they don't. Restart from PRIORITY 1.

Hard constraints (read these EVERY iteration; memory loader re-loads them automatically but they belong here too):
  - Stay scoped: PlausiDen-Forge, PlausiDen-Loom, Crawler, Annotator, Forge-LFI, PlausiDen-LFI, Crucible. DO NOT touch Sacred.Vote source.
  - Rust + WASM + CSS only. No Python anywhere (including dev-time validation: use node, cargo, or jq, not python3).
  - No time estimates anywhere.
  - No destructive ops (force-push, reset --hard, branch delete) without explicit paul approval.
  - All git ops: `sudo -u paul git -C <path>`. chown root-edited files back to paul before staging.
  - LFI repos: PR-only flow, never direct push to main.
  - Per memory feedback_no_meta_narration: no announcing what you're about to do; just do or skip.
  - Per memory ceo_mode: don't menu paul. Pick and execute.
  - Email paul at redcaptian1917@gmail.com only if something is blocking and only with concrete recommendation.

Don't restart paused work mid-step. Don't sleep. Don't poll. Don't ask paul to run commands unless absolutely required. Don't re-arm this loop — it expires 7 days from creation; ask paul before re-creating.
```

---

## Change summary

| Change | Why | Source |
|--------|-----|--------|
| Add ARCHITECTURAL-FIRST block (above priorities) | Memory directive overrides "complete all tasks" interpretation that biases toward small wins | `[[priority-architectural-first-and-cross-ai]]` |
| Add CROSS-AI TOOLING block | Make MCP + Skills + format-agnostic schema posture explicit | `[[priority-architectural-first-and-cross-ai]]` |
| Add SUBSTRATE-ONLY-PATH block with canonical defaults | Inline the load-bearing Rule 0 + canonical defaults rather than relying on memory alone | `[[substrate-only-path]]` |
| Add DETERMINISTIC-FIRST block | Supersede prior "LFI is the brain" framing inline | `[[deterministic-first-lfi-optional]]` |
| Reorder priorities: architectural-arc work to #1, task-completion to #2 | Counter-bias against avoiding long tasks | paul 2026-05-20 directive |
| Add "no Python anywhere" clarification | Caught one one-shot python3 use this iteration; tighten constraint | session evidence |
| Add SESSION START block pointing at `forge orient` | Newly-shipped meta-tool; agents should reach for it first | task #152 |
| Demote LFI to PRIORITY 9 + reframe as opt-in augmentation | LFI is no longer the brain; deterministic baseline first | `[[deterministic-first-lfi-optional]]` (supersedes `[[lfi-as-core-llm-as-peripheral]]`) |

---

## How to install (paul-side)

```bash
# 1. Find the active loop job id
CronList

# 2. Delete the existing entry
CronDelete <job-id>

# 3. Re-create with updated preamble (paste the proposed-preamble block above as the prompt)
CronCreate cron="*/1 * * * *" recurring=true prompt="<preamble text>"
```

(Or use `/loop` / `/loop-resume` workflow if preferred. The cron expression stays `*/1 * * * *`.)

---

## See also

- [`forge orient`](../crates/forge-cli/src/main.rs) — session-start meta-tool that this preamble references.
- [`mcp/manifest.json`](../mcp/manifest.json) — cross-AI tool surface that this preamble references.
- [`AGENTS.md`](../AGENTS.md) — top-level orientation; complements the preamble.
- Memories: `[[priority-architectural-first-and-cross-ai]]`, `[[substrate-only-path]]`, `[[deterministic-first-lfi-optional]]`.
