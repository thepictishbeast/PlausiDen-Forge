# SITE_REPO_ACCESS_POLICY.md

Access-control policy for AI agents (Claude, Gemini, other MCP-capable clients) operating across the PlausiDen ecosystem. **Site repos are read-only by default**; substrate repos accept writes only through the typed-extension surfaces.

> Authored to close `#164 [substrate-discipline-v10]`. Companion to `SUBSTRATE_DISCIPLINE.md` (the load-bearing doctrine), `docs/CAPABILITY_REQUEST_WORKFLOW.md` (how substrate extensions land), and `docs/CLAUDE_SESSION_AUDIT.md` (the audit of compliance).

> The policy is documented here + enforced at the infrastructure layer via branch protection + CODEOWNERS + per-repo write-access scoping. Paul applies the GitHub settings; this doc records the canonical policy so it survives operator handoffs.

---

## The rule

**Default access posture for any AI agent in the PlausiDen ecosystem: read-only on site repos; write through typed surfaces on substrate repos.**

Concretely:
- Site repos (consumers — e.g. ProsperityClub, Northbrook, third-party customer sites): **read-only**. Agents may build, render, screenshot, audit; they cannot push hand-authored content.
- Substrate repos (PlausiDen-Forge / -Loom / -Crawler / -Annotator / -CMS / -Canon): **write-allowed but only through typed extension paths** (Loom primitive additions, Forge phases, CMS schema, *-core types). Per `[[substrate-only-path]]`: every gap is a substrate change.
- Doctrine repos (PlausiDen-Meta / PlausiDen-AVP-Doctrine): **write-allowed for declared doctrine edits**; doctrine changes go through PR + dual-sign per `VERSION_DISCIPLINE.md` § Migration registry.
- LFI repos (PlausiDen-LFI / Forge-LFI): **PR-only** — never direct push to main. The dedicated LFI Claude merges. Per memory `[[lfi-out-of-scope-for-this-instance]]`.
- Sacred.Vote source: **never touched** by this Claude instance. A separate Claude owns it; this instance may build a Forge static approximation of the public site but never modifies the real repo.

---

## Repo access matrix

| Repo | Class | Default access | Notes |
|------|-------|----------------|-------|
| `PlausiDen-Meta` | meta-doctrine | write (PR) | Axiom-floor edits require formal review |
| `PlausiDen-AVP-Doctrine` | meta-doctrine | write (PR) | Rule + trait + orientation + mapping additions |
| `PlausiDen-Canon` | infrastructure | write (PR) | Canonical type changes are platform-wide |
| `PlausiDen-Forge` | substrate | write (direct or PR) | Substrate-correct extension only |
| `PlausiDen-Loom` | substrate | write (direct or PR) | Same |
| `PlausiDen-Crawler` | substrate | write (direct or PR) | Same |
| `PlausiDen-Annotator` | substrate | write (direct or PR) | Same |
| `PlausiDen-CMS` | substrate | write (direct or PR) | Same |
| `PlausiDen-LFI` | LFI | **PR-only** | Dedicated LFI Claude merges |
| `Forge-LFI` | LFI | **PR-only** | Same |
| `Sacred.Vote` source | external | **never** | Separate Claude instance owns it |
| Customer / site repos (ProsperityClub, Northbrook, customer-specific) | site | **read-only by default** | Build artifacts only; never hand-author content |

---

## Why read-only on site repos?

Two reasons compound:

1. **Substrate-only-path enforcement.** Per `[[substrate-only-path]]`: hand-authored HTML / CSS / JS in site repos is forbidden — every gap is a substrate change. The simplest way to enforce that mechanically is to deny write access to site repos by default, forcing all changes through Forge build artifacts (which go through the typed pipeline).

2. **Operator authority preservation.** Site repos belong to operators. Even within an organization, the operator owning a tenant's site retains authority over its content. AI agents help build + deploy + audit the site; they don't make site-content decisions unilaterally.

The exception is **substrate repos** (Forge / Loom / etc.) — these are platform-shared by their nature, and the substrate-only-path discipline guards quality. Doctrine repos take the strongest review.

---

## Branch protection reference settings

Paul applies these settings via the GitHub repo settings UI. The values below are the canonical reference; the actual settings supersede this doc when they diverge.

### Site repos (e.g. customer-specific deployments)

