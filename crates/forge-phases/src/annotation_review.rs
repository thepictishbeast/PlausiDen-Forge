//! `annotation_review` — consume PlausiDen-Annotator session JSON
//! and surface operator-flagged elements as Forge findings.
//!
//! Closes the human-in-the-loop gap between Forge audit (machine)
//! and Annotator capture (human). Operator inspects a site via
//! the annotator bookmarklet or relay, flags problems with typed
//! tags + comments + selectors; this phase reads those sessions
//! and lifts the findings into the same `BuildReport` shape every
//! other phase emits. Operator decisions become a first-class
//! signal in the build pipeline.
//!
//! ## Configuration
//!
//! Reads `[review]` from `forge.toml`:
//!
//! ```toml
//! [review]
//! # Filesystem path to a directory of session JSON files. Each
//! # `*.json` deserializes as an Annotator session (schema_version 1).
//! # The annotator-relay's storage root is the canonical input.
//! session_dir = "./annotator-data"
//! ```
//!
//! Missing config or missing directory → silent skip (the phase
//! doesn't fail builds that haven't opted into operator review).
//! Malformed JSON → `BuildError::Other` (loud failure — silently
//! eating a malformed operator session would discard real signal).
//!
//! ## Severity mapping
//!
//! Annotator's tag enum (per `examples/sample-session.json`) maps
//! to Forge severity per the table below. Stricter tags map to
//! `Severity::Strict` so production builds fail when an operator
//! has documented a real issue; softer tags map to `Severity::Warn`
//! so they surface in poc-mode without blocking.
//!
//! | Tag          | Severity | Rationale                                       |
//! |--------------|----------|-------------------------------------------------|
//! | `a11y`       | Strict   | Accessibility regressions are non-negotiable     |
//! | `contrast`   | Strict   | WCAG AA contrast violations are concrete         |
//! | `bug`        | Strict   | Operator flagged a real bug worth blocking on   |
//! | `alignment`  | Warn     | Visual polish; tolerable in poc, fix before prod |
//! | `copy`       | Warn     | Wording change; rarely build-blocking            |
//! | `perf`       | Warn     | Performance findings flow through `perf_budget`  |
//! | `suggestion` | Warn     | Soft signal from operator's taste                |
//! | `other`      | Warn     | Unclassified; default to Warn for safety         |
//!
//! Unknown tags (forward-compat for schema_version > 1) default
//! to `Warn` rather than panicking — the phase is liberal in what
//! it accepts so older Forges can still process newer sessions.
//!
//! ## Path attribution
//!
//! Annotator sessions carry `meta.url` (the full URL the operator
//! was inspecting). The phase extracts the path portion and uses
//! that as the `Finding.path` so findings cluster per-page in the
//! BuildReport. URLs that fail to parse fall back to `"annotator/"`
//! as a synthetic source — the finding still surfaces, just not
//! grouped with a specific page.

use std::path::{Path, PathBuf};

use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde::{Deserialize, Serialize};

/// `annotation_review` phase.
#[derive(Debug, Default)]
pub struct AnnotationReviewPhase;

impl Phase for AnnotationReviewPhase {
    fn name(&self) -> &'static str {
        "annotation_review"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let Some(session_dir) = forge_toml_session_dir(&ctx.root) else {
            tracing::debug!("annotation_review: no [review] session_dir configured — skip");
            return Ok(vec![]);
        };
        let session_dir = if session_dir.is_absolute() {
            session_dir
        } else {
            ctx.root.join(&session_dir)
        };
        if !session_dir.is_dir() {
            tracing::info!(
                ?session_dir,
                "annotation_review: configured session_dir not present — skip"
            );
            return Ok(vec![]);
        }

        let mut findings = Vec::new();
        let entries = std::fs::read_dir(&session_dir).map_err(|source| BuildError::Io {
            context: format!("annotation_review: read {}", session_dir.display()),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| BuildError::Io {
                context: format!("annotation_review: iterate {}", session_dir.display()),
                source,
            })?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let body = std::fs::read_to_string(&path).map_err(|source| BuildError::Io {
                context: format!("annotation_review: read {}", path.display()),
                source,
            })?;
            let session: Session = serde_json::from_str(&body).map_err(|e| BuildError::Other {
                phase: "annotation_review".into(),
                message: format!("session {} malformed: {e}", path.display()),
            })?;
            for annotation in session.annotations {
                findings.push(annotation_to_finding(&session.meta, annotation));
            }
        }
        Ok(findings)
    }
}

