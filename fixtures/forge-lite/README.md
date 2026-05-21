# Forge Lite fixtures

Minimal fixture set for the Forge Lite diagnostic.

Per `docs/SUBSTRATE_REFRAME_2026_05_21.md` § Forge Lite. Five
`ForgeLitePage` JSON files exercising distinct identity +
theme + primitive combinations from the closed lite vocabulary.

Each fixture is generic — substrate-discipline requires no
tenant names in fixtures. The five identities are placeholder
slugs (`alpha-saas`, `beta-editorial`, etc.) covering different
aesthetic registers a real platform would serve.

## Files

| File | Identity | Theme | Primitives exercised |
|------|----------|-------|----------------------|
| `alpha-saas.json` | SaaS landing | light | Hero, Heading, Paragraph, FeatureSpotlight×3, CallToAction |
| `beta-editorial.json` | Editorial piece | warm | ImageHero, Heading, Paragraph, PullQuote, Heading, Paragraph |
| `gamma-portfolio.json` | Portfolio tile grid | dark | ImageHero, FeatureSpotlight×2, LogoCloud |
| `delta-brief.json` | Minimal brief | light | Hero, Paragraph, Divider, Paragraph |
| `epsilon-cta-heavy.json` | CTA-heavy landing | warm | Hero, FeatureSpotlight×3, CallToAction, LogoCloud, Spacer×Large |

## Use

The diagnostic comparison protocol consumes these fixtures via
`crates/forge-phases/tests/forge_lite_fixtures.rs`. The test:

1. Resolves every fixture through `forge_lite_resolve::resolve`.
2. Confirms each resolves cleanly with no `LiteValidationError`.
3. Confirms each resolved `CmsPage` contains only primitives
   reachable from the 10 lite kinds (no smuggled
   `HeroEditorial` / `SplitHero` / etc.).
4. Confirms cross-fixture variance — at least three distinct
   primitive-kind sequences across the five fixtures (the
   lite surface SHOULD produce visible structural variation
   even within its narrow vocabulary).

The full diagnostic verdict (lite vs full-Forge comparison
runs) requires building each fixture as a static site, building
equivalent full-Forge sites, fingerprinting both sets, and
comparing surveillance metrics. That harness lands in a
follow-up.
