# Forge → Platform — architectural roadmap

> Companion to [`FORGE_VISION.md`](./FORGE_VISION.md),
> [`ARCHITECTURE_PRINCIPLES.md`](./ARCHITECTURE_PRINCIPLES.md),
> [`SITE_OPERATIONS.md`](./SITE_OPERATIONS.md),
> [`ENGINEERING_DISCIPLINES.md`](./ENGINEERING_DISCIPLINES.md),
> and [`COMMERCIALIZATION.md`](./COMMERCIALIZATION.md).
>
> Vision says what; principles say why; operations says what
> sites need; disciplines says what engineers need to know;
> commercialization says what changes when scope expands; **this
> doc says how the existing operator-toolkit substrate evolves
> into the commercial-grade platform without abandoning what
> works or pretending we're starting from scratch.**
>
> Three layers: **what stays** (substrate already correct),
> **what extends** (capabilities that fit naturally), **what's
> net-new** (genuinely separate subsystems). Sequencing matters
> more than per-layer detail; attempting all in parallel is how
> mature substrates die.

---

## 0. The architectural endpoint, in one paragraph

A typed neurosymbolic substrate where the CMS schema, design
tokens, primitive contracts, backend capability manifests, AI
tool definitions, audit phases, build attestations, and policy
declarations all derive from **one cryptographically-attested
manifest layer**; the Forge build pipeline is the verifier; Loom
is the typed primitive vocabulary; Crawler is the runtime
verifier; Annotator is the human-feedback channel; an MCP-style
capability layer is the AI agent interface; multi-network
publishing is a deployment-target tier; multi-tenancy is a
hardened isolation layer; and the whole thing is a single Rust
workspace with optional WebAssembly extension sandboxing for
third parties — running on infrastructure you control, sellable
as managed hosting or self-hosted, with the same code path for
both.

None of it is decorative. Each layer is a real architectural
commitment.

---

## 1. The single most important architectural decision

**Build the manifest layer first.**

This is the keystone. Without it, the pieces stay loosely
coupled and drift apart over years. With it, every layer is a
projection of the same source of truth, and the platform's
correctness is mechanical rather than vigilance-dependent.

Everything else — Wasm extension substrate, multi-tenancy
isolation, multi-network deployment, AI bridge, policy engine,
hosting infrastructure — is straightforward Rust engineering
against the manifest. The manifest is what makes it all hang
together.

**Hold the manifest discipline against every short-term pressure
to skip it.** Everything else can flex; the manifest cannot.

---

## 2. Workspace topology — per product unit, not per concept

Current shape (separate repos per concern, cross-repo path deps)
works for the operator-toolkit phase; strains at commercial
scale. The evolution: **one Cargo workspace per "product unit"**
— something you'd sell, deploy, or fork as a coherent thing.

| Workspace | Holds | Audience | Update cadence |
|---|---|---|---|
| **`plausiden-platform`** | Forge, Loom, CMS, Crawler, Annotator, platform admin surface | Commercial CMS / build platform customers | Continuous |
| **`plausiden-privacy`** | Suite (Tidy / Atrium / Purge / Sentinel) + plausible-deniability engine + MCP server + swarm | Individuals, journalists, dissidents, security-conscious | Quarterly major, monthly patch |
| **`plausiden-civic`** | Sacred.Vote, sacredvote-crypto, civic-news, zkTLS / post-quantum Belenios | Municipalities, civic-tech buyers | Per-election-cycle |
| **`plausiden-meta`** | AVP-Doctrine, OPERATING_PRINCIPLES, harvest conventions | Read-only governance every workspace imports | As-needed |

**Per workspace:**

- One Cargo workspace, shared lockfile, shared release cadence,
  shared CI.
