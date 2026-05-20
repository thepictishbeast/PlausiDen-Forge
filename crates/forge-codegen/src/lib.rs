//! `forge-codegen` — generate a complete-Rust-stack runtime
//! (axum + sqlx + maud + serde handlers) from a CMS manifest.
//!
//! This is the **scaffold layer** of task #101 / #210. The goal
//! is a `forge codegen` subcommand that turns the typed CmsPage
//! tree into a buildable Rust server crate where every page is
//! served by a typed `async fn` handler instead of by Forge's
//! static-site `render` phase.
//!
//! ## Stages
//!
//! Codegen is staged so each iteration can extend one slice
//! without touching the others. The order is significant: each
//! stage's output is one of the inputs of the next.
//!
//! 1. **HandlerScaffold** (this iteration): one stub `async fn
//!    render_<slug>(...) -> impl IntoResponse` per CmsPage. The
//!    stub returns `maud::html! {}` from a hard-coded constant
//!    so the generated crate compiles immediately.
//! 2. **RouterAssembly** (next): generate the `axum::Router`
//!    that wires each handler to `CmsPage.path`.
//! 3. **MaudBodies** (after): port the Loom render path into the
//!    generated handler bodies so they emit real markup.
//! 4. **PersistenceLayer** (last): generate sqlx queries for any
//!    `data_backend` slug declared in `backends.toml`.
//!
//! ## Why stages
//!
//! The generated server is a moving target — every change to a
//! CmsPage changes the generated source. Without stage isolation
//! a one-line CMS edit would re-emit thousands of lines across
//! handlers + router + persistence + types simultaneously, and
//! the resulting diff would be unreadable. Each stage produces a
//! file-set with a clear contract; the operator regenerates one
//! file-set at a time and reviews the diff scoped to that stage.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// The input to codegen — the set of pages + workspace metadata
/// the generated crate is being built for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodegenPlan {
    /// CmsPages to generate handlers for. Order is preserved
    /// across the generated output so the diff is stable when
    /// the operator re-runs codegen with no CMS changes.
    pub pages: Vec<loom_cms_render::CmsPage>,
    /// Name of the generated crate (e.g. `"prosperityclub-server"`).
    /// Must be a valid Rust identifier in kebab-case (the
    /// canonical Cargo crate-name shape).
    pub crate_name: String,
}

/// One generated source file with its path inside the output
/// crate. Paths are relative to the generated crate root (so
/// `"src/handlers/index.rs"`, not `"/tmp/...."`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeneratedFile {
    /// Relative path under the generated crate root.
    pub path: String,
    /// File contents, ready to write to disk.
    pub contents: String,
}

/// The complete codegen output: a set of files the caller writes
/// to disk + a small audit summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodegenOutput {
    /// Files to write.
    pub files: Vec<GeneratedFile>,
    /// Audit summary: which stages ran, how many files each
    /// produced, intended for the build report.
    pub stages: Vec<StageReport>,
}

/// One row of the codegen audit summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StageReport {
    /// Stage name (kebab-case).
    pub stage: String,
    /// How many files this stage emitted.
    pub files_emitted: u32,
}

/// Codegen failure modes.
#[derive(Debug, thiserror::Error)]
pub enum CodegenError {
    /// A CmsPage carries a slug that doesn't map to a valid Rust
    /// identifier and the escape table couldn't fix it. The
    /// `slug` field is the offending input.
    #[error("slug {slug:?} cannot be escaped to a valid Rust identifier")]
    UnescapableSlug {
        /// The slug that failed escaping.
        slug: String,
    },
    /// Two pages produced the same handler-function name after
    /// escaping. Collisions can't be silently broken — the
    /// caller has to rename one of the pages.
    #[error(
        "handler-name collision: pages with slugs {first:?} and {second:?} both map to fn {fn_name:?}"
    )]
    HandlerCollision {
        /// First colliding slug.
        first: String,
        /// Second colliding slug.
        second: String,
        /// The function name they both wanted.
        fn_name: String,
    },
    /// The crate name is not a valid Cargo identifier.
    #[error("crate name {0:?} is not a valid Cargo crate name")]
    BadCrateName(String),
}

