//! `render` — Forge generates content directly from CMS sources
//! using `loom-cms-render` IN-PROCESS, not via shelling out to
//! `loom cms-render`.
//!
//! Owner directive 2026-05-13: "the cms should actually
//! generate content and forge uses it so it should actually
//! generate content."
//!
//! Before this phase landed, Forge was an audit pipeline only —
//! it inspected ALREADY-rendered HTML in `static/`. The bash
//! `forge.sh` (now deprecated) shelled out to `loom cms-render`
//! before invoking the audit phases. With T70, that subprocess
//! shell-out becomes a Rust function call.
//!
//! ## What this phase does
//!
//! 1. Walks `<root>/cms/*.json` files (one per page).
//! 2. Parses each as a typed `loom_cms_render::CmsPage` via
//!    serde — `deny_unknown_fields` catches schema drift before
//!    any HTML is emitted.
//! 3. Calls `loom_cms_render::render_page(&page)` to get the
//!    body markup.
//! 4. Wraps it in a minimal HTML5 shell with `<html lang="en">`,
//!    `<meta name="color-scheme" content="light dark">`, and
//!    `<main id="content">` (the user's a11y + dual-theme
//!    defaults from PlausiDen-Loom T48c v1).
//! 5. Writes the output to `<static_dir>/_render/<slug>.html`
//!    via atomic write (temp + rename, POSIX-atomic on the
//!    same filesystem).
//!
//! v1 outputs to `static/_render/` so it doesn't fight whatever
//! is already in `static/`. T70c flips `static/` to be the
//! canonical output once parity is verified.
//!
//! ## Opt-in
//!
//! v1 only runs when a `cms/` directory exists under `ctx.root`.
//! Sites that don't use the typed CMS (legacy hand-written HTML)
//! pass cleanly with zero findings.
//!
//! ## Doctrine applied
//!
//! * **Type-safe end-to-end** — JSON parse → `CmsPage` → markup,
//!   no string-templating step where escaping could be missed.
//! * **Atomic writes** — same primitive Loom's WriteCapability
//!   uses; no torn writes on power loss.
//! * **No `unwrap`/`expect`** in lib code; lint enforces.
//! * **Schema-validated boundary** — `serde_json::from_str`
//!   fails closed with a phase finding, not a panic.
//!
//! AVP-PASS-T70: 2026-05-13.

use std::path::{Path, PathBuf};

use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// `render` phase implementation.
#[derive(Debug, Default)]
pub struct RenderPhase;

impl Phase for RenderPhase {
    fn name(&self) -> &'static str {
        "render"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let cms_dir = ctx.root.join("cms");
        if !cms_dir.is_dir() {
            // Sites without a cms/ tree pass through silently —
            // they're using the legacy hand-written HTML flow.
            return Ok(Vec::new());
        }

        // Issue #8 fix (2026-05-20): read + parse forge.toml ONCE
        // per render call, then pull every field out of the cached
        // value. The previous code did `1 + N` disk reads and `1 + N`
        // TOML parses for N pages via per-field wrapper helpers,
        // instead of one of each. We now share the parsed value
        // across `extract_render_write_canonical`, `extract_theme`,
        // and any further extractors that land.
        let forge_toml = parse_forge_toml(&ctx.root);

        // T70c (2026-05-14): per-site opt-in to write rendered HTML
        // directly to <static_dir>/<slug>.html instead of the
        // sibling <static_dir>/_render/<slug>.html. The default
        // stays false for backwards compatibility — sites can flip
        // it once they've verified parity with their existing
        // hand-rendered HTML or pre-built static set.
        //
        //   forge.toml:
        //     [render]
        //     write_canonical = true
        //
        // Closes the workflow gap surfaced by the 2026-05-14
        // stock-template dogfood loop: a CMS or Loom edit was producing
        // updated HTML in `_render/` while `static/` (which the
        // dev server actually serves) stayed stale. With
        // write_canonical=true, `forge build` rebuilds the served
        // pages in one step.
        let write_canonical = extract_render_write_canonical(forge_toml.as_ref());
        let out_dir = if write_canonical {
            ctx.static_dir.clone()
        } else {
            ctx.static_dir.join("_render")
        };
        if let Err(e) = std::fs::create_dir_all(&out_dir) {
            return Err(BuildError::Io {
                context: format!("render mkdir {}", out_dir.display()),
                source: e,
            });
        }

        let json_files = collect_cms_jsons(&cms_dir)?;
        let mut findings = Vec::new();
        let mut rendered_count = 0usize;
        let mut rendered_slugs: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        // Pre-scan the slug set so output_path_for_slug can route
        // "parent" slugs to `<slug>/index.html` rather than
        // `<slug>.html`. A slug is "parent" when another slug in the
        // same build nests under it via the `--` convention (e.g.
        // `about` is parent of `about--privacy-policy`). The
        // browser-visible URL of a parent page is the directory
        // form (`/about/`) which only resolves to the nested
        // index.html — emitting the flat sibling at `<slug>.html`
        // would leave `/about/` 404ing despite the slug existing.
        let all_slugs: std::collections::HashSet<String> = json_files
            .iter()
            .filter_map(|p| slug_from_filename(p))
            .collect();
        let parent_slugs: std::collections::HashSet<String> = all_slugs
            .iter()
            .filter_map(|s| s.split_once("--").map(|(parent, _)| parent.to_owned()))
            .filter(|p| all_slugs.contains(p))
            .collect();

