//! `network_target_enforcement` — for sites declaring a non-clearnet
//! deploy target (Tor / I2P / IPFS / Gemini / Lokinet), refuse to
//! ship any page containing a clearnet-only resource reference.
//!
//! Captures SITE_OPERATIONS.md §8 + PLATFORM_ROADMAP §7 — primitive
//! constraints per deploy target propagate back to Forge phases.
//! A Tor onion site that loads `https://cdn.example.com/x.js`
//! leaks every visitor's request to a clearnet observer — the
//! exact threat model the site supposedly defends against.
//!
//! The fix is structural: scan every shipped page + linked CSS for
//! external-URL references; emit Strict when any non-network-target
//! resource appears. The operator either swaps the resource for a
//! same-origin copy, declares it acceptable via the allowlist, or
//! removes it. Either way the build doesn't ship a leak.
//!
//! ## Configuration
//!
//! Reads `[networks]` from `forge.toml`:
//!
//! ```toml
//! [networks]
//! # The deployment targets this site supports. When ONLY clearnet
//! # is listed (or no [networks] section exists), this phase is a
//! # silent skip. When ANY non-clearnet target is listed, all pages
//! # must satisfy the strictest target's constraints.
//! targets = ["tor"]                # or ["clearnet", "tor"]
//!                                   # or ["i2p", "tor"]
//!
//! # Optional: per-target allowlists of external domains the
//! # operator has explicitly approved (e.g. an onion-mirror CDN).
//! # Default empty — strict deny for all non-target traffic.
//! [networks.allowlist]
//! tor = ["mirror.example.onion"]
//!
//! # Optional: skip the check for specific files (e.g. a clearnet-
//! # only sitemap.xml that's intentionally excluded from the onion
//! # bundle at deploy time).
//! skip_files = ["sitemap.xml"]
//! ```
//!
//! ## Strictness
//!
//! Each non-clearnet target declares a primitive_constraints set:
//!
//! | Target  | Constraints                                                   |
//! |---------|---------------------------------------------------------------|
//! | clearnet| no constraints (anything goes — phase silent-skips)            |
//! | tor     | no external clearnet URLs / no protocol-relative / no `//cdn`  |
//! | i2p     | same as tor + no `.onion` references (i2p ≠ tor)              |
//! | ipfs    | no clearnet — assets must resolve via IPFS/IPNS or relative   |
//! | gemini  | no inline images / no JS / no external scheme except `gemini:` |
//! | lokinet | same as tor + lokinet-DNS-or-relative only                    |
//!
//! All non-clearnet targets share the "no external clearnet" rule;
//! this phase enforces that union. Target-specific extra
//! constraints (e.g. Gemini's no-JS) are handled in addition.
//!
//! ## Severity
//!
//! - Page or linked CSS contains a clearnet-scheme URL → **Strict**
//! - Protocol-relative reference (`//host/path`) → **Strict** (would
//!   become clearnet on a clearnet-mirrored deploy; ambiguous on Tor)
//! - Allowlisted domain reference → silent
//! - Skip-list-included file → silent
//! - Gemini target + inline `<script>` or `<img>` outside an
//!   explicit `data:` URI → **Strict**

use std::collections::HashSet;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

use crate::html_walk::walk_html;

/// `network_target_enforcement` phase.
#[derive(Debug, Default)]
pub struct NetworkTargetEnforcementPhase;

impl Phase for NetworkTargetEnforcementPhase {
    fn name(&self) -> &'static str {
        "network_target_enforcement"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let Some(cfg) = forge_toml_networks(&ctx.root) else {
            tracing::debug!("network_target_enforcement: no [networks] section — skip");
            return Ok(vec![]);
        };
        if cfg.is_clearnet_only() {
            tracing::debug!("network_target_enforcement: clearnet-only — skip");
            return Ok(vec![]);
        }

        let mut findings = Vec::new();
        for file in walk_html(&ctx.static_dir, self.name())? {
            if cfg.skip_files.iter().any(|s| s == &file.name) {
                continue;
            }
            for issue in scan_for_leaks(&file.body, &cfg) {
                findings.push(Finding::strict(
                    self.name(),
                    file.name.clone(),
                    format!(
                        "network target [{targets}] forbids {kind}: `{value}` — \
                         {detail}",
                        targets = cfg.target_names().join(","),
                        kind = issue.kind,
                        value = issue.value,
                        detail = issue.detail,
                    ),
                ));
            }
        }
        Ok(findings)
    }
}

