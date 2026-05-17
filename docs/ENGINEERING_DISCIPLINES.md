# Forge — engineering disciplines

> Companion to [`FORGE_VISION.md`](./FORGE_VISION.md),
> [`ARCHITECTURE_PRINCIPLES.md`](./ARCHITECTURE_PRINCIPLES.md), and
> [`SITE_OPERATIONS.md`](./SITE_OPERATIONS.md).
>
> Vision says what; principles say why; operations says what
> sites need; **this doc says what every engineer working on
> Forge needs to know about the load-bearing technical
> disciplines** — the hard problems that show up regardless of
> architecture and where most platforms accumulate scar tissue.

The bias of this doc: name the hard problems explicitly, with the
known good patterns, so they're decisions instead of discoveries
made during incidents.

---

## 1. Caching and invalidation

> "There are only two hard things in computer science: cache
> invalidation and naming things."

The actual hard problem: when does a cached page need to be
invalidated, and how does the system know? This is where most
CMS performance disasters and stale-content embarrassments live.

**Pattern adopted by Forge:**

- **Surrogate keys per cache entry** (Fastly's model). Every
  cached response carries surrogate-key tags identifying every
  content-piece that contributed to it.
- **Tag-based invalidation.** Mutate a content piece → invalidate
  all surrogate-key tags it contributed to. Dependency tracking
  derived from the typed-content schema: this page depends on
  these content IDs.
- **Cache hierarchy: browser → CDN edge → CDN shield → origin.**
  Each layer has explicit eviction semantics.
- **Stale-while-revalidate** for content that's tolerant of brief
  staleness; **must-revalidate** for content that isn't.
- **Per-tenant cache namespacing.** Tenant A's purge never
  affects tenant B; the cache key construction includes tenant
  scope.
- **Cache poisoning attack surface.** Unkeyed inputs (header
  values, query params, cookies) that an attacker can influence
  must not affect cache keys. Vary header discipline.
- **The bug class: authenticated and unauthenticated responses
  sharing keys.** Catastrophic. Cache keys include auth-state
  classifier; private responses are flagged uncacheable at
  shared layers.

The capability manifest declares **what each capability
invalidates**; the system automates dependency tracking. Most
CMSes outsource this to "purge everything when in doubt" — wasteful
and still wrong sometimes.

---

## 2. Concurrency, consistency, database

**Default isolation: read committed** for most workloads,
**serializable** for the operations that need it (financial
moves, idempotency record writes, anything where the read-modify-
write pattern is correctness-critical). Serializable is correct
but expensive; pretending read committed is correct introduces
anomalies most developers don't anticipate.

- **Optimistic locking** with version columns for most mutable
  rows. Pessimistic locking only where contention is genuinely
  high and serialization would still cause unacceptable conflict
  retries.
- **No distributed transactions across services.** Use sagas
  (compensating actions) or the outbox pattern (write to local
  DB + outbox in same transaction; outbox processor publishes
  the event). Two-phase commit across services is a class of
  problem better avoided than solved.
- **Idempotency keys on every mutation.** Every mutating
  endpoint accepts an idempotency key; replaying the same key
  returns the original response without re-executing. Stripe's
  pattern; the only sane way to handle network retries.
- **Event-sourced vs state-based persistence.** Forge leans
  event-sourced for content (every edit is an event, current
  state is a fold; audit, undo, branching, time-travel fall out
  for free) and state-based for billing / settings / membership
  (no need for time-travel; current state is what matters).
- **CQRS where read and write diverge.** The write model is
  optimized for invariants; the read model is optimized for
  queries. Diverging deliberately is cheaper than forcing one
  model to serve both.
- **Read-your-writes guarantees per operation.** Some operations
  (publish → visit my own site) require RYW; others (publish →
  appear in someone else's feed) tolerate eventual consistency.
  Declared per capability, enforced by routing reads to
  appropriate replica or primary.

### Database choice

**PostgreSQL is the right default for almost everything.** Forge
leans into it: JSONB for semi-structured, FTS for search until
volumes justify a dedicated index, LISTEN/NOTIFY for in-DB
pub/sub, partial indexes for filtered queries, range types for
time intervals, generated columns for derived fields.

Specialized stores adopted where Postgres can't keep up:

- **Redis** for sessions, rate-limit counters, ephemeral
  cache. (Or KeyDB / Valkey if Redis licensing matters.)
- **ClickHouse** for analytics-grade columnar queries.
- **Typesense / Meilisearch / Algolia** for site search at scale.
- **SQLite** for tenant-local data in some architectures
  (per-tenant SQLite file is a viable isolation model).

**Don't sprawl across more stores than you can operate well.** Two
or three is fine; seven is operational debt.

---

## 3. Time, clocks, ordering

Distributed systems' deepest pitfalls.

- **Wall-clock time is unreliable across servers.** NTP drift,
  step-back, leap seconds. Don't use wall-clock for ordering.
- **Lamport / vector / hybrid logical clocks** for actual
  ordering across services. HLC is the practical winner — close
  to wall-clock but monotone and partial-order correct.
- **Store UTC, display in user's locale, accept input in
  declared locale.** Never store ambiguous local time.
- **Daylight saving transitions break things every March and
  November.** Scheduled-publish across DST boundaries; recurring
  events across DST. Test explicitly.
- **"Now" is not a thing in distributed systems** — it's whatever
  the receiving service decides it is. Operations that depend
  on synchronized time need explicit time coordination, not
  trust.

Hidden time bugs surface as "the post that was scheduled for 9 AM
published at 8 AM" or "the analytics for yesterday shows half of
today's data." Forge's tests cover DST transitions explicitly.

---

## 4. Background job architecture

Every non-trivial CMS has substantial background work — image
processing, email delivery, webhook dispatch, scheduled
publishing, search indexing, sitemap regeneration, backup jobs,
cleanup tasks.

- **Queue choice.** Postgres-backed (SolidQueue, pg_jobs,
  River) for simplicity + transactional consistency with main
  DB. Redis-backed (Sidekiq-style) where throughput is the
  concern. Dedicated (NATS, RabbitMQ) only when scale truly
  justifies it.
- **Retry with exponential backoff and jitter.** Jitter is
  non-negotiable — synchronized retries are a thundering herd.
- **Dead-letter queue** for failures past retry budget.
- **Idempotency for at-least-once delivery.** Every job is
  written to handle being executed twice.
- **Priority lanes.** Urgent webhook delivery should not queue
  behind nightly backup. At least three priority levels:
  user-facing-blocking, user-facing-async, background-batch.
- **Job dependency graphs.** Image upload → resize → transcode →
  index → invalidate cache. Modeled explicitly so failures
  surface where they happen.
- **Cron-style scheduled jobs with overlap prevention.**
  Distributed locks (Postgres advisory lock, Redis lock with
  fencing token) for jobs that must not run concurrently.
- **Observability per job.** Which jobs are slow, fail, retry,
  back up. Per-job dashboards with p50/p95/p99 latency.

---

## 5. Webhook delivery as its own hard problem

Outgoing webhooks need:

- **Exactly-once-with-receipt semantics.** Idempotency keys in
  delivered payload, receipt acknowledged before retry stops.
- **HMAC signing with rotation.** Customers verify; rotation
  without downtime via key-set with declared TTLs.
- **Payload versioning.** Customers pin to a version; we maintain
  N versions concurrently during deprecation windows.
- **Retry with exponential backoff for hours, not minutes.**
  Customer's endpoint may be temporarily down; persistence
  matters.
- **Dead-letter handling** with customer-visible replay.
- **Replay from dashboard.** Customer can replay any delivery
  for debugging.
- **Delivery monitoring per endpoint.** Failure rates surfaced.
- **Automatic disabling** of consistently-failing endpoints
  (with notification) — prevents one bad customer from
  exhausting our delivery budget.
- **Customer-visible delivery logs.** Every delivery, every
  retry, every response code. Searchable.

Stripe's webhook tooling is the bar. Most platforms ship webhook
systems that lose events under failure modes their customers
will eventually hit.

---

## 6. State machines for every multi-step process

Subscription lifecycle (`trialing → active → past_due → canceled
→ grace → expired`). Order fulfillment. User onboarding. Content
moderation review. Build pipeline state. Each is a state
machine; **modeling explicitly catches bugs at design time that
scattered if-statements miss.**

XState (TypeScript) or `state_machine` patterns in Rust (typed
state via the typestate pattern with phantom types — Rust's type
system can enforce that you can't call `publish()` on a
`Draft` state). The TLA+ spec at `docs/PHASE_PIPELINE.tla`
already captures the phase pipeline as such — extend the pattern
to all multi-step workflows.

---

## 7. Cryptography in concrete

Beyond "use crypto correctly."

| Operation | Choice | Notes |
|---|---|---|
| Password hashing | **Argon2id** | Not bcrypt, not scrypt, not PBKDF2. Argon2id is the current winner per OWASP / IETF |
| Symmetric encryption | **XChaCha20-Poly1305** | Long nonce eliminates nonce-reuse footgun |
| Signatures | **Ed25519** | Fast, small, well-vetted |
| Key exchange | **X25519** | Pair with Ed25519 |
| Post-quantum key encapsulation | **ML-KEM-768** | Hybrid mode with X25519 during transition |
| Post-quantum signatures | **ML-DSA** | Where supported; hybrid with Ed25519 |
| Key derivation | **HKDF** | Domain-separate every derived key |

Discipline:

- **AEAD always.** Never encrypt without authentication.
- **Constant-time comparisons** for any secret comparison.
- **No custom crypto.** Ever. Anywhere. libsodium or RustCrypto
  crates as foundation.
- **Cryptographic agility designed in.** Algorithm identifiers
  in protocol layers so migration is possible when (not if)
  something breaks.
- **Key rotation strategy per key category.** Automated and
  tested.
- **Key management hierarchy.** Root keys offline (hardware,
  air-gapped). Intermediate keys in HSM. Working keys in memory
  only, never serialized to disk.

---

## 8. Secrets management

- **Vault** (HashiCorp Vault, AWS Secrets Manager, Bitwarden
  Secrets Manager, Doppler, self-hosted alternatives).
- **Injection at runtime**, never baked into images.
- **Rotation** automated; tested by drilling a rotation in
  staging quarterly.
- **Audit** every secret access.
- **Secret zero problem.** The secret that unlocks the secrets
  is itself secured — hardware token at the human layer, KMS
  at the service layer.
- **No secrets in git ever.** gitleaks / trufflehog in
  pre-commit, in CI as a hard gate.
- **TRNG quality** for any secret generation.
- **Dev/prod separation.** Production secrets never present in
  dev environments.

---

## 9. Dev / staging / prod environments

- **Production data is not in staging.** Staging has synthetic
  or anonymized data. Anonymization pipeline runs on production
  → staging refresh; PII stripped or pseudonymized.
- **Per-environment config** (12-factor; environment vars or
  config-as-code with environment overlay), NOT in code.
- **Per-environment vaults** with no cross-environment access.
- **Trunk-based development with feature flags.** Code merges
  to main behind flags; ships dark; enabled progressively
  (1% → 10% → 50% → 100%) with automated rollback on error
  budget burn. **Every release is a non-event.**
- **Developers debug production safely.** Read-only access,
  audit logging, time-bound elevation. The "log in to
  production and poke around" anti-pattern that every startup
  eventually outgrows painfully — *don't grow into it in the
  first place*.

---

## 10. Code review and merge

- **CODEOWNERS per area.** Required reviewers; merges blocked
  without sign-off from the right person.
- **Automated checks block merge.** Fmt, lint, test, audit,
  contract test, type check. No "I'll fix in a follow-up."
- **Small PRs.** Single-purpose, reviewable in <30 minutes.
- **Architecture changes via RFC / ADR** before code review.
  Reviewing a 1000-line PR is too late.
- **Junior reviewers don't burn out senior ones.** Tiered
  review — first pass by peer; architectural pass by senior
  for changes that warrant it.
- **AVP-2 annotations** required per Doctrine — every line of
  code carries an `AVP-PASS-N` annotation somewhere in its
  blame history.

---

## 11. Service-to-service authentication

- **mTLS between services.** Network trust is not enough.
- **SPIFFE / SPIRE** for workload identity.
- **Service mesh** — Linkerd if you want simple and fast,
  Istio if you want comprehensive and complex. Default to
  Linkerd unless you have a specific reason for the complexity.
- **Zero-trust at the workload level.** Even internal services
  authenticate. Aligns with CARTA orientation.

---

## 12. Multi-region operational complexity

Multi-region is harder than "having multiple regions."

- **Failover orchestration.** Documented, tested via drills.
- **Split-brain prevention.** Quorum systems; STONITH for
  the worst cases.
- **Region-pinned data per tenant** as the cleaner model than
  global writes. Each tenant's writes go to their home region;
  cross-region replication for reads.
- **Global databases** (CockroachDB, YugabyteDB) only when
  truly justified — they trade latency and operational
  complexity for global write distribution.
- **Cross-region replication lag** handled explicitly with
  read-your-writes routing.
- **Recovery procedures for entire-region failure** documented
  and drilled.
- **Active-active vs active-passive** — most platforms claim
  multi-region and have only redundant standby. Actual active-
  active is harder and **most don't need it**. Be honest about
  which you have.

---

## 13. Failure modes from system dynamics

Failures that emerge from how the system behaves under load,
not from bugs. Pre-2015 *Release It!* by Michael Nygard cataloged
most of these and they remain relevant.

- **Cache stampede / thundering herd.** Cache expiration
  triggers simultaneous rebuilds. Mitigate: probabilistic early
  expiration; lock-based rebuild with stale-serve; pre-warming.
- **Retry storms** when a downstream fails. Mitigate:
  exponential backoff with jitter; circuit breakers tripping
  after N consecutive failures; load shedding.
- **Notification feedback loops** (signup triggers email
  triggers webhook triggers signup somehow). Mitigate: rate
  limit per cause-effect chain; idempotency per recipient.
- **Bulkheads.** Failure in one subsystem doesn't drain
  resources from another. Connection pools per downstream,
  not shared.
- **Request hedging.** Send duplicate requests after p99 latency,
  use whichever returns first. Costs 2× capacity but cuts tail
  latency.

Treat these as first-class. Most teams reinvent each pattern
after experiencing the failure once.

---

## 14. Database migration safety

Most production incidents at growing companies happen here.

- **Expand-contract pattern.** Schema change in multiple deploys:
  add new column nullable → backfill → make required → remove
  old column. Never single-shot ALTER on a hot table.
- **Online migration tools.** `pg_repack` for PostgreSQL.
  `pt-online-schema-change` for MySQL. Run migrations without
  table locks.
- **Lock-aware migrations.** A 30-second lock on a hot table is
  an outage. Tools (Strong Migrations gem, pgmig, custom
  linters) catch unsafe patterns in CI.
- **Forward + backward compatibility.** Old code can read what
  new code writes (forward), new code can read what old code
  wrote (backward). Required during the deploy window.
- **Strangler patterns at the DB level.** Moving tables to
  different stores without downtime — write-to-both, gradually
  migrate readers, drop the old.
- **The migration backlog.** Most platforms accumulate
  dangerous migrations they can't run because they don't have
  the patterns figured out. Get the patterns figured out before
  the backlog accumulates.

---

## 15. Security incident handling

Beyond runbooks (covered in `ARCHITECTURE_PRINCIPLES.md §8`).

- **Forensic readiness.** Logs in immutable storage,
  tamper-evident (hash-chained, write-once), correct retention.
- **Chain of custody** if law-enforcement involvement is needed.
- **Memory forensics** capability for sophisticated attacks.
- **Decision tree.** When to involve law enforcement vs handle
  internally vs hire incident-response firm (Mandiant,
  CrowdStrike).
- **Communications during active incident.** Legal review of
  disclosures, customer notifications, regulator notifications
  within statutory windows (**GDPR's 72 hours specifically** —
  the clock is real).
- **Crisis communications drilled and practiced.** Tabletop
  quarterly.
- **Insurance involvement.** Cyber liability policy holders
  notified per policy terms.

Most platforms have never been through a serious incident; the
first one teaches expensive lessons. Drill before the incident,
not during.

---

## 16. Coordinated disclosure and security culture

- **Vulnerability disclosure policy.** `security.txt` with
  contact, SLAs on response, safe harbor for researchers acting
  in good faith.
- **Bug bounty tiered by severity** and asset criticality.
  HackerOne / Intigriti once baseline is workable.
- **Internal vulnerability tracking** that doesn't disclose to
  attackers via slow patch deployment. Coordinated disclosure
  with researchers.
- **Researcher relationships.** The security research community
  is small; treat it well.
- **Public security advisories with CVE assignment** for issues
  that affect customers. Stripe / Cloudflare publish their
  advisories openly; the maturity signal earns trust.

---

## 17. Open source strategy

What's open, what's not, what's source-available, what's
contributor-licensed.

| License class | Examples | When |
|---|---|---|
| Permissive | MIT, Apache-2.0 | Foundational libraries, SDKs |
| Weak copyleft | LGPL, MPL | Where copyleft matters but linking should be free |
| Strong copyleft | GPL, **AGPL** | Where network use should trigger source disclosure |
| Source-available | FSL, BSL, Elastic License | Where competitive moat matters but transparency does too |
| Proprietary | — | Where business model requires |

**Forge's current stance: FSL-1.1-MIT** — Functional Source
License with 2-year competitor-restriction window, then converts
automatically to MIT. Aligns with sovereignty values (source-
available + eventually fully free) while preventing parasitic
competitors from extracting work into proprietary products
during the window.

Operational details:

- **CLA vs DCO.** DCO (Developer Certificate of Origin) is
  lighter-weight and respects contributors. CLA only where
  legally necessary.
- **Community management** implications scale with adoption.
- **Competitive dynamics.** Elasticsearch → OpenSearch fork,
  Terraform → OpenTofu, Redis → Valkey — instructive. Don't
  change the license post-success without expecting the fork
  response.

---

## 18. Costs, funding, and the architecture they shape

Architecture decisions are downstream of funding decisions in
subtle ways.

- **Bootstrapping:** romantic but slow; pressures toward
  profitability per customer.
- **VC:** fast but pressures toward growth-at-all-costs;
  conflicts with sovereignty values.
- **Revenue-funded growth:** requires credible early product.
- **Grants** (NLnet, Sovereign Tech Fund, Open Technology Fund)
  align with values, pressure toward openness + durability,
  cap growth rate.

**Naming the funding model explicitly shapes everything else.**
Forge's posture aligns with grants + revenue-funded — not VC
default. The architecture (sovereign-friendly defaults, FSL
license, multi-network publishing, plausible deniability) is
**internally consistent with that funding posture**.

Order-of-magnitude annual costs at scale:

- Cloud infrastructure: $K to $M depending on tenant count
- AI inference: $K to $M depending on per-tenant AI usage
- Compliance audits (SOC 2 Type II + ISO 27001): $5-6 figures
- Pen tests: $5 figures per engagement (quarterly)
- Bug bounties: scale with payouts; budget realistically
- Visual regression (Chromatic at volume): $K-$5K/mo
- Localization: $K-$10K per locale, ongoing

**Building this team-wise takes a meaningful number of engineers
across specializations.** The architecture is the easy part;
the team is the hard part. Most platforms with great
architecture and bad hiring fail to ship; great hiring with
mediocre architecture often still ships.

---

## 19. Customer support as engineering function

- **Engineer-on-call rotation includes support tickets.**
  Engineers feel the pain of their decisions.
- **Tier zero (self-service docs + search)** is where most
  support volume should resolve.
- **Tier 1 (general)** for triage.
- **Tier 2 (technical)** for product issues.
- **Tier 3 (engineering escalation)** for bugs and edge cases.
- **Support volume per active customer as tracked metric.**
  Increasing volume per customer = product getting harder to
  use.
- **AI in support:** AI as augmentation for human agents = good.
  AI as first-line solo = saves money and burns trust. AI as
  draft generator for human review = the right pattern today.
- **Community support** (forums, Discord) supplements paid
  support; doesn't replace it.

---

## 20. Product analytics for the platform itself

Beyond customer-facing analytics. The platform team needs to
know:

- Which features are used by what percentage of customers
- Time-to-first-success per feature
- Churn predictors
- Expansion predictors
- Activation cohort analysis (defined as the user action that
  predicts retention — typically "first publish with custom
  domain" or "first content created across N types")
- Product-led growth telemetry

Most platforms have less insight into their own usage than their
customers have into their sites. Don't be most platforms.

---

## 21. Decision-making framework

The part that determines whether the team sustains itself
through years of execution.

- **RFCs** for significant decisions.
- **ADRs** (Architecture Decision Records) for architecture.
  Date, context, decision, consequences, alternatives
  considered. ~1-2 pages each. The record matters more than
  the prose.
- **DACI / RAPID / RACI** for who decides what.
- **Disagree and commit** as a norm.
- **Escalation paths** explicit.
- **Conflict resolution** without poisoning relationships.
- **Strategic vs tactical cadence.** Strategic decisions get
  more thought, fewer revisions.

**Engineering organizations die more often from decision
dysfunction than from technical debt.**

---

## 22. Revisions to earlier claims

A few earlier doctrine statements deserve nuance — captured here
so the doctrine reflects what we actually believe.

**"Capability manifest as source of truth" understated maintenance
cost.** Manifests rot. Adding capabilities requires manifest
updates people forget. The CI gates help but don't eliminate the
problem. **The manifest needs explicit ownership** — a small
team that reviews changes, maintains versioning, catches
inconsistencies. Closer to language-standards-committee work than
typical engineering. Without that ownership, the manifest
devolves into outdated documentation.

**"World-class is a gradient" underemphasized that the gradient
costs continuous resources.** Visual regression at scale costs
money. External pen tests cost money. Bug bounties cost money.
Localization costs money. Accessibility user testing costs money.
**Maintaining the discipline requires either substantial budget,
substantial volunteer labor, or substantial automation — probably
all three.** Treating "world-class" as achievable without
explicit budget is the failure mode that turns architecture
documents into wishes.

**The mainstream-vs-sovereign framing was too binary.** Most real
deployments mix — Stripe for payments (mainstream), Hetzner for
hosting (sovereign), Cloudflare for CDN (mainstream), self-hosted
analytics (sovereign), OpenAI for AI (mainstream), passkeys for
auth (sovereign-compatible). **The values declaration should
let users mix per-dimension, not force a single profile.** The
profile presets in [`SITE_OPERATIONS.md §9`](./SITE_OPERATIONS.md)
are starting points that the user mixes from.

**AI integration assumes hosted models stay available.**
OpenAI / Anthropic / Google could change terms, prices,
capabilities at any time. **Capability manifest declares AI
providers as pluggable** — prompt engineering decoupled from
specific model. Self-hosted fallback for at least the critical
paths means the platform survives provider disputes or outages.

**The plugin sandbox is "Wasm-based" not "solved."** WASI Preview
2 is recent and the component model is younger. Some capabilities
to expose (filesystem-like access, network, GPU) have unstable
interfaces. The architectural commitment to Wasm is right but
the implementation hits edges that don't have great answers yet.
Honest with users: "Wasm-based sandbox" means "the right
substrate that's still maturing," not "problem solved."

---

## 23. What's missing from this doc

Honest list — not because every gap needs filling, but so future
maintainers know what hasn't been written down:

- **Architectural diagrams.** Data flow, trust boundaries,
  service relationships. The text descriptions assume the
  diagrams the reader is imagining.
- **Sequence diagrams** for important flows (publish, plugin
  invocation, AI generation, content render with caching).
- **Numerical targets.** Not "fast" but "p99 publish latency
  under 2s for sites under 1000 pages."
- **Specific technology choices with rationale.** Not "a queue"
  but "Redis Streams for ephemeral, PostgreSQL with SKIP LOCKED
  for durable, with these tradeoffs."
- **Per-subsystem threat models.**
- **Decision logs** explaining why X was chosen over Y for the
  decisions we've already made.
- **Migration paths** from each significant earlier-version
  decision so future maintainers understand what's load-bearing
  and what's accidental.

This thread is a *design exploration*. Production documentation
is a different artifact. The transition from exploration to
production docs is itself work and the value of the exploration
partly depends on whether someone does it.

---

## 24. The honest meta-observation

The architecture is coherent, ambitious, recognizably valuable.
**Remaining gaps are increasingly specialized — "things experts
in narrow domains would add" rather than "things a complete
picture is missing."** Diminishing returns from continued
elaboration are real.

The more useful question at this point isn't "what else?" but
**"what now?"** The architecture has been thought through.
Building it requires choices about sequencing, team, funding,
audience, and which compromises to accept. Those are not
engineering questions; continuing to refine the architecture
without engaging them is a comfortable form of procrastination.

If Forge is a near-term project, the next move is translating
the most important architectural decisions into ADRs defendable
in code review, picking the smallest vertical slice that proves
the substrate, and building it.

If Forge is longer-term, the next move is extracting the
cross-cutting principles (typed substrates, capability
manifests, primitive contracts, constraint-bound AI) into a
reference architecture that informs existing work and seeds
future work.

The architecture is sound. The remaining questions are about
**what to do with it**, not about what's missing from it.

That's a good place to be.
