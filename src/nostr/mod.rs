//! Nostr module for relay-based DKG and signing coordination
//!
//! Provides:
//! - Event types matching the frontend protocol
//! - Client wrapper for relay connection
//! - Room-based subscription and publishing

pub mod client;
pub mod events;
pub mod transport;

pub use client::{
    create_room_client, create_room_client_with_relays, NostrClient, NostrReceiver, DEFAULT_RELAY,
};
pub use events::{
    parse_dkg_event, parse_protocol_message, parse_recovery_event, parse_reshare_event,
    parse_signing_event, DkgRound1Event, DkgRound2EncryptedEvent, MessageReplayCache,
    NostrDkgEvent, NostrMessageKind, NostrProtocolMessage, NostrRecoveryEvent, NostrReshareEvent,
    NostrSigningEvent, RecoveryFinalizeEvent, RecoveryRound1Event, RecoverySubshareEncryptedEvent,
    ReshareFinalizeEvent, ReshareRound1Event, ReshareSubshareEncryptedEvent, RoomJoinPayload,
    RoomReadyPayload, SigningNonceEvent, SigningShareEvent, ThresholdPolicyPayload,
    ThresholdScheme, TxBroadcastEvent, TxConsentEvent, TxProposalEvent, TxReviewPayload,
    DEFAULT_MESSAGE_TTL_SECS, FROSTDAO_APP, FROSTDAO_NOSTR_PROTOCOL_VERSION, MAX_CLOCK_SKEW_SECS,
};
pub use transport::{
    FileReplayCache, InMemoryRoomTransport, NostrRoomRuntime, RelayRoomTransport,
    RoomMessageTransport,
};
