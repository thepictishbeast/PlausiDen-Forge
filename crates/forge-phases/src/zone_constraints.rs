//! `zone_constraints` — per-content-type section quotas.
//!
//! Task #254 per the variation-architecture spec. Where
//! site_identity_conformance (#235) verifies allowed/forbidden
//! primitives ACROSS the site, this phase enforces per-content-
//! type quotas: a `long_form_article` content-type page MUST
//! include heading + paragraph + pull_quote; a `pricing_editorial`
//! page MUST NOT include `feature_spotlight` regardless of the
//! site-wide allowed list.
//!
//! Built on the page-type library (#250) + content-type taxonomy
//! declared in `[site_identity.content_type]` (#234). Each
//! content type can declare a quota that constrains its CMS pages
//! tighter than the site-wide defaults.
//!
//! ## forge.toml shape
//!
//! ```toml
//! # Site-wide content-type taxonomy (from #234).
//! [[site_identity.content_type]]
//! slug = "long_form_article"
//! pattern = "cms/articles/*.json"
//!
//! [[site_identity.content_type]]
//! slug = "homepage"
//! pattern = "cms/index.json"
//!
//! # Per-content-type quotas (this phase).
//! [zone_constraints]
//! enforce = true
//!
//! [[zone_constraints.quota]]
//! content_type = "long_form_article"
//! require = ["heading", "paragraph"]
//! forbid = ["pricing", "feature_spotlight", "stat_band"]
//! min_sections = 6
//! max_sections = 40
//!
//! [[zone_constraints.quota]]
//! content_type = "homepage"
//! require = ["hero_editorial", "call_to_action"]
//! ```
//!
//! Quota semantics:
//!
//! * `require` — every kind in this list MUST appear at least
//!   once on every page of this content_type.
//! * `forbid` — none of these kinds may appear (overrides
//!   site-wide allowed_primitives).
//! * `min_sections` — total section count floor (0 = unset).
//! * `max_sections` — total section count ceiling (0 = unset).
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `unsafe_code = "deny"`.
//! * `#[non_exhaustive]` on phase struct.
//! * No unwrap/expect in non-test code.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use forge_core::site_identity::SiteIdentity;
use forge_core::{BuildCtx, BuildError, Finding, Phase};
use serde_json::Value;

/// `zone_constraints` phase implementation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct ZoneConstraintsPhase;

impl Phase for ZoneConstraintsPhase {
    fn name(&self) -> &'static str {
        "zone_constraints"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let Some(cfg) = ZoneConfig::load(&ctx.root) else {
            return Ok(findings);
        };
        if !cfg.enforce || cfg.quotas.is_empty() {
            return Ok(findings);
        }
        let identity = SiteIdentity::load(&ctx.root).unwrap_or_default();
        if identity.content_type.is_empty() {
            return Ok(findings);
        }

        let cms_dir = ctx.root.join("cms");
        if !cms_dir.is_dir() {
            return Ok(findings);
        }

        let entries = fs::read_dir(&cms_dir).map_err(|e| BuildError::Io {
            context: format!("read_dir {}", cms_dir.display()),
            source: e,
        })?;
        for entry in entries {
            let entry = entry.map_err(|e| BuildError::Io {
                context: format!("read_dir entry in {}", cms_dir.display()),
                source: e,
            })?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let raw = fs::read_to_string(&path).map_err(|e| BuildError::Io {
                context: format!("read {}", path.display()),
                source: e,
            })?;
            let Ok(value) = serde_json::from_str::<Value>(&raw) else {
                continue;
            };
            let rel = path
                .file_name()
                .and_then(|s| s.to_str())
                .map(|n| format!("cms/{n}"))
                .unwrap_or_default();
            let Some(ct_slug) = identity.content_type_for(&rel) else {
                continue;
            };
            let Some(quota) = cfg.quotas.iter().find(|q| q.content_type == ct_slug) else {
                continue;
            };

            let path_disp = path.display().to_string();
            check_page(&path_disp, ct_slug, &value, quota, &mut findings, self.name());
        }

        Ok(findings)
    }
}

#[derive(Debug, Clone)]
struct ZoneConfig {
    enforce: bool,
    quotas: Vec<Quota>,
}

#[derive(Debug, Clone)]
struct Quota {
    content_type: String,
    require: Vec<String>,
    forbid: Vec<String>,
    min_sections: u32,
    max_sections: u32,
}

