//! `dynamic_runtime` — emit a tiny client-side SPA router in
//! Dynamic / Hybrid build modes. T432 (closes #432).
//!
//! Mode behavior:
//!   * Poc / Production / Static → phase is a no-op (current behavior).
//!   * Dynamic → emit `forge-spa-runtime.js` into `static_dir` and
//!     inject `<script src="/forge-spa-runtime.js" defer></script>`
//!     into every top-level `.html` page (before `</body>`).
//!   * Hybrid → same as Dynamic (pre-rendered pages PLUS the
//!     runtime that swaps content on subsequent navigations).
//!
//! The runtime is intentionally minimal (~70 lines vanilla JS, no
//! deps, no framework, no bundler) and degrades gracefully — if
//! the fetch fails or JS is disabled, the browser does a full
//! page load. CSP-friendly: no `eval`, no inline event handlers.
//!
//! Findings:
//!   * `dynamic_runtime.skipped` (warn) — static mode encountered
//!     but the project's `forge.toml` declared `dynamic = true`
//!     in a future iteration (placeholder for T432 cycle 2).
//!   * `dynamic_runtime.injected` (warn, info-level) — file
//!     already contained a `<script src="/forge-spa-runtime.js"`
//!     tag, idempotent re-run skipped injection.
//!
//! Out of scope (deferred to T432 cycle 2+):
//!   * Route prefetch on link hover.
//!   * Scroll restoration on back/forward.
//!   * Per-route data loaders.
//!   * Service worker for offline.
//!
//! Mirror: none yet (no bash equivalent — net new phase).

use std::fs;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, BuildMode, Finding, Phase};

/// The runtime emitted into `<static_dir>/forge-spa-runtime.js`.
///
/// Hand-written vanilla JS. ~70 lines, ~2.2 KB before gzip. CSP-safe
/// (no `eval`, no inline handlers, no `new Function`). Click-to-swap
/// + History API; falls back to full nav on any error.
const SPA_RUNTIME_JS: &str = r#"// forge-spa-runtime — emitted by forge dynamic_runtime phase (T432).
// CSP-safe: no eval, no Function constructor, no inline handlers.
(function () {
  'use strict';
  if (window.__forgeSpaActive) return;
  window.__forgeSpaActive = true;

  var origin = location.origin;

  function sameOrigin(href) {
    try { return new URL(href, location.href).origin === origin; }
    catch (_) { return false; }
  }

  function isAugmented(e) {
    return e.defaultPrevented || e.button !== 0
        || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey;
  }

  async function swapTo(url, push) {
    try {
      var res = await fetch(url, { credentials: 'same-origin', headers: { 'Accept': 'text/html' } });
      if (!res.ok) throw new Error('forge-spa: HTTP ' + res.status);
      var html = await res.text();
      var doc = new DOMParser().parseFromString(html, 'text/html');
      var nextMain = doc.querySelector('main');
      var curMain = document.querySelector('main');
      if (!nextMain || !curMain) throw new Error('forge-spa: no <main>');
      curMain.replaceWith(nextMain);
      if (doc.title) document.title = doc.title;
      if (push) history.pushState({ forgeSpa: true }, '', url);
      window.scrollTo(0, 0);
      window.dispatchEvent(new CustomEvent('forge:navigated', { detail: { url: url } }));
    } catch (err) {
      // Hard fallback: full page load.
      location.assign(url);
    }
  }

  document.addEventListener('click', function (e) {
    if (isAugmented(e)) return;
    var a = e.target.closest && e.target.closest('a[href]');
    if (!a) return;
    var href = a.getAttribute('href');
    if (!href || href.startsWith('#')) return;
    if (a.target && a.target !== '' && a.target !== '_self') return;
    if (a.hasAttribute('download')) return;
    if (a.getAttribute('rel') && a.getAttribute('rel').indexOf('external') !== -1) return;
    var abs = new URL(href, location.href);
    if (!sameOrigin(abs.href)) return;
    e.preventDefault();
    swapTo(abs.href, true);
  });

  window.addEventListener('popstate', function () {
    swapTo(location.href, false);
  });
})();
"#;

/// Tag injected before `</body>` so the runtime is available on
/// every Dynamic / Hybrid page.
const SCRIPT_TAG: &str =
    r#"<script src="/forge-spa-runtime.js" defer></script>"#;

