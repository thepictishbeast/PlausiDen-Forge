//! `content_substance` — shared substance-floor tables.
//!
//! Single source of truth for the content_substance gate
//! (`forge_phases::content_substance`) and the operator-facing
//! audit CLI (`forge authoring audit`). Both consume the same
//! const tables so future floor adjustments land in one place.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"` (inherited).
//! * Const tables only; no I/O.
//!
//! ## Wire-shape alignment (2026-05-20)
//!
//! Field names below MUST match Loom's `CmsSection` struct fields
//! verbatim — `serde` deserializes by exact field name, so a table
//! entry that says `paragraph.body` when Loom's `Paragraph` struct
//! has `text: String` produces a permanent false positive (the
//! gate always sees 0 chars because the JSON key doesn't exist).
//!
//! Audit basis: `crates/PlausiDen-Loom/loom-cms-render/src/lib.rs`
//! CmsSection enum field-name verbatim cross-check. Surfaced via
//! the 11-site #222 rotation firing 50 false-positive
//! content_substance warns on `paragraph.body` + `heading.title`.

/// Per-(kind, field) minimum character count floors. Below this
/// floor, the field is treated as scaffolded-but-unauthored.
///
/// Field names are the EXACT Loom CmsSection JSON field names
/// (verified against loom-cms-render::lib.rs as of 2026-05-20).
pub const DEFAULT_MIN_CHARS: &[(&str, &str, u32)] = &[
    ("hero_editorial", "title", 20),
    ("hero_editorial", "lede", 60),
    ("hero", "title", 20),
    ("paragraph", "text", 80),      // Loom: Paragraph { text: String }
    ("pull_quote", "body", 40),     // Loom: PullQuote { body: String }
    ("code", "body", 20),           // Loom: Code { body: String }
    ("heading", "text", 8),         // Loom: Heading { text: String }
    ("call_to_action", "title", 4), // Loom: CallToAction { title: String }
    ("image_hero", "title", 8),     // Loom: ImageHero { title: String }
    ("split_hero", "title", 20),    // Loom: SplitHero { title: String }
];

/// Per-(kind, field) minimum array-length floors. Below this
/// floor, list/grid sections read as scaffolded.
pub const DEFAULT_MIN_COUNTS: &[(&str, &str, u32)] = &[
    ("kv_pair", "items", 3),
    ("feature_spotlight", "items", 3),
    ("gallery", "items", 3),
    ("logo_wall", "items", 4),
];
