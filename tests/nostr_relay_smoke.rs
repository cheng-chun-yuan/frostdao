//! Opt-in relay integration smoke tests.
//!
//! Run with:
//! FROSTDAO_TEST_NOSTR_RELAYS=wss://relay.damus.io cargo test --all-features --test nostr_relay_smoke -- --ignored --nocapture

use frostdao::nostr::{
    NostrMessageKind, NostrProtocolMessage, NostrRoomRuntime, RelayRoomTransport, RoomJoinPayload,
    ThresholdScheme,
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
    let deadline = Instant::now() + timeout;
    let mut accepted = false;
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

    party_1.transport().disconnect();
    party_2.transport().disconnect();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        accepted,
        "party 2 did not receive party 1 room_join within {:?} through {:?}",
        timeout, relay_urls
    );
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
