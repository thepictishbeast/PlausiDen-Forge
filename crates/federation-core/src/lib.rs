//! `federation-core` — typed federation + IndieWeb contract.
//!
//! Per `PLATFORM_ROADMAP.md` §13 + the "no walled garden" axis of
//! `super_society_tech_stack`, every PlausiDen tenant federates
//! out to the open web by default. This crate defines the
//! cross-protocol contract; per-protocol clients plug in via
//! [`FederationPublisher`].
//!
//! Supported protocols (closed enum):
//!   * ActivityPub (W3C Recommendation, 2018-01-23)
//!   * Webmention (W3C Recommendation, 2017-01-12)
//!   * WebSub (W3C Recommendation, 2018-01-23, supersedes PubSubHubbub)
//!   * Nostr (NIP-01)
//!   * AT Protocol (Bluesky, 2024+)
//!
//! ### Why typed
//!
//! Federation drift is the canonical interop nightmare — every
//! Mastodon instance sends slightly-different ActivityStreams JSON;
//! Webmention senders disagree on what counts as a "mention";
//! AT Protocol records change schema between releases. Pinning the
//! cross-protocol contract to a closed [`FederationProtocol`] + a
//! typed [`FederationEvent`] + per-protocol [`FederationAddress`]
//! discriminants surfaces breakage at the type-checker rather than
//! at runtime against arbitrary peers.
//!
//! ### Out of scope here
//!
//! No network. No HTTP signature signing. No relay loop. No Nostr
//! event-id derivation. Those land in per-protocol impl crates
//! that implement [`FederationPublisher`].

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Closed enum of supported federation protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FederationProtocol {
    /// ActivityPub — W3C Recommendation 2018-01-23.
    /// Inbox + outbox + ActivityStreams 2.0 JSON-LD.
    ActivityPub,
    /// Webmention — W3C Recommendation 2017-01-12.
    /// HTTP POST notifying a target URL of a source URL link.
    Webmention,
    /// WebSub — W3C Recommendation 2018-01-23.
    /// Hub-based publish/subscribe; supersedes PubSubHubbub.
    WebSub,
    /// Nostr — NIP-01.
    /// Signed event broadcast over WebSocket relays.
    Nostr,
    /// AT Protocol (Bluesky).
    AtProtocol,
}

impl FederationProtocol {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::ActivityPub => "activitypub",
            Self::Webmention => "webmention",
            Self::WebSub => "websub",
            Self::Nostr => "nostr",
            Self::AtProtocol => "at-protocol",
        }
    }

    /// W3C Recommendation / spec date this protocol entered
    /// stable. Used for the operator-facing protocol-list page
    /// (#74 site discoverability).
    pub fn spec_year(&self) -> u16 {
        match self {
            Self::ActivityPub => 2018,
            Self::Webmention => 2017,
            Self::WebSub => 2018,
            Self::Nostr => 2022,
            Self::AtProtocol => 2024,
        }
    }
}

/// Federation event lifecycle. The runtime drives every published
/// post through these states; FederationPublisher impls report
/// state transitions back via [`FederationOutcome`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FederationState {
    /// Queued for publish but not yet sent.
    Pending,
    /// Currently being sent — relay/inbox handshake in progress.
    Publishing,
    /// Successfully accepted by the remote endpoint.
    Published,
    /// Remote endpoint rejected the publish (4xx for HTTP, NIP-20
    /// failure for Nostr).
    Rejected,
    /// Sender retries exhausted without acceptance.
    Failed,
}

impl FederationState {
    /// Stable kebab-case slug.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Publishing => "publishing",
            Self::Published => "published",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
        }
    }

    /// Whether the state is terminal.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Published | Self::Rejected | Self::Failed)
    }

    /// Whether the publish reached the remote endpoint.
    pub fn is_published(&self) -> bool {
        matches!(self, Self::Published)
    }
}

