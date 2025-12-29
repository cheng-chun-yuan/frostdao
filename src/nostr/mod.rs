//! Nostr module for relay-based DKG and signing coordination
//!
//! Provides:
//! - Event types matching the frontend protocol
//! - Client wrapper for relay connection
//! - Room-based subscription and publishing

pub mod client;
pub mod events;

pub use client::{create_room_client, NostrClient, NostrReceiver, DEFAULT_RELAY};
pub use events::{
    parse_dkg_event, parse_signing_event, DkgRound1Event, DkgRound2EncryptedEvent, NostrDkgEvent,
    NostrSigningEvent, SigningNonceEvent, SigningShareEvent,
};
