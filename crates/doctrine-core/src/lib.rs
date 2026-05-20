//! doctrine-core — typed projection of PlausiDen-AVP-Doctrine/doctrine/rules/*.toml.
//!
//! Loads the canonical rule database (statement + rationale + enforcement
//! triples), validates the schema, exposes a query API.
//!
//! # Schema
//!
//! Each rule lives in `<doctrine-root>/doctrine/rules/<domain>.toml` under
//! `[[rule]]` arrays. The schema is documented at
//! `<doctrine-root>/doctrine/rules/SCHEMA.md`.
//!
//! Required fields per rule (the triple):
//! - `id` — globally unique kebab-case identifier.
//! - `statement` — precise sentence stating the rule.
//! - `rationale` — why the rule exists.
//! - `enforcement` — array naming the verification mechanisms (Forge phases,
//!   Crawler axes, lints, schema checks).
//!
//! Plus: `name`, `domain`, `severity`, `lifecycle`, `applies_to`,
//! optionally `related_traits`, `references`, `deprecated_at`, `replaced_by`.
//!
//! # Invariants
//!
//! - Every loaded rule has the full triple (statement + rationale + enforcement).
//!   Incomplete rules fail parse.
//! - `id` is globally unique across all loaded files.
//! - `domain` matches the file's declared `[meta].domain`.
//! - `deprecated` lifecycle requires `deprecated_at` set.
//! - `replaced_by` references resolve to a loaded rule.
//!
//! # AVP-2 invariants
//!
//! - `unsafe_code = "deny"` via workspace lints.
//! - Zero `unwrap` / `expect` in non-test paths.
//! - Every public type carries doc comments + `#[non_exhaustive]` where
//!   appropriate (forward-compat per [[backward-compat-version-discipline]]).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One doctrine rule with the full statement+rationale+enforcement triple.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Rule {
    /// Globally unique kebab-case identifier. Example: `prim-001`.
    pub id: String,
    /// Short human-readable label. Example: "Mobile Friendly Required".
    pub name: String,
    /// Domain key. Must match the containing file's `[meta].domain`.
    pub domain: Domain,
    /// Precise sentence stating the rule.
    pub statement: String,
    /// Multi-line explanation of why the rule exists.
    pub rationale: String,
    /// List of mechanisms verifying compliance — Forge phases, Crawler axes,
    /// lints, schema checks. Each entry is human-readable; tooling may parse
    /// the entries for cross-referencing.
    pub enforcement: Vec<String>,
    /// Where the rule applies — path globs, crate names, artifact classes.
    pub applies_to: Vec<String>,
    /// Build-time consequence of violation.
    pub severity: Severity,
    /// Stability state.
    pub lifecycle: Lifecycle,
    /// Cross-references into the trait system. Optional.
    #[serde(default)]
    pub related_traits: Vec<String>,
    /// External references — RFCs, ADRs, WCAG sections, prior incidents.
    /// Optional.
    #[serde(default)]
    pub references: Vec<String>,
    /// ISO 8601 date when sunset begins. Required if `lifecycle = "deprecated"`.
    #[serde(default)]
    pub deprecated_at: Option<String>,
    /// Replacement rule id, if this rule is deprecated and has a successor.
    #[serde(default)]
    pub replaced_by: Option<String>,
}

/// The domain a rule belongs to. Matches the per-file `[meta].domain` field.
///
/// `#[non_exhaustive]` because adding new domains is additive per
/// [[backward-compat-version-discipline]].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Domain {
    /// Build pipeline + audit gates.
    Build,
    /// Loom primitive authoring.
    Primitives,
    /// Security (input validation, crypto, secrets, auth, audit logging).
    Security,
    /// Testing discipline (unit, proptest, fixture, contract).
    Testing,
    /// Documentation (rustdoc, AGENTS.md, ADR, migration guide).
    Docs,
    /// Logging / observability.
    Logging,
    /// Performance (budgets, CWV, asset pipeline).
    Perf,
    /// Content authoring (density, claims, testimonials, statistics).
    Content,
    /// Accessibility (keyboard, screen-reader, contrast, motion, cognitive).
    Accessibility,
}

