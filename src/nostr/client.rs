//! Nostr client for relay communication
//!
//! Handles connection to Damus relay and room-based event filtering.

use anyhow::Result;
use nostr_sdk::prelude::*;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Default relay (all data is E2E encrypted, single relay is fine)
pub const DEFAULT_RELAY: &str = "wss://relay.damus.io";

/// Nostr client wrapper for DKG/signing rooms
pub struct NostrClient {
    client: Client,
    room_id: String,
    my_index: u32,
    keys: Keys,
}

impl NostrClient {
    /// Create and connect to relay
    pub async fn connect(room_id: &str, my_index: u32) -> Result<Self> {
        // Generate ephemeral keys for this session
        let keys = Keys::generate();
        let client = Client::new(keys.clone());

        // Add relay and connect
        client.add_relay(DEFAULT_RELAY).await?;
        client.connect().await;

        Ok(Self {
            client,
            room_id: room_id.to_string(),
            my_index,
            keys,
        })
    }

    /// Get my public key (for encryption key exchange)
    pub fn my_pubkey(&self) -> String {
        self.keys.public_key().to_string()
    }

    /// Publish event to room
    pub async fn publish(&self, content: &str) -> Result<EventId> {
        let event = EventBuilder::text_note(content)
            .tags(vec![Tag::custom(
                TagKind::SingleLetter(SingleLetterTag::lowercase(Alphabet::R)),
                vec![self.room_id.clone()],
            )])
            .sign_with_keys(&self.keys)?;

        let output = self.client.send_event(event).await?;
        Ok(output.val)
    }

    /// Subscribe to room events
    pub async fn subscribe(&self, tx: mpsc::Sender<(String, Timestamp)>) -> Result<()> {
        // Filter for room events from the last hour
        let since = Timestamp::now() - 3600;
        let filter = Filter::new()
            .kind(Kind::TextNote)
            .custom_tag(
                SingleLetterTag::lowercase(Alphabet::R),
                vec![self.room_id.clone()],
            )
            .since(since);

        // Handle events
        let client = self.client.clone();
        let tx = tx.clone();

        tokio::spawn(async move {
            let _ = client.subscribe(vec![filter], None).await;

            // Process notifications
            let mut notifications = client.notifications();
            while let Ok(notification) = notifications.recv().await {
                if let RelayPoolNotification::Event { event, .. } = notification {
                    let _ = tx.send((event.content.clone(), event.created_at)).await;
                }
            }
        });

        Ok(())
    }

    /// Get room ID
    pub fn room_id(&self) -> &str {
        &self.room_id
    }

    /// Get my party index
    pub fn my_index(&self) -> u32 {
        self.my_index
    }

    /// Disconnect from relay
    pub async fn disconnect(&self) {
        self.client.disconnect().await.ok();
    }
}

/// Simplified channel-based event receiver
pub struct NostrReceiver {
    rx: mpsc::Receiver<(String, Timestamp)>,
}

impl NostrReceiver {
    pub fn new(rx: mpsc::Receiver<(String, Timestamp)>) -> Self {
        Self { rx }
    }

    /// Try to receive event (non-blocking)
    pub fn try_recv(&mut self) -> Option<String> {
        self.rx.try_recv().ok().map(|(content, _)| content)
    }

    /// Receive event (blocking)
    pub async fn recv(&mut self) -> Option<String> {
        self.rx.recv().await.map(|(content, _)| content)
    }
}

/// Create client and receiver pair
pub async fn create_room_client(
    room_id: &str,
    my_index: u32,
) -> Result<(Arc<NostrClient>, NostrReceiver)> {
    let client = NostrClient::connect(room_id, my_index).await?;
    let (tx, rx) = mpsc::channel(100);

    client.subscribe(tx).await?;

    Ok((Arc::new(client), NostrReceiver::new(rx)))
}