        // Skin bytes + two derived hashes, computed once per build:
        //   - `raw_hash_b64` → goes inside the SYNC-FROM-LOOM marker
        //     that gets prepended to the skin file (loom_sync gate
        //     expects this to match sha384(loom-tokens/src/skin.css)).
        //   - `served_hash_b64` → integrity attr on every <link>;
        //     hashes the bytes the browser actually downloads, i.e.
        //     marker + raw.
        // Hoisted above the per-page loop so each rendered HTML can
        // inject the integrity attribute, AND the post-loop skin.css
        // write can use the same prepared buffer instead of recomputing.
        use base64::Engine as _;
        use sha2::Digest as _;
        let skin_raw = loom_tokens::SKIN_CSS.as_bytes();
        let raw_hash_b64 = {
            let mut h = sha2::Sha384::new();
            h.update(skin_raw);
            base64::engine::general_purpose::STANDARD.encode(h.finalize())
        };
        let skin_marker = format!(
            "/* SYNC-FROM-LOOM:sha384-{raw_hash_b64} — auto-synced by forge render at build */\n"
        );
        let skin_with_marker: Vec<u8> = {
            let mut buf = Vec::with_capacity(skin_marker.len() + skin_raw.len());
            buf.extend_from_slice(skin_marker.as_bytes());
            buf.extend_from_slice(skin_raw);
            buf
        };
        let served_hash_b64 = {
            let mut h = sha2::Sha384::new();
            h.update(&skin_with_marker);
            base64::engine::general_purpose::STANDARD.encode(h.finalize())
        };
        let skin_integrity_attr = format!("sha384-{served_hash_b64}");