```yaml
# Settings → Branches → main
require_pull_request_reviews:
  required_approving_review_count: 1
  dismiss_stale_reviews: true
  require_review_from_code_owners: true

restrict_pushes:
  enabled: true
  push_allowances:
    users: []   # empty — only PRs land
    teams: []
    apps: []

enforce_admins: true   # operator can't bypass without explicit policy override

required_status_checks:
  strict: true
  contexts:
    - substrate-discipline   # forces forge audit phantom_button + doctrine check + bypasses
    - forge-build            # strict mode, zero findings
```

The CODEOWNERS file in each site repo names the operator as the sole code owner for the `**` glob.

### Substrate repos (Forge / Loom / Crawler / Annotator / CMS)

```yaml
require_pull_request_reviews:
  required_approving_review_count: 1
  dismiss_stale_reviews: true

restrict_pushes:
  enabled: true
  push_allowances:
    users: [paul]   # operator may emergency-push; CI still gates merges to main
    teams: []
    apps: []

enforce_admins: false   # operator override available for emergency response

required_status_checks:
  strict: true
  contexts:
    - substrate-discipline
    - determ-baseline
    - backcompat-matrix
    - forge-audit
```

### LFI repos

```yaml
require_pull_request_reviews:
  required_approving_review_count: 1
  require_review_from_code_owners: true

restrict_pushes:
  enabled: true
  push_allowances:
    users: []   # ALWAYS PR-only — never direct push
    teams: []
    apps: []

enforce_admins: true   # no override; LFI surface stays disciplined
```

CODEOWNERS in LFI repos names the LFI-dedicated Claude instance as the sole owner. This Forge-side Claude instance can open PRs but not merge.

### Sacred.Vote source

This instance's access: **read-only at most**, ideally **no access**. The repo's owner is the Sacred.Vote-dedicated Claude. Paul ensures GitHub permissions reflect this scope.

---

## Agent-side compliance

Per the cron-loop preamble's hard constraints + memory `[[git-ops-run-as-paul]]`:

- All git operations run as `sudo -u paul git -C <path>`. Root-edited files get `chown paul:paul` before staging.
- Git operations on LFI repos always go through PR (`branch + commit + push + open PR`, never direct push to main).
- Destructive operations (force-push, reset --hard, branch delete) require explicit paul approval.
- Per memory `[[no-meta-narration]]`: agents don't announce what they're about to do; they just do or skip.
- Per memory `[[ceo-mode]]`: agents don't menu paul; pick + execute. Reserve confirmation for irreversible ops.

CI guardrails reinforce the policy at the infrastructure layer:

- `substrate-discipline.yml` — gates substrate-correct behavior on Forge PRs.
- `determ-baseline.yml` — refuses AI imports outside `forge-critic`.
- `backcompat-matrix.yml` — asserts renderability guarantee.
- CODEOWNERS — surfaces required reviewers per repo class.

---

## Refusing access escalation

If an AI agent finds itself needing write access to a site repo that the policy says is read-only:

1. **Stop.** That's an access-control violation in waiting.
2. **Identify the substrate gap.** Site changes that need write access typically indicate the substrate doesn't support what the operator wants.
3. **File a capability request** in the appropriate substrate repo per `docs/CAPABILITY_REQUEST_WORKFLOW.md`. The operator approves the substrate extension; the substrate ships it; the site picks it up via build.
4. **Never route around the policy.** Per `[[substrate-only-path]]`: there's no "just this once" path. If genuinely emergent: declare a substrate-bypass via the heavyweight register workflow (per `forge bypasses`).

---

## Audit cadence

The policy is audited via:

- `docs/CLAUDE_SESSION_AUDIT.md` periodic re-runs (the source-grep verifies substrate-correct behavior).
- `forge bypasses` cross-references source-tagged bypasses against the register.
- `forge doctrine check` ensures every cited rule resolves.
- GitHub branch-protection audit logs (operator-side).

Discrepancies surface as either:
- Strict findings in CI (mechanical refusal).
- Audit-doc updates (when CI catches something the audit method missed).

---

## Cross-references

- `SUBSTRATE_DISCIPLINE.md` (AVP-Doctrine) — Rule 0, the substrate-only-path doctrine.
- `docs/CAPABILITY_REQUEST_WORKFLOW.md` — substrate-extension workflow.
- `docs/CLAUDE_SESSION_AUDIT.md` — periodic compliance audit (#153 + #162).
- `docs/AI_AUDIT.md` — AI-assuming-code audit (#188).
- `PLAUSIDEN_ECOSYSTEM.md` — cross-repo orientation map.
- `.github/workflows/substrate-discipline.yml` — CI gate.
- Memory: `[[substrate-only-path]]`, `[[lfi-out-of-scope-for-this-instance]]`, `[[git-ops-run-as-paul]]`.