/// File the runtime is written to (relative to `static_dir`).
const RUNTIME_FILENAME: &str = "forge-spa-runtime.js";

/// `dynamic_runtime` phase.
#[derive(Debug, Default)]
pub struct DynamicRuntimePhase;

impl Phase for DynamicRuntimePhase {
    fn name(&self) -> &'static str {
        "dynamic_runtime"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        if !needs_runtime(ctx.mode) {
            tracing::debug!(target: "forge", "dynamic_runtime: mode={:?}, skipping", ctx.mode);
            return Ok(Vec::new());
        }

        let mut findings = Vec::new();

        // 1. Write the runtime file.
        let runtime_path = ctx.static_dir.join(RUNTIME_FILENAME);
        fs::write(&runtime_path, SPA_RUNTIME_JS).map_err(|e| BuildError::Io {
            context: format!("dynamic_runtime: write {}", runtime_path.display()),
            source: e,
        })?;
        tracing::info!(
            target: "forge",
            "dynamic_runtime: wrote {} ({} bytes)",
            runtime_path.display(),
            SPA_RUNTIME_JS.len()
        );

        // 2. Inject script tag into every top-level .html.
        let entries = match fs::read_dir(&ctx.static_dir) {
            Ok(it) => it,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(findings),
            Err(e) => {
                return Err(BuildError::Io {
                    context: format!(
                        "dynamic_runtime: read_dir {}",
                        ctx.static_dir.display()
                    ),
                    source: e,
                });
            }
        };
        for entry in entries {
            let entry = entry.map_err(|e| BuildError::Io {
                context: format!(
                    "dynamic_runtime: dir entry under {}",
                    ctx.static_dir.display()
                ),
                source: e,
            })?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("html") {
                continue;
            }
            inject_script_tag(&path, &mut findings)?;
        }

        Ok(findings)
    }
}

/// `true` when the active mode wants the SPA runtime emitted.
fn needs_runtime(mode: BuildMode) -> bool {
    matches!(mode, BuildMode::Dynamic | BuildMode::Hybrid)
}