        for json_path in json_files {
            let slug = match slug_from_filename(&json_path) {
                Some(s) => s,
                None => {
                    findings.push(Finding::strict(
                        self.name(),
                        json_path.display().to_string(),
                        "filename is not a valid slug — must match [a-z][a-z0-9-]*".to_owned(),
                    ));
                    continue;
                }
            };
            let raw = match std::fs::read_to_string(&json_path) {
                Ok(s) => s,
                Err(e) => {
                    return Err(BuildError::Io {
                        context: format!("render read {}", json_path.display()),
                        source: e,
                    });
                }
            };
            let mut page: loom_cms_render::CmsPage = match serde_json::from_str(&raw) {
                Ok(p) => p,
                Err(e) => {
                    // Schema drift — fail closed at the boundary.
                    // Strict-severity so the build blocks until
                    // the JSON is corrected.
                    findings.push(Finding::strict(
                        self.name(),
                        json_path.display().to_string(),
                        format!("CmsPage parse failed: {e}"),
                    ));
                    continue;
                }
            };

            // #322 (paul 2026-05-21): apply per-tenant
            // `{{ VAR }}` / `@asset-slug` substitution BEFORE the
            // render path. Tenants ship variables.json /
            // palette.json / assets-map.json at the project root;
            // forge_core::tenant_variables::load merges them into
            // a typed TenantVariables, then loom_cms_render
            // projects every CmsBlock/CmsSection string leaf
            // through the substitution table.
            //
            // Fail-tolerant: ANY error here (load failure, JSON
            // conversion miss) leaves `page` untouched.
            if let Some(tv) = forge_core::tenant_variables::TenantVariables::load(&ctx.root) {
                if let Some(substituted) = apply_tenant_variables(&page, &tv) {
                    page = substituted;
                }
            }

            // T70b: call Loom's full a11y / dual-theme page-shell
            // directly. Forge inherits the same WCAG-AA contrast
            // tokens, focus-visible outlines, skip-link styling,
            // and `prefers-reduced-motion` honour as Loom-rendered
            // sites — single source of truth in the render layer.
            //
            // T37 v3.b (2026-05-14): theme resolution order:
            //   1. FORGE_THEME env var (highest priority — CI override)
            //   2. forge.toml `[render] theme = "..."` entry (the
            //      `loom site init --theme` baked default)
            //   3. None → fall back to OS prefers-color-scheme
            //
            // Closed allow-list ("light"|"dark") at every layer.
            let env_theme = std::env::var("FORGE_THEME").ok();
            let toml_theme = extract_render_theme(forge_toml.as_ref());
            // Per-page theme on CmsPage wins over env / forge.toml.
            // Falls back to LOOM_THEME env, then forge.toml [theme].
            // The allowlist is wider than light/dark — page_shell_themed
            // re-validates against the closed enum, so we just need
            // some value to pass through.
            let theme_owned = page.theme.clone().or(env_theme).or(toml_theme);
            let theme_ref = theme_owned.as_deref().filter(|t| {
                matches!(
                    *t,
                    "light"
                        | "dark"
                        | "dark-amoled"
                        | "auto"
                        | "warm"
                        | "ocean"
                        | "forest"
                        | "violet"
                        | "rose"
                        | "sepia"
                        | "press"
                        | "hc-dark"
                        | "hc-light"
                )
            });
            // FORGE_DEV_DEVTOOLS=1 flips the CmsPage dev_devtools flag
            // on every page in the build. Loom's page_shell_themed
            // then drops the strict CSP and inlines the
            // localStorage-gated Eruda loader. Strictly dev-only —
            // never set this env on the prod build path.
            if std::env::var("FORGE_DEV_DEVTOOLS").is_ok() {
                page.dev_devtools = true;
            }
            let body_markup = loom_cms_render::render_page(&page).into_string();
            let html_raw = loom_cms_render::page_shell_themed(
                &page,
                "/loom-skin.css",
                &body_markup,
                None,
                theme_ref,
            );

            // Inject SRI integrity attribute on the loom-skin.css
            // link. Hash was precomputed above over the SAME bytes
            // (marker + raw) the post-loop skin.css writer ships,
            // so the browser's SRI check matches what we serve.
            let html = inject_skin_integrity(&html_raw, &skin_integrity_attr);

            // Per-tenant [style] overrides — inject after </head>
            // so the tenant CSS variables override the substrate
            // baseline. Tenants without [style] pass through (the
            // load() is fail-tolerant and returns None on every
            // error path including missing-section).
            let html = inject_tenant_style(&html, &ctx.root);

            let out_path = output_path_for_slug(&out_dir, &slug, &parent_slugs);
            if let Some(parent) = out_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return Err(BuildError::Io {
                        context: format!("render mkdir {}", parent.display()),
                        source: e,
                    });
                }
            }
            if let Err(e) = atomic_write(&out_path, html.as_bytes()) {
                return Err(BuildError::Io {
                    context: format!("render write {}", out_path.display()),
                    source: e,
                });
            }
            rendered_count += 1;
            rendered_slugs.insert(slug.clone());
        }

        // Orphan detection (issue surfaced 2026-05-20): when a
        // CMS source is deleted, the canonical static/<slug>.html
        // lingers and the audit phases that follow (sri / tokens /
        // perf_budget / unbuilt_route) re-scan it, producing N
        // warns per stale file. Surface one finding per orphan
        // here so the operator sees the root cause once instead of
        // chasing the symptoms across phases. Pure detection;
        // no deletion. Only relevant when write_canonical=true —
        // off-canonical builds write to _render/ and `static/`
        // is not the substrate's territory.
        if write_canonical {
            if let Ok(entries) = std::fs::read_dir(&ctx.static_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if !p.is_file() {
                        continue;
                    }
                    if p.extension().and_then(|s| s.to_str()) != Some("html") {
                        continue;
                    }
                    let stem = match p.file_stem().and_then(|s| s.to_str()) {
                        Some(s) => s.to_owned(),
                        None => continue,
                    };
                    if rendered_slugs.contains(&stem) {
                        continue;
                    }
                    findings.push(
                        Finding::warn(
                            self.name(),
                            p.display().to_string(),
                            format!(
                                "static/{stem}.html has no cms/{stem}.json source — stale artifact"
                            ),
                        )
                        .why(
                            "the canonical render set is cms/*.json; when a CMS source is \
                             deleted the previously-rendered static/<slug>.html lingers and \
                             downstream audit phases (sri/tokens/perf_budget/unbuilt_route) \
                             re-scan it, producing N derived warns per stale file",
                        )
                        .fix(format!(
                            "either: (a) delete static/{stem}.html if the page is intentionally \
                             gone, OR (b) restore cms/{stem}.json if the page should still ship"
                        ))
                        .skill("author-cms-content")
                        .avoid(
                            "don't manually edit static/<slug>.html — Forge regenerates it from \
                             cms/<slug>.json on every build with write_canonical=true",
                        ),
                    );
                }
            }
        }

        // T69 (cycle 96 iter 13): write the canonical loom-skin.css
        // bytes alongside the rendered HTML. Previous behaviour
        // emitted <slug>.html without the design-system CSS,
        // forcing operators to manually `cp` skin.css after every
        // build. Now Forge ships current CSS bytes automatically.
        //
        // REGRESSION-GUARD: atomic write so a half-written CSS
        // never serves to a live page. Same atomic_write helper
        // the HTML pages use. Failures surface as BuildError::Io
        // (no silent skip).
        //
        // Issue #8 fix (2026-05-20): skip the write when the
        // existing file already matches the current SKIN_CSS bytes.
        // Avoids one disk-fsync per build when the design system
        // hasn't changed, and matters when Forge runs on a watch
        // loop or in a sandbox with rate-limited writes.
        // Prepend the SYNC-FROM-LOOM marker line so loom_sync gate
        // sees the freshly-written bytes as authoritatively synced.
        // The gate computes sha384(LOOM_PATH/loom-tokens/src/skin.css)
        // and compares to the marker — since Forge embeds the same
        // SKIN_CSS bytes via the loom-tokens crate dep, the hash
        // computed over `skin_bytes` here matches Loom's source. No
        // separate `forge --sync-loom` needed; every `forge build`
        // produces a properly-marked file. (Surfaced via loom_sync
        // gate finding "no SYNC-FROM-LOOM marker" on every build.)
        let skin_path = ctx.static_dir.join("loom-skin.css");
        let needs_write = match std::fs::read(&skin_path) {
            Ok(existing) => existing != skin_with_marker,
            Err(_) => true, // missing / unreadable → write
        };
        if needs_write {
            if let Err(e) = atomic_write(&skin_path, &skin_with_marker) {
                return Err(BuildError::Io {
                    context: format!("render write {}", skin_path.display()),
                    source: e,
                });
            }
        }

        tracing::info!(
            target: "forge_phases::render",
            rendered = rendered_count,
            skin_bytes = loom_tokens::SKIN_CSS.len(),
            "phase_render generated {rendered_count} HTML page(s) from cms/*.json + loom-skin.css"
        );

        Ok(findings)
    }
}

/// Walk `cms/` for `*.json` files (one level only — sub-pages
/// are not supported in v1; pages map 1:1 to top-level JSON
/// files).
fn collect_cms_jsons(cms_dir: &Path) -> Result<Vec<PathBuf>, BuildError> {
    let entries = match std::fs::read_dir(cms_dir) {
        Ok(e) => e,
        Err(e) => {
            return Err(BuildError::Io {
                context: format!("render read_dir {}", cms_dir.display()),
                source: e,
            });
        }
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                return Err(BuildError::Io {
                    context: format!("render iter {}", cms_dir.display()),
                    source: e,
                });
            }
        };
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
            // Skip the `$schema` companion doc if present.
            if path.file_name().and_then(|s| s.to_str()) == Some("cms-schema.json") {
                continue;
            }
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

