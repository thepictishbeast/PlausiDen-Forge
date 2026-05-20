# Variation Guarantees — substrate doctrine

**Status:** load-bearing. Closes task #262. Defines the substrate's
guarantees about cross-site uniqueness, within-site consistency, and
controlled mutability — the doctrine that backs the engineering work
in tasks #231-#262.

---

## The three guarantees

The substrate makes three machine-checkable promises:

### 1. Cross-site uniqueness

No two distinct sites in the platform produce identical structured
fingerprints. Different sites = different fingerprints, full stop.

**Mechanism:** every site computes a canonical `SiteFingerprint`
(#231) over its primitives, content silhouettes, composition
rhythms, and asset distribution. The fingerprint is committed to
an append-only Merkle-chained registry (#232). At build time, the
uniqueness gate (#233) refuses any build whose fingerprint
collides with — or is within `near_duplicate_threshold` of — an
existing entry under a different `site_id`.

**Guarantee level:** machine-verifiable. The registry is signed
(Ed25519), so an external auditor can replay the chain and verify
that every site claimed to be unique was actually checked.

### 2. Within-site consistency

A site honors its declared identity — voice, mood, density, allowed
primitives, token cascade, required themes — across every page,
every build.

**Mechanism:** operators declare a `SiteIdentity` (#234) in
`forge.toml [site_identity]`. The conformance audit (#235) verifies
the actual CMS matches the declaration: forbidden primitives are
absent, voice-profile sentence-length ceilings are honored, content-
type taxonomy covers every page, required theme variants ship.

**Guarantee level:** strict-by-flag. Sites that don't declare an
identity skip the audits (back-compat). Sites that opt in commit
to the discipline; the build refuses to ship on drift.

### 3. Controlled mutability

Sites evolve over time, but evolution is auditable. Identity changes
are atomic, reviewable, and signed. Rollback is a first-class
operation.

**Mechanism (pending tasks):**
* `IdentityTransition` workflow (#238) wraps every identity change
  in an atomic commit with diff + review surface + signature.
* Identity rollback + version history (#239) makes "this is the
  identity my site shipped with at build N" a first-class query.
* Drift-prevention check (#240) refuses partial identity changes
  that would leave the site internally inconsistent.

**Guarantee level:** target. Pending tasks track delivery.

## What "structured fingerprint" means

The fingerprint hashes the SHAPE of a site, not its content:

| Component             | What's captured                                                          |
|-----------------------|--------------------------------------------------------------------------|
| Primitive occurrences | Which `CmsSection` kinds appear, with what variants, on which page       |
| Token overrides       | Which design tokens deviate from platform defaults                       |
| Content silhouette    | Per-page char-count bucket, paragraph count, list item count, headings   |
| Composition rhythm    | Per-page section count + density tier                                    |
| Asset distribution    | Image / video / interactive element counts across the site               |

What's NOT captured:
* Actual text content (would explode the fingerprint + invite content-
  fingerprinting privacy risk)
* Asset URLs (transient; substrate-irrelevant)
* Build metadata (timestamps, commit hashes)
* Operator-private identifiers

This means two sites with the same paragraph-count silhouette + same
primitives + same density across the same number of pages collide —
even if they have entirely different text. That's the point: the
substrate is enforcing that two sites can't have the same SHAPE.

## Distance vs commitment

Two operations on fingerprints:

* `commitment_hex()` — SHA-256 of the canonical bytes. 64-char lowercase
  hex. Exact-match detection: two fingerprints with the same commitment
  ARE identical under the current spec.

* `component_distance(other)` — structured hamming-ish distance. Two
  identical fingerprints have distance 0; sites with one primitive
  swapped have distance ~1; entirely different sites have distance
  far above any reasonable threshold. The uniqueness gate refuses
  builds with distance ≤ threshold (default 4).

Distance is calibrated empirically. The substrate ships a 10-reference-
site corpus (pending #252) to anchor the calibration; tests verify
that real-world dissimilar sites land above threshold.

## Per-tenant vs platform scope

The uniqueness gate has two scopes:

* **Platform scope** (`scope = "platform"`, default) — refuses any
  build that collides with ANY existing entry, regardless of tenant.
  Strongest guarantee; the platform-wide differentiation budget.
* **Tenant scope** (`scope = "tenant"`) — refuses strict-only when
  the colliding entry is in the same tenant's portfolio. Cross-tenant
  near-duplicates emit warns (informational only).

Tenant scope is for platforms hosting many sites where some cross-
tenant similarity is expected (e.g., a publishing platform with many
news sites — they share genre-driven shape). Platform scope is the
default for sovereign-substrate use.

## How operators declare identity

```toml
[site_identity]
site_id = "prosperityclub.com"
tenant_id = "plausiden"
density_preference = "dense"
allowed_primitives = ["hero_editorial", "kv_pair", "pull_quote"]
forbidden_primitives = ["hero", "feature_spotlight"]

[site_identity.voice]
tier = "editorial"
max_avg_sentence_words = 22
vocabulary_tier = "professional"

[site_identity.mood]
primary = "editorial"
drift_budget = 12

[site_identity.tokens]
max_per_page_overrides = 3
max_site_distinct_overrides = 24

[[site_identity.content_type]]
slug = "homepage"
pattern = "cms/index.json"

[[site_identity.content_type]]
slug = "blog_post"
pattern = "cms/blog/*.json"

[[site_identity.theme_variant]]
name = "light"
required = true

[[site_identity.theme_variant]]
name = "amoled"
required = true
```

Every field is optional. Empty/missing fields skip the corresponding
audit. The schema is `#[non_exhaustive]` so future axes (motion
preference, decorative budget, interactive-pattern lock) add without
breaking.

## How operators enable the gates

```toml
[uniqueness_gate]
enforce = true
# registry_path = "registry/fingerprints.jsonl"  # default
# near_duplicate_threshold = 4                   # default
# scope = "platform"                             # or "tenant"
```

The conformance audit runs automatically when `[site_identity]` is
declared. No per-phase enable flag — declaring an identity IS the
opt-in.

## Why this matters

Without these guarantees, the substrate degrades into "many sites
that all look the same." The platform's value to operators is
specifically that two PlausiDen sites are recognizably distinct,
that brand identity is enforced, that build-N+1 doesn't quietly
drift from build-N. Per memory `[[supersociety-not-easy]]`:

> Don't do what's easy, do what's best.

The easy path is "let operators ship whatever; the build always
green-lights." The substrate's commitment is the opposite — the
build refuses to ship until the variation contract holds.

## What ships today (post-2026-05-20)

* `forge_core::fingerprint::SiteFingerprint` — canonical hash + distance
* `forge_core::fingerprint_registry` — append-only Merkle-chained registry
* `forge_core::site_identity::SiteIdentity` — declarative schema
* `forge_phases::uniqueness_gate::UniquenessGatePhase` — gate phase
* `forge_phases::site_identity_conformance::SiteIdentityConformancePhase`
  — conformance audit phase
* `forge_core::diagnostic::Diagnostic` + `SubstrateAwareError` trait
  — canonical diagnostic shape per `docs/SUBSTRATE_ERRORS.md`

## What ships later in the arc

| Task | Subject                                                                  |
|------|--------------------------------------------------------------------------|
| #236 | Pattern entropy requirement audit phase                                  |
| #237 | Differentiation budget multi-dimensional check                           |
| #238 | Identity transition workflow (atomic, reviewable, attested)              |
| #239 | Identity rollback + version history                                      |
| #240 | Drift prevention: partial-identity-change refusal                        |
| #241 | Voice profile statistical audit (deterministic baseline)                 |
| #242 | Hierarchical token cascade with bounded page-overrides                   |
| #243 | Aesthetic mood lock (aggregate-measure drift detection)                  |
| #244 | Composition lineage within site (vocabulary coherence)                   |
| #245 | Composition genealogy tracking                                           |
| #246 | Provenance commitment (every decision signed)                            |
| #247 | Exhaustion tracking + auto-rebalance                                     |
| #248 | Substrate continuous self-audit                                          |
| #249 | Forced-variation reseeding cadence                                       |
| #250 | Page-type library (50-100 distinct compositional templates)              |
| #251 | Forbidden composition patterns dictionary (anti-templates)               |
| #252 | Reference-corpus statistical baseline                                    |
| #253 | Forced primitive variant distribution requirement                        |
| #254 | Composition zone constraints (per-region requirements)                   |
| #255 | Section-type quota enforcement per page-type                             |
| #256 | Per-page deviation budget                                                |
| #257 | LFI augmentation layer for HDC semantic similarity                       |
| #258 | Registry-tampering defense (Merkle chain verifier CLI)                   |
| #259 | Fingerprint-spec versioning + migration                                  |
| #260 | Quality-floor enforcement independence                                   |
| #261 | Theme variation declaration requirement                                  |

Each task is separately reviewable. The doctrine is load-bearing
but the arc lands gradually; no flag day.

## What this doctrine does NOT do

* Doesn't enforce visual distinctiveness at the pixel level — two
  sites with identical fingerprints but different palettes could
  technically pass. That gap is closed by the mood-lock (#243) +
  theme-variation (#261) gates which constrain palette + treatment.

* Doesn't replace operator judgment — the gates refuse known-bad
  shapes; positive site quality remains the operator's responsibility.
  The substrate is a floor, not a ceiling.

* Doesn't compute "is this site good" — that's a quality question
  separately gated (#260). Variation + quality are orthogonal.

## Why per-tenant corpora interact with this

Per the `[[per-tenant-corpora-doctrine]]` memory + `docs/PER_TENANT_CORPORA.md`:
tenant corpora ADD to the substrate baseline, never modify it.
Same applies here: tenant identity declarations EXTEND the
substrate's variation enforcement. A tenant cannot LOOSEN the
substrate's guarantees by declaring identity — only TIGHTEN them
(by declaring more restrictive allowed_primitives, lower max_avg_
sentence_words, etc.).

The substrate's variation guarantees are the floor; tenant identity
raises the floor, never lowers it.

## Engineering acceptance criteria

A change to the variation arc is acceptable iff:

1. `cargo test -p forge-core` + `cargo test -p forge-phases` pass.
2. The change is additive at the wire-shape level (new fields are
   `#[serde(default)]` + `#[non_exhaustive]`), OR bumps `FingerprintSpec`
   to a new variant with a documented migration path (#259).
3. Every new Diagnostic carries `Advocacy` per the
   `docs/SUBSTRATE_ERRORS.md` doctrine.
4. New tasks added to this doctrine's arc table when the change
   surfaces follow-up work.

The doctrine is load-bearing; the engineering protects it.
