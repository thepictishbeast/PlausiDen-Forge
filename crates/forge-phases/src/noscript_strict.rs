//! `noscript_strict` — enforce zero-JS rendered HTML.
//!
//! When `forge.toml [noscript_strict] enabled = true`, this
//! phase scans `static/*.html` for any `<script>` tag and fails
//! strict on each. Pairs with `LOOM_NOSCRIPT_MODE=1` (set by the
//! Forge render phase reading the same flag) which causes Loom's
//! `page_shell_themed` to drop THEME_TOGGLE_JS / DEFER_ONLOAD_JS
//! / ERUDA_LOADER_JS at render time.
//!
//! Use cases per `docs/SUBSTRATE_DE_CONSUMER_SHAPING_AUDIT.md`:
//! LibreJS visitors, Tor-strict-mode browsers, archive.org
//! indexing, hunted-tier (#124) builds, anyone who disables JS
//! by policy.
//!
//! ## Heuristic
//!
//! Walks each `static/*.html` looking for `<script` (case-
//! insensitive, with or without attributes). Excludes the
//! `<noscript>` element which contains a fallback `<link>`,
//! not a script. Strict on each hit.

use std::fs;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// `noscript_strict` phase implementation.
#[derive(Debug, Default)]
pub struct NoscriptStrictPhase;

impl Phase for NoscriptStrictPhase {
    fn name(&self) -> &'static str {
        "noscript_strict"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        // Silent skip if the gate isn't configured. Check both
        // env (Forge render sets LOOM_NOSCRIPT_MODE) and
        // forge.toml. Either signal is sufficient — they're
        // meant to fire together.
        let env_on = std::env::var("LOOM_NOSCRIPT_MODE")
            .map(|v| !v.is_empty() && v != "0")
            .unwrap_or(false);
        let toml_on = read_noscript_strict_from_toml(&ctx.root);
        if !env_on && !toml_on {
            return Ok(findings);
        }
        let static_dir = &ctx.static_dir;
        if !static_dir.is_dir() {
            return Ok(findings);
        }
        let entries = fs::read_dir(static_dir).map_err(|e| BuildError::Io {
            context: format!("read_dir {}", static_dir.display()),
            source: e,
        })?;
        for entry in entries {
            let entry = entry.map_err(|e| BuildError::Io {
                context: format!("read_dir entry in {}", static_dir.display()),
                source: e,
            })?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("html") {
                continue;
            }
            let raw = fs::read_to_string(&path).map_err(|e| BuildError::Io {
                context: format!("read {}", path.display()),
                source: e,
            })?;
            for (line_no, hit) in find_script_tags(&raw) {
                findings.push(Finding::strict(
                    self.name(),
                    path.display().to_string(),
                    format!(
                        "noscript_strict — `<script>` tag at line {line_no}: {hit}. forge.toml [noscript_strict] enabled = true forbids any inline-script / src-script in rendered HTML. Ensure LOOM_NOSCRIPT_MODE=1 was set during render so Loom dropped its bootstraps."
                    ),
                ));
            }
        }
        Ok(findings)
    }
}

fn read_noscript_strict_from_toml(root: &std::path::Path) -> bool {
    let cfg_path = root.join("forge.toml");
    let Ok(body) = fs::read_to_string(&cfg_path) else {
        return false;
    };
    let mut in_section = false;
    for raw in body.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            in_section = line == "[noscript_strict]";
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some(rest) = line.strip_prefix("enabled") {
            let v = rest.trim_start().trim_start_matches('=').trim().to_lowercase();
            return v == "true" || v == "1" || v == "\"true\"";
        }
    }
    false
}

/// Find every `<script` tag in the body. Returns (line_no, first
/// 80 chars of the offending line). Case-insensitive. Skips
/// matches inside `<noscript>` blocks.
fn find_script_tags(body: &str) -> Vec<(usize, String)> {
    let mut hits = Vec::new();
    let lower = body.to_lowercase();
    let mut in_noscript_depth: i32 = 0;
    for (idx, line) in body.lines().enumerate() {
        let l = lower
            .lines()
            .nth(idx)
            .map(|s| s.to_owned())
            .unwrap_or_default();
        // Walk the line character by character, tracking
        // noscript-block depth as we go. Any `<script` outside a
        // noscript block is a hit; `<script` inside a noscript
        // block is skipped (intentional literal text in the
        // fallback markup).
        let bytes = l.as_bytes();
        let mut i = 0;
        let mut local_in_noscript = in_noscript_depth;
        let mut script_hit_in_this_line = false;
        while i < bytes.len() {
            if l[i..].starts_with("<noscript") {
                local_in_noscript += 1;
                i += "<noscript".len();
                continue;
            }
            if l[i..].starts_with("</noscript") {
                local_in_noscript = (local_in_noscript - 1).max(0);
                i += "</noscript".len();
                continue;
            }
            if local_in_noscript == 0 && l[i..].starts_with("<script") {
                script_hit_in_this_line = true;
                // Step past this token so we don't re-flag at i+1.
                i += "<script".len();
                continue;
            }
            i += 1;
        }
        in_noscript_depth = local_in_noscript;
        if script_hit_in_this_line {
            let snippet: String = line.chars().take(80).collect();
            hits.push((idx + 1, snippet));
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_inline_script() {
        let html = r#"<!DOCTYPE html>
<html><head><title>T</title></head>
<body><script>alert(1)</script></body></html>"#;
        let hits = find_script_tags(html);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 3);
    }

    #[test]
    fn finds_src_script() {
        let html = r#"<html><body>
<script src="/x.js"></script>
</body></html>"#;
        let hits = find_script_tags(html);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn case_insensitive() {
        let html = "<HTML><BODY><SCRIPT>x</SCRIPT></BODY></HTML>";
        let hits = find_script_tags(html);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn ignores_script_inside_noscript_block() {
        // The Loom defer-stylesheet pattern emits a noscript
        // fallback that contains a <link>, not a <script>, but
        // catch-the-fallback should still apply in principle.
        let html = r#"<html><body>
<noscript><script>this is just a literal</script></noscript>
</body></html>"#;
        let hits = find_script_tags(html);
        assert!(
            hits.is_empty(),
            "expected zero hits inside noscript block, got: {hits:?}"
        );
    }

    #[test]
    fn empty_body_no_hits() {
        assert!(find_script_tags("").is_empty());
    }

    #[test]
    fn toml_reader_returns_true_when_enabled() {
        let dir = std::env::temp_dir().join(format!(
            "forge-noscript-strict-test-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&dir);
        let toml_path = dir.join("forge.toml");
        fs::write(&toml_path, "[noscript_strict]\nenabled = true\n").unwrap();
        assert!(read_noscript_strict_from_toml(&dir));
        let _ = fs::remove_file(&toml_path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn toml_reader_returns_false_when_missing() {
        let dir = std::env::temp_dir().join(format!(
            "forge-noscript-strict-test-missing-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&dir);
        assert!(!read_noscript_strict_from_toml(&dir));
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn toml_reader_returns_false_when_other_section() {
        let dir = std::env::temp_dir().join(format!(
            "forge-noscript-strict-test-other-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&dir);
        let toml_path = dir.join("forge.toml");
        fs::write(&toml_path, "[other]\nenabled = true\n").unwrap();
        assert!(!read_noscript_strict_from_toml(&dir));
        let _ = fs::remove_file(&toml_path);
        let _ = fs::remove_dir(&dir);
    }
}
