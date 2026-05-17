//! `dns_hygiene_lint` — flag DNS records the site config implies
//! the operator needs to add at their registrar.
//!
//! Captures SITE_OPERATIONS.md §1 (DNS records every site needs).
//! Forge can't reach into the operator's registrar to verify
//! records are actually set — that's an external system. But
//! Forge CAN look at the operator's site config + features-in-use
//! and emit a checklist of records that MUST exist for the site's
//! declared features to work.
//!
//! Example: a site that emits forms with email notifications +
//! sends transactional email needs SPF + DKIM + DMARC at the
//! registrar. If the operator hasn't set those, the email goes
//! to spam. Forge can't check the registrar but it can surface
//! the requirement as a Warn so the operator gets the
//! checklist on every build.
//!
//! ## Configuration
//!
//! Reads `[dns_hygiene]` from `forge.toml`:
//!
//! ```toml
//! [dns_hygiene]
//! # Primary domain the site ships under. Used in generated
//! # record examples.
//! domain = "example.com"
//!
//! # Features the operator has explicitly opted into. Drives
//! # which DNS records get checklist-emitted.
//! # Options: "email" "https" "hsts_preload" "mta_sts" "caa"
//! #          "bimi" "dnssec" "security_txt" "matrix"
//! features = ["email", "https", "caa"]
//!
//! # Optional: mark records the operator has confirmed are
//! # already set at the registrar. These suppress the checklist
//! # entry. Use sparingly — Forge can't verify.
//! confirmed = ["spf", "dkim_default"]
//! ```
//!
//! Missing `[dns_hygiene]` section → silent skip.
//!
//! ## Severity
//!
//! All findings are Warn — Forge cannot verify external DNS state,
//! so emitting Strict would mean every build of every site that
//! ships email fails until DNS state is verified manually. Warn
//! is the right policy: surface the checklist, let the operator
//! confirm via the `confirmed` list once the records are in place.
//!
//! ## Records covered
//!
//! | Feature       | Records implied                                 |
//! |---------------|-------------------------------------------------|
//! | email         | SPF, DKIM (default selector), DMARC             |
//! | mta_sts       | _mta-sts TXT + .well-known/mta-sts.txt          |
//! | bimi          | BIMI TXT (requires DMARC at p=quarantine/reject)|
//! | https         | (informational: cert provisioning, CAA optional)|
//! | hsts_preload  | HSTS preload submission (out-of-band)           |
//! | caa           | CAA records constraining which CAs may issue    |
//! | dnssec        | DNSSEC enabled at registrar                     |
//! | security_txt  | (paired with phase_required_pages security_txt) |
//! | matrix        | Matrix server delegation records                |

use std::collections::HashSet;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

/// `dns_hygiene_lint` phase.
#[derive(Debug, Default)]
pub struct DnsHygieneLintPhase;

impl Phase for DnsHygieneLintPhase {
    fn name(&self) -> &'static str {
        "dns_hygiene_lint"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let Some(cfg) = forge_toml_dns_hygiene(&ctx.root) else {
            tracing::debug!("dns_hygiene_lint: no [dns_hygiene] section — skip");
            return Ok(vec![]);
        };

        let mut findings = Vec::new();
        let records = required_records_for(&cfg);
        for record in records {
            if cfg.confirmed.contains(&record.id) {
                continue;
            }
            findings.push(Finding::warn(
                self.name(),
                record.id.clone(),
                format!(
                    "{description}\n  Record needed at {domain}:\n  {example}\n  \
                     Confirm by adding `confirmed = [\"{id}\"]` to \
                     [dns_hygiene] in forge.toml once set at registrar.",
                    description = record.description,
                    domain = cfg.domain,
                    example = record.example_for(&cfg.domain),
                    id = record.id,
                ),
            ));
        }
        Ok(findings)
    }
}

#[derive(Debug, Clone)]
struct RequiredRecord {
    id: String,
    description: &'static str,
    /// Template for the record value. `{domain}` substituted with
    /// the operator's domain at emission time.
    template: &'static str,
}

impl RequiredRecord {
    fn example_for(&self, domain: &str) -> String {
        self.template.replace("{domain}", domain)
    }
}

