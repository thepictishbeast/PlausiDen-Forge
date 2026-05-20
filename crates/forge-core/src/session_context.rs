//! `session_context` — structured context loading + caching.
//!
//! Task #285 per the MCP cluster (#284-#288). Aggregates the
//! load-bearing substrate state — identity declaration + recent
//! provenance entries + recent skill telemetry — into a single
//! payload that Claude / MCP clients fetch at session start. The
//! cache survives across calls within a configurable TTL so
//! orientation is fast.
//!
//! ## Why structured + cached
//!
//! Each Claude session asks the same orientation questions:
//! "what's the current identity? what fingerprint did we last
//! ship? what skills ran recently?". Without caching, every
//! query re-walks forge.toml + reports/ + JSONL files. With
//! caching, the answer is one fread of a small JSON blob.
//!
//! Cache is **fail-tolerant**: missing files fall through to
//! empty Vec; stale cache (older than TTL) is regenerated.
//!
//! ## API
//!
//! * [`SessionContext`] — the aggregated payload.
//! * [`load`] — compute fresh; never reads cache.
//! * [`load_cached`] — read from cache if fresh, else compute +
//!   write cache.
//! * [`invalidate_cache`] — delete the cache file (operator opt
//!   to force regeneration on next read).
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `#[non_exhaustive]` on every public type.
//! * No unwrap/expect in non-test code.
//! * Pure functions over filesystem reads — no network, no
//!   external process spawn.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::provenance::Provenance;
use crate::site_identity::SiteIdentity;
use crate::skill_telemetry::SkillInvocation;

/// Default cache TTL in seconds. Operators can override.
pub const DEFAULT_TTL_SECONDS: u64 = 60;

/// Default cache path under the build host's tmp dir.
pub fn default_cache_path() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push("forge-session-context.json");
    p
}

/// Aggregated session-orientation payload.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SessionContext {
    /// Captured-at timestamp (epoch seconds; cache freshness
    /// uses this).
    pub captured_at_epoch: u64,
    /// Captured-at ISO-8601 string for human display.
    pub captured_at: String,
    /// The site_identity declaration (default-shaped if absent).
    pub identity: SiteIdentity,
    /// SHA-256 hash of the [site_identity] section bytes
    /// (mirrors Provenance::identity_hash; computed up-front
    /// so consumers don't re-hash).
    pub identity_hash: String,
    /// Most recent provenance entries (oldest → newest).
    pub recent_provenance: Vec<Provenance>,
    /// Most recent skill-telemetry entries (oldest → newest).
    pub recent_telemetry: Vec<SkillInvocation>,
    /// Source root the context was computed against.
    pub root: String,
}

/// Options for building a context.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct LoadOptions {
    /// Maximum provenance entries to include. 0 = none.
    pub provenance_limit: usize,
    /// Maximum telemetry entries to include. 0 = none.
    pub telemetry_limit: usize,
}

impl Default for LoadOptions {
    fn default() -> Self {
        Self {
            provenance_limit: 20,
            telemetry_limit: 50,
        }
    }
}

impl LoadOptions {
    /// Construct with custom limits.
    #[must_use]
    pub fn new(provenance_limit: usize, telemetry_limit: usize) -> Self {
        Self {
            provenance_limit,
            telemetry_limit,
        }
    }
}

/// Compute a fresh session context. Never reads cache.
pub fn load(root: &Path, opts: &LoadOptions) -> SessionContext {
    let captured_at_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let identity = SiteIdentity::load(root).unwrap_or_default();
    let identity_hash = compute_identity_hash(root);
    let recent_provenance = if opts.provenance_limit == 0 {
        Vec::new()
    } else {
        load_recent_provenance(root, opts.provenance_limit)
    };
    let recent_telemetry = if opts.telemetry_limit == 0 {
        Vec::new()
    } else {
        load_recent_telemetry(root, opts.telemetry_limit)
    };

    SessionContext {
        captured_at_epoch,
        captured_at: format_rfc3339_utc(captured_at_epoch),
        identity,
        identity_hash,
        recent_provenance,
        recent_telemetry,
        root: root.display().to_string(),
    }
}

/// Read cache if fresh; else compute + write. Returns the
/// payload. Cache writes are best-effort — failures fall through
/// to the freshly-computed context. Uses [`default_cache_path`];
/// callers that need test isolation should use [`load_cached_at`].
pub fn load_cached(root: &Path, opts: &LoadOptions, ttl_seconds: u64) -> SessionContext {
    load_cached_at(root, opts, ttl_seconds, &default_cache_path())
}

