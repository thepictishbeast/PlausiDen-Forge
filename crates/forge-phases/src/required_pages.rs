//! `required_pages` — site-type-aware verification that every
//! site has the legal / identity / support / operational pages
//! and `.well-known/` standards real-world deployment requires.
//!
//! Captures the doctrine from
//! `PlausiDen-Forge/docs/SITE_OPERATIONS.md §1` (required pages by
//! site type). The phase is generic — it works for every site,
//! site-specific content lives in `cms/`, only the *which* +
//! *how-strict* axes vary by declared `site_type`.
//!
//! ## Configuration
//!
//! Reads `[required_pages]` from `forge.toml`:
//!
//! ```toml
//! [required_pages]
//! # One of: business | personal_blog | ecommerce | saas | nonprofit
//! #         government | education | anonymous_publishing
//! site_type = "business"
//!
//! # Optional: declared operating jurisdictions for compliance gates.
//! # Drives which legal-required pages are Strict vs Warn.
//! jurisdictions = ["eu", "us-ca", "de"]
//!
//! # Optional: skip checks for pages the site genuinely doesn't
//! # need. Use with care — every skip is a documented exception.
//! skip = ["modern_slavery", "imprint"]
//! ```
//!
//! Missing `[required_pages]` section → silent skip (sites that
//! haven't opted into the contract aren't gated).
//!
//! ## Check matrix
//!
//! Per [`SITE_OPERATIONS.md §1`][1], every site MUST or SHOULD
//! have a curated set of legal foundation, identity, support,
//! discovery, operational, and infrastructure pages plus the
//! `.well-known/` discoverable standards. This phase encodes the
//! matrix below — required (Strict) means the site cannot ship
//! without it; expected (Warn) means it surfaces in poc-mode and
//! escalates to Strict in production.
//!
//! [1]: https://github.com/thepictishbeast/PlausiDen-Forge/blob/main/docs/SITE_OPERATIONS.md
//!
//! | Page / asset                        | Required when                  |
//! |-------------------------------------|--------------------------------|
//! | `privacy.html` / `privacy/`         | Always (Strict)                |
//! | `terms.html` / `terms/`             | Almost always (Strict)         |
//! | `cookies.html` / `cookies/`         | If `[required_pages].jurisdictions` includes any EU code (Strict) |
//! | `accessibility.html` / `accessibility/` | EU (Strict); else Warn       |
//! | `imprint.html`                      | DE/AT/CH (Strict); else skipped |
//! | `404.html`                          | Always (Warn)                  |
//! | `.well-known/security.txt`          | Always (Warn) — RFC 9116       |
//! | `robots.txt`                        | Always (Warn)                  |
//! | `sitemap.xml`                       | Always (Warn)                  |
//! | favicon (`favicon.ico` + apple-touch-icon family) | Always (Warn)   |
//! | Open Graph + Twitter Card meta per page | Always (Warn) on every HTML  |
//!
//! Site-type-specific additions (one example each, full doctrine
//! in `SITE_OPERATIONS.md`):
//!
//! - `ecommerce` → `returns.html`, `shipping.html` (Strict)
//! - `saas` → `dpa.html` (Strict if any EU jurisdiction)
//! - `government` → `accessibility.html` (Strict everywhere)
//! - `anonymous_publishing` → relaxes the favicon/OG/JS-required
//!    checks to Warn since Tor-mode sites legitimately ship
//!    minimal chrome

use std::collections::HashSet;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `required_pages` phase.
#[derive(Debug, Default)]
pub struct RequiredPagesPhase;

impl Phase for RequiredPagesPhase {
    fn name(&self) -> &'static str {
        "required_pages"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let Some(cfg) = forge_toml_required_pages(&ctx.root) else {
            tracing::debug!("required_pages: no [required_pages] section — skip");
            return Ok(vec![]);
        };
        let mut findings = Vec::new();
        let checks = checks_for_site_type(&cfg);
        for check in checks {
            if cfg.skip.contains(check.id) {
                tracing::debug!(check = %check.id, "required_pages: skipped per config");
                continue;
            }
            match check.kind {
                CheckKind::PagePresent { path } => {
                    if !any_present(&ctx.static_dir, path) {
                        findings.push(present_finding(&check, path));
                    }
                }
                CheckKind::WellKnown { name } => {
                    let p = ctx.static_dir.join(".well-known").join(name);
                    if !p.exists() {
                        findings.push(present_finding(&check, name));
                    }
                }
                CheckKind::PerPageMeta { needle } => {
                    let missing = pages_missing_needle(&ctx.static_dir, needle)?;
                    for page in missing {
                        findings.push(per_page_finding(&check, &page, needle));
                    }
                }
                CheckKind::AnyOf { paths } => {
                    if !paths.iter().any(|p| any_present(&ctx.static_dir, p)) {
                        findings.push(present_finding(&check, &paths.join(" | ")));
                    }
                }
            }
        }
        Ok(findings)
    }
}

