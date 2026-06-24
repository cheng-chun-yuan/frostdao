//! Opt-in relay integration smoke tests.
//!
//! Run with:
//! FROSTDAO_TEST_NOSTR_RELAYS=wss://relay.damus.io cargo test --all-features --test nostr_relay_smoke -- --ignored --nocapture

use frostdao::nostr::{
    NostrMessageKind, NostrProtocolMessage, NostrRoomRuntime, RelayRoomTransport, RoomJoinPayload,
    SigningNonceEvent, SigningShareEvent, ThresholdScheme, TxBroadcastEvent,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[test]
#[ignore = "requires reachable Nostr relay URLs in FROSTDAO_TEST_NOSTR_RELAYS"]
fn relay_room_transport_round_trips_public_room_join() {
    let relay_urls = relay_urls_from_env();
    let room = format!(
        "frostdao-smoke-{}-{}",
        std::process::id(),
        unix_timestamp_secs()
    );
    let dir = std::env::temp_dir().join(format!("frostdao-relay-smoke-{}", std::process::id()));
    let path_1 = dir.join("party-1.json");
    let path_2 = dir.join("party-2.json");
    let _ = std::fs::remove_dir_all(&dir);

    let transport_2 = RelayRoomTransport::connect(&room, 2, &relay_urls).unwrap();
    let mut party_2 = NostrRoomRuntime::load(room.clone(), 2, transport_2, &path_2).unwrap();
    let transport_1 = RelayRoomTransport::connect(&room, 1, &relay_urls).unwrap();
    let mut party_1 = NostrRoomRuntime::load(room.clone(), 1, transport_1, &path_1).unwrap();

    std::thread::sleep(Duration::from_millis(1500));

    let join = RoomJoinPayload {
        party_index: 1,
        nostr_pubkey: "npub-smoke-party-1".to_string(),
        threshold: 2,
        n_parties: 2,
        scheme: ThresholdScheme::Tss,
        rank: None,
    };
    let message = NostrProtocolMessage::new(room.clone(), NostrMessageKind::RoomJoin, 1, &join)
        .unwrap()
        .with_tss();
    party_1.publish(message).unwrap();

    let timeout = smoke_timeout();
    let mut accepted = false;
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let messages = party_2.receive(unix_timestamp_secs()).unwrap();
        accepted = messages.iter().any(|message| {
            message.kind == NostrMessageKind::RoomJoin && message.from == 1 && message.room == room
        });
        if accepted {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    let nonce = SigningNonceEvent::new(1, 2, "relay-smoke-ciphertext".to_string());
    let direct_message = NostrProtocolMessage::new(
        room.clone(),
        NostrMessageKind::SigningNonceEncrypted,
        1,
        &nonce,
    )
    .unwrap()
    .with_tss()
    .with_session("relay-smoke-session")
    .to_party(2)
    .unwrap();
    party_1.publish(direct_message).unwrap();

    let mut direct_accepted = false;
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let messages = party_2.receive(unix_timestamp_secs()).unwrap();
        direct_accepted = messages.iter().any(|message| {
            message.kind == NostrMessageKind::SigningNonceEncrypted
                && message.from == 1
                && message.to == Some(2)
                && message.session.as_deref() == Some("relay-smoke-session")
        });
        if direct_accepted {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    let party_1_messages = party_1.receive(unix_timestamp_secs()).unwrap();
    let party_1_accepted_own_direct = party_1_messages
        .iter()
        .any(|message| message.kind == NostrMessageKind::SigningNonceEncrypted);

    assert!(
        accepted,
        "party 2 did not receive party 1 room_join within {:?} through {:?}",
        timeout, relay_urls
    );
    assert!(
        direct_accepted,
        "party 2 did not receive party 1 direct signing_nonce_encrypted within {:?} through {:?}",
        timeout, relay_urls
    );
    assert!(
        !party_1_accepted_own_direct,
        "party 1 accepted a direct message addressed to party 2"
    );

    let share = SigningShareEvent::new(1, 2, "relay-smoke-share-ciphertext".to_string());
    let share_message = NostrProtocolMessage::new(
        room.clone(),
        NostrMessageKind::SigningShareEncrypted,
        1,
        &share,
    )
    .unwrap()
    .with_tss()
    .with_session("relay-smoke-session")
    .to_party(2)
    .unwrap();
    party_1.publish(share_message).unwrap();

    let mut share_accepted = false;
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let messages = party_2.receive(unix_timestamp_secs()).unwrap();
        share_accepted = messages.iter().any(|message| {
            message.kind == NostrMessageKind::SigningShareEncrypted
                && message.from == 1
                && message.to == Some(2)
                && message.session.as_deref() == Some("relay-smoke-session")
        });
        if share_accepted {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    let broadcast = TxBroadcastEvent {
        txid: "relay-smoke-txid".to_string(),
        raw_tx: "02000000000100".to_string(),
        network: "testnet".to_string(),
    };
    let broadcast_message =
        NostrProtocolMessage::new(room.clone(), NostrMessageKind::TxBroadcast, 1, &broadcast)
            .unwrap()
            .with_tss()
            .with_wallet("relay-smoke-wallet")
            .with_session("relay-smoke-session");
    party_1.publish(broadcast_message).unwrap();

    let mut broadcast_accepted = false;
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let messages = party_2.receive(unix_timestamp_secs()).unwrap();
        broadcast_accepted = messages.iter().any(|message| {
            message.kind == NostrMessageKind::TxBroadcast
                && message.from == 1
                && message.to.is_none()
                && message.wallet.as_deref() == Some("relay-smoke-wallet")
                && message.session.as_deref() == Some("relay-smoke-session")
        });
        if broadcast_accepted {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    assert!(
        share_accepted,
        "party 2 did not receive party 1 direct signing_share_encrypted within {:?} through {:?}",
        timeout, relay_urls
    );
    assert!(
        broadcast_accepted,
        "party 2 did not receive party 1 tx_broadcast within {:?} through {:?}",
        timeout, relay_urls
    );

    party_1.transport().disconnect();
    party_2.transport().disconnect();
    let _ = std::fs::remove_dir_all(&dir);
}

fn relay_urls_from_env() -> Vec<String> {
    std::env::var("FROSTDAO_TEST_NOSTR_RELAYS")
        .expect("set FROSTDAO_TEST_NOSTR_RELAYS to one or more comma-separated relay URLs")
        .split(',')
        .map(str::trim)
        .filter(|relay| !relay.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>()
}

fn smoke_timeout() -> Duration {
    let seconds = std::env::var("FROSTDAO_TEST_NOSTR_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(20);
    Duration::from_secs(seconds)
}

fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(1)
}
