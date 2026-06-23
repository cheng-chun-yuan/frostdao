# Nostr Message Protocol

FrostDAO uses Nostr as a relay transport for multi-party coordination. Nostr does not define the cryptography; it only carries FrostDAO protocol messages between parties.

Sensitive material must be encrypted before publishing. DKG shares, signing nonces, signing shares, and reshare sub-shares should be sent as NIP-44 ciphertext payloads.

## Transport

- Relay: `wss://relay.damus.io` by default.
- Event kind: Nostr text note.
- Room filter: custom single-letter `r` tag containing the room ID.
- Nostr keys: ephemeral session keys are acceptable because FrostDAO payload identity is party-index based.

`NostrClient::connect_with_relays` accepts explicit relay URLs for testnet and production-like deployments. `RelayRoomTransport` adapts the async Nostr client to `NostrRoomRuntime`, so relay messages pass through the same room, recipient, timestamp, expiry, and replay-cache checks as deterministic in-memory tests.

## Envelope

All new messages should use this versioned envelope:

```json
{
  "app": "frostdao",
  "version": 1,
  "message_id": "2b8e0b5c9d6f...",
  "created_at": 1700000000,
  "expires_at": 1700003600,
  "room": "treasury-2026",
  "scheme": "htss",
  "wallet": "treasury",
  "session": "session-1700000000",
  "kind": "keygen_round1",
  "from": 1,
  "to": 2,
  "payload": {}
}
```

Fields:

| Field | Required | Description |
|-------|----------|-------------|
| `app` | yes | Must be `frostdao` |
| `version` | yes | Protocol version, currently `1` |
| `message_id` | yes | Deterministic message identifier used for replay protection |
| `created_at` | yes | Unix timestamp when the sender created the message |
| `expires_at` | no | Unix timestamp after which receivers should reject the message |
| `room` | yes | Shared room/session namespace |
| `scheme` | no | `tss` for standard threshold signing, `htss` for hierarchical threshold signing |
| `wallet` | no | Wallet name when known |
| `session` | no | Signing or transaction session ID |
| `kind` | yes | Message kind |
| `from` | yes | Sender party index, 1-based |
| `to` | no | Recipient party index for direct messages |
| `payload` | yes | Message-specific payload |

## Message Kinds

| Kind | Visibility | Purpose |
|------|------------|---------|
| `room_join` | public | Announce participant and Nostr pubkey |
| `room_ready` | public | Announce participant set is ready |
| `keygen_round1` | public | Broadcast DKG commitment and encryption pubkey |
| `keygen_round2_encrypted` | direct | Send encrypted DKG share to one party |
| `tx_proposal` | public | Propose a transaction for review |
| `tx_consent` | public/direct | Consent or reject a transaction proposal |
| `signing_nonce_encrypted` | direct | Send encrypted signing nonce |
| `signing_share_encrypted` | direct | Send encrypted signature share |
| `tx_broadcast` | public | Announce broadcast transaction |
| `reshare_round1` | public | Old party announces reshare round output |
| `reshare_subshare_encrypted` | direct | Send encrypted reshare sub-share |
| `reshare_finalize` | public | Announce new party finalized |
| `recovery_round1` | public | Helper announces recovery round output |
| `recovery_subshare_encrypted` | direct | Send encrypted recovery sub-share to the lost party |
| `recovery_finalize` | public | Announce recovered party finalized |

## Message Acceptance

Receivers should reject messages when:

- `app` or `version` does not match the supported FrostDAO protocol.
- `room` does not match the active ceremony.
- `to` is set and does not match the local party index.
- `created_at` is more than 5 minutes in the future.
- `expires_at` is present and older than the local clock.
- `message_id` was already accepted in the local replay cache.
- `from` or `to` is party index 0.

For long-running ceremonies, use `NostrRoomRuntime` with `FileReplayCache` to persist accepted `message_id` values between process restarts. The file contains only message IDs, not payloads or secret material. TUI room joins and leaves clear volatile participants, proposals, nonce/share inboxes, broadcast announcements, and active ceremony states; the replay cache remains separate.

The default envelope TTL is 1 hour. Long-running ceremonies should create fresh messages instead of extending stale ones.

## Threshold Schemes

FrostDAO supports both standard TSS and HTSS.

### TSS

Use `scheme: "tss"` for normal t-of-n signing. All parties are equivalent and have rank 0.

```json
{
  "app": "frostdao",
  "version": 1,
  "room": "dao-room",
  "scheme": "tss",
  "kind": "room_ready",
  "from": 1,
  "payload": {
    "threshold": 2,
    "n_parties": 3,
    "scheme": "tss"
  }
}
```

### HTSS

Use `scheme: "htss"` for rank-aware signing. Lower rank numbers have higher authority. HTSS uses Birkhoff interpolation, and signer sets must satisfy the rank validity rule.

```json
{
  "app": "frostdao",
  "version": 1,
  "room": "dao-room",
  "scheme": "htss",
  "kind": "room_ready",
  "from": 1,
  "payload": {
    "threshold": 3,
    "n_parties": 5,
    "scheme": "htss",
    "party_ranks": {
      "1": 0,
      "2": 1,
      "3": 1,
      "4": 2,
      "5": 2
    },
    "signing_requirement": [1, 2, 3]
  }
}
```

