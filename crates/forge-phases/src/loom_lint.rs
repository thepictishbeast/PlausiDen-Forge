//! `loom_lint` — wrap PlausiDen-Loom's `loom-lint` CSS scanner
//! into a Forge phase.
//!
//! Owner directive 2026-05-13: every Forge build should mechanically
//! enforce the loom-lint rules so doctrine drift fails closed at
//! build time. Today loom-lint is a standalone CLI; this phase makes
//! it part of the build pipeline.
//!
//! Severity ladder (per T39):
//!   * `RawColour` → strict (a hex literal in non-token CSS is a
//!     direct token-system violation).
//!   * `RawSpacing` → warn (sub-token spacing is sometimes
//!     legitimate; flag for review without blocking).
//!   * `RawTime` → warn (animation duration drift; same shape).
//!
//! AVP-PASS-T39: 2026-05-14.

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use loom_lint::{run_css_default, CssViolation, CssViolationKind};

/// `loom_lint` phase implementation.
#[derive(Debug, Default)]
pub struct LoomLintPhase;

impl Phase for LoomLintPhase {
    fn name(&self) -> &'static str {
        "loom_lint"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        // loom-lint's CSS scanner walks the project root for *.css
        // / *.scss files. We point it at ctx.root so it covers
        // both authored CSS and any generated CSS in static/.
        let violations = run_css_default(&ctx.root).map_err(|e| BuildError::Io {
            context: format!("loom_lint::run_css_default({}): {e}", ctx.root.display()),
            source: std::io::Error::other(e.to_string()),
        })?;

        let mut findings = Vec::with_capacity(violations.len());
        for v in violations {
            findings.push(violation_to_finding(self.name(), &v));
        }
        Ok(findings)
    }
}

/// Map a loom-lint `CssViolation` into a Forge `Finding` with the
/// per-kind severity (strict for colour, warn for spacing/time).
fn violation_to_finding(phase: &'static str, v: &CssViolation) -> Finding {
    let path_display = v.path.display().to_string();
    let line = v.line;
    let matched = &v.matched;
    match v.kind {
        CssViolationKind::RawColour => Finding::strict(
            phase,
            path_display,
            format!(
                "{line}: raw colour literal: {matched}"
            ),
        )
        .citing(["prim-007"])
        .why("raw color literals bypass loom-tokens' theme cascade; primitives that hard-code colors don't render correctly in light + dark + amoled themes")
        .fix(format!("wrap `{matched}` in `var(--loom-color-*)` or define the color as a token in loom-tokens/src/skin.css and reference it"))
        .skill("add-loom-primitive")
        .avoid("don't edit the static/loom-skin.css output — it's a build artifact regenerated from loom-tokens source"),
        CssViolationKind::RawSpacing => Finding::warn(
            phase,
            path_display,
            format!(
                "{line}: raw spacing literal: {matched}"
            ),
        )
        .citing(["prim-007"])
        .why("raw spacing values (px / em / pt) bypass the loom-space-* scale; primitives lose responsive + density-aware behavior")
        .fix(format!("use `var(--loom-space-N)` (N = 0..16 on the canonical scale) in place of `{matched}`"))
        .skill("add-loom-primitive"),
        CssViolationKind::RawTime => Finding::warn(
            phase,
            path_display,
            format!(
                "{line}: raw time literal: {matched}"
            ),
        )
        .citing(["prim-007"])
        .why("raw time values bypass loom-motion tokens; primitives don't respect prefers-reduced-motion + tenant-level motion overrides")
        .fix(format!("use `var(--loom-motion-duration-*)` or `var(--loom-motion-ease-*)` in place of `{matched}`"))
        .skill("add-loom-primitive"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::Severity;

    fn make_ctx_with_css(name: &str, body: &str) -> (BuildCtx, std::path::PathBuf) {
        let tmp = std::env::temp_dir().join(format!(
            "loom-lint-t39-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&tmp).expect("mk");
        std::fs::write(tmp.join(name), body).expect("write");
        let ctx = BuildCtx {
            root: tmp.clone(),
            static_dir: tmp.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        (ctx, tmp)
    }

    #[test]
    fn raw_colour_emits_strict_finding() {
        let (ctx, tmp) =
            make_ctx_with_css("style.css", ".btn { color: #ff0000; padding: 1rem; }\n");
        let findings = LoomLintPhase.run(&ctx).expect("run");
        assert!(
            findings
                .iter()
                .any(|f| matches!(f.severity, Severity::Strict) && f.message.contains("raw colour")),
            "missing strict colour finding: {findings:?}"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn raw_spacing_emits_warn_finding() {
        let (ctx, tmp) = make_ctx_with_css("style.css", ".btn { padding: 12px; }\n");
        let findings = LoomLintPhase.run(&ctx).expect("run");
        assert!(
            findings
                .iter()
                .any(|f| matches!(f.severity, Severity::Warn) && f.message.contains("raw spacing")),
            "missing warn spacing finding: {findings:?}"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn raw_time_emits_warn_finding() {
        let (ctx, tmp) =
            make_ctx_with_css("style.css", ".btn { transition: opacity 200ms ease; }\n");
        let findings = LoomLintPhase.run(&ctx).expect("run");
        assert!(
            findings
                .iter()
                .any(|f| matches!(f.severity, Severity::Warn) && f.message.contains("raw time")),
            "missing warn time finding: {findings:?}"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn token_block_passes_clean() {
        let (ctx, tmp) = make_ctx_with_css(
            "style.css",
            ":root { --loom-color-ink: #111; --loom-space-md: 16px; --loom-motion-fast: 200ms; }\n",
        );
        let findings = LoomLintPhase.run(&ctx).expect("run");
        assert!(
            findings.is_empty(),
            "token block must pass clean: {findings:?}"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn no_css_files_passes_silently() {
        // A project with no CSS at all should run cleanly.
        let tmp = std::env::temp_dir().join(format!("loom-lint-t39-empty-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).expect("mk");
        let ctx = BuildCtx {
            root: tmp.clone(),
            static_dir: tmp.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = LoomLintPhase.run(&ctx).expect("run");
        assert!(findings.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
