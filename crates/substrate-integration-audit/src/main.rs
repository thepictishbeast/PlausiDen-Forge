//! `substrate-integration-audit` — surfaces orphan crates.
//!
//! Per paul 2026-05-21 (#330): every substrate crate exists to
//! serve something downstream. A crate with zero incoming
//! dependencies is either:
//!
//! - A binary / leaf entrypoint (forge-cli, forge-mcp, the
//!   audit binaries) — legitimately has no incoming deps.
//! - An unconnected slice — code that builds but nothing
//!   consumes. Surface it so the operator decides: wire it in,
//!   delete it, or annotate as a leaf.
//!
//! ## What it scans
//!
//! Walks every `Cargo.toml` under the supplied roots (default:
//! current working directory). Skips `.git`, `target`,
//! `node_modules`. Parses `[package].name` + every dependency
//! table (`[dependencies]`, `[dev-dependencies]`,
//! `[build-dependencies]`).
//!
//! ## What it catches
//!
//! For each crate, computes:
//!
//! - depends-on count (its own dep list)
//! - depended-on count (sum of incoming refs from siblings)
//!
//! Reports any crate with depended-on count == 0 AND no `[[bin]]`
//! section (binaries are leaves by design).
//!
//! ## Inline suppression
//!
//! A crate's Cargo.toml can suppress with a comment
//! `# integration-audit-allow: <reason>` on any line; the
//! scanner walks the file and skips that crate entirely.
//!
//! ## Output
//!
//! Per-orphan line on stdout:
//!
//! ```text
//! crates/loom-audit/Cargo.toml: orphan crate `loom-audit` (no incoming deps; not a binary)
//! ```
//!
//! Trailing summary on stderr.
//!
//! ## Exit codes
//!
//! - `0` — clean (no orphans, or all orphans are binaries)
//! - `1` — at least one orphan
//! - `2` — fatal parse error

use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use walkdir::{DirEntry, WalkDir};

/// One parsed crate manifest record.
#[derive(Debug)]
struct CrateRecord {
    name: String,
    manifest_path: PathBuf,
    deps: BTreeSet<String>,
    is_binary: bool,
    audit_allow: bool,
}