impl ZoneConfig {
    fn load(root: &Path) -> Option<Self> {
        let body = fs::read_to_string(root.join("forge.toml")).ok()?;
        let value: toml::Value = toml::from_str(&body).ok()?;
        let section = value.get("zone_constraints")?.as_table()?;
        let enforce = section
            .get("enforce")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let mut quotas = Vec::new();
        if let Some(arr) = section.get("quota").and_then(|v| v.as_array()) {
            for q in arr {
                let Some(t) = q.as_table() else { continue };
                let Some(ct) = t.get("content_type").and_then(|v| v.as_str()) else {
                    continue;
                };
                let require = t
                    .get("require")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                let forbid = t
                    .get("forbid")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                let min_sections = t
                    .get("min_sections")
                    .and_then(|v| v.as_integer())
                    .and_then(|n| u32::try_from(n).ok())
                    .unwrap_or(0);
                let max_sections = t
                    .get("max_sections")
                    .and_then(|v| v.as_integer())
                    .and_then(|n| u32::try_from(n).ok())
                    .unwrap_or(0);
                quotas.push(Quota {
                    content_type: ct.to_owned(),
                    require,
                    forbid,
                    min_sections,
                    max_sections,
                });
            }
        }
        Some(Self { enforce, quotas })
    }
}