For sorted signer ranks `[r0, r1, ..., r(t-1)]`, the signer set is valid when `r[i] <= i` for every required signer position.

## DKG Flow

1. Parties join the same room with `room_join`, including `scheme` and optional `rank`.
2. Each party publishes `keygen_round1`.
3. Each party encrypts round 2 shares per recipient using NIP-44.
4. Each party sends `keygen_round2_encrypted` with `to` set to the recipient party.
5. Each recipient finalizes locally after receiving enough valid shares.

For `room_join`, the payload `party_index` must match the envelope `from` party. `keygen_round1.party_index` and `keygen_round2_encrypted.party_index` must also match envelope `from`, and `keygen_round2_encrypted.to_index` must match the direct envelope `to`. The current TUI room runtime also requires the join payload threshold, party count, scheme, and rank shape to match the active room before adding a participant.

## Signing Flow

1. Proposer publishes `tx_proposal`.
2. Signers review and publish/send `tx_consent`.
3. After threshold consent, parties exchange `signing_nonce_encrypted`.
4. Parties exchange `signing_share_encrypted`.
5. Aggregator combines shares, broadcasts the transaction, then publishes `tx_broadcast`.

`tx_proposal` must include a `review` object with `network`, `source_path`, `from_address`, `to_address`, `amount_sats`, `fee_rate_sats_vb`, and `sighash_fingerprint`. Signers should compare these fields across devices before consenting. The TUI runtime accepts inbound proposals only when the proposal's `proposer_index` matches the envelope `from`, the proposer is inside the active room range, amount and fee rate are nonzero, source and recipient addresses parse for the selected network, review amount/fee/recipient fields match the proposal payload, review network matches the selected TUI network, and the review fingerprint matches the proposal sighash.

`tx_consent` should include `reviewed_sighash_fingerprint` so the proposer can confirm which proposal fingerprint each party approved. The TUI proposer counts a consent only when the signer is inside the active room range and the reviewed fingerprint matches the active proposal fingerprint for the same wallet and session.

Session-scoped signing messages must carry a non-empty envelope `session`. Wallet-scoped signing messages must also carry a non-empty envelope `wallet`. The TUI runtime rejects `tx_proposal`, `tx_consent`, `signing_nonce_encrypted`, `signing_share_encrypted`, and `tx_broadcast` messages without an explicit session; consent envelope sessions must match the `proposal_session` payload. TUI outbound encrypted nonce/share messages require a wallet, session, non-empty ciphertext, and a recipient party inside the active room that is not the sender. Encrypted signing nonce/share payloads must bind `party_index` to the envelope `from`, bind `to_index` to the direct envelope `to`, and come from a party in the active room range. Pending proposals are labeled by wallet and the proposal picker only shows proposals for the selected wallet. TUI outbound and inbound `tx_broadcast` announcements are accepted only when the wallet, session, sender room range, selected network, raw transaction hex, and txid all match. Accepted inbound TUI signing messages append metadata-only audit entries; ciphertext, raw transactions, full sighashes, nonces, shares, and secret material are excluded.

## Reshare Flow

1. Existing parties publish `reshare_round1`.
2. Existing parties encrypt sub-shares to new parties.
3. New parties receive `reshare_subshare_encrypted`.
4. New parties finalize and may publish `reshare_finalize`.

Resharing changes party shares, not the wallet identity. The root group public key and derived address paths remain stable if the reshare preserves the same group secret.

For TSS reshare, new parties are rank 0. For HTSS reshare, reshare messages must preserve or explicitly define the new rank map.

Reshare relay envelopes must bind payload identity to transport identity. `reshare_round1.old_party_index` and `reshare_subshare_encrypted.old_party_index` must match the envelope `from`; `reshare_subshare_encrypted.new_party_index` must match the direct envelope `to`; and `reshare_finalize.new_party_index` must match the envelope `from`.

## Recovery Flow

1. Helper parties publish `recovery_round1` for the lost party index.
2. Helper parties encrypt recovery sub-shares to the lost party.
3. The lost party receives `recovery_subshare_encrypted`.
4. The lost party reconstructs the local share, verifies the wallet public key/address, then may publish `recovery_finalize`.

Recovery reconstructs one party share. It must not produce the full wallet secret, and HTSS recovery must preserve the recovered party's original rank.

Recovery relay envelopes follow the same identity binding. `recovery_round1.helper_index` and `recovery_subshare_encrypted.helper_index` must match the envelope `from`; `recovery_subshare_encrypted.lost_index` must match the direct envelope `to`; and `recovery_finalize.recovered_party_index` must match the envelope `from`.

## Compatibility

The Rust Nostr module still parses legacy payloads used by earlier room code:

- `keygen_round1`
- `keygen_round2_encrypted`
- `signing_nonce_encrypted`
- `signing_share_encrypted`

New integrations should prefer the versioned envelope.
