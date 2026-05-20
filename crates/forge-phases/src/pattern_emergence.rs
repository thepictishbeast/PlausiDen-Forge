//! `pattern_emergence` — substrate-wide convergence detector.
//!
//! Task #276 per the variation-architecture spec. Where
//! uniqueness_gate (#233) refuses single-site collisions and
//! differentiation_budget (#237) catches lukewarm-everywhere
//! within-site failures, this phase reads the fingerprint
//! registry and flags emergent convergence: when many sites in
//! the registry are drifting toward the same shape, the substrate
//! itself is producing convergent output regardless of any single
//! site's gate-pass status.
//!
//! Per `[[per-tenant-corpora-doctrine]]` + `docs/VARIATION_
//! GUARANTEES.md`: the substrate's job is to keep tenants
//! distinct. If the registry shows convergence, that's a
//! substrate-level signal that the canonical defaults are too
//! restrictive OR operators are reaching for the same primitives
//! by default.
//!
//! ## What it measures
//!
//! Reads the cross-tenant fingerprint registry. Computes the
//! pairwise component_distance for every pair of cross-tenant
//! entries among the most recent N. Reports:
//!
//! * Average pairwise distance.
//! * Convergence rate: drop in average distance compared to an
//!   earlier window of entries.
//!
//! Flags when:
//!
//! * Average pairwise distance < `min_avg_distance` (default 6).
//! * OR convergence_rate > `max_convergence_rate` (default 0.25,
//!   meaning > 25% drop in inter-site distance over the window).
//!
//! Silent when:
//!
//! * Registry has fewer than N entries.
//! * `[pattern_emergence] enforce = false` (default).
//!
//! ## forge.toml shape
//!
//! ```toml
//! [pattern_emergence]
//! enforce = true
//! registry_path = "registry/fingerprints.jsonl"   # default
//! window_recent = 10
//! window_baseline = 20
//! min_avg_distance = 6
//! max_convergence_rate = 0.25
//! ```
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * `#[non_exhaustive]` on phase struct.
//! * No unwrap/expect in non-test code.

use std::fs;
use std::path::{Path, PathBuf};

use forge_core::fingerprint_registry::{read_all, FingerprintRegistryEntry};
use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// `pattern_emergence` phase implementation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct PatternEmergencePhase;

impl Phase for PatternEmergencePhase {
    fn name(&self) -> &'static str {
        "pattern_emergence"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let Some(cfg) = EmergenceConfig::load(&ctx.root) else {
            return Ok(findings);
        };
        if !cfg.enforce {
            return Ok(findings);
        }

        let registry_path = ctx.root.join(&cfg.registry_path);
        if !registry_path.exists() {
            return Ok(findings);
        }

        let entries = match read_all(&registry_path) {
            Ok(e) => e,
            Err(_) => return Ok(findings),
        };
        if entries.len() < cfg.window_recent {
            return Ok(findings);
        }

        let recent = take_last(&entries, cfg.window_recent);
        let recent_avg = avg_pairwise_distance(recent);

        if recent_avg < f64::from(cfg.min_avg_distance) {
            findings.push(
                Finding::warn(
                    self.name(),
                    registry_path.display().to_string(),
                    format!(
                        "pattern_emergence — recent {} registry entries have avg pairwise distance {:.2}; below floor {}",
                        cfg.window_recent, recent_avg, cfg.min_avg_distance
                    ),
                )
                .citing(["pattern-501"])
                .why("the platform-wide fingerprint registry shows the most-recent sites converging toward similar structured shapes; the substrate's canonical defaults may be too restrictive")
                .fix("review which primitive defaults or variant suggestions are driving convergence; consider expanding the primitive vocabulary in Loom OR widening the recommended-variants set"),
            );
        }

        if entries.len() >= cfg.window_baseline + cfg.window_recent {
            let baseline_window = take_window(&entries, cfg.window_baseline + cfg.window_recent, cfg.window_baseline);
            let baseline_avg = avg_pairwise_distance(baseline_window);
            if baseline_avg > 0.0 {
                let rate = (baseline_avg - recent_avg) / baseline_avg;
                if rate > cfg.max_convergence_rate {
                    findings.push(
                        Finding::warn(
                            self.name(),
                            registry_path.display().to_string(),
                            format!(
                                "pattern_emergence — convergence rate {:.2} (baseline avg distance {:.2} → recent avg {:.2}) exceeds threshold {:.2}",
                                rate, baseline_avg, recent_avg, cfg.max_convergence_rate
                            ),
                        )
                        .citing(["pattern-502"])
                        .why("inter-site distance is dropping over time; the substrate's recent outputs are more alike than its earlier outputs were")
                        .fix("audit substrate evolution: did a new primitive default land that everyone reaches for? Was a variant suggestion narrowed? Reverse the change OR diversify the substrate canon"),
                    );
                }
            }
        }

        Ok(findings)
    }
}

#[derive(Debug, Clone)]
struct EmergenceConfig {
    enforce: bool,
    registry_path: PathBuf,
    window_recent: usize,
    window_baseline: usize,
    min_avg_distance: u32,
    max_convergence_rate: f64,
}

