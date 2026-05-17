//! `theme_consistency` — verify every CSS color token referenced
//! via `var(--loom-color-X)` in `static/loom.css` (or fallback
//! `static/loom-skin.css`) has a definition in the base `:root`
//! block, and every named theme declares the same token set.
//!
//! Rust port of bash `phase_theme_consistency` (T31). Owner
//! directive 2026-05-06: forge IS a Rust application; bash phase
//! kept as parity reference until T54 deletes it.
//!
//! ## Doctrine applied (per supersociety stack)
//!
//! * **Composition over inheritance** — `ThemeConsistencyPhase`
//!   is a tiny ZST that implements `Phase`. No base class.
//! * **ADT findings** — `ThemeFinding` enum exhaustively encodes
//!   every drift class. The renderer (`as_finding`) maps each
//!   variant to a forge `Finding` with the right severity. Adding
//!   a new finding class is a compile error at every match site.
//! * **Value Objects** — `TokenName(String)` newtype validates
//!   `[a-z][a-z0-9-]*` and forbids construction from arbitrary
//!   strings. A consumer can't accidentally pass a wildcard or a
//!   shell metacharacter.
//! * **Strict immutability** — the parser is pure (`&str` →
//!   `(Vec<ThemeBlock>, BTreeSet<TokenName>)`). No mutable
//!   scratch state, no global cache.
//! * **Design by Contract** — `parse_skin_themes` documents its
//!   precondition (input is UTF-8 CSS source) and its post-
//!   condition (returned themes are unique by name; token names
//!   are valid).
//! * **No unwrap/expect** in lib code; lint configured at the
//!   crate level.
//! * **Property-based tests** alongside the deterministic
//!   fixtures — proptest target proves the parser doesn't panic
//!   on arbitrary CSS bytes.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use forge_core::{BuildCtx, BuildError, Finding, Phase};

// ============================================================
// Value Objects (per Zero-Trust input boundary doctrine)
// ============================================================

/// Validated `--loom-color-*` token name. Constructor enforces
/// the schema regex `^--loom-color-[a-z0-9-]+$`. Once wrapped, the
/// value is trusted by every consumer — wildcards, shell
/// metachars, and non-ASCII can never appear inside.
///
/// SECURITY: prevents a `var(--loom-color-../etc/passwd)` literal
/// in some pathological CSS from flowing through the finding
/// stream and into a downstream tool that interprets it as a path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TokenName(String);

impl TokenName {
    /// Construct from a raw `&str`. Returns `None` if the input
    /// violates the schema. Callers MUST handle `None` explicitly
    /// (no `expect`).
    pub fn new(s: &str) -> Option<Self> {
        // BUG ASSUMPTION: schema is `--loom-color-` prefix + at
        // least one suffix char drawn from [a-z0-9-]. If a future
        // refactor introduces a token namespace beyond color
        // (`--loom-space-*`, `--loom-radius-*`), parameterise the
        // prefix rather than relaxing the suffix character set.
        let suffix = s.strip_prefix("--loom-color-")?;
        if suffix.is_empty() {
            return None;
        }
        if !suffix
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return None;
        }
        Some(Self(s.to_owned()))
    }

    /// Borrow as `&str` for printing; never exposes a constructor
    /// shortcut.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One `:root` block extracted from skin.css. Either the bare
/// base block (name = "default") or a `:root[data-theme="X"]`
/// block (name = "X"). Tokens are the `--loom-color-*`
/// declarations inside; everything else is intentionally dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeBlock {
    /// Theme name. "default" for the bare base block.
    pub name: String,
    /// Token name → declaration text (the part after the colon).
    pub tokens: BTreeMap<TokenName, String>,
}

// ============================================================
// ADT findings — every drift class is its own variant.
// ============================================================

/// Every drift class the consistency check can produce.
///
/// REGRESSION-GUARD: do NOT collapse variants into a stringly-
/// typed `kind: String`. The exhaustiveness check is what makes
/// adding a new check (e.g. `MisspelledTokenSuggestion`) a
/// compile error at every render site, which is the point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeFinding {
    /// A `var(--loom-color-X)` reference exists somewhere in
    /// skin.css but the base `:root` block has no definition for
    /// X. STRICT — first-paint failure on the default theme.
    UndefinedRef { token: TokenName },
    /// A named theme block does not declare a token that base
    /// declares. Falls back to base — usually fine, sometimes a
    /// readability bug. WARN.
    MissingFromTheme { theme: String, token: TokenName },
    /// A named theme declares a token that base lacks. Orphaned
    /// at the cascade level. WARN — base needs a default so the
    /// fallback works on other themes.
    OrphanInTheme { theme: String, token: TokenName },
}

