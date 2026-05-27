# PlausiDen-Forge skills

Per `[[priority-architectural-first-and-cross-ai]]` + Anthropic's skill convention: each SKILL.md captures the canonical workflow for one high-frequency task class. AI agents (Claude, Gemini, others) reading a skill before starting that task class avoid the "every session re-derives the workflow" failure mode.

Skills are typed, cross-AI-compatible (YAML frontmatter + Markdown body), version-tracked with the codebase.

## Skill set

| Skill | Use when |
|-------|----------|
| [add-forge-phase](add-forge-phase/SKILL.md) | Adding a new Forge audit phase that implements the `Phase` trait |
| [add-loom-primitive](add-loom-primitive/SKILL.md) | Adding a new Loom primitive (or variant) for sites to compose with |
| [author-cms-content](author-cms-content/SKILL.md) | Authoring `cms/*.json` for a new site or new page |
| [forge-build-site-from-brief](forge-build-site-from-brief/SKILL.md) | Building a tenant site from a written brief (paired with `forge.build_site_from_brief` MCP tool) |
| [forge-modify-site](forge-modify-site/SKILL.md) | Applying a scoped modification to an existing site (paired with `forge.modify_site` MCP tool) |
| [forge-modify-primitive](forge-modify-primitive/SKILL.md) | Modifying an existing primitive without breaking back-compat (paired with `forge.modify_primitive` MCP tool) |
| [forge-verify-content-originality](forge-verify-content-originality/SKILL.md) | Anti-reuse gate: detect verbatim content overlap with reference corpora (paired with `forge.verify_content_originality` MCP tool) |
| [forge-site-fingerprint-check](forge-site-fingerprint-check/SKILL.md) | Structural-fingerprint anti-duplicate gate against the fingerprint registry (paired with `forge.site_fingerprint_check` MCP tool) |
| [forge-reference-extraction](forge-reference-extraction/SKILL.md) | Run the deterministic URL → SiteSpec pipeline against a captured reference site (paired with `forge.reference_extraction` MCP tool) |
| [forge-substrate-gap-registration](forge-substrate-gap-registration/SKILL.md) | Register substrate-capability gaps into the canonical JSONL gap registry (paired with `forge.substrate_gap_registration` MCP tool) |
| [pixel-reproduce-site](pixel-reproduce-site/SKILL.md) | Reproducing a live site pixel-by-pixel via Forge for capability validation |
| [extend-doctrine-rules](extend-doctrine-rules/SKILL.md) | Adding new rules to the AVP-Doctrine database |

## Conventions

Each skill follows the Anthropic skill-creator schema:

```yaml
---
name: <kebab-case-slug>
description: <one-line summary>
metadata:
  tags: [<tags>]
  related_doctrine_rules: [<rule-ids>]
  related_traits: [<trait-names>]
---

# <Title>

<body — Markdown, mostly procedural, with code examples + cross-references>
```

Skills are read BEFORE the task class begins. They name:
1. **When to invoke** — recognition rules so the AI knows this skill applies.
2. **Prerequisites** — what to read first (AGENTS.md, TOOLS.md, applicable doctrine rules via `forge doctrine for <path>`).
3. **Procedure** — step-by-step, with concrete commands.
4. **Common pitfalls** — failure modes the AI tends toward + the substrate-correct alternative.
5. **Acceptance criteria** — concrete signals for "done" (tests, build green, doctrine compliance).

## Adding a new skill

1. Identify a task class that's repeated across sessions / contributors / AI agents.
2. Document the canonical workflow as a SKILL.md following the Anthropic schema.
3. Cross-reference applicable doctrine rules.
4. Link from this README.
5. Commit; future sessions consult the skill before starting that task.

The first time anyone does a particular task in the codebase, they write the skill. Future instances find it and follow it. This is how onboarding scales.
