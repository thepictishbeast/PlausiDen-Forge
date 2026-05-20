//! `uniqueness_gate` — refuses builds whose fingerprint collides
//! with an entry already in the registry, or whose component
//! distance to an existing fingerprint is below threshold.
//!
//! Task #233 per the variation-architecture spec. First consumer
//! of `forge_core::fingerprint` (#231) + `forge_core::
//! fingerprint_registry` (#232) + `forge_core::site_identity`
//! (#234). Read-only against the registry — the gate verifies;
//! a separate `forge fingerprint register` CLI command writes
//! (kept separate so the gate phase doesn't need a signing key).
//!
//! ## What it does
//!
//! When `[uniqueness_gate] enforce = true` in `forge.toml`:
//!
//! 1. Compute a `SiteFingerprint` from the current build's
//!    `cms/*.json` (primitive occurrences + per-page silhouettes +
//!    asset distribution).
//! 2. Resolve the registry path (default `registry/fingerprints.jsonl`
//!    under the project root; overridable via
//!    `[uniqueness_gate].registry_path`).
//! 3. Look up the fingerprint's exact commitment hex in the
//!    registry.
//!    * Match found AND the matched entry's `site_id` differs from
//!      the current site's `site_id` (from `[site_identity]`)
//!      → strict finding (collision).
//!    * Match found AND `site_id` matches → silent (a rebuild of
//!      the same site).
//! 4. Scan for near-duplicates with `component_distance` ≤ the
//!    configured threshold (default 4).
//!    * Each match emits a strict finding when scope = `platform`,
//!      a warn when scope = `tenant` and the matched entry's
//!      tenant differs from the current tenant.
//!
//! ## forge.toml config
//!
//! ```toml
//! [uniqueness_gate]
//! enforce = true
//! # registry_path = "registry/fingerprints.jsonl"  # default
//! # near_duplicate_threshold = 4                   # default
//! # scope = "platform"                             # or "tenant"
//! ```
//!
//! Without the section the phase is silent — back-compat for
//! sites that haven't migrated yet.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * `#[non_exhaustive]` on the phase struct.
//! * No unwrap/expect in non-test code.
//! * Pure walk over JSON + read-only registry I/O — never mutates
//!   the registry from inside a phase.

use std::fs;
use std::path::{Path, PathBuf};

use forge_core::fingerprint::SiteFingerprint;
use forge_core::fingerprint_registry::{find_by_hash, find_near_duplicates};
use forge_core::site_identity::SiteIdentity;
use forge_core::{BuildCtx, BuildError, Finding, Phase};
#[cfg(test)]
use serde_json::Value;

/// `uniqueness_gate` phase implementation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct UniquenessGatePhase;

const DEFAULT_THRESHOLD: u32 = 4;
const DEFAULT_REGISTRY_PATH: &str = "registry/fingerprints.jsonl";

impl Phase for UniquenessGatePhase {
    fn name(&self) -> &'static str {
        "uniqueness_gate"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let Some(cfg) = UniquenessConfig::load(&ctx.root) else {
            return Ok(findings);
        };
        if !cfg.enforce {
            return Ok(findings);
        }

        // Build the fingerprint from cms/*.json.
        let cms_dir = ctx.root.join("cms");
        if !cms_dir.is_dir() {
            return Ok(findings);
        }
        let fingerprint = build_fingerprint(&cms_dir)?;

        // Resolve site identity (optional) — used for site-id matching.
        let identity = SiteIdentity::load(&ctx.root).unwrap_or_default();
        let our_site_id = identity.site_id.clone().unwrap_or_default();
        let our_tenant_id = identity.tenant_id.clone().unwrap_or_default();

        // Resolve registry path.
        let registry_path = ctx.root.join(&cfg.registry_path);
        if !registry_path.exists() {
            // Registry doesn't exist yet — fail-open. First build
            // establishes the registry. Operators set this up via
            // `forge fingerprint register`.
            return Ok(findings);
        }

