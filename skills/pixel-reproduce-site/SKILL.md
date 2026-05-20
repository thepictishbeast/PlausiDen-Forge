---
name: pixel-reproduce-site
description: Pixel-by-pixel reproduce a live site via Forge as a substrate-capability validation. Crawler screenshots → identify primitive gaps → file capability requests → implement → exercise via CMS → iterate.
metadata:
  tags: [forge, loom, crawler, capability-validation, priority-3]
  related_doctrine_rules: [build-001, prim-001, prim-006, prim-009]
  related_traits: [MobileFriendly, RTLAware, ThemeAware]
---

# Pixel-by-pixel reproduce a live site via Forge

Use this skill when paul (or the loop's Priority 3) asks for a pixel-perfect Forge reproduction of a target site. The work is a **capability test** for the substrate, not a deliverable site per se. Every gap surfaced = a substrate addition that compounds for every future site.

## When to invoke

Recognition signals:
- Priority 3 in the loop preamble: "pixel-by-pixel real-site reproductions via Forge".
- Paul says "reproduce X.com" or "make Forge match X.com pixel by pixel".
- A capability test of "can Forge handle this site shape?" is needed before a customer signs.

Anti-signals:
- Paul wants the actual content from that site for an unrelated rebuild (e.g. parked in `/home/paul/ProsperityClub/`) — that's content staging, not pixel reproduction.
- The byte-mirror approach (curl + cp) is being mistaken for substrate capability. Per `[[substrate-only-path]]`: byte-mirroring is forbidden as a capability claim. Only Forge-rendered output counts.

## Prerequisites

1. Read `[[substrate-only-path]]` doctrine — pixel reproduction is a capability test of the substrate.
2. Read `[[priority-architectural-first-and-cross-ai]]` — substrate work first; don't hand-code around gaps.
3. Run `forge doctrine query --domain primitives` to know which rules apply.
4. Identify the target site's URL.

## Procedure

### 1. Snapshot the target site

```bash
cd PlausiDen-Crawler
./target/release/crawler --journey journeys/<target>-1280.json --headless
```

Where `journeys/<target>-1280.json` is a journey JSON with `goto` + `screenshot` steps at the target site URL, viewport `{w: 1280, h: 900}`. Repeat for 768 and 390 viewports.

Screenshots land in `runs/<journey>-<timestamp>/`. Use these as the visual reference.

(Known: if Crawler hangs on chromium zombies — task #182 — work around by manually capturing screenshots via firefox-esr --headless until that's fixed.)

### 2. Inventory the composition

For each page of the target, write down the primitives needed by shape (not by site-specific name):

- "Top icon-bar with email + phone slots, left-aligned" → `icon_bar` primitive (if it exists; else file capability request)
- "Dropdown nav with section headers under each top-level link" → `dropdown_nav` primitive
- "Article body with sidebar + sticky TOC" → `article_with_sidebar` primitive

Cross-reference against existing primitives: `forge doctrine query --search "<primitive name>"` and grep `crates/loom-cms-render/src/lib.rs` for the `CmsSection` enum.

### 3. File capability requests for every gap

For each primitive that doesn't exist, file a capability request (per `docs/CAPABILITY_REQUEST_WORKFLOW.md`). The proposed contract names the typed slot structure, variant enum, default traits, and visual behavior.

**Do NOT proceed to hand-coding HTML/CSS.** Per `[[substrate-only-path]]` doctrine: hand-authored CSS / HTML / JS in site repos is forbidden. The substrate_purity phase will flag it.

### 4. Implement the capabilities

Implement each missing primitive in Loom following the `add-loom-primitive` skill. Land the primitives one at a time; each lands with its own visual regression baseline + a11y fixtures + property tests.

### 5. Author the CMS file

Once primitives exist, author `cms/<slug>.json` for each page of the target site, composing primitives that now exist. Follow the `author-cms-content` skill.

### 6. Build + deploy

```bash
forge build --mode production    # strict-clean required
rsync -a --delete static/ /var/www/dev.plausiden.com/
chown -R caddy:caddy /var/www/dev.plausiden.com/
```

### 7. Diff visually

```bash
# Crawler screenshots Forge output at same viewports
./target/release/crawler --journey journeys/forge-build-1280.json --headless

# Diff against live-site baselines
# (When the visual-diff tool exists. For now, side-by-side comparison.)
```

For each remaining divergence: classify as primitive gap (substrate work) or content gap (CMS authoring) and iterate.

### 8. Acceptance

Identical at 1280px first (desktop is least ambiguous). Then 768px. Then 390px. WCAG AA contrast at every viewport (rule a11y-003).

## Common pitfalls

| ❌ Don't | ✅ Do |
|---------|------|
| `curl https://target.com/route/ \| ... \| cp -- /var/www/dev.plausiden.com/` (byte-mirror) | File capability requests for missing primitives; build through Forge (`[[substrate-only-path]]`) |
| Add a `ProsperityClubHero` site-specific primitive | Generalize the shape into a typed variant `Hero { variant: ... }` (rule prim-012) |
| Inline `<style>` to match a target color | Add the color as a loom-tokens theme variable (rule prim-007) |
| Center-align everything by default to match the target's SaaS-style hero | Default start-align + `align: center` only on explicit primitives (rule prim-009) |
| Skip a11y review because "the target site doesn't have it either" | The substrate's commitment is WCAG AA minimum; the target's a11y failures don't excuse the reproduction (rule a11y-003) |
| Declare done when 1280px matches but mobile breaks | Per rule prim-001: MobileFriendly is a default-required trait; all three viewports must match |

## Acceptance criteria

- [ ] Crawler-captured screenshots of the target site at 390 / 768 / 1280
- [ ] All required primitives exist in Loom (file capability requests + implement)
- [ ] CMS files compose primitives — zero hand-authored HTML / CSS / JS
- [ ] `forge build --mode production` strict-clean
- [ ] Deployed to `dev.plausiden.com` via rsync from `static/`
- [ ] Forge-output screenshots taken via Crawler at same viewports as the reference
- [ ] Visual diff: identical at 1280, 768, and 390
- [ ] WCAG AA contrast at every viewport
- [ ] No site-specific primitives added (rule prim-012)
- [ ] Substrate additions (new primitives / variants) committed to Loom upstream, not vendored locally

## Cross-references

- Substrate Discipline doctrine: `PlausiDen-AVP-Doctrine/SUBSTRATE_DISCIPLINE.md`
- Capability request template: `.github/ISSUE_TEMPLATE/capability-request.yml`
- Loom primitives: `PlausiDen-Loom/loom-cms-render/src/lib.rs`
- Crawler journeys: `PlausiDen-Crawler/journeys/`
- Skill `add-loom-primitive`: for each primitive surfaced
- Skill `author-cms-content`: for each page authored
- Task #182: Crawler chromium-shell zombie investigation (workaround until fixed)