- Cross-workspace boundaries are formal — types cross via
  published crates (semver-disciplined) or via stable wire
  formats (JSON Schema, protobuf, Cap'n Proto).
- Per-product release independence without paying per-concept
  independence overhead.

### `plausiden-platform` crate layout

```
plausiden-platform/
├── crates/
│   ├── manifest-core/        # single source of truth: types for every capability
│   ├── manifest-codegen/     # proc-macros + build.rs generators
│   ├── manifest-attest/      # Merkle chain + Ed25519 sigs (lifted from forge-core::attest)
│   ├── loom-tokens/          # design tokens (existing)
│   ├── loom-components/      # typed primitives (existing)
│   ├── loom-cms-render/      # typed render (existing)
│   ├── loom-icons/           # (existing)
│   ├── loom-bridge/          # SSH bridge for AI agent sessions (existing)
│   ├── forge-core/           # Phase trait, BuildCtx (existing, extended)
│   ├── forge-phases/         # all phases (existing, extended)
│   ├── forge-pipeline/       # type-state pipeline (Discover→Parse→Render→Audit→Deploy→Verify)
│   ├── forge-cli/            # forge binary (existing)
│   ├── forge-serve/          # dev server (existing)
│   ├── forge-replay/         # report replay (existing scaffold)
│   ├── cms-core/             # typed content, schema, audit log (existing scaffold)
│   ├── cms-storage-fs/       # filesystem adapter (existing scaffold)
│   ├── cms-storage-sqlite/   # sqlite adapter (new)
│   ├── cms-storage-pg/       # postgres adapter (new, optional)
│   ├── cms-auth-webauthn/    # admin auth (existing scaffold)
│   ├── cms-api/              # axum HTTP API for editor + read-side
│   ├── cms-admin-ui/         # maud-based admin surface
│   ├── crawler-journey/      # typed journey schema (existing)
│   ├── crawler-detectors/    # 45+ detector axes (existing)
│   ├── crawler-runner/       # chromiumoxide runtime (existing)
│   ├── crawler-report/       # wire format (existing)
│   ├── annotator-core/       # session schema (existing)
│   ├── annotator-relay/      # HTTP daemon (existing)
│   ├── annotator-bookmarklet/# WASM-built or hand-vanilla JS bundle source
│   ├── deploy-core/          # deployment orchestration types
│   ├── deploy-hetzner/       # Hetzner adapter
│   ├── deploy-onion/         # Tor onion service adapter (new)
│   ├── deploy-i2p/           # I2P eepsite adapter (new)
│   ├── deploy-ipfs/          # IPFS / IPNS adapter (new)
│   ├── tenancy-core/         # multi-tenant isolation primitives (new)
│   ├── policy-core/          # declarative policy + enforcement (new)
│   ├── extension-host/       # Wasmtime-based plugin runtime (new)
│   ├── extension-abi/        # stable WIT-defined plugin interface (new)
│   ├── lfi-core/             # consumes Neurosymbolic-Toolkit; primitives + types (NEW; see §6)
│   ├── lfi-policy/           # NeuPSL policy DSL for the platform (NEW)
│   ├── lfi-corpus/           # HDC-encoded curated patterns + per-tenant corpora (NEW)
│   ├── lfi-critic/           # Critic trait + canonical implementations (NEW; bypass-impossible seam)
│   ├── llm-adapter-anthropic/# Anthropic adapter — lives BEHIND lfi-critic (new)
│   ├── llm-adapter-local/    # llama.cpp / vLLM / Ollama — lives BEHIND lfi-critic (new)
│   ├── obs-core/             # tracing + metrics + structured events (lifted from PlausiDen-Obs)
│   ├── obs-otlp/             # OpenTelemetry exporter
│   ├── obs-loki/             # log aggregation
│   └── platform-bin/         # the actual deployable binary (axum server + worker pool)
└── apps/
    ├── platform-server/      # `plausiden-platform` daemon (multi-tenant CMS host)
    ├── platform-cli/         # operator CLI
    └── platform-installer/   # self-hosted bootstrap
```

**~30+ crates.** Sounds like a lot; isn't. The existing repos
already imply most of these — they're either (a) renames, (b)
extractions of subsystems currently living as files inside
larger repos, or (c) net-new crates for genuinely new
capabilities. **Discipline: one crate per axis-of-change.** A
detector-logic change doesn't recompile admin UI; a token change
doesn't recompile deploy adapters.

---

## 3. The manifest layer — the architectural keystone

Consolidate the partial manifests already in the codebase
(Forge phases, `backends.toml`, MCP capabilities, AVP-2
doctrine) into one typed manifest layer that everything else
projects from. This is "single capability manifest as source of
truth" applied **retroactively** to consolidate what's already
there.

### Shape

```rust
// manifest-core
pub struct PlatformManifest {
    pub version: SemVer,
    pub capabilities: Vec<Capability>,
    pub content_types: Vec<ContentType>,
    pub primitives: Vec<Primitive>,
    pub tokens: TokenSet,
    pub audit_phases: Vec<PhaseDescriptor>,
    pub policies: Vec<Policy>,
    pub ai_tools: Vec<AiTool>,
    pub deploy_targets: Vec<DeployTarget>,
}

pub struct Capability {
    pub id: CapabilityId,
    pub signature: TypedSignature,
    pub permissions: PermissionSet,
    pub audit_category: AuditCategory,
    pub ai_callable: bool,
    pub ui_hints: UiHints,
    pub network_targets: NetworkTargetSet, // Clearnet | Onion | I2P | All
    pub deprecation: Option<Deprecation>,
}
```

### Every existing concept has a place

| Existing | Projection |
|---|---|
| `backends.toml` entries | `Capability` with `ai_callable=false` + `ui_hints` from existing convention |
| Forge phases | `audit_phases` projections |
| MCP tools | `ai_tools` projections, gated by same permission system |
| Loom primitives | `primitives` entries with typed props as `TypedSignature` |
| CMS schema | `content_types` entries with field-level sensitivity tags |
| AVP-2 doctrine references | `policies` as **NeuPSL weighted rules** (see §6 — LFI is the policy engine, not a separate substrate) |
| PlausiDen-Shield access-control rules | `policies` as NeuPSL projections (lift existing patterns) |

### Codegen as projection mechanism

`manifest-codegen` is a proc-macro + build.rs combination that
consumes the manifest and emits:

- Backend handler trait stubs the developer must implement
- Typed frontend / admin-UI client code
- The `phantom_button` / `backend_coverage` / `unbuilt_route`
  phase logic (these become **manifest-derived** rather than
  hand-coded against a separate file)
- The MCP tool schema for AI agents
- The OpenAPI / GraphQL schema for the external API
- The CLI subcommand definitions
- The audit log category enums
- The permission matrix
- The capability test harness (one generated test per capability
  asserting handler + UI + permission + audit + telemetry
  coverage)

This is the substrate that makes the **whole "frontend always
matches backend, nothing falls through the cracks" guarantee
mechanical rather than disciplinary.** Existing Forge
`backend_coverage` phase becomes one of N coverage checks, all
derived from the same manifest, all enforced at build time.

### Versioning the manifest itself

**The manifest is the constitution.** Changes follow a stricter
process than changes to anything that derives from it.

- Each version signed (lifting Forge's Ed25519 attestation
  infrastructure)
- Documented migration path between adjacent versions
- Capabilities marked `deprecated` ship for N releases before
  removal
- Capabilities marked `experimental` are not stable interface
- Customers pin to a manifest version; manifest upgrades are
  explicit decisions

---

## 4. Type-state pipeline as architectural skeleton

Forge already has the type-state pipeline (`FORGE_PIPELINE=1`,
currently opt-in). **Make it canonical and extend it to span the
whole platform**, not just the build:

```rust
// forge-pipeline
pub struct Pipeline<S: PipelineStage> { /* ... */ }
pub trait PipelineStage { type Artifacts; }

pub struct Initialized;
pub struct Discovered;      // Filesystem walked, inputs catalogued
pub struct Parsed;          // CMS JSON typed, manifest typed
pub struct Rendered;        // HTML emitted, atomic writes complete
pub struct Audited;         // All phases run, findings collected
pub struct AttestedReport;  // Merkle-chained, Ed25519-signed
pub struct Deployed;        // Pushed to target(s)
pub struct Verified;        // Post-deploy verification passed
```

- Each transition: typed method that consumes previous state +
  produces next.
- Skipping stages: compile error.
- Parallelizable steps within a stage: explicit.
- Roll-back: typed operation.
- Pipeline doubles as **unit of replay** (`forge replay
  <report>` re-runs same stages against new tree + diffs) and
  as **audit-trail artifact** (every stage transition logged
  with timestamp, duration, inputs hash, outputs hash).

Vercel and Netlify implement this internally but don't expose
it. **Exposing it as the public architecture means every
customer can see, verify, and trust the build process.**

---

## 5. Extension substrate — Wasm Component Model

For multi-tenant commercialization, the single-operator extension
story (Loom's no-`extra_classes` discipline + SSH-bridged Claude
Code sessions) doesn't extend cleanly. Third-party extensions
need stronger isolation.

### WebAssembly Component Model with WIT-defined interfaces

```
extension-abi/wit/
├── extension.wit              # Top-level component interface
├── primitives.wit             # Loom primitive contracts
├── content.wit                # CMS content type extensions
├── lifecycle.wit              # init / shutdown hooks
├── capabilities.wit           # What the host exposes to plugins
└── policy.wit                 # Permission grants
```

- **Host capabilities granted explicitly.** Plugins declare what
  they need (`read:posts`, `write:comments`,
  `network:api.stripe.com`, `render:custom-block`); host grants
  only what's declared.
- **WASI interfaces (filesystem, network) go through
  manifest-declared capabilities.** No ambient authority.
- **Wasmtime instance** with declared memory limits, CPU time
  limits, instruction budget. Epoch-based deadlines for the
  latter.
- **Plugin SDK is a Rust crate** for Rust authors directly;
  other-language SDKs are wit-bindgen-generated from same WIT
  files. **"Rust-first, other languages later" honored at the
  architecture layer.**

### Why Wasm Component Model

Versus V8 isolates or container-based plugins:

- Cap-based security model fits the threat model
- Faster cold start than containers (μs vs s)
- Smaller resource footprint (no per-tenant VM)
- Cross-language interop without rewriting host
- Bytecode signing for supply-chain assurance

**Honest risk:** WASI Preview 3 with async, GC proposal landing,
threads stabilizing — surface evolves. Pin to specific Wasmtime
versions; treat WIT contracts as semver-governed; let runtime
catch up.

---

## 6. AI surface — LFI as core, LLM as constrained peripheral

> **Earlier sketch was wrong.** This section was originally drafted
> describing an "AI bridge with provider adapters" — a generic LLM
> bridge with retry-on-violation and post-hoc validation. After
> the reviewer looked at PlausiDen-Engine + Neurosymbolic-Toolkit
> properly, the correct architectural commitment is a **deliberate
> inversion of the standard "AI platform" pattern**: LFI is the
> core, LLMs are constrained peripherals. The distinction
> propagates through every layer.

### Why the inversion matters

Most "AI-powered" platforms put the LLM at center: user input →
LLM decides → LLM produces output → platform applies guardrails
after. The values-aligned PlausiDen architecture inverts this:
**LFI is the brain, the LLM is a peripheral that proposes
candidates LFI evaluates.**

The platform's correctness gates, policy decisions, similarity
judgments, drift detection, and explainable critiques all run
through LFI. An LLM is invoked **only** for capabilities LFI
genuinely cannot provide — fluent natural-language generation,
conversational interfaces, free-form text drafting — and its
output is treated as a **proposal that flows back through LFI
for evaluation before anything is committed.**

This is a substantive architectural commitment, not a metaphor.
LFI's output is auditable in the sense that matters legally and
ethically — you can show a third party exactly what reasoning
produced a given decision, and they can verify it without
trusting your runtime. An LLM's output is not auditable in this
sense. For a platform whose values include CARTA, blast-radius
minimization, sovereignty, and AVP-2's "every commit is guilty
until proven innocent," the LLM's opacity is a **load-bearing
problem** and LFI's interpretability is a **load-bearing asset**.

### What LFI actually is

The Neurosymbolic-Toolkit is **seven Rust crates** implementing
a fundamentally different AI substrate than what the industry
currently calls "AI":

- **HDC** — hyperdimensional computing (10k-dimensional bipolar
  vectors, bind, bundle, permute, cosine similarity)
- **PSL / NeuPSL** — probabilistic soft logic (weighted rules
  with gradient solvers and human-readable explanations)
- **LNN** — logical neural networks (differentiable logic gates
  with bounded truth values)
- **VSA** — vector symbolic architectures (typed records,
  sequences, sets, drift detection)
- **HDLM** — hyperdimensional lexicon modeling (text-to-vector
  concept encoding without training a transformer)
- **math-codec** — mathematical expressions as HDC vectors with
  syntactic discrimination

**No training phase. No gradient descent on terabytes. No cloud
inference. Every operation runs locally, in-process,
deterministically, with no API keys, no telemetry, no data ever
leaving the machine. Results are explainable by construction.**
150+ tests passing.

### Crate layout — LFI-first, LLM as adapter behind the critic

```
plausiden-platform/crates/
├── lfi-core/         # consumes Neurosymbolic-Toolkit; primitives + types
├── lfi-policy/       # NeuPSL policy DSL for the platform
├── lfi-corpus/       # HDC-encoded curated patterns + per-tenant private corpora
├── lfi-critic/       # the Critic trait + canonical implementations
└── llm-adapter/      # workspace whose adapters live BEHIND the critic
    ├── anthropic/    # Anthropic adapter
    └── local/        # llama.cpp / vLLM / Ollama for sovereignty
```

**Not `ai-bridge-core` as a separate substrate.** Most of what
the earlier sketch called "AI bridge" is actually LFI substrate
plus a thin LLM-adapter layer for candidate generation.

### The Critic trait — typed seam, compiler-enforced bypass impossibility

```rust
// lfi-critic
pub trait Critic: Send + Sync {
    fn evaluate(&self, proposal: Proposal) -> Decision;
}

pub enum Decision {
    Accept {
        confidence: f32,
        traced_rules_fired: Vec<(RuleId, Strength)>,
    },
    Reject {
        rule_violations: Vec<(RuleId, Strength)>,
    },
    Refine {
        targeted_regeneration_guidance: String,
        violated_rules: Vec<(RuleId, Strength)>,
    },
}
```

Every LLM invocation is wrapped in a **`Proposal →
Critic.evaluate → Decision`** flow. The platform's `commit`
function takes an LFI-attested `Decision`, not raw LLM output.

**There is no `let llm_response = anthropic.generate(prompt).await?;
commit(llm_response);` path** because `commit` does not accept raw
LLM output. The architectural commitment lives in the type
system; bypassing the critic doesn't compile.

### What LFI does in the platform

| Capability | LFI primitive | Role |
|---|---|---|
| **Policy evaluation** | NeuPSL | Weighted rules with explanations. The `policy-core` layer §9 is an application of NeuPSL, not a separate substrate to build. |
| **Originality + similarity in AI generation** | HDC | Encode validated-patterns corpus as 10k-d geometry. Cosine similarity rejects "too similar to existing entries" (LLM regressing to training mean) and flags "too far from anything" for human review. |
| **Drift detection** | VSA | Has actual primitive usage drifted from canonical tokens? Have new content types accumulated fields outside the manifest? Drift surfaces before it hardens into permanent inconsistency. |
| **Internal linking + semantic search** | HDLM | Content vectors locally — no embedding service call, no data leaves the tenant. Search + recommendation + clustering from one substrate. |
| **Brand consistency** | NeuPSL | Brand voice rules ("prefers active voice, weight 0.8"; "avoids superlatives, weight 0.9"; "uses three-bullet feature lists, weight 0.6"). LLM-generated copy flows through; violations surface with rule + weight; regeneration is targeted. |
| **Anomaly + abuse detection** | HDC | Encode normal tenant behavior; runtime activity becomes vector stream; anomalies surface as drift from the normal manifold. **Abuse detection without third-party fraud-detection vendor.** |
| **Math + formal reasoning** | math-codec | Equation rendering, schema migration validation, structural rewriting — formal reasoning through HDC rather than asking an LLM to reason about math (which it does badly). |
| **MCP agent action evaluation** | NeuPSL + HDC | Agent's typed inputs LFI-evaluated for consistency with declared user profile. High-risk capabilities require HDC similarity to existing-data baseline before accepted. |

### What the LLM does

| Capability | Why LLM, not LFI |
|---|---|
| Fluent natural-language generation (copy, blog drafts, microcopy, alt text) | LFI encodes text into geometry + reasons about its structure; does not generate fluent prose |
| Conversational interfaces (admin-portal chat via loom-bridge SSH) | Open-ended dialogue requires NL fluency |
| Free-form structural reasoning over loosely-typed input ("site that feels like Bloomberg crossed with Linear") | LLM extracts structured intent from NL brief; LFI then evaluates extraction against manifest grammar |
| Translation, summarization, NL classification | Customer support triage, content moderation classification, CMS content translation |

**The boundary is consistent:** LLM is invoked for NL fluency or
open-ended structural reasoning over loosely-typed inputs. LFI
is invoked for everything else, **including evaluation of LLM
outputs.** The LLM never has final say; LFI does.

### Staged generation pipeline — LFI-evaluated, not LLM-driven

Same stages as the earlier sketch (brief → IA → wireframe →
content → tokens → audit) — but reframed:

- **The stages are LFI's reasoning sequence.** LFI evaluates
  every stage transition against policy + brand + similarity
  rules.
- **The LLM appears at specific stages as candidate generator**
  where NL fluency or open-ended extraction is needed.
- **Audit stage is Forge running against the LFI-evaluated tree**
  — same gates as human-authored.
- **Failures trigger targeted regeneration of the offending
  block via Refine decisions**, not full restart.

### Provenance per artifact lives at the LFI layer naturally

LFI's outputs already carry explanations. **Trace is "which
rules fired at what strength produced this decision," not just
"which model gave this answer."** `TracedDerivation` vs
`ReconstructedRationalization` distinction at the architecture
layer.

Customer-facing as "explain why this section looks this way"
introspection. **Compliance auditors get exactly what they need.**

### Cost containment is mostly a non-issue

LFI runs locally, deterministically, no token spend. Cost
containment remains relevant only at the LLM-adapter layer for
hosted-model calls — and that's a much smaller surface than the
earlier sketch implied because most "AI" work happens at the
LFI layer.

### What the platform adds vs consumes

**Consumes (Cargo dep, NOT contributed to from this instance per
[[lfi-out-of-scope-for-this-instance]]):**

- **Neurosymbolic-Toolkit** — the shared substrate (HDC, PSL,
  LNN, VSA, HDLM, math-codec). PlausiDen-Engine consumes it;
  Sacred.Vote consumes it; PlausiDen-Shield consumes it for
  policy evaluation; PlausiDen-LFI is the reasoning engine
  built on it; **CMS / Forge / Loom / Crawler platform should
  consume it identically.**
- **PlausiDen-LFI** — the reasoning engine. Out-of-scope for
  this instance. File issues on the repo for any LFI-side work
  the platform needs.

**Adds (platform-specific work):**

- **Platform-specific NeuPSL policy library** — weighted rules
  expressing brand voice, accessibility soft-constraints, design
  system invariants, content moderation, tenant isolation,
  AVP-2 ship-decision logic. Written once per platform,
  customizable per tenant within bounds. Starting point: lift
  existing PlausiDen-Shield NeuPSL-based access control
  patterns. Extension: brand / content / aesthetic policies.
- **Platform-specific HDC corpus** — curated reference patterns
  the AI generation evaluates against. Canonical sites, validated
  style packs, accepted compositions. Grows as customers accept
  generated content. Per-tenant private corpora (one tenant's
  designs don't leak into another's variation rejections) plus
  global public corpus (platform's curated taste).
- **The bridge crate** — typed seam between LLM adapter layer
  and LFI. Small, opinionated, **mandatory pass-through**. The
  architectural commitment that LLM output can never bypass
  LFI evaluation lives here.
- **LLM adapter layer** — Anthropic adapter, local-model
  adapter, fallback orchestration. Cost containment, rate
  limiting, prompt caching, structured output via constrained
  decoding where the model supports it. **Stays as small as
  possible** because most of the AI work happens at the LFI
  layer, not here.

### The positioning this unlocks

Most platforms today claim "AI-powered" and mean "we route
through OpenAI." The values-aligned PlausiDen platform claims
**"neurosymbolic-powered"** and means it: every decision is
interpretable, every policy is auditable, every similarity
judgment is local, no tenant data leaves the platform for AI
evaluation, no opaque model has final say on anything that
ships. The LLM is present for fluency where fluency is needed;
everywhere else, reasoning is **the kind that can be explained
in court, examined by an auditor, or shown to a regulator.**

That's a positioning **no competitor currently occupies**, because
no competitor has invested in a working neurosymbolic substrate.
The toolkit is rare — production-hardened HDC + PSL + LNN + VSA
+ HDLM + math-codec in one Rust workspace with 150+ tests is
unprecedented. Most academic implementations are research code
that never matures. Most industry "neurosymbolic" claims are
LLMs with knowledge graphs bolted on. The PlausiDen substrate
is **the genuine thing**, in a language and runtime suited to
commercial deployment, with the architectural discipline
(AVP-2, typed everything, no-panic semantics) to be trustworthy.

---

## 7. Multi-network publishing as deploy target tier

The gap. Fits the existing architecture cleanly. `deploy-core`
defines:

```rust
pub trait DeployTarget: Send + Sync {
    fn id(&self) -> DeployTargetId;
    fn network_class(&self) -> NetworkClass; // Clearnet, Onion, I2P, IPFS, Hyper, Gemini
    fn security_profile(&self) -> SecurityProfile;
    fn primitive_constraints(&self) -> PrimitiveConstraints;
    async fn deploy(&self, artifacts: &BuildArtifacts) -> Result<DeployRecord>;
    async fn verify(&self, record: &DeployRecord) -> Result<VerificationReport>;
}
```

Adapter crates implement specific targets — `deploy-hetzner`,
`deploy-onion`, `deploy-i2p`, `deploy-ipfs`, `deploy-static-
archive`.

### Constraint propagation back to Forge

A `deploy-onion` target declares constraints like
`external_resources_forbidden`, `javascript_optional`,
`system_fonts_only`, `cookies_forbidden`,
`timing_information_stripped`. Forge's `external_assets`, `csp`,
new `network_target_enforcement` phase consume these constraints
from the deploy target and **fail the build if any primitive in
the rendered tree violates them**.

Same source tree can build cleanly for clearnet but fail for
onion deployment — the build tells the operator exactly which
primitive used which forbidden resource.

### Security profile dashboard

Lifting the multi-network security-rating concept from
[`SITE_OPERATIONS.md §8`](./SITE_OPERATIONS.md). Each deployment
emits a typed `SecurityProfile` covering anonymity, content
secrecy, reader safety, operational security, infrastructure
dimensions. Operator sees what protections apply per deploy
target, what's enforced by the build, what requires operational
discipline they own.

**Honest education layer that distinguishes a serious tool from
a marketing exercise** — promising no more than infrastructure
can guarantee.

---

## 8. Multi-tenant isolation

The architectural commitment that opens the platform to external
customers.

### `tenancy-core` defines the tenant primitive

Every database row, every filesystem path, every metric, every
audit log entry, every background job, every cache key, every
queue message has an explicit `TenantId`. **Not "we'll add a
tenant column later"** — type-system commitment from the start.

```rust
pub struct TenantScoped<T> {
    tenant: TenantId,
    inner: T,
}

impl<T> TenantScoped<T> {
    pub fn access(&self, principal: &Principal) -> Result<&T, AccessDenied> {
        principal.assert_tenant(self.tenant)?;
        Ok(&self.inner)
    }
}
```

### Three isolation tiers

| Tier | Mechanism | Cost | Scale | Default for |
|---|---|---|---|---|
| **Logical** | Row-level security, tenant-scoped queries enforced at ORM, **type system rejects hand-written tenant_id WHERE clauses at compile time** | Cheap | Millions of tenants | Free + basic paid |
| **Process** | Per-tenant background workers, per-tenant connection pools, per-tenant request handlers in critical paths | Moderate | Tens of thousands | Professional / business |
| **Infrastructure** | Per-tenant Firecracker / Cloud Hypervisor microVM, per-tenant database, per-tenant TLS cert, per-tenant Wasm-runtime instance | Most | Hundreds | Enterprise + compliance |

Tier = manifest declaration per tenant. Runtime enforces it.
**Cross-tenant access at any tier is a structural impossibility,
not a policy that could be violated.**

### Cost attribution per tenant

Every operation (HTTP request, background job, AI token, storage
byte-second, bandwidth byte) metered with tenant attribution.
Metering substrate feeds **both billing** (revenue per tenant)
**and abuse detection** (anomalous usage). **FinOps as substrate,
not as afterthought.**

---

## 9. Policy as code

Current AVP-2 doctrine + OPERATING_PRINCIPLES is excellent
governance documentation. Commercial-scale operation requires
those policies to be **executable**, not just readable.

```rust
// policy-core
pub trait Policy: Send + Sync {
    fn id(&self) -> PolicyId;
    fn applies_to(&self, ctx: &PolicyContext) -> bool;
    fn evaluate(&self, ctx: &PolicyContext) -> PolicyDecision;
}
```

Policies cover:

- **Auth requirements** — which capabilities require WebAuthn
  vs allow TOTP
- **Data residency** — which content types stay in which regions
- **Retention** — which fields purge automatically and when
- **Consent** — which capabilities require user consent before
  invocation
- **Rate limiting** — per-tenant, per-capability, per-principal
- **Abuse detection** — anomalous-usage thresholds
- **AI safety** — which prompts allowed against which content,
  which AI capabilities require human approval
- **AVP-2 ship-decision logic** — which test outcomes block
  which capabilities from being marked stable

Open Policy Agent's Rego is the obvious external benchmark;
**Rust-native alternatives (Cedar from AWS, or custom DSL) keep
the substrate Rust-native.** Policies are versioned, signed
(Ed25519, same chain), customer-modifiable within bounds the
platform allows.

---

## 10. Observability substrate

`obs-core` lifted from PlausiDen-Obs into the workspace.

- **Every operation emits structured event** with consistent
  fields: tenant, principal, capability, duration, outcome,
  trace_id, error category if applicable.
- **OpenTelemetry wire format** (OTLP exporter in `obs-otlp` for
  shipping to Honeycomb / Grafana / Loki / Tempo / self-hosted
  backends).
- **Logs are immutable.** Per AVP-2 doctrine, audit logs
  hash-chain (blake3, lifting MCP audit pattern). Storage:
  append-only object storage with object-lock semantics.
  Retention per data class is policy-declared.
- **Metrics first-class.** Per-tenant SLO definitions, error
  budgets, burn-rate alerts. Status page from real metric data,
  not hand-updated. Customer-visible health dashboards per
  tenant.
- **Tracing covers async work.** Background jobs, webhook
  deliveries, AI generation pipelines propagate trace context
  through every stage. A single trace covers "user clicked
  publish → CMS validated → Forge built → Crawler audited →
  deploy completed → verification passed" in one timeline.

---

## 11. Cryptographic substrate

Existing Ed25519 attestation in Forge is the seed.

### `manifest-attest` is the canonical signing layer

Every artifact the platform produces is signed: build reports
(existing), deploy records (new), audit log batches (new),
policy decisions (new), content publishes (new), tenant
configuration changes (new). Same Ed25519 keypair model, same
`forge attest init` workflow, same external pubkey for auditor
trust pinning.

### Post-quantum migration designed in

Lifting from Sacred.Vote work — **ML-DSA (FIPS 204) for
signatures**, **ML-KEM (FIPS 203) for key encapsulation in TLS**.
Hybrid modes during transition (ML-DSA + Ed25519 dual signatures;
X25519 + ML-KEM-768 hybrid key exchange).

**Cryptographic agility:** signing primitive configurable via
manifest, not hard-coded. Algorithm identifiers in every
artifact. Transition is gradual but designed for, not deferred.

### Per-tenant key material in HSMs

- Cloud HSM, YubiHSM, or self-hosted PKCS#11 HSM for enterprise
- Software-backed (libsodium / RustCrypto) for lower tiers
- Key rotation automated, tested, audited

### Encryption at rest by classification

- Field-level encryption for PII (deterministic only where joins
  require)
- Envelope encryption with KMS-managed root keys
- **Customer-managed keys (BYOK) for enterprise** customers who
  require it

---

## 12. Build, test, release discipline

The substrate is sound; sustaining it commercially requires
release engineering matching the architectural ambition.

- **Trunk-based development.** Short-lived feature branches;
  merge to main daily. **Feature flags as unit of incremental
  delivery, not git branches.** Forge's mode-gating pattern
  generalizes: every new capability ships behind a flag, enabled
  for developer's tenant in CI staging, progressively enabled
  per tenant tier in production. **Rollback is flag-flip, not
  deploy.**
- **CI gate is the existing Forge build pipeline, scaled.**
  Every PR runs full audit suite against representative content.
  Per-PR Forge reports including chain extension, comparison
  against main, regression visualization. **27 phases extend to
  60-80 at platform scale.**
- **Mutation testing substrate-wide.** AVP-2 Tier 6 threshold
  (currently `<5%` on critical paths) extends across the
  platform. Critical crates (`manifest-core`, `tenancy-core`,
  `forge-attest`, `policy-core`, `extension-host`,
  `ai-bridge-core`, anything cryptographic) mutation-tested in
  CI. `forge audit mutants` becomes platform-wide gate.
- **Property testing where I/O happens.** Every parser, every
  deserializer, every input boundary. 1024+ cases per property
  is the floor.
- **Fuzz testing on protocol boundaries.** cargo-fuzz / AFL.rs
  for HTTP, JSON parsing, CMS schema deserialization,
  WIT-bound plugin inputs. Continuous fuzzing infrastructure
  (ClusterFuzzLite / OSS-Fuzz integration if open-sourced).
- **Static analysis maximized.** Clippy at pedantic level.
  cargo-deny for license / advisory / banned-dependency.
  cargo-audit for vuln scanning. cargo-vet for supply-chain
  auditing (lifting SLSA). Custom lint rules per workspace
  (Loom no-`extra_classes` discipline generalizes — every typed
  substrate gets a "no escape hatches" lint).
- **Reproducible builds.** Pin every dependency. Vendor in
  critical cases. cargo-zigbuild for cross-compilation.
  Provenance attestation per release artifact (in-toto / SLSA
  3+ targets).

---

## 13. Hosting architecture

The "tech of tomorrow" platform infrastructure:

- **Rust-native runtime.** Tokio for async, axum for HTTP,
  hyper 1.x lower layer, rustls for TLS, rustls-acme for
  automatic cert management. **Already largely in place** across
  existing repos. No JVM, no Node, no Python at runtime.
- **Deployment topology.** Stateless `platform-server`
  instances behind LB. State in PostgreSQL (managed or
  self-hosted per sovereignty preference). Object storage in
  MinIO or S3-compatible (Wasabi, Bunny.net, Hetzner Object
  Storage — sovereignty-aligned options exist). Cache in Redis
  or in-process per-tenant LRU for small-tenant workloads.
  **Background workers = same `platform-server` binary in
  `--mode worker`** — same code path, different runtime config.
- **Edge layer.** Static assets via CDN (Bunny.net is
  sovereignty-aligned; Cloudflare for convenience tier as
  opt-in). Edge functions where they make sense — **not as
  substrate commitment** (those lock to a vendor). Origin
  shielding for cost control on cache misses.
- **Database.** PostgreSQL default. Read replicas in target
  regions. **Single writer per tenant (region-pinned).** Logical
  replication for cross-region eventual consistency where
  needed. CockroachDB / YugabyteDB only if true active-active
  across regions ever required — enormous operational complexity
  tax, probably never warranted.
- **Multi-region from architectural day one even at
  single-region deployment.** State partitionable per tenant.
  No cross-tenant global locks. No "everything in us-east-1"
  assumptions. Region pinning per tenant is manifest
  declaration. Initial deployment likely Hetzner Finland or
  Helsinki for European-aligned hosting.
- **DR substrate.** Continuous backup to object storage in
  different region + different provider. Point-in-time recovery
  7-30 days per tier. Monthly restore drills. Documented RTO /
  RPO targets per tier (1h / 5m enterprise; 8h / 1h
  professional; 24h / 6h free). Validated by drills, not
  assumed.

---

## 14. What stays / extends / net-new

### Stays — keep doing what works

Architecturally correct already; commercializing means
stabilizing semver, documenting for external consumers,
hardening for hostile inputs, scaling for multi-tenancy without
changing substrate shape.

- Loom's typed primitive discipline + no-`extra_classes` lint
- Existing Forge phase pipeline (currently 27 phases)
- Crawler detector axes (45+)
- Annotator session schema
- MCP capability gating pattern
- AVP-2 ship-decision discipline
- Merkle-chain build reports
- Ed25519 attestation
- Type-state pipeline mode (`FORGE_PIPELINE=1`)
- SkillShots PoC as real-product testbed
- `backends.toml` capability declaration

### Extends — natural extensions of existing patterns

- **Manifest layer** consolidating `backends.toml` + Forge
  phases + MCP tools + Loom primitive contracts as projections
  rather than rewriting them (existing patterns, unified)
- **Forge phases** extending from 27 to 60+ (existing pattern,
  more phases for tenancy / policy / supply chain / deploy
  verification)
- **Loom primitives** expanding from current set to ~50+
  (existing pattern, plus motion / typography / asymmetric /
  brutalist primitives per [`ARCHITECTURE_PRINCIPLES.md
  §3`](./ARCHITECTURE_PRINCIPLES.md))
- **Crawler detector axes** expanding from 45 to 80+ (existing
  pattern, more detectors)
- **CMS admin UI** (existing scaffold filled in)
- **`platform-server`** (existing axum-server pattern,
  consolidated as multi-tenant host)

### Net-new — genuine architectural additions

- Wasm Component Model extension substrate (`extension-host` +
  `extension-abi` + WIT contracts)
- `tenancy-core` isolation layer
- **`lfi-core` / `lfi-policy` / `lfi-corpus` / `lfi-critic`** —
  the LFI-as-platform-core layer (consumes Neurosymbolic-Toolkit
  as Cargo dep; **does NOT contribute to PlausiDen-LFI** per
  [[lfi-out-of-scope-for-this-instance]] — file issues if LFI-
  side work needed). `policy-core` from earlier sketch is
  subsumed by `lfi-policy` (NeuPSL is the policy engine).
- `llm-adapter-anthropic` + `llm-adapter-local` (small surface,
  always behind `lfi-critic`)
- `deploy-onion` / `deploy-i2p` / `deploy-ipfs` adapters +
  network-target enforcement propagating constraints back to
  Forge phases (constraint propagation expressed as NeuPSL
  rules per §6)
- Staged generation pipeline (LFI-evaluated; LLM at specific
  stages as candidate generator) + provenance per artifact
  (lives at LFI layer naturally)
- Cost-attribution metering layer
- Customer-facing billing system
- Status-page-from-real-metrics infrastructure
- Self-serve onboarding flow

**Consumed from existing PlausiDen ecosystem (Cargo dep):**

- **Neurosymbolic-Toolkit** — HDC + PSL + LNN + VSA + HDLM +
  math-codec. Shared substrate; PlausiDen-Engine /
  Sacred.Vote / PlausiDen-Shield / PlausiDen-LFI already
  consume it.

---

## 15. The 24-30 month sequencing

Honest. Treating it as a 24-30 month horizon with explicit
milestones avoids both the trap of underestimating ("we'll ship
in six months") and the trap of indefinite-future-product
("we'll commercialize someday").

### Months 1-6 — substrate consolidation (+ LFI policy projections from day one)

- Workspace reorganization
- **Manifest layer designed and built** (`manifest-core`,
  `manifest-codegen`) — KEYSTONE PRIORITY
- Existing concepts (`backends.toml`, Forge phases, MCP tools)
  projected from manifest
- **LFI policy declarations land alongside typed capability
  declarations as projections from day one** — NeuPSL rules
  expressing platform invariants (e.g. "every capability with
  `audit_category=critical` reachable only via WebAuthn-gated UI"
  as a weighted rule that fires on policy violations). Forge
  `backend_coverage` / `phantom_button` / `unbuilt_route` extend
  with **LFI-evaluated policy phases**.
- Type-state pipeline becomes canonical
- Existing capabilities continue working through consolidation
  — **strangler pattern, not big-bang**
- CMS scaffold gets fleshed out into working multi-site backend
  with WebAuthn admin

**End-of-phase deliverable:** same capabilities as today,
cleaner foundation, LFI policy substrate operating.

### Months 4-10 (overlap) — multi-tenancy and isolation (+ LFI for abuse detection from start)

- `tenancy-core` lands with type-system-enforced tenant scoping
- Migration of existing code to use it
- Per-tenant resource isolation at logical tier
- Cost attribution metering
- **LFI for tenant isolation invariants AND abuse detection from
  the start** — HDC vectors representing normal tenant behavior;
  drift detection on the live stream; alerts on anomalies.
  Integration is mechanical because Neurosymbolic-Toolkit
  already provides the primitives. **No separate fraud-detection
  / anomaly-detection subsystem to build later.**

**End-of-phase deliverable:** existing capabilities run under
multi-tenant isolation; "second tenant" milestone reached;
abuse detection live from day one.

### Months 8-14 (overlap) — commercial surface

- Customer-facing admin UI (filling in `cms-admin-ui`)
- Self-serve onboarding
- Billing integration (Stripe or sovereignty-aligned alternative;
  pricing model committed)
- Status page + customer dashboards
- Documentation aimed at external consumers (Diátaxis four-tier)

**End-of-phase deliverable:** first paying customer onboarded;
SOC 2 Type I observation period begins.

### Months 12-18 (overlap) — extension and AI (LFI-first ordering NON-NEGOTIABLE)

- Wasm Component Model substrate (`extension-host`,
  `extension-abi`)
- Reference plugins
- WIT contracts stabilized
- **AI integration becomes LFI-first:** build the policy library
  (`lfi-policy`), build the curated corpus (`lfi-corpus`), build
  the `Critic` crate (`lfi-critic`), **THEN** bolt the LLM
  adapter on as candidate-generation surface (`llm-adapter-*`).
- **Reverse order is forbidden.** Building the LLM adapter first
  and layering LFI on top later produces a platform where LLM
  output sometimes bypasses LFI evaluation because the seam
  wasn't there from the start. **The values-aligned ordering is
  non-negotiable.**
- Constrained decoding substrate (LFI's structural grammar
  derived from the manifest, projected as constraints for
  whichever generation engine produces candidates — constraint
  comes from interpretable rules, not separate grammar
  engineering)
- Provenance per artifact (lives at LFI layer naturally)
- AI cost containment (small surface — only LLM-adapter layer
  for hosted-model calls; LFI is free)

**End-of-phase deliverable:** third-party extensions possible;
LFI-evaluated AI-generated sites meet quality bar; LLM bypass
structurally impossible.

### Months 16-24 (overlap) — multi-network and advanced isolation (LFI for security profile)

- Onion / I2P / IPFS deploy adapters
- **Network-target constraint propagation as NeuPSL rules.** "If
  deploy target is `.onion`, then no external resources allowed"
  with weight 1.0 is a rule, not a hand-coded if-statement in the
  build pipeline. **Adding a new network target means writing
  rules, not patching the pipeline.**
- Security profile dashboard (LFI-evaluated)
- Process + infrastructure isolation tiers
- Enterprise tier features (SSO, SCIM, custom DPAs, BYOK)

**End-of-phase deliverable:** sovereignty-conscious +
privacy-network customers served; SOC 2 Type II audit period
completes.

### Months 22-30 — maturation (LFI for migration policy)

- Performance tuning at scale
- Cost optimization
- Geographic expansion if customer demand justifies
- **Post-quantum cryptographic migration via LFI policy** — when
  to upgrade per tenant, what hybrid modes to use, how to handle
  transition. Rules expressed; migration mechanical.
- Long-tail commercialization (agencies tier, white-label,
  marketplace)

**End-of-phase deliverable:** production-mature platform with
viable customer base.

### Capacity reality check

- Dates compress where capacity is high (you working alone or
  with one collaborator achieves substantially less than what's
  listed; team of 3-5 focused engineers achieves it close to
  schedule)
- Dates expand where reality intervenes (**compliance is slower
  than engineering, sales is slower than building, hiring is
  slower than designing**)

---

## 16. Hold the manifest discipline

Build the manifest layer first. Project existing capabilities
through it (consolidating `backends.toml`, Forge phases, MCP
tools, Loom primitive contracts as projections rather than
rewriting them). Make every new capability flow through it from
the start.

**The substrate built across existing repos is already most of
the way there.** Consolidation work is **months, not years**, and
pays dividends across the entire commercial trajectory.

Everything else — Wasm extension substrate, multi-tenancy
isolation, multi-network deployment, AI bridge, policy engine,
hosting infrastructure — is straightforward Rust engineering
against the manifest.

**Lose the manifest discipline →** fragmented stack that aspires
to coherence and never achieves it.

**Hold the manifest discipline →** platform compounds. Every new
capability inherits all the gates, all the projections, all the
attestations, automatically.

That's the architectural commitment worth defending against
every short-term pressure to skip it. Everything else can flex;
the manifest cannot.