/// Parsed `[required_pages]` config.
#[derive(Debug, Clone, Default)]
struct RequiredPagesConfig {
    /// One of the site-type enum variants.
    site_type: SiteType,
    /// Lowercased declared operating jurisdictions ("eu", "us-ca",
    /// "de", "uk", "ca", "br", "au", "sg", etc.).
    jurisdictions: HashSet<String>,
    /// Check IDs the operator explicitly opts out of.
    skip: HashSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SiteType {
    #[default]
    Business,
    PersonalBlog,
    Ecommerce,
    Saas,
    Nonprofit,
    Government,
    Education,
    AnonymousPublishing,
}

impl SiteType {
    fn from_str(s: &str) -> Self {
        match s {
            "business" => Self::Business,
            "personal_blog" => Self::PersonalBlog,
            "ecommerce" => Self::Ecommerce,
            "saas" => Self::Saas,
            "nonprofit" => Self::Nonprofit,
            "government" => Self::Government,
            "education" => Self::Education,
            "anonymous_publishing" => Self::AnonymousPublishing,
            _ => Self::Business,
        }
    }
}

#[derive(Debug, Clone)]
struct Check {
    id: &'static str,
    /// Human-readable summary surfaced in the finding message.
    description: &'static str,
    severity: CheckSeverity,
    kind: CheckKind<'static>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckSeverity {
    Strict,
    Warn,
}

#[derive(Debug, Clone)]
enum CheckKind<'a> {
    /// A specific file must exist at one or more candidate paths.
    PagePresent { path: &'a str },
    /// A specific filename under `.well-known/` must exist.
    WellKnown { name: &'a str },
    /// Every shipped HTML page must contain a specific substring
    /// (e.g. `<meta property="og:`). Surfaces per-page misses.
    PerPageMeta { needle: &'a str },
    /// At least one of the listed paths must exist (used for
    /// "404.html OR 404/" + "favicon.ico OR favicon.svg" etc).
    AnyOf { paths: &'a [&'a str] },
}

/// Compose the check matrix for the declared site type +
/// jurisdictions. Order matters only for output legibility;
/// findings are independent.
fn checks_for_site_type(cfg: &RequiredPagesConfig) -> Vec<Check> {
    let has_eu_jur = cfg.jurisdictions.iter().any(|j| {
        matches!(
            j.as_str(),
            "eu" | "de"
                | "fr"
                | "es"
                | "it"
                | "nl"
                | "pl"
                | "pt"
                | "se"
                | "ie"
                | "at"
                | "be"
                | "fi"
                | "dk"
                | "ee"
                | "cy"
                | "cz"
                | "gr"
                | "hr"
                | "hu"
                | "lt"
                | "lu"
                | "lv"
                | "mt"
                | "ro"
                | "si"
                | "sk"
                | "bg"
        )
    });
    let dach = cfg
        .jurisdictions
        .iter()
        .any(|j| matches!(j.as_str(), "de" | "at" | "ch"));
    let mut out: Vec<Check> = Vec::new();

    // Legal foundation — every site
    out.push(Check {
        id: "privacy",
        description: "Privacy Policy required by GDPR / CCPA / every state privacy law",
        severity: CheckSeverity::Strict,
        kind: CheckKind::AnyOf {
            paths: &["privacy.html", "privacy/index.html", "privacy-policy.html"],
        },
    });
    out.push(Check {
        id: "terms",
        description: "Terms of Service limits liability + sets jurisdiction",
        severity: CheckSeverity::Strict,
        kind: CheckKind::AnyOf {
            paths: &[
                "terms.html",
                "terms/index.html",
                "terms-of-service.html",
                "tos.html",
            ],
        },
    });
    if has_eu_jur {
        out.push(Check {
            id: "cookies",
            description: "Cookie Policy required separately under GDPR",
            severity: CheckSeverity::Strict,
            kind: CheckKind::AnyOf {
                paths: &["cookies.html", "cookies/index.html", "cookie-policy.html"],
            },
        });
        out.push(Check {
            id: "accessibility",
            description: "Accessibility Statement required under EAA (EU)",
            severity: CheckSeverity::Strict,
            kind: CheckKind::AnyOf {
                paths: &["accessibility.html", "accessibility/index.html"],
            },
        });
    } else {
        out.push(Check {
            id: "accessibility",
            description: "Accessibility Statement reduces ADA litigation risk (US)",
            severity: CheckSeverity::Warn,
            kind: CheckKind::AnyOf {
                paths: &["accessibility.html", "accessibility/index.html"],
            },
        });
    }
    if dach {
        out.push(Check {
            id: "imprint",
            description: "Impressum legally required in DE / AT / CH",
            severity: CheckSeverity::Strict,
            kind: CheckKind::AnyOf {
                paths: &["imprint.html", "impressum.html", "imprint/index.html"],
            },
        });
    }

    // Operational
    out.push(Check {
        id: "404",
        description: "Custom 404 page (designed, helpful, not a generic dead end)",
        severity: CheckSeverity::Warn,
        kind: CheckKind::AnyOf {
            paths: &["404.html", "404/index.html"],
        },
    });

    // Discoverable infrastructure
    out.push(Check {
        id: "security_txt",
        description: "RFC 9116 .well-known/security.txt declares vulnerability disclosure contact",
        severity: CheckSeverity::Warn,
        kind: CheckKind::WellKnown {
            name: "security.txt",
        },
    });
    out.push(Check {
        id: "robots",
        description: "robots.txt configures crawler access deliberately",
        severity: CheckSeverity::Warn,
        kind: CheckKind::PagePresent { path: "robots.txt" },
    });
    out.push(Check {
        id: "sitemap",
        description: "sitemap.xml drives discoverability + search-engine indexing",
        severity: CheckSeverity::Warn,
        kind: CheckKind::PagePresent {
            path: "sitemap.xml",
        },
    });
    out.push(Check {
        id: "favicon",
        description: "Favicon (any one of ico / svg / png variants)",
        severity: CheckSeverity::Warn,
        kind: CheckKind::AnyOf {
            paths: &["favicon.ico", "favicon.svg", "favicon-32x32.png"],
        },
    });

    // Per-page meta — every HTML page
    out.push(Check {
        id: "og_meta",
        description: "Open Graph meta tag (og:title / og:description / og:image)",
        severity: CheckSeverity::Warn,
        kind: CheckKind::PerPageMeta {
            needle: "property=\"og:",
        },
    });
    out.push(Check {
        id: "twitter_card",
        description: "Twitter Card meta tag (rich social-share preview)",
        severity: CheckSeverity::Warn,
        kind: CheckKind::PerPageMeta {
            needle: "name=\"twitter:",
        },
    });

    // Site-type specifics
    match cfg.site_type {
        SiteType::Ecommerce => {
            out.push(Check {
                id: "returns",
                description: "Returns policy required for transactional sites",
                severity: CheckSeverity::Strict,
                kind: CheckKind::AnyOf {
                    paths: &["returns.html", "returns/index.html"],
                },
            });
            out.push(Check {
                id: "shipping",
                description: "Shipping policy required for transactional sites",
                severity: CheckSeverity::Strict,
                kind: CheckKind::AnyOf {
                    paths: &["shipping.html", "shipping/index.html"],
                },
            });
        }
        SiteType::Saas if has_eu_jur => {
            out.push(Check {
                id: "dpa",
                description: "Data Processing Addendum required for B2B SaaS in EU markets",
                severity: CheckSeverity::Strict,
                kind: CheckKind::AnyOf {
                    paths: &["dpa.html", "dpa/index.html"],
                },
            });
        }
        SiteType::Government => {
            // Override the EU/else accessibility severity — government
            // sites need Strict everywhere.
            for check in out.iter_mut() {
                if check.id == "accessibility" {
                    check.severity = CheckSeverity::Strict;
                }
            }
        }
        SiteType::AnonymousPublishing => {
            // Tor-mode sites legitimately ship minimal chrome. Relax
            // favicon + OG to Warn-but-non-blocking; the operator's
            // threat model can justify omitting these (favicons +
            // OG previews are fingerprinting surface).
            for check in out.iter_mut() {
                if matches!(check.id, "favicon" | "og_meta" | "twitter_card") {
                    check.severity = CheckSeverity::Warn;
                }
            }
        }
        _ => {}
    }

    out
}

/// Check whether `static_dir/relative` exists as a regular file.
fn any_present(static_dir: &Path, relative: &str) -> bool {
    static_dir.join(relative).is_file()
}

/// Scan every shipped HTML page for `needle`. Return the list of
/// page basenames that don't contain it.
fn pages_missing_needle(static_dir: &Path, needle: &str) -> Result<Vec<String>, BuildError> {
    let files = walk_html(static_dir, "required_pages")?;
    let mut missing = Vec::new();
    for file in &files {
        if !file.body.contains(needle) {
            missing.push(file.name.clone());
        }
    }
    Ok(missing)
}

/// Render a check as a Forge `Finding` for a missing page / asset.
fn present_finding(check: &Check, what: &str) -> Finding {
    let msg = format!(
        "{description} — expected {what} but not found",
        description = check.description,
    );
    match check.severity {
        CheckSeverity::Strict => Finding::strict("required_pages", check.id, msg),
        CheckSeverity::Warn => Finding::warn("required_pages", check.id, msg),
    }
}

/// Render a check as a Forge `Finding` for a per-page meta miss.
fn per_page_finding(check: &Check, page: &str, needle: &str) -> Finding {
    let msg = format!(
        "{description} — page {page} missing `{needle}` meta",
        description = check.description,
    );
    match check.severity {
        CheckSeverity::Strict => Finding::strict("required_pages", page.to_owned(), msg),
        CheckSeverity::Warn => Finding::warn("required_pages", page.to_owned(), msg),
    }
}

/// Read `[required_pages]` from `<root>/forge.toml`. Returns
/// `None` if file missing, parse error, or section absent.
fn forge_toml_required_pages(root: &Path) -> Option<RequiredPagesConfig> {
    let path = root.join("forge.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    let parsed: toml::Value = content.parse().ok()?;
    let section = parsed.get("required_pages")?;
    let site_type = section
        .get("site_type")
        .and_then(|v| v.as_str())
        .map(SiteType::from_str)
        .unwrap_or_default();
    let jurisdictions = section
        .get("jurisdictions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_ascii_lowercase()))
                .collect()
        })
        .unwrap_or_default();
    let skip = section
        .get("skip")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect()
        })
        .unwrap_or_default();
    Some(RequiredPagesConfig {
        site_type,
        jurisdictions,
        skip,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::{BuildMode, Severity};

    fn ctx_in(dir: &Path) -> BuildCtx {
        BuildCtx {
            root: dir.to_path_buf(),
            static_dir: dir.to_path_buf(),
            mode: BuildMode::Poc,
        }
    }

    fn write_forge_toml(dir: &Path, body: &str) {
        std::fs::write(dir.join("forge.toml"), body).unwrap();
    }

    fn write_page(dir: &Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).unwrap();
    }

    #[test]
    fn missing_required_pages_section_silent_skip() {
        let dir = tempfile::tempdir().unwrap();
        // no forge.toml at all
        let findings = RequiredPagesPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn forge_toml_without_required_pages_section_silent_skip() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[forge]\nmode = \"poc\"\n");
        let findings = RequiredPagesPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn business_us_emits_strict_for_missing_privacy_and_terms() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[required_pages]
site_type = "business"
jurisdictions = ["us"]
"#,
        );
        let findings = RequiredPagesPhase.run(&ctx_in(dir.path())).unwrap();
        let strict: Vec<_> = findings
            .iter()
            .filter(|f| f.severity == Severity::Strict)
            .collect();
        assert!(strict.iter().any(|f| f.path == "privacy"));
        assert!(strict.iter().any(|f| f.path == "terms"));
        // US-only → accessibility is Warn, not Strict
        assert!(!strict.iter().any(|f| f.path == "accessibility"));
    }

    #[test]
    fn eu_jurisdiction_escalates_cookies_and_accessibility_to_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[required_pages]