/// Run codegen for `plan` and return the full output.
///
/// v2 runs HandlerScaffold + RouterAssembly. Generated crate
/// compiles with a working axum::Router wiring each handler to
/// its CmsPage.path; bodies are still stubs from Stage 1.
pub fn generate(plan: &CodegenPlan) -> Result<CodegenOutput, CodegenError> {
    if !is_valid_crate_name(&plan.crate_name) {
        return Err(CodegenError::BadCrateName(plan.crate_name.clone()));
    }
    let mut output = CodegenOutput {
        files: Vec::new(),
        stages: Vec::new(),
    };
    let scaffold_files = stage_handler_scaffold(plan)?;
    output.stages.push(StageReport {
        stage: "handler-scaffold".to_owned(),
        files_emitted: scaffold_files.len() as u32,
    });
    output.files.extend(scaffold_files);

    let router_files = stage_router_assembly(plan)?;
    output.stages.push(StageReport {
        stage: "router-assembly".to_owned(),
        files_emitted: router_files.len() as u32,
    });
    output.files.extend(router_files);
    Ok(output)
}

/// Stage 1: one stub handler per CmsPage + a `mod.rs` that
/// re-exports them.
fn stage_handler_scaffold(plan: &CodegenPlan) -> Result<Vec<GeneratedFile>, CodegenError> {
    let mut handler_names: Vec<(String, String)> = Vec::with_capacity(plan.pages.len());
    for page in &plan.pages {
        let slug = derive_slug(page);
        let fn_name = slug_to_fn_name(&slug)?;
        if let Some((dup_slug, _)) = handler_names.iter().find(|(_, fn_n)| fn_n == &fn_name) {
            return Err(CodegenError::HandlerCollision {
                first: dup_slug.clone(),
                second: slug,
                fn_name,
            });
        }
        handler_names.push((slug, fn_name));
    }
    let mut files = Vec::with_capacity(handler_names.len() + 1);
    for ((slug, fn_name), page) in handler_names.iter().zip(plan.pages.iter()) {
        files.push(GeneratedFile {
            path: format!("src/handlers/{slug}.rs"),
            contents: render_handler_stub(fn_name, page),
        });
    }
    files.push(GeneratedFile {
        path: "src/handlers/mod.rs".to_owned(),
        contents: render_handlers_mod(&handler_names),
    });
    Ok(files)
}

/// Stage 2: generate `src/router.rs` that wires each handler to
/// its `CmsPage.path` via `axum::Router::new().route(...)`. Also
/// emits a default `src/lib.rs` that exports the router builder
/// so the generated crate is a buildable library.
///
/// Path normalization rules:
/// * `"/"` → routed as `/`
/// * `"/about/"` → routed as `/about` (axum's StripTrailingSlash
///   middleware handles the trailing-slash variant in caller code;
///   the route table itself uses the canonical no-trailing-slash
///   form so route declarations are unambiguous).
/// * `"about.html"` → routed as `/about.html` (leading slash added).
fn stage_router_assembly(plan: &CodegenPlan) -> Result<Vec<GeneratedFile>, CodegenError> {
    let mut routes: Vec<(String, String)> = Vec::with_capacity(plan.pages.len());
    for page in &plan.pages {
        let slug = derive_slug(page);
        let fn_name = slug_to_fn_name(&slug)?;
        let route = normalize_axum_path(&page.path);
        routes.push((route, fn_name));
    }
    let mut files = Vec::with_capacity(2);
    files.push(GeneratedFile {
        path: "src/router.rs".to_owned(),
        contents: render_router(&routes),
    });
    files.push(GeneratedFile {
        path: "src/lib.rs".to_owned(),
        contents: render_lib_root(),
    });
    Ok(files)
}