/// Extract a slug from a filename like `home.json` → `Some("home")`.
/// Returns None if the stem doesn't match `[a-z][a-z0-9-]*`.
fn slug_from_filename(path: &Path) -> Option<String> {
    let stem = path.file_stem().and_then(|s| s.to_str())?;
    let mut chars = stem.chars();
    let first = chars.next()?;
    if !first.is_ascii_lowercase() {
        return None;
    }
    if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return None;
    }
    if stem.len() > 80 {
        return None;
    }
    Some(stem.to_owned())
}

// T70b: `wrap_html_shell` and `escape_text` removed — replaced
// by direct calls into `loom_cms_render::page_shell` and
// `loom_cms_render::escape_html_text`. Keeps the a11y / dual-
// theme contract co-located with the renderer that owns it.

/// Read + parse `<root>/forge.toml` once. Returns `None` if the
/// file is absent or malformed. Issue #8 fix: callers extract
/// every field they need from this single parsed value rather
/// than re-reading the file per-field.
fn parse_forge_toml(root: &Path) -> Option<toml::Value> {
    let path = root.join("forge.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    content.parse::<toml::Value>().ok()
}

/// Extract `[render] theme = "..."` from an already-parsed
/// forge.toml value. Closed allow-list — anything other than
/// `light` / `dark` is treated as absent.
fn extract_render_theme(toml: Option<&toml::Value>) -> Option<String> {
    let theme = toml?
        .get("render")
        .and_then(|r| r.get("theme"))
        .and_then(|t| t.as_str())?;
    match theme {
        "light" | "dark" => Some(theme.to_owned()),
        _ => None,
    }
}

/// Extract `[render] write_canonical = true` from an already-
/// parsed forge.toml value. Anything other than literal `true`
/// (missing key, missing section, non-bool value) returns false.
fn extract_render_write_canonical(toml: Option<&toml::Value>) -> bool {
    toml.and_then(|t| t.get("render"))
        .and_then(|r| r.get("write_canonical"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// T37 v3.b: read `[render] theme = "..."` from `<root>/forge.toml`.
/// Returns `Some("light"|"dark")` on a valid entry; `None` for any
/// other case (file missing, parse error, missing section, unknown
/// theme value). Closed allow-list — anything other than the two
/// canonical values is treated as absent.
///
/// Retained for unit-test ergonomics. The live render path uses
/// [`parse_forge_toml`] + [`extract_render_theme`] to share one
/// parse across all field extractions. `#[cfg(test)]`-gated so
/// release builds don't carry the wrapper.
#[cfg(test)]
fn forge_toml_theme(root: &Path) -> Option<String> {
    extract_render_theme(parse_forge_toml(root).as_ref())
}

/// T70c (2026-05-14): read `[render] write_canonical = true` from
/// `<root>/forge.toml`. Returns false for any non-true value
/// (missing file, parse error, key absent, key set to anything
/// other than the literal boolean `true`).
///
/// Defence in depth: a typo'd or deliberately-malformed
/// Atomic write: tmp file + rename. POSIX guarantees rename is
/// atomic on the same filesystem.
/// Map a slug to its output `.html` path under `out_dir`.
///
/// Convention: a slug containing `--` is treated as a nested-URL
/// path. Each `--` becomes a `/` and the page is written to
/// `out_dir/<a>/<b>/.../<n>/index.html`. A flat slug retains the
/// original `out_dir/<slug>.html` shape — UNLESS another slug in
/// the same build nests under it (`about--privacy-policy` makes
/// `about` a parent slug), in which case the flat slug emits to
/// `out_dir/<slug>/index.html` instead. That ensures URLs like
/// `/about/` resolve to a page when the site links to them.
///
/// Examples (assume `parent_slugs = {"about"}`):
/// * `index`                       → `out_dir/index.html`
/// * `about`                       → `out_dir/about/index.html` (parent)
/// * `contact`                     → `out_dir/contact.html` (not parent)
/// * `about--privacy-policy`       → `out_dir/about/privacy-policy/index.html`
/// * `legal--terms--us`            → `out_dir/legal/terms/us/index.html`
///
/// Why a sentinel rather than allowing literal `/` in the slug:
/// the slug grammar (`[a-z][a-z0-9-]*`) was the contract for
/// `loom site init --route` and for every audit phase that joins
/// the slug to a file path. Keeping that grammar intact and using
/// `--` as an explicit nest marker means existing code keeps
/// working; the only place `--` is interpreted is here.
fn output_path_for_slug(
    out_dir: &Path,
    slug: &str,
    parent_slugs: &std::collections::HashSet<String>,
) -> PathBuf {
    if !slug.contains("--") {
        if parent_slugs.contains(slug) {
            return out_dir.join(slug).join("index.html");
        }
        return out_dir.join(format!("{slug}.html"));
    }
    let mut p = out_dir.to_path_buf();
    for segment in slug.split("--") {
        p.push(segment);
    }
    p.push("index.html");
    p
}

/// Inject `integrity="sha384-..." crossorigin="anonymous"` onto
/// the loom-skin.css `<link rel="stylesheet">` tag in a rendered
/// HTML page. Idempotent: if `integrity=` is already present the
/// html is returned unchanged. Returns an owned `String` either way.
///
/// Forge-side post-processing instead of a Loom page_shell change:
/// avoids stacking a 4th open Loom PR; the sri gate closes without
/// waiting on Loom merge. When Loom's page_shell accepts a
/// `skin_integrity: Option<String>` param, this helper is deleted.
fn inject_skin_integrity(html: &str, integrity_attr: &str) -> String {
    const NEEDLE: &str = "<link rel=\"stylesheet\" href=\"/loom-skin.css\">";
    if !html.contains(NEEDLE) {
        return html.to_owned();
    }
    let replacement = format!(
        "<link rel=\"stylesheet\" href=\"/loom-skin.css\" integrity=\"{integrity_attr}\" crossorigin=\"anonymous\">",
    );
    html.replacen(NEEDLE, &replacement, 1)
}

/// Bridge between `forge_core::tenant_variables::TenantVariables`
/// and `loom_cms_render::apply_variables`. The two types are
/// shape-compatible (three sibling `BTreeMap<String,String>`
/// fields with identical JSON wire format) so we round-trip
/// through `serde_json::Value` to convert without compile-time
/// coupling forge-core to loom-variables.
///
/// Returns `None` on any conversion error — the caller leaves
/// the page untouched (placeholders preserved verbatim per
/// substitute() contract).
fn apply_tenant_variables(
    page: &loom_cms_render::CmsPage,
    tv: &forge_core::tenant_variables::TenantVariables,
) -> Option<loom_cms_render::CmsPage> {
    let tv_json = serde_json::to_value(tv).ok()?;
    let loom_tv: loom_cms_render::TenantVariables = serde_json::from_value(tv_json).ok()?;
    loom_cms_render::apply_variables(page, &loom_tv).ok()
}

/// Inject the tenant's `[style]` CSS overrides into the rendered
/// page-shell head. Pass-through when the tenant declares no
/// `[style]` section in `forge.toml`. Injected AFTER the
/// `loom-skin.css` link so source-order cascade lets tenant
/// values override substrate baselines.
fn inject_tenant_style(html: &str, root: &Path) -> String {
    let Some(style) = forge_core::tenant_style::TenantStyle::load(root) else {
        return html.to_owned();
    };
    let style_tag = style.to_style_tag();
    if style_tag.is_empty() {
        return html.to_owned();
    }
    if !html.contains("</head>") {
        // Unexpected shape; leave untouched rather than risk a
        // malformed document.
        return html.to_owned();
    }
    html.replacen("</head>", &format!("{style_tag}</head>"), 1)
}

fn atomic_write(dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = dest.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "dest has no parent")
    })?;
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_name = format!(
        ".{}.tmp.{pid}.{nanos}",
        dest.file_name().and_then(|s| s.to_str()).unwrap_or("out")
    );
    let tmp = parent.join(tmp_name);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx_with_cms(pages: &[(&str, &str)]) -> (BuildCtx, std::path::PathBuf) {
        let tmp = std::env::temp_dir().join(format!(
            "render-t70-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(tmp.join("cms")).expect("mk cms");
        std::fs::create_dir_all(tmp.join("static")).expect("mk static");
        for (name, body) in pages {
            std::fs::write(tmp.join("cms").join(format!("{name}.json")), body).expect("write json");
        }
        let ctx = BuildCtx {
            root: tmp.clone(),
            static_dir: tmp.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        (ctx, tmp)
    }

    fn sample_page(title: &str) -> String {
        serde_json::to_string(&serde_json::json!({
            "title": title,
            "description": "test",
            "path": "/",
            "sections": [
                {"kind": "heading", "level": 2, "text": "Hello"},
                {"kind": "paragraph", "text": "World."}
            ]
        }))
        .expect("serialise sample")
    }

    #[test]
    fn render_passes_through_when_no_cms_dir() {
        let tmp = std::env::temp_dir().join(format!("render-t70-empty-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join("static")).expect("mk");
        let ctx = BuildCtx {
            root: tmp.clone(),
            static_dir: tmp.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = RenderPhase.run(&ctx).expect("run");
        assert!(findings.is_empty(), "no cms dir = no findings");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn render_emits_html_for_each_cms_json() {
        let (ctx, tmp) = make_ctx_with_cms(&[
            ("home", &sample_page("Home")),
            ("about", &sample_page("About")),
        ]);
        let findings = RenderPhase.run(&ctx).expect("run");
        assert!(findings.is_empty(), "expected clean run, got {findings:?}");
        let home =
            std::fs::read_to_string(tmp.join("static/_render/home.html")).expect("home.html");
        let about =
            std::fs::read_to_string(tmp.join("static/_render/about.html")).expect("about.html");
        // Title from CMS landed in <title>.
        assert!(home.contains("<title>Home</title>"));
        assert!(about.contains("<title>About</title>"));
        // The actual rendered body markup from loom-cms-render
        // includes the heading text and paragraph.
        assert!(home.contains("Hello"), "missing heading: {home}");
        assert!(home.contains("World."), "missing paragraph");
        // a11y defaults from loom_cms_render::page_shell (T70b).
        assert!(home.contains("<main id=\"content\">"));
        assert!(home.contains("color-scheme"));
        assert!(home.contains("lang=\"en\""));
        // Dual-theme + reduced-motion + skip-link styling
        // inherited from loom-cms-render's BASE_THEME_CSS:
        assert!(
            home.contains("prefers-color-scheme:dark"),
            "T70b: phase_render must inherit Loom's dark-mode CSS"
        );
        assert!(
            home.contains("prefers-reduced-motion:reduce"),
            "T70b: phase_render must honour reduced-motion"
        );
        assert!(
            home.contains(".loom-skip:focus"),
            "T70b: phase_render must surface the skip link on focus"
        );
        // CSP inline-style hash present (no unsafe-inline).
        assert!(home.contains("sha256-"), "base-theme must be CSP-pinned");
        assert!(
            !home.contains("'unsafe-inline'"),
            "page_shell must never grant unsafe-inline"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn render_emits_strict_finding_on_schema_drift() {
        let (ctx, tmp) = make_ctx_with_cms(&[(
            "broken",
            r#"{"title":"X","description":"D","path":"/","sections":[{"kind":"paragraph","body":"WRONG FIELD NAME"}]}"#,
        )]);
        let findings = RenderPhase.run(&ctx).expect("run");
        assert_eq!(findings.len(), 1);
        assert!(
            findings[0].message.contains("CmsPage parse failed"),
            "wrong message: {}",
            findings[0].message
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn render_skips_invalid_slug_filenames() {
        // Filename has uppercase / dot — not a valid slug.
        let (ctx, tmp) = make_ctx_with_cms(&[("good", &sample_page("OK"))]);
        std::fs::write(ctx.root.join("cms/Bad.Name.json"), sample_page("Bad")).expect("write bad");
        let findings = RenderPhase.run(&ctx).expect("run");
        assert_eq!(findings.len(), 1, "one finding for the bad filename");
        assert!(
            findings[0].message.contains("not a valid slug"),
            "wrong message: {}",
            findings[0].message
        );
        // The good slug still rendered.
        assert!(tmp.join("static/_render/good.html").is_file());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn render_writes_atomically_and_overwrites_existing() {
        let (ctx, tmp) = make_ctx_with_cms(&[("home", &sample_page("v1"))]);
        // First run writes v1.
        let _ = RenderPhase.run(&ctx).expect("run v1");
        let v1 = std::fs::read_to_string(tmp.join("static/_render/home.html")).expect("v1 file");
        assert!(v1.contains("<title>v1</title>"));
        // Update CMS, second run overwrites.
        std::fs::write(tmp.join("cms/home.json"), sample_page("v2")).expect("update");
        let _ = RenderPhase.run(&ctx).expect("run v2");
        let v2 = std::fs::read_to_string(tmp.join("static/_render/home.html")).expect("v2 file");
        assert!(v2.contains("<title>v2</title>"));
        assert!(!v2.contains("<title>v1</title>"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    // T70c (2026-05-14): write_canonical opt-in.
    #[test]
    fn render_writes_to_underscore_render_by_default() {
        let (ctx, tmp) = make_ctx_with_cms(&[("home", &sample_page("Home"))]);
        // No forge.toml present → default behaviour.
        let _ = RenderPhase.run(&ctx).expect("run");
        assert!(
            tmp.join("static/_render/home.html").is_file(),
            "default writes to _render/"
        );
        assert!(
            !tmp.join("static/home.html").is_file(),
            "default must NOT write to static/<slug>"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn render_writes_to_static_when_write_canonical_true() {
        let (ctx, tmp) = make_ctx_with_cms(&[("home", &sample_page("Home"))]);
        std::fs::write(tmp.join("forge.toml"), "[render]\nwrite_canonical = true\n")
            .expect("write forge.toml");
        let _ = RenderPhase.run(&ctx).expect("run");
        assert!(
            tmp.join("static/home.html").is_file(),
            "write_canonical=true must write to static/<slug>"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn render_detects_orphan_static_html_when_write_canonical_true() {
        // Set up: cms/home.json exists; static/about.html lingers
        // from a previous build whose cms/about.json was deleted.
        let (ctx, tmp) = make_ctx_with_cms(&[("home", &sample_page("Home"))]);
        std::fs::write(tmp.join("forge.toml"), "[render]\nwrite_canonical = true\n")
            .expect("write forge.toml");
        std::fs::write(
            tmp.join("static/about.html"),
            "<!doctype html><html><body>stale orphan</body></html>",
        )
        .expect("seed orphan");

        let findings = RenderPhase.run(&ctx).expect("run");

        // The orphan must surface as exactly one render-phase warn
        // citing the about slug; the rendered home page must NOT
        // appear in the orphan list.
        let orphan_findings: Vec<&Finding> = findings
            .iter()
            .filter(|f| f.message.contains("stale artifact"))
            .collect();
        assert_eq!(
            orphan_findings.len(),
            1,
            "expected exactly one orphan warn, got {}: {:?}",
            orphan_findings.len(),
            findings
        );
        assert!(
            orphan_findings[0].message.contains("about"),
            "orphan finding must cite the about slug: {:?}",
            orphan_findings[0].message
        );
        assert_eq!(orphan_findings[0].severity, forge_core::Severity::Warn);
        // Substrate advocacy doctrine: every render-phase finding
        // carries why/fix/skill/avoid alongside the message.
        let adv = &orphan_findings[0].advocacy;
        assert!(!adv.why.is_empty(), "orphan finding must carry .why()");
        assert!(
            adv.substrate_fix.contains("delete") || adv.substrate_fix.contains("restore"),
            ".fix() must name the substrate-correct action: {:?}",
            adv.substrate_fix
        );
        assert_eq!(adv.skill.as_deref(), Some("author-cms-content"));
        assert!(
            adv.anti_pattern.is_some(),
            "orphan finding must carry .avoid() naming the wrong path"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn render_orphan_detection_silent_when_write_canonical_false() {
        // off-canonical builds write to _render/; static/ is not
        // the substrate's territory in that mode and orphan
        // findings would be false-positive.
        let (ctx, tmp) = make_ctx_with_cms(&[("home", &sample_page("Home"))]);
        std::fs::write(tmp.join("forge.toml"), "").expect("write forge.toml");
        // Pre-create static/ with an orphan-shaped file.
        std::fs::create_dir_all(tmp.join("static")).expect("mkdir");
        std::fs::write(
            tmp.join("static/about.html"),
            "<!doctype html><html><body>not our problem</body></html>",
        )
        .expect("seed");
        let findings = RenderPhase.run(&ctx).expect("run");
        let orphan_findings: Vec<&Finding> = findings
            .iter()
            .filter(|f| f.message.contains("stale artifact"))
            .collect();
        assert!(
            orphan_findings.is_empty(),
            "off-canonical builds must NOT emit orphan warns: {orphan_findings:?}"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn render_falls_back_safely_on_malformed_forge_toml() {
        let (ctx, tmp) = make_ctx_with_cms(&[("home", &sample_page("Home"))]);
        std::fs::write(tmp.join("forge.toml"), "this is = not [valid toml{").expect("write");
        let _ = RenderPhase.run(&ctx).expect("run");
        // Defence-in-depth: malformed config falls back to the
        // safe default (write to _render/, leave static/ alone).
        assert!(tmp.join("static/_render/home.html").is_file());
        assert!(!tmp.join("static/home.html").is_file());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn render_write_canonical_false_or_missing_uses_underscore_render() {
        for forge_toml in &[
            "",                                      // empty
            "[render]\n",                            // section but no key
            "[render]\nwrite_canonical = false\n",   // explicit false
            "[render]\nwrite_canonical = \"yes\"\n", // wrong type
        ] {
            let (ctx, tmp) = make_ctx_with_cms(&[("home", &sample_page("Home"))]);
            std::fs::write(tmp.join("forge.toml"), forge_toml).expect("write");
            let _ = RenderPhase.run(&ctx).expect("run");
            assert!(
                tmp.join("static/_render/home.html").is_file(),
                "default-bound case '{forge_toml}' must write to _render/"
            );
            assert!(
                !tmp.join("static/home.html").is_file(),
                "default-bound case '{forge_toml}' must NOT write to static/<slug>"
            );
            let _ = std::fs::remove_dir_all(&tmp);
        }
    }

    #[test]
    fn slug_from_filename_validates() {
        assert_eq!(
            slug_from_filename(Path::new("home.json")),
            Some("home".to_owned())
        );
        assert_eq!(
            slug_from_filename(Path::new("about-us.json")),
            Some("about-us".to_owned())
        );
        assert_eq!(
            slug_from_filename(Path::new("page-1.json")),
            Some("page-1".to_owned())
        );
        assert_eq!(slug_from_filename(Path::new("Home.json")), None);
        assert_eq!(slug_from_filename(Path::new("1home.json")), None);
        assert_eq!(slug_from_filename(Path::new("home page.json")), None);
        // file_stem() drops the directory prefix; the caller
        // (collect_cms_jsons) doesn't recurse, so a sub-dir
        // entry never reaches this validator.
        assert_eq!(slug_from_filename(Path::new(".hidden.json")), None);
    }

    #[test]
    fn collect_cms_jsons_skips_schema_companion() {
        let (ctx, tmp) = make_ctx_with_cms(&[("home", &sample_page("Home"))]);
        std::fs::write(
            ctx.root.join("cms/cms-schema.json"),
            r#"{"$schema": "http://json-schema.org/draft-07/schema#"}"#,
        )
        .expect("write schema");
        let jsons = collect_cms_jsons(&ctx.root.join("cms")).expect("collect");
        let names: Vec<_> = jsons
            .iter()
            .filter_map(|p| p.file_name().and_then(|s| s.to_str()).map(str::to_owned))
            .collect();
        assert!(names.contains(&"home.json".to_owned()));
        assert!(!names.contains(&"cms-schema.json".to_owned()));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ---- T37 v3.b: forge.toml [render] theme reading ----

    fn tmpdir_for(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "forge-render-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn forge_toml_theme_reads_dark() {
        let tmp = tmpdir_for("dark");
        std::fs::create_dir_all(&tmp).expect("mk");
        std::fs::write(
            tmp.join("forge.toml"),
            "mode = \"poc\"\n[render]\ntheme = \"dark\"\n",
        )
        .expect("write");
        assert_eq!(forge_toml_theme(&tmp), Some("dark".to_owned()));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn forge_toml_theme_reads_light() {
        let tmp = tmpdir_for("light");
        std::fs::create_dir_all(&tmp).expect("mk");
        std::fs::write(tmp.join("forge.toml"), "[render]\ntheme = \"light\"\n").expect("write");
        assert_eq!(forge_toml_theme(&tmp), Some("light".to_owned()));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn forge_toml_theme_returns_none_when_missing() {
        let tmp = tmpdir_for("none");
        std::fs::create_dir_all(&tmp).expect("mk");
        // No forge.toml at all.
        assert_eq!(forge_toml_theme(&tmp), None);
        // forge.toml exists but no [render] section.
        std::fs::write(tmp.join("forge.toml"), "mode = \"poc\"\n").expect("write");
        assert_eq!(forge_toml_theme(&tmp), None);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn forge_toml_theme_drops_unknown_values() {
        let tmp = tmpdir_for("evil");
        std::fs::create_dir_all(&tmp).expect("mk");
        for hostile in ["evil", "DARK", "auto", "high-contrast"] {
            std::fs::write(
                tmp.join("forge.toml"),
                format!("[render]\ntheme = \"{hostile}\"\n"),
            )
            .expect("write");
            assert_eq!(
                forge_toml_theme(&tmp),
                None,
                "hostile value `{hostile}` must be dropped"
            );
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn forge_toml_theme_returns_none_on_parse_error() {
        let tmp = tmpdir_for("garbage");
        std::fs::create_dir_all(&tmp).expect("mk");
        std::fs::write(tmp.join("forge.toml"), "this is = not valid toml [[[ ").expect("write");
        assert_eq!(forge_toml_theme(&tmp), None);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn output_path_flat_slug_stays_flat() {
        let out_dir = Path::new("/tmp/out");
        let no_parents = std::collections::HashSet::new();
        let p = output_path_for_slug(out_dir, "index", &no_parents);
        assert_eq!(p, Path::new("/tmp/out/index.html"));
        let p = output_path_for_slug(out_dir, "contact", &no_parents);
        assert_eq!(p, Path::new("/tmp/out/contact.html"));
    }

    #[test]
    fn output_path_double_dash_nests_into_index_html() {
        let out_dir = Path::new("/tmp/out");
        let no_parents = std::collections::HashSet::new();
        let p = output_path_for_slug(out_dir, "about--privacy-policy", &no_parents);
        assert_eq!(p, Path::new("/tmp/out/about/privacy-policy/index.html"));
        let p = output_path_for_slug(out_dir, "legal--terms--us", &no_parents);
        assert_eq!(p, Path::new("/tmp/out/legal/terms/us/index.html"));
    }

    #[test]
    fn output_path_parent_slug_emits_to_directory_index() {
        // When `about--privacy-policy.json` exists in the same build,
        // a sibling `about.json` is the page that serves at /about/
        // — must emit to about/index.html, not about.html.
        let out_dir = Path::new("/tmp/out");
        let mut parents = std::collections::HashSet::new();
        parents.insert("about".to_owned());
        let p = output_path_for_slug(out_dir, "about", &parents);
        assert_eq!(p, Path::new("/tmp/out/about/index.html"));
        // Non-parent flat slugs stay flat even with a populated set.
        let p = output_path_for_slug(out_dir, "contact", &parents);
        assert_eq!(p, Path::new("/tmp/out/contact.html"));
    }

    #[test]
    fn apply_tenant_variables_substitutes_in_block_text() {
        // Generic placeholder fixture — substrate tests never
        // hardcode real tenant names per the substrate-discipline
        // rule (no client/site/operator names in substrate Rust).
        use std::collections::BTreeMap;
        let mut variables = BTreeMap::new();
        variables.insert("BRAND".into(), "Acme".into());
        let tv = forge_core::tenant_variables::TenantVariables {
            variables,
            palette: BTreeMap::new(),
            assets: BTreeMap::new(),
        };
        let page: loom_cms_render::CmsPage = serde_json::from_str(
            r#"{
                "brand": null, "theme": null, "chrome": null,
                "content_width": null, "nav_actions": [],
                "title": "Welcome to {{ BRAND }}",
                "description": "About {{ BRAND }}",
                "path": "/p", "nav_links": [], "dev_devtools": false,
                "sections": [
                    { "kind": "compose", "blocks": [
                        { "kind": "text", "text": "Made by {{ BRAND }}." }
                    ]}
                ]
            }"#,
        )
        .expect("page parses");
        let substituted = apply_tenant_variables(&page, &tv).expect("round-trip");
        assert_eq!(substituted.title, "Welcome to Acme");
        assert_eq!(substituted.description, "About Acme");
        let html = loom_cms_render::render_page(&substituted).into_string();
        assert!(html.contains("Made by Acme."));
        assert!(!html.contains("{{ BRAND }}"));
    }

    #[test]
    fn inject_skin_integrity_replaces_link_once() {
        let html = "<head><link rel=\"stylesheet\" href=\"/loom-skin.css\"></head>";
        let out = inject_skin_integrity(html, "sha384-AAAA");
        assert!(out.contains("integrity=\"sha384-AAAA\""));
        assert!(out.contains("crossorigin=\"anonymous\""));
        // Exactly one occurrence of the original needle remains as the
        // base href portion; verify no duplicated link tag.
        assert_eq!(out.matches("<link rel=\"stylesheet\"").count(), 1);
    }

    #[test]
    fn inject_skin_integrity_passes_through_when_needle_absent() {
        let html = "<head></head>";
        let out = inject_skin_integrity(html, "sha384-AAAA");
        assert_eq!(out, html);
    }

    #[test]
    fn inject_tenant_style_injects_when_style_section_present() {
        let dir = std::env::temp_dir().join(format!(
            "forge-phases-tenant-style-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("forge.toml"),
            "[style.palette]\nprimary = \"#733635\"\n",
        )
        .unwrap();
        let html = "<head><title>x</title></head><body></body>";
        let out = inject_tenant_style(html, &dir);
        assert!(out.contains("data-loom-tenant-style"));
        assert!(out.contains("--loom-color-primary: #733635;"));
        // Style tag lands BEFORE </head> so the cascade after
        // loom-skin.css applies.
        let idx_style = out.find("data-loom-tenant-style").unwrap();
        let idx_close = out.find("</head>").unwrap();
        assert!(idx_style < idx_close);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inject_tenant_style_passes_through_when_no_forge_toml() {
        let dir = std::env::temp_dir().join(format!(
            "forge-phases-tenant-style-empty-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let html = "<head></head>";
        let out = inject_tenant_style(html, &dir);
        assert_eq!(out, html);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inject_tenant_style_passes_through_when_style_section_empty() {
        let dir = std::env::temp_dir().join(format!(
            "forge-phases-tenant-style-empty-section-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("forge.toml"), "[forge]\nmode = \"poc\"\n").unwrap();
        let html = "<head></head>";
        let out = inject_tenant_style(html, &dir);
        assert_eq!(out, html);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn render_emits_integrity_attribute_on_canonical_build() {
        // Sanity check that the per-page injection lands in real
        // rendered HTML. Uses write_canonical=true so the output
        // hits <static>/<slug>.html — same path the sri gate scans.
        let (ctx, tmp) = make_ctx_with_cms(&[("index", &sample_page("SRI smoke test"))]);
        std::fs::write(tmp.join("forge.toml"), "[render]\nwrite_canonical = true\n")
            .expect("write toml");
        let findings = RenderPhase.run(&ctx).expect("run");
        assert!(
            findings
                .iter()
                .all(|f| !matches!(f.severity, forge_core::Severity::Strict)),
            "render must not emit strict findings on a valid sample page: {findings:?}"
        );
        let html =
            std::fs::read_to_string(tmp.join("static/index.html")).expect("read rendered html");
        assert!(
            html.contains("integrity=\"sha384-"),
            "rendered page missing integrity attr — sri gate would warn"
        );
        assert!(
            html.contains("crossorigin=\"anonymous\""),
            "rendered page missing crossorigin attr"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
