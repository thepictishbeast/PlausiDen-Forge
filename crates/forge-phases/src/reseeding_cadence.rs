//! `reseeding_cadence` — flags sites whose fingerprint hasn't
//! changed in N consecutive builds.
//!
//! Task #249 per the variation-architecture spec. Where every
//! other variation gate refuses CHANGES that drift, this phase
//! refuses the OPPOSITE drift: composition that never evolves.
//! A site that ships the same primitive-distribution + variant-
//! signature mix build after build is calcifying; readers and
//! returning visitors experience monotony even if the snapshot
//! passes every other gate.
//!
//! Reads the on-disk provenance history (`reports/provenance-*.
//! json`). If the most recent N entries share the same
//! `fingerprint_commitment_hex`, emits a warn-by-default finding
//! that asks the operator to introduce composition variation.
//!
//! Silent when:
//!
//! * No `reports/` directory exists.
//! * Fewer than N provenance entries are on disk.
//! * Most recent N entries have at least 2 distinct commitments.
//! * `[reseeding_cadence] enforce = false` (default).
//!
//! ## forge.toml shape
//!
//! ```toml
//! [reseeding_cadence]
//! enforce = true
//! threshold = 20             # default; warns after 20 identical builds
//! strict = false             # if true, escalates to strict
//! ```
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * `#[non_exhaustive]` on phase struct.
//! * No unwrap/expect in non-test code.
//! * Pure walk over reports/.

use std::fs;
use std::path::{Path, PathBuf};

use forge_core::provenance::Provenance;
use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// `reseeding_cadence` phase implementation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct ReseedingCadencePhase;

const DEFAULT_THRESHOLD: usize = 20;

impl Phase for ReseedingCadencePhase {
    fn name(&self) -> &'static str {
        "reseeding_cadence"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let Some(cfg) = CadenceConfig::load(&ctx.root) else {
            return Ok(findings);
        };
        if !cfg.enforce {
            return Ok(findings);
        }
        let reports_dir = ctx.root.join("reports");
        if !reports_dir.is_dir() {
            return Ok(findings);
        }
        let entries = recent_provenance_entries(&reports_dir, cfg.threshold)
            .map_err(|e| BuildError::Io {
                context: format!("read provenance in {}", reports_dir.display()),
                source: e,
            })?;
        if entries.len() < cfg.threshold {
            return Ok(findings);
        }
        let first = &entries[0].fingerprint_commitment_hex;
        let all_same = entries
            .iter()
            .all(|e| &e.fingerprint_commitment_hex == first);
        if all_same {
            let short = if first.len() >= 16 { &first[..16] } else { first };
            let finding = if cfg.strict {
                Finding::strict(
                    self.name(),
                    reports_dir.display().to_string(),
                    format!(
                        "reseeding_cadence — last {} builds shared fingerprint `{short}…`; substrate composition is calcifying",
                        cfg.threshold
                    ),
                )
            } else {
                Finding::warn(
                    self.name(),
                    reports_dir.display().to_string(),
                    format!(
                        "reseeding_cadence — last {} builds shared fingerprint `{short}…`; substrate composition is calcifying",
                        cfg.threshold
                    ),
                )
            };
            findings.push(
                finding
                    .citing(["pattern-301"])
                    .why("a site that never evolves its composition shape will read as monotonous to returning visitors regardless of content freshness")
                    .fix("introduce composition variation: swap a primitive, alter a section's variant, add a new content type, OR raise the reseeding_cadence threshold if the calcification is intentional"),
            );
        }

        Ok(findings)
    }
}

#[derive(Debug, Clone)]
struct CadenceConfig {
    enforce: bool,
    threshold: usize,
    strict: bool,
}

impl CadenceConfig {
    fn load(root: &Path) -> Option<Self> {
        let body = fs::read_to_string(root.join("forge.toml")).ok()?;
        let value: toml::Value = toml::from_str(&body).ok()?;
        let section = value.get("reseeding_cadence")?.as_table()?;
        let enforce = section
            .get("enforce")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let threshold = section
            .get("threshold")
            .and_then(|v| v.as_integer())
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(DEFAULT_THRESHOLD);
        let strict = section
            .get("strict")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Some(Self {
            enforce,
            threshold,
            strict,
        })
    }
}

