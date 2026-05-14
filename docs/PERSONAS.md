# PlausiDen-Forge Personas

> **Status**: design doc. T76 cycle 89 closes Forge T61 (web-designer persona)
> and T62 (end-user persona). Living document — update as the product gains
> a fifth, sixth, nth persona, OR as observations from real users contradict
> assumptions baked in here.

## Why persona docs exist

Every product decision implicitly answers "who is this for, and what are
they trying to do?". Writing the answers down makes the assumptions
auditable: if a feature ships that nobody in this document would
actually use, the feature is wrong. If a real user shows up and doesn't
match any persona, the document is wrong (and gets a new persona, not a
brushed-aside complaint).

Five personas below. Each gets:

- A short identity sketch (who they are, what they care about).
- Goals (the outcomes they're after).
- Pains (what currently blocks them).
- Forge superpowers (what THIS product specifically gives them that
  the alternatives don't).
- Anti-features (things we should NOT ship for them — these decay into
  scope creep).
- One sample user story written first-person.

## P1 — Devon, the design-system author

> **Pronouns**: any. **Role**: senior front-end / design-systems engineer
> at a mid-size company. **Forge experience**: power user.

**Identity.** Devon has spent five years building component libraries and
hates how every prior attempt to roll one out at scale eroded under
real-world pressure: marketing pages shipped raw HTML, contractors
pasted Tailwind classes verbatim, a CMS dropped inline styles into the
DOM, six months in nobody remembers what `--brand-primary-50` actually
resolves to. Devon picked Forge specifically because the linter
*refuses* raw class strings — the doctrine is enforced by the compiler,
not by code review.

**Goals.**
- Ship a typed token system + a small primitive layer (Button, Card,
  Hero, Section) once, then never touch it again unless the brand
  changes.
- Have a single command that proves "the system is internally
  consistent": every theme declares the same set of tokens, every
  `var(--…)` reference resolves, every contrast pair passes WCAG.
- Bring a new visual designer onboard in under an hour. The system,
  not the docs, should onboard them.
- Treat the design system as a *contract* between design and
  engineering. PRs that violate the contract fail CI.

**Pains.**
- Other systems require manual audits at release time. Forge's lint +
  audit phases make that an always-on background process.
- "Mobile + desktop" testing usually means an engineer eyeballs four
  breakpoints in DevTools. Devon wants pixel-level visual diff (Forge
  T33 phase_visual_diff: 4 themes × 3 viewports snapshot grid).
- New components get added in PRs and merge without anyone noticing
  they don't pass contrast in one specific dark-mode tile. Devon
  wants a CI step that flags this before merge.

**Forge superpowers.**
- `forge` build phases are *typed*: each phase implements a Rust
  trait, returns a `Vec<Finding>` with severity, and the pipeline
  short-circuits on errors. Doctrine breaches show up as findings
  with severity `Strict`, blocking deploy.
- `theme_consistency`, `theme_contrast`, `tokens`, `contrast`, and
  `dual_theme` phases mean Devon's token + theme contract is
  enforced on every build.
- Merkle-chained `reports/build-*.json` give Devon an auditable
  history of "what was the state of the design system on date X".
- The Loom CMS sits on top: when content editors mutate a CMS page,
  the underlying typed components are still the only renderable
  primitives. The content can change; the design contract can't.

**Anti-features.**
- Plugin marketplaces. Every plugin is unaudited code that bypasses
  the doctrine. Hard no.
- Drag-and-drop "free positioning" of components. The grid is
  4/8-px-aligned for accessibility reasons; ad-hoc placement
  defeats the system.
- Inline-style escape hatches. There is no `style="…"` prop on any
  primitive. If a designer needs something the system doesn't
  support, the answer is to extend the system in a separate PR,
  not to special-case the page.

**Sample story.**
> "I rolled out a new accent color last quarter. I edited two tokens
> in `loom-tokens/src/skin.css`, ran `forge` once, and saw the build
> fail with three Strict findings: a button variant in the
> contractor's marketing landing page no longer met 4.5:1 contrast,
> a CSS variable I'd renamed wasn't updated in the legacy `/about`
> page, and the dual-theme snapshot for the hero section was now
> visually inconsistent. I fixed all three in 20 minutes. Before
> Forge, that change would have shipped, been caught by a customer
> on Twitter, and burned a week of cleanup."

---

## P2 — Maya, the content editor

> **Pronouns**: she/her. **Role**: communications coordinator at a
> small nonprofit. **Forge experience**: zero. Uses Loom CMS daily,
> never touches Forge directly.

**Identity.** Maya is a non-technical content owner. She publishes a
weekly blog post, swaps imagery on the homepage hero, and updates the
"Our Programs" page when a new initiative launches. She's been burned
twice in previous jobs: a CMS that let her break the live site with a
typo, and a CMS that locked her out of edits because every change had
to go through a developer first. Maya wants something between those
two extremes: she should be able to publish without help, but the
system should physically prevent her from breaking the design.

**Goals.**
- Edit one piece of content at a time. Inline edits feel like
  Google Docs; she doesn't want to learn a new mental model.
- Save automatically. Losing 20 minutes of writing to a browser
  crash is an unforgivable product failure.
- See revisions of her own work — if she changes her mind, "undo"
  goes back further than the last keystroke.
- Trust the design. She should never have to ask "does this look
  right on mobile?" because the system has answered that already.

**Pains.**
- Past CMSes asked her to choose between fonts, colors, and
  layouts. She doesn't want those choices; they're design
  decisions, not content decisions.
- Image uploads in past tools shipped at 4 MB and made her pages
  slow. She wants the system to handle compression and srcset.
- "Preview" buttons that show desktop only. She publishes on her
  phone half the time.

**Forge / Loom superpowers.**
- Inline editing on the rendered page (Loom T42). Click a heading,
  type, save. The CMS surface is the rendered surface — no
  separate "edit mode" with a different layout.
- Cmd-S to save + dirty-state visual indicator + `beforeunload`
  warning + localStorage drafts. Four layers of "don't lose
  Maya's work" (Loom cycles 79-82).
- Revision history with restore — Loom T76 cycle 81 + 84. Every
  save snapshots prior content; Maya can see, diff, and roll back
  weeks of edits.
- Drag-drop section reorder with a 6-dot handle (Loom cycle 85).
  No "move up / move down" buttons; the section literally moves
  where she drops it.
- The design contract enforced by Forge means Maya physically
  cannot break the layout. She can change text, change images,
  reorder sections; she can't accidentally inline a font or
  break the grid.

**Anti-features.**
- "Style this paragraph" picker. The paragraph already has the
  correct style — that's the contract.
- A WYSIWYG toolbar with 30 buttons. The buttons hide the
  semantic model: heading vs. paragraph vs. emphasis vs. link.
  Loom shows those four as separate section kinds; nothing else.
- A "publish workflow" with approval queues. Maya is the author
  AND the approver. Bigger orgs can add a queue later behind a
  feature flag, but it must not appear in the default UI.

**Sample story.**
> "I was halfway through writing this week's blog post when my
> battery died. I plugged in, reloaded the page, and the editor
> showed a 'restore unsaved draft from 8 minutes ago' banner. I
> clicked it. Every word came back. I finished, hit Cmd-S, and
> the page was live before my coffee got cold."

---

## P3 — Joel, the small-business owner

> **Pronouns**: he/him. **Role**: owner-operator of a regional
> coffee roastery. **Forge experience**: deployed his own site
> via a friend's recommendation; touches it ~once a quarter.

**Identity.** Joel needs a website to take retail orders, list
wholesale partners, and post the occasional event. He doesn't have
a developer on staff. He doesn't want to learn what "CSP" or
"WCAG" means, and he REALLY doesn't want a SaaS subscription that
goes up every year. Forge gives him a site that he owns, hosted
where he picks, with no recurring cost beyond the VPS.

**Goals.**
- Have a site that loads fast on bad rural mobile networks.
- Have a site that ranks for "specialty coffee [his region]"
  without him paying an SEO consultant.
- Accept retail orders via a simple form that emails him.
- Look professional to wholesale partners — clean typography,
  consistent imagery, no broken links.

**Pains.**
- His previous Wix site cost $20/mo, hijacked his domain for
  customer redirects, and inserted ad scripts he didn't approve.
- His previous Wordpress site got hacked through an outdated
  plugin and started serving viagra ads. He never trusted a CMS
  again until Forge — which doesn't have plugins at all.
- His phone's data plan is awful; pages that take 5 seconds to
  load on his own site are an embarrassment.

**Forge superpowers.**
- Static builds (Forge's `render` phase): the site is just HTML +
  CSS, no runtime server-side rendering. Loads instantly on the
  worst network.
- `seo` phase, `link_check` phase, `asset_optimization` phase:
  Forge audits Joel's site every build and tells him what to
  fix. Joel never has to learn SEO; the linter knows.
- `loom deploy` (T47) ships to Joel's VPS atomically with a
  rollback link if something goes wrong. No SSH knowledge needed.
- No plugin surface, no auto-update treadmill. Joel's site does
  exactly what Forge shipped when he installed it. If he
  upgrades Forge, he reads one release note and re-deploys.

**Anti-features.**
- A built-in shopping cart. Forge is a static site builder; if
  Joel needs commerce, he integrates Stripe Checkout or links to
  Shopify. Forge is not building yet-another-payment-processor.
- An admin UI for managing DNS / SSL / hosting. Those are real
  problems but they belong to the hosting provider.
- "Templates" that look like Squarespace stock sites. Forge ships
  one or two canonical site shapes; everything else is
  customizable via the design system.

**Sample story.**
> "I added our new winter blend to the site last night from my
> phone. Opened Loom on cellular, edited the homepage section,
> uploaded the photo (the system resized it for me), hit save,
> and saw it live before I went to bed. The whole site loaded
> in under a second on the airport Wi-Fi the next morning."

---

## P4 — Priya, the agency builder

> **Pronouns**: she/her. **Role**: independent web designer who
> takes on 4-6 client projects a year. **Forge experience**:
> heavy user across multiple sites; runs the latest version.

**Identity.** Priya builds client sites for a living. Every client
wants something a little different, every client expects bug-free
delivery, and every client's brand kit clashes with the last
client's. She picked Forge because the typed primitives let her
spin up a new site from a template in 30 minutes, then customize
only the tokens.

**Goals.**
- Onboard a new client in an afternoon. `loom site init` →
  swap tokens → write three pages → ship.
- Hand the site over to the client's non-technical staff and
  walk away. Maya (above) is who Priya's clients are; the system
  has to onboard Maya without Priya being on call.
- Re-use her design system across clients while still letting
  each client's brand stay distinct. Tokens vs. components is
  the right axis.
- Bill by deliverable, not by hour. The faster a project ships,
  the more she earns; Forge's quality gates mean she doesn't
  spend the savings on bug-fixing after delivery.

**Pains.**
- Past tooling didn't separate "design system" from "site"
  cleanly. Every client got a slightly different fork of the
  components, drift accumulated, maintenance got expensive.
- Manual cross-browser testing eats half her budget on visual
  tweaks she can't reproduce locally.
- A previous client's content team broke the layout twice. Forge's
  doctrine means that doesn't happen anymore.

**Forge superpowers.**
- `loom site init --template <kind>` (T41) scaffolds a complete
  buildable site in 30 seconds. Priya never starts from
  scratch.
- The token system makes brand swaps a 10-minute exercise: edit
  `skin.css`, run `forge`, check the contrast report.
- `forge deploy publish` + signed manifests (T47) mean Priya
  hands over a SECURE deployment story, not just a folder of
  files. Clients trust her more for it.
- Multi-tenant Loom (T45) and the Claude Code SSH bridge (T46)
  will let Priya host all her clients on one VPS with hard
  isolation — coming soon.

**Anti-features.**
- "Branding plugins" that re-skin the entire editor. The editor
  is the contract; clients see the same one Priya sees, with
  their own tokens.
- A reseller / agency program. Forge is FOSS; Priya doesn't pay
  Anthropic / PlausiDen anything to use it commercially, and
  PlausiDen doesn't run an agency-tier hosting business.

**Sample story.**
> "A new client called Friday with a launch on Wednesday. I
> spun up a site Saturday morning: `loom site init coffee`,
> imported their brand kit (one CSS file), wrote four pages of
> copy from their content brief, ran `forge` to audit, deployed
> to their VPS. Wednesday's launch went live without a hitch
> and I had Sunday off."

---

## P5 — Sam, the security auditor

> **Pronouns**: they/them. **Role**: independent security
> consultant hired by Forge-using companies to verify their
> deploys. **Forge experience**: reads the source; runs the
> tooling in adversarial mode.

**Identity.** Sam was hired by Priya's biggest client to confirm
the new site doesn't leak data, doesn't ship known-vulnerable
dependencies, and doesn't expose admin endpoints to the internet.
Sam expects a battle. Most of the sites they audit fail on
basics: missing CSP, default-admin passwords, plaintext keys
committed to git, no audit log of who-published-what.

**Goals.**
- Prove (or disprove) that a Forge-deployed site meets the AVP-2
  threat model: no plaintext outbound, hash-pinned CSP, Trusted
  Types, no inline event handlers, signed deploy manifests.
- Independently verify the build report's Merkle chain against
  on-disk artifacts.
- Read every commit in the relevant repos and trace each
  finding to a fix.

**Pains.**
- Most products bury security behind a "we take security
  seriously" page with no audit log. Sam needs primary sources.
- Audit tools that don't expose their methodology can't be
  independently verified.

**Forge superpowers.**
- Every build produces a signed, Merkle-chained
  `reports/build-*.json` (Forge T26, T56). Sam can prove the
  current site matches a specific historical artifact.
- The 6-layer security observability pipeline in Loom (Loom
  cycles 63-88: detect → enforce → report → COLLECT → audit →
  REVIEW) gives Sam a triage UI for live CSP violations on a
  deployed site.
- AVP-2 doctrine is published, repeatable, and machine-grepable
  via the inline-annotation standard. Sam can `grep AVP-PASS-`
  the whole codebase and read every prior pass.
- No unsafe code (`#![forbid(unsafe_code)]`). No mystery
  binary dependencies. The whole pipeline is Rust + a small set
  of vendored, hardened crates.

**Anti-features.**
- A "security score" badge generator. Scores hide the underlying
  evidence; Sam wants the evidence.
- A SaaS audit dashboard that reports back to PlausiDen. The
  build report stays on the customer's filesystem. Anything else
  is a supply-chain risk.

**Sample story.**
> "I was hired to audit a deploy of their main site. I cloned
> Forge at the version they shipped, re-ran `forge` against
> their CMS root, and got a build report whose Merkle chain
> matched theirs byte-for-byte. The signed manifest verified
> against the published key. I read every Strict finding and
> traced it back to a commit. I billed four hours instead of
> the usual sixteen and recommended Forge to the next client
> who asked."

---

## How these personas drive product decisions

Whenever a feature lands in the roadmap, ask: which persona is this
for? If it's none of P1-P5, either it's a new persona (write it down
here) or it shouldn't ship.

- A "section-level preview" (Loom T76 cycle 88) is for **Maya** —
  she wants to verify her edit looks right before saving.
- A "phase_visual_diff" (Forge T33) is for **Devon + Priya** — they
  need cross-theme/cross-viewport snapshots they can diff in CI.
- "WebAuthn passkeys" (Loom T43d) is for **Maya + Sam** — Maya
  hates passwords, Sam hates how badly most CMSes implement them.
- "Multi-tenant Loom" (T45) is for **Priya** — she wants one VPS
  with N client tenants and zero data leakage.

Conversely, before shipping anything, ask:
- Does this feature reduce a pain in P1-P5's pain list?
- Does it preserve every anti-feature on P1-P5's anti-feature list?
- If we ship this AND a competitor's parity feature, does Forge
  retain its supersociety edge (typed primitives, audit chain,
  no plugin surface, AVP-2 doctrine)?

If the answer to any of those is no, route the feature back through
design review before merging.