/// Protocol-discriminated address. Closed enum prevents an
/// ActivityPub inbox URL from being passed to a Nostr publisher
/// at compile time.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "protocol", rename_all = "kebab-case")]
pub enum FederationAddress {
    /// ActivityPub inbox URL (HTTPS POST endpoint).
    ActivityPub {
        /// Inbox URL.
        inbox: String,
    },
    /// Webmention endpoint (HTTPS POST endpoint).
    Webmention {
        /// Target endpoint discovered via Link header / HTML
        /// `<link rel="webmention">`.
        endpoint: String,
        /// Target URL being mentioned.
        target: String,
    },
    /// WebSub hub.
    WebSub {
        /// Hub URL.
        hub: String,
        /// Topic URL the hub publishes for.
        topic: String,
    },
    /// Nostr relay (wss://).
    Nostr {
        /// Relay WebSocket URL.
        relay: String,
    },
    /// AT Protocol PDS (Personal Data Server).
    AtProtocol {
        /// PDS host URL.
        pds: String,
        /// Repo DID.
        did: String,
    },
}

impl FederationAddress {
    /// Which protocol this address routes to.
    pub fn protocol(&self) -> FederationProtocol {
        match self {
            Self::ActivityPub { .. } => FederationProtocol::ActivityPub,
            Self::Webmention { .. } => FederationProtocol::Webmention,
            Self::WebSub { .. } => FederationProtocol::WebSub,
            Self::Nostr { .. } => FederationProtocol::Nostr,
            Self::AtProtocol { .. } => FederationProtocol::AtProtocol,
        }
    }
}

/// Federation event — one publish to one address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FederationEvent {
    /// Operator-assigned stable id (deterministic per content +
    /// destination pair).
    pub id: String,
    /// Source content reference (operator-defined — typically a
    /// CmsSection id).
    pub source_id: String,
    /// Destination address.
    pub address: FederationAddress,
    /// Lifecycle state.
    pub state: FederationState,
    /// When the event was originally queued.
    pub queued_at: time::OffsetDateTime,
    /// Number of publish attempts to date.
    pub attempt_count: u32,
    /// Most recent error (when state == Rejected | Failed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

impl FederationEvent {
    /// Construct a Pending event.
    pub fn new(
        id: impl Into<String>,
        source_id: impl Into<String>,
        address: FederationAddress,
        queued_at: time::OffsetDateTime,
    ) -> Self {
        Self {
            id: id.into(),
            source_id: source_id.into(),
            address,
            state: FederationState::Pending,
            queued_at,
            attempt_count: 0,
            last_error: None,
        }
    }
}

/// Outcome reported back by a [`FederationPublisher`] after one
/// publish attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FederationOutcome {
    /// New state after the attempt.
    pub state: FederationState,
    /// Remote-assigned id, if any (ActivityPub remote-Activity id,
    /// Nostr event id, AT Protocol record cid).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_id: Option<String>,
    /// When the attempt completed.
    pub completed_at: time::OffsetDateTime,
    /// Failure detail (state == Rejected | Failed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Typed errors at the federation boundary.
#[derive(Debug, thiserror::Error)]
pub enum FederationError {
    /// Address belongs to a different protocol than the publisher.
    #[error("protocol mismatch: publisher={publisher:?}, address={address:?}")]
    ProtocolMismatch {
        /// The publisher's protocol.
        publisher: FederationProtocol,
        /// The address's protocol.
        address: FederationProtocol,
    },
    /// Remote rejected the publish (4xx HTTP, NIP-20 negative
    /// OK, etc.).
    #[error("remote rejected: {0}")]
    RemoteRejected(String),
    /// Network / transport failure.
    #[error("transport: {0}")]
    Transport(String),
}

/// Per-protocol publisher. Impl crates land per protocol
/// (federation-activitypub, federation-webmention, federation-
/// websub, federation-nostr, federation-atproto).
pub trait FederationPublisher {
    /// Which protocol this publisher handles.
    fn protocol(&self) -> FederationProtocol;
    /// Publish one event. The address's protocol MUST match this
    /// publisher's protocol — mismatch returns
    /// [`FederationError::ProtocolMismatch`].
    fn publish(&self, event: &FederationEvent) -> Result<FederationOutcome, FederationError>;
}

