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
//! 1. **HandlerScaffold + MaudBodies (merged)**: one `async fn
//!    render_<slug>() -> Html<String>` per CmsPage. The body
//!    embeds the CmsPage as a JSON literal, parses it once via
//!    OnceLock, and renders through `loom_cms_render::render_page`
//!    + `page_shell_themed` so the markup matches what Forge's
//!    static-render path emits.
//! 2. **RouterAssembly**: generate the `axum::Router` that wires
//!    each handler to `CmsPage.path`.
//! 3. **CrateManifest** (next): emit `Cargo.toml` for the
//!    generated crate so `cargo build` works without operator-
//!    written manifest scaffolding.
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
    /// Name of the generated crate (e.g. `"acme-server"`).
    /// Must be a valid Rust identifier in kebab-case (the
    /// canonical Cargo crate-name shape).
    pub crate_name: String,
    /// Backends declared in `backends.toml` for this site. The
    /// PersistenceLayer stage emits one async stub per backend
    /// that the operator fills in with a real sqlx query. Empty
    /// vec → no `src/db/` tree emitted.
    #[serde(default)]
    pub backends: Vec<BackendSpec>,
}

/// One backend entry from `backends.toml`. Mirrors what
/// `backend_coverage` phase parses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendSpec {
    /// Slug from the `[backends.<name>]` header. Kebab-case
    /// matching `[a-z][a-z0-9-]*`.
    pub name: String,
    /// HTTP method declared in the entry (`"GET"`, `"POST"`,
    /// etc). Free-form string; the generated stub doesn't
    /// actually dispatch on it yet.
    pub method: String,
    /// URL path the backend maps to.
    pub path: String,
    /// Operator-facing purpose string (free text).
    pub purpose: String,
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
    /// Serialization of a CmsPage to JSON failed during codegen.
    /// In practice this only fires on non-JSON-representable
    /// values somewhere in the page tree (e.g. NaN float).
    #[error("serialize CmsPage for slug {slug:?}: {message}")]
    SerializePage {
        /// Slug of the page whose serialization failed.
        slug: String,
        /// serde_json error message.
        message: String,
    },
}