        let hex = fingerprint.commitment_hex();
        // Exact-hash collision check.
        match find_by_hash(&registry_path, &hex) {
            Ok(Some(entry)) => {
                if entry.site_id != our_site_id {
                    findings.push(
                        Finding::strict(
                            self.name(),
                            registry_path.display().to_string(),
                            format!(
                                "uniqueness_gate — exact-fingerprint collision with site `{}` (commitment {}); two distinct sites produced identical structured fingerprints",
                                entry.site_id, hex
                            ),
                        )
                        .citing(["var-001"])
                        .why("the current build's structured fingerprint exactly matches another site already in the registry — the substrate's cross-site uniqueness guarantee is being violated")
                        .fix("introduce structural variation: swap primitives, alter composition rhythm, change content silhouette, or declare a distinct site_identity")
                        .skill("variation-resolution"),
                    );
                }
            }
            Ok(None) => {}
            Err(e) => {
                findings.push(Finding::warn(
                    self.name(),
                    registry_path.display().to_string(),
                    format!("uniqueness_gate — registry lookup failed: {e}"),
                ));
            }
        }

        // Near-duplicate scan.
        match find_near_duplicates(&registry_path, &fingerprint, cfg.threshold) {
            Ok(matches) => {
                for (entry, distance) in matches {
                    // Skip self-matches.
                    if entry.site_id == our_site_id {
                        continue;
                    }
                    // Exact-match was already emitted above; skip distance-0 self-emit.
                    if distance == 0 {
                        continue;
                    }
                    let (severity_label, finding) = match cfg.scope {
                        UniquenessScope::Platform => (
                            "strict",
                            Finding::strict(
                                self.name(),
                                registry_path.display().to_string(),
                                format!(
                                    "uniqueness_gate — near-duplicate of site `{}` (distance {} ≤ threshold {}); platform-wide convergence detected",
                                    entry.site_id, distance, cfg.threshold
                                ),
                            ),
                        ),
                        UniquenessScope::Tenant => {
                            // Tenant scope: only strict when within the same tenant.
                            if entry.tenant_id == our_tenant_id && !our_tenant_id.is_empty() {
                                (
                                    "strict",
                                    Finding::strict(
                                        self.name(),
                                        registry_path.display().to_string(),
                                        format!(
                                            "uniqueness_gate — near-duplicate of site `{}` within tenant `{}` (distance {} ≤ threshold {})",
                                            entry.site_id, entry.tenant_id, distance, cfg.threshold
                                        ),
                                    ),
                                )
                            } else {
                                (
                                    "warn",
                                    Finding::warn(
                                        self.name(),
                                        registry_path.display().to_string(),
                                        format!(
                                            "uniqueness_gate — near-duplicate of cross-tenant site `{}` (distance {} ≤ threshold {}); tenant-scoped gate doesn't refuse cross-tenant similarity",
                                            entry.site_id, distance, cfg.threshold
                                        ),
                                    ),
                                )
                            }
                        }
                    };
                    let _ = severity_label;
                    findings.push(
                        finding
                            .citing(["var-002"])
                            .why("the build's fingerprint is too close to an existing site's; the substrate's differentiation budget is being violated")
                            .fix("vary primitive selection, composition rhythm, or content silhouette to push distance beyond threshold"),
                    );
                }
            }
            Err(e) => {
                findings.push(Finding::warn(
                    self.name(),
                    registry_path.display().to_string(),
                    format!("uniqueness_gate — near-duplicate scan failed: {e}"),
                ));
            }
        }

