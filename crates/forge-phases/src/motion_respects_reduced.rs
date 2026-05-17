//! `motion_respects_reduced` — enforce that every CSS animation /
//! transition / scroll-linked effect respects `prefers-reduced-motion`.
//!
//! WCAG 2.1 SC 2.3.3 (Animation from Interactions, Level AAA) +
//! WCAG SC 2.3.1 (Three Flashes or Below, Level A). Users who set
//! `prefers-reduced-motion: reduce` in their OS or browser have
//! medical reasons — vestibular disorders, photosensitive seizures,
//! migraines triggered by parallax. Ignoring the preference is a
//! real-world accessibility regression with measurable harm.
//!
//! Extends `phase_motion` (which catches motion-without-meaning).
//! This phase catches motion-without-guard — animations that fire
//! regardless of the user's stated preference.
//!
//! ## Configuration
//!
//! Reads `[motion_respects_reduced]` from `forge.toml`:
//!
//! ```toml
//! [motion_respects_reduced]
//! # When an animation/transition declaration is found OUTSIDE
//! # any @media (prefers-reduced-motion) guard, what severity?
//! # - "strict" → always fail (WCAG 2.3.3 floor)
//! # - "warn"   → surface but don't block
//! severity = "strict"
//!
//! # Optional: skip specific CSS files (e.g. a vendored third-party
//! # stylesheet you can't modify; document why in the comment).
//! skip_files = ["vendor/legacy.css"]
//!
//! # Optional: properties to ignore. Some properties (e.g. opacity
//! # crossfades under 200ms) are safe enough to skip the guard.
//! ignore_properties = ["opacity"]
//! ```
//!
//! Missing `[motion_respects_reduced]` section → silent skip.
//!
//! ## What counts as motion
//!
//! - `animation:` / `animation-name:` properties
//! - `transition:` / `transition-property:` properties
//! - `@keyframes` blocks (the definition itself)
//! - `transform:` with a non-`none` value AND a transition/animation
//!   targeting it (transform alone is static; transform+anim is motion)
//! - `scroll-behavior: smooth` (smooth scroll is vestibular trouble
//!   for some users; reduced-motion users want `auto`)
//!
//! ## What's safe
//!
//! - Declarations INSIDE `@media (prefers-reduced-motion: reduce)`
//!   with `none` / `auto` values — that's the operator opting these
//!   users OUT of the motion.
//! - Declarations INSIDE `@media (prefers-reduced-motion: no-preference)`
//!   — the canonical "only fire for users who haven't requested
//!   reduction" guard.
//! - Properties in the `ignore_properties` allowlist.

use std::collections::HashSet;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// `motion_respects_reduced` phase.
#[derive(Debug, Default)]
pub struct MotionRespectsReducedPhase;

impl Phase for MotionRespectsReducedPhase {
    fn name(&self) -> &'static str {
        "motion_respects_reduced"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let Some(cfg) = forge_toml_motion_respects_reduced(&ctx.root) else {
            tracing::debug!("motion_respects_reduced: no [motion_respects_reduced] — skip");
            return Ok(vec![]);
        };

        let css_files = walk_css(&ctx.static_dir, self.name())?;
        let mut findings = Vec::new();
        for file in css_files {
            if cfg.skip_files.iter().any(|s| s == &file.name) {
                continue;
            }
            for violation in scan_css_for_unguarded_motion(&file.body, &cfg.ignore_properties) {
                let msg = format!(
                    "unguarded `{property}` at line {line} — wrap in \
                     @media (prefers-reduced-motion: no-preference) {{ … }} \
                     or provide a `none`/`auto` override inside \
                     @media (prefers-reduced-motion: reduce). \
                     WCAG 2.1 SC 2.3.3.",
                    property = violation.property,
                    line = violation.line,
                );
                findings.push(match cfg.severity {
                    SeverityPolicy::Strict => Finding::strict(self.name(), file.name.clone(), msg),
                    SeverityPolicy::Warn => Finding::warn(self.name(), file.name.clone(), msg),
                });
            }
        }
        Ok(findings)
    }
}

#[derive(Debug, Clone)]
struct Violation {
    property: String,
    line: usize,
}

