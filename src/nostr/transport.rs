//! Testable FrostDAO Nostr transport primitives.
//!
//! The production relay client lives in `client.rs`. This module defines a small
//! room transport contract plus an in-memory implementation for deterministic
//! multi-device tests.

use anyhow::{bail, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::client::{create_room_client_with_relays, NostrClient, NostrReceiver};
use super::events::{parse_protocol_message, MessageReplayCache, NostrProtocolMessage};

pub trait RoomMessageTransport {
    fn publish(&mut self, message: NostrProtocolMessage) -> Result<()>;

    fn receive_for_party(
        &mut self,
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
        &mut self,
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

pub struct RelayRoomTransport {
    runtime: tokio::runtime::Runtime,
    client: Arc<NostrClient>,
    receiver: NostrReceiver,
}

impl RelayRoomTransport {
    pub fn connect(
        room: &str,
        party_index: u32,
        relay_urls: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self> {
        let relays = relay_urls
            .into_iter()
            .map(|relay| relay.as_ref().to_string())
            .collect::<Vec<_>>();
        if relays.is_empty() {
            bail!("at least one relay URL is required");
        }

        let runtime = tokio::runtime::Runtime::new()?;
        let (client, receiver) =
            runtime.block_on(create_room_client_with_relays(room, party_index, &relays))?;

        Ok(Self {
            runtime,
            client,
            receiver,
        })
    }

    pub fn client(&self) -> &NostrClient {
        &self.client
    }

    pub fn disconnect(&self) {
        self.runtime.block_on(self.client.disconnect());
    }
}

impl RoomMessageTransport for RelayRoomTransport {
    fn publish(&mut self, message: NostrProtocolMessage) -> Result<()> {
        self.runtime
            .block_on(self.client.publish_protocol_message(&message))?;
        Ok(())
    }

    fn receive_for_party(
        &mut self,
        room: &str,
        party_index: u32,
        now: u64,
        replay_cache: &mut MessageReplayCache,
    ) -> Vec<NostrProtocolMessage> {
        let mut messages = Vec::new();
        while let Some(content) = self.receiver.try_recv() {
            if let Some(message) =
                accept_relay_protocol_content(&content, room, party_index, now, replay_cache)
            {
                messages.push(message);
            }
        }
        messages
    }
}

fn accept_relay_protocol_content(
    content: &str,
    room: &str,
    party_index: u32,
    now: u64,
    replay_cache: &mut MessageReplayCache,
) -> Option<NostrProtocolMessage> {
    let message = parse_protocol_message(content).ok()?;
    replay_cache.accept(&message, room, party_index, now).ok()?;
    Some(message)
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

#[derive(Debug, Clone)]
pub struct NostrRoomRuntime<T> {
    room: String,
    my_party_index: u32,
    transport: T,
    replay_cache: FileReplayCache,
}

impl<T: RoomMessageTransport> NostrRoomRuntime<T> {
    pub fn load(
        room: impl Into<String>,
        my_party_index: u32,
        transport: T,
        replay_cache_path: impl Into<PathBuf>,
    ) -> Result<Self> {
        let room = room.into();
        if room.trim().is_empty() {
            bail!("room cannot be empty");
        }
        if my_party_index == 0 {
            bail!("party index must be nonzero");
        }

        Ok(Self {
            room,
            my_party_index,
            transport,
            replay_cache: FileReplayCache::load(replay_cache_path)?,
        })
    }

    pub fn publish(&mut self, message: NostrProtocolMessage) -> Result<()> {
        if message.room != self.room {
            bail!(
                "message belongs to room '{}', expected '{}'",
                message.room,
                self.room
            );
        }
        if message.from != self.my_party_index {
            bail!(
                "message sender {} does not match local party {}",
                message.from,
                self.my_party_index
            );
        }

        self.transport.publish(message)
    }

    pub fn receive(&mut self, now: u64) -> Result<Vec<NostrProtocolMessage>> {
        let messages = self.transport.receive_for_party(
            &self.room,
            self.my_party_index,
            now,
            self.replay_cache.cache_mut(),
        );
        if !messages.is_empty() {
            self.replay_cache.save()?;
        }
        Ok(messages)
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub fn replay_cache(&self) -> &FileReplayCache {
        &self.replay_cache
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

    #[test]
    fn room_runtime_enforces_room_sender_and_persists_replay_cache() {
        let dir = std::env::temp_dir().join(format!(
            "frostdao-runtime-cache-test-{}",
            std::process::id()
        ));
        let path = dir.join("room-a-party-1.json");
        let _ = std::fs::remove_file(&path);

        let join = RoomJoinPayload {
            party_index: 1,
            nostr_pubkey: "npub-test".to_string(),
            threshold: 2,
            n_parties: 3,
            scheme: ThresholdScheme::Tss,
            rank: None,
        };
        let own_message =
            NostrProtocolMessage::new_at("room-a", NostrMessageKind::RoomJoin, 1, &join, 100)
                .unwrap()
                .with_tss();
        let wrong_room =
            NostrProtocolMessage::new_at("room-b", NostrMessageKind::RoomJoin, 1, &join, 100)
                .unwrap()
                .with_tss();
        let wrong_sender =
            NostrProtocolMessage::new_at("room-a", NostrMessageKind::RoomJoin, 2, &join, 100)
                .unwrap()
                .with_tss();

        let mut runtime =
            NostrRoomRuntime::load("room-a", 1, InMemoryRoomTransport::new(), &path).unwrap();
        assert!(runtime.publish(wrong_room).is_err());
        assert!(runtime.publish(wrong_sender).is_err());
        runtime.publish(own_message).unwrap();

        let transport = runtime.transport().clone();
        let mut first_restart = NostrRoomRuntime::load("room-a", 1, transport, &path).unwrap();
        assert_eq!(first_restart.receive(110).unwrap().len(), 1);
        assert_eq!(first_restart.replay_cache().cache().len(), 1);

        let transport = first_restart.transport().clone();
        let mut second_restart = NostrRoomRuntime::load("room-a", 1, transport, &path).unwrap();
        assert_eq!(second_restart.receive(110).unwrap().len(), 0);
        assert_eq!(second_restart.replay_cache().cache().len(), 1);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn relay_content_filter_accepts_only_valid_room_messages_once() {
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
        let content = serde_json::to_string(&message).unwrap();
        let mut cache = MessageReplayCache::new();

        assert!(accept_relay_protocol_content(&content, "room-a", 1, 100, &mut cache).is_some());
        assert!(accept_relay_protocol_content(&content, "room-a", 1, 100, &mut cache).is_none());
        assert!(accept_relay_protocol_content(&content, "room-b", 1, 100, &mut cache).is_none());
        assert!(accept_relay_protocol_content("not json", "room-a", 1, 100, &mut cache).is_none());
    }
}