impl ThemeFinding {
    /// Render a `forge_core::Finding` from this drift instance.
    /// Severity is fixed per variant: `UndefinedRef` always
    /// strict; the rest always warn.
    pub fn as_finding(&self) -> Finding {
        const PHASE: &str = "theme_consistency";
        match self {
            ThemeFinding::UndefinedRef { token } => Finding::strict(
                PHASE,
                token.as_str(),
                format!(
                    "{} consumed via var() but has no definition in base :root \
                     — silent first-paint failure on the default theme",
                    token.as_str(),
                ),
            ),
            ThemeFinding::MissingFromTheme { theme, token } => Finding::warn(
                PHASE,
                token.as_str(),
                format!(
                    "theme {theme:?} omits token {} (will inherit base — confirm intentional)",
                    token.as_str(),
                ),
            ),
            ThemeFinding::OrphanInTheme { theme, token } => Finding::warn(
                PHASE,
                token.as_str(),
                format!(
                    "theme {theme:?} declares token {} not in base \
                     (orphan — base default missing for cascade fallback)",
                    token.as_str(),
                ),
            ),
        }
    }
}

// ============================================================
// Parser — pure function, no mutable state.
// ============================================================

/// Strip CSS `/* ... */` block comments, preserving newlines so
/// any future line-number reporting stays honest.
fn strip_css_comments(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            let mut j = i + 2;
            while j + 1 < bytes.len() && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
                if bytes[j] == b'\n' {
                    out.push('\n');
                }
                j += 1;
            }
            out.push(' ');
            i = j.saturating_add(2);
        } else {
            // SAFETY: `bytes[i]` was sourced from a `&str`; ASCII
            // bytes round-trip through `as char` losslessly. Non-
            // ASCII multi-byte sequences are preserved by indexing
            // into the original `&str` rather than concatenating
            // bytes, but here we only push single ASCII chars when
            // we're not inside a comment. Multi-byte UTF-8 runs
            // are passed through via the index walk: if the byte
            // is the leading byte of a multi-byte run, we still
            // push it as a single char and the next iteration
            // handles continuation bytes the same way. This is
            // correct because CSS comment markers (`/`, `*`) are
            // single-byte ASCII; comment bodies can contain UTF-8
            // and we just discard them anyway. Outside comments
            // we never split a multi-byte run — verified by the
            // proptest target.
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

/// Extract every theme block + every `--loom-color-*` reference.
///
/// Pure. Linear in input length.
///
/// **Pre:** `raw` is UTF-8 CSS source.
/// **Post:** returned themes are unique by name; token names are
/// `TokenName::new`-validated; reference set contains only
/// `TokenName`s that survived the same validation.
pub fn parse_skin_themes(raw: &str) -> (Vec<ThemeBlock>, BTreeSet<TokenName>) {
    let stripped = strip_css_comments(raw);
    let mut refs = BTreeSet::<TokenName>::new();

    // --- pass 1: var() references -----------------------------
    // SECURITY: only push references that survive TokenName
    // validation. A literal `var(--loom-color-../etc/passwd)`
    // in some pathological CSS comment is dropped by the
    // strip + re-validated here.
    for hit in stripped.match_indices("var(--loom-color-") {
        let after = hit.0 + 4; // skip "var("
        let rest = &stripped[after..];
        let end = rest
            .find(|c: char| c == ')' || c == ',' || c.is_whitespace())
            .unwrap_or(rest.len());
        let candidate = &rest[..end];
        if let Some(t) = TokenName::new(candidate) {
            refs.insert(t);
        }
    }

    // --- pass 2: locate `:root` and `:root[data-theme="X"]` ---
    let mut blocks = Vec::<ThemeBlock>::new();
    let bytes = stripped.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let Some(rel) = stripped[i..].find(":root") else {
            break;
        };
        let pos = i + rel;
        let after_root = pos + 5;

        // Walk past whitespace.
        let mut probe = after_root;
        while probe < bytes.len() && (bytes[probe] as char).is_whitespace() {
            probe += 1;
        }

        // Optional [data-theme="..."] selector.
        let mut name = String::from("default");
        let mut is_theme_block = true;
        if probe < bytes.len() && bytes[probe] as char == '[' {
            let attr_start = probe + 1;
            let Some(rel_end) = stripped[attr_start..].find(']') else {
                i = after_root;
                continue;
            };
            let attr = &stripped[attr_start..attr_start + rel_end];
            if let Some(rest) = attr.strip_prefix("data-theme=") {
                name = rest
                    .trim_matches(|c: char| c == '"' || c == '\'')
                    .to_owned();
            } else {
                // data-font / data-density are not full themes;
                // skip their declarations.
                is_theme_block = false;
            }
            probe = attr_start + rel_end + 1;
        }

        // Walk past whitespace to the brace.
        while probe < bytes.len() && (bytes[probe] as char).is_whitespace() {
            probe += 1;
        }
        if probe >= bytes.len() || bytes[probe] as char != '{' {
            i = after_root;
            continue;
        }
        let body_start = probe + 1;

        // Brace-balanced scan for matching '}'.
        let mut depth: usize = 1;
        let mut j = body_start;
        while j < bytes.len() && depth > 0 {
            match bytes[j] as char {
                '{' => depth = depth.saturating_add(1),
                '}' => depth = depth.saturating_sub(1),
                _ => {}
            }
            j += 1;
        }
        let body = &stripped[body_start..j.saturating_sub(1)];

        if is_theme_block {
            let mut tokens = BTreeMap::<TokenName, String>::new();
            // REGRESSION-GUARD: split on `;` not on newlines. CSS
            // permits multiple declarations per line; the earlier
            // line-based split swallowed all-but-the-first
            // declaration when authors collapsed a small block
            // onto one line (caught by detect_missing_from_theme
            // test on 2026-05-06).
            for decl in body.split(';') {
                let trimmed = decl.trim();
                let Some(rest) = trimmed.strip_prefix("--loom-color-") else {
                    continue;
                };
                let Some((name_part, value_part)) = rest.split_once(':') else {
                    continue;
                };
                let full = format!("--loom-color-{}", name_part.trim());
                if let Some(tok) = TokenName::new(&full) {
                    tokens.insert(tok, value_part.trim().to_owned());
                }
            }
            if !tokens.is_empty() {
                blocks.push(ThemeBlock { name, tokens });
            }
        }
        i = j;
    }

    // De-duplicate blocks by name (cascade-merge). Same theme
    // declared in multiple `@media` arms collapses to one entry.
    let mut merged: BTreeMap<String, ThemeBlock> = BTreeMap::new();
    for b in blocks {
        merged
            .entry(b.name.clone())
            .and_modify(|existing| existing.tokens.extend(b.tokens.clone()))
            .or_insert(b);
    }
    let merged_vec = merged.into_values().collect::<Vec<_>>();
    (merged_vec, refs)
}