site_type = "business"
jurisdictions = ["eu"]
"#,
        );
        let findings = RequiredPagesPhase.run(&ctx_in(dir.path())).unwrap();
        let strict: Vec<_> = findings
            .iter()
            .filter(|f| f.severity == Severity::Strict)
            .collect();
        assert!(strict.iter().any(|f| f.path == "cookies"));
        assert!(strict.iter().any(|f| f.path == "accessibility"));
    }

    #[test]
    fn dach_jurisdiction_requires_imprint() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[required_pages]
site_type = "business"
jurisdictions = ["de"]
"#,
        );
        let findings = RequiredPagesPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings
            .iter()
            .any(|f| f.path == "imprint" && f.severity == Severity::Strict));
    }

    #[test]
    fn ecommerce_requires_returns_and_shipping_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[required_pages]
site_type = "ecommerce"
jurisdictions = ["us"]
"#,
        );
        let findings = RequiredPagesPhase.run(&ctx_in(dir.path())).unwrap();
        let strict: Vec<_> = findings
            .iter()
            .filter(|f| f.severity == Severity::Strict)
            .collect();
        assert!(strict.iter().any(|f| f.path == "returns"));
        assert!(strict.iter().any(|f| f.path == "shipping"));
    }

    #[test]
    fn skip_list_silences_checks() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[required_pages]
