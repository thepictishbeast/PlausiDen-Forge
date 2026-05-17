//! `path_consistency` — every `cms/<page>.json`'s `path` field
//! must resolve to a real `static/<file>.html` and follow the
//! canonical URL→file mapping.
//!
//! Rust port of bash `phase_path_consistency` (T57). Pure
//! analysis — no shell-out, no subprocess.
//!
//! ## Why
//!
//! Static servers (python http.server, nginx, Hetzner) serve
//! files literally. A CmsPage that declares `path="/compose"`
//! while the file is `static/compose.html` causes a 404 for
//! every visitor typing the URL. The auto-derived crawler
//! journey catches this at runtime; this phase catches it at
//! build time, where the operator can fix the typo without
//! waiting on the audit.
//!
//! ## Mapping rules
//!
//! ```text
//!   path "/"           → static/index.html
//!   path "/foo.html"   → static/foo.html
//!   path "/foo"        → STRICT (must include .html or trailing /)
//!   path "/foo/"       → static/foo/index.html
//! ```
//!
//! Bash version was lenient (`*) candidate="$STATIC$path.html"`
//! accepted `/compose` as if it were `/compose.html`); the Rust
//! port tightens to require either `.html` suffix, trailing `/`,
//! or root `/`. Documented bug class (T11): compose.json once
//! shipped `/compose` and the page silently 404'd until the
//! crawler caught it.
//!
//! ## Doctrine applied
//!
//! * **ADT findings** — `enum PathConsistencyFinding` per
//!   failure mode.
//! * **Value Object** — `CmsPath(String)` validated against
//!   `^/[A-Za-z0-9._/-]*$`.
//! * **Pure parsing** — `serde_json` to read the `path` field;
//!   no regex hacks.
//! * **deny `unsafe_code`**, no `unwrap`/`expect`.
//! * **Property-based tests** — mapping rules + path validator
//!   panic-free on arbitrary input.

use std::path::{Path, PathBuf};

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde::Deserialize;

// ============================================================
// Value Object
// ============================================================

/// Validated CMS path. Constrained to `/`-rooted segments of
/// `[A-Za-z0-9._/-]`. Rejects path traversal, backslashes,
/// whitespace, query strings, fragments — the visitor-facing URL
/// is just a path, not a full URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmsPath(String);

#[derive(Debug, PartialEq, Eq)]
pub enum CmsPathError {
    Empty,
    NoLeadingSlash,
    InvalidChar(char),
    Traversal,
}

impl std::fmt::Display for CmsPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("empty path"),
            Self::NoLeadingSlash => f.write_str("path must start with `/`"),
            Self::InvalidChar(c) => write!(
                f,
                "path contains invalid character {c:?} (allowed: A-Z a-z 0-9 . _ / -)"
            ),
            Self::Traversal => f.write_str("path contains `..` traversal segment"),
        }
    }
}