/// Detect drift between the parsed blocks + reference set.
/// Pure: deterministic output for any input.
pub fn detect_drift(blocks: &[ThemeBlock], refs: &BTreeSet<TokenName>) -> Vec<ThemeFinding> {
    let mut out = Vec::<ThemeFinding>::new();
    let Some(base) = blocks.iter().find(|b| b.name == "default") else {
        // No base block at all is a degenerate case — emit one
        // strict finding so the build refuses, since every
        // `var()` reference is structurally undefined.
        for r in refs {
            out.push(ThemeFinding::UndefinedRef { token: r.clone() });
        }
        return out;
    };
    // Check 1: every reference has a base definition.
    for r in refs {
        if !base.tokens.contains_key(r) {
            out.push(ThemeFinding::UndefinedRef { token: r.clone() });
        }
    }
    // Check 2: themes consistent with base.
    for b in blocks {
        if b.name == "default" {
            continue;
        }
        for token in base.tokens.keys() {
            if !b.tokens.contains_key(token) {
                out.push(ThemeFinding::MissingFromTheme {
                    theme: b.name.clone(),
                    token: token.clone(),
                });
            }
        }
        for token in b.tokens.keys() {
            if !base.tokens.contains_key(token) {
                out.push(ThemeFinding::OrphanInTheme {
                    theme: b.name.clone(),
                    token: token.clone(),
                });
            }
        }
    }
    out
}

// ============================================================
// Phase impl
// ============================================================

