//! `domains-core` — typed custom-domain + SSL contract.
//!
//! Per `PLATFORM_ROADMAP.md` §20, every tenant can
//! attach their own custom domain with auto-managed Let's Encrypt
//! (or any ACME RFC 8555 CA) certificates including wildcards,
//! HSTS-preload compatibility, and one-click verification.
//!
//! This crate defines the typed contract:
//!   * Domain — apex / subdomain / wildcard kinds
//!   * DomainVerification — DNS / HTTP-01 / DNS-01 challenges
//!   * CertState — lifecycle for an ACME certificate request
//!   * HstsPolicy — RFC 6797 + hstspreload.org submission rules
//!
//! Actual ACME client lives downstream (e.g. `domains-acme-rustls`
//! built on rustls-acme); this crate is the cross-impl shape.
//!
//! ### Why typed
//!
//! Custom-domain pipelines are the canonical place where a tenant
//! "verified" status drifts ("dns-01 propagated but cert renewal
//! tried http-01 and got the wrong path"), where wildcard support
//! is forgotten on the renewal path, and where HSTS preload
//! submission criteria (max-age ≥ 31536000, includeSubDomains,
//! preload) drift between sites. Closed enums + per-challenge
//! invariants prevent each at the type-checker.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Closed enum of domain kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DomainKind {
    /// Apex (root) domain like `example.com`.
    Apex,
    /// Subdomain like `blog.example.com`.
    Subdomain,
    /// Wildcard like `*.example.com`. Requires DNS-01 challenge
    /// (HTTP-01 cannot validate wildcards per RFC 8555 §8.4).
    Wildcard,
}

impl DomainKind {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Apex => "apex",
            Self::Subdomain => "subdomain",
            Self::Wildcard => "wildcard",
        }
    }

    /// Whether this kind requires DNS-01 challenge (the only one
    /// that can validate wildcards). True for Wildcard, false
    /// otherwise.
    pub fn requires_dns_01(&self) -> bool {
        matches!(self, Self::Wildcard)
    }
}

/// A domain the tenant wants to attach.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Domain {
    /// Fully-qualified domain name. For wildcard, includes
    /// the literal leading "*.".
    pub fqdn: String,
    /// Domain kind.
    pub kind: DomainKind,
}

impl Domain {
    /// Validate the FQDN shape:
    ///   * lowercase ASCII letters / digits / hyphens / dots
    ///     (Wildcard kind allows a leading "*.")
    ///   * each label ≤ 63 chars; total ≤ 253 (RFC 1035 §2.3.4)
    ///   * no empty labels; no consecutive dots
    pub fn validate(&self) -> Result<(), DomainError> {
        let raw = self.fqdn.as_str();
        let core = if let DomainKind::Wildcard = self.kind {
            raw.strip_prefix("*.").ok_or_else(|| {
                DomainError::Invalid(format!("wildcard kind requires leading \"*.\": {}", raw))
            })?
        } else {
            if raw.starts_with("*.") {
                return Err(DomainError::Invalid(format!(
                    "leading \"*.\" requires Wildcard kind: {}",
                    raw
                )));
            }
            raw
        };
        if core.is_empty() || core.len() > 253 {
            return Err(DomainError::Invalid(format!(
                "fqdn length out of [1, 253]: {}",
                core.len()
            )));
        }
        for label in core.split('.') {
            if label.is_empty() {
                return Err(DomainError::Invalid("empty DNS label".into()));
            }
            if label.len() > 63 {
                return Err(DomainError::Invalid(format!(
                    "DNS label > 63 chars: {}",
                    label
                )));
            }
            for c in label.chars() {
                if !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '-' {
                    return Err(DomainError::Invalid(format!(
                        "invalid char in label: {:?}",
                        c
                    )));
                }
            }
            if label.starts_with('-') || label.ends_with('-') {
                return Err(DomainError::Invalid(format!(
                    "label starts/ends with hyphen: {}",
                    label
                )));
            }
        }
        Ok(())
    }
}

/// ACME challenge type per RFC 8555 §8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AcmeChallenge {
    /// HTTP-01 — operator hosts a file at
    /// `/.well-known/acme-challenge/<token>`. Cannot validate
    /// wildcards (RFC 8555 §8.4).
    #[serde(rename = "http-01")]
    Http01,
    /// DNS-01 — operator publishes a TXT record at
    /// `_acme-challenge.<domain>`. Required for wildcards.
    #[serde(rename = "dns-01")]
    Dns01,
    /// TLS-ALPN-01 — operator serves a special TLS handshake
    /// (RFC 8737). Cannot validate wildcards.
    #[serde(rename = "tls-alpn-01")]
    TlsAlpn01,
}