fn check_page(
    path: &str,
    ct_slug: &str,
    page: &Value,
    quota: &Quota,
    findings: &mut Vec<Finding>,
    phase: &'static str,
) {
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    let mut total: u32 = 0;
    if let Some(sections) = page.get("sections").and_then(|s| s.as_array()) {
        for section in sections {
            if let Some(kind) = section.get("kind").and_then(|v| v.as_str()) {
                *counts.entry(kind.to_owned()).or_insert(0) += 1;
                total += 1;
            }
        }
    }

    for req in &quota.require {
        if counts.get(req).copied().unwrap_or(0) == 0 {
            findings.push(
                Finding::strict(
                    phase,
                    path.to_owned(),
                    format!(
                        "zone_constraints — content_type `{ct_slug}` requires `{req}` but the page has none"
                    ),
                )
                .citing(["zone-001"])
                .why("the declared content type's required-sections quota is not met; readers of this content type expect this primitive")
                .fix(format!("add a `{req}` section to this page OR remove `{req}` from the `[[zone_constraints.quota]]` for `{ct_slug}` if it's no longer required")),
            );
        }
    }
    for forbid in &quota.forbid {
        if let Some(count) = counts.get(forbid).copied() {
            if count > 0 {
                findings.push(
                    Finding::strict(
                        phase,
                        path.to_owned(),
                        format!(
                            "zone_constraints — content_type `{ct_slug}` forbids `{forbid}` but the page has {count}"
                        ),
                    )
                    .citing(["zone-002"])
                    .why("the declared content type forbids this primitive; site-wide allowed_primitives doesn't relax the per-content-type quota")
                    .fix(format!("remove `{forbid}` sections from this page OR drop `{forbid}` from the `[[zone_constraints.quota]]` forbid list for `{ct_slug}`")),
                );
            }
        }
    }
    if quota.min_sections > 0 && total < quota.min_sections {
        findings.push(
            Finding::strict(
                phase,
                path.to_owned(),
                format!(
                    "zone_constraints — content_type `{ct_slug}` requires at least {} sections; page has {total}",
                    quota.min_sections
                ),
            )
            .citing(["zone-003"])
            .why("the declared content type's minimum-sections floor is not met; under-content for the declared type")
            .fix(format!("add sections to reach {} OR lower min_sections in the [[zone_constraints.quota]] for `{ct_slug}`", quota.min_sections)),
        );
    }
    if quota.max_sections > 0 && total > quota.max_sections {
        findings.push(
            Finding::strict(
                phase,
                path.to_owned(),
                format!(
                    "zone_constraints — content_type `{ct_slug}` allows at most {} sections; page has {total}",
                    quota.max_sections
                ),
            )
            .citing(["zone-004"])
            .why("the declared content type's maximum-sections ceiling is exceeded; consider splitting the page")
            .fix(format!("split the page OR raise max_sections in the [[zone_constraints.quota]] for `{ct_slug}`")),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "forge-zone-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("cms")).unwrap();
        p
    }

    fn ctx_for(root: &Path) -> BuildCtx {
        BuildCtx {
            root: root.to_path_buf(),
            static_dir: root.join("static"),
            mode: forge_core::BuildMode::Poc,
        }
    }

    fn write_cms(root: &Path, name: &str, body: &str) {
        fs::write(root.join("cms").join(name), body).unwrap();
    }

    fn baseline_toml() -> &'static str {
        r#"
[site_identity]

[[site_identity.content_type]]
slug = "long_form_article"
pattern = "cms/article.json"

[[site_identity.content_type]]
slug = "homepage"
pattern = "cms/index.json"

[zone_constraints]
enforce = true

[[zone_constraints.quota]]
content_type = "long_form_article"
require = ["heading", "paragraph"]
forbid = ["pricing", "feature_spotlight"]
min_sections = 5
max_sections = 30

[[zone_constraints.quota]]
content_type = "homepage"
require = ["hero_editorial"]
"#
    }

    #[test]
    fn phase_silent_when_not_enforced() {
        let root = temp_root("not-enforced");
        fs::write(
            root.join("forge.toml"),
            "[zone_constraints]\nenforce = false\n",
        )
        .unwrap();
        write_cms(&root, "index.json", r#"{"sections":[]}"#);
        let findings = ZoneConstraintsPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_no_content_type_taxonomy() {
        let root = temp_root("no-taxonomy");
        fs::write(
            root.join("forge.toml"),
            r#"
[zone_constraints]
enforce = true

[[zone_constraints.quota]]
content_type = "anything"
require = ["heading"]
"#,
        )
        .unwrap();
        write_cms(&root, "index.json", r#"{"sections":[]}"#);
        let findings = ZoneConstraintsPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_missing_required_kind() {
        let root = temp_root("missing-required");
        fs::write(root.join("forge.toml"), baseline_toml()).unwrap();
        write_cms(
            &root,
            "article.json",
            r#"{"sections":[
              {"kind":"heading"},
              {"kind":"image_hero"},
              {"kind":"heading"},
              {"kind":"image_hero"},
              {"kind":"heading"}
            ]}"#,
        );
        let findings = ZoneConstraintsPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("requires `paragraph` but the page has none")),
            "expected missing-paragraph finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_forbidden_kind() {
        let root = temp_root("forbidden");
        fs::write(root.join("forge.toml"), baseline_toml()).unwrap();
        write_cms(
            &root,
            "article.json",
            r#"{"sections":[
              {"kind":"heading"},
              {"kind":"paragraph"},
              {"kind":"pricing"},
              {"kind":"paragraph"},
              {"kind":"paragraph"}
            ]}"#,
        );
        let findings = ZoneConstraintsPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("forbids `pricing`")),
            "expected forbidden-pricing finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_min_sections_violation() {
        let root = temp_root("under-min");
        fs::write(root.join("forge.toml"), baseline_toml()).unwrap();
        write_cms(
            &root,
            "article.json",
            r#"{"sections":[
              {"kind":"heading"},
              {"kind":"paragraph"}
            ]}"#,
        );
        let findings = ZoneConstraintsPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("requires at least 5 sections")),
            "expected min-sections finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_flags_max_sections_violation() {
        let root = temp_root("over-max");
        fs::write(root.join("forge.toml"), baseline_toml()).unwrap();
        let many = (0..40)
            .map(|_| r#"{"kind":"paragraph"}"#)
            .collect::<Vec<_>>()
            .join(",");
        write_cms(
            &root,
            "article.json",
            &format!(r#"{{"sections":[{{"kind":"heading"}},{many}]}}"#),
        );
        let findings = ZoneConstraintsPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("allows at most 30 sections")),
            "expected max-sections finding; got: {findings:#?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_silent_when_quota_satisfied() {
        let root = temp_root("clean");
        fs::write(root.join("forge.toml"), baseline_toml()).unwrap();
        write_cms(
            &root,
            "article.json",
            r#"{"sections":[
              {"kind":"heading"},
              {"kind":"paragraph"},
              {"kind":"paragraph"},
              {"kind":"pull_quote"},
              {"kind":"paragraph"},
              {"kind":"paragraph"}
            ]}"#,
        );
        let findings = ZoneConstraintsPhase.run(&ctx_for(&root)).unwrap();
        assert!(findings.is_empty(), "should pass; got: {findings:#?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn phase_uses_per_content_type_quota() {
        let root = temp_root("per-content");
        fs::write(root.join("forge.toml"), baseline_toml()).unwrap();
        // homepage requires hero_editorial; article requires paragraph.
        // index.json missing hero_editorial → fire.
        // article.json present hero_editorial but missing paragraph →
        // fire for article only.
        write_cms(&root, "index.json", r#"{"sections":[{"kind":"heading"}]}"#);
        write_cms(
            &root,
            "article.json",
            r#"{"sections":[
              {"kind":"heading"},
              {"kind":"hero_editorial"},
              {"kind":"heading"},
              {"kind":"hero_editorial"},
              {"kind":"heading"}
            ]}"#,
        );
        let findings = ZoneConstraintsPhase.run(&ctx_for(&root)).unwrap();
        assert!(
            findings.iter().any(|f| f.path.contains("index.json")
                && f.message.contains("requires `hero_editorial`")),
            "expected homepage hero_editorial finding"
        );
        assert!(
            findings.iter().any(|f| f.path.contains("article.json")
                && f.message.contains("requires `paragraph`")),
            "expected article paragraph finding"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
