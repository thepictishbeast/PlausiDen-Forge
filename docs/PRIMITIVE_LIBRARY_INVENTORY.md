# Primitive Library Inventory

Reference catalogue of UI primitive libraries whose behavioral contracts the
Loom `CmsBlock` substrate mirrors. Tracking task: #325.

Substrate doctrine: **do not copy source code from these libraries into Loom.**
Each library has its own license; many are MIT but some are not, and even MIT
code carries attribution requirements. Loom reimplements the **behavioral
contract** (typed slots, ARIA wiring, keyboard handler stubs) in idiomatic
typed Rust. Module docs cite the upstream library as the source of the spec.

## Composition convention

Most modern unstyled-primitive libraries converge on a **slot-based composition
pattern**, popularised by Radix UI: each behavior exposes named slots
(`Trigger`, `Content`, `Portal`, `Anchor`, `Close`, …) that the consumer
composes. Loom's atomic primitives adopt this convention so the substrate's
shape is recognisable to operators arriving from the web ecosystem.

---

## Web / JavaScript libraries (unstyled headless)

| Library | URL | License | Framework | Primitive count |
|---|---|---|---|---|
| Radix UI | <https://www.radix-ui.com/primitives> | MIT | React | ~30 |
| Headless UI | <https://headlessui.com/> | MIT | React, Vue | ~10 |
| React Aria | <https://react-spectrum.adobe.com/react-aria/> | Apache-2.0 | React | ~50 |
| Base UI | <https://base-ui.com/> | MIT | React | ~25 |
| Ariakit | <https://ariakit.org/> | MIT | React | ~40 |
| Ark UI | <https://ark-ui.com/> | MIT | React, Vue, Solid | ~30 |
| Reka UI | <https://reka-ui.com/> | MIT | Vue | ~30 |
| Melt UI | <https://melt-ui.com/> | MIT | Svelte | ~30 |
| Kobalte | <https://kobalte.dev/> | MIT | Solid | ~25 |
| shadcn/ui | <https://ui.shadcn.com/> | MIT | React (Radix + Tailwind) | ~40 copy-paste components |
| Catalyst | <https://catalyst.tailwindui.com/> | Tailwind UI (commercial) | React | ~30 |
| Leptix UI | <https://leptix.dev/> | MIT | Solid | ~25 |

## Rust headless

| Library | URL | License | Framework | Primitive count |
|---|---|---|---|---|
| Dioxus Headless (`dioxus-primitives`) | <https://github.com/DioxusLabs/sdk> | MIT/Apache | Dioxus | ~15 |
| Biji UI | <https://github.com/bijibao/biji-ui> | MIT | Dioxus | ~20 |

## Native Rust GUI / full-stack

| Library | URL | License | Stack | Notes |
|---|---|---|---|---|
| egui | <https://www.egui.rs/> | MIT/Apache | Immediate-mode native | Wide widget set; behavioral patterns differ from web headless |
| Iced | <https://iced.rs/> | MIT | Elm-architecture native | Strong cross-platform; widget set covers most needs |
| Leptos | <https://leptos.dev/> | MIT | Full-stack reactive (Rust → WASM/SSR) | Closest browser-target stack to substrate |
| Dioxus UI System | <https://dioxuslabs.com/> | MIT/Apache | Full-stack (Web/Native/Mobile) | Composes with Dioxus Headless + Biji UI |
| Thaw UI | <https://thawui.vercel.app/> | MIT | Leptos | Styled-component layer atop Leptos |

---

## Behavioral patterns to canonicalise

When adapting a primitive into a Loom `CmsBlock` variant, decide once which
upstream's API shape is the substrate canon. Open questions:

1. **Slot naming**: Radix uses `Trigger / Content / Portal / Anchor / Close`.
   React Aria uses `*.Trigger / *.Popover`. Loom adopts **Radix's naming** as
   the substrate canon — most modern libraries converge on it.

2. **Composition style**: Radix exposes compound components
   (`<Dialog.Root><Dialog.Trigger /><Dialog.Content /></Dialog.Root>`). Loom's
   atomic-primitive layer mirrors this via nested `CmsBlock` variants and
   structural roles encoded in `data-loom-slot="trigger"` attributes that the
   skin cascade picks up.

3. **State machine ownership**: Radix owns the state (open/closed) inside the
   component; React Aria uses hooks that surface state to the consumer. Loom
   keeps state SERVER-AUTHORITATIVE where possible (static-first substrate),
   with progressive JS enhancement carrying interactive state on the client.

4. **Focus management**: every library handles `aria-hidden` + tab-order +
   focus-return. Loom emits the same `data-*` attributes Radix's React layer
   uses so a future Loom JS runtime can replay the same behaviors.

---

## Implementation order (proposal)

Higher-frequency primitives first; each becomes a Loom `CmsBlock` variant
in a separate PR pinned to #325.

Tier 1 — universal essentials (10 primitives):
- Dialog (modal)
- Popover (anchored content)
- Tooltip (hover hint)
- Tabs (segmented panel switcher)
- Accordion (collapsible disclosure list)
- Dropdown menu (action picker)
- Combobox (search + select)
- Toast (transient notification)
- Switch (boolean toggle)
- Slider (numeric range)

Tier 2 — composite forms (8 primitives):
- RadioGroup
- Checkbox (already exists in form-core? confirm)
- Select (single + multi)
- DatePicker
- TimeField
- ColorPicker
- NumberField
- TagInput

Tier 3 — navigation + structure (7 primitives):
- NavigationMenu
- Menubar
- ContextMenu
- Breadcrumb
- Pagination
- ScrollArea
- Resizable (split-pane)

Tier 4 — advanced (6 primitives):
- Form (field-error + submission orchestration)
- Toolbar
- ToggleGroup
- HoverCard
- Sheet (off-canvas Drawer)
- AspectRatio (already exists? confirm)

Total target: ~31 atomic interactive primitives, on top of the 9 already
shipped in PR #84 + Button in PR #86.

---

## Licensing reminder

Whatever PRs land MUST cite the source library in the module docs. Acceptable
attribution: a single comment block in the module head with library name +
URL + license short-name. Example:

```rust
//! Behavioral contract mirrors Radix UI's `Dialog` primitive.
//! Upstream: <https://www.radix-ui.com/primitives/docs/components/dialog>
//! (MIT). No source code copied — typed Rust reimplementation
//! of the slot composition + ARIA contract.
```

If a primitive shape is genuinely original to one library (no obvious
precedent), pick the cleanest licensing for citation. Avoid Tailwind UI
(Catalyst) as the canonical source — it's commercial and obligates
attribution-in-product.
