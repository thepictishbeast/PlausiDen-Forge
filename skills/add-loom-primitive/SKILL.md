---
name: add-loom-primitive
description: Add a new Loom primitive (or variant) to the typed-substrate UI surface. Covers CmsSection variant, render emission, skin.css, manifest declaration, trait satisfactions, default-required traits.
metadata:
  tags: [loom, primitives, ui, doctrine]
  related_doctrine_rules: [prim-001, prim-002, prim-003, prim-004, prim-005, prim-006, prim-007, prim-008, prim-009, prim-010, prim-011, prim-012]
  related_traits: [MobileFriendly, RTLAware, RespectsReducedMotion, ThemeAware, Interactive, Focusable, KeyboardOperable, ScreenReaderAccessible]
---

# Add a Loom primitive

Use this skill when a site needs UI composition the existing primitive set doesn't support. Primitives are **substrate-general** — they work for every site, every theme, every audience. Site-specific shapes live in CMS content, not in primitive code (rule prim-012).

## When to invoke

Recognition signals:
- A capability-request issue (per `docs/CAPABILITY_REQUEST_WORKFLOW.md`) categorized as "Loom primitive" was accepted.
- Reviewing a site's CMS authoring intent, you can't compose the desired result from existing variants.
- A theme / brand pack needs a primitive that the pack's audience would commonly use.

Anti-signals:
- The shape is needed by exactly one site. Don't add `ProsperityClubHero` (rule prim-012). Either generalize the shape into `Hero { variant: ... }`, or use existing primitives.
- The "primitive" is actually data — that's CMS content, not a new component.
- The need is a styling tweak — extend an existing primitive's variant enum instead.

## Prerequisites

1. Read `PlausiDen-Loom/CLAUDE.md` — the canonical Loom doctrine.
2. Read existing similar primitives. Browse `crates/loom-cms-render/src/lib.rs` for the `CmsSection` enum variants and their render impls.
3. Skim `loom-tokens/src/skin.css` for the CSS layer conventions.
4. Run `forge doctrine for crates/loom-cms-render --terse` for applicable rules.

## Procedure

### 1. File the capability request

Substrate layer: "Loom primitive". Proposed contract:
- Variant enum (closed; rule prim-002)
- Slot structure (typed fields)
- Default trait set (per rule prim-001: MobileFriendly + RTLAware + RespectsReducedMotion + ThemeAware always)
- Additional traits per primitive class (Interactive → Focusable → KeyboardOperable → ScreenReaderAccessible cascade if applicable)
- Accessible-name slot if any sub-element is interactive (rule prim-008)
- Default alignment (start; centered opt-in per rule prim-009)

### 2. Add the CmsSection variant

In `crates/loom-cms-render/src/lib.rs`:

```rust
pub enum CmsSection {
    // ... existing variants ...
    YourPrimitive {
        // Typed slots — enums for variants, no open Strings.
        variant: YourPrimitiveVariant,
        // Default-start alignment per prim-009; opt-in via Option.
        align: Option<HorizontalAlign>,
        // ... typed content slots ...
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YourPrimitiveVariant {
    Default,
    Editorial,    // strip card chrome
    Minimal,      // no chrome at all
}
```

### 3. Add the render emission

```rust
CmsSection::YourPrimitive { variant, align, /* fields */ } => html! {
    section
        class={"loom-your-primitive" (variant_class(*variant))}
        data-loom-reveal
        data-align=(align_attr(*align))
    {
        // emit typed HTML — Maud or your-render-fn-of-choice
    }
}
```

### 4. Add the CSS in `loom-tokens/src/skin.css`

Follow the existing conventions:
- Default = start-align + minimal chrome (per rule prim-009)
- Use loom-tokens variables — no raw px/hex/rgb (rule prim-007)
- Logical properties only — `padding-inline-start`, not `padding-left` (rule prim-003)
- `@container` queries for responsive behavior; `@media` only at page-shell level (rule prim-004)
- `@media (prefers-reduced-motion: reduce)` gates any animation (rule prim-004 + a11y-004)

```css
/* ============================================================
 * YourPrimitive — one-line description
 * ============================================================ */
.loom-your-primitive {
  /* Default: start-aligned, no decoration */
  padding-block: var(--loom-space-6);
  text-align: start;
  /* ... */
}
.loom-your-primitive[data-align="center"] {
  text-align: center;
}
.loom-your-primitive--editorial {
  /* opt-in editorial variant */
}
```

