# Mom's site (ProsperityClub) — routing decision (2026-05-27)

> **⚠️ SUPERSEDED 2026-05-27 (same day).** Per owner directive
> issued ~hours after this doc shipped: substrate must grow to
> handle ProsperityClub's needs in Rust rather than routing
> around. The original Option 1 decision is reversed; the
> substrate-growth path is now active.
>
> Substrate work completed in response (#406-#412):
> - **#406** tenant brand palette — substrate already supported per-tenant hex; mom's forge.toml updated to her real #733635 red / #257C4E green / #DBA830 gold (captured from live site via Crawler reference-capture)
> - **#407** button hover-color transition — substrate-general CSS-var override mechanism (`--loom-color-button-primary-hover` etc.); any tenant can opt in to red→green or any other transition
> - **#408** HeroSlideshow primitive — new CmsSection variant with pure-CSS auto-rotation, per-slide eyebrow/title/lede/CTA overlay, arbitrary image src (.webp/.jpg/.png/.avif)
> - **#410** HeroBackground default → None — neutralizes the substrate-default-band gradient bleed per #360 doctrine
> - **#411** ProsperityClub tenant content — palette + 4-slide carousel authored against the live site
>
> Reframe-doctrine update: tenant-specific brand identity NOW
> belongs in `tenant_style` + substrate-general primitive surfaces,
> NOT outside Forge. The `[[dont-pixel-reproduce-outside-band]]`
> doctrine still applies for sites whose vocabulary the substrate
> can't express; ProsperityClub turned out to be expressible once
> the gaps (per-tenant palette + hover-color + slideshow + gradient-
> default neutralization) were filled.
>
> The original Option-1 analysis below is preserved for context.

## Decision (superseded)

**Route ProsperityClub to a non-Forge build path (Option 1).**

The Forge attempt is documented at `/home/paul/projects/ProsperityClub/`
(cms/, forge.toml, backends.toml, reports/, static/). The site
attempted a build via Forge as part of the 2026-05-20 reframe-
investigation arc.

The attempt is suspended; ProsperityClub will not ship through
Forge. The cms/ + forge.toml + backends.toml artifacts remain on
disk for reference but are no longer the canonical build path.

## Why out-of-band

Per `[[substrate-reframe-2026-05-21]]` doctrine + the 5-reference-
site frame (`docs/SUBSTRATE_REFERENCE_SITES_2026_05_27.md`),
ProsperityClub's needs sit outside the substrate's currently-
calibrated band:

1. **Single-tenant business shape**: ProsperityClub is a personal
   business site for paul's mother, not a generalizable substrate
   pattern. The Forge substrate is built for repeatable shapes
   across many tenants; per `prim-012`, single-tenant-specific
   accommodations belong in tenant-corpora, not the substrate.

2. **Content-led, not structure-led**: the operator (mom) cares
   about the words + photos + day-to-day editorial — not the
   substrate-shape. Forge's structural-fingerprint + audit
   discipline is overhead for this use case.

3. **Substrate-vocabulary mismatch**: per the
   `[[dont-pixel-reproduce-outside-band]]` doctrine, attempting
   to express ProsperityClub through Forge's primitive vocabulary
   produces low-fidelity results without growing the substrate
   in a useful direction.

4. **Reframe finding empirically supported**: this is exactly the
   case the substrate reframe identified — operators reaching for
   Forge to build sites it wasn't shaped for. The correct response
   is to route around (option 1), not to bend the substrate (option 2)
   nor to ship sub-par output through Forge anyway (option 3).

## What gets retained

- **Content**: any cms/*.json prose that's substantive remains as
  raw content the operator can hand-port to whatever non-Forge path
  ships ProsperityClub.
- **Brand assets**: any logo / photos / typography choices in the
  attempt directory remain available.
- **Domain + DNS**: ProsperityClub.com (or whatever the canonical
  domain is) stays under paul's control; routing changes happen
  at infrastructure level.

## What gets discarded

- **forge.toml + backends.toml + phases.toml**: no longer the build
  config; the non-Forge build won't read these.
- **The fingerprint registry entry** (if any was committed) for
  ProsperityClub: marked stale; no longer authoritative.
- **Any Forge audit-finding follow-ups** specifically for
  ProsperityClub: closed without action.

## Non-Forge build paths (operator decides)

The substrate is opinion-free about which non-Forge path to use.
Reasonable options:

- **Static-site generator** (Eleventy / Astro / Hugo): straight-
  forward HTML build with a small CMS-like content tree.
- **WordPress / Ghost**: full editorial CMS if mom prefers an
  in-browser editor.
- **Hand-authored static HTML**: simplest path; one author, low
  page count, occasional updates.
- **Squarespace / Wix / Cargo**: hosted platforms; trade-off is
  zero technical maintenance for vendor lock-in.

Choosing among these is paul's call (with mom's input). This task
just removes Forge from the candidate list.

## Mapping to substrate doctrine

- **`[[dont-pixel-reproduce-outside-band]]`**: This decision IS the
  operational form of that doctrine.
- **`[[substrate-only-path]]` does NOT apply here**: substrate-
  only-path is the rule for sites that *belong in* the substrate.
  Per the reframe, ProsperityClub is the case where the rule's
  premise (substrate fit) fails.
- **`prim-012`**: single-tenant accommodations belong in tenant-
  corpora not primitives; per this rule, ProsperityClub couldn't
  motivate substrate change even if we wanted to keep it in Forge.

## Follow-ups (out of substrate scope)

- **Operator decision**: paul + mom pick the non-Forge build path.
- **Domain routing**: re-point DNS / CDN to the new build's
  hosting if/when it lands.
- **Content migration**: hand-port any substantive prose from the
  cms/ tree to the new build's content schema.

## Status

This task closes the loop on the routing question. The substrate
reframe → 4 audit docs → 5 default-fragmentation pools → 11
paired workflows → 6-layer reframe stack → 4 accessibility
modules → 1 brick library → 1 exemplar library → 1 generation-
plan audit work is now complete with respect to deciding what
DOESN'T belong in the substrate, of which ProsperityClub was the
canonical case.

The substrate is in good shape. The opportunity to grow it for
band-mismatch sites stays open via the gap registry (#372) if
future operators register similar needs across multiple
independent tenants.