impl AcmeChallenge {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Http01 => "http-01",
            Self::Dns01 => "dns-01",
            Self::TlsAlpn01 => "tls-alpn-01",
        }
    }

    /// Whether the challenge can validate a wildcard domain.
    /// Only DNS-01 can per RFC 8555 §8.4.
    pub fn validates_wildcard(&self) -> bool {
        matches!(self, Self::Dns01)
    }
}

/// Cross-check that a challenge is compatible with a domain
/// kind. Wildcards REQUIRE DNS-01.
pub fn challenge_compatible(
    domain_kind: DomainKind,
    challenge: AcmeChallenge,
) -> Result<(), DomainError> {
    if domain_kind == DomainKind::Wildcard && !challenge.validates_wildcard() {
        return Err(DomainError::ChallengeIncompatible {
            domain_kind,
            challenge,
        });
    }
    Ok(())
}

/// Domain verification lifecycle. Operator wants their FQDN
/// pointed at the platform before cert issuance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DomainVerification {
    /// CNAME / A / AAAA record not yet pointing at platform.
    DnsPending,
    /// DNS-01 challenge token not yet propagated.
    #[serde(rename = "dns-01-pending")]
    Dns01Pending,
    /// HTTP-01 token not yet servable.
    #[serde(rename = "http-01-pending")]
    Http01Pending,
    /// All checks passed; cert issuance is unblocked.
    Verified,
    /// Verification failed permanently (operator must restart).
    Failed,
}

impl DomainVerification {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::DnsPending => "dns-pending",
            Self::Dns01Pending => "dns-01-pending",
            Self::Http01Pending => "http-01-pending",
            Self::Verified => "verified",
            Self::Failed => "failed",
        }
    }

    /// Whether the state is terminal.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Verified | Self::Failed)
    }
}

/// Certificate issuance lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CertState {
    /// Order created at ACME directory.
    OrderCreated,
    /// Challenge presented; awaiting CA validation.
    AwaitingValidation,
    /// CA accepted the challenge.
    Validated,
    /// Order finalized; cert downloaded.
    Issued,
    /// Cert nearing expiry (T-30 days by convention).
    RenewalDue,
    /// Renewal in flight.
    Renewing,
    /// Order failed; operator must inspect.
    Failed,
    /// Cert revoked.
    Revoked,
}

impl CertState {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::OrderCreated => "order-created",
            Self::AwaitingValidation => "awaiting-validation",
            Self::Validated => "validated",
            Self::Issued => "issued",
            Self::RenewalDue => "renewal-due",
            Self::Renewing => "renewing",
            Self::Failed => "failed",
            Self::Revoked => "revoked",
        }
    }

    /// Whether the cert is currently usable for TLS.
    pub fn is_usable(&self) -> bool {
        matches!(self, Self::Issued | Self::RenewalDue | Self::Renewing)
    }
}

/// HSTS policy. Built on RFC 6797 + the hstspreload.org
/// submission rules:
///   * max_age_secs ≥ 31_536_000 (1 year)
///   * includeSubDomains required
///   * preload directive set
///   * served over HTTPS only
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct HstsPolicy {
    /// max-age directive (seconds).
    pub max_age_secs: u32,
    /// includeSubDomains directive.
    pub include_subdomains: bool,
    /// preload directive (intent to submit to hstspreload.org).
    pub preload: bool,
}

impl HstsPolicy {
    /// The platform default: max-age = 2 years, includeSubDomains,
    /// preload. Submitting to hstspreload.org is operator-side.
    pub fn platform_default() -> Self {
        Self {
            max_age_secs: 2 * 365 * 24 * 60 * 60,
            include_subdomains: true,
            preload: true,
        }
    }

    /// Whether this policy is eligible for hstspreload.org
    /// submission per their current rules.
    pub fn is_preload_eligible(&self) -> bool {
        self.max_age_secs >= 31_536_000 && self.include_subdomains && self.preload
    }

    /// Render the Strict-Transport-Security HTTP header value.
    pub fn header_value(&self) -> String {
        let mut s = format!("max-age={}", self.max_age_secs);
        if self.include_subdomains {
            s.push_str("; includeSubDomains");
        }
        if self.preload {
            s.push_str("; preload");
        }
        s
    }
}

impl Default for HstsPolicy {
    fn default() -> Self {
        Self::platform_default()
    }
}