/// Captured Annotator session (schema_version 1).
///
/// Matches `PlausiDen-Annotator/examples/sample-session.json`.
/// `#[serde(default)]` on optional fields lets us forward-compat
/// with future schema versions that drop fields without breaking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Session {
    #[serde(default)]
    schema_version: u32,
    meta: SessionMeta,
    #[serde(default)]
    annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SessionMeta {
    /// Page URL the operator was inspecting. Used for path attribution.
    url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Annotation {
    id: String,
    tag: String,
    comment: String,
    element: AnnotationElement,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AnnotationElement {
    selector: String,
}

/// Map an Annotator tag string to Forge severity per the doc-
/// header table. Unknown tags default to `Warn`.
fn severity_for_tag(tag: &str) -> Severity {
    match tag {
        "a11y" | "contrast" | "bug" => Severity::Strict,
        "alignment" | "copy" | "perf" | "suggestion" | "other" => Severity::Warn,
        _ => Severity::Warn,
    }
}

/// Local mirror of forge-core's Severity so the mapping table
/// is dense and inspectable here. The Phase return type still
/// uses `Finding` which carries `forge_core::Severity`; this enum
/// is just an internal classifier with a single conversion site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Severity {
    Strict,
    Warn,
}

/// Convert one captured annotation into a Forge `Finding`.
fn annotation_to_finding(meta: &SessionMeta, ann: Annotation) -> Finding {
    let path = path_from_url(&meta.url);
    let msg = format!(
        "[{tag}] {comment} (selector: {selector})",
        tag = ann.tag,
        comment = ann.comment,
        selector = ann.element.selector,
    );
    match severity_for_tag(&ann.tag) {
        Severity::Strict => Finding::strict("annotation_review", path, msg),
        Severity::Warn => Finding::warn("annotation_review", path, msg),
    }
}

/// Extract the path portion of a URL for `Finding.path`. URLs
/// that fail to parse degrade to a synthetic `"annotator/"` so
/// the finding still surfaces.
fn path_from_url(url: &str) -> String {
    // Lightweight extraction — full URL parsing would pull in the
    // `url` crate which isn't currently a forge-phases dep. The
    // bash-port style is `scheme://host[:port]/path` so taking
    // everything from the first `/` after the authority works.
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    match after_scheme.find('/') {
        Some(idx) => after_scheme[idx..].to_owned(),
        None => "annotator/".to_owned(),
    }
}

/// Read `[review] session_dir = "..."` from `<root>/forge.toml`.
/// Returns `None` if file missing, parse error, key absent, or
/// value is not a string.
fn forge_toml_session_dir(root: &Path) -> Option<PathBuf> {
    let path = root.join("forge.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    let parsed: toml::Value = content.parse().ok()?;
    let dir = parsed
        .get("review")
        .and_then(|r| r.get("session_dir"))
        .and_then(|d| d.as_str())?;
    Some(PathBuf::from(dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::Severity as ForgeSeverity;

    fn meta(url: &str) -> SessionMeta {
        SessionMeta { url: url.into() }
    }

    fn ann(tag: &str, comment: &str, selector: &str) -> Annotation {
        Annotation {
            id: "a1".into(),
            tag: tag.into(),
            comment: comment.into(),
            element: AnnotationElement {
                selector: selector.into(),
            },
        }
    }

    #[test]
    fn severity_strict_for_a11y_family() {
        assert_eq!(severity_for_tag("a11y"), Severity::Strict);
        assert_eq!(severity_for_tag("contrast"), Severity::Strict);
        assert_eq!(severity_for_tag("bug"), Severity::Strict);
    }

    #[test]
    fn severity_warn_for_soft_tags() {
        for tag in ["alignment", "copy", "perf", "suggestion", "other"] {
            assert_eq!(severity_for_tag(tag), Severity::Warn, "tag: {tag}");
        }
    }

    #[test]
    fn severity_warn_for_unknown_tag_forward_compat() {
        // Schema v2 might add new tags; we default to Warn rather
        // than panicking so older Forges still process newer
        // sessions liberally.
        assert_eq!(severity_for_tag("future-tag"), Severity::Warn);
    }

    #[test]
    fn url_path_extraction_typical() {
        assert_eq!(path_from_url("https://example.com/admin"), "/admin");
        assert_eq!(
            path_from_url("http://example.com:8080/path/to/page"),
            "/path/to/page"
        );
    }

    #[test]
    fn url_path_extraction_no_scheme() {
        assert_eq!(path_from_url("example.com/page"), "/page");
    }

    #[test]
    fn url_path_extraction_degrades_when_no_path() {
        assert_eq!(path_from_url("https://example.com"), "annotator/");
    }

    #[test]
    fn finding_renders_message_with_tag_selector_comment() {
        let f = annotation_to_finding(
            &meta("https://example.com/admin"),
            ann("contrast", "Too low against gradient", "button.cta"),
        );
        assert_eq!(f.phase, "annotation_review");
        assert_eq!(f.path, "/admin");
        assert_eq!(f.severity, ForgeSeverity::Strict);
        assert!(f.message.contains("[contrast]"));
        assert!(f.message.contains("Too low against gradient"));
        assert!(f.message.contains("button.cta"));
    }

    #[test]
    fn finding_severity_warn_for_alignment() {
        let f = annotation_to_finding(
            &meta("https://example.com/sidebar"),
            ann("alignment", "Off-grid", "h3.section-heading"),
        );
        assert_eq!(f.severity, ForgeSeverity::Warn);
    }

    #[test]
    fn empty_session_produces_no_findings() {
        let dir = tempfile::tempdir().expect("tempdir");
        let payload = serde_json::json!({
            "schema_version": 1,
            "meta": {"url": "https://example.com/p1"},
            "annotations": [],
        });
        std::fs::write(dir.path().join("session-empty.json"), payload.to_string()).unwrap();
        std::fs::write(
            dir.path().join("forge.toml"),
            "[review]\nsession_dir = \".\"\n",
        )
        .unwrap();
        let ctx = BuildCtx {
            root: dir.path().to_path_buf(),
            static_dir: dir.path().to_path_buf(),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = AnnotationReviewPhase.run(&ctx).expect("run");
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn multi_annotation_session_produces_per_annotation_findings() {
        let dir = tempfile::tempdir().expect("tempdir");
        let payload = serde_json::json!({
            "schema_version": 1,
            "meta": {"url": "https://example.com/admin"},
            "annotations": [
                {
                    "id": "a1",
                    "tag": "contrast",
                    "comment": "Fails WCAG AA",
                    "element": {"selector": "button.cta"}
                },
                {
                    "id": "a2",
                    "tag": "alignment",
                    "comment": "Off-grid",
                    "element": {"selector": "h3"}
                },
                {
                    "id": "a3",
                    "tag": "bug",
                    "comment": "Dropdown reopens",
                    "element": {"selector": ".dropdown"}
                },
            ],
        });
        std::fs::write(dir.path().join("session.json"), payload.to_string()).unwrap();
        std::fs::write(
            dir.path().join("forge.toml"),
            "[review]\nsession_dir = \".\"\n",
        )
        .unwrap();
        let ctx = BuildCtx {
            root: dir.path().to_path_buf(),
            static_dir: dir.path().to_path_buf(),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = AnnotationReviewPhase.run(&ctx).expect("run");
        assert_eq!(findings.len(), 3);

        let strict_count = findings
            .iter()
            .filter(|f| f.severity == ForgeSeverity::Strict)
            .count();
        assert_eq!(
            strict_count, 2,
            "contrast + bug should be strict; alignment should be warn"
        );

        // All findings attribute to /admin
        for f in &findings {
            assert_eq!(f.path, "/admin");
            assert_eq!(f.phase, "annotation_review");
        }
    }

    #[test]
    fn missing_session_dir_silent_skip() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("forge.toml"),
            "[review]\nsession_dir = \"./nope\"\n",
        )
        .unwrap();
        let ctx = BuildCtx {
            root: dir.path().to_path_buf(),
            static_dir: dir.path().to_path_buf(),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = AnnotationReviewPhase.run(&ctx).expect("run");
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn missing_forge_toml_silent_skip() {
        let dir = tempfile::tempdir().expect("tempdir");
        // No forge.toml at all
        let ctx = BuildCtx {
            root: dir.path().to_path_buf(),
            static_dir: dir.path().to_path_buf(),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = AnnotationReviewPhase.run(&ctx).expect("run");
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn malformed_session_returns_build_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("session.json"), "{not valid json").unwrap();
        std::fs::write(
            dir.path().join("forge.toml"),
            "[review]\nsession_dir = \".\"\n",
        )
        .unwrap();
        let ctx = BuildCtx {
            root: dir.path().to_path_buf(),
            static_dir: dir.path().to_path_buf(),
            mode: forge_core::BuildMode::Poc,
        };
        let result = AnnotationReviewPhase.run(&ctx);
        assert!(
            matches!(result, Err(BuildError::Other { .. })),
            "malformed session must surface as BuildError, not be silently dropped"
        );
    }
}
