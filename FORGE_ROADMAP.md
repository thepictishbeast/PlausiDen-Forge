# Forge / Loom / CMS / Crawler — improvement loop roadmap

> 60 tasks ordered by priority. Owner directive 2026-05-04: keep
> looping. Each iteration of the loop picks the top unstarted
> task, reads relevant code line-by-line, adds debug logging,
> implements, runs the full forge build + crawler audit, fixes
> any findings, pushes to GitHub, marks done, surfaces what was
> learned. If crawler finds nothing, the *detection* is what's
> broken; tighten it.
>
> Status flags: ✅ done · 🚧 in flight · ⏳ next · 📋 queued

## Crawler — make absence meaningful (10)

1. ✅ **Detect missing/broken CSS.** A page that loads CSS but
   styles don't apply (cache, CSP block, selector miss) should
   trigger a "stylesheet load count = N but computed style on
   `body { background-color }` is the user-agent default."
2. 📋 Crawler reports `"checked N categories, found 0 issues
   in K"` so absence = positive signal, not silent gap.
3. 📋 Detect render-blocking resources slowing FCP.
4. 📋 Detect layout shifts (CLS) live, not just static.
5. 📋 Detect long-tasks blocking main thread > 50ms.
6. 📋 Detect visible text with effective contrast < 4.5:1
   computed at runtime (axe is static — runtime catches
   dynamic-color bugs).
7. 📋 Detect off-screen scroll-bound content not loading
   (lazy-load failures).
8. 📋 Detect tap targets < 44×44 px on mobile viewport.
9. 📋 Detect viewport overflow on each breakpoint
   (horizontal scroll appearing).
10. 📋 Detect text overflowing its container at small viewports.

## Forge — phases (15)

11. 📋 `forge contrast` — compute every (color, bg) token pair
    in skin.css and fail any < 4.5:1 in either theme.
12. 📋 `forge motion` — fail if any animation lacks
    prefers-reduced-motion fallback.
13. 📋 `forge schema` — JSON-Schema validate forge.toml +
    backends.toml + every CMS page TOML.
14. 📋 `forge keyboard-trap` — Crawler-driven Tab cycle that
    confirms focus can reach + escape every interactive.
15. 📋 `forge i18n` — flag any hardcoded English string in
    HTML that should come from CMS.
16. 📋 `forge link-check` — every `<a href>` returns 2xx
    (internal + external).
17. 📋 `forge sri` — emit SRI hash for every linked asset.
18. 📋 `forge minify` — minify HTML/CSS/JS in dist/, gzip + brotli
    pre-compress, content-hash filenames.
19. 📋 `forge hash-bust` — generate `<filename>-<8charhash>.<ext>`
    + rewrite all references.
20. 📋 `forge sitemap` — emit sitemap.xml + robots.txt from CMS
    page list.
21. 📋 `forge structured-data` — generate JSON-LD per
    BlockKind that has Schema.org mapping.
22. 📋 `forge security-headers` — emit Caddy/Nginx config
    snippet with CSP nonce, HSTS, COOP/COEP.
23. 📋 `forge debug-log <level>` — set verbosity; logs every
    phase to `reports/debug-<ts>.log`.
24. 📋 `forge --watch` — rebuild on Loom/CMS/static change
    + WebSocket live-reload.
25. 📋 `forge build --mode dynamic|static|hybrid` — three
    output shapes; same CMS source.

## Loom — design system (15)

26. 📋 Add `<picture>` / `<source>` typed component for
    multi-format images (avif, webp, jpg fallback).
27. 📋 Add `<video>` typed component with autoplay-muted
    governance + captions slot + poster.
28. 📋 Add Avatar with image source slot (currently
    decorative-only gradient).
29. 📋 Add DataTable typed primitive (sortable, paginated,
    sticky header, mobile = card-stack).
30. 📋 Add CommandPalette (cmd-k) typed primitive.
31. 📋 Add Pagination, Tabs (real keyboard-handled), Accordion
    (details/summary), Tooltip (positioning logic), Popover.
32. 📋 Add ProgressBar + Spinner (both honor reduced-motion).
33. 📋 Add Toast queue (announces to aria-live).
34. 📋 Move every `hsl(...)` literal in skin.css decoration
    rules to `--comp-*` tokens so plugin authors override.
35. 📋 Document every `--comp-*` token in
    `loom-tokens-comp.json` + a docs page.
36. 📋 Migrate component-base classes (`.loom-card`,
    `.loom-grid`, `.loom-listrow`) so variants extend via
    `data-shape="..."` instead of duplicating.
37. 📋 Print stylesheet (`@media print`) — receipts, share-by-
    email flows render cleanly on paper.
38. 📋 More themes: `nord`, `dracula`, `solarized-light`,
    `solarized-dark`, `gruvbox`.
39. 📋 More fonts: load self-hosted Inter / JetBrains via
    woff2 + `font-display: swap`.
40. 📋 RTL support — `dir="rtl"` flips inline directional
    properties (margin-inline-start etc. used everywhere).

## CMS (10)

41. 📋 New BlockKind: BattleCard, LeaderRow, StatGroup,
    LiveBadge, BadgeCard, VoteRow, KvPair, AvatarCard.
42. 📋 `cms-render` crate: Block → loom-components Markup.
43. 📋 `cms section add <site> <page> --kind <kind> --field …`
    CLI for non-TOML editing.
44. 📋 `cms publish-page` requires reviewer signature
    (Ed25519 — leverages existing audit_log).
45. 📋 `cms i18n add <site> <locale>` — locale slot on every
    text field; per-locale fallback chain.
46. 📋 `cms upload <site> <file>` — image-optim pipeline
    (resize, AVIF + WebP encode).
47. 📋 `cms diff <site> <prev-snapshot>` — show what changed
    between two snapshots of the same site.
48. 📋 CMS schema in JSON-Schema so non-Rust consumers
    validate.
49. 📋 CMS audit log surfaced via `cms list-audit <site>`.
50. 📋 CMS-driven nav menu (replaces hand-written nav HTML
    in pages).

## SkillShots PoC + cross-cutting (10)

51. 📋 Convert ALL hardcoded HTML pages to CMS-rendered.
52. 📋 Add real loading-state skeletons on every panel +
    every card.
53. 📋 Add fade-in-up stagger animation on first feed render.
54. 📋 Replace decorative avatars with real `<picture>` once
    Avatar component lands.
55. 📋 Add Crawler journey for tablet (768) + ultrawide
    (1920) viewports.
56. 📋 Lighthouse-style perf-budget integration in `forge
    audit`: LCP < 2.5s, CLS < 0.1, FID/INP < 200ms.
57. 📋 `forge selfaudit` — Forge audits Forge: every shell
    function has a unit test, every phase has a regression
    fixture, every report.json schema validates.
58. 📋 GitHub Actions workflow: every push runs `forge build`
    + crawler audit + posts diff as PR comment.
59. 📋 Backwards-compatibility test suite: render every CMS
    site under each historical Loom version + diff result.
60. 📋 Forward-compat plan: design CMS schema upgrades to
    auto-migrate old TOML to new shape via `cms migrate`.

---

*Update one ✅ per loop iteration. When all 60 close, generate
the next 60 from the open backlog + new findings.*