/// Typed errors at the domain boundary.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    /// Domain validation refused.
    #[error("invalid: {0}")]
    Invalid(String),
    /// Challenge / kind incompatibility.
    #[error("challenge {challenge:?} cannot validate kind {domain_kind:?}")]
    ChallengeIncompatible {
        /// Domain kind.
        domain_kind: DomainKind,
        /// Challenge.
        challenge: AcmeChallenge,
    },
    /// ACME backend error.
    #[error("acme: {0}")]
    Acme(String),
}

/// Per-CA ACME client. Impl crates land per CA / library
/// (domains-acme-rustls, domains-acme-acme2).
pub trait AcmeClient {
    /// Stable identifier (e.g. "letsencrypt-v2", "buypass-go").
    fn directory_id(&self) -> &'static str;
    /// Begin issuance for a given (domain, challenge) pair.
    fn order(&self, domain: &Domain, challenge: AcmeChallenge) -> Result<CertState, DomainError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_kind_slugs_distinct() {
        let ks = [
            DomainKind::Apex,
            DomainKind::Subdomain,
            DomainKind::Wildcard,
        ];
        let mut s = std::collections::HashSet::new();
        for k in ks {
            assert!(s.insert(k.slug()));
        }
    }

    #[test]
    fn only_wildcard_requires_dns_01() {
        assert!(DomainKind::Wildcard.requires_dns_01());
        assert!(!DomainKind::Apex.requires_dns_01());
        assert!(!DomainKind::Subdomain.requires_dns_01());
    }

    #[test]
    fn challenge_slugs_distinct() {
        let cs = [
            AcmeChallenge::Http01,
            AcmeChallenge::Dns01,
            AcmeChallenge::TlsAlpn01,
        ];
        let mut s = std::collections::HashSet::new();
        for c in cs {
            assert!(s.insert(c.slug()));
        }
    }

    #[test]
    fn only_dns_01_validates_wildcard() {
        assert!(AcmeChallenge::Dns01.validates_wildcard());
        assert!(!AcmeChallenge::Http01.validates_wildcard());
        assert!(!AcmeChallenge::TlsAlpn01.validates_wildcard());
    }

    // Regression-guard: serde's `rename_all = "kebab-case"`
    // does NOT insert a hyphen between adjacent lowercase /
    // digit characters, so `Http01` becomes `http01` (not
    // `http-01`). The slug() helper returns the human-friendly
    // `http-01` form. Without per-variant `#[serde(rename)]`,
    // serde + slug() disagree silently — the kind of bug that
    // breaks a TOML parser without ever firing a unit test.
    // This test asserts they match for every challenge variant.
    #[test]
    fn challenge_serde_wire_format_matches_slug() {
        for c in [
            AcmeChallenge::Http01,
            AcmeChallenge::Dns01,
            AcmeChallenge::TlsAlpn01,
        ] {
            let wire = serde_json::to_string(&c).unwrap();
            // Strip surrounding quotes: "\"http-01\"" → "http-01"
            let stripped = wire.trim_matches('"');
            assert_eq!(stripped, c.slug(), "wire vs slug for {:?}", c);
        }
    }

    #[test]
    fn verification_serde_wire_format_matches_slug() {
        for v in [
            DomainVerification::DnsPending,
            DomainVerification::Dns01Pending,
            DomainVerification::Http01Pending,
            DomainVerification::Verified,
            DomainVerification::Failed,
        ] {
            let wire = serde_json::to_string(&v).unwrap();
            let stripped = wire.trim_matches('"');
            assert_eq!(stripped, v.slug(), "wire vs slug for {:?}", v);
        }
    }

    #[test]
    fn challenge_compatible_wildcard_only_with_dns_01() {
        assert!(challenge_compatible(DomainKind::Wildcard, AcmeChallenge::Dns01).is_ok());
        assert!(challenge_compatible(DomainKind::Wildcard, AcmeChallenge::Http01).is_err());
        assert!(challenge_compatible(DomainKind::Wildcard, AcmeChallenge::TlsAlpn01).is_err());
        assert!(challenge_compatible(DomainKind::Apex, AcmeChallenge::Http01).is_ok());
        assert!(challenge_compatible(DomainKind::Subdomain, AcmeChallenge::TlsAlpn01).is_ok());
    }

    #[test]
    fn domain_validates_apex_ok() {
        let d = Domain {
            fqdn: "example.com".into(),
            kind: DomainKind::Apex,
        };
        assert!(d.validate().is_ok());
    }

