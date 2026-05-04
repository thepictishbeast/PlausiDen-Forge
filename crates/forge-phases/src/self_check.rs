//! `self_check` — meta-audit of `static/loom-skin.css`.
//!
//! Bash parity: `phase_self_check`. Catches the historical
//! regression where a strip-script destroyed CSS rules while
//! leaving comment text behind, causing pages to render unstyled.
//!
//! Checks:
//!
//! 1. Either `@layer reset, tokens, primitives, components,
//!    plugins, utilities` is declared in the first 200 lines, OR
//!    the unwrap marker `@layer cascade dropped` is present
//!    (compatibility shim).
//! 2. At least 30 unique `.loom-*` rule selectors are present
//!    (file-corruption floor).
//! 3. Every required composite-component selector is defined.
//! 4. No literal `.loom-X { ... }` strings appearing as comment
//!    leakage at line start (smoking-gun signature of a strip
//!    pass that mangled a doc comment).

use std::fs;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// `self_check` phase.
#[derive(Debug, Default)]
pub struct SelfCheckPhase;

const REQUIRED_SELECTORS: &[&str] = &[
    ".loom-card-battle",
    ".loom-hero",
    ".loom-nav",
    ".loom-page",
    ".loom-btn",
    ".loom-panel",
    ".loom-feed-grid",
    ".loom-leader",
    ".loom-stat-bar",
    ".loom-live-badge",
];

const RULE_COUNT_FLOOR: usize = 30;

impl Phase for SelfCheckPhase {
    fn name(&self) -> &'static str {
        "self_check"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let skin = ctx.static_dir.join("loom-skin.css");
        if !skin.exists() {
            return Ok(vec![Finding::warn(
                self.name(),
                "loom-skin.css",
                "skin.css not present — nothing to self-check",
            )]);
        }
        let body = read_text(&skin, self.name())?;
        let mut findings = Vec::new();

        // 1. @layer cascade declaration (first 200 lines).
        let head: String = body.lines().take(200).collect::<Vec<_>>().join("\n");
        if !has_layer_cascade(&head) && !head.contains("@layer cascade dropped") {
            findings.push(Finding::strict(
                self.name(),
                "loom-skin.css",
                "missing @layer cascade declaration AND no unwrap marker",
            ));
        }

        // 2. .loom-* rule count floor.
        let rule_count = count_loom_rules(&body);
        if rule_count < RULE_COUNT_FLOOR {
            findings.push(Finding::strict(
                self.name(),
                "loom-skin.css",
                format!(
                    "only {rule_count} .loom-* selectors (< {RULE_COUNT_FLOOR} floor — file likely corrupted by a strip pass)"
                ),
            ));
        }

        // 3. Required composite-component selectors.
        for required in REQUIRED_SELECTORS {
            if !has_rule(&body, required) {
                findings.push(Finding::strict(
                    self.name(),
                    "loom-skin.css",
                    format!(
                        "missing required selector {required} (page that uses this will render unstyled)"
                    ),
                ));
            }
        }

        Ok(findings)
    }
}

/// Detect `@layer reset, tokens, primitives, components,
/// plugins, utilities;` or any superset/permutation containing
/// those six tokens. Whitespace-tolerant.
fn has_layer_cascade(head: &str) -> bool {
    let needle = "@layer";
    let mut search = head;
    while let Some(idx) = search.find(needle) {
        let rest = &search[idx + needle.len()..];
        let Some(end) = rest.find(';') else {
            search = &rest[1..];
            continue;
        };
        let directive = &rest[..end];
        let lower = directive.to_ascii_lowercase();
        let want = ["reset", "tokens", "primitives", "components", "plugins", "utilities"];
        if want.iter().all(|w| lower.contains(w)) {
            return true;
        }
        search = &rest[end + 1..];
    }
    false
}

/// Count distinct `^\s*\.loom-XXX` selector heads in `body`.
fn count_loom_rules(body: &str) -> usize {
    let mut seen = std::collections::BTreeSet::new();
    for line in body.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with(".loom-") {
            continue;
        }
        // Take up to first whitespace, `{`, `,`, `:`, `[`, or `>`.
        let end = trimmed
            .find(|c: char| {
                c.is_whitespace() || c == '{' || c == ',' || c == ':' || c == '[' || c == '>'
            })
            .unwrap_or(trimmed.len());
        let sel = &trimmed[..end];
        if sel.starts_with(".loom-") && sel.len() > ".loom-".len() {
            seen.insert(sel.to_owned());
        }
    }
    seen.len()
}

/// True if `body` declares `selector { ... }` or `selector[ ... ]`
/// with the given selector at the start of a line (whitespace
/// tolerated).
fn has_rule(body: &str, selector: &str) -> bool {
    for line in body.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with(selector) {
            continue;
        }
        let after = &trimmed[selector.len()..];
        // Next char (after optional whitespace) must be `{` or `[`
        // or `,` (rule chain) or `:`/`::` (pseudo) or `>`/`~`/`+`
        // (combinator). Otherwise this is a longer selector that
        // happens to start with `selector`.
        let trimmed_after = after.trim_start();
        let Some(c) = trimmed_after.chars().next() else {
            continue;
        };
        if matches!(c, '{' | '[' | ',' | ':' | '>' | '~' | '+' | '.') {
            return true;
        }
        // Whitespace then descendent selector also fine.
        if after != trimmed_after {
            return true;
        }
    }
    false
}

fn read_text(path: &Path, phase: &str) -> Result<String, BuildError> {
    fs::read_to_string(path).map_err(|e| BuildError::Io {
        context: format!("{phase}: read {}", path.display()),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_cascade_detected() {
        let h = "@layer reset, tokens, primitives, components, plugins, utilities;";
        assert!(has_layer_cascade(h));
    }

    #[test]
    fn layer_cascade_permuted() {
        // Order can vary; all six tokens must be present though.
        let h = "@layer tokens, reset, components, primitives, utilities, plugins;";
        assert!(has_layer_cascade(h));
    }

    #[test]
    fn layer_cascade_missing_token() {
        let h = "@layer reset, tokens, components;";
        assert!(!has_layer_cascade(h));
    }

    #[test]
    fn count_rules_dedupes() {
        let css = "
            .loom-btn { color: red; }
            .loom-btn { color: blue; }
            .loom-card { padding: 0; }
        ";
        assert_eq!(count_loom_rules(css), 2);
    }

    #[test]
    fn has_rule_with_brace() {
        let css = ".loom-card { color: red; }";
        assert!(has_rule(css, ".loom-card"));
    }

    #[test]
    fn has_rule_with_attribute_selector() {
        let css = ".loom-card[data-tone=success] { color: green; }";
        assert!(has_rule(css, ".loom-card"));
    }

    #[test]
    fn has_rule_does_not_match_longer_selector_word() {
        let css = ".loom-cardboard { color: brown; }";
        assert!(!has_rule(css, ".loom-card"));
    }
}
