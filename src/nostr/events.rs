//! Nostr event types for DKG and signing protocols
//!
//! These types define the FrostDAO relay message protocol.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

pub const FROSTDAO_APP: &str = "frostdao";
pub const FROSTDAO_NOSTR_PROTOCOL_VERSION: u16 = 1;
pub const DEFAULT_MESSAGE_TTL_SECS: u64 = 60 * 60;
pub const MAX_CLOCK_SKEW_SECS: u64 = 5 * 60;

/// Versioned FrostDAO message envelope carried in a Nostr event.
///
/// Sensitive payloads must be encrypted before being placed in `payload`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NostrProtocolMessage {
    pub app: String,
    pub version: u16,
    pub message_id: String,
    pub created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    pub room: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme: Option<ThresholdScheme>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    pub kind: NostrMessageKind,
    pub from: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<u32>,
    pub payload: serde_json::Value,
}

impl NostrProtocolMessage {
    pub fn new<T: Serialize>(
        room: impl Into<String>,
        kind: NostrMessageKind,
        from: u32,
        payload: &T,
    ) -> Result<Self> {
        Self::new_at(room, kind, from, payload, current_unix_time())
    }

    pub fn new_at<T: Serialize>(
        room: impl Into<String>,
        kind: NostrMessageKind,
        from: u32,
        payload: &T,
        created_at: u64,
    ) -> Result<Self> {
        let payload = serde_json::to_value(payload)?;
        let room = room.into();
        let message_id = compute_message_id(&room, kind, from, created_at, &payload);
        let message = Self {
            app: FROSTDAO_APP.to_string(),
            version: FROSTDAO_NOSTR_PROTOCOL_VERSION,
            message_id,
            created_at,
            expires_at: Some(created_at + DEFAULT_MESSAGE_TTL_SECS),
            room,
            scheme: None,
            wallet: None,
            session: None,
            kind,
            from,
            to: None,
            payload,
        };
        message.validate()?;
        Ok(message)
    }

    pub fn with_wallet(mut self, wallet: impl Into<String>) -> Self {
        self.wallet = Some(wallet.into());
        self
    }

    pub fn with_scheme(mut self, scheme: ThresholdScheme) -> Self {
        self.scheme = Some(scheme);
        self
    }

    pub fn with_tss(self) -> Self {
        self.with_scheme(ThresholdScheme::Tss)
    }

    pub fn with_htss(self) -> Self {
        self.with_scheme(ThresholdScheme::Htss)
    }

    pub fn with_session(mut self, session: impl Into<String>) -> Self {
        self.session = Some(session.into());
        self
    }

    pub fn with_expiry(mut self, expires_at: u64) -> Result<Self> {
        self.expires_at = Some(expires_at);
        self.validate()?;
        Ok(self)
    }

    pub fn to_party(mut self, party_index: u32) -> Result<Self> {
        self.to = Some(party_index);
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<()> {
        if self.app != FROSTDAO_APP {
            bail!("unsupported app '{}'", self.app);
        }
        if self.version != FROSTDAO_NOSTR_PROTOCOL_VERSION {
            bail!("unsupported protocol version {}", self.version);
        }
        if self.message_id.trim().is_empty() {
            bail!("message_id cannot be empty");
        }
        if self.created_at == 0 {
            bail!("created_at must be nonzero");
        }
        if let Some(expires_at) = self.expires_at {
            if expires_at <= self.created_at {
                bail!("expires_at must be after created_at");
            }
        }
        if self.room.trim().is_empty() {
            bail!("room cannot be empty");
        }
        if self.from == 0 {
            bail!("sender party index must be nonzero");
        }
        if self.to == Some(0) {
            bail!("recipient party index must be nonzero");
        }
        Ok(())
    }

    pub fn payload_as<T: for<'de> Deserialize<'de>>(&self) -> Result<T> {
        Ok(serde_json::from_value(self.payload.clone())?)
    }
}

/// Replay and recipient filter for received protocol messages.
#[derive(Debug, Default, Clone)]
pub struct MessageReplayCache {
    seen_message_ids: HashSet<String>,
}

impl MessageReplayCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_seen_message_ids<I, S>(message_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            seen_message_ids: message_ids.into_iter().map(Into::into).collect(),
        }
    }

    pub fn seen_message_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.seen_message_ids.iter().cloned().collect();
        ids.sort();
        ids
    }

    pub fn len(&self) -> usize {
        self.seen_message_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seen_message_ids.is_empty()
    }

    pub fn accept(
        &mut self,
        message: &NostrProtocolMessage,
        expected_room: &str,
        my_party_index: u32,
        now: u64,
    ) -> Result<()> {
        message.validate()?;

        if message.room != expected_room {
            bail!(
                "message belongs to room '{}', expected '{}'",
                message.room,
                expected_room
            );
        }

        if let Some(to) = message.to {
            if to != my_party_index {
                bail!(
                    "message recipient {} does not match local party {}",
                    to,
                    my_party_index
                );
            }
        }

        if message.created_at > now + MAX_CLOCK_SKEW_SECS {
            bail!("message timestamp is too far in the future");
        }

        if let Some(expires_at) = message.expires_at {
            if expires_at < now {
                bail!("message expired");
            }
        }

        if !self.seen_message_ids.insert(message.message_id.clone()) {
            bail!("duplicate message '{}'", message.message_id);
        }

        Ok(())
    }
}

