//! `forge-auth` — typed auth surface for Forge.
//!
//! Closed-enum [`AuthMethod`] / [`MfaMethod`] / [`AuthEvent`] +
//! a pluggable [`AuthProvider`] trait. Forge dispatches through
//! `&dyn AuthProvider`; concrete providers (Postgres-backed,
//! Lightning Memory-Mapped DB, SQLite, vault-backed, etc.)
//! implement the trait. The seam is the contract.
//!
//! ## Methods shipped
//!
//! - `Password`               — classic salted-Argon2id
//! - `PasswordlessEmail`      — email magic-link
//! - `PasswordlessSms`        — SMS OTP code
//! - `WebAuthnPlatform`       — Apple TouchID / Windows Hello
//! - `WebAuthnRoaming`        — YubiKey / Solo / Titan
//! - `OAuth`                  — OAuth 2.0 / OIDC (GitHub, Google, Apple)
//! - `Passkey`                — WebAuthn discoverable credential
//! - `MagicLinkSso`           — SSO single sign-on link
//! - `Anonymous`              — anonymous-but-receipt-bearing
//!                               (Sacred.Vote pattern)
//!
//! ## MFA shipped
//!
//! - `Totp`                   — RFC 6238 time-based OTP
//! - `WebAuthnSecondFactor`   — WebAuthn as 2FA
//! - `SmsOtp`                 — SMS-delivered OTP
//! - `EmailOtp`               — email-delivered OTP
//! - `BackupCodes`            — printable one-time codes
//!
//! No method or MFA option is "default-on" — operators
//! enable per tenant via typed config.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Closed enum of auth methods Forge supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthMethod {
    /// Classic password — salted Argon2id.
    Password,
    /// Email magic-link.
    PasswordlessEmail,
    /// SMS one-time code.
    PasswordlessSms,
    /// WebAuthn platform (Touch ID / Face ID / Windows Hello).
    WebAuthnPlatform,
    /// WebAuthn roaming (YubiKey / Solo / Titan).
    WebAuthnRoaming,
    /// OAuth 2.0 / OIDC (GitHub / Google / Apple / Microsoft).
    Oauth,
    /// WebAuthn discoverable credential ("passkey").
    Passkey,
    /// SSO magic-link.
    MagicLinkSso,
    /// Anonymous-but-receipt-bearing (Sacred.Vote pattern).
    Anonymous,
}

impl AuthMethod {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::PasswordlessEmail => "passwordless-email",
            Self::PasswordlessSms => "passwordless-sms",
            Self::WebAuthnPlatform => "webauthn-platform",
            Self::WebAuthnRoaming => "webauthn-roaming",
            Self::Oauth => "oauth",
            Self::Passkey => "passkey",
            Self::MagicLinkSso => "magic-link-sso",
            Self::Anonymous => "anonymous",
        }
    }

    /// Whether this method requires server-side secret storage
    /// (drives "are we PCI-DSS-style sensitive?" reasoning).
    pub fn server_stores_secret(&self) -> bool {
        matches!(self, Self::Password)
    }

    /// Whether this method is phishing-resistant by construction.
    pub fn phishing_resistant(&self) -> bool {
        matches!(
            self,
            Self::WebAuthnPlatform | Self::WebAuthnRoaming | Self::Passkey
        )
    }
}

/// Closed enum of MFA methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MfaMethod {
    /// RFC 6238 TOTP (Authy / Google Authenticator / 1Password).
    Totp,
    /// WebAuthn as second factor.
    WebAuthnSecondFactor,
    /// SMS-delivered OTP.
    SmsOtp,
    /// Email-delivered OTP.
    EmailOtp,
    /// Printable backup codes (one-time-use).
    BackupCodes,
}

impl MfaMethod {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Totp => "totp",
            Self::WebAuthnSecondFactor => "webauthn-2fa",
            Self::SmsOtp => "sms-otp",
            Self::EmailOtp => "email-otp",
            Self::BackupCodes => "backup-codes",
        }
    }

    /// Whether this is phishing-resistant.
    pub fn phishing_resistant(&self) -> bool {
        matches!(self, Self::WebAuthnSecondFactor)
    }
}

/// A challenge issued during an auth flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AuthChallenge {
    /// Stable challenge id.
    pub id: String,
    /// Method this challenge is for.
    pub method: AuthMethod,
    /// Optional sub-MFA factor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mfa: Option<MfaMethod>,
    /// Issued-at, RFC 3339.
    #[serde(with = "time::serde::rfc3339")]
    pub issued_at: time::OffsetDateTime,
    /// Expires-at, RFC 3339. Solver MUST respect this.
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: time::OffsetDateTime,
    /// Tenant scope.
    pub tenant_id: String,
}

/// The user-supplied response to an [`AuthChallenge`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AuthResponse {
    /// Challenge id this responds to.
    pub challenge_id: String,
    /// Opaque payload — concrete provider decodes (password
    /// hash, WebAuthn assertion JSON, OTP code, etc.).
    pub payload: serde_json::Value,
    /// When the response was submitted.
    #[serde(with = "time::serde::rfc3339")]
    pub submitted_at: time::OffsetDateTime,
}