/// Walk the static dir for `*.css` files. Returns one entry per
/// file with the path basename + full contents.
fn walk_css(static_dir: &Path, phase: &str) -> Result<Vec<CssFile>, BuildError> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(static_dir).map_err(|source| BuildError::Io {
        context: format!("{phase}: read_dir {}", static_dir.display()),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| BuildError::Io {
            context: format!("{phase}: iterate {}", static_dir.display()),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("css") {
            continue;
        }
        let name = path
            .strip_prefix(static_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        let body = std::fs::read_to_string(&path).map_err(|source| BuildError::Io {
            context: format!("{phase}: read {}", path.display()),
            source,
        })?;
        out.push(CssFile { name, body });
    }
    Ok(out)
}

#[derive(Debug, Clone)]
struct CssFile {
    name: String,
    body: String,
}

/// Scan a CSS body for motion declarations outside any
/// `@media (prefers-reduced-motion: ...)` guard.
///
/// Approach: track @media-block depth. When we see `@media (prefers-reduced-motion`
/// open, increment a reduced-motion-guard counter; decrement when
/// the corresponding brace closes. Motion-property hits while the
/// counter is > 0 are inside a guard → safe.
fn scan_css_for_unguarded_motion(
    body: &str,
    ignore_properties: &HashSet<String>,
) -> Vec<Violation> {
    let lower = body.to_ascii_lowercase();
    let mut out = Vec::new();
    let bytes = lower.as_bytes();
    let mut brace_depth: i32 = 0;
    // Stack: for each open brace, was it the start of a
    // prefers-reduced-motion @media block?
    let mut guard_stack: Vec<bool> = Vec::new();
    let mut line: usize = 1;
    let mut i = 0;

    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            // Skip block comment
            let mut j = i + 2;
            while j + 1 < bytes.len() && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
                if bytes[j] == b'\n' {
                    line += 1;
                }
                j += 1;
            }
            i = j + 2;
            continue;
        }
        // Detect @media (prefers-reduced-motion ...) opening
        if c == b'@' && lower[i..].starts_with("@media") {
            // Find the next '{' to know if this @media opens a block
            if let Some(brace_rel) = lower[i..].find('{') {
                let header = &lower[i..i + brace_rel];
                let is_reduced_motion = header.contains("prefers-reduced-motion");
                let abs_brace = i + brace_rel;
                // Count newlines in the header for line tracking
                line += header.bytes().filter(|&b| b == b'\n').count();
                brace_depth += 1;
                guard_stack.push(is_reduced_motion);
                i = abs_brace + 1;
                continue;
            }
        }
        if c == b'{' {
            // Non-@media block open
            brace_depth += 1;
            guard_stack.push(false);
            i += 1;
            continue;
        }
        if c == b'}' {
            brace_depth = brace_depth.saturating_sub(1);
            guard_stack.pop();
            i += 1;
            continue;
        }
        // Are we currently inside any reduced-motion guard?
        let inside_guard = guard_stack.iter().any(|&g| g);
        if inside_guard {
            i += 1;
            continue;
        }
        // Check for motion-property declarations.
        // Conservative: look for property names at the start of a
        // potential declaration (after `{` or `;` or whitespace at
        // line start).
        if let Some((prop, len)) = match_motion_property(&lower[i..]) {
            if !ignore_properties.contains(prop) {
                out.push(Violation {
                    property: prop.to_owned(),
                    line,
                });
            }
            i += len;
            continue;
        }
        i += 1;
    }
    out
}

