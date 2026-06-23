//! Nostr event types for DKG and signing protocols
//!
//! These types match the frontend protocol (frontend/js/rooms.js)

use serde::{Deserialize, Serialize};

/// DKG Round 1 event (broadcast)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound1Event {
    #[serde(rename = "type")]
    pub event_type: String,
    pub party_index: u32,
    pub keygen_input: String,
    pub encryption_pubkey: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hierarchical: Option<bool>,
}

impl DkgRound1Event {
    pub fn new(party_index: u32, keygen_input: String, encryption_pubkey: String) -> Self {
        Self {
            event_type: "keygen_round1".to_string(),
            party_index,
            keygen_input,
            encryption_pubkey: Some(encryption_pubkey),
            rank: None,
            hierarchical: None,
        }
    }

    pub fn with_rank(mut self, rank: u32, hierarchical: bool) -> Self {
        self.rank = Some(rank);
        self.hierarchical = Some(hierarchical);
        self
    }
}

/// DKG Round 2 encrypted share event (per-recipient)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound2EncryptedEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub party_index: u32,
    pub to_index: u32,
    pub ciphertext: String,
}

impl DkgRound2EncryptedEvent {
    pub fn new(party_index: u32, to_index: u32, ciphertext: String) -> Self {
        Self {
            event_type: "keygen_round2_encrypted".to_string(),
            party_index,
            to_index,
            ciphertext,
        }
    }
}

/// Signing nonce event (encrypted per-recipient)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningNonceEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub party_index: u32,
    pub to_index: u32,
    pub ciphertext: String,
}

impl SigningNonceEvent {
    pub fn new(party_index: u32, to_index: u32, ciphertext: String) -> Self {
        Self {
            event_type: "signing_nonce_encrypted".to_string(),
            party_index,
            to_index,
            ciphertext,
        }
    }
}

/// Signing share event (encrypted per-recipient)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningShareEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub party_index: u32,
    pub to_index: u32,
    pub ciphertext: String,
}

impl SigningShareEvent {
    pub fn new(party_index: u32, to_index: u32, ciphertext: String) -> Self {
        Self {
            event_type: "signing_share_encrypted".to_string(),
            party_index,
            to_index,
            ciphertext,
        }
    }
}

/// Parsed Nostr event from relay
#[derive(Debug, Clone)]
pub enum NostrDkgEvent {
    Round1(DkgRound1Event),
    Round2Encrypted(DkgRound2EncryptedEvent),
}

#[derive(Debug, Clone)]
pub enum NostrSigningEvent {
    Nonce(SigningNonceEvent),
    Share(SigningShareEvent),
}

/// Parse event content JSON into typed event
pub fn parse_dkg_event(content: &str) -> Option<NostrDkgEvent> {
    let v: serde_json::Value = serde_json::from_str(content).ok()?;
    let event_type = v.get("type")?.as_str()?;

    match event_type {
        "keygen_round1" => {
            let evt: DkgRound1Event = serde_json::from_str(content).ok()?;
            Some(NostrDkgEvent::Round1(evt))
        }
        "keygen_round2_encrypted" => {
            let evt: DkgRound2EncryptedEvent = serde_json::from_str(content).ok()?;
            Some(NostrDkgEvent::Round2Encrypted(evt))
        }
        _ => None,
    }
}

pub fn parse_signing_event(content: &str) -> Option<NostrSigningEvent> {
    let v: serde_json::Value = serde_json::from_str(content).ok()?;
    let event_type = v.get("type")?.as_str()?;

    match event_type {
        "signing_nonce_encrypted" => {
            let evt: SigningNonceEvent = serde_json::from_str(content).ok()?;
            Some(NostrSigningEvent::Nonce(evt))
        }
        "signing_share_encrypted" => {
            let evt: SigningShareEvent = serde_json::from_str(content).ok()?;
            Some(NostrSigningEvent::Share(evt))
        }
        _ => None,
    }
}