struct Leak {
    kind: &'static str,
    value: String,
    detail: &'static str,
}

fn scan_for_leaks(body: &str, cfg: &NetworksConfig) -> Vec<Leak> {
    let mut out: Vec<Leak> = Vec::new();
    // 1. External-scheme URLs (http://, https://, ws://, wss://, ftp://, ...)
    //    in any href/src/srcset/url(...).
    let urls = extract_referenced_urls(body);
    for url in urls {
        if let Some(scheme) = url_scheme(&url) {
            if scheme == "data" || scheme == "mailto" || scheme == "tel" {
                continue;
            }
            // Allow the target's own scheme (gemini:// on a Gemini
            // target).
            if cfg
                .targets
                .iter()
                .any(|t| t.allowed_scheme() == Some(scheme))
            {
                continue;
            }
            // Allow allowlisted hosts for any target.
            let host = extract_host(&url);
            if let Some(h) = host {
                if cfg.is_allowlisted_host(&h) {
                    continue;
                }
            }
            out.push(Leak {
                kind: "external clearnet URL",
                value: url.clone(),
                detail: "cold-cache visitors on the anonymity network would \
                         have this fetched from clearnet, leaking their request \
                         to a third-party observer. Either rehost same-origin, \
                         add the domain to [networks.allowlist], or remove the \
                         reference",
            });
        } else if url.starts_with("//") {
            // Protocol-relative — ambiguous; on Tor it would fall back
            // to clearnet.
            out.push(Leak {
                kind: "protocol-relative URL",
                value: url.clone(),
                detail: "protocol-relative URLs fall back to clearnet on \
                         anonymity-network deploys; replace with same-origin \
                         absolute path",
            });
        }
    }

    // 2. Gemini target specifically forbids inline <script> + <img>
    if cfg
        .targets
        .iter()
        .any(|t| matches!(t, NetworkTarget::Gemini))
    {
        if body.contains("<script") {
            out.push(Leak {
                kind: "inline <script>",
                value: "<script>".into(),
                detail: "Gemini protocol explicitly forbids JS — strip every \
                         <script> from pages targeting gemini:// deployment",
            });
        }
        if body.contains("<img") {
            out.push(Leak {
                kind: "inline <img>",
                value: "<img>".into(),
                detail: "Gemini protocol delivers no inline images — switch to \
                         a link with text alternative or split the image to a \
                         separate gemini:// resource",
            });
        }
    }

    // 3. I2P specifically forbids .onion references (cross-network leak)
    if cfg.targets.iter().any(|t| matches!(t, NetworkTarget::I2p)) && body.contains(".onion") {
        out.push(Leak {
            kind: ".onion reference on I2P target",
            value: ".onion".into(),
            detail: "I2P and Tor are separate networks; referencing a .onion \
                     URL from an I2P site requires the visitor to bridge \
                     networks. Either deliver via I2P or document the bridge \
                     explicitly via [networks.allowlist]",
        });
    }

    out
}

/// Pull href / src / srcset / inline-CSS url(...) values from
/// the HTML body. Matches `extract_asset_refs` in
/// `carbon_budget` but returns ALL URLs (including external ones)
/// so this phase can reason about scheme.
fn extract_referenced_urls(body: &str) -> Vec<String> {
    let mut out: HashSet<String> = HashSet::new();
    for attr in ["href=\"", "src=\"", "srcset=\""] {
        let mut search = body;
        while let Some(idx) = search.find(attr) {
            let after = &search[idx + attr.len()..];
            let Some(end) = after.find('"') else {
                break;
            };
            let raw = &after[..end];
            if attr == "srcset=\"" {
                for piece in raw.split(',') {
                    if let Some(url) = piece.trim().split_whitespace().next() {
                        if !url.is_empty() && !url.starts_with('#') {
                            out.insert(url.to_owned());
                        }
                    }
                }
            } else if !raw.is_empty() && !raw.starts_with('#') {
                out.insert(raw.to_owned());
            }
            search = &after[end + 1..];
        }
    }
    let mut s = body;
    while let Some(i) = s.find("url(") {
        let rest = &s[i + 4..];
        let Some(end) = rest.find(')') else {
            break;
        };
        let raw = rest[..end].trim().trim_matches('"').trim_matches('\'');
        if !raw.is_empty() && !raw.starts_with('#') {
            out.insert(raw.to_owned());
        }
        s = &rest[end + 1..];
    }
    out.into_iter().collect()
}