/// Verdict of an auth attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AuthVerdict {
    /// Authentication succeeded.
    Authenticated {
        /// Subject identifier (user id / pseudonymous id).
        subject: String,
        /// Issued session token (opaque).
        session_token: String,
        /// Methods that fired during this attempt.
        methods: Vec<AuthMethod>,
    },
    /// Authentication needs an MFA second factor.
    MfaRequired {
        /// MFA factors the user can pick from.
        offered: Vec<MfaMethod>,
        /// Continuation token.
        continuation_token: String,
    },
    /// Authentication rejected.
    Rejected {
        /// Kebab-case reason slug.
        reason: String,
    },
    /// Authentication needs to retry (rate-limit, transient).
    Retry {
        /// When to retry.
        retry_after_seconds: u32,
    },
}

impl AuthVerdict {
    /// True if fully authenticated.
    pub fn is_authenticated(&self) -> bool {
        matches!(self, Self::Authenticated { .. })
    }
}

/// A typed auth session emitted on successful auth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AuthSession {
    /// Subject id.
    pub subject: String,
    /// Tenant scope.
    pub tenant_id: String,
    /// Issued-at.
    #[serde(with = "time::serde::rfc3339")]
    pub issued_at: time::OffsetDateTime,
    /// Expires-at.
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: time::OffsetDateTime,
    /// Methods used during this session's auth.
    pub methods: Vec<AuthMethod>,
    /// Whether MFA was completed.
    pub mfa_completed: bool,
}

/// Audit event emitted at every auth boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AuthEvent {
    /// Challenge issued.
    ChallengeIssued {
        /// The issued challenge.
        challenge: AuthChallenge,
    },
    /// Response received.
    ResponseReceived {
        /// Challenge id.
        challenge_id: String,
        /// When.
        #[serde(with = "time::serde::rfc3339")]
        at: time::OffsetDateTime,
    },
    /// Verdict computed.
    Verdict {
        /// Challenge id.
        challenge_id: String,
        /// The verdict.
        verdict: AuthVerdict,
    },
    /// Session minted.
    SessionMinted {
        /// The minted session.
        session: AuthSession,
    },
    /// Session revoked.
    SessionRevoked {
        /// Subject.
        subject: String,
        /// Reason slug (e.g. `logout`, `password-changed`, `admin-revoke`).
        reason: String,
        /// When.
        #[serde(with = "time::serde::rfc3339")]
        at: time::OffsetDateTime,
    },
}

/// Error type for AuthProvider calls.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// Challenge expired.
    #[error("challenge expired: {0}")]
    Expired(String),
    /// Provider unavailable.
    #[error("provider unavailable: {0}")]
    Unavailable(String),
    /// Configuration error.
    #[error("config: {0}")]
    Config(String),
    /// Internal error.
    #[error("internal: {0}")]
    Internal(String),
}

/// Per-tenant auth configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AuthConfig {
    /// Tenant.
    pub tenant_id: String,
    /// Methods allowed for primary authentication.
    pub allowed_methods: Vec<AuthMethod>,
    /// MFA factors allowed.
    pub allowed_mfa: Vec<MfaMethod>,
    /// MFA required for these methods.
    pub require_mfa_for: Vec<AuthMethod>,
    /// Session TTL in seconds.
    pub session_ttl_seconds: u32,
}

impl AuthConfig {
    /// Sensible default — passkey + passwordless email + TOTP MFA.
    pub fn modern_defaults(tenant_id: impl Into<String>) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            allowed_methods: vec![
                AuthMethod::Passkey,
                AuthMethod::PasswordlessEmail,
                AuthMethod::WebAuthnPlatform,
                AuthMethod::WebAuthnRoaming,
            ],
            allowed_mfa: vec![
                MfaMethod::Totp,
                MfaMethod::WebAuthnSecondFactor,
                MfaMethod::BackupCodes,
            ],
            require_mfa_for: vec![AuthMethod::Password],
            session_ttl_seconds: 3600 * 24 * 14,
        }
    }
}

/// Pluggable AuthProvider trait.
pub trait AuthProvider: Send + Sync {
    /// Provider identifier ("postgres-argon2", "sqlite-passkey",
    /// "vault-webauthn", etc.).
    fn ident(&self) -> &'static str;

    /// Issue a challenge for the given method + tenant.
    fn issue_challenge(
        &self,
        method: AuthMethod,
        tenant_id: &str,
    ) -> Result<AuthChallenge, AuthError>;

    /// Verify a response against a previously-issued challenge.
    fn verify(
        &self,
        challenge: &AuthChallenge,
        response: &AuthResponse,
    ) -> Result<AuthVerdict, AuthError>;