impl Domain {
    /// All known domain variants. Useful for `doctrine query --domain` validation.
    pub fn all() -> &'static [Domain] {
        &[
            Domain::Build,
            Domain::Primitives,
            Domain::Security,
            Domain::Testing,
            Domain::Docs,
            Domain::Logging,
            Domain::Perf,
            Domain::Content,
            Domain::Accessibility,
        ]
    }

    /// Filename stem for the per-domain TOML (e.g., `build` → `build.toml`).
    pub fn file_stem(self) -> &'static str {
        match self {
            Domain::Build => "build",
            Domain::Primitives => "primitives",
            Domain::Security => "security",
            Domain::Testing => "testing",
            Domain::Docs => "docs",
            Domain::Logging => "logging",
            Domain::Perf => "perf",
            Domain::Content => "content",
            Domain::Accessibility => "accessibility",
        }
    }
}

/// Build-time consequence of violating a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Severity {
    /// Build fails on violation. The default for stable rules.
    Strict,
    /// Build emits warning; doesn't gate ship.
    Warn,
    /// Surface-only; appears in reports but never warns or fails.
    Informational,
    /// Experimental severity — rule is being trialed; behaves like Warn
    /// until promoted. Distinct lifecycle value also tracks this.
    #[serde(rename = "experimental")]
    Experimental,
}

/// Rule lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Lifecycle {
    /// Being trialed; warnings only, not strict-enforced.
    Experimental,
    /// Binding doctrine; strict enforcement per `severity`.
    Stable,
    /// Being removed; warnings + sunset date in `deprecated_at`.
    Deprecated,
}

/// File-level metadata block. Each `doctrine/rules/<domain>.toml` declares
/// `[meta]` at the top with the doctrine version and the domain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct FileMeta {
    /// Doctrine schema version this file targets.
    pub doctrine_version: String,
    /// Domain key. Must match every rule's `domain` field in this file.
    pub domain: Domain,
    /// Last manual review date (ISO 8601). Optional.
    #[serde(default)]
    pub last_reviewed: Option<String>,
}

/// Raw per-file TOML shape (parses one `<domain>.toml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleFile {
    meta: FileMeta,
    #[serde(default, rename = "rule")]
    rules: Vec<Rule>,
}

/// Errors that can occur loading + validating the rule database.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DoctrineError {
    /// I/O error reading a rule file.
    #[error("I/O error reading {path}: {source}")]
    Io {
        /// File that was being read.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// TOML parse error.
    #[error("parse error in {path}: {message}")]
    Parse {
        /// File that failed to parse.
        path: PathBuf,
        /// Parser message.
        message: String,
    },
    /// A rule's `domain` field doesn't match the file's `[meta].domain`.
    #[error(
        "rule {rule_id} declares domain {rule_domain:?} but lives in file declaring domain \
         {file_domain:?} ({path})"
    )]
    DomainMismatch {
        /// File path.
        path: PathBuf,
        /// Rule id with the mismatch.
        rule_id: String,
        /// Domain the rule claims.
        rule_domain: Domain,
        /// Domain the file's meta declares.
        file_domain: Domain,
    },
    /// Two rules share the same id.
    #[error("duplicate rule id {0} across files {1} and {2}")]
    DuplicateId(String, PathBuf, PathBuf),
    /// A `deprecated` lifecycle rule lacks `deprecated_at`.
    #[error("rule {0} has lifecycle=deprecated but no deprecated_at date")]
    MissingDeprecatedAt(String),
    /// A `replaced_by` reference doesn't resolve to a known rule.
    #[error("rule {0} replaced_by={1} but no such rule exists in the loaded set")]
    UnresolvedReplacedBy(String, String),
    /// A required triple field is empty.
    #[error("rule {0} has empty {1} — the triple (statement + rationale + enforcement) is required")]
    IncompleteTriple(String, &'static str),
}