/// RFC 3986 §3.1 scheme detection. Returns the scheme name
/// (lowercase) or None for paths.
fn url_scheme(url: &str) -> Option<&str> {
    let mut chars = url.char_indices();
    let (first_idx, first) = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    let _ = first_idx;
    for (i, c) in chars {
        if c == ':' {
            return Some(&url[..i]);
        }
        if !(c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
            return None;
        }
    }
    None
}

/// Extract the host portion of an absolute URL. Returns lowercase
/// host without port. None for relative paths or malformed URLs.
fn extract_host(url: &str) -> Option<String> {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let host = after_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("")
        .split('#')
        .next()
        .unwrap_or("");
    let host_no_port = host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host);
    if host_no_port.is_empty() {
        None
    } else {
        Some(host_no_port.to_ascii_lowercase())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NetworkTarget {
    Clearnet,
    Tor,
    I2p,
    Ipfs,
    Gemini,
    Lokinet,
}

impl NetworkTarget {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "clearnet" | "https" | "http" => Some(Self::Clearnet),
            "tor" | "onion" => Some(Self::Tor),
            "i2p" => Some(Self::I2p),
            "ipfs" | "ipns" => Some(Self::Ipfs),
            "gemini" => Some(Self::Gemini),
            "lokinet" | "loki" => Some(Self::Lokinet),
            _ => None,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Clearnet => "clearnet",
            Self::Tor => "tor",
            Self::I2p => "i2p",
            Self::Ipfs => "ipfs",
            Self::Gemini => "gemini",
            Self::Lokinet => "lokinet",
        }
    }

    /// Each target's allowed URL scheme. Clearnet/Tor/Lokinet
    /// share https; I2P uses its own; Gemini uses gemini://; IPFS
    /// uses ipfs:// or ipns://.
    fn allowed_scheme(&self) -> Option<&'static str> {
        match self {
            Self::Clearnet => Some("https"),
            Self::Gemini => Some("gemini"),
            Self::Ipfs => Some("ipfs"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct NetworksConfig {
    targets: Vec<NetworkTarget>,
    allowlist: std::collections::BTreeMap<String, Vec<String>>,
    skip_files: Vec<String>,
}

impl NetworksConfig {
    fn is_clearnet_only(&self) -> bool {
        self.targets.is_empty()
            || self
                .targets
                .iter()
                .all(|t| matches!(t, NetworkTarget::Clearnet))
    }

    fn target_names(&self) -> Vec<&'static str> {
        self.targets.iter().map(|t| t.name()).collect()
    }

    fn is_allowlisted_host(&self, host: &str) -> bool {
        let host_lower = host.to_ascii_lowercase();
        self.allowlist
            .values()
            .any(|list| list.iter().any(|h| h.to_ascii_lowercase() == host_lower))
    }
}

fn forge_toml_networks(root: &Path) -> Option<NetworksConfig> {
    let path = root.join("forge.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    let parsed: toml::Value = content.parse().ok()?;
    let section = parsed.get("networks")?;
    let targets: Vec<NetworkTarget> = section
        .get("targets")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().and_then(NetworkTarget::from_str))
                .collect()
        })
        .unwrap_or_default();
    let allowlist = section
        .get("allowlist")
        .and_then(|v| v.as_table())
        .map(|t| {
            t.iter()
                .map(|(k, v)| {
                    let hosts = v
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|h| h.as_str().map(str::to_owned))
                                .collect()
                        })
                        .unwrap_or_default();
                    (k.clone(), hosts)
                })
                .collect()
        })
        .unwrap_or_default();
    let skip_files = section
        .get("skip_files")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    Some(NetworksConfig {
        targets,
        allowlist,
        skip_files,
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
    fn no_networks_section_silent_skip() {
        let dir = tempfile::tempdir().unwrap();
        write_page(
            dir.path(),
            "page.html",
            r#"<html><script src="https://cdn.evil.com/x.js"></script></html>"#,
        );
        let findings = NetworkTargetEnforcementPhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn clearnet_only_silent_skip() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"clearnet\"]\n");
        write_page(
            dir.path(),
            "page.html",
            r#"<html><script src="https://cdn.example.com/x.js"></script></html>"#,
        );
        let findings = NetworkTargetEnforcementPhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn tor_target_https_url_emits_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"tor\"]\n");
        write_page(
            dir.path(),
            "page.html",
            r#"<html><script src="https://cdn.evil.com/x.js"></script></html>"#,
        );
        let findings = NetworkTargetEnforcementPhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Strict);
        assert!(findings[0].message.contains("clearnet"));
        assert!(findings[0].message.contains("https://cdn.evil.com/x.js"));
    }

    #[test]
    fn tor_target_relative_url_accepted() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"tor\"]\n");
        write_page(
            dir.path(),
            "page.html",
            r#"<html><link rel="stylesheet" href="/local.css"></html>"#,
        );
        let findings = NetworkTargetEnforcementPhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn tor_target_protocol_relative_url_emits_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"tor\"]\n");
        write_page(
            dir.path(),
            "page.html",
            r#"<html><script src="//cdn.example.com/x.js"></script></html>"#,
        );
        let findings = NetworkTargetEnforcementPhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert!(findings
            .iter()
            .any(|f| f.message.contains("protocol-relative")));
    }

    #[test]
    fn allowlisted_host_accepted() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[networks]
