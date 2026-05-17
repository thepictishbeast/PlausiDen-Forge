# Forge — site operations playbook

> Companion to [`FORGE_VISION.md`](./FORGE_VISION.md) and
> [`ARCHITECTURE_PRINCIPLES.md`](./ARCHITECTURE_PRINCIPLES.md).
> Vision says what Forge does; principles say why; **this doc says
> what every published site needs to *operate* in the real world.**
>
> The thesis: most CMSes hand users a published site and abandon
> them. The gap between "site exists" and "site succeeds" is
> filled by a hundred small actions experienced operators take
> automatically and beginners don't know exist. Forge systematically
> closes this gap — via scaffolds, guided generators, contextual
> reminders, and automation where possible.

---

## 1. Required pages by site type

New-site creation includes a `site type` declaration: `business`,
`personal blog`, `ecommerce`, `saas product`, `nonprofit`,
`government`, `education`, `anonymous publishing`. Each type ships
a required-pages template. Required pages are created as **drafts
on day one**, pre-populated with sensible content, flagged for
review. *Empty by default means missing forever; pre-stubbed means
visible and finishable.*

### Legal foundation (almost every site)

| Page | Required when | Notes |
|---|---|---|
| Privacy Policy | Always (GDPR, CCPA, every state privacy law) | Guided generator — structured Q&A → policy reflecting actual practice |
| Terms of Service | Almost always | Limits liability, sets jurisdiction, defines acceptable use, refund policy |
| Cookie Policy | If any non-essential cookies | Separate doc under GDPR; lists each cookie's purpose + duration; **auto-populated from detected scripts** |
| Acceptable Use Policy | If UGC exists | Specific enumeration, not "we reserve the right" hand-waving |
| DMCA / copyright complaint | If hosting third-party content | **US safe-harbor requires $6 USPTO designated-agent registration** — most sites skip this and lose the protection |
| Accessibility Statement | EU (EAA), increasingly everywhere | Generated from actual conformance, not aspirational claims |
| Data Processing Addendum availability | B2B selling to EU customers | DPA template + SCCs |
| Sub-processor list | If GDPR-scoped | Public, notified-on-change |
| Imprint / `Impressum` | DE / AT / CH always; other EU markets variably | Legally required identifying info |
| Modern Slavery Statement | UK / AU / CA over revenue thresholds | |
| Trust center | Enterprise sales | Security certs, audit reports, compliance docs |

### Identity and trust

About; Team / leadership (B2B credibility); Contact with **multiple
methods** ("contact" with only a form looks evasive); Press /
media kit; Careers if hiring; Customers / case studies /
testimonials (real ones, with permission, never stock photos with
fake names); Logo wall; Awards.

### User support

FAQ (organized by user journey stage, not random); Help center /
KB; Documentation; Tutorials; Video walkthroughs; Glossary;
Community forum link; Support hours + SLA; **Status page** at
`status.yourdomain.com` with subscribe-to-incidents capability.

### Discovery and conversion

Homepage with clear value prop above the fold; Product / features;
Pricing with comparison table + FAQ + guarantee terms; Use cases /
solutions (same product positioned for different personas);
Comparison pages (Us vs. Competitor — controversial but effective);
Integrations directory; Templates gallery; Blog with categories,
tags, RSS, author pages.

### Operational

Search results; 404 (designed, helpful, navigation back to value
+ search + popular pages — **not a generic dead end**); 500 / error
(informative, status page link); Maintenance; Newsletter signup
confirmation (double opt-in); Unsubscribe in one click (CAN-SPAM,
GDPR); Email preference center; Account / settings; Login, signup,
password reset, email verification; Onboarding sequence; **Empty
states for every list / feed / dashboard**.

### Discoverable infrastructure

`sitemap.xml` (auto-generated); HTML sitemap (often forgotten);
`robots.txt` configured deliberately; `humans.txt`; **`security.txt`
declaring vulnerability disclosure contact (RFC 9116)** — every
site should have this; `ads.txt` if monetized; RSS / Atom; JSON
Feed; OPML; `manifest.json`; Apple touch icons, favicons in all
sizes; Open Graph / Twitter Card images per page; Schema.org
JSON-LD per content type.

### `.well-known/` discoverable standards