### 5. Update the CMS schema

`cms-schema.json` is generated from Rust source. Run the schema export to refresh it:

```bash
cd PlausiDen-Loom
cargo run -p loom-bridge -- emit-schema > ../PlausiDen-Forge/cms-schema.json
```

(If `emit-schema` doesn't exist yet, that's a separate substrate gap — file a capability request.)

### 6. Update the manifest

Declare the new primitive's trait satisfactions in the manifest. Add to `manifest-core` or the per-primitive manifest declaration file (whichever the substrate has at the time of writing).

### 7. Tests

Per rule prim-010 + prim-011:

- **Visual regression baseline at 390 / 768 / 1280 viewports** — required.
- **A11y fixtures**: keyboard journey, screen-reader semantic tree, color contrast at every declared theme.
- **Property tests** at the deserialization boundary: any valid CMS content with this variant deserializes; invalid forms (e.g. unknown variant strings) fail.

```rust
proptest! {
    #[test]
    fn variant_deserializes_for_every_declared_value(v: YourPrimitiveVariant) {
        let json = serde_json::json!({"variant": v});
        let back: YourPrimitiveVariant = serde_json::from_value(json).unwrap();
        prop_assert_eq!(v, back);
    }
}
```

### 8. Ship the demo CMS section

Add an example to one of the demo sites (`cms-northbrook-backup` or a dedicated `cms-primitive-demo/`) using the new primitive. Build clean.

### 9. Update AGENTS.md / TOOLS.md if surfaced

Most primitives don't surface as separate CLI invocations, but if your primitive adds a new content type that operators need to know about, document it.

## Common pitfalls

| ❌ Don't | ✅ Do |
|---------|------|
| Use `String` for variant ("editorial"/"minimal"/"default") | Closed enum (rule prim-002) |
| Center-align by default | Start-align default; centered opt-in (rule prim-009) |
| Use `padding-left` / `text-align: left` | `padding-inline-start` / `text-align: start` (rule prim-003) |
| Use `@media (min-width: 768px)` in primitive CSS | `@container` query keyed on the primitive's container (rule prim-004) |
| Hardcode `1.5rem` / `#1A6B3C` / `4px` | Use loom-tokens vars: `var(--loom-space-4)` / `var(--loom-color-accent)` (rule prim-007) |
| Make the primitive require JS to render its core content | Render the content server-side; enhance with JS (rule prim-005) |
| Name the primitive after a site (`SkillShotsLeaderboard`) | Generalize the shape so multiple sites can use it (rule prim-012) |
| Add raw `extra_classes` props | Closed variant enum only (rule prim-006) |
| Omit the accessible-name slot on Interactive primitives | Required at construction time (rule prim-008) |
| Add the primitive without visual regression baselines | Required per rule prim-010 |

## Acceptance criteria

- [ ] CmsSection variant added with closed enum sub-types
- [ ] Render emission uses typed HTML; no raw class strings outside loom-components
- [ ] CSS uses logical properties, container queries, token variables
- [ ] Default alignment is start; centered opt-in via variant
- [ ] Default-required traits declared (MobileFriendly + RTLAware + RespectsReducedMotion + ThemeAware)
- [ ] Interactive primitives also declare Focusable + KeyboardOperable + ScreenReaderAccessible
- [ ] Visual regression baseline captured at 390 / 768 / 1280
- [ ] A11y fixtures cover keyboard + screen-reader + contrast
- [ ] Property tests at the variant deserialization boundary
- [ ] Demo CMS section + Forge build clean
- [ ] CMS schema regenerated
- [ ] AGENTS.md updated if surfaced

## Cross-references

- Loom doctrine: `PlausiDen-Loom/CLAUDE.md`
- Loom-tokens skin: `PlausiDen-Loom/loom-tokens/src/skin.css`
- CmsSection definitions: `PlausiDen-Loom/loom-cms-render/src/lib.rs`
- Existing primitives by name: `feature_spotlight`, `stat_band`, `testimonial`, `pricing`, `logo_cloud`, `cta_band`, `image_hero`, `split_hero`, `paragraph`, `pull_quote`, `kv_pair`, `code`
- Doctrine rules for primitives: `forge doctrine query --domain primitives`
