# Per-Tenant Corpora — substrate-deepening doctrine

**Status:** doctrine + spec, paired with PRIORITY 7 of the
continuous-improvement loop and the consumer-shaped-substrate
memory cluster ([[crawler-stays-general-purpose]],
[[forge-substrate-flexible-product-opinionated]],
[[substrate-only-path]]).

This doc defines how a tenant extends the substrate's audit
phases with their own dictionaries, reference corpora, and
scaffold-default overrides WITHOUT forking Loom / Crawler /
Forge.

---

## The pressure this doctrine relieves

The substrate ships with curated lists that fit MOST sites:

* `JARGON_PHRASES` in `forge-phases/aesthetic_distinctiveness`
  (~110 entries — SaaS jargon + vague superlatives + AI-tell
  phrases + ecosystem bingo).
* `SCAFFOLD_DEFAULTS` in `forge-phases/placeholder_value_audit`
  (~35 entries — "My Site", "you@example.com", etc.).
* `VAGUE_PHRASES` in `crawler-detectors/link_text_distinguishable`
  (~7 entries — "click here", "learn more", etc.).
* Reference corpus in `loom-tokens/docs/REFERENCE_CORPUS.md`
  (~14 sites: Stripe, Linear, Vercel, GitHub, etc.).
* `BODY_LEAK_MARKERS` in `forge-phases/hunted_tier` (~12
  client-state API marker strings).

Each list is intentionally CURATED + SHIPPED with the
substrate so every consumer gets the same baseline. But two
problems emerge as the substrate scales:

1. **A given tenant ships content that the substrate doesn't
   recognize as "generic-bad"** — e.g., a financial-services
   tenant uses "compound interest" frequently and shouldn't get
   "lectured" by a jargon detector that misclassifies their
   domain-specific copy.
2. **A given tenant has its own scaffold defaults** — e.g., an
   internal CMS pre-populates `title: "Acme Internal — Untitled
   Project"` which isn't in `SCAFFOLD_DEFAULTS`. The substrate
   can't know to flag it.

Tenant-specific extensions belong in tenant-specific config —
NOT bolted into the substrate's curated lists.

## The doctrine

**The substrate ships baseline. Tenants extend via
`forge.toml [tenant_corpus]`.**

Each tenant's site root MAY include a `[tenant_corpus]`
section in `forge.toml` that augments — never replaces — the
substrate baseline. Phases that consume corpora MUST read the
substrate baseline first, then layer the tenant extensions on
top.

Layered semantics:

| Corpus | Layering |
|---|---|
| Jargon phrases | Additive (`baseline ∪ tenant`) |
| Scaffold defaults | Additive (`baseline ∪ tenant`) |
| Vague link phrases | Additive |
| Body leak markers | Additive |
| Reference corpus | Additive (tenants can register their own reference sites) |
| Density tier mappings | Replace (`tenant > baseline` for per-tenant overrides) |
| Allow-list (suppress baseline entries) | Subtractive (`baseline − tenant.suppress`) |

The substrate baseline is the floor — tenants can ADD entries +
SUPPRESS entries from the baseline but cannot REMOVE the
mechanism. A tenant that wants to suppress `placeholder_value_
audit` entirely sets `[placeholder_value_audit] enabled = false`
at the phase level, not by emptying the corpus.

## Spec — `forge.toml [tenant_corpus]`

```toml
[tenant_corpus]

# Tenant-specific jargon phrases. ADDITIVE to JARGON_PHRASES.
# Each entry is matched verbatim against body text (lowercased
# + trimmed). Use sparingly — tenants overcooking this list
# defeat the purpose.
extra_jargon = [
  "synergy with our framework",
  "the Acme advantage",
]

# Suppress baseline jargon phrases that DON'T apply to this
# tenant's domain. Each entry must match an existing entry in
# `JARGON_PHRASES` exactly; non-matches are warn-flagged at
# build time.
suppress_jargon = [
  "transform your",  # tenant runs an actual transformation business
]

# Tenant-specific scaffold defaults. Field-name + literal-value
# pairs to flag as scaffold defaults beyond the substrate
# baseline. ADDITIVE.
extra_scaffold_defaults = [
  { field = "title", value = "Acme Internal — Untitled Project" },
  { field = "brand", value = "Acme — Replace Me" },
]

# Tenant-specific vague link phrases (link-purpose detector).
# ADDITIVE.
extra_vague_link_phrases = [
  "go here",
  "this link",
]

# Tenant-specific body leak markers (hunted_tier detector).
# ADDITIVE. Use to flag client-state APIs that the substrate
# doesn't enumerate but the tenant cares about.
extra_body_leak_markers = [
  "indexedDB.open",   # hunted-tier excludes IndexedDB markers
  "FileSystem API",
]

# Density tier OVERRIDE per page-pattern. Replaces the
# substrate's default classification. Pattern is a glob over
# the cms/ relative path.
[[tenant_corpus.density_override]]
pattern = "cms/blog/*.json"
tier = "dense"          # Tenant's blog pages are dense regardless
                        # of empirical body-char count.

[[tenant_corpus.density_override]]
pattern = "cms/index.json"
tier = "sparse"         # Tenant's homepage targets sparse density.

# Tenant-specific reference sites. ADDITIVE to the substrate's
# 14-entry corpus. Used by pixel-reproduction rotation.
[[tenant_corpus.reference_site]]
url = "https://competitor.example"
tier = "comfortable"
note = "Competitor we want to match on type-density signals"

[[tenant_corpus.reference_site]]
url = "https://acme-design.example/landing-2025"
tier = "sparse"
note = "Internal team's reference for sparse marketing"
```