| Standard | Purpose |
|---|---|
| `change-password` | RFC redirect to password change; used by password managers |
| `apple-app-site-association` | iOS universal links |
| `assetlinks.json` | Android app links |
| `openid-configuration` | If OIDC provider |
| `nostr.json` | If culturally relevant |
| `security.txt` (RFC 9116) | Vulnerability disclosure contact |

### DNS records every site needs

| Record | Purpose | Forge action |
|---|---|---|
| SPF | Sender authentication | Generate based on email infra used |
| DKIM | Sender authentication | Platform-managed key, rotation, monitoring |
| DMARC | Auth alignment + reporting | Start `p=none`, progress to `quarantine` then `reject` based on aligned reports |
| CAA | Restrict which CAs can issue certs | Lock to your declared issuers |
| HSTS (Strict-Transport-Security) | Force HTTPS | Auto; preload-list submission for high-traffic |
| MTA-STS | Email TLS enforcement | Optional, increasingly expected |
| BIMI | Verified logo in email clients | Requires VMC cert ($), increasingly worth it for branded mail |
| DNSSEC | DNS integrity | Enable at registrar |

Forge detects required records from platform features in use and
either pushes via registrar API (Cloudflare / Route53 / Namecheap
/ Porkbun) or shows copy-pasteable values with provider-specific
screenshots.

---

## 2. Live linkage between practice and policy

The underrated capability: **drift between practice and stated
policy is what regulators are increasingly checking for, and
what most companies fail at.** Forge binds them at the source.

- Cookie banner's cookie list generated from actually-running
  scripts (detected by the CMP). Cookie Policy reads from same
  source. Add a tracking pixel → policy updates automatically.
- Privacy Policy's "data we collect" section references the
  schema's PII-tagged fields. Adding a PII field updates the
  policy.
- Sub-processor list reflects actual integrations. Integrate
  Stripe, Postmark, OpenAI → sub-processor list updates with
  per-processor disclosure (purpose, data shared, jurisdiction,
  DPA link).
- Accessibility Statement generated from primitive conformance.
  Pages using escape-hatch primitives that haven't passed audit
  appear in the "known issues" section.
- GDPR Article 30 record of processing activities generated from
  the schema rather than maintained as a Word doc.

**Drift becomes structurally impossible** — same architectural
principle as capability-manifest projection.

---

## 3. Contextual reminders during authoring

Reminders appear when relevant, not as a permanent nag list. The
CMS knows what was just done and what the action implies.

- Publishing a page with a contact form → "Review Privacy
  Policy's 'how we use form submissions' section."
- Adding analytics → "Cookie banner configuration required."
- Adding a newsletter signup → "CAN-SPAM / GDPR consent text +
  unsubscribe link in templates."
- Adding an ecommerce primitive → "Return policy, shipping
  policy, terms of sale required."
- Adding a plugin that makes network requests to X.com → "This
  plugin will leak visitor IPs from Tor-mode sites. Override
  required to proceed."

### Severity tiers

Legal compliance + security issues = **urgent**. SEO improvements
= **important**. Polish items = **nice-to-have**. The CMS
communicates priority clearly. A missing OG image is not the same
as a missing Privacy Policy.

### Dismissible with memory

"I don't want Google Analytics" dismisses that recommendation
**permanently**, not re-surface next session. Respect choices.

### Education embedded, not separated

Rather than a separate "learn SEO" course, every recommendation
includes a one-paragraph explanation of *why it matters*, with
a link to deeper reading. Learning happens in the flow of work.

### Jurisdiction-aware behavior

Site declares operating jurisdictions and target markets. The CMS
adapts:

- EU traffic → GDPR cookie banner (legal-by-default, not the
  illegal-pre-ticked-boxes pattern)
- California traffic → CCPA "Do Not Sell" link
- Brazil → LGPD compliance
- Age-restricted content (alcohol, gambling) + the right
  jurisdiction → age verification flow
- Operating thresholds reached → tax registration prompts (UK
  VAT, EU OSS, US state nexus)

Jurisdiction is **data, not assumption**; compliance UI follows
from it.

### Template freshness as a maintained system