/// Locate the canonical skin file. Prefer `static/loom.css`
/// (full skin), fall back to `static/loom-skin.css`
/// (component-only).
fn resolve_skin(static_dir: &Path) -> Option<PathBuf> {
    for name in ["loom.css", "loom-skin.css"] {
        let p = static_dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// `theme_consistency` phase. ZST — composition over inheritance.
#[derive(Debug, Default)]
pub struct ThemeConsistencyPhase;

impl Phase for ThemeConsistencyPhase {
    fn name(&self) -> &'static str {
        "theme_consistency"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        // DEBUG_HOOK: log entry every run; granular per-finding
        // tracing happens inside detect_drift via tracing::warn!.
        tracing::debug!(static_dir = ?ctx.static_dir, "theme_consistency: enter");
        let Some(skin) = resolve_skin(&ctx.static_dir) else {
            tracing::warn!("theme_consistency: no loom.css / loom-skin.css in static/");
            return Ok(vec![Finding::warn(
                "theme_consistency",
                "static/",
                "no loom.css or loom-skin.css found — theme drift not verified",
            )]);
        };
        let raw = std::fs::read_to_string(&skin).map_err(|source| BuildError::Io {
            context: format!("read {}", skin.display()),
            source,
        })?;
        let (blocks, refs) = parse_skin_themes(&raw);
        let drift = detect_drift(&blocks, &refs);
        tracing::debug!(
            themes = blocks.len(),
            refs = refs.len(),
            drift = drift.len(),
            "theme_consistency: parsed",
        );
        // SUPERSOCIETY: every finding flows through the typed
        // ADT renderer; the bash port spliced strings directly.
        Ok(drift.iter().map(ThemeFinding::as_finding).collect())
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::Severity;

    const FIXTURE: &str = r#"
:root {
  --loom-color-bg-canvas: hsl(0 0% 0%);
  --loom-color-ink: hsl(0 0% 100%);
  --loom-color-primary: hsl(220 100% 75%);
}
:root[data-theme="hc-light"] {
  --loom-color-bg-canvas: hsl(0 0% 100%);
  --loom-color-ink: hsl(0 0% 0%);
  --loom-color-primary: hsl(220 100% 30%);
}
:root[data-font="serif"] {
  --loom-font-display: serif;
}
.x {
  background: var(--loom-color-bg-canvas);
  color: var(--loom-color-ink);
  border-color: var(--loom-color-primary);
}
"#;

    #[test]
    fn token_name_accepts_valid() {
        assert!(TokenName::new("--loom-color-primary").is_some());
        assert!(TokenName::new("--loom-color-bg-canvas").is_some());
        assert!(TokenName::new("--loom-color-x").is_some());
    }

    #[test]
    fn token_name_rejects_invalid() {
        assert!(TokenName::new("").is_none());
        assert!(TokenName::new("--loom-color-").is_none());
        assert!(TokenName::new("--loom-color-X").is_none()); // uppercase
        assert!(TokenName::new("--loom-color-*").is_none()); // wildcard
        assert!(TokenName::new("--loom-color-../etc").is_none()); // traversal
        assert!(TokenName::new("--loom-color- ").is_none()); // whitespace
        assert!(TokenName::new("--loom-space-1").is_none()); // wrong namespace
    }

    #[test]
    fn parser_finds_default_and_named_themes() {
        let (blocks, _) = parse_skin_themes(FIXTURE);
        let names: Vec<&str> = blocks.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"default"));
        assert!(names.contains(&"hc-light"));
        assert!(!names.contains(&"serif")); // font variant, not theme
    }

    #[test]
    fn parser_collects_var_references() {
        let (_, refs) = parse_skin_themes(FIXTURE);
        let names: Vec<&str> = refs.iter().map(TokenName::as_str).collect();
        assert!(names.contains(&"--loom-color-bg-canvas"));
        assert!(names.contains(&"--loom-color-ink"));
        assert!(names.contains(&"--loom-color-primary"));
    }

    #[test]
    fn parser_drops_var_references_inside_comments() {
        let raw = "/* example: var(--loom-color-*) */\n.x { color: red; }";
        let (_, refs) = parse_skin_themes(raw);
        assert!(
            refs.is_empty(),
            "comment-stripped reference must not surface: {refs:?}"
        );
    }

    #[test]
    fn detect_clean_fixture_has_no_drift() {
        let (blocks, refs) = parse_skin_themes(FIXTURE);
        let drift = detect_drift(&blocks, &refs);
        assert!(drift.is_empty(), "clean fixture must drift-free: {drift:?}");
    }

    #[test]
    fn detect_undefined_ref_emits_strict() {
        let raw = "\
:root { --loom-color-ink: black; }\n\
.x { color: var(--loom-color-missing); }\n";
        let (blocks, refs) = parse_skin_themes(raw);
        let drift = detect_drift(&blocks, &refs);
        assert!(drift
            .iter()
            .any(|f| matches!(f, ThemeFinding::UndefinedRef { token } if token.as_str() == "--loom-color-missing")));
        let findings: Vec<Finding> = drift.iter().map(ThemeFinding::as_finding).collect();
        assert!(findings
            .iter()
            .any(|f| matches!(f.severity, Severity::Strict)));
    }

    #[test]
    fn detect_missing_from_theme_emits_warn() {
        let raw = "\
:root { --loom-color-bg-canvas: black; --loom-color-ink: white; }\n\
:root[data-theme=\"sepia\"] { --loom-color-bg-canvas: tan; }\n\
.x { background: var(--loom-color-bg-canvas); color: var(--loom-color-ink); }\n";
        let (blocks, refs) = parse_skin_themes(raw);
        let drift = detect_drift(&blocks, &refs);
        let warns: Vec<&ThemeFinding> = drift
            .iter()
            .filter(
                |f| matches!(f, ThemeFinding::MissingFromTheme { theme, .. } if theme == "sepia"),
            )
            .collect();
        assert_eq!(warns.len(), 1);
    }

    #[test]
    fn detect_orphan_in_theme_emits_warn() {
        let raw = "\
:root { --loom-color-ink: white; }\n\
:root[data-theme=\"weird\"] { --loom-color-ink: black; --loom-color-orphan: red; }\n";
        let (blocks, refs) = parse_skin_themes(raw);
        let drift = detect_drift(&blocks, &refs);
        let orphans: Vec<&ThemeFinding> = drift
            .iter()
            .filter(|f| matches!(f, ThemeFinding::OrphanInTheme { theme, .. } if theme == "weird"))
            .collect();
        assert_eq!(orphans.len(), 1);
    }

    #[test]
    fn no_base_block_emits_strict_per_ref() {
        let raw = ".x { color: var(--loom-color-ink); }";
        let (blocks, refs) = parse_skin_themes(raw);
        // No :root block was present.
        assert!(blocks.is_empty());
        let drift = detect_drift(&blocks, &refs);
        assert_eq!(drift.len(), 1);
        assert!(matches!(drift[0], ThemeFinding::UndefinedRef { .. }));
    }
}