    /// Mint a session for a fully-authenticated subject.
    fn mint_session(
        &self,
        subject: &str,
        tenant_id: &str,
        methods: &[AuthMethod],
        mfa_completed: bool,
    ) -> Result<AuthSession, AuthError>;
}

/// No-op provider — returns deterministic stub data for tests +
/// pipelines without a real backend.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopAuthProvider;

impl AuthProvider for NoopAuthProvider {
    fn ident(&self) -> &'static str {
        "noop"
    }

    fn issue_challenge(
        &self,
        method: AuthMethod,
        tenant_id: &str,
    ) -> Result<AuthChallenge, AuthError> {
        let now = time::OffsetDateTime::now_utc();
        Ok(AuthChallenge {
            id: format!("noop-{}-{}", method.slug(), tenant_id),
            method,
            mfa: None,
            issued_at: now,
            expires_at: now + time::Duration::seconds(300),
            tenant_id: tenant_id.to_owned(),
        })
    }

    fn verify(
        &self,
        challenge: &AuthChallenge,
        response: &AuthResponse,
    ) -> Result<AuthVerdict, AuthError> {
        if response.submitted_at > challenge.expires_at {
            return Err(AuthError::Expired(challenge.id.clone()));
        }
        Ok(AuthVerdict::Authenticated {
            subject: format!("noop-subject-{}", challenge.tenant_id),
            session_token: format!("noop-session-{}", challenge.id),
            methods: vec![challenge.method],
        })
    }

    fn mint_session(
        &self,
        subject: &str,
        tenant_id: &str,
        methods: &[AuthMethod],
        mfa_completed: bool,
    ) -> Result<AuthSession, AuthError> {
        let now = time::OffsetDateTime::now_utc();
        Ok(AuthSession {
            subject: subject.to_owned(),
            tenant_id: tenant_id.to_owned(),
            issued_at: now,
            expires_at: now + time::Duration::seconds(3600 * 24 * 14),
            methods: methods.to_vec(),
            mfa_completed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_slugs_distinct() {
        let methods = [
            AuthMethod::Password,
            AuthMethod::PasswordlessEmail,
            AuthMethod::PasswordlessSms,
            AuthMethod::WebAuthnPlatform,
            AuthMethod::WebAuthnRoaming,
            AuthMethod::Oauth,
            AuthMethod::Passkey,
            AuthMethod::MagicLinkSso,
            AuthMethod::Anonymous,
        ];
        let mut seen = std::collections::HashSet::new();
        for m in methods {
            assert!(seen.insert(m.slug()), "duplicate {}", m.slug());
        }
    }

    #[test]
    fn phishing_resistance_marked() {
        assert!(AuthMethod::Passkey.phishing_resistant());
        assert!(AuthMethod::WebAuthnPlatform.phishing_resistant());
        assert!(!AuthMethod::Password.phishing_resistant());
        assert!(!AuthMethod::PasswordlessEmail.phishing_resistant());
    }

    #[test]
    fn modern_defaults_include_passkey() {
        let c = AuthConfig::modern_defaults("acme");
        assert!(c.allowed_methods.contains(&AuthMethod::Passkey));
        assert!(c.allowed_mfa.contains(&MfaMethod::Totp));
        assert_eq!(c.tenant_id, "acme");
    }

    #[test]
    fn noop_issues_and_verifies() {
        let p = NoopAuthProvider;
        let c = p
            .issue_challenge(AuthMethod::PasswordlessEmail, "acme")
            .unwrap();
        assert_eq!(c.tenant_id, "acme");
        assert_eq!(c.method, AuthMethod::PasswordlessEmail);
        let r = AuthResponse {
            challenge_id: c.id.clone(),
            payload: serde_json::json!({"code": "123456"}),
            submitted_at: c.issued_at + time::Duration::seconds(5),
        };
        let v = p.verify(&c, &r).unwrap();
        assert!(v.is_authenticated());
    }

    #[test]
    fn noop_refuses_expired() {
        let p = NoopAuthProvider;
        let mut c = p.issue_challenge(AuthMethod::Password, "x").unwrap();
        c.expires_at = c.issued_at - time::Duration::seconds(1);
        let r = AuthResponse {
            challenge_id: c.id.clone(),
            payload: serde_json::json!({}),
            submitted_at: c.issued_at,
        };
        assert!(matches!(p.verify(&c, &r), Err(AuthError::Expired(_))));
    }

    #[test]
    fn verdict_helpers() {
        let a = AuthVerdict::Authenticated {
            subject: "s".into(),
            session_token: "t".into(),
            methods: vec![AuthMethod::Passkey],
        };
        assert!(a.is_authenticated());
        let r = AuthVerdict::Rejected {
            reason: "wrong-password".into(),
        };
        assert!(!r.is_authenticated());
    }

    #[test]
    fn dyn_provider_works() {
        let providers: Vec<Box<dyn AuthProvider>> = vec![Box::new(NoopAuthProvider)];
        for p in &providers {
            assert_eq!(p.ident(), "noop");
        }
    }
}