        Ok(findings)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UniquenessScope {
    Platform,
    Tenant,
}

#[derive(Debug, Clone)]
struct UniquenessConfig {
    enforce: bool,
    registry_path: PathBuf,
    threshold: u32,
    scope: UniquenessScope,
}

impl UniquenessConfig {
    fn load(root: &Path) -> Option<Self> {
        let body = fs::read_to_string(root.join("forge.toml")).ok()?;
        let value: toml::Value = toml::from_str(&body).ok()?;
        let section = value.get("uniqueness_gate")?.as_table()?;
        let enforce = section
            .get("enforce")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let registry_path = section
            .get("registry_path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_REGISTRY_PATH));
        let threshold = section
            .get("near_duplicate_threshold")
            .and_then(|v| v.as_integer())
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(DEFAULT_THRESHOLD);
        let scope = match section
            .get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("platform")
        {
            "tenant" => UniquenessScope::Tenant,
            _ => UniquenessScope::Platform,
        };
        Some(Self {
            enforce,
            registry_path,
            threshold,
            scope,
        })
    }
}

/// Walk `cms/*.json` and compute a `SiteFingerprint`. Delegates
/// to `forge_core::fingerprint::build_from_cms_dir` so the gate
/// phase and the `forge fingerprint compute` CLI subcommand
/// share a single source of truth.
fn build_fingerprint(cms_dir: &Path) -> Result<SiteFingerprint, BuildError> {
    forge_core::fingerprint::build_from_cms_dir(cms_dir).map_err(|e| BuildError::Io {
        context: format!("build_from_cms_dir {}", cms_dir.display()),
        source: e,
    })
}

/// Re-exports for backward compatibility with this module's tests.
/// `#[cfg(test)]` because non-test callers should hit forge-core
/// directly via `forge_core::fingerprint::*`.
#[cfg(test)]
fn guess_variant(_kind: &str, section: &Value) -> String {
    forge_core::fingerprint::guess_section_variant(section)
}

#[cfg(test)]
fn bucket_chars(n: u32) -> u32 {
    forge_core::fingerprint::bucket_chars(n)
}

#[cfg(test)]
fn density_tier_for(section_count: u32, total_chars: u32) -> &'static str {
    forge_core::fingerprint::density_tier_for(section_count, total_chars)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("forge-uniq-gate-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("cms")).expect("create cms");
        p
    }

    fn write_cms(root: &Path, name: &str, body: &str) {
        fs::write(root.join("cms").join(name), body).expect("write cms");
    }