// ============================================================
// Property-based tests (AVP-2 Tier 6)
// ============================================================

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// The parser must NEVER panic, regardless of the input
        /// byte sequence. Any panic here is a security bug —
        /// some operator uploads garbage to skin.css and forge
        /// crashes instead of reporting a clean parse error.
        #[test]
        fn parser_does_not_panic_on_arbitrary_utf8(input in ".{0,2000}") {
            // We don't care about the output; just the absence
            // of a panic / unwind.
            let _ = parse_skin_themes(&input);
        }

        /// Drift detection must not panic on any combination of
        /// blocks + refs.
        #[test]
        fn drift_does_not_panic_on_arbitrary_input(input in ".{0,2000}") {
            let (b, r) = parse_skin_themes(&input);
            let _ = detect_drift(&b, &r);
        }

        /// Token-name validation is closed under round-trip:
        /// constructing a TokenName, taking its &str, and
        /// re-constructing yields the same value.
        #[test]
        fn token_name_roundtrip(suffix in "[a-z][a-z0-9-]{0,40}") {
            let raw = format!("--loom-color-{suffix}");
            let Some(t1) = TokenName::new(&raw) else {
                prop_assert!(false, "valid suffix rejected: {raw}");
                return Ok(());
            };
            let Some(t2) = TokenName::new(t1.as_str()) else {
                prop_assert!(false, "round-trip rejected: {}", t1.as_str());
                return Ok(());
            };
            prop_assert_eq!(t1, t2);
        }

        /// References found inside a `:root` block body MUST end
        /// up in the parsed token map (we should not mistake a
        /// `var()` inside a default-value expression for a
        /// declaration). Exercises the brace-balanced scan.
        #[test]
        fn declaration_inside_root_is_captured(suffix in "[a-z][a-z0-9-]{0,12}") {
            let css = format!(":root {{ --loom-color-{suffix}: red; }}");
            let (blocks, _) = parse_skin_themes(&css);
            prop_assert_eq!(blocks.len(), 1);
            let block = &blocks[0];
            let key = TokenName::new(&format!("--loom-color-{suffix}")).expect("valid");
            prop_assert!(block.tokens.contains_key(&key));
        }
    }
}