targets = ["tor"]

[networks.allowlist]
tor = ["mirror.example.onion"]
"#,
        );
        write_page(
            dir.path(),
            "page.html",
            r#"<html><link href="https://mirror.example.onion/x.css"></html>"#,
        );
        let findings = NetworkTargetEnforcementPhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn data_uri_accepted() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"tor\"]\n");
        write_page(
            dir.path(),
            "page.html",
            r#"<html><img src="data:image/svg+xml,%3Csvg%2F%3E"></html>"#,
        );
        let findings = NetworkTargetEnforcementPhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn mailto_and_tel_accepted() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"tor\"]\n");
        write_page(
            dir.path(),
            "page.html",
            r#"<html><a href="mailto:x@y.com">m</a><a href="tel:+1234">t</a></html>"#,
        );
        let findings = NetworkTargetEnforcementPhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn gemini_target_forbids_inline_script() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"gemini\"]\n");
        write_page(
            dir.path(),
            "page.html",
            "<html><script>alert(1)</script></html>",
        );
        let findings = NetworkTargetEnforcementPhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert!(findings
            .iter()
            .any(|f| f.message.contains("inline <script>")));
    }

    #[test]
    fn gemini_target_forbids_inline_img() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"gemini\"]\n");
        write_page(dir.path(), "page.html", "<html><img src=\"/x.png\"></html>");
        let findings = NetworkTargetEnforcementPhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert!(findings.iter().any(|f| f.message.contains("inline <img>")));
    }

    #[test]
    fn i2p_target_flags_onion_references() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(dir.path(), "[networks]\ntargets = [\"i2p\"]\n");
        write_page(
            dir.path(),
            "page.html",
            r#"<html><a href="http://abc.onion">x</a></html>"#,
        );
        let findings = NetworkTargetEnforcementPhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert!(findings.iter().any(|f| f.message.contains(".onion")));
    }

    #[test]
    fn skip_files_excluded() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[networks]
targets = ["tor"]
skip_files = ["sitemap-clearnet.html"]
"#,
        );
        write_page(
            dir.path(),
            "sitemap-clearnet.html",
            r#"<html><a href="https://example.com/x">x</a></html>"#,
        );
        let findings = NetworkTargetEnforcementPhase
            .run(&ctx_in(dir.path()))
            .unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn url_scheme_detects_known_schemes() {
        assert_eq!(url_scheme("https://example.com"), Some("https"));
        assert_eq!(url_scheme("http://example.com"), Some("http"));
        assert_eq!(url_scheme("ws://example.com"), Some("ws"));
        assert_eq!(url_scheme("gemini://example.com"), Some("gemini"));
        assert_eq!(url_scheme("/relative/path"), None);
        assert_eq!(url_scheme("relative.html"), None);
        assert_eq!(url_scheme("//host"), None);
    }

    #[test]
    fn extract_host_handles_ports_and_paths() {
        assert_eq!(
            extract_host("https://example.com:443/path?q=1#frag"),
            Some("example.com".into())
        );
        assert_eq!(
            extract_host("https://EXAMPLE.com/path"),
            Some("example.com".into())
        );
    }
}