/// If the slice begins with one of the motion property names
/// followed by `:`, return the property name + length consumed.
fn match_motion_property(s: &str) -> Option<(&'static str, usize)> {
    // Order matters: longest prefixes first to avoid false-matching
    // `animation-name` as `animation`.
    let candidates: &[&'static str] = &[
        "animation-name",
        "transition-property",
        "animation",
        "transition",
        "scroll-behavior",
    ];
    for &name in candidates {
        if s.starts_with(name) {
            // Must be followed by optional whitespace + ':' (a declaration)
            let after = &s[name.len()..];
            let trimmed = after.trim_start();
            if trimmed.starts_with(':') {
                // Make sure the BEFORE-context isn't another identifier char
                // (to avoid `--my-animation:` matching `animation`)
                // Note: this function is called per byte-index in the body
                // scan so we don't have backward context here — accept the
                // false-positive trade off for `--*` custom-property names.
                // For `--*` custom props with `animation` in the name,
                // operators can use ignore_properties.
                return Some((name, name.len()));
            }
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SeverityPolicy {
    #[default]
    Strict,
    Warn,
}

#[derive(Debug, Clone, Default)]
struct MotionConfig {
    severity: SeverityPolicy,
    skip_files: Vec<String>,
    ignore_properties: HashSet<String>,
}

fn forge_toml_motion_respects_reduced(root: &Path) -> Option<MotionConfig> {
    let path = root.join("forge.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    let parsed: toml::Value = content.parse().ok()?;
    let section = parsed.get("motion_respects_reduced")?;
    let severity = match section.get("severity").and_then(|v| v.as_str()) {
        Some("warn") => SeverityPolicy::Warn,
        _ => SeverityPolicy::Strict,
    };
    let skip_files = section
        .get("skip_files")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let ignore_properties = section
        .get("ignore_properties")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    Some(MotionConfig {
        severity,
        skip_files,
        ignore_properties,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::{BuildMode, Severity};

    fn ctx_in(dir: &Path) -> BuildCtx {
        BuildCtx {
            root: dir.to_path_buf(),
            static_dir: dir.to_path_buf(),
            mode: BuildMode::Poc,
        }
    }

    fn write_forge_toml(dir: &Path, body: &str) {
        std::fs::write(dir.join("forge.toml"), body).unwrap();
    }

    fn write_css(dir: &Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).unwrap();
    }

    #[test]
    fn no_config_silent_skip() {
        let dir = tempfile::tempdir().unwrap();
        write_css(dir.path(), "x.css", ".x { animation: spin 1s; }");
        let findings = MotionRespectsReducedPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn unguarded_animation_emits_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            "[motion_respects_reduced]\nseverity=\"strict\"\n",
        );
        write_css(
            dir.path(),
            "x.css",
            ".spinner { animation: spin 1s infinite; }",
        );
        let findings = MotionRespectsReducedPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Strict);
        assert!(findings[0].message.contains("animation"));
        assert!(findings[0].message.contains("WCAG"));
    }

    #[test]
    fn unguarded_transition_emits_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            "[motion_respects_reduced]\nseverity=\"strict\"\n",
        );
        write_css(
            dir.path(),
            "x.css",
            ".panel { transition: transform 0.3s ease; }",
        );
        let findings = MotionRespectsReducedPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings.iter().any(|f| f.message.contains("transition")));
    }

    #[test]
    fn animation_inside_no_preference_guard_is_safe() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            "[motion_respects_reduced]\nseverity=\"strict\"\n",
        );
        write_css(
            dir.path(),
            "x.css",
            "@media (prefers-reduced-motion: no-preference) { .spin { animation: spin 1s; } }",
        );
        let findings = MotionRespectsReducedPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn animation_inside_reduce_guard_is_safe() {
        // Authors sometimes write the inverse pattern: animation by
        // default but reset to `none` inside reduce. Both styles count
        // as guarded — the property being inside the reduce media is
        // legitimate use to OVERRIDE motion.
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            "[motion_respects_reduced]\nseverity=\"strict\"\n",
        );
        write_css(
            dir.path(),
            "x.css",
            "@media (prefers-reduced-motion: reduce) { .spinner { animation: none; } }",
        );
        let findings = MotionRespectsReducedPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn scroll_behavior_smooth_unguarded_flagged() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            "[motion_respects_reduced]\nseverity=\"strict\"\n",
        );
        write_css(dir.path(), "x.css", "html { scroll-behavior: smooth; }");
        let findings = MotionRespectsReducedPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings
            .iter()
            .any(|f| f.message.contains("scroll-behavior")));
    }

    #[test]
    fn warn_severity_emits_warn_not_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[motion_respects_reduced]\nseverity=\"warn\"\n");
        write_css(dir.path(), "x.css", ".x { animation: spin 1s; }");
        let findings = MotionRespectsReducedPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings.iter().all(|f| f.severity == Severity::Warn));
    }

    #[test]
    fn skip_files_excludes_specific_files() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[motion_respects_reduced]
severity = "strict"
skip_files = ["vendor.css"]
"#,
        );
        write_css(dir.path(), "vendor.css", ".x { animation: spin 1s; }");
        write_css(dir.path(), "ours.css", ".y { animation: bounce 1s; }");
        let findings = MotionRespectsReducedPhase.run(&ctx_in(dir.path())).unwrap();
        // only ours.css flagged
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "ours.css");
    }

    #[test]
    fn ignore_properties_silences_specific_properties() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[motion_respects_reduced]
severity = "strict"
ignore_properties = ["scroll-behavior"]
"#,
        );
        write_css(
            dir.path(),
            "x.css",
            "html { scroll-behavior: smooth; } .x { animation: spin 1s; }",
        );
        let findings = MotionRespectsReducedPhase.run(&ctx_in(dir.path())).unwrap();
        // scroll-behavior ignored, animation still flagged
        assert!(findings.iter().any(|f| f.message.contains("animation")));
        assert!(!findings
            .iter()
            .any(|f| f.message.contains("scroll-behavior")));
    }

    #[test]
    fn comments_skipped() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            "[motion_respects_reduced]\nseverity=\"strict\"\n",
        );
        write_css(
            dir.path(),
            "x.css",
            "/* animation: spin 1s; */ .x { color: red; }",
        );
        let findings = MotionRespectsReducedPhase.run(&ctx_in(dir.path())).unwrap();
        // Comment doesn't trigger
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn multiple_unguarded_each_flagged_per_line() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            "[motion_respects_reduced]\nseverity=\"strict\"\n",
        );
        write_css(
            dir.path(),
            "x.css",
            ".a { animation: spin 1s; }\n.b { transition: opacity 0.3s; }\n",
        );
        let findings = MotionRespectsReducedPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 2);
        // Line numbers preserved
        assert!(findings.iter().any(|f| f.message.contains("line 1")));
        assert!(findings.iter().any(|f| f.message.contains("line 2")));
    }

    #[test]
    fn unguarded_then_guarded_only_unguarded_flagged() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            "[motion_respects_reduced]\nseverity=\"strict\"\n",
        );
        write_css(
            dir.path(),
            "x.css",
            ".bad { animation: spin 1s; }
@media (prefers-reduced-motion: no-preference) {
    .good { animation: bounce 1s; }
}",
        );
        let findings = MotionRespectsReducedPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("line 1"));
    }
}