/// Verify that a publisher's protocol matches an event's address.
/// Useful for the dispatcher when routing events.
pub fn assert_protocol_match(
    publisher: FederationProtocol,
    address: FederationProtocol,
) -> Result<(), FederationError> {
    if publisher == address {
        Ok(())
    } else {
        Err(FederationError::ProtocolMismatch { publisher, address })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn protocol_slugs_distinct() {
        let ps = [
            FederationProtocol::ActivityPub,
            FederationProtocol::Webmention,
            FederationProtocol::WebSub,
            FederationProtocol::Nostr,
            FederationProtocol::AtProtocol,
        ];
        let mut s = std::collections::HashSet::new();
        for p in ps {
            assert!(s.insert(p.slug()));
            assert!(p.spec_year() >= 2017);
        }
    }

    #[test]
    fn state_terminal_set() {
        assert!(FederationState::Published.is_terminal());
        assert!(FederationState::Rejected.is_terminal());
        assert!(FederationState::Failed.is_terminal());
        assert!(!FederationState::Pending.is_terminal());
        assert!(!FederationState::Publishing.is_terminal());
    }

    #[test]
    fn only_published_state_is_published() {
        assert!(FederationState::Published.is_published());
        for s in [
            FederationState::Pending,
            FederationState::Publishing,
            FederationState::Rejected,
            FederationState::Failed,
        ] {
            assert!(!s.is_published());
        }
    }

    #[test]
    fn address_protocol_matches_each_variant() {
        let ap = FederationAddress::ActivityPub {
            inbox: "https://m.example/inbox".into(),
        };
        let wm = FederationAddress::Webmention {
            endpoint: "https://t.example/webmention".into(),
            target: "https://t.example/post".into(),
        };
        let ws = FederationAddress::WebSub {
            hub: "https://hub.example".into(),
            topic: "https://t.example/feed".into(),
        };
        let no = FederationAddress::Nostr {
            relay: "wss://relay.example".into(),
        };
        let at = FederationAddress::AtProtocol {
            pds: "https://bsky.social".into(),
            did: "did:plc:abc".into(),
        };
        assert_eq!(ap.protocol(), FederationProtocol::ActivityPub);
        assert_eq!(wm.protocol(), FederationProtocol::Webmention);
        assert_eq!(ws.protocol(), FederationProtocol::WebSub);
        assert_eq!(no.protocol(), FederationProtocol::Nostr);
        assert_eq!(at.protocol(), FederationProtocol::AtProtocol);
    }

    #[test]
    fn event_starts_in_pending_with_zero_attempts() {
        let e = FederationEvent::new(
            "e1",
            "section-1",
            FederationAddress::Nostr {
                relay: "wss://r".into(),
            },
            datetime!(2026-05-18 00:00:00 UTC),
        );
        assert_eq!(e.state, FederationState::Pending);
        assert_eq!(e.attempt_count, 0);
        assert!(e.last_error.is_none());
    }

    #[test]
    fn assert_protocol_match_ok_on_match() {
        assert!(assert_protocol_match(
            FederationProtocol::ActivityPub,
            FederationProtocol::ActivityPub
        )
        .is_ok());
    }

    #[test]
    fn assert_protocol_match_errors_on_mismatch() {
        let r = assert_protocol_match(FederationProtocol::Nostr, FederationProtocol::ActivityPub);
        let is_mismatch = matches!(r, Err(FederationError::ProtocolMismatch { .. }));
        assert!(is_mismatch);
    }

    #[test]
    fn event_serde_round_trip() {
        let e = FederationEvent::new(
            "e1",
            "section-1",
            FederationAddress::AtProtocol {
                pds: "https://bsky.social".into(),
                did: "did:plc:abc".into(),
            },
            datetime!(2026-05-18 00:00:00 UTC),
        );
        let j = serde_json::to_string(&e).unwrap();
        let back: FederationEvent = serde_json::from_str(&j).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn event_rejects_unknown_field() {
        let bad = r#"{"id":"e1","source-id":"s1","address":{"protocol":"nostr","relay":"wss://r"},"state":"pending","queued-at":"2026-05-18T00:00:00Z","attempt-count":0,"ahem":1}"#;
        let r: Result<FederationEvent, _> = serde_json::from_str(bad);
        assert!(r.is_err());
    }

    #[test]
    fn outcome_round_trip() {
        let o = FederationOutcome {
            state: FederationState::Published,
            remote_id: Some("nostr-event-abc".into()),
            completed_at: datetime!(2026-05-18 00:01:00 UTC),
            error: None,
        };
        let j = serde_json::to_string(&o).unwrap();
        let back: FederationOutcome = serde_json::from_str(&j).unwrap();
        assert_eq!(o, back);
    }
}
