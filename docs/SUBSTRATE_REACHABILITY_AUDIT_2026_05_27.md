# Substrate primitive reachability audit — 2026-05-27

Per task #355. Surveys every CmsSection variant (163) + CmsBlock
variant (62) for "reach" — how often the primitive's slug appears
in observed content (fixtures, tenant CMS files, Rust test
literals). Identifies the long tail that's been built but isn't
being reached for.

## Methodology

Sources scanned:
- `PlausiDen-Forge/fixtures/forge-lite/*.json` (5 lite fixtures)
- `ProsperityClub/cms/*.json` (mom's site content)
- `PlausiDen-Loom/loom-cms-render/src/lib.rs` (snapshot tests +
  test literals)
- `PlausiDen-Forge/crates/**/*.rs` (audit phase fixtures, test
  literals)

Total: 197 source files. Counted `"kind": "<slug>"` references
across all of them.

## CmsSection reach distribution

**Total variants:** 163
**Variants with non-zero reach:** 58 (35.6%)
**Variants never reached:** 105 (64.4%)

### Top 20 most-reached variants

| Reach | Slug |
|------:|:-----|
| 200 | `paragraph` |
| 72 | `heading` |
| 36 | `hero_editorial` |
| 32 | `kv_pair` |
| 30 | `feature_spotlight` |
| 22 | `image_hero` |
| 19 | `hero` |
| 16 | `call_to_action` |
| 14 | `pull_quote` |
| 13 | `disclaimer` |
| 11 | `source_list` |
| 10 | `pricing` |
| 7 | `form` |
| 6 | `testimonial` |
| 5 | `marquee` |
| 5 | `stat_band` |
| 4 | `code` |
| 4 | `divider` |
| 4 | `loom_fact` |
| 4 | `spacer` |

### Never-reached (count = 0): 105 variants

```
accordion_group, account_summary, add_to_cart, alert, anchor_list,
aside_note, audio_embed, auth_card, auth_flow_stepper, avatar,
avatar_stack, award_badges, back_to_top, badge_grid, banner,
before_after, caption, card_feed, cart_drawer, case_study,
chat_bubble, chat_thread, citation, cluster, columns,
comment_thread, comparison, contact_strip, crucible_widget,
def_list, diagram, drawer, drop_cap, empty_state, faq,
feed_post, figure, figure_group, follow_button, footnote,
form_color, form_date, form_file, form_input, form_search,
form_select, form_slider, form_submit, form_textarea,
form_toggle, game_grid, game_tile, glossary, grid_layout,
group, hashtag_inline, hero_minimal, hero_split, icon_row,
image_grid, lang_switch, legal_doc, lightbox, marginalia,
math_block, mega_menu, mention_inline, mfa_prompt, modal,
mosaic_grid, nav_tabs, password_reset, picture, price_tag,
product_card, product_gallery, product_grid, product_spec,
profile_card, profile_edit, promo_strip, pull_stat,
reaction_row, reveal, review_card, review_stars, roadmap,
settings_panel, share_row, sidebar, signed_in_card, skeleton,
slideshow, stack, steps, sub_heading, thread_list, thread_row,
timeline, toc_block, vertical_nav, video_card, video_embed,
video_grid_section, wishlist
```

## CmsBlock reach distribution

**Total variants:** 62
**Variants with non-zero reach:** 42 (67.7%)
**Variants never reached:** 20 (32.3%)

### Top 20 most-reached variants

| Reach | Slug |
|------:|:-----|
| 72 | `heading` |
| 15 | `text` |
| 7 | `form` |
| 7 | `image` |
| 5 | `marquee` |
| 4 | `code` |
| 4 | `divider` |
| 4 | `slider` |
| 4 | `spacer` |
| 4 | `switch` |
| 4 | `toast` |
| 4 | `tooltip` |
| 3 | `accordion` |
| 3 | `dialog` |
| 3 | `navigation_menu` |
| 3 | `number_field` |
| 3 | `sheet` |
| 2 | `aspect_ratio` |
| 2 | `breadcrumb` |
| 2 | `combobox` |

### Never-reached (count = 0): 20 variants

```
alert, avatar, badge, card, carousel, column, definition_list,
empty_state, figure, grid, iframe, kbd_shortcut, link, list,
row, stat, stepper, table, timeline, video
```

## Analysis

### Heavy concentration in tiny long head

`paragraph` (200) + `heading` (72) account for **47%** of all CmsSection
reach. The top 10 variants (paragraph, heading, hero_editorial,
kv_pair, feature_spotlight, image_hero, hero, call_to_action,
pull_quote, disclaimer) account for **84%** of total reach.

The remaining 153 variants share 16% of observed reach. This is
the classic long-tail distribution: a few primitives do most of
the work; most primitives are rarely used.

### Three categories within the never-reached tail

Walking the 105 never-reached CmsSection variants:

**Category A — recently shipped, no consumer yet (~25 variants).**
Primitives added during specific design pushes that haven't
landed in production content yet. Examples: `crucible_widget`,
`auth_flow_stepper`, `mfa_prompt`, `signed_in_card`,
`auth_card`, `password_reset`. These are intentional — they
serve future tenant content that hasn't shipped.

**Category B — duplicate or near-duplicate of reached variants (~30 variants).**
The substrate has several primitives that solve the same problem
slightly differently. Examples:
- `picture` / `image_grid` / `image_hero` / `mosaic_grid` /
  `slideshow` — all images, different shapes; only `image_hero`
  reached
- `hero_minimal` / `hero_split` / `hero_editorial` / `hero` —
  Hero family; `hero_editorial` (36), `image_hero` (22), `hero`
  (19) reached, but `hero_minimal` and `hero_split` never
- `card_feed` / `feed_post` / `review_card` / `case_study` /
  `product_card` / `profile_card` — many card shapes; `kv_pair`
  is reached as a structured-content alternative; the card
  variants are not

These represent the audit's central finding from the
2026-05-21 reframe: substrate has the vocabulary, callers
default to the familiar handful.

**Category C — legitimately niche (~50 variants).**
Real-but-narrow use cases: `legal_doc`, `award_badges`,
`game_grid`, `game_tile`, `wishlist`, `cart_drawer`,
`add_to_cart`, `product_spec`, etc. These serve commerce /
gaming / specific verticals. Their never-reached state means
no tenant in those verticals has shipped yet, not that the
primitive is wrong.

## Actionable recommendations

### Recommendation 1: Surface the long tail via doc-query (#386 + #398)

The 105 never-reached CmsSection variants + 20 CmsBlock
variants need to be discoverable. Currently the substrate's
`doc_query` index ships only 10 hand-curated entries; expand
it so operators can find these primitives:

- Add a `doc_query` entry per never-reached variant in Category
  C (niche-but-valid). At minimum: slug + one-line description
  + example minimal use.
- Add cross-references in Category A doc entries
  ("see also: auth_card, signed_in_card, mfa_prompt") so
  related primitives surface together.

This converts "shipped but invisible" into "shipped + discoverable."

### Recommendation 2: Identify deprecation candidates in Category B

The substrate's existing variation-arc phases (`primitive_exhaustion`,
`differentiation_budget`) measure usage but don't surface specific
deprecation candidates. Walk Category B (~30 near-duplicate
primitives); for each, decide:

- (a) Keep both because they're substantively different — extend
  docs to explain when to use each
- (b) Mark one as preferred; the other becomes deprecated
- (c) Merge into a single primitive with property-composition
  (the Hero family pilot pattern, shipped #387)

This is design-led work. The audit produces the candidate list;
the design decisions per candidate are separate iterations.

### Recommendation 3: Add a substrate-reachability audit phase

Currently no audit phase measures whether a tenant's content
is using only the well-trafficked head, ignoring the long tail.
Add a phase: `primitive_reachability_breadth` — for each tenant,
report what fraction of distinct primitives it uses out of
total available. Tenants that use only 5/163 get a finding
suggesting alternatives. (Soft Warn finding, not Strict.)

### Recommendation 4: Test-fixture coverage push

The substrate's snapshot tests in `loom-cms-render` exercise
the top ~50 variants. The 105 never-reached should at minimum
have a snapshot test each so they're proven to render.
Currently many are typed but untested at the render layer.

## Mapping to other audit tasks

This audit informs the rest of the #355-#360 chain:

- **#356** (decorative coverage) — Category B includes many
  decorative shapes; the audit overlap matters
- **#357** (compositional coverage) — `columns`, `grid_layout`,
  `cluster`, `stack` are all never-reached layout primitives;
  this is where the compositional gap shows up
- **#358** (theme system growth) — orthogonal to this audit;
  theme work doesn't depend on primitive reach
- **#359** (content-model BlockKind coverage) — the 20
  never-reached blocks (alert, avatar, badge, card, carousel,
  table, timeline, etc.) ARE the gaps that #359 should address
- **#360** (neutralize defaults) — the heavy concentration in
  `paragraph` + `heading` reveals that defaults pull strongly
  toward the head; neutralizing requires actively surfacing
  the long tail

## Honest scope note

This audit measures REACH (how often each primitive is referenced
in content) not USAGE QUALITY. A primitive could be reached
many times but used wrong; another could be reached rarely but
used well. Reach is a proxy; the deeper question is whether
the right primitive is being chosen for each context.

That deeper question requires per-tenant per-page review, which
is out of scope here. This audit produces the head + tail
inventory; per-tenant content review happens in actual site
builds, not in substrate-level audits.

## Substrate-roadmap implication

The reframe's central claim — "substrate has the vocabulary but
operators can't reach for it" — is empirically supported by
this audit. 64% of CmsSection variants never reached. The
remediation isn't "ship more primitives"; it's "make the
existing ones reachable" (doc-query surfacing + targeted
deprecation of near-duplicates + reach-breadth audit phase).
