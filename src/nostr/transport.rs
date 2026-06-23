//! Testable FrostDAO Nostr transport primitives.
//!
//! The production relay client lives in `client.rs`. This module defines a small
//! room transport contract plus an in-memory implementation for deterministic
//! multi-device tests.

use anyhow::Result;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone)]
pub struct FileReplayCache {
    path: PathBuf,
    cache: MessageReplayCache,
}

impl FileReplayCache {
    pub fn load(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let cache = if path.exists() {
            let data = std::fs::read_to_string(&path)?;
            let message_ids: Vec<String> = serde_json::from_str(&data)?;
            MessageReplayCache::from_seen_message_ids(message_ids)
        } else {
            MessageReplayCache::new()
        };

        Ok(Self { path, cache })
    }

    pub fn cache(&self) -> &MessageReplayCache {
        &self.cache
    }

    pub fn cache_mut(&mut self) -> &mut MessageReplayCache {
        &mut self.cache
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let tmp_path = tmp_path_for(&self.path);
        let data = serde_json::to_vec_pretty(&self.cache.seen_message_ids())?;
        std::fs::write(&tmp_path, data)?;
        std::fs::rename(tmp_path, &self.path)?;
        Ok(())
    }

    pub fn accept_and_save(
        &mut self,
        message: &NostrProtocolMessage,
        expected_room: &str,
        my_party_index: u32,
        now: u64,
    ) -> Result<()> {
        self.cache
            .accept(message, expected_room, my_party_index, now)?;
        self.save()
    }
}

fn tmp_path_for(path: &Path) -> PathBuf {
    let mut tmp_path = path.to_path_buf();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!("{value}.tmp"))
        .unwrap_or_else(|| "tmp".to_string());
    tmp_path.set_extension(extension);
    tmp_path
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

    #[test]
    fn file_replay_cache_survives_restart() {
        let dir =
            std::env::temp_dir().join(format!("frostdao-replay-cache-test-{}", std::process::id()));
        let path = dir.join("room-a-party-2.json");
        let _ = std::fs::remove_file(&path);

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

        let mut first_cache = FileReplayCache::load(&path).unwrap();
        first_cache
            .accept_and_save(&message, "room-a", 2, 110)
            .unwrap();
        assert_eq!(first_cache.cache().len(), 1);

        let mut reloaded_cache = FileReplayCache::load(&path).unwrap();
        assert_eq!(reloaded_cache.cache().len(), 1);
        assert!(reloaded_cache
            .accept_and_save(&message, "room-a", 2, 110)
            .is_err());

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }
}