    #[test]
    fn phase_is_silent_when_no_uniqueness_gate_section() {
        let root = temp_root("no-section");
        fs::write(root.join("forge.toml"), "[other]\n").unwrap();
        write_cms(&root, "index.json", r#"{"sections":[]}"#);
        let ctx = BuildCtx {
            root: root.clone(),
            static_dir: root.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = UniquenessGatePhase.run(&ctx).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_is_silent_when_enforce_false() {
        let root = temp_root("enforce-false");
        fs::write(
            root.join("forge.toml"),
            "[uniqueness_gate]\nenforce = false\n",
        )
        .unwrap();
        write_cms(&root, "index.json", r#"{"sections":[]}"#);
        let ctx = BuildCtx {
            root: root.clone(),
            static_dir: root.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = UniquenessGatePhase.run(&ctx).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_is_silent_when_registry_does_not_exist() {
        let root = temp_root("no-registry");
        fs::write(
            root.join("forge.toml"),
            "[uniqueness_gate]\nenforce = true\n",
        )
        .unwrap();
        write_cms(
            &root,
            "index.json",
            r#"{"sections":[{"kind":"hero_editorial","title":"Hello"}]}"#,
        );
        let ctx = BuildCtx {
            root: root.clone(),
            static_dir: root.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = UniquenessGatePhase.run(&ctx).unwrap();
        // No registry = fail-open
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_detects_exact_collision_with_different_site() {
        use forge_core::attest::generate_keypair;
        use forge_core::fingerprint_registry::append;

        let root = temp_root("collision");
        fs::create_dir_all(root.join("registry")).unwrap();
        let registry_path = root.join("registry/fingerprints.jsonl");

        // Write a cms file.
        write_cms(
            &root,
            "index.json",
            r#"{"sections":[{"kind":"hero_editorial","title":"Hello"}]}"#,
        );

        // Compute the build's fingerprint, append it under a DIFFERENT site_id.
        let fingerprint = build_fingerprint(&root.join("cms")).unwrap();
        let key = generate_keypair();
        append(
            &registry_path,
            "OTHER_SITE",
            "OTHER_TENANT",
            fingerprint,
            "2026-05-20T12:00:00Z",
            &key,
        )
        .unwrap();

        // Now run the gate — current build's site_id is empty (no site_identity),
        // so it should refuse the collision.
        fs::write(
            root.join("forge.toml"),
            "[uniqueness_gate]\nenforce = true\n",
        )
        .unwrap();
        let ctx = BuildCtx {
            root: root.clone(),
            static_dir: root.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = UniquenessGatePhase.run(&ctx).unwrap();
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("exact-fingerprint collision")),
            "expected collision finding, got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_is_silent_when_site_id_matches_registry_entry() {
        use forge_core::attest::generate_keypair;
        use forge_core::fingerprint_registry::append;

        let root = temp_root("self-rebuild");
        fs::create_dir_all(root.join("registry")).unwrap();
        let registry_path = root.join("registry/fingerprints.jsonl");

        write_cms(
            &root,
            "index.json",
            r#"{"sections":[{"kind":"hero_editorial","title":"Hello"}]}"#,
        );

        let fingerprint = build_fingerprint(&root.join("cms")).unwrap();
        let key = generate_keypair();
        append(
            &registry_path,
            "my-site",
            "my-tenant",
            fingerprint,
            "2026-05-20T12:00:00Z",
            &key,
        )
        .unwrap();

        // forge.toml declares the same site_id — rebuild, no collision.
        fs::write(
            root.join("forge.toml"),
            r#"
[uniqueness_gate]
enforce = true

[site_identity]
site_id = "my-site"
tenant_id = "my-tenant"
"#,
        )
        .unwrap();
        let ctx = BuildCtx {
            root: root.clone(),
            static_dir: root.join("static"),
            mode: forge_core::BuildMode::Poc,
        };
        let findings = UniquenessGatePhase.run(&ctx).unwrap();
        assert!(
            !findings.iter().any(|f| f.message.contains("collision")),
            "self-rebuild should not collide; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn build_fingerprint_produces_stable_silhouette() {
        let root = temp_root("silhouette");
        write_cms(
            &root,
            "index.json",
            r#"{"sections":[
              {"kind":"hero_editorial","title":"Hello world","lede":"A long-ish lede with text."},
              {"kind":"kv_pair","items":[{"k":"a","v":"b"},{"k":"c","v":"d"}]}
            ]}"#,
        );
        let fp = build_fingerprint(&root.join("cms")).unwrap();
        assert_eq!(fp.primitives.len(), 2);
        assert!(fp.silhouettes.contains_key("index"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn variant_guess_prefers_explicit_field() {
        let v = serde_json::json!({"variant":"compact"});
        assert_eq!(guess_variant("hero", &v), "variant=compact");
        let v = serde_json::json!({"columns": 3});
        assert_eq!(guess_variant("feature_spotlight", &v), "columns=3");
        let v = serde_json::json!({"items": [1,2,3,4]});
        assert_eq!(guess_variant("kv_pair", &v), "items=4");
        let v = serde_json::json!({});
        assert_eq!(guess_variant("paragraph", &v), "");
    }

    #[test]
    fn bucket_chars_groups_lengths() {
        assert_eq!(bucket_chars(0), 0);
        assert_eq!(bucket_chars(50), 0);
        assert_eq!(bucket_chars(150), 1);
        assert_eq!(bucket_chars(600), 2);
        assert_eq!(bucket_chars(3000), 4);
        assert_eq!(bucket_chars(100000), 5);
    }

    #[test]
    fn density_tier_for_classifies_correctly() {
        assert_eq!(density_tier_for(2, 200), "sparse");
        assert_eq!(density_tier_for(5, 1000), "comfortable");
        assert_eq!(density_tier_for(5, 4000), "dense");
        assert_eq!(density_tier_for(10, 2000), "dense");
        assert_eq!(density_tier_for(20, 5000), "extreme");
    }
}