fn current_unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(1)
}

fn compute_message_id(
    room: &str,
    kind: NostrMessageKind,
    from: u32,
    created_at: u64,
    payload: &serde_json::Value,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(FROSTDAO_APP.as_bytes());
    hasher.update(FROSTDAO_NOSTR_PROTOCOL_VERSION.to_be_bytes());
    hasher.update(room.as_bytes());
    hasher.update(format!("{:?}", kind).as_bytes());
    hasher.update(from.to_be_bytes());
    hasher.update(created_at.to_be_bytes());
    hasher.update(payload.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

/// Threshold scheme used by a FrostDAO room/session.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdScheme {
    /// Standard t-of-n threshold signatures. All parties have rank 0.
    Tss,
    /// Hierarchical threshold signatures with rank-aware Birkhoff interpolation.
    Htss,
}

impl ThresholdScheme {
    pub fn from_hierarchical(hierarchical: bool) -> Self {
        if hierarchical {
            Self::Htss
        } else {
            Self::Tss
        }
    }

    pub fn is_htss(self) -> bool {
        matches!(self, Self::Htss)
    }
}

/// FrostDAO protocol message kind.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NostrMessageKind {
    RoomJoin,
    RoomReady,
    KeygenRound1,
    KeygenRound2Encrypted,
    TxProposal,
    TxConsent,
    SigningNonceEncrypted,
    SigningShareEncrypted,
    TxBroadcast,
    ReshareRound1,
    ReshareSubshareEncrypted,
    ReshareFinalize,
    RecoveryRound1,
    RecoverySubshareEncrypted,
    RecoveryFinalize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoomJoinPayload {
    pub party_index: u32,
    pub nostr_pubkey: String,
    pub threshold: u32,
    pub n_parties: u32,
    pub scheme: ThresholdScheme,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<u32>,
}

/// Public threshold policy for a room or wallet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThresholdPolicyPayload {
    pub threshold: u32,
    pub n_parties: u32,
    pub scheme: ThresholdScheme,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub party_ranks: BTreeMap<u32, u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_requirement: Option<Vec<u32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoomReadyPayload {
    pub participants: Vec<u32>,
}

/// DKG Round 1 event (broadcast)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

/// Transaction proposal broadcast by a proposer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TxProposalEvent {
    pub proposer_index: u32,
    pub to_address: String,
    pub amount_sats: u64,
    pub fee_rate: u64,
    pub sighash: String,
    #[serde(default)]
    pub review: TxReviewPayload,
    pub description: String,
    pub timestamp: u64,
}

/// Human-checkable transaction review data carried over relay proposals.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TxReviewPayload {
    pub network: String,
    pub source_path: String,
    pub from_address: String,
    pub to_address: String,
    pub amount_sats: u64,
    pub fee_rate_sats_vb: u64,
    pub sighash_fingerprint: String,
}

impl Default for TxReviewPayload {
    fn default() -> Self {
        Self {
            network: "unknown".to_string(),
            source_path: "unknown".to_string(),
            from_address: "unknown".to_string(),
            to_address: String::new(),
            amount_sats: 0,
            fee_rate_sats_vb: 0,
            sighash_fingerprint: String::new(),
        }
    }
}

