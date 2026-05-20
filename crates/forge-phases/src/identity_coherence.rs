//! `identity_coherence` — cross-axis identity-consistency check.
//!
//! Task #240 per the variation-architecture spec. Where
//! site_identity_conformance (#235) checks the actual CMS matches
//! the declared identity, and theme_variation_required (#261)
//! checks the platform default-theme floor, this phase checks the
//! declared identity for INTERNAL CROSS-AXIS CONSISTENCY:
//!
//! * `voice.tier = "technical"` without `code`/`terminal` in
//!   `allowed_primitives` — technical sites should allow technical
//!   primitives.
//! * `voice.tier = "plain"` without `code`/`terminal` in
//!   `forbidden_primitives` — plain sites should restrict
//!   technical primitives.
//! * `mood.primary = "editorial"` without any theme variant
//!   declared — editorial mood implies thoughtful theme work.
//! * `mood.primary = "kinetic"` without motion-related primitives
//!   in `allowed_primitives` — kinetic mood needs motion vocabulary.
//! * `density_preference = "dense"` with
//!   `tokens.max_per_page_overrides < 4` — dense sites need
//!   per-page flexibility above the substrate default.
//! * `density_preference = "sparse"` with
//>   `tokens.max_per_page_overrides > 6` — sparse sites should
//>   stay consistent; high per-page overrides drift away.
//!
//! All findings are **warn** by default — these are coherence
//! heuristics, not hard rules. Operators can ignore them or
//! escalate via `[identity_coherence] strict = true` to make
//! every coherence warning a strict finding.
//!
//! Per the controlled-mutability arc (#238-#240), this phase
//! catches the failure mode where an operator updates one axis
//! of identity (e.g. voice.tier) without thinking through the
//! cascade effects on related axes.
//!
//! ## forge.toml shape
//!
//! No phase-specific section required — the phase runs against
//! whatever `[site_identity]` declares.
//!
//! ```toml
//! [identity_coherence]
//! strict = true   # optional; escalate all warns to strict
//! ```
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * `#[non_exhaustive]` on phase struct.
//! * No unwrap/expect in non-test code.

use std::fs;
use std::path::Path;

use forge_core::site_identity::SiteIdentity;
use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// `identity_coherence` phase implementation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct IdentityCoherencePhase;

impl Phase for IdentityCoherencePhase {
    fn name(&self) -> &'static str {
        "identity_coherence"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let Some(identity) = SiteIdentity::load(&ctx.root) else {
            return Ok(findings);
        };
        if identity.is_default() {
            return Ok(findings);
        }
        let strict = read_strict_flag(&ctx.root);

        let mut push = |code: &str, message: String, why: &str, fix: &str| {
            let f = if strict {
                Finding::strict("identity_coherence", ctx.root.join("forge.toml").display().to_string(), message)
            } else {
                Finding::warn("identity_coherence", ctx.root.join("forge.toml").display().to_string(), message)
            };
            findings.push(f.citing([code]).why(why.to_owned()).fix(fix.to_owned()));
        };

        // Voice ↔ primitives.
        if let Some(tier) = identity.voice.tier.as_deref() {
            match tier {
                "technical" => {
                    let has_technical = identity
                        .allowed_primitives
                        .iter()
                        .any(|p| p == "code" || p == "terminal" || p == "code_block")
                        || identity.allowed_primitives.is_empty(); // empty = all allowed
                    if !has_technical {
                        push(
                            "ident-201",
                            "identity_coherence — voice.tier = `technical` but allowed_primitives doesn't include `code`/`terminal`/`code_block`"
                                .to_owned(),
                            "technical voice tier implies technical content; the whitelist actively excludes the primitives that carry that content",
                            "add `code` / `terminal` / `code_block` to [site_identity].allowed_primitives, OR change voice.tier to one that doesn't expect technical content",
                        );
                    }
                }
                "plain" => {
                    let restricts_technical = identity
                        .forbidden_primitives
                        .iter()
                        .any(|p| p == "code" || p == "terminal" || p == "code_block");
                    if !restricts_technical {
                        push(
                            "ident-202",
                            "identity_coherence — voice.tier = `plain` but forbidden_primitives doesn't restrict `code`/`terminal`/`code_block`"
                                .to_owned(),
                            "plain voice tier targets non-technical readers; technical primitives contradict the declared register",
                            "add `code` / `terminal` / `code_block` to [site_identity].forbidden_primitives, OR change voice.tier to `casual` or higher",
                        );
                    }
                }
                _ => {}
            }
        }