Legal templates + required-page scaffolds aren't a one-time
generator output — versioned, maintained by the platform vendor,
updates flow to existing sites as suggested changes. New EU
regulation drops, cookie banner template updates, every site sees
a "recommended update" notification with diff and one-click apply.

---

## 4. Site-success operational layer

The codified expertise — operational knowledge experienced web
operators internalize over years — made systematically available
to first-time users. **Competitive advantage of doing this well
is enormous** because most CMSes (Webflow, Squarespace, Wix)
hand users a published site and abandon them.

### Discovery and indexing

- **Search engine verification.** Google Search Console, Bing
  Webmaster Tools, Yandex Webmaster (if relevant), Baidu (if
  China-targeting). Forge automates: generates verification meta /
  DNS TXT, walks the OAuth flow where possible, surfaces Search
  Console data **inside the CMS dashboard** rather than forcing
  context-switch. Sitemap auto-submitted. Crawl errors,
  indexation status, manual actions surfaced as alerts.
- **Business directory submissions.** Google Business Profile
  (massive local-SEO impact, free, single highest-leverage local
  action). Bing Places, Apple Maps Connect, Yelp, TripAdvisor,
  industry-specific (Avvo / Healthgrades / Houzz). Checklist
  surfaces based on declared business type + location. Consistent
  NAP (Name, Address, Phone) across all — inconsistency tanks
  local SEO; Forge enforces consistency from a single source.
- **Social profile claiming.** Reserve handles on every major
  platform even if not actively used — prevents impersonation
  and brand confusion. Knowem-style audit in one pass: Twitter/X,
  Instagram, Facebook, LinkedIn, TikTok, YouTube, Pinterest,
  Threads, Bluesky, Mastodon. Brand monitoring after.
- **Knowledge graph + entity establishment.** Wikidata entry
  where notable. Schema.org Organization / Person markup with
  `sameAs` links to verified social, Crunchbase, LinkedIn,
  Wikipedia — establishes entity in Google's knowledge graph
  for brand SERPs with rich panel display.

### Content / SEO operating practice

- **Keyword + topic research integration.** Native using public
  data (Google Suggest, Search Console queries, PAA boxes), or
  connect Ahrefs / Semrush. Suggest content topics from declared
  niche + competitor analysis + existing content gaps. **Surface
  opportunities, don't generate content automatically** (AI-
  generated content without curation is increasingly penalized).
- **Internal linking suggestions.** Semantic-similarity-based
  suggestions when publishing. Most sites under-link internally.
- **Anchor text discipline.** Lint vague "click here" on publish.
- **Heading hierarchy.** One H1; semantic levels (don't skip for
  visual styling — use CSS).
- **Live SERP previews.** Title (50-60 chars before truncation),
  meta (150-160), OG image (1200×630), Twitter Card — all
  previewed live during editing.
- **Content freshness.** High-traffic-pages-by-age list prompts
  periodic refresh. *"Refresh"* means actual updates, not just
  bumping publication date (Google catches the manipulation).
- **Publication cadence.** A blog with two posts a year ranks
  like one. Cadence shown; realistic schedules encouraged.
- **Topical authority clustering.** Pillar pages + cluster
  content linking back. Forge visualizes the topic clusters in
  the content graph and surfaces gaps.

### Email deliverability

Most small-business emails land in spam because nobody set up
SPF/DKIM/DMARC. Forge — even when the customer uses external
email like Google Workspace — detects the receiving email provider
from MX records and offers to generate the right authentication
records. **Monitor DMARC reports** (received as XML at a specified
inbox, parsed by Forge, surfaced as a dashboard showing which
senders are authenticating, which are failing, what to fix).

### Registrar best practices

