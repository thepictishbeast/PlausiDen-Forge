# Forge — commercialization framing

> Companion to [`FORGE_VISION.md`](./FORGE_VISION.md),
> [`ARCHITECTURE_PRINCIPLES.md`](./ARCHITECTURE_PRINCIPLES.md),
> [`SITE_OPERATIONS.md`](./SITE_OPERATIONS.md), and
> [`ENGINEERING_DISCIPLINES.md`](./ENGINEERING_DISCIPLINES.md).
>
> Vision says what; principles say why; operations says what
> sites need; disciplines says what engineers need to know;
> **this doc says what changes when scope expands from
> operator-toolkit to platform-we-sell.**
>
> Commercialization is a substantive product and organizational
> shift, not just a marketing decision. The architecture supports
> it; the operating principles need explicit revision; the
> existing AVP-2 / sovereignty / no-mainstream-compromise
> commitments interact with commercialization in ways worth
> surfacing clinically before committing.

---

## 1. The substrate is already commercializable as-is

Forge, Loom, Crawler, Annotator, MCP, AVP-Doctrine are coherent
products with clear value propositions, working code, and stated
licensing (FSL-1.1-MIT converting to MIT after two years for
most; AGPL-3.0-or-later for the protection suite). **The
architectural work to make them sellable is mostly done.**

What hasn't been done is the *productization* layer above them:
hosted product, onboarding flow, billing, support function,
documentation aimed at external customers, brand and positioning,
sales motion, legal entity structure for customer contracts.

---

## 2. Commercialization tiers (not binary)

Several distinct tiers, each with different implications:

| Tier | Model | Examples | Forge fit |
|---|---|---|---|
| **1** | Open-source-with-paid-support | GitLab early, Sentry, many infra tools | **Natural fit.** Repos stay public, FSL/AGPL preserved, revenue from support contracts + custom dev + deployment assistance + training + hosted-version-for-customers-who-don't-want-to-self-host. Aligns with existing operating principles. Lowest org change required. |
| **2** | Managed hosting primary, source-available as marketing | Sanity, Supabase, PostHog, Plausible | **Workable fit.** Hosted version is commercial product; OSS drives credibility. Customers self-host if they want; most won't. Maps cleanly to existing architecture if you accept customers' data living on infrastructure you operate. Requires meaningful hosting + support + billing infrastructure. |
| **3** | Proprietary SaaS, open SDKs | Stripe, Linear | **Conflicts with AGPL commitment in protection suite specifically.** Would require relicensing or carving SaaS components into separate IP. |
| **4** | Pure SaaS, proprietary stack | most mainstream platforms | **Conflicts with stated values across existing repos.** Not a viable direction. |

**Natural fit given existing commitments: Tier 1 sliding toward
Tier 2.** Keep source-available posture, build a hosted offering
as the convenient path, support self-hosters who want to operate
the stack themselves. Minimizes conflict with stated principles;
maximizes leverage on architecture already built.

---

## 3. Which products commercialize first

Not all PlausiDen-* repos are equally market-ready, and not all
are individually commercializable. A coherent commercial offering
picks two or three to lead with:

### 3.1 Loom + Forge + Crawler as the publishing platform — strongest commercial story

**Pitch:** *"Typed CMS → in-process render → typed audit
pipeline → cryptographically-attested build artifact → static OR
hybrid OR dynamic output → identical primitives across modes."*

Competitive against Astro (which has dynamic but lacks audit
gating + attestation), Vercel (which has hybrid rendering but is
a hosting commitment), Webflow (which has neither typed schemas
nor verifiable builds), and Next.js (which doesn't gate on
accessibility / performance / security at build time).

Sells to:

- **Agencies and in-house marketing teams** that need verifiable
  accessibility / performance / security gates
- **Compliance-sensitive customers** (finance, healthcare,
  government, journalism) who can hand auditors a signed Merkle
  chain of every build that produced production
- **Sovereignty-conscious operators** who want the same
  primitives static or dynamic without committing to a specific
  hosting topology

This is the strongest commercial story. Architectural work is
most mature; market is largest; differentiation is sharpest;
customer profile overlaps your existing network.

### 3.2 PlausiDen-MCP + plausible deniability engine

**Pitch:** *"Privacy infrastructure for AI applications. Integrate
plausible deniability without building the forensic-realism layer
yourself."*

