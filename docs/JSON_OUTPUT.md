# JSON_OUTPUT.md

Canonical reference for the structured-output (`--json`) surface of every PlausiDen platform tool. Per AVP-Doctrine rule `docs-008`: every Forge / Loom / Crawler / MCP tool emits machine-readable JSON when invoked with `--json`. AI agents + scripts consume the JSON; humans read the text rendering.

> Authored to close `#150 [toolsurface-v6]`. Companion to `TOOLS.md` (human surface) and `mcp/manifest.json` (MCP surface).

---

## Discovery

```bash
forge orient --json                 # session-start brief, machine-readable
forge <subcommand> --json           # per-subcommand structured output (where supported)
make help                           # human-readable index
make mcp-list                       # MCP tool surface
```

Every JSON envelope follows the shape:

```jsonc
{
  "status": "ok" | "warn" | "fail" | "fatal" | "empty",
  // ...subcommand-specific payload...
}
```

`status` is mandatory; other fields are subcommand-specific.

---

## Coverage matrix

✓ = `--json` supported (or the subcommand emits JSON unconditionally).
◐ = partial: text only currently; JSON pending (file capability-request to add).
✗ = not applicable (the subcommand has no machine-consumable output, e.g. `watch`).

| Subcommand | --json | Schema section |
|------------|--------|----------------|
| `forge build` | ✓ (via `--json-report <path>`) | [build](#forge-build) |
| `forge watch` | ✗ | n/a (streaming) |
| `forge verify` | ◐ | TODO |
| `forge attest *` | ◐ | TODO |
| `forge audit secrets` | ◐ | TODO |
| `forge audit phantom_button` | ◐ | TODO |
| `forge audit external_assets` | ◐ | TODO |
| `forge fix` | ◐ | TODO |
| `forge manifest` | ✓ | [manifest](#forge-manifest) |
| `forge privacy` | ✓ | [privacy](#forge-privacy) |
| `forge trust-safety` | ✓ | [trust-safety](#forge-trust-safety) |
| `forge domains` | ✓ | [domains](#forge-domains) |
| `forge audit-log` | ✓ | [audit-log](#forge-audit-log) |
| `forge forms` | ✓ | [forms](#forge-forms) |
| `forge federation` | ✓ | [federation](#forge-federation) |
| `forge email` | ✓ | [email](#forge-email) |
| `forge commerce` | ✓ | [commerce](#forge-commerce) |
| `forge memberships` | ✓ | [memberships](#forge-memberships) |
| `forge config` | ✓ | [config](#forge-config) |
| `forge content validate` | ✓ | [content](#forge-content) |
| `forge content format-list` | ✓ | [content](#forge-content) |
| `forge content project-to-export` | ✓ | [content](#forge-content) |
| `forge search` | ✓ | [search](#forge-search) |
| `forge assets` | ✓ | [assets](#forge-assets) |
| `forge doctrine query` | ✓ | [doctrine](#forge-doctrine) |
| `forge doctrine check` | ✓ | [doctrine](#forge-doctrine) |
| `forge doctrine exceptions` | ✓ | [doctrine](#forge-doctrine) |
| `forge doctrine for` | ✓ | [doctrine](#forge-doctrine) |
| `forge doctrine render` | ✗ | n/a (markdown output) |
| `forge doctrine lifecycle` | ✓ | [doctrine](#forge-doctrine) |
| `forge bypasses` | ✓ | [bypasses](#forge-bypasses) |
| `forge orient` | ✓ | [orient](#forge-orient) |
| `loom validate` | ✓ | [loom](#loom-validate) |
| `loom edit serve` | ✗ | n/a (interactive) |
| `loom sync` | ◐ | TODO |
| `loom deploy hetzner` | ◐ | TODO |
| `crawler --journey ... --json` | ✓ | [crawler](#crawler-journey) |

**Summary:** **27 ✓** + **8 ◐** + **5 ✗**. The eight ◐ subcommands have follow-on capability requests; see § Gaps below.

---

## Schema sections

Each section documents the JSON shape with key fields, types, semantics, and exit-code mapping. Field names use snake_case; the canonical envelope is `{"status": ..., ...}`.

### `forge build`

Emitted to `--json-report <path>` (also written to `reports/build-<chain-id>.json` for the audit chain).

```jsonc
{
  "status": "ok" | "fail",
  "mode": "poc" | "production" | "hybrid" | "dynamic" | "static",
  "chain_length": 12,                  // integer
  "prev_hash": "<sha256>",             // hex (T26 Merkle linkage)
  "entry_hash": "<sha256>",            // hex
  "started_at": "<RFC 3339>",          // build start timestamp
  "completed_at": "<RFC 3339>",
  "findings": [
    {
      "phase": "<phase_name>",
      "severity": "strict" | "warn" | "informational",
      "code": "<finding-id>",
      "message": "...",
      "file": "<path>",
      "line": 42,                       // optional
      "enforces_rules": ["prim-007", "sec-001"]  // doctrine citation
    }
  ],
  "summary": {
    "strict_count": 0,
    "warn_count": 3,
    "info_count": 12
  }
}
```

Exit code: `0` on `status="ok"`, `1` on strict findings (production mode), `2` on fatal infrastructure failure.

### `forge orient`

```jsonc
{
  "status": "ok" | "degraded",
  "scope": "<path>",                   // resolved scope path
  "forge_root": "<path>",
  "doctrine": {
    "dir": "<path>",
    "status": "loaded" | "unavailable",
    "applicable_rules": [
      { "id": "prim-007", "name": "...", "severity": "strict", "lifecycle": "stable" }
    ]
  },
  "affordances": [
    ["AGENTS.md", true],               // [label, present-on-disk]
    ["TOOLS.md", true]
  ],
  "rule_zero": "...",                  // full substrate-only-path statement
  "canonical_defaults": "...",         // multi-line string
  "anti_patterns": "...",              // multi-line string
  "skills_for_common_tasks": [
    ["Add a Forge phase", "add-forge-phase"]
  ]
}
```

### `forge doctrine`

`query`:
```jsonc
{
  "status": "ok" | "empty" | "fatal",
  "filters": { "rule": null, "domain": "primitives", "severity": null, "lifecycle": null, "search": null },
  "matched": 12,
  "rules": [
    {
      "id": "prim-001",
      "name": "...",
      "domain": "primitives",
      "statement": "...",
      "rationale": "...",
      "enforcement": ["...", "..."],
      "applies_to": ["..."],
      "severity": "strict",
      "lifecycle": "stable",
      "related_traits": ["MobileFriendly"],
      "references": ["WCAG 2.1 §1.4.3"]
    }
  ]
}
```

`check`:
```jsonc
{
  "status": "ok" | "fail",
  "source_dir": "<path>",
  "doctrine_dir": "<path>",
  "citations_scanned": 142,
  "unresolved": [
    { "file": "crates/forge-phases/src/foo.rs", "line": 33, "rule_id": "prim-999" }
  ]
}
```

`for`:
```jsonc
{
  "status": "ok" | "empty",
  "path": "<path>",
  "needles": ["crates", "forge-phases", "..."],
  "matched": 3,
  "rules": [
    { "id": "prim-001", "name": "...", "domain": "primitives", "severity": "strict" }
  ]
}
```

`exceptions`:
```jsonc
{
  "status": "ok" | "fail",
  "source_dir": "<path>",
  "register_path": "<path>",
  "tags_scanned": 47,
  "orphans": [
    { "kind": "tag_without_register", "file": "...", "line": 12, "id": "ISSUE-12" }
  ]
}
```

`lifecycle`:
```jsonc
{
  "status": "ok",
  "totals": { "experimental": 8, "stable": 60, "deprecated": 3 },
  "experimental_rules": [...],
  "deprecated_rules": [
    { "id": "log-001", "name": "...", "deprecated_at": "<RFC 3339>", "replaced_by": "log-002" }
  ],
  "promotion_candidates": [...]
}
```

### `forge bypasses`

```jsonc
{
  "status": "ok" | "fail",
  "register_path": "<path>",
  "source_dir": "<path>",
  "register_entries": 3,
  "source_tags": 4,
  "orphans": [
    { "kind": "tag_without_register" | "register_without_tag" | "expired_deadline", "id": "...", "where": "..." }
  ]
}
```

### `forge manifest`

```jsonc
{
  "status": "ok" | "fail",
  "phases_total": 30,
  "backends_total": 12,
  "issues": [ { "kind": "...", "message": "..." } ]
}
```

### `forge privacy`

```jsonc
{
  "status": "ok" | "fail",
  "categories_covered": ["pii", "phi", "marketing"],
  "uncovered": [],
  "duplicates": [],
  "issues": []
}
```

### `forge trust-safety`

```jsonc
{
  "status": "ok" | "fail",
  "concerns_total": 5,
  "mandatory_without_scanner": [],
  "duplicates": [],
  "advisories": []
}
```

### `forge domains`

```jsonc
{
  "status": "ok" | "fail",
  "domains_total": 7,
  "wildcards_not_dns01": [],
  "hsts_not_preload_eligible": [],
  "duplicate_fqdns": []
}
```

### `forge audit-log`

```jsonc
{
  "status": "ok" | "fail",
  "path": "<path>",
  "entries": 142,
  "monotonic": true,
  "tampered_entry": null,
  "first_break": null
}
```

### `forge forms`

```jsonc
{
  "status": "ok" | "fail",
  "forms_total": 4,
  "non_https_webhooks": [],
  "unlabelled_fields": [],
  "duplicate_field_ids": [],
  "multiple_honeypots": []
}
```

### `forge federation`

```jsonc
{
  "status": "ok" | "fail",
  "destinations_total": 8,
  "protocol_address_mismatches": [],
  "duplicates": []
}
```

### `forge email`

```jsonc
{
  "status": "ok" | "fail",
  "messages_total": 12,
  "missing_required_fields": [],
  "marketing_without_unsubscribe": []
}
```

### `forge commerce`

```jsonc
{
  "status": "ok" | "fail",
  "products_total": 24,
  "issues": [
    { "product_id": "...", "variant_index": 0, "kind": "negative_price" | "empty_sku" | "non_iso_currency", "value": "..." }
  ]
}
```

### `forge memberships`

```jsonc
{
  "status": "ok" | "fail",
  "tiers_total": 4,
  "issues": []
}
```

### `forge config` (umbrella)

```jsonc
{
  "status": "ok" | "fail",
  "gates": {
    "manifest":     { "status": "ok",  "issues": 0 },
    "privacy":      { "status": "warn", "issues": 1 },
    "trust-safety": { "status": "ok",  "issues": 0 }
    // ... etc
  },
  "missing_configs": ["commerce.toml"]
}
```

### `forge content`

`validate`:
```jsonc
{
  "status": "ok" | "fail",
  "path": "<path>",
  "page_id": "...",
  "section_count": 12,
  "issues": []
}
```

`format-list`:
```jsonc
{
  "status": "ok",
  "importers": [ "html", "markdown", "json" ],
  "exporters": [ "html", "amp", "rss", "json" ]
}
```

`project-to-export`:
```jsonc
{
  "status": "ok" | "fail",
  "input_path": "<path>",
  "format": "amp",
  "output_bytes": 12345,
  "warnings": []
}
```

### `forge search`

```jsonc
{
  "status": "ok" | "fail",
  "input_path": "<path>",
  "documents_total": 142,
  "issues": []
}
```

### `forge assets`

```jsonc
{
  "status": "ok" | "fail",
  "bundle_path": "<path>",
  "ladder_complete": true,
  "missing_formats": [],
  "alt_text_issues": []
}
```

### `loom validate`

```jsonc
{
  "status": "ok" | "fail",
  "path": "<path>",
  "schema": "<path>",
  "violations": [ { "pointer": "/sections/3/kind", "message": "unknown variant" } ]
}
```

### `crawler --journey ... --json`

```jsonc
{
  "status": "ok" | "fail",
  "journey": "<path>",
  "started_at": "<RFC 3339>",
  "completed_at": "<RFC 3339>",
  "steps": [
    { "kind": "goto", "url": "...", "duration_ms": 142, "status": "ok" },
    { "kind": "screenshot", "viewport": "1280x900", "path": "runs/.../...png", "status": "ok" },
    { "kind": "detector", "axis": "contrast_runtime", "findings": [...], "status": "ok" }
  ],
  "summary": { "steps_total": 12, "failed_steps": 0, "detector_findings": 3 }
}
```

---

## Gaps (follow-on work)

Originally 8 subcommands; closed via task #200 batches:

1. **`forge verify`** — ✓ closed (chain integrity envelope; signature summary nested)
2. `forge attest sign` — n/a (there's no separate `attest sign` subcommand; signing happens automatically in `forge build` when a key exists. `forge attest init` / `pubkey` / `fingerprint` are the actual subcommands, all closed below)
3. **`forge attest init`** — ✓ closed (`{status, key_path, pub_path, key_mode, pubkey}`)
4. **`forge attest pubkey`** — ✓ closed (`{status, pub_path, pubkey}`)
5. **`forge attest fingerprint`** — ✓ closed (`{status, pub_path, fingerprint}`)
6. **`forge audit secrets`** — ✓ closed (matches[] with path + rule)
7. `forge audit phantom_button` — n/a (this is a Forge build phase, not a CLI subcommand; emits via the report JSON)
8. `forge audit external_assets` — n/a (same — Forge build phase, in-report)
9. `forge audit mutants` — pending small addition; pattern from #4 directly applicable
10. `forge fix` — pending substantial refactor (the function has many print-statement branches); pattern from #1 directly applicable
11. `loom sync` / `loom deploy hetzner` — Loom-side; out of scope here.

**5 sites closed in #200**, 3 reclassified as n/a, 2 still pending (`audit mutants` + `fix`). Refactor pattern from `forge verify` + `forge audit secrets` + the 3 attest subcommands is sufficient for the remaining ones — each is a mechanical extension.

Per `[[backward-compat-version-discipline]]`: adding `--json` is a Cat 2 additive change. New consumers may pass `--json`; legacy callers continue to get text output (default `--json false`).

---

## Cross-AI consumption

The MCP tool definitions in `mcp/tools/*.json` declare `"json": { "default": true }` for every wrapped command. Agents (Claude / Gemini / other MCP-capable clients) consume the JSON envelopes documented here directly.

The schemas above are JSON-Schema-friendly; a future task projects them into a single `schemas/forge-output-v1.json` for client-side validation.

---

## Stability + versioning

Per `[[backward-compat-version-discipline]]`:

- **Additive change** (new field): non-breaking. Bump `output_schema_version` minor.
- **Field rename**: requires auto-migration; bump major.
- **Field removal**: deprecation lifecycle (one minor as `deprecated=true`, then removal in next major).
- **Status enum extension**: additive when new statuses preserve existing semantics; otherwise major.

Consumers should parse defensively (`status` known + payload may have unknown fields).

---

## See also

- `TOOLS.md` — human-readable command index.
- `AGENTS.md` — orientation, including JSON-output discipline.
- `mcp/manifest.json` — MCP tool surface; every entry references this doc's schema.
- AVP-Doctrine rule `docs-008` — JSON output enforcement.
- `[[backward-compat-version-discipline]]` — versioning policy.
