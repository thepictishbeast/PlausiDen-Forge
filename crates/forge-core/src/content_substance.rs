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

/// Per-(kind, field) minimum character count floors. Below this
/// floor, the field is treated as scaffolded-but-unauthored.
pub const DEFAULT_MIN_CHARS: &[(&str, &str, u32)] = &[
    ("hero_editorial", "title", 20),
    ("hero_editorial", "lede", 60),
    ("hero", "title", 20),
    ("paragraph", "body", 80),
    ("pull_quote", "body", 40),
    ("code", "body", 20),
    ("code_block", "body", 20),
    ("heading", "title", 8),
    ("sub_heading", "title", 6),
    ("section_heading", "title", 6),
    ("call_to_action", "label", 4),
    ("image_hero", "title", 8),
    ("split_hero", "title", 20),
];

/// Per-(kind, field) minimum array-length floors. Below this
/// floor, list/grid sections read as scaffolded.
pub const DEFAULT_MIN_COUNTS: &[(&str, &str, u32)] = &[
    ("kv_pair", "items", 3),
    ("feature_spotlight", "items", 3),
    ("gallery", "items", 3),
    ("logo_wall", "items", 4),
];