    #[test]
    fn domain_validates_subdomain_ok() {
        let d = Domain {
            fqdn: "blog.example.com".into(),
            kind: DomainKind::Subdomain,
        };
        assert!(d.validate().is_ok());
    }

    #[test]
    fn domain_validates_wildcard_with_star_prefix() {
        let d = Domain {
            fqdn: "*.example.com".into(),
            kind: DomainKind::Wildcard,
        };
        assert!(d.validate().is_ok());
    }

    #[test]
    fn domain_rejects_wildcard_without_star_prefix() {
        let d = Domain {
            fqdn: "example.com".into(),
            kind: DomainKind::Wildcard,
        };
        assert!(d.validate().is_err());
    }

    #[test]
    fn domain_rejects_star_prefix_on_non_wildcard() {
        let d = Domain {
            fqdn: "*.example.com".into(),
            kind: DomainKind::Apex,
        };
        assert!(d.validate().is_err());
    }

    #[test]
    fn domain_rejects_uppercase() {
        let d = Domain {
            fqdn: "Example.com".into(),
            kind: DomainKind::Apex,
        };
        assert!(d.validate().is_err());
    }

    #[test]
    fn domain_rejects_leading_hyphen_in_label() {
        let d = Domain {
            fqdn: "-bad.example.com".into(),
            kind: DomainKind::Subdomain,
        };
        assert!(d.validate().is_err());
    }

    #[test]
    fn domain_rejects_consecutive_dots() {
        let d = Domain {
            fqdn: "bad..example.com".into(),
            kind: DomainKind::Subdomain,
        };
        assert!(d.validate().is_err());
    }

    #[test]
    fn domain_rejects_label_over_63_chars() {
        let label = "a".repeat(64);
        let d = Domain {
            fqdn: format!("{}.example.com", label),
            kind: DomainKind::Subdomain,
        };
        assert!(d.validate().is_err());
    }

    #[test]
    fn verification_terminal_set() {
        assert!(DomainVerification::Verified.is_terminal());
        assert!(DomainVerification::Failed.is_terminal());
        assert!(!DomainVerification::DnsPending.is_terminal());
    }

    #[test]
    fn cert_usable_set() {
        assert!(CertState::Issued.is_usable());
        assert!(CertState::RenewalDue.is_usable());
        assert!(CertState::Renewing.is_usable());
        assert!(!CertState::OrderCreated.is_usable());
        assert!(!CertState::Failed.is_usable());
        assert!(!CertState::Revoked.is_usable());
    }

    #[test]
    fn hsts_platform_default_is_preload_eligible() {
        let p = HstsPolicy::platform_default();
        assert!(p.is_preload_eligible());
        assert!(p.max_age_secs >= 31_536_000);
        assert!(p.include_subdomains);
        assert!(p.preload);
    }

    #[test]
    fn hsts_header_value_serializes_directives_in_order() {
        let p = HstsPolicy {
            max_age_secs: 31_536_000,
            include_subdomains: true,
            preload: true,
        };
        assert_eq!(
            p.header_value(),
            "max-age=31536000; includeSubDomains; preload"
        );
    }

    #[test]
    fn hsts_header_value_skips_unset_directives() {
        let p = HstsPolicy {
            max_age_secs: 60,
            include_subdomains: false,
            preload: false,
        };
        assert_eq!(p.header_value(), "max-age=60");
    }

    #[test]
    fn hsts_preload_eligibility_strict() {
        // Under 1 year: not eligible.
        let p1 = HstsPolicy {
            max_age_secs: 31_535_999,
            include_subdomains: true,
            preload: true,
        };
        assert!(!p1.is_preload_eligible());
        // No includeSubDomains: not eligible.
        let p2 = HstsPolicy {
            max_age_secs: 31_536_000,
            include_subdomains: false,
            preload: true,
        };
        assert!(!p2.is_preload_eligible());
        // No preload: not eligible.
        let p3 = HstsPolicy {
            max_age_secs: 31_536_000,
            include_subdomains: true,
            preload: false,
        };
        assert!(!p3.is_preload_eligible());
    }

    #[test]
    fn domain_serde_round_trip() {
        let d = Domain {
            fqdn: "*.example.com".into(),
            kind: DomainKind::Wildcard,
        };
        let j = serde_json::to_string(&d).unwrap();
        let back: Domain = serde_json::from_str(&j).unwrap();
        assert_eq!(d, back);
    }

    #[test]
    fn domain_rejects_unknown_field() {
        let bad = r#"{"fqdn":"x.com","kind":"apex","ahem":1}"#;
        let r: Result<Domain, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }
}