site_type = "business"
jurisdictions = ["us"]
skip = ["privacy", "terms"]
"#,
        );
        let findings = RequiredPagesPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(!findings.iter().any(|f| f.path == "privacy"));
        assert!(!findings.iter().any(|f| f.path == "terms"));
    }

    #[test]
    fn present_pages_silence_their_checks() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[required_pages]
site_type = "business"
jurisdictions = ["us"]
"#,
        );
        write_page(
            dir.path(),
            "privacy.html",
            "<html><body>privacy</body></html>",
        );
        write_page(dir.path(), "terms.html", "<html><body>terms</body></html>");
        let findings = RequiredPagesPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(!findings.iter().any(|f| f.path == "privacy"));
        assert!(!findings.iter().any(|f| f.path == "terms"));
    }

    #[test]
    fn well_known_security_txt_check() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[required_pages]
site_type = "business"
"#,
        );
        let findings = RequiredPagesPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings.iter().any(|f| f.path == "security_txt"));

        // After we add it, the finding goes away.
        std::fs::create_dir_all(dir.path().join(".well-known")).unwrap();
        std::fs::write(
            dir.path().join(".well-known/security.txt"),
            "Contact: mailto:security@example.com\n",
        )
        .unwrap();
        let findings = RequiredPagesPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(!findings.iter().any(|f| f.path == "security_txt"));
    }

    #[test]
    fn per_page_og_meta_miss_emits_per_page_finding() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[required_pages]