fn required_records_for(cfg: &DnsHygieneConfig) -> Vec<RequiredRecord> {
    let mut out = Vec::new();
    if cfg.features.contains("email") {
        out.push(RequiredRecord {
            id: "spf".to_owned(),
            description: "SPF record — declares which servers may send mail FROM @{domain}; without it, your mail lands in spam",
            template: "{domain}.  TXT  \"v=spf1 ip4:<your-mail-server-ip> -all\"",
        });
        out.push(RequiredRecord {
            id: "dkim_default".to_owned(),
            description: "DKIM record (default selector) — cryptographic sender-authentication; required for major mail providers (Gmail, Outlook) to accept your mail",
            template: "default._domainkey.{domain}.  TXT  \"v=DKIM1; k=rsa; p=<your-public-key>\"",
        });
        out.push(RequiredRecord {
            id: "dmarc".to_owned(),
            description: "DMARC record — tells receiving servers what to do when SPF/DKIM fail (start at p=none for monitoring, escalate to quarantine/reject)",
            template: "_dmarc.{domain}.  TXT  \"v=DMARC1; p=none; rua=mailto:dmarc-reports@{domain}\"",
        });
    }
    if cfg.features.contains("mta_sts") {
        out.push(RequiredRecord {
            id: "mta_sts".to_owned(),
            description: "MTA-STS TXT — declares that senders MUST use TLS when delivering mail to {domain}; pair with /.well-known/mta-sts.txt on the HTTPS surface",
            template: "_mta-sts.{domain}.  TXT  \"v=STSv1; id=<timestamp>\"",
        });
    }
    if cfg.features.contains("bimi") {
        out.push(RequiredRecord {
            id: "bimi".to_owned(),
            description: "BIMI TXT — display your verified logo next to mail in supporting clients (Gmail / Apple Mail / etc); requires DMARC at p=quarantine or p=reject + VMC certificate",
            template: "default._bimi.{domain}.  TXT  \"v=BIMI1; l=https://{domain}/bimi-logo.svg; a=https://{domain}/vmc-cert.pem\"",
        });
    }
    if cfg.features.contains("hsts_preload") {
        out.push(RequiredRecord {
            id: "hsts_preload".to_owned(),
            description: "HSTS preload — submit {domain} to https://hstspreload.org/ AFTER serving Strict-Transport-Security with max-age >= 31536000; preload-list inclusion is browser-side, NOT a DNS record, but required for hsts_preload completeness",
            template: "(no DNS record — submit at https://hstspreload.org/ once HSTS header live)",
        });
    }
    if cfg.features.contains("caa") {
        out.push(RequiredRecord {
            id: "caa".to_owned(),
            description: "CAA records — restrict which Certificate Authorities may issue certs for {domain}; defense against CA-side mis-issuance",
            template: "{domain}.  CAA  0 issue \"letsencrypt.org\"\n  {domain}.  CAA  0 iodef \"mailto:security@{domain}\"",
        });
    }
    if cfg.features.contains("dnssec") {
        out.push(RequiredRecord {
            id: "dnssec".to_owned(),
            description: "DNSSEC — sign the zone at the registrar. Without DNSSEC, DNS queries are spoofable by anyone in the network path",
            template: "(registrar-side action — enable DNSSEC for {domain} in registrar control panel; DS record auto-published to parent zone)",
        });
    }
    if cfg.features.contains("security_txt") {
        out.push(RequiredRecord {
            id: "security_txt_well_known".to_owned(),
            description: "security.txt — file at /.well-known/security.txt declares vulnerability-disclosure contact (RFC 9116); pair with phase_required_pages config to verify file presence",
            template: "(no DNS record — file at https://{domain}/.well-known/security.txt with Contact: + Expires:)",
        });
    }
    if cfg.features.contains("matrix") {
        out.push(RequiredRecord {
            id: "matrix_server_delegation".to_owned(),
            description: "Matrix server delegation — federate your homeserver under {domain} without running it ON that host",
            template: "_matrix._tcp.{domain}.  SRV  10 5 8448 matrix.{domain}.\n  {domain}/.well-known/matrix/server  {{\"m.server\": \"matrix.{domain}:8448\"}}",
        });
    }
    out
}

#[derive(Debug, Clone, Default)]
struct DnsHygieneConfig {
    domain: String,
    features: HashSet<String>,
    confirmed: HashSet<String>,
}

fn forge_toml_dns_hygiene(root: &Path) -> Option<DnsHygieneConfig> {
    let path = root.join("forge.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    let parsed: toml::Value = content.parse().ok()?;
    let section = parsed.get("dns_hygiene")?;
    let domain = section
        .get("domain")
        .and_then(|v| v.as_str())
        .unwrap_or("example.com")
        .to_owned();
    let features: HashSet<String> = section
        .get("features")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_ascii_lowercase()))
                .collect()
        })
        .unwrap_or_default();
    let confirmed: HashSet<String> = section
        .get("confirmed")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    Some(DnsHygieneConfig {
        domain,
        features,
        confirmed,
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

    #[test]
    fn no_config_silent_skip() {
        let dir = tempfile::tempdir().unwrap();
        let findings = DnsHygieneLintPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn email_feature_emits_three_records() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[dns_hygiene]
domain = "example.com"
features = ["email"]
"#,
        );
        let findings = DnsHygieneLintPhase.run(&ctx_in(dir.path())).unwrap();
        let ids: HashSet<String> = findings.iter().map(|f| f.path.clone()).collect();
        assert!(ids.contains("spf"));
        assert!(ids.contains("dkim_default"));
        assert!(ids.contains("dmarc"));
        // All Warn (Forge can't verify external DNS state)
        assert!(findings.iter().all(|f| f.severity == Severity::Warn));
    }

    #[test]
    fn confirmed_records_suppressed() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[dns_hygiene]
domain = "example.com"
features = ["email"]
confirmed = ["spf", "dkim_default"]
"#,
        );
        let findings = DnsHygieneLintPhase.run(&ctx_in(dir.path())).unwrap();
        let ids: HashSet<String> = findings.iter().map(|f| f.path.clone()).collect();
        assert!(!ids.contains("spf"));
        assert!(!ids.contains("dkim_default"));
        // dmarc still needed
        assert!(ids.contains("dmarc"));
    }

    #[test]
    fn domain_substituted_into_record_template() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[dns_hygiene]