        // Mood ↔ themes.
        if let Some(mood) = identity.mood.primary.as_deref() {
            if mood == "editorial" && identity.theme_variant.is_empty() {
                push(
                    "ident-203",
                    "identity_coherence — mood.primary = `editorial` but no theme_variant declared"
                        .to_owned(),
                    "editorial mood implies considered design; an undeclared theme leaves the dual-theme floor unmet",
                    "declare at least one light + one dark theme_variant under [site_identity]",
                );
            }
            if mood == "kinetic" {
                let kinetic_kinds = [
                    "marquee",
                    "motion_section",
                    "sparkline",
                    "histogram",
                    "bar_chart",
                ];
                let has_kinetic = identity
                    .allowed_primitives
                    .is_empty()
                    || identity
                        .allowed_primitives
                        .iter()
                        .any(|p| kinetic_kinds.contains(&p.as_str()));
                if !has_kinetic {
                    push(
                        "ident-204",
                        "identity_coherence — mood.primary = `kinetic` but allowed_primitives excludes motion-family primitives"
                            .to_owned(),
                        "kinetic mood requires a vocabulary that can express motion + data velocity; the whitelist excludes that vocabulary",
                        "add `marquee` / `sparkline` / `histogram` / `motion_section` to allowed_primitives, OR change mood.primary",
                    );
                }
            }
        }

        // Density ↔ token budget.
        if let Some(density) = identity.density_preference.as_deref() {
            let max_per_page = identity.tokens.max_per_page_overrides;
            match density {
                "dense" | "extreme" => {
                    if max_per_page > 0 && max_per_page < 4 {
                        push(
                            "ident-205",
                            format!(
                                "identity_coherence — density_preference = `{density}` but tokens.max_per_page_overrides = {max_per_page} (< 4)"
                            ),
                            "dense layouts often need per-page token overrides to honor section variety; the budget is too tight",
                            "raise tokens.max_per_page_overrides to 4+ OR reduce density_preference",
                        );
                    }
                }
                "sparse" => {
                    if max_per_page > 6 {
                        push(
                            "ident-206",
                            format!(
                                "identity_coherence — density_preference = `sparse` but tokens.max_per_page_overrides = {max_per_page} (> 6)"
                            ),
                            "sparse layouts intend consistent rhythm; a high per-page override budget invites the consistency to drift",
                            "lower tokens.max_per_page_overrides to 6 or below OR raise density_preference",
                        );
                    }
                }
                _ => {}
            }
        }

        Ok(findings)
    }
}

fn read_strict_flag(root: &Path) -> bool {
    let Some(body) = fs::read_to_string(root.join("forge.toml")).ok() else {
        return false;
    };
    let Ok(value) = toml::from_str::<toml::Value>(&body) else {
        return false;
    };
    value
        .get("identity_coherence")
        .and_then(|v| v.get("strict"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "forge-ident-coherence-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn ctx_for(root: &Path) -> BuildCtx {
        BuildCtx {
            root: root.to_path_buf(),
            static_dir: root.join("static"),
            mode: forge_core::BuildMode::Poc,
        }
    }

    #[test]
    fn phase_silent_when_no_identity() {
        let root = temp_root("no-identity");
        let findings = IdentityCoherencePhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_technical_voice_without_code_allowed() {
        let root = temp_root("tech-no-code");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]
allowed_primitives = ["hero_editorial", "paragraph"]

[site_identity.voice]
tier = "technical"
"#,
        )
        .unwrap();
        let findings = IdentityCoherencePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("voice.tier = `technical`")),
            "expected ident-201 finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_for_technical_with_code_allowed() {
        let root = temp_root("tech-with-code");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]
allowed_primitives = ["code", "paragraph", "heading"]

[site_identity.voice]
tier = "technical"
"#,
        )
        .unwrap();
        let findings = IdentityCoherencePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            !findings.iter().any(|f| f.message.contains("voice.tier = `technical`")),
            "should be silent when code allowed; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_plain_voice_without_code_forbidden() {
        let root = temp_root("plain-no-restrict");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]

[site_identity.voice]
tier = "plain"
"#,
        )
        .unwrap();
        let findings = IdentityCoherencePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("voice.tier = `plain`")),
            "expected ident-202 finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_editorial_mood_without_themes() {
        let root = temp_root("editorial-no-themes");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]

[site_identity.mood]
primary = "editorial"
"#,
        )
        .unwrap();
        let findings = IdentityCoherencePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("editorial`")),
            "expected ident-203 finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_dense_with_tight_token_budget() {
        let root = temp_root("dense-tight");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]
density_preference = "dense"

[site_identity.tokens]
max_per_page_overrides = 2
"#,
        )
        .unwrap();
        let findings = IdentityCoherencePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("density_preference = `dense`")),
            "expected ident-205 finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_sparse_with_loose_token_budget() {
        let root = temp_root("sparse-loose");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]
density_preference = "sparse"

[site_identity.tokens]
max_per_page_overrides = 10
"#,
        )
        .unwrap();
        let findings = IdentityCoherencePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("density_preference = `sparse`")),
            "expected ident-206 finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn strict_flag_escalates_warns_to_strict() {
        let root = temp_root("strict");
        fs::write(
            root.join("forge.toml"),
            r#"
[identity_coherence]
strict = true

[site_identity]

[site_identity.voice]
tier = "plain"
"#,
        )
        .unwrap();
        let findings = IdentityCoherencePhase.run(&ctx_for(&root)).unwrap();
        assert!(!findings.is_empty());
        for f in &findings {
            assert_eq!(f.severity, forge_core::Severity::Strict, "expected strict; got warn for: {}", f.message);
        }
        let _ = fs::remove_dir_all(&root);
    }
}
