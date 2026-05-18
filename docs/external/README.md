# PlausiDen documentation

External-customer documentation for the PlausiDen platform —
Forge + Loom + Crawler + Annotator + CMS.

Organised per the [Diátaxis framework](https://diataxis.fr/) —
four tiers, each serving one purpose:

| Tier | Purpose | When you need it |
|---|---|---|
| **[Tutorials](./tutorials/)** | Learning-oriented; a guided walkthrough that teaches the basics by doing | First time. You want to learn the platform. |
| **[How-to guides](./how-to/)** | Task-oriented; concrete recipes for specific problems | You know the platform but need to do a specific thing. |
| **[Reference](./reference/)** | Information-oriented; the canonical descriptions of every typed surface | You need the exact shape of a config field or API. |
| **[Explanation](./explanation/)** | Understanding-oriented; the why behind the design | You want to understand the trade-offs and choices. |

## Where to start

* **New to PlausiDen** → start with [Tutorial 01: Your first site](./tutorials/01-first-site.md).
* **Migrating an existing site** → see [How-to: import from WordPress](./how-to/).
* **Looking up a config field** → [Reference](./reference/).
* **Wondering "why is it built this way?"** → [Explanation](./explanation/).

## Internal vs external docs

Internal architecture docs (FORGE_VISION, ARCHITECTURE_PRINCIPLES,
PLATFORM_ROADMAP, etc.) live in the parent `docs/` directory.
Those describe the platform's invariants for contributors. The
docs here describe the platform's behaviour for *operators*.

## Conventions

* Every page links to relevant typed surface (Rust crate docs
  via `cargo doc`) for the source-of-truth.
* Examples use the platform's reference defaults; substitute
  your own values where indicated.
* Slugs are kebab-case; commit-IDs in examples are anonymized
  to `abcdef…`.
