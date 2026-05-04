//! `motion` — every CSS file shipping `animation:` / `transition:`
//! / `@keyframes` MUST also contain a `@media (prefers-reduced-
//! motion: reduce)` block that disables motion (touches both
//! `animation-duration` AND `transition-duration`).
//!
//! Bash parity: forge.sh `phase_motion`. WCAG 2.3.3 AAA but
//! ship-blocking for any production app. Vestibular accessibility
//! is non-negotiable.

use std::fs;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// `motion` phase.
#[derive(Debug, Default)]
pub struct MotionPhase;

impl Phase for MotionPhase {
    fn name(&self) -> &'static str {
        "motion"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let entries = match fs::read_dir(&ctx.static_dir) {
            Ok(it) => it,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(findings),
            Err(e) => {
                return Err(BuildError::Io {
                    context: format!("{}: read_dir {}", self.name(), ctx.static_dir.display()),
                    source: e,
                });
            }
        };
        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| BuildError::Io {
                context: format!("{}: dir entry", self.name()),
                source: e,
            })?;
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) == Some("css") {
                paths.push(p);
            }
        }
        paths.sort();

        for p in paths {
            let body = match fs::read_to_string(&p) {
                Ok(s) => s,
                Err(e) => {
                    return Err(BuildError::Io {
                        context: format!("{}: read {}", self.name(), p.display()),
                        source: e,
                    });
                }
            };
            if !has_motion(&body) {
                continue;
            }
            let name = p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_owned();
            if !has_reduced_motion_kill_switch(&body) {
                findings.push(Finding::strict(
                    self.name(),
                    name,
                    "ships motion (animation/transition/@keyframes) without prefers-reduced-motion fallback that disables BOTH animation-duration AND transition-duration",
                ));
            }
        }

        Ok(findings)
    }
}

/// True if the file declares motion in some form.
fn has_motion(css: &str) -> bool {
    has_decl(css, "animation:") || has_decl(css, "transition:") || css.contains("@keyframes")
}

fn has_decl(css: &str, key: &str) -> bool {
    // Must be at a word boundary (no `transitionable:` matches).
    let mut search = css;
    while let Some(idx) = search.find(key) {
        let prev = if idx == 0 {
            ' '
        } else {
            search.as_bytes()[idx - 1] as char
        };
        if !prev.is_alphanumeric() && prev != '_' && prev != '-' {
            return true;
        }
        search = &search[idx + key.len()..];
    }
    false
}

/// True if there's a `@media (prefers-reduced-motion: reduce)` block
/// that contains both `animation-duration` AND `transition-duration`.
fn has_reduced_motion_kill_switch(css: &str) -> bool {
    let needle = "prefers-reduced-motion";
    let mut search = css;
    while let Some(idx) = search.find(needle) {
        // Find the `{` that opens the @media block, then the
        // matching `}` (depth-aware).
        let after = &search[idx..];
        let Some(brace_idx) = after.find('{') else {
            break;
        };
        let block = match find_balanced_block(&after[brace_idx..]) {
            Some(b) => b,
            None => break,
        };
        if block.contains("animation-duration") && block.contains("transition-duration") {
            return true;
        }
        search = &after[brace_idx + block.len()..];
    }
    false
}

/// Given a string starting with `{`, return the substring up to
/// and including the matching `}`. Brace-depth aware so nested
/// blocks (e.g. inner selectors inside a media query) work.
fn find_balanced_block(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'{') {
        return None;
    }
    let mut depth: i32 = 0;
    for (i, &c) in bytes.iter().enumerate() {
        if c == b'{' {
            depth += 1;
        } else if c == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(&s[..=i]);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_motion_in_animation() {
        assert!(has_motion(".x { animation: spin 2s linear infinite; }"));
    }

    #[test]
    fn detects_motion_in_transition() {
        assert!(has_motion(".x { transition: color 0.2s; }"));
    }

    #[test]
    fn detects_motion_in_keyframes() {
        assert!(has_motion("@keyframes spin { from {} to {} }"));
    }

    #[test]
    fn no_false_positive_on_substring_property_name() {
        // `transitionable:` is hypothetical but proves the boundary
        // check works.
        assert!(!has_decl(".x { mytransition: foo; }", "transition:"));
    }

    #[test]
    fn detects_complete_kill_switch() {
        let css = "
            @media (prefers-reduced-motion: reduce) {
                * {
                    animation-duration: 0.001ms !important;
                    transition-duration: 0.001ms !important;
                }
            }
            .x { animation: spin 2s; }
        ";
        assert!(has_reduced_motion_kill_switch(css));
    }

    #[test]
    fn flags_partial_kill_switch() {
        // Animation-duration only — transition still runs.
        let css = "
            @media (prefers-reduced-motion: reduce) {
                * { animation-duration: 0.001ms !important; }
            }
        ";
        assert!(!has_reduced_motion_kill_switch(css));
    }

    #[test]
    fn flags_missing_kill_switch_entirely() {
        let css = ".x { animation: spin 2s; }";
        assert!(!has_reduced_motion_kill_switch(css));
    }
}