/// Collect the most-recent N provenance entries, sorted by
/// filename (provenance-<RFC3339>.json sort is monotonic).
fn recent_provenance_entries(
    reports_dir: &Path,
    n: usize,
) -> Result<Vec<Provenance>, std::io::Error> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(reports_dir)? {
        let entry = entry?;
        let p = entry.path();
        let name = p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if name.starts_with("provenance-") && name.ends_with(".json") {
            paths.push(p);
        }
    }
    paths.sort();
    // Most recent N.
    let start = paths.len().saturating_sub(n);
    let recent = &paths[start..];
    let mut out = Vec::new();
    for p in recent {
        let body = fs::read_to_string(p)?;
        if let Ok(prov) = serde_json::from_str::<Provenance>(&body) {
            out.push(prov);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "forge-reseeding-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("reports")).unwrap();
        p
    }

    fn ctx_for(root: &Path) -> BuildCtx {
        BuildCtx {
            root: root.to_path_buf(),
            static_dir: root.join("static"),
            mode: forge_core::BuildMode::Poc,
        }
    }

    fn write_provenance(root: &Path, ts: &str, fp: &str) {
        let p = serde_json::json!({
            "spec": "v1",
            "identity_hash": "",
            "fingerprint_commitment_hex": fp,
            "timestamp": ts,
            "site_id": "test",
            "tenant_id": "",
            "signature_b64": "",
        });
        fs::write(
            root.join("reports").join(format!("provenance-{ts}.json")),
            serde_json::to_string(&p).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn phase_silent_when_not_enforced() {
        let root = temp_root("not-enforced");
        fs::write(
            root.join("forge.toml"),
            "[reseeding_cadence]\nenforce = false\n",
        )
        .unwrap();
        write_provenance(&root, "2026-05-20T00:00:00Z", "abc");
        let findings = ReseedingCadencePhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_no_reports_dir() {
        let root = temp_root("no-reports");
        let _ = fs::remove_dir_all(root.join("reports"));
        fs::write(
            root.join("forge.toml"),
            "[reseeding_cadence]\nenforce = true\nthreshold = 3\n",
        )
        .unwrap();
        let findings = ReseedingCadencePhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_fewer_than_threshold_provenance_entries() {
        let root = temp_root("below-threshold");
        fs::write(
            root.join("forge.toml"),
            "[reseeding_cadence]\nenforce = true\nthreshold = 5\n",
        )
        .unwrap();
        for i in 0..3 {
            write_provenance(&root, &format!("2026-05-20T00:00:0{i}Z"), "same");
        }
        let findings = ReseedingCadencePhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_when_threshold_identical_provenance_entries() {
        let root = temp_root("calcified");
        fs::write(
            root.join("forge.toml"),
            "[reseeding_cadence]\nenforce = true\nthreshold = 3\n",
        )
        .unwrap();
        for i in 0..3 {
            write_provenance(&root, &format!("2026-05-20T00:00:0{i}Z"), "abc123def456");
        }
        let findings = ReseedingCadencePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("calcifying")),
            "expected calcified finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_recent_provenance_diverges() {
        let root = temp_root("diverged");
        fs::write(
            root.join("forge.toml"),
            "[reseeding_cadence]\nenforce = true\nthreshold = 3\n",
        )
        .unwrap();
        write_provenance(&root, "2026-05-20T00:00:00Z", "abc");
        write_provenance(&root, "2026-05-20T00:00:01Z", "abc");
        write_provenance(&root, "2026-05-20T00:00:02Z", "different");
        let findings = ReseedingCadencePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            !findings.iter().any(|f| f.message.contains("calcifying")),
            "shouldn't flag when diverged; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn strict_flag_escalates_finding() {
        let root = temp_root("strict");
        fs::write(
            root.join("forge.toml"),
            "[reseeding_cadence]\nenforce = true\nthreshold = 3\nstrict = true\n",
        )
        .unwrap();
        for i in 0..3 {
            write_provenance(&root, &format!("2026-05-20T00:00:0{i}Z"), "same");
        }
        let findings = ReseedingCadencePhase.run(&ctx_for(&root)).unwrap();
        assert!(!findings.is_empty());
        assert!(
            findings.iter().all(|f| f.severity == forge_core::Severity::Strict),
            "expected strict; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_uses_only_most_recent_threshold_entries() {
        let root = temp_root("recent-only");
        fs::write(
            root.join("forge.toml"),
            "[reseeding_cadence]\nenforce = true\nthreshold = 3\n",
        )
        .unwrap();
        // Older entries differ; only the 3 most recent share a hash.
        write_provenance(&root, "2026-05-20T00:00:01Z", "old1");
        write_provenance(&root, "2026-05-20T00:00:02Z", "old2");
        write_provenance(&root, "2026-05-20T00:00:03Z", "calcified");
        write_provenance(&root, "2026-05-20T00:00:04Z", "calcified");
        write_provenance(&root, "2026-05-20T00:00:05Z", "calcified");
        let findings = ReseedingCadencePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("calcifying")),
            "expected finding from recent 3; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
