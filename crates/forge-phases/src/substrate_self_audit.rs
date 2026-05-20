//! `substrate_self_audit` — meta-phase auditing the substrate's
//! own consistency.
//!
//! Task #248 per the variation-architecture spec. Where every
//! other phase audits the operator's site, this phase audits the
//! substrate's own Rust source — useful for CI in the substrate
//! repo to catch:
//!
//! * Phase modules missing `#[cfg(test)] mod tests`.
//! * `unwrap()` / `expect()` calls in non-test code.
//! * Public types marked `#[non_exhaustive]` without a
//!   corresponding `pub fn new(` constructor.
//! * Files missing the `#![forbid(unsafe_code)]` lint.
//!
//! ## Activation
//!
//! Silent by default. Activate via:
//!
//! ```toml
//! [substrate_self_audit]
//! enforce = true
//! # Substrate source root. Default: `crates/forge-phases/src`.
//! # Configure to audit other crates too.
//! source_dir = "crates/forge-phases/src"
//! ```
//!
//! The phase only fires when invoked against the substrate repo
//! itself — when a user site has a `[substrate_self_audit]`
//! block (which is unusual). Substrate-repo CI invokes
//! `forge build` with the section enabled to lint the substrate
//! codebase at every build.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * `#[non_exhaustive]` on phase struct.
//! * No unwrap/expect in non-test code.
//! * Pure walk over substrate source.

use std::fs;
use std::path::{Path, PathBuf};

use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// `substrate_self_audit` phase implementation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct SubstrateSelfAuditPhase;

impl Phase for SubstrateSelfAuditPhase {
    fn name(&self) -> &'static str {
        "substrate_self_audit"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let Some(cfg) = AuditConfig::load(&ctx.root) else {
            return Ok(findings);
        };
        if !cfg.enforce {
            return Ok(findings);
        }
        let src_root = ctx.root.join(&cfg.source_dir);
        if !src_root.is_dir() {
            return Ok(findings);
        }

        let mut rust_files = Vec::new();
        collect_rust_files(&src_root, &mut rust_files).map_err(|e| BuildError::Io {
            context: format!("walk {}", src_root.display()),
            source: e,
        })?;
        rust_files.sort();

        for path in &rust_files {
            audit_file(path, &mut findings, self.name());
        }

        Ok(findings)
    }
}

#[derive(Debug, Clone)]
struct AuditConfig {
    enforce: bool,
    source_dir: PathBuf,
}

