# FORGE_GAPS — Stripe pricing page rebuild

Task #660 / T73. First public site rebuilt as a Forge CmsPage. Each
gap below is a Forge component-variant that the rebuild *needed* and
that the typed `CmsSection` enum does NOT yet have. Each one earns a
T70 sub-ticket so the registry of missing variants accumulates as the
rebuild list grows.

## Gaps surfaced

1. **`Pricing` section variant** — three (or more) priced-tier cards
   in a row, each with: tier name, price per unit, headline rate,
   bulleted included-features list, and a primary CTA. The rebuild
   collapses the cards into `Group` blocks with prose explanations,
   which loses the visual scan-ability of priced columns.

   Proposed shape:
   ```rust
   Pricing {
       columns: Vec<CmsPricingColumn>,
   }
   pub struct CmsPricingColumn {
       pub name: String,
       pub price: String,        // formatted; renderer doesn't compute
       pub unit_suffix: String,  // "per transaction", "/month", etc.
       pub headline: String,
       pub features: Vec<String>,
       pub cta: Option<HeroCta>,
       pub featured: bool,       // for the "popular" callout
   }
   ```

2. **Country picker + currency-aware pricing** — Stripe's pricing
   page lets the visitor pick country → all rates update. Forge has
   no concept of view-side interactivity beyond static HTML. Options:
   (a) ship as Forge `Dynamic` mode (T432, just shipped) and emit a
   small piece of vanilla JS, (b) introduce a typed `CountryPicker`
   widget that the SPA runtime knows how to bind to a JSON
   `pricing-table.json` data file. Defer until a second
   country-switching site lands.

3. **Savings calculator** — input monthly volume → see discount
   tier. Same widget-system question as #2. Same defer reason.

4. **Comparison table** — Stripe's page has a feature × tier matrix
   that's much richer than `KvPair`. Needs typed `ComparisonTable`
   with explicit columns + per-cell content (text, ✓, ✗, or "custom").

5. **Quote + logo wall** — "Trusted by Shopify, Lyft, Postmates…"
   social-proof bar. No `LogoWall` variant; the rebuild omits.

6. **FAQ accordion** — Stripe has a Q&A section at the bottom. No
   `Faq` variant. Would need disclosure semantics for a11y.

7. **Footer site-map** — multi-column footer with links. The
   page-shell footer currently takes a single `<p>` blob. Needs
   typed `FooterSitemap`.

## Visual-diff regression suite

Captured via Forge's CrawlPhase against the rendered page vs the
saved-screenshot baseline at `examples/stripe-pricing/baseline.png`
(NOT included yet — needs a separate `forge crawl --capture-baseline`
step that the rebuild pipeline will land in T660 cycle 2). The diff
is expected to be high (~50%) because of the gaps above; the GOAL is
to drive the diff DOWN by adding component variants, not to
artificially match by hacking the CMS data.

## Next rebuilds in the T660 series

Per the task description: Linear, Vercel, GitHub, Apple product page,
NYT, ProductHunt. Each will surface a different gap set. Expectation
is that after ~4 sites the gap-list deduplicates and the CmsSection
enum stabilizes around 15–20 variants.