domain = "plausiden.com"
features = ["email"]
"#,
        );
        let findings = DnsHygieneLintPhase.run(&ctx_in(dir.path())).unwrap();
        let spf_finding = findings
            .iter()
            .find(|f| f.path == "spf")
            .expect("spf finding present");
        assert!(spf_finding.message.contains("plausiden.com"));
        let dmarc_finding = findings
            .iter()
            .find(|f| f.path == "dmarc")
            .expect("dmarc finding present");
        assert!(dmarc_finding.message.contains("_dmarc.plausiden.com"));
        assert!(dmarc_finding
            .message
            .contains("dmarc-reports@plausiden.com"));
    }

    #[test]
    fn caa_feature_emits_caa_records() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[dns_hygiene]
domain = "example.com"
features = ["caa"]
"#,
        );
        let findings = DnsHygieneLintPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings.iter().any(|f| f.path == "caa"));
    }

    #[test]
    fn mta_sts_feature_emits_record() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[dns_hygiene]
domain = "example.com"
features = ["mta_sts"]
"#,
        );
        let findings = DnsHygieneLintPhase.run(&ctx_in(dir.path())).unwrap();
        assert!(findings.iter().any(|f| f.path == "mta_sts"));
    }

    #[test]
    fn bimi_feature_emits_record_and_mentions_dmarc_requirement() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[dns_hygiene]
domain = "example.com"
features = ["bimi"]
"#,
        );
        let findings = DnsHygieneLintPhase.run(&ctx_in(dir.path())).unwrap();
        let bimi = findings.iter().find(|f| f.path == "bimi").expect("bimi");
        assert!(bimi.message.contains("DMARC at p=quarantine"));
    }

    #[test]
    fn hsts_preload_emits_no_dns_record_marker() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[dns_hygiene]
domain = "example.com"
features = ["hsts_preload"]
"#,
        );
        let findings = DnsHygieneLintPhase.run(&ctx_in(dir.path())).unwrap();
        let hsts = findings
            .iter()
            .find(|f| f.path == "hsts_preload")
            .expect("hsts");
        assert!(hsts.message.contains("hstspreload.org"));
        assert!(hsts.message.contains("no DNS record"));
    }

    #[test]
    fn dnssec_feature_emits_registrar_action() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[dns_hygiene]
domain = "example.com"
features = ["dnssec"]
"#,
        );
        let findings = DnsHygieneLintPhase.run(&ctx_in(dir.path())).unwrap();
        let dnssec = findings
            .iter()
            .find(|f| f.path == "dnssec")
            .expect("dnssec");
        assert!(dnssec.message.contains("registrar"));
    }

    #[test]
    fn multiple_features_emit_combined_checklist() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[dns_hygiene]
domain = "example.com"
features = ["email", "caa", "mta_sts", "dnssec", "bimi"]
"#,
        );
        let findings = DnsHygieneLintPhase.run(&ctx_in(dir.path())).unwrap();
        let ids: HashSet<String> = findings.iter().map(|f| f.path.clone()).collect();
        // email → 3 + caa + mta_sts + dnssec + bimi = 7
        assert!(ids.contains("spf"));
        assert!(ids.contains("dkim_default"));
        assert!(ids.contains("dmarc"));
        assert!(ids.contains("caa"));
        assert!(ids.contains("mta_sts"));
        assert!(ids.contains("dnssec"));
        assert!(ids.contains("bimi"));
        assert_eq!(findings.len(), 7);
    }

    #[test]
    fn empty_features_no_findings() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[dns_hygiene]
domain = "example.com"
"#,
        );
        let findings = DnsHygieneLintPhase.run(&ctx_in(dir.path())).unwrap();
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn message_includes_confirm_instructions() {
        let dir = tempfile::tempdir().unwrap();
        write_forge_toml(
            dir.path(),
            r#"
[dns_hygiene]
domain = "example.com"
features = ["email"]
"#,
        );
        let findings = DnsHygieneLintPhase.run(&ctx_in(dir.path())).unwrap();
        let spf = findings.iter().find(|f| f.path == "spf").unwrap();
        assert!(spf.message.contains("confirmed = [\"spf\"]"));
    }
}