/// Read `path`, insert `SCRIPT_TAG` immediately before `</body>` if
/// not already present, write back. Idempotent.
fn inject_script_tag(path: &Path, findings: &mut Vec<Finding>) -> Result<(), BuildError> {
    let html = fs::read_to_string(path).map_err(|e| BuildError::Io {
        context: format!("dynamic_runtime: read {}", path.display()),
        source: e,
    })?;
    if html.contains(RUNTIME_FILENAME) {
        // Already injected — idempotent re-run.
        return Ok(());
    }
    let Some(idx) = html.rfind("</body>") else {
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_owned();
        findings.push(Finding::warn(
            "dynamic_runtime",
            name,
            format!(
                "no </body> close tag — cannot inject {SCRIPT_TAG}; runtime won't load on this page"
            ),
        ));
        return Ok(());
    };
    let mut out = String::with_capacity(html.len() + SCRIPT_TAG.len() + 1);
    out.push_str(&html[..idx]);
    out.push_str(SCRIPT_TAG);
    out.push('\n');
    out.push_str(&html[idx..]);
    fs::write(path, out).map_err(|e| BuildError::Io {
        context: format!("dynamic_runtime: write {}", path.display()),
        source: e,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir_all, write};
    use tempfile::TempDir;

    fn ctx(static_dir: &Path, mode: BuildMode) -> BuildCtx {
        BuildCtx {
            root: static_dir.parent().unwrap().to_path_buf(),
            static_dir: static_dir.to_path_buf(),
            mode,
        }
    }

    #[test]
    fn static_mode_is_noop() {
        let tmp = TempDir::new().unwrap();
        let sd = tmp.path().join("static");
        create_dir_all(&sd).unwrap();
        write(sd.join("index.html"), "<html><body>hi</body></html>").unwrap();

        let phase = DynamicRuntimePhase;
        let findings = phase.run(&ctx(&sd, BuildMode::Static)).unwrap();

        assert!(findings.is_empty(), "static mode should produce no findings");
        assert!(
            !sd.join(RUNTIME_FILENAME).exists(),
            "static mode must not emit the runtime file"
        );
        let html = std::fs::read_to_string(sd.join("index.html")).unwrap();
        assert!(
            !html.contains(RUNTIME_FILENAME),
            "static mode must not inject the script tag"
        );
    }

    #[test]
    fn dynamic_mode_emits_runtime_and_injects() {
        let tmp = TempDir::new().unwrap();
        let sd = tmp.path().join("static");
        create_dir_all(&sd).unwrap();
        write(
            sd.join("index.html"),
            "<html><body><main>hi</main></body></html>",
        )
        .unwrap();
        write(
            sd.join("about.html"),
            "<html><body><main>about</main></body></html>",
        )
        .unwrap();

        let phase = DynamicRuntimePhase;
        let findings = phase.run(&ctx(&sd, BuildMode::Dynamic)).unwrap();

        assert!(findings.is_empty(), "no findings expected on happy path");
        let rt = std::fs::read(sd.join(RUNTIME_FILENAME)).unwrap();
        assert_eq!(rt, SPA_RUNTIME_JS.as_bytes());

        for name in ["index.html", "about.html"] {
            let html = std::fs::read_to_string(sd.join(name)).unwrap();
            assert!(
                html.contains(SCRIPT_TAG),
                "page {name} should have the script tag injected"
            );
            assert!(
                html.find(SCRIPT_TAG).unwrap() < html.find("</body>").unwrap(),
                "script tag must precede </body> on {name}"
            );
        }
    }

    #[test]
    fn hybrid_mode_behaves_like_dynamic() {
        let tmp = TempDir::new().unwrap();
        let sd = tmp.path().join("static");
        create_dir_all(&sd).unwrap();
        write(sd.join("page.html"), "<html><body></body></html>").unwrap();

        let phase = DynamicRuntimePhase;
        phase.run(&ctx(&sd, BuildMode::Hybrid)).unwrap();

        assert!(sd.join(RUNTIME_FILENAME).exists());
        let html = std::fs::read_to_string(sd.join("page.html")).unwrap();
        assert!(html.contains(SCRIPT_TAG));
    }

    #[test]
    fn injection_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let sd = tmp.path().join("static");
        create_dir_all(&sd).unwrap();
        write(sd.join("p.html"), "<html><body></body></html>").unwrap();

        let phase = DynamicRuntimePhase;
        phase.run(&ctx(&sd, BuildMode::Dynamic)).unwrap();
        phase.run(&ctx(&sd, BuildMode::Dynamic)).unwrap();
        let html = std::fs::read_to_string(sd.join("p.html")).unwrap();
        // Tag should appear exactly once even after two runs.
        assert_eq!(
            html.matches(SCRIPT_TAG).count(),
            1,
            "second run must be a no-op"
        );
    }

    #[test]
    fn no_body_emits_warning() {
        let tmp = TempDir::new().unwrap();
        let sd = tmp.path().join("static");
        create_dir_all(&sd).unwrap();
        write(sd.join("broken.html"), "<html><main>no body</main></html>").unwrap();

        let phase = DynamicRuntimePhase;
        let findings = phase.run(&ctx(&sd, BuildMode::Dynamic)).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].phase, "dynamic_runtime");
        assert!(findings[0].message.contains("no </body>"));
    }

    #[test]
    fn poc_mode_is_noop() {
        let tmp = TempDir::new().unwrap();
        let sd = tmp.path().join("static");
        create_dir_all(&sd).unwrap();
        write(sd.join("i.html"), "<html><body></body></html>").unwrap();

        DynamicRuntimePhase
            .run(&ctx(&sd, BuildMode::Poc))
            .unwrap();
        assert!(!sd.join(RUNTIME_FILENAME).exists());
    }

    #[test]
    fn production_mode_is_noop() {
        let tmp = TempDir::new().unwrap();
        let sd = tmp.path().join("static");
        create_dir_all(&sd).unwrap();
        write(sd.join("i.html"), "<html><body></body></html>").unwrap();

        DynamicRuntimePhase
            .run(&ctx(&sd, BuildMode::Production))
            .unwrap();
        assert!(!sd.join(RUNTIME_FILENAME).exists());
    }

    #[test]
    fn needs_runtime_matrix() {
        assert!(!needs_runtime(BuildMode::Poc));
        assert!(!needs_runtime(BuildMode::Production));
        assert!(!needs_runtime(BuildMode::Static));
        assert!(needs_runtime(BuildMode::Hybrid));
        assert!(needs_runtime(BuildMode::Dynamic));
    }
}
