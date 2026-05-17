//! Typed deploy target — a kebab-case ID + a closed-enum
//! [`NetworkClass`] + optional public URL + extra fields.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One deployment target the platform ships to.
///
/// Multiple targets per site are normal — `clearnet` + `tor-onion`
/// + `ipfs-ipns` is a common triplet for high-anonymity
/// publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DeployTarget {
    /// Stable identifier (e.g. `"tor-main"`). Must be kebab-case +
    /// unique within a site.
    pub id: String,
    /// Which network class this target lives on.
    pub class: NetworkClass,
    /// Public URL once deployed, if known at config time. Onion
    /// services and IPNS names get filled in by the adapter on
    /// first publish.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_url: Option<String>,
    /// Adapter-specific fields (auth keys, peer hints, eepsite
    /// settings, etc.). Adapters parse this map on their own
    /// schema.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Closed enum of network classes the platform recognises.
///
/// `phase_network_target_enforcement` reads this discriminator
/// to decide whether content (e.g. inline clearnet URLs, remote
/// fonts, embedded analytics) is permitted on the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkClass {
    /// Standard HTTPS over the open internet.
    Clearnet,
    /// Tor v3 hidden service (.onion).
    TorOnion,
    /// I2P eepsite (.b32.i2p).
    I2pEepsite,
    /// IPFS / IPNS — content-addressed.
    Ipfs,
    /// Gemini protocol (gemini://).
    Gemini,
    /// Lokinet alternative-mesh (.loki).
    Lokinet,
}

impl NetworkClass {
    /// All known variants, in declaration order. Used by admin UI
    /// + the deploy adapter coverage gate.
    pub const ALL: &'static [NetworkClass] = &[
        NetworkClass::Clearnet,
        NetworkClass::TorOnion,
        NetworkClass::I2pEepsite,
        NetworkClass::Ipfs,
        NetworkClass::Gemini,
        NetworkClass::Lokinet,
    ];

    /// Stable kebab-case slug for serialization + UI.
    pub fn slug(&self) -> &'static str {
        match self {
            NetworkClass::Clearnet => "clearnet",
            NetworkClass::TorOnion => "tor-onion",
            NetworkClass::I2pEepsite => "i2p-eepsite",
            NetworkClass::Ipfs => "ipfs",
            NetworkClass::Gemini => "gemini",
            NetworkClass::Lokinet => "lokinet",
        }
    }

    /// Whether outgoing traffic on this network class can be
    /// trivially observed by ISPs + on-path adversaries.
    pub fn observable_by_isp(&self) -> bool {
        match self {
            NetworkClass::Clearnet => true,
            NetworkClass::Gemini => true,
            NetworkClass::TorOnion => false,
            NetworkClass::I2pEepsite => false,
            NetworkClass::Ipfs => true, // metadata at the gateway
            NetworkClass::Lokinet => false,
        }
    }

    /// Whether this class provides cryptographic source-address
    /// anonymity by default.
    pub fn anonymous_by_default(&self) -> bool {
        matches!(
            self,
            NetworkClass::TorOnion | NetworkClass::I2pEepsite | NetworkClass::Lokinet
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_classes_have_distinct_slugs() {
        let mut seen = std::collections::HashSet::new();
        for c in NetworkClass::ALL {
            assert!(seen.insert(c.slug()), "duplicate slug for {c:?}");
        }
        assert_eq!(seen.len(), NetworkClass::ALL.len());
    }

    #[test]
    fn slugs_round_trip_serde() {
        for c in NetworkClass::ALL {
            let s = serde_json::to_string(c).unwrap();
            let back: NetworkClass = serde_json::from_str(&s).unwrap();
            assert_eq!(*c, back);
        }
    }

    #[test]
    fn anonymity_classification_matches_intent() {
        assert!(NetworkClass::TorOnion.anonymous_by_default());
        assert!(NetworkClass::I2pEepsite.anonymous_by_default());
        assert!(NetworkClass::Lokinet.anonymous_by_default());
        assert!(!NetworkClass::Clearnet.anonymous_by_default());
        assert!(!NetworkClass::Gemini.anonymous_by_default());
        assert!(!NetworkClass::Ipfs.anonymous_by_default());
    }

    #[test]
    fn isp_observability_classification_matches_intent() {
        assert!(NetworkClass::Clearnet.observable_by_isp());
        assert!(NetworkClass::Gemini.observable_by_isp());
        assert!(NetworkClass::Ipfs.observable_by_isp());
        assert!(!NetworkClass::TorOnion.observable_by_isp());
        assert!(!NetworkClass::I2pEepsite.observable_by_isp());
        assert!(!NetworkClass::Lokinet.observable_by_isp());
    }

    #[test]
    fn target_serde_round_trip() {
        let t = DeployTarget {
            id: "tor-main".into(),
            class: NetworkClass::TorOnion,
            public_url: Some("http://example.onion".into()),
            extra: Default::default(),
        };
        let s = serde_json::to_string(&t).unwrap();
        let back: DeployTarget = serde_json::from_str(&s).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn target_rejects_unknown_fields() {
        let bad = r#"{"id":"x","class":"clearnet","ahem":1}"#;
        let r: Result<DeployTarget, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }
}
