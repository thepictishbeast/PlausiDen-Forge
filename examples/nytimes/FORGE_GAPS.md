# FORGE_GAPS — nytimes.com rebuild

Task #660 / T73 cycle 4. First news-media-vertical rebuild
(previous three were dev-tools: Stripe pricing, Linear, Vercel).
Reference: nytimes.com homepage.

## Why this rebuild matters for the dedup theory

The first three rebuilds were all dev-tools marketing sites, so
the "LogoWall surfaces 3-of-3" dedup signal was vertical-specific
by construction. Adding a news-media site tests whether the
backlog priorities hold across categories or are dev-tools artefacts.

## Dedup table after cycle 4

| Variant       | Stripe | Linear | Vercel | NYT  | Surfaces | T70 priority |
|---------------|--------|--------|--------|------|----------|--------------|
| LogoWall      |  ✅    |  ✅    |  ✅    |  ❌  | 3-of-4   | P1 (shipped, marketing-only) |
| Pricing tiers |  ✅    |  ✅    |  ✅    |  ❌  | 3-of-4   | P2 (queued, marketing-only) |
| Quote         |  ✅    |  ✅    |  ✅    |  ✅  | 4-of-4   | **P2 LOCKED (shipped) — universal** |
| Code block    |  ✅    |  ❌    |  ✅    |  ❌  | 2-of-4   | P3 (shipped) |
| ArticleCard   |  ❌    |  ❌    |  ❌    |  ✅  | 1-of-4   | NEW: MED (news-only so far) |
| Live-blog     |  ❌    |  ❌    |  ❌    |  ✅  | 1-of-4   | NEW: LOW (kv_pair style hint) |

**Key finding:** Quote is the only variant to hit 4-of-4. It is
now load-bearing for ALL marketing + content site rebuilds. P2
priority confirmed; T660 P2 implementation (shipped earlier this
session) is the right call across the board.

LogoWall and Pricing fell off in news-media (NYT doesn't ship
customer logos or pricing tiers on a news homepage). They are
marketing-vertical-specific universals — still load-bearing for
SaaS/marketing sites, less so for content sites.

ArticleCard is a NEW MED-priority gap that didn't surface in
ANY of the dev-tools rebuilds. Needs a second news/publication
rebuild (cycle 5: maybe Vox or theatlantic.com) to confirm it's
universal across the news vertical before bumping to T70 HIGH.

## Gaps surfaced (logged in the CmsPage content itself)

* `GAP-T660-NEW3` — `ArticleCard` variant (headline + dek + byline
  + dateline + thumbnail). New MED priority. Awaits 2nd-news-site
  confirmation.
* `GAP-T660-NEW4` — Opinion lede pattern. **Resolved by existing
  T660 P2 `Quote` variant** — no new gap.
* `GAP-T660-NEW5` — Live-blog timestamp pattern. Additive `style:
  timeline | table | grid` hint on existing `kv_pair`, not a new
  variant. LOW priority, awaits 2nd surface.

## Phase 2 (queued)

* Build a second news/publication rebuild (cycle 5: theatlantic.com,
  vox.com, or a magazine like wired.com) and re-run the dedup
  table. If ArticleCard hits 2-of-2 news rebuilds, bump to T70 HIGH.
* If Live-blog hits 2 surfaces, ship the `kv_pair.style` hint.

## Cross-rebuild gap registry now lives in this file + the prior

* `examples/stripe-pricing/FORGE_GAPS.md` — cycles 1-3 dedup table
* `examples/nytimes/FORGE_GAPS.md` — cycle 4 (this file)

Next rebuild should update this file (not the stripe one) so the
dedup table accumulates here.