/// Loaded + validated doctrine rule database. Constructed by [`load_from_dir`].
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct DoctrineDatabase {
    rules: Vec<Rule>,
    by_id: HashMap<String, usize>,
    by_domain: HashMap<Domain, Vec<usize>>,
}

impl DoctrineDatabase {
    /// Returns every loaded rule.
    pub fn all(&self) -> &[Rule] {
        &self.rules
    }

    /// Returns the rule with the given id, if any.
    pub fn by_id(&self, id: &str) -> Option<&Rule> {
        self.by_id.get(id).map(|&i| &self.rules[i])
    }

    /// Returns every rule in the given domain.
    pub fn by_domain(&self, domain: Domain) -> impl Iterator<Item = &Rule> + '_ {
        self.by_domain
            .get(&domain)
            .into_iter()
            .flatten()
            .map(move |&i| &self.rules[i])
    }

    /// Returns every rule with the given severity.
    pub fn by_severity(&self, severity: Severity) -> impl Iterator<Item = &Rule> + '_ {
        self.rules.iter().filter(move |r| r.severity == severity)
    }

    /// Returns every rule with the given lifecycle.
    pub fn by_lifecycle(&self, lifecycle: Lifecycle) -> impl Iterator<Item = &Rule> + '_ {
        self.rules.iter().filter(move |r| r.lifecycle == lifecycle)
    }

    /// Returns every rule that references the given trait name in
    /// `related_traits`.
    pub fn by_trait<'a>(&'a self, trait_name: &'a str) -> impl Iterator<Item = &'a Rule> + 'a {
        self.rules
            .iter()
            .filter(move |r| r.related_traits.iter().any(|t| t == trait_name))
    }

    /// Total rule count across all loaded domains.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Whether the database has any rules loaded.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