/// Like [`load_cached`] but with a caller-supplied cache path.
/// Used by tests so each test owns its own cache file and
/// parallel test execution doesn't cross-contaminate.
pub fn load_cached_at(
    root: &Path,
    opts: &LoadOptions,
    ttl_seconds: u64,
    cache_path: &Path,
) -> SessionContext {
    if let Ok(body) = fs::read_to_string(cache_path) {
        if let Ok(cached) = serde_json::from_str::<SessionContext>(&body) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if ttl_seconds > 0
                && now.saturating_sub(cached.captured_at_epoch) < ttl_seconds
                && cached.root == root.display().to_string()
            {
                return cached;
            }
        }
    }
    let fresh = load(root, opts);
    if let Ok(body) = serde_json::to_string(&fresh) {
        let _ = fs::write(cache_path, body);
    }
    fresh
}

/// Delete the cache file.
pub fn invalidate_cache() -> Result<(), std::io::Error> {
    let path = default_cache_path();
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn compute_identity_hash(root: &Path) -> String {
    Provenance::compute(root, "", "", "", "")
        .map(|p| p.identity_hash)
        .unwrap_or_default()
}

fn load_recent_provenance(root: &Path, limit: usize) -> Vec<Provenance> {
    let reports_dir = root.join("reports");
    if !reports_dir.is_dir() {
        return Vec::new();
    }
    let mut paths: Vec<PathBuf> = match fs::read_dir(&reports_dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map_or(false, |n| n.starts_with("provenance-") && n.ends_with(".json"))
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    paths.sort();
    let start = paths.len().saturating_sub(limit);
    paths[start..]
        .iter()
        .filter_map(|p| fs::read_to_string(p).ok())
        .filter_map(|s| serde_json::from_str::<Provenance>(&s).ok())
        .collect()
}

fn load_recent_telemetry(root: &Path, limit: usize) -> Vec<SkillInvocation> {
    let path = root.join("reports/skill-telemetry.jsonl");
    let entries = crate::skill_telemetry::read_invocations(&path).unwrap_or_default();
    let start = entries.len().saturating_sub(limit);
    entries[start..].to_vec()
}

/// Format an epoch second timestamp as RFC-3339 UTC. Simple
/// fixed-width formatter; no chrono dep.
fn format_rfc3339_utc(epoch: u64) -> String {
    let days = epoch / 86400;
    let secs_in_day = epoch % 86400;
    let hour = secs_in_day / 3600;
    let minute = (secs_in_day % 3600) / 60;
    let second = secs_in_day % 60;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    )
}

/// Howard Hinnant's "civil from days" — converts day-count since
/// 1970-01-01 to (year, month, day). Public-domain algorithm,
/// works for any reasonable date range.
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z / 146097 } else { (z - 146096) / 146097 };
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = (y + i64::from(m <= 2)) as i32;
    (year, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attest::generate_keypair;
    use crate::provenance::Provenance;
    use crate::skill_telemetry::{append_invocation, SkillInvocation, SkillOutcome};

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "forge-session-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("reports")).unwrap();
        p
    }

    #[test]
    fn load_with_no_files_returns_empty_context() {
        let root = temp_root("empty");
        let ctx = load(&root, &LoadOptions::default());
        assert!(ctx.identity.is_default());
        assert!(ctx.identity_hash.is_empty());
        assert!(ctx.recent_provenance.is_empty());
        assert!(ctx.recent_telemetry.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_picks_up_site_identity() {
        let root = temp_root("with-identity");
        fs::write(
            root.join("forge.toml"),
            r#"
[site_identity]
site_id = "x"
tenant_id = "t"
"#,
        )
        .unwrap();
        let ctx = load(&root, &LoadOptions::default());
        assert_eq!(ctx.identity.site_id.as_deref(), Some("x"));
        assert_eq!(ctx.identity.tenant_id.as_deref(), Some("t"));
        assert!(!ctx.identity_hash.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_picks_up_recent_provenance() {
        let root = temp_root("with-prov");
        // Write 5 provenance files; ask for 3.
        for i in 0..5 {
            let p = Provenance::compute(
                &root,
                format!("fp-{i}"),
                format!("2026-05-20T00:00:0{i}Z"),
                "site",
                "",
            )
            .unwrap();
            let path = root
                .join("reports")
                .join(format!("provenance-2026-05-20T00-00-0{i}Z.json"));
            fs::write(&path, serde_json::to_string(&p).unwrap()).unwrap();
        }
        let ctx = load(&root, &LoadOptions::new(3, 0));
        assert_eq!(ctx.recent_provenance.len(), 3);
        // Most recent (last after sort) should be the 4th file (index 4).
        assert!(ctx.recent_provenance[2].fingerprint_commitment_hex.ends_with("4"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_picks_up_recent_telemetry() {
        let root = temp_root("with-tel");
        let path = root.join("reports/skill-telemetry.jsonl");
        for i in 0..7 {
            let inv = SkillInvocation::record(
                format!("skill-{i}"),
                format!("2026-05-20T00:00:0{i}Z"),
                format!("2026-05-20T00:00:0{i}Z"),
                100,
                SkillOutcome::Success,
            );
            append_invocation(&path, &inv).unwrap();
        }
        let ctx = load(&root, &LoadOptions::new(0, 4));
        assert_eq!(ctx.recent_telemetry.len(), 4);
        // Most recent (last) should be skill-6.
        assert_eq!(ctx.recent_telemetry[3].skill_id, "skill-6");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn limits_zero_skip_loading() {
        let root = temp_root("zero-limits");
        let path = root.join("reports/skill-telemetry.jsonl");
        let inv = SkillInvocation::record("s", "t", "t", 0, SkillOutcome::Success);
        append_invocation(&path, &inv).unwrap();
        let ctx = load(&root, &LoadOptions::new(0, 0));
        assert!(ctx.recent_provenance.is_empty());
        assert!(ctx.recent_telemetry.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn cache_writes_and_reads_back_within_ttl() {
        let root = temp_root("cache");
        let cache = root.join("test-cache.json");
        fs::write(
            root.join("forge.toml"),
            "[site_identity]\nsite_id = \"cached\"\n",
        )
        .unwrap();
        let _ = fs::remove_file(&cache);
        let ctx1 = load_cached_at(&root, &LoadOptions::default(), 60, &cache);
        assert_eq!(ctx1.identity.site_id.as_deref(), Some("cached"));

        // Mutate forge.toml — but within TTL, cache should still
        // return the old payload.
        fs::write(
            root.join("forge.toml"),
            "[site_identity]\nsite_id = \"changed\"\n",
        )
        .unwrap();
        let ctx2 = load_cached_at(&root, &LoadOptions::default(), 60, &cache);
        assert_eq!(ctx2.identity.site_id.as_deref(), Some("cached"));

        // Invalidate; next read picks up the change.
        let _ = fs::remove_file(&cache);
        let ctx3 = load_cached_at(&root, &LoadOptions::default(), 60, &cache);
        assert_eq!(ctx3.identity.site_id.as_deref(), Some("changed"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn cache_invalidated_when_ttl_zero() {
        let root = temp_root("ttl-zero");
        let cache = root.join("test-cache.json");
        fs::write(
            root.join("forge.toml"),
            "[site_identity]\nsite_id = \"a\"\n",
        )
        .unwrap();
        let _ = fs::remove_file(&cache);
        let _ = load_cached_at(&root, &LoadOptions::default(), 60, &cache);
        fs::write(
            root.join("forge.toml"),
            "[site_identity]\nsite_id = \"b\"\n",
        )
        .unwrap();
        // TTL 0 = always stale → re-compute.
        let ctx = load_cached_at(&root, &LoadOptions::default(), 0, &cache);
        assert_eq!(ctx.identity.site_id.as_deref(), Some("b"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn format_rfc3339_utc_produces_known_dates() {
        // 1970-01-01T00:00:00Z = epoch 0
        assert_eq!(format_rfc3339_utc(0), "1970-01-01T00:00:00Z");
        // 2026-05-20T00:00:00Z = ?
        // 56 years from 1970 to 2026 → 14 leap years (1972, 76, 80, 84, 88, 92, 96, 2000, 2004, 2008, 2012, 2016, 2020, 2024) = 14 leap
        // Days: 56*365 + 14 = 20440 + 14 = 20454. Plus 31 (Jan) + 28 + 31 + 30 = 120 days (2026 is non-leap) + 19 = 139. So 20454 + 139 = 20593.
        // 20593 * 86400 = 1779235200.
        assert_eq!(format_rfc3339_utc(1779235200), "2026-05-20T00:00:00Z");
    }
}