impl CmsPath {
    pub fn new(s: &str) -> Result<Self, CmsPathError> {
        if s.is_empty() {
            return Err(CmsPathError::Empty);
        }
        if !s.starts_with('/') {
            return Err(CmsPathError::NoLeadingSlash);
        }
        for c in s.chars() {
            if !(c.is_ascii_alphanumeric() || c == '/' || c == '.' || c == '_' || c == '-') {
                return Err(CmsPathError::InvalidChar(c));
            }
        }
        // SECURITY: explicit traversal check after char validation.
        // `..` is structurally allowed by the per-char rule (both
        // dots are valid), but we must reject any segment of `..`
        // to keep the resolver from escaping the static root.
        for segment in s.split('/') {
            if segment == ".." {
                return Err(CmsPathError::Traversal);
            }
        }
        Ok(Self(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Map a CmsPath to the expected static file path.
    /// Returns `None` if the path is structurally invalid for
    /// static-file serving (no `.html`, no trailing `/`, not
    /// the root).
    #[must_use]
    pub fn resolve_static(&self, static_dir: &Path) -> Option<PathBuf> {
        let p = &self.0;
        if p == "/" {
            return Some(static_dir.join("index.html"));
        }
        if let Some(rest) = p.strip_prefix('/') {
            if let Some(_html) = rest.strip_suffix(".html") {
                return Some(static_dir.join(rest));
            }
            if rest.ends_with('/') {
                return Some(static_dir.join(rest).join("index.html"));
            }
        }
        None
    }
}

// ============================================================
// ADT findings
// ============================================================

/// What the phase concluded about one CMS file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathConsistencyFinding {
    /// `path` field missing or unreadable.
    PathFieldMissing { source: String },
    /// `path` field present but fails `CmsPath::new` validation.
    PathInvalid {
        source: String,
        raw: String,
        why: String,
    },
    /// `path` doesn't follow the .html / / / "/" mapping rules.
    AmbiguousMapping { source: String, raw: String },
    /// `path` resolves but the target file doesn't exist.
    TargetMissing {
        source: String,
        raw: String,
        target: PathBuf,
    },
}

impl PathConsistencyFinding {
    pub fn as_finding(&self) -> Finding {
        const PHASE: &str = "path_consistency";
        match self {
            Self::PathFieldMissing { source } => Finding::strict(
                PHASE,
                source.clone(),
                "missing or unreadable 'path' field in CmsPage",
            ),
            Self::PathInvalid { source, raw, why } => Finding::strict(
                PHASE,
                source.clone(),
                format!("path {raw:?} invalid: {why}"),
            ),
            Self::AmbiguousMapping { source, raw } => Finding::strict(
                PHASE,
                source.clone(),
                format!(
                    "path={raw} → must end in .html, end in /, or be / \
                     (visitors hit 404 on ambiguous path; static server \
                     doesn't strip .html)"
                ),
            ),
            Self::TargetMissing {
                source,
                raw,
                target,
            } => Finding::strict(
                PHASE,
                source.clone(),
                format!(
                    "path={raw} → expected file at {} but it doesn't exist",
                    target.display()
                ),
            ),
        }
    }
}

// ============================================================
// JSON shape — we only care about the `path` field.
// ============================================================

#[derive(Deserialize)]
struct CmsPagePathField {
    #[serde(default)]
    path: Option<String>,
}

// ============================================================
// Phase impl
// ============================================================

#[derive(Debug, Default)]
pub struct PathConsistencyPhase;

impl Phase for PathConsistencyPhase {
    fn name(&self) -> &'static str {
        "path_consistency"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let cms_dir = ctx.root.join("cms");
        if !cms_dir.is_dir() {
            tracing::info!(?cms_dir, "path_consistency: no cms/ — skip");
            return Ok(vec![]);
        }
        let entries = std::fs::read_dir(&cms_dir).map_err(|source| BuildError::Io {
            context: format!("read_dir {}", cms_dir.display()),
            source,
        })?;
        let mut found = Vec::<PathConsistencyFinding>::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let source = path
                .strip_prefix(&ctx.root)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| path.display().to_string());
            let raw = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(_) => {
                    found.push(PathConsistencyFinding::PathFieldMissing { source });
                    continue;
                }
            };
            // Use a tiny shape-only struct so unknown fields in
            // future CMS evolutions don't break this phase.
            let parsed: CmsPagePathField = match serde_json::from_str(&raw) {
                Ok(p) => p,
                Err(_) => {
                    found.push(PathConsistencyFinding::PathFieldMissing { source });
                    continue;
                }
            };
            let Some(raw_path) = parsed.path.filter(|s| !s.is_empty()) else {
                found.push(PathConsistencyFinding::PathFieldMissing { source });
                continue;
            };
            let cms_path = match CmsPath::new(&raw_path) {
                Ok(p) => p,
                Err(e) => {
                    found.push(PathConsistencyFinding::PathInvalid {
                        source,
                        raw: raw_path,
                        why: e.to_string(),
                    });
                    continue;
                }
            };
            let Some(target) = cms_path.resolve_static(&ctx.static_dir) else {
                found.push(PathConsistencyFinding::AmbiguousMapping {
                    source,
                    raw: raw_path,
                });
                continue;
            };
            if !target.is_file() {
                found.push(PathConsistencyFinding::TargetMissing {
                    source,
                    raw: raw_path,
                    target,
                });
            }
        }
        Ok(found
            .iter()
            .map(PathConsistencyFinding::as_finding)
            .collect())
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::Severity;

    #[test]
    fn cms_path_accepts_valid_shapes() {
        for v in ["/", "/about.html", "/foo/", "/a-b/c.html", "/index.html"] {
            assert!(CmsPath::new(v).is_ok(), "should accept {v:?}");
        }
    }

    #[test]
    fn cms_path_rejects_invalid() {
        assert!(matches!(CmsPath::new(""), Err(CmsPathError::Empty)));
        assert!(matches!(
            CmsPath::new("about"),
            Err(CmsPathError::NoLeadingSlash)
        ));
        assert!(matches!(
            CmsPath::new("/a b"),
            Err(CmsPathError::InvalidChar(' '))
        ));
        assert!(matches!(
            CmsPath::new("/../etc"),
            Err(CmsPathError::Traversal)
        ));
        assert!(matches!(
            CmsPath::new("/foo?q"),
            Err(CmsPathError::InvalidChar('?'))
        ));
    }

    #[test]
    fn resolve_static_root() {
        let p = CmsPath::new("/").expect("valid");
        let r = p.resolve_static(Path::new("/s")).expect("resolves");
        assert_eq!(r, PathBuf::from("/s/index.html"));
    }

    #[test]
    fn resolve_static_explicit_html() {
        let p = CmsPath::new("/about.html").expect("valid");
        let r = p.resolve_static(Path::new("/s")).expect("resolves");
        assert_eq!(r, PathBuf::from("/s/about.html"));
    }

    #[test]
    fn resolve_static_trailing_slash() {
        let p = CmsPath::new("/blog/").expect("valid");
        let r = p.resolve_static(Path::new("/s")).expect("resolves");
        assert_eq!(r, PathBuf::from("/s/blog/index.html"));
    }

    #[test]
    fn resolve_static_ambiguous_returns_none() {
        let p = CmsPath::new("/about").expect("valid");
        // No .html, no trailing /, not root → can't safely map.
        assert!(p.resolve_static(Path::new("/s")).is_none());
    }

    #[test]
    fn finding_path_field_missing_is_strict() {
        let f = PathConsistencyFinding::PathFieldMissing {
            source: "cms/x.json".into(),
        }
        .as_finding();
        assert_eq!(f.severity, Severity::Strict);
    }

    #[test]
    fn finding_target_missing_is_strict() {
        let f = PathConsistencyFinding::TargetMissing {
            source: "cms/x.json".into(),
            raw: "/x.html".into(),
            target: PathBuf::from("/s/x.html"),
        }
        .as_finding();
        assert_eq!(f.severity, Severity::Strict);
        assert!(f.message.contains("doesn't exist"));
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// CmsPath validator must not panic on arbitrary input.
        #[test]
        fn validator_never_panics(s in ".{0,300}") {
            let _ = CmsPath::new(&s);
        }

        /// resolve_static must not panic on any valid CmsPath.
        #[test]
        fn resolver_never_panics(suffix in "[A-Za-z0-9./_-]{0,40}") {
            let raw = format!("/{suffix}");
            if let Ok(p) = CmsPath::new(&raw) {
                let _ = p.resolve_static(Path::new("/s"));
            }
        }

        /// Round-trip: any valid CmsPath survives as_str + new.
        #[test]
        fn round_trip(suffix in "[a-z0-9.-]{0,20}") {
            let raw = format!("/{suffix}");
            if let Ok(p1) = CmsPath::new(&raw) {
                let p2 = CmsPath::new(p1.as_str()).expect("round-trip");
                prop_assert_eq!(p1, p2);
            }
        }
    }
}