- Domain locked at registrar (prevents transfer hijacking)
- 2FA on registrar account
- Registrar supports DNSSEC and DNSSEC enabled
- Auto-renew with payment method that won't expire
- **Registrar contact email NOT on the same domain** (catastrophic
  if domain expires and renewal warnings can't be received)
- Registrar separate from hosting provider (blast-radius
  separation)

Surfaced as one-time setup checks with verification.

### Domain expiry + cert monitoring

Auto-renew **fails** when payment methods expire. Forge alerts
before expiry, not after. SSL renewals fail occasionally; Forge
alerts before expiry, not after.

### Backup + recovery awareness

- Platform backups automatic + tested via monthly restore drills
- User can download a **full external export periodically**
  (sovereignty: external copy protects against platform-level
  disasters — account suspension, vendor closure, billing
  lockouts)
- Operational runbook (where domain is registered, who has
  access to what, where backups go, recovery procedure) stored
  *outside the site* so it's accessible when the site is down

### Trust signals

- HTTPS (auto)
- Trust badges composited from declared sources (uptime from
  status page, certifications from compliance metadata, customer
  count from billing, awards from curated list, press from
  tagged content type) — bind to source data so badges never go
  stale
- Customer reviews via Google / Trustpilot / G2 / Capterra
  integration with schema.org Review markup for rich snippets
- Physical address + phone (builds local trust, legally required
  in some jurisdictions)

---

## 5. Asset and content pipeline

- **Image pipeline.** Accept any format → transform to optimal
  delivery (AVIF + WebP + JPEG fallbacks), generate responsive
  variants, extract EXIF, **strip PII from EXIF** (GPS, camera
  serial), generate alt-text suggestions from vision models,
  deduplicate by content hash. Generated LQIP placeholders.
- **Video pipeline.** Transcode to HLS / DASH at multiple
  bitrates, thumbnails, captions auto-generated, transcripts
  auto-generated.
- **PDF.** Indexed for full-text search.
- **Asset library.** Searchable, taggable, AI-described.
- **Document metadata stripping.** Author names, software
  versions, edit history — stripped at upload via MAT2 /
  ExifTool. One of the most common deanonymization vectors.

---

## 6. Forms as first-class primitive

Form builder generating **accessible** forms with:
validation, anti-spam (honeypot + Turnstile + rate limiting +
Akismet), submission storage, email notifications, webhook
delivery, integration to CRM / email / Slack, GDPR-compliant
consent capture, file uploads with **virus scanning**,
conditional logic. Submissions queryable as structured data.

One of the single most-used CMS features and the one most
platforms get wrong (security holes, a11y failures, no spam
handling).

---

## 7. Search

Site search that **works**: typo tolerance, faceted filtering,
ranking by recency + relevance, instant results, search
analytics revealing what users want and can't find, content gap
detection. Typesense / Meilisearch / Algolia under the hood
depending on scale. Most CMS native search is unusable.

Beyond site search: cross-content semantic search using
embeddings, hybrid keyword + vector, RAG over customer content
for AI assistants the customer deploys, structured queries.

---

## 8. Multi-network publishing as a first-class capability

A site declares itself reachable on clearnet, Tor (`.onion`), I2P
(`.i2p`), Lokinet (`.loki`), Hyper, IPFS (via IPNS), Gemini
(`gemini://`), Yggdrasil mesh. Each network has different threat
models, operational requirements, audiences, configurations. Forge
treats each as a **typed deployment target** with its own
constraints, gates, security profile.

### Networks supported

| Network | Use case | What Forge provisions |
|---|---|---|
| **Tor onion v3** | Operator anonymity + reader privacy | ed25519 keys (or vanity via mkp224o), hidden service config, OnionBalance for HA, optional client-auth, Onion-Location header on clearnet, single-vs-full onion choice |
| **I2P eepsites** | Different threat model (garlic routing) | i2pd router (Java client avoided), `.b32.i2p` destination, optional jump-service registration |
| **Lokinet SNApps** | Smaller audience, lower latency than Tor | Service node config, `.loki` address |
| **IPFS + Hypercore** | Censorship resistance (NOT anonymity) | Pin to multiple gateways, IPNS key management, Hypercore append-only log |
| **Gemini / Gopher** | Smolnet / small web | Capsules generated from typed content stripped to protocol constraints |
| **Yggdrasil / CJDNS** | Mesh / community networks | IPv6 mesh declaration |

### Threat-model tiers Forge supports

The fundamental decision tree Forge walks the user through.
**What's the threat model?**

1. **Reader privacy only.** Operator runs normal clearnet site +
   publishes onion mirror so visitors who care can read over
   Tor. Operator doesn't care about being known. Clearnet + onion
   share infrastructure. Newspapers (NYT, BBC, ProPublica),
   corporations (Facebook, Twitter, ProtonMail) operate this way.
   Easiest case — a checkbox.
2. **Operator pseudonymity.** Publishing identity separate from
   legal identity, not defending against nation-state attribution.
   Workable with reasonable opsec: dedicated infra paid with
   cryptocurrency from a clean wallet, separate authoring
   environment, no cross-network linking, no personally-identifying
   patterns. Forge provides infra isolation but **cannot enforce
   behavioral opsec**.
3. **Operator anonymity against capable adversaries.** Defending
   against intel services, large corporations with legal access,
   sophisticated criminal investigation. Requires substantially
   more discipline than Forge alone provides — Tails for
   authoring; Monero properly mixed; hosting without KYC paid
   anonymously in adversary-inconvenient jurisdictions; writing-
   style discipline; legal anonymity in entity structure. **Forge
   provides the technical infrastructure and explicitly documents
   what it cannot enforce.**

Earning trust = being explicit about the boundary between what
infrastructure provides and what the operator must provide
themselves. Most "anonymous hosting" services oversell.

### Security rating dashboard

Per-dimension assessment, presented as a dashboard. Each
dimension green / yellow / red with one-line explanation and
click-through to technical detail.

**Anonymity dimensions:** Any clearnet resource loads from the
onion site (external script, image, font, analytics, social
embed)? Server clock NTP-synced with revealing pool? TLS cert in
transparency logs linking it to operator? Package update patterns
reveal OS + version? HTTP server sends identifying headers
(`Server`, `X-Powered-By`, `ETag` revealing filesystem)? Error
pages reveal stack traces / paths / framework versions? JavaScript
present, and if so phone home / fingerprint / load external?
Cookies persist beyond session? Onion site mirrored on clearnet
with identical content enabling correlation? Timestamps include
timezone? Writing style fingerprint matches known-attributed
content (stylometry via JStylo / Anonymouth)?

**Content secrecy:** Reachable only on Tor, or does clearnet
caching exist (Wayback, Google Cache, archive.today)? RSS / sitemap
exposing intended-discoverable-only-by-direct-navigation content?
Search-engine index status appropriate (noindex where required)?

**Reader safety:** Works without JavaScript (Tor Browser "Safest"
disables it)? Without cookies? Tor Browser Safest level? Avoids
font fingerprinting (Tor Browser ships limited fonts; non-standard
forces fallback revealing the user is on Tor)? Avoids CAPTCHAs
that fail or harass Tor users? Works in text-mode browsers
(Lynx, w3m) for max-security reader setups?

**Operator security:** Admin interface accessible ONLY over Tor?
Admin onion address separate from public + not derivable? Admin
creds hardware-backed (YubiKey, Nitrokey)? Server hardened beyond
stock (kernel hardening, AppArmor/SELinux, audit logging, IDS)?
Hosting provider chosen for threat model (Hetzner fine for most,
terrible if adversary has German legal access; Njalla for
pseudonymity; offshore for jurisdictional inconvenience)? Payment
chain anonymous? Backups encrypted, destination not correlated to
operator?

**Infrastructure:** NTP randomized / hardened? MAC randomized at
provisioning? Disk encrypted with keys not in cloud KMS? Logs
minimized (retention short + content has no IPs / UAs / referers;
ideally no logging where compliance permits)? Swap disabled or
encrypted? Kernel hostname not revealing? No cron jobs contacting
clearnet?

### Specific technical controls

- **Stripped HTTP responses.** Server header removed/randomized.
  Date in UTC. No ETag (filesystem inode leak). No
  `X-Powered-By`. CSP locked down — `default-src 'self'`. No
  external sources permitted by default. `Referrer-Policy:
  no-referrer` for onion. `Permissions-Policy` deny-all sensors.
- **No clearnet leakage by construction.** Primitive system
  enforces this at the primitive layer — primitives used on a
  Tor-only site cannot include external resources unless those
  resources are also served from the onion. External fonts
  replaced with system fonts or onion-hosted variants. External
  images proxied through onion or rejected at publish. Embeds
  (YouTube, Twitter) replaced with link-only fallbacks or proxied
  through onion-friendly alternatives (Invidious, Nitter,
  Teddit). **Safe path = default path.**
- **Onion-Location and onion-aware behavior.** Clearnet emits
  Onion-Location header pointing to onion variant. Tor Browser
  displays the option to switch.
- **Time-resistant publishing.** Posts published with timezone
  normalized to UTC. Publishing times jittered or batched to
  avoid revealing operator working hours. Most operational
  deanonymization of pseudonymous publishers comes from timing
  correlation; make timing discipline easy.
- **Metadata stripping** by default (EXIF, document metadata,
  audio recording metadata).
- **Server config discipline.** SSH on server reachable only
  over onion. No reverse DNS to identifying hostnames. No web
  server status pages exposed. Custom error pages revealing
  nothing. Audit logging configured but scrubbed of identifying
  data.

### What Forge **cannot** provide and must warn about

This is what distinguishes a serious tool from marketing. **Forge
is brutally honest about what infrastructure cannot do:**

- **Behavioral opsec is the operator's job.** Writing style,
  posting times, topic selection, accidental personal references,
  photos with identifying backgrounds, screenshots with
  identifying UI elements, file names with personal context.
  Forge scans + warns for some (EXIF, filenames, mentioned names
  against a configured do-not-mention list) but cannot enforce.
- **Network opsec is the operator's job.** Connecting to admin
  from any identified network even once may be catastrophic.
  Forge can mandate Tor for admin (and should) but cannot
  ensure the user is taking care upstream.
- **Legal opsec is the operator's job.** Entity structure,
  jurisdiction selection, who knows what, what's documented in
  writing where. Forge does not provide legal advice.
- **Financial opsec is the operator's job.** How hosting is paid
  for. Whether the cryptocurrency has been mixed properly.
  Whether the payment trail eventually leads back to a KYC
  exchange.
- **Physical security is the operator's job.** Where the laptop
  is when used. Webcams. Screen visibility from windows. Smart
  speakers. Border crossings.
- **Endpoint security is the operator's job — mostly.** Forge
  recommends Tails / Whonix and refuses to accept admin
  connections from configurations that obviously aren't using
  Tor, but cannot prevent the user from using a malware-infected
  machine. Hardware-level threats (Intel ME, Pluton, firmware
  backdoors) are beyond Forge's reach.
- **Adversaries with legal access to infra providers.** If the
  hosting provider can be compelled to image disks or capture
  traffic, the user's threat model has to account for that.
- **Adversaries with physical access.** Server seizure during
  operation is the worst case. Forge supports full disk
  encryption, encrypted swap, encrypted backups, key separation,
  panic features (immediate shutdown, optionally with wipe where
  legally defensible) — but physical seizure of a running,
  decrypted server is approximately game over.

### Education layer

Forge ships an **operator's handbook** — not a marketing page, an
actual operating manual. Threat modeling, opsec layers, network
selection, hosting selection, payment selection, authoring
environment, content discipline, timing discipline, stylometric
discipline, incident response, known historical cases of operator
deanonymization with lessons (Silk Road's DPR — Bitcoin tracing
+ CAPTCHA leak; Freedom Hosting — JS exploit deanonymizing
visitors; AlphaBay — Hotmail recovery email; many others).
Studying real failures is the fastest way to internalize the
threat model. Versioned. Available offline. Translated where it
matters most (Persian, Russian, Arabic, Mandarin, Spanish, French).

### Plausible deniability layer

Integrates with PlausiDen tooling (synthetic-activity generation,
decoy services, traffic obfuscation, deadman-switch publication,
Shamir-shared keys with geographically distributed trustees).
Forge becomes the publishing surface; PlausiDen handles the
obfuscation layer.

---

## 9. Mainstream vs sovereign — the explicit duality

Most of the market wants frictionless integration with the
mainstream surveillance economy. Some users want the opposite.
**Forge serves both with explicit choice, honest tradeoffs,
educational framing.** Neither mode is judged; both are offered
deliberately.

### Profile presets

At site setup, an explicit values declaration: *"What matters
most for this site?"*

| Preset | Defaults |
|---|---|
| **Standard corporate** | Mainstream cloud (AWS/GCP/Azure), GA4 + Tag Manager + Meta Pixel with appropriate consent, social login, Stripe, mainstream email infra |
| **Privacy-respecting business** | First-party analytics, sovereign hosting where viable (Hetzner/Scaleway/OVH), passkey-first auth, Plausible/self-hosted analytics, opt-in cookies legal-by-default |
| **Anonymous publishing** | Tor-mode constraints from §8, no JS by default, no clearnet leaks, hardened defaults, operator's handbook surfaced |
| **Personal blog** | Balance of simplicity + privacy, no surveillance by default |
| **Compliance-focused enterprise** | SOC 2 / ISO 27001 stack, audit log export, SSO, custom contracts pathway |

Users start from a preset and customize.

### Values configuration panel

Visible in settings, shows current choices in plain language with
**what each implies**:

- Hosting provider (mainstream cloud vs sovereign vs self-hosted)
- Analytics stack (surveillance vs first-party vs none)
- Authentication options (social vs email vs passkey)
- AI providers (hosted frontier vs self-hosted open models)
- Embed handling (native trackers vs proxies vs disabled)
- Payment processors
- Email infrastructure

Each setting shows tradeoffs with links to deeper documentation.
**Non-judgmental throughout.** A small bakery probably wants
Stripe + GA + Mailchimp; pretending otherwise is condescending.
A user publishing on Tor probably wants none of those; offering
them prominently is malpractice.

### Why support both

The architecture proves itself in both contexts. A platform that
handles the high-end privacy case forces architectural discipline
(sandboxed extensions, primitive contracts, no clearnet leakage by
construction) that **benefits the mainstream case too** — every
mainstream site benefits from a CMS that *could* run a Tor site
safely, because the discipline shows up as better default
security, cleaner extension model, less performance bloat, more
honest defaults.

The architectures aren't in opposition; they're points on the
same axis. Designing for the harder case improves the easier
case as a byproduct.

---

## 10. Things most site owners forget

A non-exhaustive operator-tip checklist Forge surfaces
contextually:

- Favicon in **all** sizes (Apple touch 180×180, Android 192 +
  512, Windows tile, browser config) — generated from one upload
- OG + Twitter Card per page (default site-wide, override per
  high-value page)
- 404 that helps (search, popular, recent, "report broken link")
- Site search that works
- Newsletter signup placement that converts (inline within
  content, footer, non-intrusive corner) — track which converts
- Contact form **confirms submission visibly** with expected
  response time + alternative contact methods
- Loading states for anything >200ms (skeletons, optimistic UI)
- Error states that explain what happened + what the user can do
- Empty states designed (illustration, explanation, clear next
  action)
- Print stylesheets (clean typography, hidden nav, expanded
  URL link text)
- Email previews tested in major clients (Gmail, Outlook, Apple
  Mail) — bundle Litmus/Email-on-Acid-style rendering
- Mobile testing on **actual devices**, not just devtools
- Cross-browser testing — Safari and Firefox have real
  differences that desktop Chrome masks
- A11y testing with real assistive tech (automated catches ~30%)
- Spam protection on all forms (honeypot + Turnstile + rate
  limit + content classifier)
- Conversion tracking aligned with business outcomes
- Social sharing via Web Share API / native intents, not 50
  third-party scripts
- Trademark consideration before brand investment (USPTO TESS
  check)
- Copyright registration for high-value original works
- Recovery email + phone **separate from the business domain**
- 2FA on registrar, hosting/CMS, email, social — passkeys where
  supported
- Password manager + WebAuthn discovery
- Document the operational runbook **outside the site**

---

## 11. Surfacing strategy: progressive disclosure on a learning curve

Setup checklist for the first session focused on launch-critical
items. Operational checklist for the first week (verify search
engines, set up email auth, claim business profiles). Monthly
check-ins surface drift (DMARC report shows new senders, content
freshness drops, sub-processor list changed). Annual reviews prompt
bigger items (legal page refresh, accessibility audit, security
review).

**The pattern:** automated where possible (Forge generates
sitemap, sets up SSL, optimizes images, strips EXIF); surfaced
where it requires decisions (claim Google Business Profile,
write Privacy Policy review with counsel, choose AI provider).

The competitive advantage of doing this well is enormous because
the gap between "platform that publishes a site" and "platform
that helps a site *succeed*" is a chasm most CMSes never cross.

The deeper synthesis: Forge becomes not just an authoring tool
but an **operating partner** — which is the positioning that
lets it command premium pricing and durable customer relationships.
