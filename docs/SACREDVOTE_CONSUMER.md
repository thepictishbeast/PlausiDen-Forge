# Sacred.Vote consumer integration — Loom → Forge → consumer

Reference for how `sacred.vote` (an outside-the-monorepo consumer)
consumes the typed Loom + Forge surface for its login + lost-device
flows. Written 2026-05-20 alongside the Sacred.Vote passkey-rename
PR (commit `9465c6d` in `github.com/thepictishbeast/sacred.vote`).

This doc is descriptive — it captures the integration shape that the
consumer actually wants — not prescriptive. Forge maintainers should
push back if a shape here contradicts the substrate-only-path rule
in `PlausiDen-Loom/CLAUDE.md`.

## Scope

Sacred.Vote runs three login surfaces today:

1. **`/login` (admin)** — admin-code + 2FA, transitioning to admin
   passkey (see `project_admin_passwordless_cutover`).
2. **`/verify-identity` (voter pre-login)** — voter enters their
   per-voter opaque identifier ("voter code"), then optionally
   completes KBA / mDL / zkTLS / passkey enrollment.
3. **`/dashboard` (voter post-login)** — passkey management surface
   (list, rename, delete, reset all via recovery code).

The cleanest Loom mapping is:

| Sacred.Vote surface | Loom `CmsSection` | Methods used |
|---------------------|-------------------|--------------|
| `/login` admin code | `AuthCard` | `Password` (legacy) + `Passkey` (cutover) |
| `/login` admin passkey | `AuthCard` | `Passkey` + `WebauthnPlatform` + `WebauthnRoaming` |
| `/verify-identity` voter | `AuthCard` | `OpaqueIdentifierCode` + `Passkey` (discoverable) |
| `/verify-identity` lost device | `AuthCard` | `RecoveryCode` |
| `/dashboard` Passkeys panel | (out of Loom — uses VoterPasskeyPanel React component) | n/a |

`OpaqueIdentifierCode` + `RecoveryCode` are the two variants that
land in `loom-cms-render` via PlausiDen-Loom PR #18; the other
variants are pre-existing.

## Wire shape

A Sacred.Vote `/verify-identity` page authored via Forge's `cms/`
directory looks like this in JSON form (excerpted — `kind` tags
match the Loom enum tags):

```json
{
  "sections": [
    {
      "kind": "auth_card",
      "title": "Sign in to vote",
      "tagline": "Use your voter code, or a passkey if you have one enrolled.",
      "methods": [
        {
          "kind": "passkey",
          "label": "Sign in with a passkey"
        },
        {
          "kind": "divider",
          "label": "or"
        },
        {
          "kind": "opaque_identifier_code",
          "placeholder": "Voter code (e.g. SV-7K9F-2X3M)",
          "submit_label": "Continue",
          "helper": "Look on your registration card."
        },
        {
          "kind": "divider",
          "label": "lost your device?"
        },
        {
          "kind": "recovery_code",
          "identifier_placeholder": "Voter code",
          "recovery_placeholder": "Recovery code",
          "submit_label": "Reset access and re-enroll",
          "helper": "Burns one printed recovery code, wipes all existing passkeys for this voter, and walks you through enrolling a fresh one on this device."
        }
      ],
      "footer": null
    }
  ]
}
```

Forge's `forge build` ingests this, validates it against
`cms-schema.json` (Loom emits the schema; Forge's `loom sync
--regenerate` writes it into the consumer repo), and the rendered
output goes through every applicable Forge phase: a11y_landmarks,
contrast, semantic_html, security_headers, crawl, etc.

## Server contract per method

Sacred.Vote's Express + future axum backend exposes these endpoints
that the form posts to:

| Method | Endpoint | Payload | Success response |
|--------|----------|---------|------------------|
| `passkey` | `POST /api/voter/passkey/authenticate/start` then `/finish` | `{ clientDataJSON, ... }` | `{ voterHash, voterId }` |
| `opaque_identifier_code` | `POST /api/auth/verify-voter` | `{ code: "SV-..." }` | `{ voterHash }` |
| `recovery_code` | `POST /api/voter/recovery/redeem` then `/api/voter/passkey/register/{start,finish}` | `{ code, recoveryCode }` then standard WebAuthn enrollment | `{ voterCode, ... }` |

The Forge renderer doesn't know about these endpoints — they're the
consumer's contract. Forge just emits `<form action="...">` markup
derived from the `submit_label` + variant.

## Why not extend Forge with auth phases

Auth-specific build phases (e.g. "every AuthCard has at least one
recovery method", "every AuthCard with `Password` also has a
divider before it") were considered and deferred:

1. **Consumer policy varies**. Sacred.Vote requires a recovery
   surface; a marketing site that lets visitors sign up with magic
   link only does not. Encoding the policy in a Forge phase would
   pick one tenant's policy as canon.
2. **The audit is better placed in the consumer**. Sacred.Vote
   already has integration tests asserting `/verify-identity`
   surfaces both passkey + voter-code; that's the right layer.
3. **Forge phases that *do* belong here** include the existing
   `a11y_landmarks` + `contrast` checks — they apply uniformly
   regardless of which AuthMethodChoice variants are present.

## Downstream artifacts

When PlausiDen-Loom PR #18 merges:

1. Forge's `crates/forge-phases/Cargo.toml` git-branch pin to
   `main` automatically picks up the new variants on next
   `cargo update`.
2. `loom sync --regenerate` from inside the consumer repo (or from
   inside Forge's CI) regenerates `cms-schema.json` so consumer
   pages can declare the new `kind: "recovery_code"` + `kind:
   "opaque_identifier_code"` shapes without schema-validation
   failing.
3. Consumer pages that already author one of the variants render
   correctly the next time `forge build` runs against them.

No Forge code change is required to consume the new variants —
that's the load-bearing property of the substrate-only-path.

## See also

- `PlausiDen-Loom/loom-cms-render/src/lib.rs::AuthMethodChoice` — the canonical enum
- `PlausiDen-CMS/cms-admin-auth/src/lib.rs::AdminAuthBackend` — the credential-CRUD trait (PR #10 adds list/rename/delete)
- `sacred.vote/server/routes/webauthn.ts` — Express endpoint wiring (commit `9465c6d`)
- `sacred.vote/client/src/components/VoterPasskeyPanel.tsx` — the SPA component that maps to `OpaqueIdentifierCode` post-login