/// Load every `doctrine/rules/<domain>.toml` under `root` and validate.
///
/// `root` is the path to the AVP-Doctrine repository (the directory
/// containing `doctrine/rules/`). The function reads every known domain's
/// file if it exists; missing files are silently skipped (a partial
/// doctrine deployment is permitted, e.g. during incremental migration).
pub fn load_from_dir<P: AsRef<Path>>(root: P) -> Result<DoctrineDatabase, DoctrineError> {
    let root = root.as_ref();
    let rules_dir = root.join("doctrine").join("rules");

    let mut all_rules: Vec<(Rule, PathBuf)> = Vec::new();

    for &domain in Domain::all() {
        let path = rules_dir.join(format!("{}.toml", domain.file_stem()));
        if !path.exists() {
            continue;
        }
        let bytes = std::fs::read_to_string(&path).map_err(|source| DoctrineError::Io {
            path: path.clone(),
            source,
        })?;
        let parsed: RuleFile = toml::from_str(&bytes).map_err(|e| DoctrineError::Parse {
            path: path.clone(),
            message: e.to_string(),
        })?;

        // File-level meta.domain must match the file we're reading.
        if parsed.meta.domain != domain {
            // Treat as parse error — the file lives in the wrong slot.
            return Err(DoctrineError::DomainMismatch {
                path: path.clone(),
                rule_id: "<file-meta>".to_string(),
                rule_domain: parsed.meta.domain,
                file_domain: domain,
            });
        }

        for rule in parsed.rules {
            // Per-rule domain agreement.
            if rule.domain != domain {
                return Err(DoctrineError::DomainMismatch {
                    path: path.clone(),
                    rule_id: rule.id.clone(),
                    rule_domain: rule.domain,
                    file_domain: domain,
                });
            }
            // The triple is non-negotiable.
            if rule.statement.trim().is_empty() {
                return Err(DoctrineError::IncompleteTriple(rule.id.clone(), "statement"));
            }
            if rule.rationale.trim().is_empty() {
                return Err(DoctrineError::IncompleteTriple(rule.id.clone(), "rationale"));
            }
            if rule.enforcement.is_empty() {
                return Err(DoctrineError::IncompleteTriple(rule.id.clone(), "enforcement"));
            }
            // Deprecated rules need a sunset date.
            if rule.lifecycle == Lifecycle::Deprecated && rule.deprecated_at.is_none() {
                return Err(DoctrineError::MissingDeprecatedAt(rule.id.clone()));
            }
            all_rules.push((rule, path.clone()));
        }
    }

    // Global uniqueness on rule.id.
    let mut by_id: HashMap<String, usize> = HashMap::with_capacity(all_rules.len());
    let mut by_domain: HashMap<Domain, Vec<usize>> = HashMap::new();
    let mut rules: Vec<Rule> = Vec::with_capacity(all_rules.len());
    let mut paths: Vec<PathBuf> = Vec::with_capacity(all_rules.len());
    for (rule, path) in all_rules {
        if let Some(&existing) = by_id.get(&rule.id) {
            return Err(DoctrineError::DuplicateId(
                rule.id.clone(),
                paths[existing].clone(),
                path,
            ));
        }
        let idx = rules.len();
        by_id.insert(rule.id.clone(), idx);
        by_domain.entry(rule.domain).or_default().push(idx);
        rules.push(rule);
        paths.push(path);
    }

    // Resolve replaced_by references.
    for rule in &rules {
        if let Some(replacement) = &rule.replaced_by {
            if !by_id.contains_key(replacement) {
                return Err(DoctrineError::UnresolvedReplacedBy(
                    rule.id.clone(),
                    replacement.clone(),
                ));
            }
        }
    }

    Ok(DoctrineDatabase {
        rules,
        by_id,
        by_domain,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixture: minimal valid rule file as TOML.
    const VALID_BUILD_RULES: &str = r#"
[meta]
doctrine_version = "1.0"
domain = "build"

[[rule]]
id        = "build-001"
name      = "Test rule"
domain    = "build"
statement = "Some statement."
rationale = "Some rationale."
enforcement = ["forge phase: test_phase"]
applies_to    = ["all crates"]
severity      = "strict"
lifecycle     = "stable"
"#;

    #[test]
    fn parses_valid_rule_file() {
        let parsed: RuleFile = toml::from_str(VALID_BUILD_RULES).expect("valid TOML parses");
        assert_eq!(parsed.meta.domain, Domain::Build);
        assert_eq!(parsed.rules.len(), 1);
        assert_eq!(parsed.rules[0].id, "build-001");
    }

    #[test]
    fn detects_incomplete_triple_missing_statement() {
        let toml_str = r#"
[meta]
doctrine_version = "1.0"
domain = "build"

[[rule]]
id        = "build-bad"
name      = "Bad rule"
domain    = "build"
statement = "   "
rationale = "Has rationale."
enforcement = ["forge phase: x"]
applies_to    = ["x"]
severity      = "strict"
lifecycle     = "stable"
"#;
        let tmp = tempfile_dir(&[("build.toml", toml_str)]);
        let err = load_from_dir(&tmp).unwrap_err();
        assert!(matches!(err, DoctrineError::IncompleteTriple(_, "statement")));
    }

    #[test]
    fn rejects_duplicate_ids_across_files() {
        let dup1 = r#"
[meta]
doctrine_version = "1.0"
domain = "build"

[[rule]]
id        = "shared-id"
name      = "A"
domain    = "build"
statement = "S."
rationale = "R."
enforcement = ["e"]
applies_to    = ["x"]
severity      = "strict"
lifecycle     = "stable"
"#;
        let dup2 = r#"
[meta]
doctrine_version = "1.0"
domain = "primitives"

[[rule]]
id        = "shared-id"
name      = "B"
domain    = "primitives"
statement = "S2."
rationale = "R2."
enforcement = ["e"]
applies_to    = ["y"]
severity      = "strict"
lifecycle     = "stable"
"#;
        let tmp = tempfile_dir(&[("build.toml", dup1), ("primitives.toml", dup2)]);
        let err = load_from_dir(&tmp).unwrap_err();
        assert!(matches!(err, DoctrineError::DuplicateId(_, _, _)));
    }

    #[test]
    fn deprecated_rule_requires_deprecated_at() {
        let toml_str = r#"
[meta]
doctrine_version = "1.0"
domain = "build"

[[rule]]
id        = "build-old"
name      = "Old rule"
domain    = "build"
statement = "S."
rationale = "R."
enforcement = ["e"]
applies_to    = ["x"]
severity      = "warn"
lifecycle     = "deprecated"
"#;
        let tmp = tempfile_dir(&[("build.toml", toml_str)]);
        let err = load_from_dir(&tmp).unwrap_err();
        assert!(matches!(err, DoctrineError::MissingDeprecatedAt(_)));
    }

    #[test]
    fn replaced_by_must_resolve() {
        let toml_str = r#"
[meta]
doctrine_version = "1.0"
domain = "build"

[[rule]]
id        = "build-old"
name      = "Old"
domain    = "build"
statement = "S."
rationale = "R."
enforcement = ["e"]
applies_to    = ["x"]
severity      = "warn"
lifecycle     = "deprecated"
deprecated_at = "2026-05-20"
replaced_by   = "build-new"
"#;
        let tmp = tempfile_dir(&[("build.toml", toml_str)]);
        let err = load_from_dir(&tmp).unwrap_err();
        assert!(matches!(err, DoctrineError::UnresolvedReplacedBy(_, _)));
    }

    #[test]
    fn missing_files_are_silently_skipped() {
        // Empty rules dir loads to empty database.
        let tmp = tempfile_dir(&[]);
        let db = load_from_dir(&tmp).expect("empty dir loads");
        assert_eq!(db.len(), 0);
        assert!(db.is_empty());
    }

    #[test]
    fn query_by_domain_filters_correctly() {
        let build = r#"
[meta]
doctrine_version = "1.0"
domain = "build"

[[rule]]
id        = "build-q-1"
name      = "B1"
domain    = "build"
statement = "S."
rationale = "R."
enforcement = ["e"]
applies_to    = ["x"]
severity      = "strict"
lifecycle     = "stable"
"#;
        let prim = r#"
[meta]
doctrine_version = "1.0"
domain = "primitives"

[[rule]]
id        = "prim-q-1"
name      = "P1"
domain    = "primitives"
statement = "S."
rationale = "R."
enforcement = ["e"]
applies_to    = ["x"]
severity      = "strict"
lifecycle     = "stable"
"#;
        let tmp = tempfile_dir(&[("build.toml", build), ("primitives.toml", prim)]);
        let db = load_from_dir(&tmp).expect("loads");
        assert_eq!(db.by_domain(Domain::Build).count(), 1);
        assert_eq!(db.by_domain(Domain::Primitives).count(), 1);
        assert_eq!(db.by_domain(Domain::Security).count(), 0);
        assert_eq!(db.by_id("build-q-1").map(|r| r.name.as_str()), Some("B1"));
        assert_eq!(db.by_id("nope"), None);
    }

    /// Helper: write the given `(filename, contents)` pairs under
    /// `<tmp>/doctrine/rules/` and return the tmp root path.
    fn tempfile_dir(files: &[(&str, &str)]) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let root = std::env::temp_dir().join(format!("doctrine-core-test-{pid}-{n}"));
        let rules_dir = root.join("doctrine").join("rules");
        std::fs::create_dir_all(&rules_dir).expect("mkdir rules");
        for (name, contents) in files {
            std::fs::write(rules_dir.join(name), contents).expect("write file");
        }
        root
    }
}