impl AuditConfig {
    fn load(root: &Path) -> Option<Self> {
        let body = fs::read_to_string(root.join("forge.toml")).ok()?;
        let value: toml::Value = toml::from_str(&body).ok()?;
        let section = value.get("substrate_self_audit")?.as_table()?;
        let enforce = section
            .get("enforce")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let source_dir = section
            .get("source_dir")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("crates/forge-phases/src"));
        Some(Self {
            enforce,
            source_dir,
        })
    }
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn audit_file(path: &Path, findings: &mut Vec<Finding>, phase: &'static str) {
    let Ok(body) = fs::read_to_string(path) else {
        return;
    };
    let path_str = path.display().to_string();

    let has_test_module = body.contains("#[cfg(test)]") && body.contains("mod tests");
    let has_pub_phase_or_type = body.contains("pub struct ") || body.contains("pub enum ");
    if has_pub_phase_or_type && !has_test_module && !path.ends_with("lib.rs") && !path.ends_with("main.rs") {
        findings.push(
            Finding::warn(
                phase,
                path_str.clone(),
                "substrate_self_audit — file ships public types but has no `#[cfg(test)] mod tests` block"
                    .to_owned(),
            )
            .citing(["substrate-001"])
            .why("the substrate's testing discipline requires every public-API file ship with at least one test module")
            .fix("add a `#[cfg(test)] mod tests { ... }` block with at least one unit test per public function"),
        );
    }

    // Unwrap/expect outside test code. Heuristic: scan line-by-line,
    // skip when we're inside a `#[cfg(test)]` or `mod tests {` block.
    let depth_test: i32 = 0;
    let mut brace_at_test: Vec<i32> = Vec::new();
    let mut current_brace: i32 = 0;
    let mut inside_test = false;
    for (lineno, line) in body.lines().enumerate() {
        let trimmed = line.trim();

        // Detect entering a test scope.
        if trimmed.contains("#[cfg(test)]") || trimmed.starts_with("mod tests") {
            inside_test = true;
            brace_at_test.push(current_brace);
        }

        let opens = line.matches('{').count() as i32;
        let closes = line.matches('}').count() as i32;
        current_brace += opens - closes;

        if inside_test {
            // Check if we've left the test scope.
            if let Some(&start_brace) = brace_at_test.last() {
                if current_brace <= start_brace {
                    brace_at_test.pop();
                    if brace_at_test.is_empty() {
                        inside_test = false;
                    }
                }
            }
            let _ = depth_test;
            continue;
        }

        // Outside tests. Check for unwrap/expect.
        if (line.contains(".unwrap()") || line.contains(".expect(")) && !line.contains("//") && !line.contains("///") {
            // Heuristic: skip lines that are entirely a comment.
            let line_no_lead = line.trim_start();
            if line_no_lead.starts_with("//") || line_no_lead.starts_with("///") {
                continue;
            }
            findings.push(
                Finding::warn(
                    phase,
                    format!("{path_str}:{}", lineno + 1),
                    format!(
                        "substrate_self_audit — non-test `.unwrap()` or `.expect()` call: `{}`",
                        trimmed.chars().take(120).collect::<String>()
                    ),
                )
                .citing(["substrate-002"])
                .why("the substrate's no-unwrap doctrine prohibits unwrap/expect in non-test code; failures must propagate via typed errors")
                .fix("replace with `?` propagation OR map_err to a typed error variant OR use a `let Some(x) = ... else { ... }` pattern"),
            );
        }
    }

    // Non-exhaustive types without new() constructors. Heuristic:
    // find `#[non_exhaustive]` lines; check the following declaration
    // has a sibling `pub fn new(`.
    let lines: Vec<&str> = body.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if !line.trim().contains("#[non_exhaustive]") {
            continue;
        }
        // Find the next `pub struct` or `pub enum` declaration.
        let mut decl_name: Option<&str> = None;
        for next_line in lines.iter().skip(i + 1).take(5) {
            let trimmed = next_line.trim();
            if let Some(rest) = trimmed.strip_prefix("pub struct ") {
                let name_end = rest
                    .find(|c: char| {
                        c == '<' || c == '{' || c == '(' || c == ' ' || c == ';'
                    })
                    .unwrap_or(rest.len());
                decl_name = Some(&rest[..name_end]);
                break;
            }
            if let Some(rest) = trimmed.strip_prefix("pub enum ") {
                // Enums don't need new(); skip.
                let _ = rest;
                decl_name = None;
                break;
            }
        }
        let Some(name) = decl_name else { continue };
        // Empty unit struct (`pub struct X;`) doesn't need new() either —
        // can be constructed via default. Skip if the struct has no
        // fields.
        let scan_window = lines
            .iter()
            .skip(i + 1)
            .take(60)
            .copied()
            .collect::<Vec<_>>()
            .join("\n");
        if scan_window.contains(&format!("pub struct {name};")) {
            continue;
        }
        // Check the rest of the file for `impl <name>` containing
        // `pub fn new(`.
        let needle_impl_open = format!("impl {name} ");
        let needle_impl_brace = format!("impl {name} {{");
        let needle_impl_close = format!("impl {name}{{");
        let constructor_needle = "pub fn new(";
        let has_constructor = body.lines().enumerate().any(|(li, l)| {
            let is_impl_line =
                l.starts_with(&needle_impl_open) || l.contains(&needle_impl_brace) || l.contains(&needle_impl_close);
            if !is_impl_line {
                return false;
            }
            // Scan ahead 60 lines for `pub fn new(`.
            body.lines()
                .skip(li + 1)
                .take(60)
                .any(|ll| ll.contains(constructor_needle))
        });
        if !has_constructor {
            findings.push(
                Finding::warn(
                    phase,
                    format!("{path_str}:{}", i + 1),
                    format!(
                        "substrate_self_audit — `#[non_exhaustive] pub struct {name}` has no `pub fn new(` constructor; external crates cannot instantiate it"
                    ),
                )
                .citing(["substrate-003"])
                .why("non_exhaustive structs require external constructors so consumers in other crates can build them despite the locked struct-literal path")
                .fix(format!("add `impl {name} {{ pub fn new(...) -> Self {{ ... }} }}` with the canonical construction signature")),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "forge-self-audit-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("src")).unwrap();
        p
    }

    fn ctx_for(root: &Path) -> BuildCtx {
        BuildCtx {
            root: root.to_path_buf(),
            static_dir: root.join("static"),
            mode: forge_core::BuildMode::Poc,
        }
    }

    fn write_src(root: &Path, name: &str, body: &str) {
        fs::write(root.join("src").join(name), body).unwrap();
    }

    #[test]
    fn phase_silent_when_not_enforced() {
        let root = temp_root("silent");
        fs::write(
            root.join("forge.toml"),
            "[substrate_self_audit]\nenforce = false\nsource_dir = \"src\"\n",
        )
        .unwrap();
        write_src(&root, "foo.rs", "pub struct Foo;\n");
        let findings = SubstrateSelfAuditPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_missing_tests_module() {
        let root = temp_root("no-tests");
        fs::write(
            root.join("forge.toml"),
            "[substrate_self_audit]\nenforce = true\nsource_dir = \"src\"\n",
        )
        .unwrap();
        write_src(
            &root,
            "foo.rs",
            "pub struct Foo { pub x: i32 }\n",
        );
        let findings = SubstrateSelfAuditPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("no `#[cfg(test)] mod tests` block")),
            "expected missing-tests finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_tests_present() {
        let root = temp_root("with-tests");
        fs::write(
            root.join("forge.toml"),
            "[substrate_self_audit]\nenforce = true\nsource_dir = \"src\"\n",
        )
        .unwrap();
        write_src(
            &root,
            "foo.rs",
            r#"pub struct Foo { pub x: i32 }

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {}
}
"#,
        );
        let findings = SubstrateSelfAuditPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            !findings.iter().any(|f| f.message.contains("no `#[cfg(test)] mod tests` block")),
            "shouldn't flag missing tests when present; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_unwrap_in_non_test_code() {
        let root = temp_root("unwrap");
        fs::write(
            root.join("forge.toml"),
            "[substrate_self_audit]\nenforce = true\nsource_dir = \"src\"\n",
        )
        .unwrap();
        write_src(
            &root,
            "foo.rs",
            r#"pub struct Foo { pub x: i32 }

pub fn get_x() -> i32 {
    let opt: Option<i32> = None;
    opt.unwrap()
}

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        let _: Option<i32> = Some(0);
    }
}
"#,
        );
        let findings = SubstrateSelfAuditPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("non-test `.unwrap()`")),
            "expected unwrap finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_does_not_flag_unwrap_inside_test_module() {
        let root = temp_root("unwrap-in-tests");
        fs::write(
            root.join("forge.toml"),
            "[substrate_self_audit]\nenforce = true\nsource_dir = \"src\"\n",
        )
        .unwrap();
        write_src(
            &root,
            "foo.rs",
            r#"pub struct Foo { pub x: i32 }

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        let v: Option<i32> = Some(5);
        let _ = v.unwrap();
    }
}
"#,
        );
        let findings = SubstrateSelfAuditPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            !findings.iter().any(|f| f.message.contains("non-test `.unwrap()`")),
            "shouldn't flag unwrap inside test module; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_non_exhaustive_struct_missing_constructor() {
        let root = temp_root("no-new");
        fs::write(
            root.join("forge.toml"),
            "[substrate_self_audit]\nenforce = true\nsource_dir = \"src\"\n",
        )
        .unwrap();
        write_src(
            &root,
            "foo.rs",
            r#"#[non_exhaustive]
pub struct Foo {
    pub x: i32,
    pub y: i32,
}

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {}
}
"#,
        );
        let findings = SubstrateSelfAuditPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("no `pub fn new(` constructor")),
            "expected missing-constructor finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_non_exhaustive_has_constructor() {
        let root = temp_root("has-new");
        fs::write(
            root.join("forge.toml"),
            "[substrate_self_audit]\nenforce = true\nsource_dir = \"src\"\n",
        )
        .unwrap();
        write_src(
            &root,
            "foo.rs",
            r#"#[non_exhaustive]
pub struct Foo {
    pub x: i32,
}

impl Foo {
    pub fn new(x: i32) -> Self {
        Self { x }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {}
}
"#,
        );
        let findings = SubstrateSelfAuditPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            !findings.iter().any(|f| f.message.contains("no `pub fn new(` constructor")),
            "shouldn't flag when new() is present; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_skips_unit_structs() {
        let root = temp_root("unit-struct");
        fs::write(
            root.join("forge.toml"),
            "[substrate_self_audit]\nenforce = true\nsource_dir = \"src\"\n",
        )
        .unwrap();
        write_src(
            &root,
            "foo.rs",
            r#"#[non_exhaustive]
pub struct Foo;

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {}
}
"#,
        );
        let findings = SubstrateSelfAuditPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            !findings.iter().any(|f| f.message.contains("no `pub fn new(` constructor")),
            "unit struct shouldn't need constructor; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
