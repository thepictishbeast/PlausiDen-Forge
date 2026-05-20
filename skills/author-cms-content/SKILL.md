---
name: author-cms-content
description: Author cms/*.json content for a new site or new page. Covers the typed CmsPage shape, sections, nav, footer, backend declarations, and the strict-build validation cycle.
metadata:
  tags: [cms, forge, site-build]
  related_doctrine_rules: [build-001, prim-006, sec-001, content-001, content-002, content-003]
  related_traits: []
---

# Author CMS content for a site

Use this skill when standing up a new site OR adding a new page to an existing site. CMS content is **typed TOML/JSON** that drives Forge's render phase — NOT a place to embed HTML or CSS.

## When to invoke

Recognition signals:
- A new route needs to exist on `dev.plausiden.com` or a customer site.
- An existing site needs a page added (e.g. an article, landing, or static doc).
- The operator wants to change content on a page.

Anti-signals:
- The needed shape doesn't exist in any primitive — that's a Loom primitive request (see `add-loom-primitive` skill), not a CMS authoring task.
- The needed change is to a primitive's appearance — that's a Loom variant or theme update, not CMS work.

## Prerequisites

1. Read `cms-schema.json` (the live schema export — it's the source of truth for valid section variants).
2. Read existing `cms/*.json` files in the target site for shape examples.
3. Check `backends.toml` for the data-backend slugs the site declares.
4. Run `forge doctrine for cms` to see applicable content rules.

## Procedure

### 1. Set up the file

Page CMS files live at `cms/<slug>.json`. Always include the schema reference:

```json
{
  "$schema": "../cms-schema.json",
  "title": "Page title — Site Name",
  "description": "One-paragraph meta description for SEO.",
  "brand": "Site Name",
  "chrome": "page_shell",
  "theme": "light",
  ...
}
```

- `chrome` is one of `page_shell` / `floating_pill` / `minimal` — typed enum.
- `theme` matches a declared loom-tokens theme.
- `path` is the deployed URL path (e.g. `/about/`).

### 2. Declare nav + footer

```json
"nav_links": [
  {"label": "About",    "href": "/about/",    "data_backend": "view-about"},
  {"label": "Services", "href": "/services/", "data_backend": "view-services"}
],
"nav_actions": [
  {"label": "Sign in", "href": "/login/", "data_backend": "cta-login"}
],
"footer": {
  "columns": [
    {"heading": "Company", "links": [...]}
  ],
  "contact": {...},
  "legal_links": [...],
  "colophon": "© 2026 ..."
}
```

Every `data_backend` slug MUST exist in `backends.toml` (rule sec-007 + phantom_button phase). Add missing slugs there in the same commit.

### 3. Compose the page from sections

The `sections` array is the page body. Each entry's `kind` matches a CmsSection variant:

```json
"sections": [
  {"kind": "image_hero", "align": "start", "eyebrow": "...", "title": "...", "lede": "...", "background": {...}},
  {"kind": "heading", "level": 2, "text": "..."},
  {"kind": "paragraph", "text": "..."},
  {"kind": "pull_quote", "body": "...", "attribution": "..."},
  {"kind": "feature_spotlight", "items": [...]},
  {"kind": "call_to_action", "title": "...", "cta": {...}}
]
```

Follow the section-by-section shape from the schema. **Do not invent fields**; `serde(deny_unknown_fields)` rejects them at parse time (rule sec-001).

### 4. Default alignment: start

Per rule prim-009: image_hero / heading / pricing / cta_band etc. default to **start** alignment. Override to `"center"` only when there's an editorial reason. Centered-everything is the SaaS-marketing default that produces indistinguishable sites.

### 5. Anchor ids on jump-link targets

If `nav_links` has `href="/page#section-id"`, the corresponding heading needs `"id": "section-id"` (loom-render's `link_check` phase catches missing ids).

### 6. Content discipline

Per the content domain rules:
- **Claims have sources** (rule content-002): statistics, regulatory references, technical assertions need citation.
- **Testimonials are structured** (rule content-003): `name`, `role`, `organization`, optional `photo` with WCAG-compliant `alt`. Anonymous requires explicit flag + rationale. Fictional testimonials are forbidden.
- **Reading-level + decision-density** (rule a11y-005): pages declare reading-level target via content type; substrate flags drift.

### 7. Validate

```bash
loom validate cms/<your-slug>.json  # typed-schema validation
forge build                         # full pipeline; strict findings must == 0 in production mode
```

Strict findings to address:
- `phantom_button` — every `data_backend` must exist in `backends.toml`
- `label_consistency` — same href can't have two different labels without `data-loom-poly-action="true"`
- `tokens` — no raw px/hex/rgb in content (use loom-token references for any inline styles)
- `link_check` — anchor jumps need matching `id` attributes
- `substrate_purity` — your CMS file is fine; this phase checks for hand-coded assets in `static/`

### 8. Deploy

```bash
forge build --mode production       # strict-clean
rsync -a --delete static/ /var/www/<your-site>/
chown -R caddy:caddy /var/www/<your-site>/
```

(Or `loom deploy hetzner` when wired.)

## Common pitfalls

| ❌ Don't | ✅ Do |
|---------|------|
| Center-align hero by default | `"align": "start"` (rule prim-009) |
| Embed inline `<style>` or `style="..."` in content | Compose primitives; new variant via Loom (rule prim-006) |
| Make up a section kind not in the schema | `serde(deny_unknown_fields)` rejects; check `cms-schema.json` |
| Reference an undeclared `data_backend` | Add to `backends.toml` same commit (sec-007) |
| Use generic testimonials like "Sarah K., happy customer" | Full structured attribution: name + role + organization (rule content-003) |
| Cite a statistic without source | Add citation field (rule content-002) |
| Hardcode raw px/hex in content fields | Tokens only via theme references (rule prim-007) |
| Hand-author `static/foo.html` | Site content is CMS only; the substrate emits HTML (rule build-007 + substrate_purity phase) |

## Acceptance criteria

- [ ] `cms/<slug>.json` parses clean against `cms-schema.json`
- [ ] Every `data_backend` slug exists in `backends.toml`
- [ ] Every anchor jump-target has a corresponding `"id"` field
- [ ] `forge build` strict findings == 0 (production mode)
- [ ] No raw HTML / CSS / JS authored anywhere
- [ ] Default alignment is start unless editorially justified
- [ ] Claims sourced; testimonials structured

## Cross-references

- Schema: `cms-schema.json`
- Existing CMS examples: `cms/*.json`, `cms-northbrook-backup/*.json`
- Backend declarations: `backends.toml`
- Loom render: `PlausiDen-Loom/loom-cms-render/src/lib.rs`
- Content rules: `forge doctrine query --domain content`
- Primitive rules: `forge doctrine query --domain primitives`