/// Run codegen for `plan` and return the full output.
///
/// v4 runs all four stages: HandlerScaffold + RouterAssembly +
/// CrateManifest + PersistenceLayer. Generated output is a
/// self-contained Cargo crate that `cargo build` directly,
/// with one async stub per backend declared in `backends.toml`
/// ready for the operator to fill in with a real sqlx query.
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

    let manifest_files = stage_crate_manifest(plan);
    output.stages.push(StageReport {
        stage: "crate-manifest".to_owned(),
        files_emitted: manifest_files.len() as u32,
    });
    output.files.extend(manifest_files);

    let persistence_files = stage_persistence_layer(plan)?;
    output.stages.push(StageReport {
        stage: "persistence-layer".to_owned(),
        files_emitted: persistence_files.len() as u32,
    });
    output.files.extend(persistence_files);

    let smoke_files = stage_smoke_tests(plan)?;
    output.stages.push(StageReport {
        stage: "smoke-tests".to_owned(),
        files_emitted: smoke_files.len() as u32,
    });
    output.files.extend(smoke_files);
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
        let mod_name = slug.replace('-', "_");
        files.push(GeneratedFile {
            // File name MUST match the mod name (snake_case) or
            // Rust won't find the module from `pub mod <name>;`.
            path: format!("src/handlers/{mod_name}.rs"),
            contents: render_handler_stub(fn_name, page)?,
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
        contents: render_lib_root(!plan.backends.is_empty()),
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

fn render_lib_root(include_db: bool) -> String {
    let mut out = String::from(
        "//! Generated by `forge codegen`.\n\
         //! Do not edit by hand; regenerate from cms/.\n\n\
         pub mod handlers;\n\
         pub mod router;\n",
    );
    if include_db {
        out.push_str("pub mod db;\n");
    }
    out.push_str("\npub use router::build_router;\n");
    out
}

/// Stage 3: emit `Cargo.toml` + `src/main.rs` so the generated
/// output is a self-contained Cargo crate that builds without
/// any operator-written scaffolding.
///
/// Dep pinning policy: pull loom-cms-render from the upstream
/// git ref (the workspace [patch] table in the consuming
/// project redirects to a local checkout during development).
/// axum + tokio + serde versions match the canonical-stack
/// pinning per RECOMMENDED_LOOP_PREAMBLE.md.
fn stage_crate_manifest(plan: &CodegenPlan) -> Vec<GeneratedFile> {
    vec![
        GeneratedFile {
            path: "Cargo.toml".to_owned(),
            contents: render_cargo_toml(&plan.crate_name, !plan.backends.is_empty()),
        },
        GeneratedFile {
            path: "src/main.rs".to_owned(),
            contents: render_main_rs(&plan.crate_name),
        },
    ]
}

/// Cargo turns package name `demo-server` into library identifier
/// `demo_server` (kebab → snake). Apply the same rule here so
/// `main.rs` can name the lib without operator intervention.
fn package_to_lib_ident(s: &str) -> String {
    s.replace('-', "_")
}

fn render_cargo_toml(crate_name: &str, include_sqlx: bool) -> String {
    // Crate-name is already validated by is_valid_crate_name
    // before this stage runs, so the escape is moot — but
    // round-trip through escape_str_for_rust anyway for the
    // single source of "Rust-string-escaping" truth.
    let name_lit = escape_str_for_rust(crate_name);
    let sqlx_dep = if include_sqlx {
        "sqlx = { version = \"0.8\", default-features = false, features = [\"runtime-tokio\", \"postgres\", \"macros\", \"json\"] }\n"
    } else {
        ""
    };
    format!(
        "# Generated by `forge codegen` — CrateManifest stage.\n\
         # Do not edit by hand; regenerate from cms/.\n\
         #\n\
         # axum + tokio + serde versions track the canonical\n\
         # Forge/Loom stack. Bump these in the codegen template,\n\
         # not in the generated output.\n\n\
         [package]\n\
         name = {name_lit}\n\
         version = \"0.1.0\"\n\
         edition = \"2021\"\n\
         publish = false\n\n\
         [dependencies]\n\
         axum = {{ version = \"0.7\", default-features = false, features = [\"http1\", \"tokio\"] }}\n\
         tokio = {{ version = \"1\", features = [\"macros\", \"rt-multi-thread\", \"net\"] }}\n\
         serde = {{ version = \"1\", features = [\"derive\"] }}\n\
         serde_json = \"1\"\n\
         loom-cms-render = {{ git = \"https://github.com/thepictishbeast/PlausiDen-Loom.git\", branch = \"main\" }}\n\
         {sqlx_dep}\n\
         [dev-dependencies]\n\
         tower = {{ version = \"0.5\", features = [\"util\"] }}\n\
         http-body-util = \"0.1\"\n\n\
         [lints.rust]\n\
         unsafe_code = \"forbid\"\n"
    )
}

/// Stage 4: emit `src/db/<backend>.rs` + `src/db/mod.rs` per
/// declared backend. Each stub is an async function that
/// returns `Result<T, sqlx::Error>` and currently fails closed
/// with `sqlx::Error::Protocol("not implemented yet")` so the
/// generated crate compiles + runs but a real call surfaces an
/// honest "not wired" error rather than a panic.
///
/// No-op when `plan.backends` is empty — the substrate doesn't
/// emit a `src/db/` tree on sites that don't declare any.
fn stage_persistence_layer(plan: &CodegenPlan) -> Result<Vec<GeneratedFile>, CodegenError> {
    if plan.backends.is_empty() {
        return Ok(Vec::new());
    }
    let mut stub_names: Vec<(String, String)> = Vec::with_capacity(plan.backends.len());
    for backend in &plan.backends {
        // Reuse the slug→fn-name escaper to keep identifier
        // discipline consistent with the handler side.
        let fn_name = slug_to_fn_name(&backend.name)?;
        // Strip the "render_" prefix that slug_to_fn_name adds;
        // backends emit as `query_<slug>` not `render_<slug>`.
        let stripped = fn_name
            .strip_prefix("render_")
            .unwrap_or(&fn_name)
            .to_owned();
        let final_name = format!("query_{stripped}");
        stub_names.push((backend.name.clone(), final_name));
    }
    let mut files = Vec::with_capacity(stub_names.len() + 1);
    for ((slug, fn_name), backend) in stub_names.iter().zip(plan.backends.iter()) {
        let mod_name = slug.replace('-', "_");
        files.push(GeneratedFile {
            path: format!("src/db/{mod_name}.rs"),
            contents: render_backend_stub(fn_name, backend),
        });
    }
    files.push(GeneratedFile {
        path: "src/db/mod.rs".to_owned(),
        contents: render_db_mod(&stub_names),
    });
    Ok(files)
}

fn render_backend_stub(fn_name: &str, backend: &BackendSpec) -> String {
    let method_lit = escape_str_for_rust(&backend.method);
    let path_lit = escape_str_for_rust(&backend.path);
    let purpose_lit = escape_str_for_rust(&backend.purpose);
    format!(
        "//! Generated by `forge codegen` — PersistenceLayer stage.\n\
         //! Do not edit by hand; regenerate from backends.toml.\n\
         //!\n\
         //! Backend: {fn_name}\n\
         //!   method:  {method_lit}\n\
         //!   path:    {path_lit}\n\
         //!   purpose: {purpose_lit}\n\n\
         use sqlx::PgPool;\n\n\
         /// Fail-closed stub. Replace the `Err(...)` body with the\n\
         /// real sqlx query when the schema lands. The signature\n\
         /// is the contract — the operator only edits the body.\n\
         pub async fn {fn_name}(_pool: &PgPool) -> Result<serde_json::Value, sqlx::Error> {{\n\
         \x20   Err(sqlx::Error::Protocol(\n\
         \x20       \"backend stub not implemented — fill in src/db/ from backends.toml\".to_owned()\n\
         \x20   ))\n\
         }}\n"
    )
}

fn render_db_mod(stubs: &[(String, String)]) -> String {
    let mut out = String::from(
        "//! Generated by `forge codegen` — db module index.\n\
         //! Do not edit by hand; regenerate from backends.toml.\n\n",
    );
    for (slug, _) in stubs {
        let mod_name = slug.replace('-', "_");
        out.push_str(&format!("pub mod {mod_name};\n"));
    }
    // Flatten each submodule's query fn into `db::<fn>` so call
    // sites don't have to know the submodule name.
    out.push('\n');
    for (slug, fn_name) in stubs {
        let mod_name = slug.replace('-', "_");
        out.push_str(&format!("pub use {mod_name}::{fn_name};\n"));
    }
    out
}

/// Stage 5: emit `tests/smoke.rs` containing one
/// `#[tokio::test]` per CmsPage that boots the generated
/// router via `tower::ServiceExt::oneshot` (no socket bind)
/// and asserts each route returns HTTP 200 with a non-empty
/// body.
///
/// Self-verifying generated crate: `cargo test` on the output
/// directory proves every handler the codegen claims to
/// produce actually responds.
///
/// Adds tower + http-body-util to the generated crate's
/// dev-deps via render_cargo_toml — see that function for the
/// dev-dep pinning.
fn stage_smoke_tests(plan: &CodegenPlan) -> Result<Vec<GeneratedFile>, CodegenError> {
    if plan.pages.is_empty() {
        return Ok(Vec::new());
    }
    let lib_ident = package_to_lib_ident(&plan.crate_name);
    let mut body = String::from(
        "//! Generated by `forge codegen` — TestScaffold stage.\n\
         //! Do not edit by hand; regenerate from cms/.\n\n\
         use axum::body::Body;\n\
         use axum::http::{Request, StatusCode};\n\
         use http_body_util::BodyExt;\n\
         use tower::ServiceExt;\n\n",
    );
    for page in &plan.pages {
        let slug = derive_slug(page);
        let fn_name = slug_to_fn_name(&slug)?;
        let route = normalize_axum_path(&page.path);
        let test_name = format!(
            "smoke_{}",
            fn_name.strip_prefix("render_").unwrap_or(&fn_name)
        );
        let route_lit = escape_str_for_rust(&route);
        body.push_str(&format!(
            "#[tokio::test]\n\
             async fn {test_name}() {{\n\
             \x20   let app = {lib_ident}::build_router();\n\
             \x20   let req = Request::builder()\n\
             \x20       .uri({route_lit})\n\
             \x20       .body(Body::empty())\n\
             \x20       .unwrap();\n\
             \x20   let resp = app.oneshot(req).await.unwrap();\n\
             \x20   let status = resp.status();\n\
             \x20   let body = resp.into_body().collect().await.unwrap().to_bytes();\n\
             \x20   assert_eq!(status, StatusCode::OK,\n\
             \x20       \"non-200: {{}}\", status);\n\
             \x20   assert!(!body.is_empty(), \"empty body\");\n\
             }}\n\n"
        ));
    }
    Ok(vec![GeneratedFile {
        path: "tests/smoke.rs".to_owned(),
        contents: body,
    }])
}

fn render_main_rs(crate_name: &str) -> String {
    let lib_ident = package_to_lib_ident(crate_name);
    format!(
        "//! Generated by `forge codegen` — CrateManifest stage.\n\
         //! Binary entrypoint; mounts build_router() on an axum\n\
         //! server bound to $FORGE_LISTEN (default 127.0.0.1:8080).\n\n\
         use std::env;\n\
         use std::net::SocketAddr;\n\n\
         #[tokio::main]\n\
         async fn main() -> Result<(), Box<dyn std::error::Error>> {{\n\
         \x20   let addr: SocketAddr = env::var(\"FORGE_LISTEN\")\n\
         \x20       .unwrap_or_else(|_| \"127.0.0.1:8080\".to_owned())\n\
         \x20       .parse()?;\n\
         \x20   let app = {lib_ident}::build_router();\n\
         \x20   let listener = tokio::net::TcpListener::bind(addr).await?;\n\
         \x20   eprintln!(\"{lib_ident} listening on http://{{addr}}\");\n\
         \x20   axum::serve(listener, app).await?;\n\
         \x20   Ok(())\n\
         }}\n",
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
    // Strip a single `.html` suffix so a path like `/anthropic.html`
    // and a path like `/anthropic/` produce the same slug shape.
    // Multi-suffix files (`.html.gz`, etc) are out of scope.
    let p = p.strip_suffix(".html").unwrap_or(p);
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
    if !out
        .chars()
        .nth("render_".len())
        .is_some_and(|c| !c.is_ascii_digit())
    {
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

fn render_handler_stub(
    fn_name: &str,
    page: &loom_cms_render::CmsPage,
) -> Result<String, CodegenError> {
    let path_lit = escape_str_for_rust(&page.path);
    // Round-trip the CmsPage through serde_json so the embedded
    // payload is the canonical wire shape (deny_unknown_fields
    // round-trip clean). Serialization can fail in principle
    // (e.g. on a NaN float somewhere downstream), but the
    // CmsPage tree contains no float fields today — so this
    // realistically never errors. Surface the error rather
    // than panicking to keep the generator total.
    let page_json = serde_json::to_string(page).map_err(|e| CodegenError::SerializePage {
        slug: page.path.clone(),
        message: e.to_string(),
    })?;
    // Raw-string the JSON with a pound-padding that won't
    // collide with the payload's own #" sequences. Scan the
    // JSON for the longest run of `"` followed by `#` and
    // pick one more `#` than that.
    let padding = "#".repeat(longest_hash_run_after_quote(&page_json) + 1);
    Ok(format!(
        "//! Generated by `forge codegen` — handlers stage (with maud bodies).\n\
         //! Do not edit by hand; regenerate from cms/.\n\n\
         use axum::response::Html;\n\
         use std::sync::OnceLock;\n\n\
         /// Path: {path_lit}\n\
         ///\n\
         /// CmsPage payload embedded as JSON; parsed once on first\n\
         /// hit via OnceLock so cold-start cost is amortized. Failures\n\
         /// at parse time can only happen if the generated file was\n\
         /// hand-edited (which the doc-comment forbids).\n\
         const PAGE_JSON: &str = r{padding}\"{page_json}\"{padding};\n\n\
         fn page() -> &'static loom_cms_render::CmsPage {{\n\
         \x20   static CELL: OnceLock<loom_cms_render::CmsPage> = OnceLock::new();\n\
         \x20   #[allow(clippy::expect_used)]\n\
         \x20   CELL.get_or_init(|| serde_json::from_str(PAGE_JSON)\n\
         \x20       .expect(\"generated CmsPage JSON must parse\"))\n\
         }}\n\n\
         pub async fn {fn_name}() -> Html<String> {{\n\
         \x20   let p = page();\n\
         \x20   let body = loom_cms_render::render_page(p).into_string();\n\
         \x20   Html(loom_cms_render::page_shell_themed(\n\
         \x20       p,\n\
         \x20       \"/loom-skin.css\",\n\
         \x20       &body,\n\
         \x20       None,\n\
         \x20       p.theme.as_deref(),\n\
         \x20   ))\n\
         }}\n"
    ))
}

/// Return the length of the longest `#` run that follows a
/// quote in `s`. Used to pick a raw-string padding for embedding
/// arbitrary content (incl. JSON containing `#"` sequences).
fn longest_hash_run_after_quote(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut max_run = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let mut run = 0;
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] == b'#' {
                run += 1;
                j += 1;
            }
            if run > max_run {
                max_run = run;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    max_run
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
    // Flatten each submodule's handler fn into `handlers::<fn>`
    // so the generated router can call them without naming the
    // submodule. The router emits `handlers::render_<slug>()`;
    // without these re-exports that resolves to E0425.
    out.push('\n');
    for (_, fn_name) in handlers {
        let mod_name = fn_name
            .strip_prefix("render_")
            .map(|s| s.to_owned())
            .unwrap_or_else(|| fn_name.clone());
        out.push_str(&format!("pub use {mod_name}::{fn_name};\n"));
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

    fn backend(name: &str) -> BackendSpec {
        BackendSpec {
            name: name.to_owned(),
            method: "GET".to_owned(),
            path: format!("/api/{name}"),
            purpose: format!("Test backend: {name}"),
        }
    }

    fn page(path: &str, title: &str) -> loom_cms_render::CmsPage {
        loom_cms_render::CmsPage {
            brand: None,
            brand_logo: None,
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
    fn slug_strips_html_suffix() {
        // `/anthropic.html` and `/anthropic/` both produce
        // the same handler slug — the codegen layer treats
        // file-extension URLs and directory-style URLs as
        // referring to the same page.
        assert_eq!(derive_slug(&page("/anthropic.html", "x")), "anthropic");
        assert_eq!(derive_slug(&page("/anthropic/", "x")), "anthropic");
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
    fn generate_emits_self_contained_crate() {
        let plan = CodegenPlan {
            pages: vec![page("/", "Home"), page("/about/", "About")],
            crate_name: "demo-server".to_owned(),
            backends: vec![],
        };
        let out = generate(&plan).unwrap();
        // 2 handlers + handlers/mod.rs + router.rs + lib.rs +
        // Cargo.toml + main.rs + tests/smoke.rs
        assert_eq!(out.files.len(), 8);
        for expected in [
            "src/handlers/index.rs",
            "src/handlers/about.rs",
            "src/handlers/mod.rs",
            "src/router.rs",
            "src/lib.rs",
            "Cargo.toml",
            "src/main.rs",
            "tests/smoke.rs",
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
        // pub use re-exports so router can call `handlers::render_*`
        // directly without naming the submodule.
        assert!(mod_rs.contents.contains("pub use index::render_index;"));
        assert!(mod_rs.contents.contains("pub use about::render_about;"));
    }

    #[test]
    fn handler_file_paths_use_snake_case() {
        // Rust requires `pub mod <ident>;` to map to a file named
        // `<ident>.rs` (or `<ident>/mod.rs`). Slugs with dashes
        // need to be snake-cased on disk; the mod re-exports
        // re-flatten under the kebab-aware fn_name on the call
        // side.
        let plan = CodegenPlan {
            pages: vec![page("/about/privacy-policy/", "Privacy")],
            crate_name: "demo-server".to_owned(),
            backends: vec![],
        };
        let out = generate(&plan).unwrap();
        assert!(
            out.files
                .iter()
                .any(|f| f.path == "src/handlers/about_privacy_policy.rs"),
            "expected snake-case file name on disk; got: {:?}",
            out.files.iter().map(|f| &f.path).collect::<Vec<_>>()
        );
        // Kebab variant must NOT exist (Rust won't load it).
        assert!(!out
            .files
            .iter()
            .any(|f| f.path == "src/handlers/about-privacy-policy.rs"));
    }

    #[test]
    fn generate_emits_handler_with_maud_body_and_path_comment() {
        let plan = CodegenPlan {
            pages: vec![page("/about/", "About Us")],
            crate_name: "demo-server".to_owned(),
            backends: vec![],
        };
        let out = generate(&plan).unwrap();
        let about = out
            .files
            .iter()
            .find(|f| f.path == "src/handlers/about.rs")
            .unwrap();
        assert!(about.contents.contains("Path: \"/about/\""));
        assert!(about
            .contents
            .contains("pub async fn render_about() -> Html<String>"));
        // Title round-trips through the embedded JSON payload.
        assert!(
            about.contents.contains(r#""title":"About Us""#),
            "expected serialized page json to embed the title verbatim, got:\n{}",
            about.contents
        );
        // Real render path, not the stub string return.
        assert!(about.contents.contains("loom_cms_render::render_page(p)"));
        assert!(about
            .contents
            .contains("loom_cms_render::page_shell_themed"));
        // OnceLock parse-once-per-process.
        assert!(about.contents.contains("static CELL: OnceLock"));
    }

    #[test]
    fn longest_hash_run_after_quote_picks_right_padding() {
        assert_eq!(longest_hash_run_after_quote(""), 0);
        assert_eq!(longest_hash_run_after_quote("no quotes here"), 0);
        assert_eq!(longest_hash_run_after_quote(r#"a "b"#), 0);
        assert_eq!(longest_hash_run_after_quote(r##"a "#b"##), 1);
        assert_eq!(longest_hash_run_after_quote(r###"a "##b "###), 2);
    }

    #[test]
    fn handler_raw_string_padding_avoids_payload_quote_hash_sequences() {
        // Craft a page whose title contains "# which would
        // collide with a r#"..."# delimiter — generator must pick
        // a longer pound padding. Built without raw strings here
        // to avoid the same delimiter problem in test source.
        let title = String::from("weird \"# title");
        let desc = String::from("another \"## one");
        let mut p = page("/edge/", &title);
        p.description = desc;
        let plan = CodegenPlan {
            pages: vec![p],
            crate_name: "demo-server".to_owned(),
            backends: vec![],
        };
        let out = generate(&plan).unwrap();
        let edge = out
            .files
            .iter()
            .find(|f| f.path == "src/handlers/edge.rs")
            .unwrap();
        assert!(
            edge.contents.contains("r###\""),
            "expected raw-string padding of at least 3 # to clear \"## in payload; got:\n{}",
            edge.contents
        );
    }

    #[test]
    fn generate_rejects_bad_crate_name() {
        let plan = CodegenPlan {
            pages: vec![],
            crate_name: "Demo Server".to_owned(),
            backends: vec![],
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
            backends: vec![],
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
            backends: vec![],
        };
        let out = generate(&plan).unwrap();
        assert_eq!(out.stages.len(), 5);
        assert_eq!(out.stages[0].stage, "handler-scaffold");
        assert_eq!(out.stages[0].files_emitted, 2); // 1 handler + 1 mod.rs
        assert_eq!(out.stages[1].stage, "router-assembly");
        assert_eq!(out.stages[1].files_emitted, 2); // router.rs + lib.rs
        assert_eq!(out.stages[2].stage, "crate-manifest");
        assert_eq!(out.stages[2].files_emitted, 2); // Cargo.toml + main.rs
        assert_eq!(out.stages[3].stage, "persistence-layer");
        assert_eq!(out.stages[3].files_emitted, 0); // empty backends → no files
        assert_eq!(out.stages[4].stage, "smoke-tests");
        assert_eq!(out.stages[4].files_emitted, 1); // tests/smoke.rs
    }

    #[test]
    fn smoke_tests_cover_every_page() {
        let plan = CodegenPlan {
            pages: vec![page("/", "Home"), page("/about/", "About")],
            crate_name: "demo-server".to_owned(),
            backends: vec![],
        };
        let out = generate(&plan).unwrap();
        let smoke = out
            .files
            .iter()
            .find(|f| f.path == "tests/smoke.rs")
            .unwrap();
        assert!(smoke.contents.contains("async fn smoke_index()"));
        assert!(smoke.contents.contains("async fn smoke_about()"));
        // Routes match normalize_axum_path output.
        assert!(smoke.contents.contains(".uri(\"/\")"));
        assert!(smoke.contents.contains(".uri(\"/about\")"));
        // Asserts on status + non-empty body.
        assert!(smoke.contents.contains("StatusCode::OK"));
        assert!(smoke.contents.contains("body.is_empty()"));
    }

    #[test]
    fn smoke_tests_skipped_when_pages_empty() {
        let plan = CodegenPlan {
            pages: vec![],
            crate_name: "demo-server".to_owned(),
            backends: vec![],
        };
        let out = generate(&plan).unwrap();
        let smoke_row = out
            .stages
            .iter()
            .find(|s| s.stage == "smoke-tests")
            .unwrap();
        assert_eq!(smoke_row.files_emitted, 0);
    }

    #[test]
    fn cargo_toml_includes_dev_deps_for_smoke_tests() {
        let plan = CodegenPlan {
            pages: vec![page("/", "x")],
            crate_name: "demo-server".to_owned(),
            backends: vec![],
        };
        let out = generate(&plan).unwrap();
        let toml = out.files.iter().find(|f| f.path == "Cargo.toml").unwrap();
        assert!(toml.contents.contains("[dev-dependencies]"));
        assert!(toml.contents.contains("tower ="));
        assert!(toml.contents.contains("http-body-util"));
    }

    #[test]
    fn persistence_layer_emits_one_stub_per_backend_plus_mod() {
        let plan = CodegenPlan {
            pages: vec![page("/", "Home")],
            crate_name: "demo-server".to_owned(),
            backends: vec![backend("subscribe"), backend("contact-form")],
        };
        let out = generate(&plan).unwrap();
        assert!(out.files.iter().any(|f| f.path == "src/db/subscribe.rs"));
        assert!(
            out.files.iter().any(|f| f.path == "src/db/contact_form.rs"),
            "kebab slug must produce snake-case file on disk"
        );
        assert!(out.files.iter().any(|f| f.path == "src/db/mod.rs"));
        let mod_rs = out
            .files
            .iter()
            .find(|f| f.path == "src/db/mod.rs")
            .unwrap();
        assert!(mod_rs.contents.contains("pub mod subscribe;"));
        assert!(mod_rs.contents.contains("pub mod contact_form;"));
        assert!(mod_rs
            .contents
            .contains("pub use subscribe::query_subscribe;"));
        assert!(mod_rs
            .contents
            .contains("pub use contact_form::query_contact_form;"));
        // Audit reflects emit count.
        let persistence_row = out
            .stages
            .iter()
            .find(|s| s.stage == "persistence-layer")
            .unwrap();
        assert_eq!(persistence_row.files_emitted, 3); // 2 backends + mod.rs
    }

    #[test]
    fn persistence_stub_is_fail_closed_signature() {
        let plan = CodegenPlan {
            pages: vec![page("/", "Home")],
            crate_name: "demo-server".to_owned(),
            backends: vec![backend("subscribe")],
        };
        let out = generate(&plan).unwrap();
        let stub = out
            .files
            .iter()
            .find(|f| f.path == "src/db/subscribe.rs")
            .unwrap();
        assert!(stub.contents.contains("use sqlx::PgPool;"));
        assert!(stub
            .contents
            .contains("pub async fn query_subscribe(_pool: &PgPool)"));
        assert!(stub
            .contents
            .contains("Result<serde_json::Value, sqlx::Error>"));
        assert!(stub.contents.contains("not implemented"));
    }

    #[test]
    fn cargo_toml_includes_sqlx_only_when_backends_declared() {
        let no_be = CodegenPlan {
            pages: vec![page("/", "x")],
            crate_name: "demo-server".to_owned(),
            backends: vec![],
        };
        let toml_no_be = generate(&no_be).unwrap();
        let toml_no_be_str = &toml_no_be
            .files
            .iter()
            .find(|f| f.path == "Cargo.toml")
            .unwrap()
            .contents;
        assert!(!toml_no_be_str.contains("sqlx ="));

        let with_be = CodegenPlan {
            pages: vec![page("/", "x")],
            crate_name: "demo-server".to_owned(),
            backends: vec![backend("subscribe")],
        };
        let toml_be = generate(&with_be).unwrap();
        let toml_be_str = &toml_be
            .files
            .iter()
            .find(|f| f.path == "Cargo.toml")
            .unwrap()
            .contents;
        assert!(toml_be_str.contains("sqlx ="));
        assert!(toml_be_str.contains("postgres"));
    }

    #[test]
    fn lib_root_exports_db_module_only_when_backends_declared() {
        let no_be = CodegenPlan {
            pages: vec![page("/", "x")],
            crate_name: "demo-server".to_owned(),
            backends: vec![],
        };
        let out_no_be = generate(&no_be).unwrap();
        let lib_no_be = out_no_be
            .files
            .iter()
            .find(|f| f.path == "src/lib.rs")
            .unwrap();
        assert!(!lib_no_be.contents.contains("pub mod db;"));

        let with_be = CodegenPlan {
            pages: vec![page("/", "x")],
            crate_name: "demo-server".to_owned(),
            backends: vec![backend("subscribe")],
        };
        let out_be = generate(&with_be).unwrap();
        let lib_be = out_be
            .files
            .iter()
            .find(|f| f.path == "src/lib.rs")
            .unwrap();
        assert!(lib_be.contents.contains("pub mod db;"));
    }

    #[test]
    fn cargo_toml_pins_canonical_stack_and_uses_package_name() {
        let plan = CodegenPlan {
            pages: vec![page("/", "x")],
            crate_name: "demo-server".to_owned(),
            backends: vec![],
        };
        let out = generate(&plan).unwrap();
        let toml = out.files.iter().find(|f| f.path == "Cargo.toml").unwrap();
        assert!(toml.contents.contains("name = \"demo-server\""));
        assert!(toml.contents.contains("edition = \"2021\""));
        assert!(toml.contents.contains("publish = false"));
        // Canonical stack pins (loose semver acceptable):
        assert!(toml.contents.contains("axum = "));
        assert!(toml.contents.contains("tokio = "));
        assert!(toml.contents.contains("serde = "));
        assert!(toml.contents.contains("loom-cms-render = "));
        assert!(toml.contents.contains("unsafe_code = \"forbid\""));
    }

    #[test]
    fn main_rs_references_lib_via_snake_case_ident() {
        // package name `demo-server` → lib ident `demo_server`.
        let plan = CodegenPlan {
            pages: vec![page("/", "x")],
            crate_name: "demo-server".to_owned(),
            backends: vec![],
        };
        let out = generate(&plan).unwrap();
        let main = out.files.iter().find(|f| f.path == "src/main.rs").unwrap();
        assert!(main.contents.contains("demo_server::build_router()"));
        assert!(main.contents.contains("FORGE_LISTEN"));
        assert!(main.contents.contains("#[tokio::main]"));
        assert!(main.contents.contains("axum::serve"));
    }

    #[test]
    fn package_to_lib_ident_kebabs_to_snake() {
        assert_eq!(package_to_lib_ident("demo"), "demo");
        assert_eq!(package_to_lib_ident("demo-server"), "demo_server");
        assert_eq!(
            package_to_lib_ident("prosperity-club-server"),
            "prosperity_club_server"
        );
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
            backends: vec![],
        };
        let out = generate(&plan).unwrap();
        let router = out
            .files
            .iter()
            .find(|f| f.path == "src/router.rs")
            .unwrap();
        assert!(router
            .contents
            .contains(".route(\"/\", get(handlers::render_index))"));
        assert!(router
            .contents
            .contains(".route(\"/about\", get(handlers::render_about))"));
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
            backends: vec![],
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