/// Consent message sent by a signer after reviewing a proposal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TxConsentEvent {
    pub proposal_session: String,
    pub consent: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reviewed_sighash_fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Signing nonce event (encrypted per-recipient)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TxBroadcastEvent {
    pub txid: String,
    pub raw_tx: String,
    pub network: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReshareRound1Event {
    pub old_party_index: u32,
    pub new_threshold: u32,
    pub new_n_parties: u32,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReshareSubshareEncryptedEvent {
    pub old_party_index: u32,
    pub new_party_index: u32,
    pub ciphertext: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReshareFinalizeEvent {
    pub new_party_index: u32,
    pub wallet_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryRound1Event {
    pub helper_index: u32,
    pub lost_index: u32,
    pub helper_rank: u32,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoverySubshareEncryptedEvent {
    pub helper_index: u32,
    pub lost_index: u32,
    pub ciphertext: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryFinalizeEvent {
    pub recovered_party_index: u32,
    pub wallet_name: String,
}

/// Parsed Nostr event from relay
#[derive(Debug, Clone)]
pub enum NostrDkgEvent {
    Round1(DkgRound1Event),
    Round2Encrypted(DkgRound2EncryptedEvent),
}

#[derive(Debug, Clone)]
pub enum NostrSigningEvent {
    Proposal(TxProposalEvent),
    Consent(TxConsentEvent),
    Nonce(SigningNonceEvent),
    Share(SigningShareEvent),
    Broadcast(TxBroadcastEvent),
}

#[derive(Debug, Clone)]
pub enum NostrReshareEvent {
    Round1(ReshareRound1Event),
    SubshareEncrypted(ReshareSubshareEncryptedEvent),
    Finalize(ReshareFinalizeEvent),
}

#[derive(Debug, Clone)]
pub enum NostrRecoveryEvent {
    Round1(RecoveryRound1Event),
    SubshareEncrypted(RecoverySubshareEncryptedEvent),
    Finalize(RecoveryFinalizeEvent),
}

/// Parse a versioned FrostDAO Nostr message envelope.
pub fn parse_protocol_message(content: &str) -> Result<NostrProtocolMessage> {
    let message: NostrProtocolMessage = serde_json::from_str(content)?;
    message.validate()?;
    Ok(message)
}

/// Parse event content JSON into typed event
pub fn parse_dkg_event(content: &str) -> Option<NostrDkgEvent> {
    if let Ok(message) = parse_protocol_message(content) {
        return match message.kind {
            NostrMessageKind::KeygenRound1 => {
                let event: DkgRound1Event = message.payload_as().ok()?;
                (event.party_index == message.from).then_some(NostrDkgEvent::Round1(event))
            }
            NostrMessageKind::KeygenRound2Encrypted => {
                let event: DkgRound2EncryptedEvent = message.payload_as().ok()?;
                encrypted_dkg_round2_matches_envelope(&message, &event)
                    .then_some(NostrDkgEvent::Round2Encrypted(event))
            }
            _ => None,
        };
    }

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

fn encrypted_dkg_round2_matches_envelope(
    message: &NostrProtocolMessage,
    event: &DkgRound2EncryptedEvent,
) -> bool {
    event.party_index == message.from && message.to == Some(event.to_index)
}

pub fn parse_signing_event(content: &str) -> Option<NostrSigningEvent> {
    if let Ok(message) = parse_protocol_message(content) {
        return match message.kind {
            NostrMessageKind::TxProposal => {
                message.payload_as().ok().map(NostrSigningEvent::Proposal)
            }
            NostrMessageKind::TxConsent => {
                message.payload_as().ok().map(NostrSigningEvent::Consent)
            }
            NostrMessageKind::SigningNonceEncrypted => {
                let event: SigningNonceEvent = message.payload_as().ok()?;
                encrypted_signing_nonce_matches_envelope(&message, &event)
                    .then_some(NostrSigningEvent::Nonce(event))
            }
            NostrMessageKind::SigningShareEncrypted => {
                let event: SigningShareEvent = message.payload_as().ok()?;
                encrypted_signing_share_matches_envelope(&message, &event)
                    .then_some(NostrSigningEvent::Share(event))
            }
            NostrMessageKind::TxBroadcast => {
                message.payload_as().ok().map(NostrSigningEvent::Broadcast)
            }
            _ => None,
        };
    }

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

fn encrypted_signing_nonce_matches_envelope(
    message: &NostrProtocolMessage,
    event: &SigningNonceEvent,
) -> bool {
    event.party_index == message.from && message.to == Some(event.to_index)
}

fn encrypted_signing_share_matches_envelope(
    message: &NostrProtocolMessage,
    event: &SigningShareEvent,
) -> bool {
    event.party_index == message.from && message.to == Some(event.to_index)
}

pub fn parse_reshare_event(content: &str) -> Option<NostrReshareEvent> {
    let message = parse_protocol_message(content).ok()?;

    match message.kind {
        NostrMessageKind::ReshareRound1 => {
            let event: ReshareRound1Event = message.payload_as().ok()?;
            (event.old_party_index == message.from).then_some(NostrReshareEvent::Round1(event))
        }
        NostrMessageKind::ReshareSubshareEncrypted => {
            let event: ReshareSubshareEncryptedEvent = message.payload_as().ok()?;
            encrypted_reshare_subshare_matches_envelope(&message, &event)
                .then_some(NostrReshareEvent::SubshareEncrypted(event))
        }
        NostrMessageKind::ReshareFinalize => {
            let event: ReshareFinalizeEvent = message.payload_as().ok()?;
            (event.new_party_index == message.from).then_some(NostrReshareEvent::Finalize(event))
        }
        _ => None,
    }
}

pub fn parse_recovery_event(content: &str) -> Option<NostrRecoveryEvent> {
    let message = parse_protocol_message(content).ok()?;

    match message.kind {
        NostrMessageKind::RecoveryRound1 => {
            let event: RecoveryRound1Event = message.payload_as().ok()?;
            (event.helper_index == message.from).then_some(NostrRecoveryEvent::Round1(event))
        }
        NostrMessageKind::RecoverySubshareEncrypted => {
            let event: RecoverySubshareEncryptedEvent = message.payload_as().ok()?;
            encrypted_recovery_subshare_matches_envelope(&message, &event)
                .then_some(NostrRecoveryEvent::SubshareEncrypted(event))
        }
        NostrMessageKind::RecoveryFinalize => {
            let event: RecoveryFinalizeEvent = message.payload_as().ok()?;
            (event.recovered_party_index == message.from)
                .then_some(NostrRecoveryEvent::Finalize(event))
        }
        _ => None,
    }
}

fn encrypted_reshare_subshare_matches_envelope(
    message: &NostrProtocolMessage,
    event: &ReshareSubshareEncryptedEvent,
) -> bool {
    event.old_party_index == message.from && message.to == Some(event.new_party_index)
}

fn encrypted_recovery_subshare_matches_envelope(
    message: &NostrProtocolMessage,
    event: &RecoverySubshareEncryptedEvent,
) -> bool {
    event.helper_index == message.from && message.to == Some(event.lost_index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_message_round_trips() {
        let payload = TxProposalEvent {
            proposer_index: 1,
            to_address: "tb1ptest".to_string(),
            amount_sats: 10_000,
            fee_rate: 5,
            sighash: "00".repeat(32),
            review: TxReviewPayload {
                network: "testnet".to_string(),
                source_path: "m/86'/0'/0'/0/0".to_string(),
                from_address: "tb1pfrom".to_string(),
                to_address: "tb1ptest".to_string(),
                amount_sats: 10_000,
                fee_rate_sats_vb: 5,
                sighash_fingerprint: "000000000000...000000000000".to_string(),
            },
            description: "test spend".to_string(),
            timestamp: 1_700_000_000,
        };

        let message =
            NostrProtocolMessage::new("room-a", NostrMessageKind::TxProposal, 1, &payload)
                .unwrap()
                .with_wallet("treasury")
                .with_session("session-a");

        let encoded = serde_json::to_string(&message).unwrap();
        let parsed = parse_protocol_message(&encoded).unwrap();
        assert_eq!(parsed.kind, NostrMessageKind::TxProposal);
        assert_eq!(parsed.payload_as::<TxProposalEvent>().unwrap(), payload);
    }

    #[test]
    fn tx_consent_carries_reviewed_fingerprint() {
        let consent = TxConsentEvent {
            proposal_session: "session-a".to_string(),
            consent: true,
            reviewed_sighash_fingerprint: "001122334455...aabbccddeeff".to_string(),
            reason: None,
        };

        let encoded = serde_json::to_string(&consent).unwrap();
        let parsed: TxConsentEvent = serde_json::from_str(&encoded).unwrap();
        assert_eq!(
            parsed.reviewed_sighash_fingerprint,
            consent.reviewed_sighash_fingerprint
        );
    }

    #[test]
    fn rejects_invalid_protocol_message() {
        let payload = serde_json::json!({});
        let message = NostrProtocolMessage {
            app: "other".to_string(),
            version: FROSTDAO_NOSTR_PROTOCOL_VERSION,
            message_id: "message-a".to_string(),
            created_at: 1_700_000_000,
            expires_at: Some(1_700_003_600),
            room: "room-a".to_string(),
            scheme: None,
            wallet: None,
            session: None,
            kind: NostrMessageKind::RoomJoin,
            from: 1,
            to: None,
            payload,
        };

        let encoded = serde_json::to_string(&message).unwrap();
        assert!(parse_protocol_message(&encoded).is_err());
    }

    #[test]
    fn parses_legacy_signing_event() {
        let event = SigningShareEvent::new(1, 2, "ciphertext".to_string());
        let encoded = serde_json::to_string(&event).unwrap();
        assert!(matches!(
            parse_signing_event(&encoded),
            Some(NostrSigningEvent::Share(_))
        ));
    }

    #[test]
    fn parses_signing_events_with_bound_envelope_identity() {
        let nonce = SigningNonceEvent::new(1, 2, "nonce-ciphertext".to_string());
        let nonce_message =
            NostrProtocolMessage::new("room-a", NostrMessageKind::SigningNonceEncrypted, 1, &nonce)
                .unwrap()
                .to_party(2)
                .unwrap();
        let encoded_nonce = serde_json::to_string(&nonce_message).unwrap();
        assert!(matches!(
            parse_signing_event(&encoded_nonce),
            Some(NostrSigningEvent::Nonce(_))
        ));

        let share = SigningShareEvent::new(1, 2, "share-ciphertext".to_string());
        let share_message =
            NostrProtocolMessage::new("room-a", NostrMessageKind::SigningShareEncrypted, 1, &share)
                .unwrap()
                .to_party(2)
                .unwrap();
        let encoded_share = serde_json::to_string(&share_message).unwrap();
        assert!(matches!(
            parse_signing_event(&encoded_share),
            Some(NostrSigningEvent::Share(_))
        ));
    }

    #[test]
    fn rejects_signing_events_with_mismatched_envelope_identity() {
        let mismatched_nonce = SigningNonceEvent::new(3, 2, "nonce-ciphertext".to_string());
        let mismatched_nonce_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::SigningNonceEncrypted,
            1,
            &mismatched_nonce,
        )
        .unwrap()
        .to_party(2)
        .unwrap();
        let encoded_mismatched_nonce = serde_json::to_string(&mismatched_nonce_message).unwrap();
        assert!(parse_signing_event(&encoded_mismatched_nonce).is_none());

        let wrong_recipient_nonce = SigningNonceEvent::new(1, 3, "nonce-ciphertext".to_string());
        let wrong_recipient_nonce_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::SigningNonceEncrypted,
            1,
            &wrong_recipient_nonce,
        )
        .unwrap()
        .to_party(2)
        .unwrap();
        let encoded_wrong_recipient_nonce =
            serde_json::to_string(&wrong_recipient_nonce_message).unwrap();
        assert!(parse_signing_event(&encoded_wrong_recipient_nonce).is_none());

        let mismatched_share = SigningShareEvent::new(3, 2, "share-ciphertext".to_string());
        let mismatched_share_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::SigningShareEncrypted,
            1,
            &mismatched_share,
        )
        .unwrap()
        .to_party(2)
        .unwrap();
        let encoded_mismatched_share = serde_json::to_string(&mismatched_share_message).unwrap();
        assert!(parse_signing_event(&encoded_mismatched_share).is_none());

        let wrong_recipient_share = SigningShareEvent::new(1, 3, "share-ciphertext".to_string());
        let wrong_recipient_share_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::SigningShareEncrypted,
            1,
            &wrong_recipient_share,
        )
        .unwrap()
        .to_party(2)
        .unwrap();
        let encoded_wrong_recipient_share =
            serde_json::to_string(&wrong_recipient_share_message).unwrap();
        assert!(parse_signing_event(&encoded_wrong_recipient_share).is_none());
    }

    #[test]
    fn parses_dkg_events_with_bound_envelope_identity() {
        let round1 = DkgRound1Event::new(1, "keygen-input".to_string(), "pubkey".to_string());
        let round1_message =
            NostrProtocolMessage::new("room-a", NostrMessageKind::KeygenRound1, 1, &round1)
                .unwrap();
        let encoded_round1 = serde_json::to_string(&round1_message).unwrap();
        assert!(matches!(
            parse_dkg_event(&encoded_round1),
            Some(NostrDkgEvent::Round1(_))
        ));

        let round2 = DkgRound2EncryptedEvent::new(1, 2, "ciphertext".to_string());
        let round2_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::KeygenRound2Encrypted,
            1,
            &round2,
        )
        .unwrap()
        .to_party(2)
        .unwrap();
        let encoded_round2 = serde_json::to_string(&round2_message).unwrap();
        assert!(matches!(
            parse_dkg_event(&encoded_round2),
            Some(NostrDkgEvent::Round2Encrypted(_))
        ));
    }

    #[test]
    fn rejects_dkg_events_with_mismatched_envelope_identity() {
        let mismatched_round1 =
            DkgRound1Event::new(2, "keygen-input".to_string(), "pubkey".to_string());
        let mismatched_round1_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::KeygenRound1,
            1,
            &mismatched_round1,
        )
        .unwrap();
        let encoded_mismatched_round1 = serde_json::to_string(&mismatched_round1_message).unwrap();
        assert!(parse_dkg_event(&encoded_mismatched_round1).is_none());

        let mismatched_round2 = DkgRound2EncryptedEvent::new(3, 2, "ciphertext".to_string());
        let mismatched_round2_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::KeygenRound2Encrypted,
            1,
            &mismatched_round2,
        )
        .unwrap()
        .to_party(2)
        .unwrap();
        let encoded_mismatched_round2 = serde_json::to_string(&mismatched_round2_message).unwrap();
        assert!(parse_dkg_event(&encoded_mismatched_round2).is_none());

        let wrong_recipient_round2 = DkgRound2EncryptedEvent::new(1, 3, "ciphertext".to_string());
        let wrong_recipient_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::KeygenRound2Encrypted,
            1,
            &wrong_recipient_round2,
        )
        .unwrap()
        .to_party(2)
        .unwrap();
        let encoded_wrong_recipient = serde_json::to_string(&wrong_recipient_message).unwrap();
        assert!(parse_dkg_event(&encoded_wrong_recipient).is_none());
    }

    #[test]
    fn replay_cache_rejects_duplicate_expired_and_wrong_recipient() {
        let payload = SigningNonceEvent::new(1, 2, "ciphertext".to_string());
        let message = NostrProtocolMessage::new_at(
            "room-a",
            NostrMessageKind::SigningNonceEncrypted,
            1,
            &payload,
            1_700_000_000,
        )
        .unwrap()
        .to_party(2)
        .unwrap();

        let mut cache = MessageReplayCache::new();
        assert!(cache.accept(&message, "room-a", 2, 1_700_000_010).is_ok());
        assert!(cache.accept(&message, "room-a", 2, 1_700_000_010).is_err());

        let mut wrong_recipient_cache = MessageReplayCache::new();
        assert!(wrong_recipient_cache
            .accept(&message, "room-a", 3, 1_700_000_010)
            .is_err());

        let expired = message
            .clone()
            .with_expiry(1_700_000_100)
            .expect("valid expiry");
        let mut expired_cache = MessageReplayCache::new();
        assert!(expired_cache
            .accept(&expired, "room-a", 2, 1_700_000_101)
            .is_err());
    }

    #[test]
    fn supports_tss_protocol_mode() {
        let payload = ThresholdPolicyPayload {
            threshold: 2,
            n_parties: 3,
            scheme: ThresholdScheme::Tss,
            party_ranks: std::collections::BTreeMap::new(),
            signing_requirement: None,
        };

        let message =
            NostrProtocolMessage::new("room-tss", NostrMessageKind::RoomReady, 1, &payload)
                .unwrap()
                .with_tss();

        let encoded = serde_json::to_string(&message).unwrap();
        let parsed = parse_protocol_message(&encoded).unwrap();
        assert_eq!(parsed.scheme, Some(ThresholdScheme::Tss));
        assert!(!parsed.scheme.unwrap().is_htss());
        assert_eq!(
            parsed.payload_as::<ThresholdPolicyPayload>().unwrap(),
            payload
        );
    }

    #[test]
    fn supports_htss_protocol_mode() {
        let mut party_ranks = std::collections::BTreeMap::new();
        party_ranks.insert(1, 0);
        party_ranks.insert(2, 1);
        party_ranks.insert(3, 1);

        let payload = ThresholdPolicyPayload {
            threshold: 2,
            n_parties: 3,
            scheme: ThresholdScheme::Htss,
            party_ranks,
            signing_requirement: Some(vec![1, 2]),
        };

        let message =
            NostrProtocolMessage::new("room-htss", NostrMessageKind::RoomReady, 1, &payload)
                .unwrap()
                .with_htss();

        let encoded = serde_json::to_string(&message).unwrap();
        let parsed = parse_protocol_message(&encoded).unwrap();
        assert_eq!(parsed.scheme, Some(ThresholdScheme::Htss));
        assert!(parsed.scheme.unwrap().is_htss());
        assert_eq!(
            parsed.payload_as::<ThresholdPolicyPayload>().unwrap(),
            payload
        );
    }

    #[test]
    fn parses_reshare_event() {
        let round1 = ReshareRound1Event {
            old_party_index: 1,
            new_threshold: 2,
            new_n_parties: 3,
            output: "round1".to_string(),
        };
        let round1_message =
            NostrProtocolMessage::new("room-a", NostrMessageKind::ReshareRound1, 1, &round1)
                .unwrap();
        let encoded_round1 = serde_json::to_string(&round1_message).unwrap();
        assert!(matches!(
            parse_reshare_event(&encoded_round1),
            Some(NostrReshareEvent::Round1(_))
        ));

        let payload = ReshareSubshareEncryptedEvent {
            old_party_index: 1,
            new_party_index: 2,
            ciphertext: "ciphertext".to_string(),
        };
        let message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::ReshareSubshareEncrypted,
            1,
            &payload,
        )
        .unwrap()
        .to_party(2)
        .unwrap();

        let encoded = serde_json::to_string(&message).unwrap();
        assert!(matches!(
            parse_reshare_event(&encoded),
            Some(NostrReshareEvent::SubshareEncrypted(_))
        ));

        let finalize = ReshareFinalizeEvent {
            new_party_index: 2,
            wallet_name: "wallet-new".to_string(),
        };
        let finalize_message =
            NostrProtocolMessage::new("room-a", NostrMessageKind::ReshareFinalize, 2, &finalize)
                .unwrap();
        let encoded_finalize = serde_json::to_string(&finalize_message).unwrap();
        assert!(matches!(
            parse_reshare_event(&encoded_finalize),
            Some(NostrReshareEvent::Finalize(_))
        ));
    }

    #[test]
    fn rejects_reshare_events_with_mismatched_envelope_identity() {
        let mismatched_round1 = ReshareRound1Event {
            old_party_index: 2,
            new_threshold: 2,
            new_n_parties: 3,
            output: "round1".to_string(),
        };
        let mismatched_round1_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::ReshareRound1,
            1,
            &mismatched_round1,
        )
        .unwrap();
        let encoded_mismatched_round1 = serde_json::to_string(&mismatched_round1_message).unwrap();
        assert!(parse_reshare_event(&encoded_mismatched_round1).is_none());

        let mismatched_subshare = ReshareSubshareEncryptedEvent {
            old_party_index: 3,
            new_party_index: 2,
            ciphertext: "ciphertext".to_string(),
        };
        let mismatched_subshare_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::ReshareSubshareEncrypted,
            1,
            &mismatched_subshare,
        )
        .unwrap()
        .to_party(2)
        .unwrap();
        let encoded_mismatched_subshare =
            serde_json::to_string(&mismatched_subshare_message).unwrap();
        assert!(parse_reshare_event(&encoded_mismatched_subshare).is_none());

        let wrong_recipient_subshare = ReshareSubshareEncryptedEvent {
            old_party_index: 1,
            new_party_index: 3,
            ciphertext: "ciphertext".to_string(),
        };
        let wrong_recipient_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::ReshareSubshareEncrypted,
            1,
            &wrong_recipient_subshare,
        )
        .unwrap()
        .to_party(2)
        .unwrap();
        let encoded_wrong_recipient = serde_json::to_string(&wrong_recipient_message).unwrap();
        assert!(parse_reshare_event(&encoded_wrong_recipient).is_none());

        let mismatched_finalize = ReshareFinalizeEvent {
            new_party_index: 2,
            wallet_name: "wallet-new".to_string(),
        };
        let mismatched_finalize_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::ReshareFinalize,
            1,
            &mismatched_finalize,
        )
        .unwrap();
        let encoded_mismatched_finalize =
            serde_json::to_string(&mismatched_finalize_message).unwrap();
        assert!(parse_reshare_event(&encoded_mismatched_finalize).is_none());
    }

    #[test]
    fn parses_recovery_event() {
        let round1 = RecoveryRound1Event {
            helper_index: 1,
            lost_index: 2,
            helper_rank: 0,
            output: "round1".to_string(),
        };
        let round1_message =
            NostrProtocolMessage::new("room-a", NostrMessageKind::RecoveryRound1, 1, &round1)
                .unwrap();
        let encoded_round1 = serde_json::to_string(&round1_message).unwrap();
        assert!(matches!(
            parse_recovery_event(&encoded_round1),
            Some(NostrRecoveryEvent::Round1(_))
        ));

        let payload = RecoverySubshareEncryptedEvent {
            helper_index: 1,
            lost_index: 2,
            ciphertext: "ciphertext".to_string(),
        };
        let message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::RecoverySubshareEncrypted,
            1,
            &payload,
        )
        .unwrap()
        .to_party(2)
        .unwrap();

        let encoded = serde_json::to_string(&message).unwrap();
        assert!(matches!(
            parse_recovery_event(&encoded),
            Some(NostrRecoveryEvent::SubshareEncrypted(_))
        ));

        let finalize = RecoveryFinalizeEvent {
            recovered_party_index: 2,
            wallet_name: "wallet-recovered".to_string(),
        };
        let finalize_message =
            NostrProtocolMessage::new("room-a", NostrMessageKind::RecoveryFinalize, 2, &finalize)
                .unwrap();
        let encoded_finalize = serde_json::to_string(&finalize_message).unwrap();
        assert!(matches!(
            parse_recovery_event(&encoded_finalize),
            Some(NostrRecoveryEvent::Finalize(_))
        ));
    }

    #[test]
    fn rejects_recovery_events_with_mismatched_envelope_identity() {
        let mismatched_round1 = RecoveryRound1Event {
            helper_index: 2,
            lost_index: 3,
            helper_rank: 0,
            output: "round1".to_string(),
        };
        let mismatched_round1_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::RecoveryRound1,
            1,
            &mismatched_round1,
        )
        .unwrap();
        let encoded_mismatched_round1 = serde_json::to_string(&mismatched_round1_message).unwrap();
        assert!(parse_recovery_event(&encoded_mismatched_round1).is_none());

        let mismatched_subshare = RecoverySubshareEncryptedEvent {
            helper_index: 3,
            lost_index: 2,
            ciphertext: "ciphertext".to_string(),
        };
        let mismatched_subshare_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::RecoverySubshareEncrypted,
            1,
            &mismatched_subshare,
        )
        .unwrap()
        .to_party(2)
        .unwrap();
        let encoded_mismatched_subshare =
            serde_json::to_string(&mismatched_subshare_message).unwrap();
        assert!(parse_recovery_event(&encoded_mismatched_subshare).is_none());

        let wrong_recipient_subshare = RecoverySubshareEncryptedEvent {
            helper_index: 1,
            lost_index: 3,
            ciphertext: "ciphertext".to_string(),
        };
        let wrong_recipient_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::RecoverySubshareEncrypted,
            1,
            &wrong_recipient_subshare,
        )
        .unwrap()
        .to_party(2)
        .unwrap();
        let encoded_wrong_recipient = serde_json::to_string(&wrong_recipient_message).unwrap();
        assert!(parse_recovery_event(&encoded_wrong_recipient).is_none());

        let mismatched_finalize = RecoveryFinalizeEvent {
            recovered_party_index: 2,
            wallet_name: "wallet-recovered".to_string(),
        };
        let mismatched_finalize_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::RecoveryFinalize,
            1,
            &mismatched_finalize,
        )
        .unwrap();
        let encoded_mismatched_finalize =
            serde_json::to_string(&mismatched_finalize_message).unwrap();
        assert!(parse_recovery_event(&encoded_mismatched_finalize).is_none());
    }
}
