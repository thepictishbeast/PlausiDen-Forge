# PlausiDen trait catalog

Generated from the `loom-traits` crate per AVP-Doctrine `TRAIT_DAG.md`. **54 traits across 11 categories.**

Every substrate-managed entity (Loom primitive, CMS section, Forge phase, Crawler detector, *-core type) declares which traits it satisfies. Audit phases enforce category-shaped invariants + default-required-trait sets per entity class. See `TRAIT_DAG.md` for the design rationale + entity-class default tables.

---

## Visibility & Lifecycle

_6 traits._

| Slug | Variant |
|------|---------|
| `renderable` | `Renderable` |
| `visible` | `Visible` |
| `client-only` | `ClientOnly` |
| `server-only` | `ServerOnly` |
| `cacheable` | `Cacheable` |
| `streamable` | `Streamable` |

## Interaction

_5 traits._

| Slug | Variant |
|------|---------|
| `interactive` | `Interactive` |
| `focusable` | `Focusable` |
| `keyboard-operable` | `KeyboardOperable` |
| `mouse-operable` | `MouseOperable` |
| `touch-operable` | `TouchOperable` |

## Accessibility (a11y)

_5 traits._

| Slug | Variant |
|------|---------|
| `screen-reader-accessible` | `ScreenReaderAccessible` |
| `reduced-motion-aware` | `ReducedMotionAware` |
| `high-contrast-supported` | `HighContrastSupported` |
| `color-blind-safe` | `ColorBlindSafe` |
| `low-vision-supported` | `LowVisionSupported` |

## Responsive

_5 traits._

| Slug | Variant |
|------|---------|
| `mobile-friendly` | `MobileFriendly` |
| `tablet-friendly` | `TabletFriendly` |
| `desktop-friendly` | `DesktopFriendly` |
| `container-query-aware` | `ContainerQueryAware` |
| `orientation-aware` | `OrientationAware` |

## Internationalization (i18n)

_4 traits._

| Slug | Variant |
|------|---------|
| `rtl-aware` | `RtlAware` |
| `locale-aware` | `LocaleAware` |
| `number-format-aware` | `NumberFormatAware` |
| `date-format-aware` | `DateFormatAware` |

## Theming

_4 traits._

| Slug | Variant |
|------|---------|
| `theme-aware` | `ThemeAware` |
| `color-scheme-picked` | `ColorSchemePicked` |
| `dark-mode-first` | `DarkModeFirst` |
| `amoled-optimized` | `AmoledOptimized` |

## Security

_5 traits._

| Slug | Variant |
|------|---------|
| `csp-compatible` | `CspCompatible` |
| `sri-verified` | `SriVerified` |
| `nonce-aware` | `NonceAware` |
| `origin-isolated` | `OriginIsolated` |
| `no-eval` | `NoEval` |

## Sovereignty (PSA — privacy / security / anonymity)

_6 traits._

| Slug | Variant |
|------|---------|
| `anonymous` | `Anonymous` |
| `private` | `Private` |
| `local` | `Local` |
| `ephemeral-by-default` | `EphemeralByDefault` |
| `tor-compatible` | `TorCompatible` |
| `offline-capable` | `OfflineCapable` |

## Performance

_5 traits._

| Slug | Variant |
|------|---------|
| `carbon-budgeted` | `CarbonBudgeted` |
| `lcp-safe` | `LcpSafe` |
| `cls-stable` | `ClsStable` |
| `bundle-size-bounded` | `BundleSizeBounded` |
| `lazy-loadable` | `LazyLoadable` |

## Reliability

_4 traits._

| Slug | Variant |
|------|---------|
| `property-tested` | `PropertyTested` |
| `fuzz-tested` | `FuzzTested` |
| `regression-fixtured` | `RegressionFixtured` |
| `fails-closed` | `FailsClosed` |

## Discipline

_5 traits._

| Slug | Variant |
|------|---------|
| `doctrine-cited` | `DoctrineCited` |
| `substrate-native` | `SubstrateNative` |
| `no-site-specific` | `NoSiteSpecific` |
| `manifested` | `Manifested` |
| `versioned` | `Versioned` |

---

## Default-required trait sets

Per AVP-Doctrine rule `prim-001` + `TRAIT_DAG.md` § Default-required traits per entity class:

### Loom primitive (Visible lineage)

- `mobile-friendly` (Responsive)
- `rtl-aware` (Internationalization (i18n))
- `reduced-motion-aware` (Accessibility (a11y))
- `theme-aware` (Theming)
- `no-site-specific` (Discipline)
- `manifested` (Discipline)
- `versioned` (Discipline)
- `doctrine-cited` (Discipline)

### Loom primitive (Interactive lineage — additional)

Cascades onto the Visible set above when the primitive declares Interactive.

- `focusable` (Interaction)
- `keyboard-operable` (Interaction)
- `screen-reader-accessible` (Accessibility (a11y))

---

## Cross-references

- [TRAIT_DAG.md](../../PlausiDen-AVP-Doctrine/TRAIT_DAG.md) — design rationale
- [N_ORIENTATION_SUBSTRATE.md](../../PlausiDen-AVP-Doctrine/N_ORIENTATION_SUBSTRATE.md) — companion orientation system
- [MAPPING_TABLES.md](../../PlausiDen-AVP-Doctrine/MAPPING_TABLES.md) — cross-orientation mappings that drive default-required selection
- [VERSION_DISCIPLINE.md](../../PlausiDen-AVP-Doctrine/VERSION_DISCIPLINE.md) — trait lifecycle + additive change classification
- Source: `crates/loom-traits/src/lib.rs` — typed enum source of truth