Sells to developers + companies building AI products who want
plausible-deniability features integrated. Smaller TAM but
underserved + high-value-per-customer. **Politically charged in
ways that affect funding + partnership prospects** — decide
deliberately whether to lead with this or trail it.

### 3.3 PlausiDen-Suite (Tidy / Atrium / Purge / Sentinel)

**Pitch:** Privacy desktop toolkit for individuals, journalists,
dissidents, security-conscious professionals.

Comparison: BleachBit / VeraCrypt / specific antiforensic tools.
Lower per-customer revenue, higher unit volume, different sales
motion (probably self-serve from a website, not enterprise sales).

### 3.4 Sacred.Vote for municipalities

Different sales motion entirely (government procurement, RFPs,
long sales cycles). The Tim Porter relationship is the wedge for
one specific case; generalizing requires both technical
generalization (multi-tenant, configurable, jurisdiction-aware)
+ a B2G sales function. Worth pursuing on a different track.

### 3.5 The Salesman as a productized tool

Currently scoped as internal client acquisition. Could be
productized for other founders / agencies. Lower priority given
less mature implementation.

**Discipline:** these are different products targeting different
audiences with different sales motions. **Trying to commercialize
all of them simultaneously fragments attention and dilutes
brand.** Pick one to lead, build a credible commercial offering
around it, then expand. **Probable lead: §3.1 (Loom + Forge +
Crawler).**

---

## 4. What has to change to actually sell

### 4.1 Legal + operational infrastructure

- PlausiDen Technologies LLC exists; needs to actually transact
- Payment processor (Stripe is obvious + conflicts with
  sovereignty for some customers; offer both Stripe + crypto +
  possibly invoice for enterprise)
- Customer ToS, Privacy Policy, DPA template for B2B
- SOC 2 readiness if selling to enterprise
- Business insurance (E&O, cyber liability, general liability)
- Commercial bank account separate from personal
- Accounting handling deferred revenue for subscriptions

### 4.2 Hosted infrastructure (if Tier 2)

Customer data on your infrastructure means:

- Multi-tenant isolation hardened beyond what self-hosting needs
- Backup + DR with tested restore (monthly drills)
- Status page + incident communication
- On-call rotation
- Capacity planning
- Cost attribution per customer (FinOps)
- Abuse handling (because customers will eventually do things
  you don't want associated with your platform)

The Hetzner-based infrastructure today can serve this **if
hardened deliberately**, but the operational discipline required
to host other people's data is meaningfully more than hosting
your own.

### 4.3 Customer support function

Even at Tier 1, customers need help. Initially this is you,
evenings and weekends. At some volume (typically **50-200 active
paying customers depending on product complexity**) it becomes
unsustainable and you hire or contract.

### 4.4 Documentation aimed at external consumers

Current READMEs are excellent internal documentation — they
assume the reader is you or someone who's been in the codebase.
External customers need:

- Tutorials (learning-oriented)
- How-to guides (task-oriented)
- Conceptual explanations
- Troubleshooting
- FAQs
- Video walkthroughs

The Diátaxis four-tier structure becomes mandatory. **Months of
work + ongoing maintenance, not a one-time task.**

### 4.5 Pricing model + metering

Per-site, per-seat, per-page, usage-based on builds or audits,
freemium with paid tier — each is viable, each has different
customer-expectation implications.

**Metering infrastructure has to land before pricing is set** —
retroactively pricing things you haven't been measuring is
structurally dishonest. Forge already has the audit-log
infrastructure to meter; what's missing is the billing layer
that consumes the meter + the customer-facing UI that shows
usage + limits.

### 4.6 Brand + positioning beyond "Paul's projects"

PlausiDen Technologies LLC currently signals to anyone who looks
like a founder-vehicle for related-but-distinct projects.
Commercializing requires deciding what the company *is* in
customer-facing terms — CMS / static-site platform vendor,
privacy infrastructure vendor, civic-tech vendor, all three under
one brand?

Either a unified narrative ("infrastructure for sovereign
computing") or distinct sub-brands separate the offerings.
**Decide deliberately.**

### 4.7 Sales + marketing function

Commercial software requires customer acquisition. Content
marketing (engineering blog, case studies, demos), conference
presence in relevant communities (Astro/Eleventy/Webflow for the
CMS; OWASP/DEF CON/CCC for privacy tools; civic-tech / open-
government for Sacred.Vote), partnerships with agencies +
consultancies, eventually paid acquisition if unit economics
support it.

Most engineers underestimate the lead time. Treat marketing as
its own discipline, not an afterthought.

---

## 5. What the operating principles need to say now

The current `OPERATING_PRINCIPLES` (in the relevant repos) were
written for a **single-operator-toolkit posture**. Commercialization
changes the calculus. Updating the principles deliberately is
better than letting them quietly drift.

### 5.1 Triggers reflect commercial validation, not personal friction

The "three consumers" trigger for the CMS becomes **"three
paying customers, or one paying customer at MRR threshold X."**
The trigger isn't unmet need; it's market validation. Different
threshold; may fire earlier or later.

### 5.2 Meta-infrastructure principle contracts in scope

"Meta-infrastructure is net-negative until proven otherwise"
**stays for genuinely internal infrastructure** but doesn't apply
to commercial product features. Customers buy products partly
*because* of the meta-infrastructure (admin UIs, dashboards,
billing portals, documentation sites) that operator-only
deployments don't need. Building these isn't violating the
principle; the principle's scope contracts to internal-only.

### 5.3 AVP-2 stays internally; needs honest customer-facing translation

The AVP-2 doctrine continues unchanged but with explicit
acknowledgment that "STILL BROKEN" as default verdict creates
customer-communication challenges. **You can't sell software
whose stated status is "DO NOT USE — UNSAFE."**

Verdict stays internally accurate; customer-facing version needs
honest-but-saleable framing — *"pre-1.0, see status page for
production-readiness per component"* or similar. The doctrine is
right; the public face needs translation.

### 5.4 Sovereignty commitments preserved, not relaxed for reach

The honest path is **preserving sovereignty commitments and
selling specifically to the audience that values them** —
privacy-conscious agencies, security-focused enterprises,
journalism organizations, civic-tech buyers, sovereignty-aligned
governments, dissident-supporting NGOs.

Smaller TAM than mainstream, but a **higher-conviction one, and
the architecture's correctness is a real differentiator inside
it.** Trying to grow into the mainstream market would require
compromises that erase the differentiation.

The mainstream/sovereign per-dimension duality
([`SITE_OPERATIONS.md §9`](./SITE_OPERATIONS.md)) lets individual
customers mix dimensions — Stripe for payments, Hetzner for
hosting, etc. — without the platform itself compromising its
sovereign defaults. Customers choose their values per dimension;
the platform doesn't choose for them.

---

## 6. Architecture extensions commercialization unlocks

### 6.1 Multi-tenant isolation hardened

Current scaffold: "one CMS, N sites" for PlausiDen-namespace
sites you own. Selling to external customers means **N tenants
you don't own**, with hostile-tenant threat models:

- One customer cannot access another customer's data
- Cannot exhaust shared resources
- Cannot affect others' availability
- Cannot fingerprint that they exist as tenants on the same
  platform

Implementation: row-level security with tenant-scoped policies
enforced at the ORM layer, per-tenant background jobs, per-tenant
resource quotas, per-tenant audit logs, possibly per-tenant
database schemas for enterprise customers who require it.

### 6.2 Customer admin UI

No customer-facing admin surface today — the CMS's admin portal
is scaffolded but trigger-gated. Commercialization makes this the
next thing to build, because customers can't use a product whose
admin interface doesn't exist. **Months of work** — content
editing UI, asset management, user management within a customer's
org, billing dashboard, audit log viewing, support ticket
integration. Probably the **single largest unit of work in the
commercialization path.**

### 6.3 Billing + subscription infrastructure

Customer signup, plan selection, payment method capture,
recurring billing, invoice generation, dunning when payments
fail, plan upgrades/downgrades with proration, cancellation,
refunds, tax per jurisdiction (Stripe Tax), revenue recognition
for accounting.

**Most of this is solved by Stripe Billing if you accept Stripe.**
Building it from scratch is a quarter of work for no good reason
unless Stripe is rejected for principled reasons (in which case
Paddle or similar exists with different tradeoffs).

### 6.4 Status page, SLA, on-call alerting

Customers paying for a hosted service expect to know when it's
down and expect remediation. Status page (Statuspage.io / Better
Stack / Cachet) + alerting (PagerDuty / Opsgenie / Grafana
OnCall) + documented SLAs that alerting enforces against. **The
Crawler-as-monitoring-tool could feed the status page directly**
— turn your own audit infrastructure into customer-facing uptime
monitoring.

### 6.5 Self-serve onboarding

Today: deploying the stack requires reading the READMEs,
understanding the architecture, building from source, configuring
infrastructure. **Customers will not do this.**

Self-serve onboarding: hosted signup → empty workspace
provisioned automatically → guided first-site-creation flow →
templates to start from → sample content demonstrating value
within five minutes. The Loom site scaffolder is the seed of
this; the customer-facing wrapper hasn't been built.

### 6.6 Multi-region (deferred until customer demand)

Single-region Hetzner is fine for European + Atlantic-region
customers; Asian or Pacific customers experience latency.
Multi-region requires re-architecting data residency per tenant,
replication, regional failover. **Significant work; appropriately
deferred until customer demand justifies it.**

### 6.7 The "mainstream mode" question gets answered

The substrate could support both sovereignty and mainstream modes
via configuration. **Commercializing forces the question:** are
you willing to add a mainstream integration tier (Google
Analytics opt-in, social embed primitives, Stripe payments
built-in) to reach customers who want those, or do you commit to
the sovereign audience and accept the smaller TAM?

Per existing operating principles + this doc §5.4: **commit to
the sovereign audience.** Make the choice explicit; defend it.

---

## 7. Required vs optional, by customer count

Many founders try to build all of this before launching. The
disciplined version is:

### 7.1 Required for first paying customer

- Hosted instance that works
- Way to sign up + pay
- ToS + Privacy Policy that exist
- Way to contact support
- Basic documentation
- Backups
- Legal entity actually able to receive payment

### 7.2 Required by 10 paying customers

- Stable hosting with monitoring
- Status page
- Proper customer support workflow (even just you + email)
- Onboarding without walking every customer through setup
  personally
- Docs covering top 10 support questions
- Billing automation (no manual invoicing)

### 7.3 Required by 100 paying customers

- SOC 2 or equivalent if selling to meaningful B2B
- Real support function (you can't do it alone at that volume)
- Self-serve onboarding that works without you involved
- Proper SLAs
- Documented incident response process
- Probably a first hire or contractor

### 7.4 Optional until forced by specific customer demand

- Enterprise SSO, SCIM, custom DPAs
- On-premises deployment
- Advanced compliance (HIPAA / FedRAMP / IL4)
- Specific integrations
- White-label
- Agency tooling

Each takes months. **Sold-then-built, not built-then-sold.**

**The trap:** building all the "by 100 customers" features before
having 10, because that's the comfortable engineering work
compared to the harder marketing + sales work. The architecture
supports the deferral; discipline about what to actually do next
is the harder question.

---

## 8. Updated commercial differentiation (post-code-review)

After reviewer-Claude looked at the actual code (not just READMEs),
the commercial differentiation is **sharper** than earlier framing
allowed:

**"Sovereignty-leaning CMS / static-site platform" understates it.** The
positioning is:

> Typed CMS → in-process render → typed audit pipeline →
> cryptographically-attested build artifact → static OR hybrid
> OR dynamic output → identical primitives across modes.

That story is competitive with:

- **Astro** — which has dynamic but lacks audit gating +
  attestation
- **Vercel** — which has hybrid rendering but is a hosting
  commitment
- **Webflow** — which has neither typed schemas nor verifiable
  builds
- **Next.js** — which doesn't gate on accessibility / performance
  / security at build time

Per audience the differentiation is concrete:

- **Agencies + in-house teams** — verifiable accessibility /
  performance / security gates as platform features, not bolted
  on
- **Compliance-sensitive customers** — hand auditors a signed
  Merkle chain of every build that produced production
- **Sovereignty-conscious operators** — same primitives static or
  dynamic without committing to a specific hosting topology

**The wrong sales axis is "static-only."** The right axis is
*"verifiable build pipeline that emits whatever shape your
deployment needs"* — static for cheap edge-delivery, hybrid for
progressive enhancement, dynamic for SPA UX, all from the same
source with the same audit guarantees. Sells to a substantially
wider audience without compromising sovereignty.

### 8.1 Sellable artifacts in their own right

Some Forge subsystems are commercially packageable independently:

**`backends.toml` pattern** — most platforms have the
frontend-backend drift problem and have given up solving it.
Static configuration CI-gated against actual UI + actual backend
implementation, four finding types covering every possible drift
state, fatal in production. **Could be extracted as a separate
open-source tool for use with any frontend framework.** "We
solved the orphan-button / orphan-endpoint problem" is a niche
but compelling pitch to engineering leaders who've been bitten
by it.

**`forge attest`** is a compliance + trust artifact. Cryptographic
build chain with externally-publishable public keys for auditor
pinning is meaningfully ahead of competitors on supply-chain
security. Customers in finance / healthcare / government /
journalism increasingly require artifact attestation (SLSA,
Sigstore, in-toto). **Forge has the substrate; the
productization is packaging it as a verifiable-build feature in
marketing rather than burying it as an internal AVP-2 mechanism.**

**SkillShots PoC.** Real consumer product proving the substrate
handles dynamic, transactional, video-uploading, money-touching
workflows — not just brochure-ware. **Substantially stronger
marketing story** than "Paul's own marketing sites + Sacred.Vote"
because it's an end-to-end case study with 20+ backends, Stripe
integration, S3 multipart upload, live streams.

---

## 9. The honest closing question

The architecture is sound; the code is more mature than the
docs suggest; the product surface is bounded by principles, and
those principles are coherent. **Expanding scope to commercialization
is a decision about what kind of company PlausiDen Technologies
LLC is going to be** — consulting + tooling shop selling support
around an OSS toolkit, hosted-product SaaS company with
sovereignty as a brand position, civic-tech vendor selling
Sacred.Vote to municipalities, privacy-infrastructure vendor
selling MCP + related tools to AI companies, desktop privacy
toolkit vendor selling the suite to individuals, or some
combination.

**These are different companies.** Different organizational
shapes, different sales motions, different customer support
functions, different capital structures, different team
compositions, different time-horizons-to-revenue. Not all
simultaneously addressable by one founder in any reasonable
timeframe.

The most leveraged version of "expand the scope and sell" is
probably:

1. **Pick one of the three commercial products with the strongest
   signal-to-effort ratio.** Loom+Forge+Crawler bundle as the
   sovereignty-leaning publishing platform is probably it.
2. **Commit to a specific tier of commercialization** — probably
   Tier 1 sliding toward Tier 2. Decide deliberately rather than
   letting it drift.
3. **Defer the other commercial threads** until the first has
   product-market fit.
4. **Update OPERATING_PRINCIPLES** to reflect commercialization
   as deliberate scope expansion with documented trigger
   thresholds for the conditions that have changed.
5. **Plan team + capital implications honestly.** Tier 1 still
   eventually requires you not being the only person doing every
   job. Grant funding aligns with values but caps growth rate;
   revenue funding requires actual sales execution; VC pressures
   toward growth-at-all-costs in ways that often conflict with
   sovereignty commitments. **Pick the funding model that fits
   the company shape you want; accept its constraints.**

Work to build commercial Forge on top of the existing substrate:
**probably 18 months to 2 years of focused work to reach a
defensible commercial position**, assuming the architectural
substrate continues at current quality + one of the existing
commercial threads is picked and pursued seriously.

**Harder work is non-engineering discipline:** sales, support,
documentation, customer success, marketing, hiring, organizational
design. Those are the dimensions where founders with strong
engineering instincts most often falter, and the part of the
commercialization plan most worth being deliberate about now.

**Architecture, you've solved. Product surface, you've bounded.
Commercialization is the next forcing function — and it's a
decision about company, not about code.**

### 9.1 The immediate blocker

Per reviewer-Claude after code review:

> If the goal is commercializing this, **the docs lag is the
> immediate blocker, not the missing features. Customers can't
> buy what they can't see.**

Three layers of progressively-more-current truth visible to a
prospective customer:

1. README narrative (most conservative — emphasized "static-site
   generator")
2. README technical content (more current — listed dynamic +
   hybrid modes)
3. Actual code (most current — type-state pipeline, build
   attestation, mutation gate, secrets gate, watch+serve dev
   loop, in-process render, type-safe Crawler boundary)

**The cheapest single thing to fix on the commercialization
path: bring the README + DESIGN.md up to date with the code.**
The product is more mature than its presentation. Fix the
presentation first.