## Phase-side consumption pattern

Each existing phase that consumes a corpus gets a small wrapper
that reads `[tenant_corpus]` and layers it on the baseline.
Example (pseudocode for `aesthetic_distinctiveness`'s jargon
check):

```rust
fn jargon_corpus(ctx: &BuildCtx) -> Vec<&str> {
    let mut corpus: Vec<&str> = JARGON_PHRASES.iter().copied().collect();
    let tenant = TenantCorpus::load(&ctx.root);  // tolerant — None on missing/malformed
    if let Some(ref t) = tenant {
        for extra in &t.extra_jargon {
            corpus.push(extra.as_str());
        }
        // Suppression — emit a warn finding if a suppress entry
        // doesn't match an existing baseline entry (operator
        // typo).
        for sup in &t.suppress_jargon {
            if !corpus.iter().any(|c| *c == sup.as_str()) {
                // Warn about typo'd suppression entry.
            }
        }
        corpus.retain(|c| !t.suppress_jargon.iter().any(|s| s == c));
    }
    corpus
}
```

The TenantCorpus type lives in a future `forge-core::tenant_corpus`
module. Loading is best-effort — missing or malformed
`tenant_corpus.toml` does NOT fail the build; it just leaves
the baseline as-is.

## What this does NOT do

* **It does NOT replace per-PHASE config.** `[aesthetic_
  distinctiveness] strict = true` (build-blocking promotion)
  stays at the phase level. `[tenant_corpus]` only governs
  the per-phase corpora content, not the phase's threshold
  behavior.
* **It does NOT add a per-tenant Loom theme.** Tenant themes
  flow through the typed `[composition] theme = "..."` config
  + `loom-tokens` palette — see `THEME_TOGGLE_DESIGN.md`. The
  tenant-corpus mechanism is for CHECKS and CORPORA, not for
  visual design.
* **It does NOT distribute corpora across tenants.** Each
  tenant's `tenant_corpus.toml` is local to that tenant's
  site root. A multi-tenant SaaS that wants to share corpora
  across its tenants ships a Rust-level corpus module that
  the substrate phases call into — out of scope here.

## Migration plan

The forward steps to make this doctrine real:

1. **`forge-core::tenant_corpus::TenantCorpus`** — typed
   loader + parser. Best-effort, never fails the build on
   malformed input.
2. **Phase wrappers** — for each of the 5 corpus-consuming
   phases (aesthetic_distinctiveness, placeholder_value_audit,
   link_text_distinguishable [Crawler — special-cased since
   it's not a Forge phase], hunted_tier, future density_audit
   override): replace direct const-access with the tenant-
   layered corpus.
3. **CONSUMER_SHAPING_AUDIT.md update** — mark per-tenant
   corpora as the canonical mechanism for ADDING site-specific
   content-shape signals.
4. **Tenant-corpus typo warnings** — phase emits warnings when
   a tenant's `suppress_*` entry doesn't match any baseline
   entry (operator typo guard).

This doc is the spec; the implementation is the queued
follow-up work.

## Memory + cross-references

This doctrine sits alongside:

* [[crawler-stays-general-purpose]] — the Crawler must work
  for any consumer; same principle applies to corpora.
* [[forge-substrate-flexible-product-opinionated]] — substrate
  serves multiple audiences; per-tenant corpora is the
  flexibility lever.
* [[substrate-only-path]] — hand-coding sites forbidden; the
  per-tenant corpus is the substrate-shaped path to expressing
  tenant-specific check signals.
* [[consumer-shaped-substrate]] — the meta-doctrine; per-
  tenant corpora is the v1 substrate response.
