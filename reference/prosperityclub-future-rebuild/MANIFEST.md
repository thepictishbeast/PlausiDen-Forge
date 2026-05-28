# prosperityclub-future-rebuild manifest

Files paul sends one at a time, for the FUTURE rebuild with a
completely new design. NOT used for the current pixel-by-pixel
reproduction (which works against the live screenshot).

## Inventory

| # | Filename                       | Received        | Description |
|---|--------------------------------|-----------------|-------------|
| 1 | 01-home.html                   | 2026-05-20      | Pre-JS prosperityclub.com home (raw WP-served HTML, before carousel script runs). Partial — `[FILE TRUNCATED AT SAVE BOUNDARY]` marker; full body in chat transcript at that timestamp. |
| 2 | (rendered post-JS home page)   | 2026-05-20      | Full DOM after WP + Cloudflare + carousel scripts ran. Preserved in chat transcript only — was too large to save inline. |

The chat transcript is the canonical archive for these files. When
the future-rebuild work begins, search the transcript for the
2026-05-20 messages tagged "prosperityclub-future-rebuild".

## Color scheme observed (for both pre-JS and post-JS files)

| Token                | Hex       | Use |
|----------------------|-----------|-----|
| Top bar bg           | `#257c4e` | Green strip with email + phone |
| Top bar text         | `#ffffff` | White on green |
| Header bg            | `#ffffff` | White |
| Nav text (desktop)   | `#733635` | Maroon |
| Nav active / hover   | `#257c4e` | Green |
| Mobile nav border    | `#ededed` | Light gray |
| Body text            | `#282828` | Near-black |
| Heading text         | `#333333` | Dark gray |
| Accent gold          | `#dba830` | Card titles, headings |
| Accent gold dark     | `#db9900` | Some borders / btn-dark |
| Accent gold border 2 | `#c28700` | Hover state of btn-dark |
| CTA primary bg       | `#733635` | Maroon CTA buttons |
| CTA primary border   | `#c1a47a` | Tan outline |
| Footer top bg        | `#eff4f1` | Mint |
| Footer bottom bg     | `#dba830` | Gold |
| Card border maroon   | `#733635` | Bordered editorial cards |
| Card border gold     | `#db9900` | Bordered editorial cards |
| Card border green    | `#257c4e` | Bordered editorial cards |

## Fonts observed

- Headings: `Lucida Grande, Lucida Sans Unicode, Lucida Sans, Geneva, Verdana, sans-serif`
- Body: `PT Sans` (Google Fonts, weights 400 / 700, with italic)
- Buttons / inputs: `Calibri, Candara, Segoe, Segoe UI, Optima, Arial, sans-serif`
- Nav: `Lucida Grande` (custom-css override)

## Layout primitives observed (need eventual Loom support)

1. Top contact bar (email left + phone right, green)
2. Image logo brand (Prosperity Club phoenix gradient logo)
3. Two-row primary nav (overflow wraps "Master Class Videos" to row 2)
4. Header social row (Facebook + LinkedIn + GTranslate language dropdown)
5. Image carousel hero (4 slides, auto-advance 5s, prev/next arrows)
6. Carousel content overlay (title + body + 2 CTA buttons)
7. Asymmetric image-text block (left image + right HTML content)
8. 3-up bordered card row with thin gold/maroon/green borders
9. Two-tone split-color heading (line 1 black, line 2 gold with text-shadow)
10. Site-origin theme widget (footer top mint bg + footer middle + footer bottom gold)
11. Page-bottom social row with FB + LinkedIn icons
12. Google Translate language picker
13. SiteOrigin Panels (.panel-grid / .panel-grid-cell layout system)

These are reference for the FUTURE rebuild design, not the current
pixel-by-pixel.