site_type = "business"
"#,
        );
        // Two pages — one with OG, one without
        write_page(
            dir.path(),
            "index.html",
            r#"<html><head><meta property="og:title" content="Home"></head></html>"#,
        );
        write_page(
            dir.path(),
            "about.html",
            "<html><head><title>About</title></head></html>",
        );
        let findings = RequiredPagesPhase.run(&ctx_in(dir.path())).unwrap();
        let og_misses: Vec<_> = findings
            .iter()
            .filter(|f| f.message.contains("og:"))
            .collect();
        assert_eq!(og_misses.len(), 1);
        assert_eq!(og_misses[0].path, "about.html");
    }

    #[test]
    fn anonymous_publishing_relaxes_favicon_and_og() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[required_pages]
site_type = "anonymous_publishing"
"#,
        );
        let findings = RequiredPagesPhase.run(&ctx_in(dir.path())).unwrap();
        // favicon / og / twitter should be Warn, never Strict, even
        // though they're absent.
        for path in ["favicon", "og_meta", "twitter_card"] {
            for f in findings.iter().filter(|f| f.path == path) {
                assert_eq!(
                    f.severity,
                    Severity::Warn,
                    "{} should never be Strict for anonymous_publishing",
                    path
                );
            }
        }
    }

    #[test]
    fn unknown_site_type_falls_back_to_business() {
        assert_eq!(SiteType::from_str("garbage"), SiteType::Business);
    }
}
