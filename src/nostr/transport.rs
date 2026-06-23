//! Testable FrostDAO Nostr transport primitives.
//!
//! The production relay client lives in `client.rs`. This module defines a small
//! room transport contract plus an in-memory implementation for deterministic
//! multi-device tests.

use anyhow::Result;
use std::collections::BTreeMap;

use super::events::{MessageReplayCache, NostrProtocolMessage};

pub trait RoomMessageTransport {
    fn publish(&mut self, message: NostrProtocolMessage) -> Result<()>;

    fn receive_for_party(
        &self,
        room: &str,
        party_index: u32,
        now: u64,
        replay_cache: &mut MessageReplayCache,
    ) -> Vec<NostrProtocolMessage>;
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryRoomTransport {
    rooms: BTreeMap<String, Vec<NostrProtocolMessage>>,
}

impl InMemoryRoomTransport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn room_len(&self, room: &str) -> usize {
        self.rooms.get(room).map_or(0, Vec::len)
    }
}

impl RoomMessageTransport for InMemoryRoomTransport {
    fn publish(&mut self, message: NostrProtocolMessage) -> Result<()> {
        message.validate()?;
        self.rooms
            .entry(message.room.clone())
            .or_default()
            .push(message);
        Ok(())
    }

    fn receive_for_party(
        &self,
        room: &str,
        party_index: u32,
        now: u64,
        replay_cache: &mut MessageReplayCache,
    ) -> Vec<NostrProtocolMessage> {
        self.rooms
            .get(room)
            .into_iter()
            .flatten()
            .filter_map(|message| {
                replay_cache
                    .accept(message, room, party_index, now)
                    .ok()
                    .map(|_| message.clone())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nostr::events::{
        NostrMessageKind, RecoverySubshareEncryptedEvent, RoomJoinPayload, ThresholdScheme,
    };

    #[test]
    fn in_memory_transport_routes_public_and_direct_messages() {
        let mut transport = InMemoryRoomTransport::new();
        let join = RoomJoinPayload {
            party_index: 1,
            nostr_pubkey: "npub-test".to_string(),
            threshold: 2,
            n_parties: 3,
            scheme: ThresholdScheme::Tss,
            rank: None,
        };
        let public_message =
            NostrProtocolMessage::new_at("room-a", NostrMessageKind::RoomJoin, 1, &join, 100)
                .unwrap()
                .with_tss();

        let recovery = RecoverySubshareEncryptedEvent {
            helper_index: 1,
            lost_index: 2,
            ciphertext: "ciphertext".to_string(),
        };
        let direct_message = NostrProtocolMessage::new_at(
            "room-a",
            NostrMessageKind::RecoverySubshareEncrypted,
            1,
            &recovery,
            101,
        )
        .unwrap()
        .to_party(2)
        .unwrap();

        transport.publish(public_message).unwrap();
        transport.publish(direct_message).unwrap();

        let mut party_2_cache = MessageReplayCache::new();
        let party_2_messages = transport.receive_for_party("room-a", 2, 110, &mut party_2_cache);
        assert_eq!(party_2_messages.len(), 2);

        let mut party_3_cache = MessageReplayCache::new();
        let party_3_messages = transport.receive_for_party("room-a", 3, 110, &mut party_3_cache);
        assert_eq!(party_3_messages.len(), 1);
        assert_eq!(party_3_messages[0].kind, NostrMessageKind::RoomJoin);
    }

    #[test]
    fn in_memory_transport_deduplicates_per_replay_cache() {
        let mut transport = InMemoryRoomTransport::new();
        let join = RoomJoinPayload {
            party_index: 1,
            nostr_pubkey: "npub-test".to_string(),
            threshold: 2,
            n_parties: 3,
            scheme: ThresholdScheme::Tss,
            rank: None,
        };
        let message =
            NostrProtocolMessage::new_at("room-a", NostrMessageKind::RoomJoin, 1, &join, 100)
                .unwrap()
                .with_tss();

        transport.publish(message).unwrap();
        let mut cache = MessageReplayCache::new();
        assert_eq!(
            transport
                .receive_for_party("room-a", 2, 110, &mut cache)
                .len(),
            1
        );
        assert_eq!(
            transport
                .receive_for_party("room-a", 2, 110, &mut cache)
                .len(),
            0
        );
    }
}
