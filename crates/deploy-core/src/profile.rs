//! Typed security/anonymity profile every adapter declares.
//!
//! The admin UI (task #43 — security rating dashboard) consumes
//! this directly: each adapter's `profile()` return value feeds
//! the per-target rating without any free-form interpretation.

use serde::{Deserialize, Serialize};

/// What an adapter promises about the deployment.
///
/// **Adapter's intrinsic properties.** This is what the adapter
/// CAN deliver if its prerequisites are met (Tor daemon running,
/// peer reachable, etc.). The deployment's current health is a
/// separate runtime concern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SecurityProfile {
    /// Source-address anonymity for the *reader*.
    pub reader_anonymity: AnonymityLevel,
    /// Source-address anonymity for the *publisher* (the operator
    /// running `forge deploy`).
    pub publisher_anonymity: AnonymityLevel,
    /// How visible the traffic is to passive on-path adversaries
    /// (ISPs, transit providers, state actors monitoring backbone).
    pub traffic_observability: TrafficObservability,
    /// How resilient the deployment is to active censorship
    /// (state-level blocks, DNS poisoning, etc.).
    pub censorship_resistance: CensorshipResistance,
    /// Whether content is cryptographically content-addressed
    /// (immutable; tampering produces a new address).
    pub content_addressed: bool,
    /// Whether the platform's standard TLS chain applies (false
    /// for Tor/I2P/IPFS/Gemini, true for clearnet HTTPS).
    pub uses_standard_tls: bool,
}

impl SecurityProfile {
    /// Baseline profile for a clearnet HTTPS deploy. Anonymity is
    /// "depends on the user's connection" (none by default); TLS
    /// is on; observability is high.
    pub const fn clearnet_baseline() -> Self {
        Self {
            reader_anonymity: AnonymityLevel::None,
            publisher_anonymity: AnonymityLevel::None,
            traffic_observability: TrafficObservability::High,
            censorship_resistance: CensorshipResistance::Low,
            content_addressed: false,
            uses_standard_tls: true,
        }
    }

    /// Baseline profile for a Tor v3 hidden service.
    pub const fn tor_onion_baseline() -> Self {
        Self {
            reader_anonymity: AnonymityLevel::Strong,
            publisher_anonymity: AnonymityLevel::Strong,
            traffic_observability: TrafficObservability::Low,
            censorship_resistance: CensorshipResistance::High,
            content_addressed: false,
            uses_standard_tls: false,
        }
    }

    /// Baseline profile for an IPFS / IPNS deploy.
    pub const fn ipfs_baseline() -> Self {
        Self {
            reader_anonymity: AnonymityLevel::None,
            publisher_anonymity: AnonymityLevel::None,
            traffic_observability: TrafficObservability::Medium,
            censorship_resistance: CensorshipResistance::Medium,
            content_addressed: true,
            uses_standard_tls: false,
        }
    }
}

/// Discrete anonymity rating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AnonymityLevel {
    /// Source address visible to the destination + on-path
    /// adversaries.
    #[default]
    None,
    /// Source address is visible but stripped of trivial
    /// fingerprinting (e.g. behind a VPN that doesn't log).
    Partial,
    /// Source address cryptographically separated from request
    /// content (Tor / I2P / Lokinet).
    Strong,
}

/// Discrete traffic-observability rating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TrafficObservability {
    /// ISP / on-path adversary cannot determine source or destination.
    Low,
    /// Metadata (destination, sometimes timing) visible.
    #[default]
    Medium,
    /// Full visibility — plaintext or TLS-with-SNI.
    High,
}

/// Discrete censorship-resistance rating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CensorshipResistance {
    /// A state can trivially block the deployment (DNS, IP, BGP).
    #[default]
    Low,
    /// Block requires nontrivial effort (deep-packet inspection,
    /// active probing).
    Medium,
    /// Block requires shutting down the underlying protocol layer
    /// (e.g. blocking Tor altogether, IPFS at the gateway level).
    High,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clearnet_baseline_is_consistent() {
        let p = SecurityProfile::clearnet_baseline();
        assert_eq!(p.reader_anonymity, AnonymityLevel::None);
        assert_eq!(p.publisher_anonymity, AnonymityLevel::None);
        assert!(p.uses_standard_tls);
        assert!(!p.content_addressed);
    }

    #[test]
    fn tor_baseline_is_consistent() {
        let p = SecurityProfile::tor_onion_baseline();
        assert_eq!(p.reader_anonymity, AnonymityLevel::Strong);
        assert_eq!(p.publisher_anonymity, AnonymityLevel::Strong);
        assert!(!p.uses_standard_tls);
        assert_eq!(p.censorship_resistance, CensorshipResistance::High);
    }

    #[test]
    fn ipfs_baseline_is_content_addressed() {
        let p = SecurityProfile::ipfs_baseline();
        assert!(p.content_addressed);
    }

    #[test]
    fn profile_serde_round_trip() {
        let p = SecurityProfile::tor_onion_baseline();
        let s = serde_json::to_string(&p).unwrap();
        let back: SecurityProfile = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }
}