const SKIP_DIRS: &[&str] = &[".git", "target", "node_modules", "dist", "runs"];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let roots: Vec<PathBuf> = if args.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        args.iter().map(PathBuf::from).collect()
    };
    match audit(&roots) {
        Ok(orphans) => {
            for o in &orphans {
                let path_display = display_path(&o.manifest_path);
                println!(
                    "{path_display}: orphan crate `{name}` (no incoming deps; not a binary)",
                    name = o.name
                );
            }
            eprintln!();
            if orphans.is_empty() {
                eprintln!("substrate-integration-audit: clean (0 orphan crates)");
                ExitCode::SUCCESS
            } else {
                eprintln!("substrate-integration-audit: {} orphan crate(s)", orphans.len());
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("substrate-integration-audit: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn audit(roots: &[PathBuf]) -> Result<Vec<CrateRecord>> {
    let mut crates: BTreeMap<String, CrateRecord> = BTreeMap::new();
    for root in roots {
        let abs = root
            .canonicalize()
            .with_context(|| format!("canonicalize {}", root.display()))?;
        for entry in WalkDir::new(&abs).into_iter().filter_entry(is_not_skipped) {
            let entry = entry.context("walk")?;
            if !entry.file_type().is_file() {
                continue;
            }
            if entry.file_name() != "Cargo.toml" {
                continue;
            }
            if let Some(record) = parse_cargo_toml(entry.path()) {
                crates.insert(record.name.clone(), record);
            }
        }
    }
    let mut incoming: BTreeMap<String, usize> = BTreeMap::new();
    for c in crates.values() {
        for dep in &c.deps {
            *incoming.entry(dep.clone()).or_insert(0) += 1;
        }
    }
    let mut orphans = Vec::new();
    for c in crates.into_values() {
        if c.audit_allow || c.is_binary {
            continue;
        }
        let count = incoming.get(&c.name).copied().unwrap_or(0);
        if count == 0 {
            orphans.push(c);
        }
    }
    Ok(orphans)
}

fn is_not_skipped(entry: &DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    if entry.file_type().is_dir() {
        !SKIP_DIRS.iter().any(|d| name == *d)
    } else {
        true
    }
}

fn parse_cargo_toml(path: &Path) -> Option<CrateRecord> {
    let body = std::fs::read_to_string(path).ok()?;
    let audit_allow = body.contains("integration-audit-allow:");
    let value: toml::Value = toml::from_str(&body).ok()?;
    let package = value.get("package")?.as_table()?;
    let name = package.get("name")?.as_str()?.to_owned();
    let is_binary = value.get("bin").map(|v| v.is_array()).unwrap_or(false)
        || path
            .parent()
            .map(|p| p.join("src/main.rs").exists())
            .unwrap_or(false);
    let mut deps: BTreeSet<String> = BTreeSet::new();
    for table_key in &[
        "dependencies",
        "dev-dependencies",
        "build-dependencies",
    ] {
        if let Some(table) = value.get(*table_key).and_then(toml::Value::as_table) {
            for k in table.keys() {
                deps.insert(k.to_owned());
            }
        }
    }
    Some(CrateRecord {
        name,
        manifest_path: path.to_path_buf(),
        deps,
        is_binary,
        audit_allow,
    })
}

fn display_path(path: &Path) -> String {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    path.strip_prefix(&cwd)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tempdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ia-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn write_crate(root: &Path, name: &str, deps: &[&str], is_bin: bool) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        let mut s = String::new();
        s.push_str(&format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"
        ));
        if is_bin {
            fs::create_dir_all(dir.join("src")).unwrap();
            fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();
        }
        if !deps.is_empty() {
            s.push_str("\n[dependencies]\n");
            for d in deps {
                s.push_str(&format!("{d} = \"1\"\n"));
            }
        }
        fs::write(dir.join("Cargo.toml"), s).unwrap();
    }

    #[test]
    fn library_with_no_incoming_deps_is_orphan() {
        let root = tempdir("orphan");
        write_crate(&root, "lonely-lib", &[], false);
        let orphans = audit(&[root.clone()]).unwrap();
        let names: Vec<_> = orphans.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"lonely-lib"));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn binary_is_not_orphan() {
        let root = tempdir("bin");
        write_crate(&root, "my-bin", &[], true);
        let orphans = audit(&[root.clone()]).unwrap();
        let names: Vec<_> = orphans.iter().map(|c| c.name.as_str()).collect();
        assert!(!names.contains(&"my-bin"));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn library_with_incoming_dep_is_not_orphan() {
        let root = tempdir("connected");
        write_crate(&root, "leaf-lib", &[], false);
        write_crate(&root, "consumer", &["leaf-lib"], false);
        let orphans = audit(&[root.clone()]).unwrap();
        let names: Vec<_> = orphans.iter().map(|c| c.name.as_str()).collect();
        // `consumer` has no incoming deps and isn't a binary, so it's an orphan.
        // `leaf-lib` is consumed → not orphan.
        assert!(!names.contains(&"leaf-lib"));
        assert!(names.contains(&"consumer"));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn audit_allow_directive_suppresses() {
        let root = tempdir("allow");
        let dir = root.join("opted-out");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            "# integration-audit-allow: substrate doctrine entrypoint\n\
             [package]\nname = \"opted-out\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        let orphans = audit(&[root.clone()]).unwrap();
        let names: Vec<_> = orphans.iter().map(|c| c.name.as_str()).collect();
        assert!(!names.contains(&"opted-out"));
        fs::remove_dir_all(&root).ok();
    }
}
