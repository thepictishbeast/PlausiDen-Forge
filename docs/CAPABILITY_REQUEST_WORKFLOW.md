# Capability request workflow

Companion to [`SUBSTRATE_DISCIPLINE.md`](https://github.com/thepictishbeast/PlausiDen-AVP-Doctrine/blob/main/SUBSTRATE_DISCIPLINE.md). When the substrate cannot currently do something a site or capability needs, the workflow is:

1. **File a capability request issue** using the template at `.github/ISSUE_TEMPLATE/capability-request.yml`. This produces a typed declaration of (what's needed, why, proposed contract, blocked-by-site, acceptance criteria).
2. **Do NOT hand-code around the gap.** Hand-authored CSS / HTML / JS in site repos is forbidden per the substrate-only-path doctrine. The `substrate_purity` Forge phase will flag any such files on the next build. If the work genuinely cannot wait, use the substrate-bypass register (task #161) — it's heavyweight and visible.
3. **Implement the capability in the appropriate substrate repo.** Anyone (operator, contributor, AI agent) can implement; the proposal in the issue establishes the contract; the PR fulfills it.
4. **Exercise the new capability by authoring CMS content.** The site work that triggered the request is unblocked once the capability is in main.

## What goes in a request

The issue template enforces structure. Required fields:

- **Substrate layer** — which layer of the substrate this fits (Loom primitive, Forge phase, *-core, CMS schema, etc.).
- **What's needed** — one-paragraph description, specific to the missing capability.
- **Why** — what site / feature is blocked, with concrete examples.
- **Proposed contract** — typed interface (props, audit gates, accessibility behavior). Not implementation — the *shape* the substrate would project.

Optional but encouraged:

- **Blocked-by site** — link to the work the request unblocks.
- **Related rules and traits** — cross-reference to AVP-Doctrine rules (run `forge doctrine for <path>` to see applicable rules).
- **Bypass status** — if there's an active substrate-bypass tagged in code waiting for this capability.
- **Acceptance criteria** — concrete signals for "done."

## What's NOT a capability request

- "The build is failing on rule prim-001" — that's a bug fix or rule-compliance work, not a substrate gap.
- "I want to use a different HTTP framework than Axum" — canonical defaults are not relitigated; reach for an ADR via `docs/adr/` if you genuinely need to change.
- "I want hand-rolled CSS for this one page" — forbidden per substrate-only-path. File a Loom primitive request instead.
- Content-only changes (CMS edits) — those are PRs to the site repo, not capability requests.

## Triage + prioritization

Capability requests live in the GitHub Issues queue with label `capability-request`. Triage cadence:

- **Weekly or as-needed** — review open requests; prioritize by blocked-by-site impact + cross-site reuse potential.
- **Per request** — confirm the contract makes sense; refine if the requester proposed too narrow or too wide a shape.
- **Per PR** — when the capability lands, the implementing PR cites the request issue id in its body; the request closes automatically on merge.

## Connection to other workflows

- **Doctrine rules**: a capability request that surfaces a missing rule (e.g. "we need a rule about X") also files an AVP-Doctrine PR adding the rule to the appropriate `doctrine/rules/<domain>.toml`.
- **Substrate-bypass register**: every active bypass references the capability request that would replace it (task #161 wires the bypass-register.toml).
- **AGENTS.md updates**: when the new capability lands, the implementing PR updates the tool inventory section in AGENTS.md in the same commit per doctrine rule docs-007.
- **TOOLS.md**: new commands or subcommands also get added to the canonical command index.

## Workflow as an AI agent

Claude (and Gemini, and other AI agents) working in the substrate:

1. **At session start**: read `AGENTS.md` + the doctrine rule set applicable to the working directory (`forge doctrine for <crate-path>`).
2. **Mid-task**: if you find yourself wanting to hand-author CSS / HTML / JS, **STOP**.
3. **Identify the gap**: what primitive / phase / capability would the substrate need so this site work becomes pure CMS content authoring?
4. **File the capability request** using this template via `gh issue create --template capability-request.yml`. Be specific in the proposed contract — the contract is the deliverable.
5. **Decide**: implement the capability immediately (if simple), or defer the site work (if not). Either way, file the request before continuing.

Per [`[[tool-starvation-anti-pattern]]`](https://github.com/thepictishbeast/PlausiDen-AVP-Doctrine/blob/main/doctrine/rules/docs.toml) doctrine + the substrate-only-path rule: the capability-request workflow is the canonical channel for "the substrate doesn't do this." Use it.