impl EmergenceConfig {
    fn load(root: &Path) -> Option<Self> {
        let body = fs::read_to_string(root.join("forge.toml")).ok()?;
        let value: toml::Value = toml::from_str(&body).ok()?;
        let section = value.get("pattern_emergence")?.as_table()?;
        let enforce = section
            .get("enforce")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let registry_path = section
            .get("registry_path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("registry/fingerprints.jsonl"));
        let window_recent = section
            .get("window_recent")
            .and_then(|v| v.as_integer())
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(10);
        let window_baseline = section
            .get("window_baseline")
            .and_then(|v| v.as_integer())
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(20);
        let min_avg_distance = section
            .get("min_avg_distance")
            .and_then(|v| v.as_integer())
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(6);
        let max_convergence_rate = section
            .get("max_convergence_rate")
            .and_then(|v| v.as_float())
            .unwrap_or(0.25);
        Some(Self {
            enforce,
            registry_path,
            window_recent,
            window_baseline,
            min_avg_distance,
            max_convergence_rate,
        })
    }
}

fn take_last(entries: &[FingerprintRegistryEntry], n: usize) -> &[FingerprintRegistryEntry] {
    let start = entries.len().saturating_sub(n);
    &entries[start..]
}

fn take_window(entries: &[FingerprintRegistryEntry], offset_from_end: usize, n: usize) -> &[FingerprintRegistryEntry] {
    let end = entries.len().saturating_sub(offset_from_end - n);
    let start = end.saturating_sub(n);
    &entries[start..end]
}

fn avg_pairwise_distance(entries: &[FingerprintRegistryEntry]) -> f64 {
    if entries.len() < 2 {
        return 0.0;
    }
    let mut sum: u64 = 0;
    let mut count: u64 = 0;
    for (i, a) in entries.iter().enumerate() {
        for b in entries.iter().skip(i + 1) {
            let d = a.fingerprint.component_distance(&b.fingerprint);
            if d == u32::MAX {
                continue;
            }
            sum = sum.saturating_add(u64::from(d));
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        sum as f64 / count as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::attest::generate_keypair;
    use forge_core::fingerprint::{
        AssetDistribution, ContentSilhouette, FingerprintSpec, PrimitiveOccurrence,
        SiteFingerprint,
    };
    use forge_core::fingerprint_registry::append;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "forge-emergence-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("registry")).unwrap();
        p
    }

    fn ctx_for(root: &Path) -> BuildCtx {
        BuildCtx {
            root: root.to_path_buf(),
            static_dir: root.join("static"),
            mode: forge_core::BuildMode::Poc,
        }
    }

    fn sample_fingerprint(salt: u32) -> SiteFingerprint {
        let mut sil = BTreeMap::new();
        sil.insert(
            format!("page-{salt}"),
            ContentSilhouette::new(salt, salt, salt, "h1"),
        );
        SiteFingerprint::new(
            FingerprintSpec::V1,
            vec![PrimitiveOccurrence::new(
                "hero_editorial",
                format!("background=v{salt}"),
                format!("page-{salt}"),
            )],
            Vec::new(),
            sil,
            BTreeMap::new(),
            AssetDistribution::default(),
        )
    }

    fn seed_registry(root: &Path, count: usize, salt_offset: u32) {
        let key = generate_keypair();
        for i in 0..count {
            append(
                &root.join("registry/fingerprints.jsonl"),
                &format!("site-{i}-{salt_offset}"),
                "tenant",
                sample_fingerprint(salt_offset + i as u32),
                &format!("2026-05-20T00:00:{:02}Z", i),
                &key,
            )
            .unwrap();
        }
    }

    #[test]
    fn phase_silent_when_not_enforced() {
        let root = temp_root("not-enforced");
        fs::write(
            root.join("forge.toml"),
            "[pattern_emergence]\nenforce = false\n",
        )
        .unwrap();
        seed_registry(&root, 15, 0);
        let findings = PatternEmergencePhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_registry_below_window() {
        let root = temp_root("below-window");
        fs::write(
            root.join("forge.toml"),
            "[pattern_emergence]\nenforce = true\nwindow_recent = 10\n",
        )
        .unwrap();
        seed_registry(&root, 5, 0);
        let findings = PatternEmergencePhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn avg_pairwise_distance_zero_for_single_entry() {
        let key = generate_keypair();
        let path = std::env::temp_dir().join(format!("emerge-single-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        append(&path, "s", "t", sample_fingerprint(1), "2026-05-20T12:00:00Z", &key).unwrap();
        let entries = read_all(&path).unwrap();
        assert_eq!(avg_pairwise_distance(&entries), 0.0);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn avg_pairwise_distance_positive_for_distinct_sites() {
        let key = generate_keypair();
        let path = std::env::temp_dir().join(format!("emerge-distinct-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        for i in 0..5 {
            let ts = format!("2026-05-20T12:00:{i:02}Z");
            append(&path, &format!("s{i}"), "t", sample_fingerprint(i as u32 + 10), &ts, &key).unwrap();
        }
        let entries = read_all(&path).unwrap();
        let avg = avg_pairwise_distance(&entries);
        assert!(avg > 0.0, "5 distinct sites should have positive avg distance");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn phase_flags_low_avg_distance_below_floor() {
        let root = temp_root("low-distance");
        fs::write(
            root.join("forge.toml"),
            r#"
[pattern_emergence]
enforce = true
window_recent = 5
min_avg_distance = 100
"#,
        )
        .unwrap();
        seed_registry(&root, 5, 0);
        let findings = PatternEmergencePhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("below floor")),
            "expected min-avg-distance finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