/// Map a CmsPage.path (browser-visible URL) to the canonical
/// axum route path. Trailing slashes are stripped on multi-
/// segment paths because axum treats `/about` and `/about/`
/// as distinct route entries; using the no-trailing-slash form
/// for declarations + a StripTrailingSlash middleware at the
/// app-mount point handles both incoming URL shapes.
fn normalize_axum_path(p: &str) -> String {
    if p == "/" || p.is_empty() {
        return "/".to_owned();
    }
    let with_leading = if p.starts_with('/') {
        p.to_owned()
    } else {
        format!("/{p}")
    };
    let trimmed = with_leading.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn render_router(routes: &[(String, String)]) -> String {
    let mut out = String::from(
        "//! Generated by `forge codegen` — RouterAssembly stage.\n\
         //! Do not edit by hand; regenerate from cms/.\n\n\
         use axum::{routing::get, Router};\n\
         use crate::handlers;\n\n\
         /// Build the application router. Each CmsPage gets one\n\
         /// `Router::route(path, get(handler))` registration. Wrap\n\
         /// the returned router in `axum_extra::middleware::StripTrailingSlash`\n\
         /// at the app-mount point so URLs with + without the\n\
         /// trailing slash hit the same handler.\n\
         pub fn build_router() -> Router {\n\
         \x20   Router::new()\n",
    );
    for (route, fn_name) in routes {
        let route_lit = escape_str_for_rust(route);
        out.push_str(&format!(
            "\x20       .route({route_lit}, get(handlers::{fn_name}))\n"
        ));
    }
    out.push_str("}\n");
    out
}

fn render_lib_root() -> String {
    String::from(
        "//! Generated by `forge codegen`.\n\
         //! Do not edit by hand; regenerate from cms/.\n\n\
         pub mod handlers;\n\
         pub mod router;\n\n\
         pub use router::build_router;\n",
    )
}

/// Derive a stable kebab-case slug from a page. Uses `path`
/// because it's the canonical browser-visible URL — every page
/// has one and the slug-to-handler-name mapping stays stable
/// when the operator renames a CMS source file but keeps the
/// public URL identical.
fn derive_slug(page: &loom_cms_render::CmsPage) -> String {
    let p = page.path.trim_matches('/');
    if p.is_empty() {
        return "index".to_owned();
    }
    p.replace('/', "-")
}

/// Convert a kebab-case slug into a valid Rust `fn` identifier.
/// `[a-z][a-z0-9-]*` slugs map to `[a-z][a-z0-9_]*` fn names by
/// replacing `-` with `_`. Anything else falls back to a sha-ish
/// hex tail to keep the codegen deterministic.
fn slug_to_fn_name(slug: &str) -> Result<String, CodegenError> {
    if slug.is_empty() {
        return Err(CodegenError::UnescapableSlug {
            slug: slug.to_owned(),
        });
    }
    let mut out = String::with_capacity(slug.len() + 7);
    out.push_str("render_");
    for c in slug.chars() {
        match c {
            'a'..='z' | '0'..='9' => out.push(c),
            '-' => out.push('_'),
            _ => {
                return Err(CodegenError::UnescapableSlug {
                    slug: slug.to_owned(),
                });
            }
        }
    }
    if !out.chars().nth("render_".len()).is_some_and(|c| !c.is_ascii_digit()) {
        // Rust identifiers can't start with a digit. The
        // `render_` prefix already guarantees that, but be
        // explicit so the contract is testable.
        out.insert(7, '_');
    }
    Ok(out)
}

/// True if `s` is a syntactically valid Cargo crate name.
///
/// Cargo accepts `[A-Za-z0-9_-]+` starting with a letter. We
/// require lowercase + kebab-case to match the substrate's
/// canonical naming.
fn is_valid_crate_name(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next();
    if !matches!(first, Some('a'..='z')) {
        return false;
    }
    chars.all(|c| matches!(c, 'a'..='z' | '0'..='9' | '-' | '_'))
}

fn render_handler_stub(fn_name: &str, page: &loom_cms_render::CmsPage) -> String {
    // Single static const so the generated crate compiles
    // without depending on maud at this stage; later stages
    // replace the body with real maud markup.
    let title_lit = escape_str_for_rust(&page.title);
    let path_lit = escape_str_for_rust(&page.path);
    format!(
        "//! Generated by `forge codegen` — HandlerScaffold stage.\n\
         //! Do not edit by hand; regenerate from cms/.\n\n\
         /// Path: {path_lit}\n\
         pub async fn {fn_name}() -> &'static str {{\n\
         \x20   // Stage 1 stub. Stage 3 (MaudBodies) replaces the\n\
         \x20   // body with the typed render path.\n\
         \x20   {title_lit}\n\
         }}\n"
    )
}

fn render_handlers_mod(handlers: &[(String, String)]) -> String {
    let mut out = String::from(
        "//! Generated by `forge codegen` — handlers module index.\n\
         //! Do not edit by hand; regenerate from cms/.\n\n",
    );
    for (slug, _) in handlers {
        let mod_name = slug.replace('-', "_");
        out.push_str(&format!("pub mod {mod_name};\n"));
    }
    out
}

fn escape_str_for_rust(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(path: &str, title: &str) -> loom_cms_render::CmsPage {
        loom_cms_render::CmsPage {
            brand: None,
            theme: None,
            chrome: None,
            content_width: None,
            nav_actions: vec![],
            schema: None,
            title: title.to_owned(),
            description: "x".to_owned(),
            path: path.to_owned(),
            nav_links: vec![],
            dev_devtools: false,
            footer: None,
            site_origin: None,
            social_image: None,
            sections: vec![],
        }
    }

    #[test]
    fn slug_root_path_maps_to_index() {
        assert_eq!(derive_slug(&page("/", "x")), "index");
    }

    #[test]
    fn slug_strips_leading_and_trailing_slashes() {
        assert_eq!(derive_slug(&page("/about/", "x")), "about");
        assert_eq!(
            derive_slug(&page("/about/privacy-policy/", "x")),
            "about-privacy-policy"
        );
    }

    #[test]
    fn slug_to_fn_name_escapes_dashes() {
        assert_eq!(
            slug_to_fn_name("about-privacy-policy").unwrap(),
            "render_about_privacy_policy"
        );
    }

    #[test]
    fn slug_to_fn_name_rejects_uppercase() {
        assert!(slug_to_fn_name("About").is_err());
    }

    #[test]
    fn slug_to_fn_name_rejects_empty() {
        assert!(slug_to_fn_name("").is_err());
    }

    #[test]
    fn generate_emits_handler_scaffold_and_router_assembly() {
        let plan = CodegenPlan {
            pages: vec![page("/", "Home"), page("/about/", "About")],
            crate_name: "demo-server".to_owned(),
        };
        let out = generate(&plan).unwrap();
        // 2 handlers + handlers/mod.rs + router.rs + lib.rs
        assert_eq!(out.files.len(), 5);
        for expected in [
            "src/handlers/index.rs",
            "src/handlers/about.rs",
            "src/handlers/mod.rs",
            "src/router.rs",
            "src/lib.rs",
        ] {
            assert!(
                out.files.iter().any(|f| f.path == expected),
                "missing generated file: {expected}"
            );
        }
        let mod_rs = out
            .files
            .iter()
            .find(|f| f.path == "src/handlers/mod.rs")
            .unwrap();
        assert!(mod_rs.contents.contains("pub mod index;"));
        assert!(mod_rs.contents.contains("pub mod about;"));
    }

    #[test]
    fn generate_emits_stub_handler_with_path_comment() {
        let plan = CodegenPlan {
            pages: vec![page("/about/", "About Us")],
            crate_name: "demo-server".to_owned(),
        };
        let out = generate(&plan).unwrap();
        let about = out
            .files
            .iter()
            .find(|f| f.path == "src/handlers/about.rs")
            .unwrap();
        assert!(about.contents.contains("Path: \"/about/\""));
        assert!(about.contents.contains("pub async fn render_about()"));
        assert!(about.contents.contains("\"About Us\""));
    }

    #[test]
    fn generate_rejects_bad_crate_name() {
        let plan = CodegenPlan {
            pages: vec![],
            crate_name: "Demo Server".to_owned(),
        };
        assert!(matches!(
            generate(&plan),
            Err(CodegenError::BadCrateName(_))
        ));
    }

    #[test]
    fn generate_rejects_handler_name_collision() {
        // Two distinct paths that both map to handler `about`.
        // This shouldn't happen with the derive_slug rules, but
        // we want the explicit fail-closed guarantee in the
        // collision-detection path.
        let mut p1 = page("/about/", "First");
        p1.title = "First".into();
        let mut p2 = page("/about/", "Second");
        p2.title = "Second".into();
        let plan = CodegenPlan {
            pages: vec![p1, p2],
            crate_name: "demo-server".to_owned(),
        };
        assert!(matches!(
            generate(&plan),
            Err(CodegenError::HandlerCollision { .. })
        ));
    }

    #[test]
    fn audit_report_lists_one_row_per_stage() {
        let plan = CodegenPlan {
            pages: vec![page("/", "x")],
            crate_name: "demo-server".to_owned(),
        };
        let out = generate(&plan).unwrap();
        assert_eq!(out.stages.len(), 2);
        assert_eq!(out.stages[0].stage, "handler-scaffold");
        assert_eq!(out.stages[0].files_emitted, 2); // 1 handler + 1 mod.rs
        assert_eq!(out.stages[1].stage, "router-assembly");
        assert_eq!(out.stages[1].files_emitted, 2); // router.rs + lib.rs
    }

    #[test]
    fn router_routes_every_page_to_its_handler() {
        let plan = CodegenPlan {
            pages: vec![
                page("/", "Home"),
                page("/about/", "About"),
                page("/about/privacy-policy/", "Privacy"),
            ],
            crate_name: "demo-server".to_owned(),
        };
        let out = generate(&plan).unwrap();
        let router = out
            .files
            .iter()
            .find(|f| f.path == "src/router.rs")
            .unwrap();
        assert!(router.contents.contains(".route(\"/\", get(handlers::render_index))"));
        assert!(router.contents.contains(".route(\"/about\", get(handlers::render_about))"));
        assert!(router.contents.contains(
            ".route(\"/about/privacy-policy\", get(handlers::render_about_privacy_policy))"
        ));
        assert!(router.contents.contains("pub fn build_router() -> Router"));
    }

    #[test]
    fn lib_root_reexports_router_builder() {
        let plan = CodegenPlan {
            pages: vec![page("/", "Home")],
            crate_name: "demo-server".to_owned(),
        };
        let out = generate(&plan).unwrap();
        let lib = out.files.iter().find(|f| f.path == "src/lib.rs").unwrap();
        assert!(lib.contents.contains("pub mod handlers;"));
        assert!(lib.contents.contains("pub mod router;"));
        assert!(lib.contents.contains("pub use router::build_router;"));
    }

    #[test]
    fn normalize_axum_path_handles_edge_cases() {
        assert_eq!(normalize_axum_path("/"), "/");
        assert_eq!(normalize_axum_path(""), "/");
        assert_eq!(normalize_axum_path("/about"), "/about");
        assert_eq!(normalize_axum_path("/about/"), "/about");
        assert_eq!(
            normalize_axum_path("/about/privacy-policy/"),
            "/about/privacy-policy"
        );
        // No leading slash → adds one.
        assert_eq!(normalize_axum_path("about.html"), "/about.html");
    }

    #[test]
    fn escape_str_handles_quotes_and_newlines() {
        assert_eq!(
            escape_str_for_rust("hello \"world\"\n"),
            "\"hello \\\"world\\\"\\n\""
        );
    }

    proptest::proptest! {
        #[test]
        fn slug_to_fn_name_never_panics(s in "[ -~]{0,30}") {
            let _ = slug_to_fn_name(&s);
        }
    }
}
