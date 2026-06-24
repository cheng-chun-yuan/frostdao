# FrostDAO Run Guide

This guide matches the current CLI version. Use it for local testing, demos, and multi-device dry runs.

## 1. Build And Verify

```bash
cargo build
./scripts/quality.sh
```

Before a release or serious demo:

```bash
./scripts/quality.sh --full
```

This runs formatting checks, strict Clippy, tests, a debug build, and rustdoc warnings.

Transaction build, broadcast, local auto-sign, and TUI Nostr proposal/consent flows append metadata-only audit events to `.frost_state/audit.jsonl`. Accepted inbound TUI Nostr signing messages are also audited with metadata only. Set `FROSTDAO_AUDIT_LOG=/path/to/audit.jsonl` to move the log for demos or CI runs.

## 2. Open The TUI

```bash
cargo run -- tui
```

Use the TUI for guided wallet management. Use the CLI below when you want fully reproducible steps.

## 3. Create A 2-of-3 TSS Wallet

Each device uses the same wallet name, threshold, and party count, but a different `--my-index`.

Device 1:

```bash
frostdao keygen-round1 --name treasury --threshold 2 --n-parties 3 --my-index 1
```

Device 2:

```bash
frostdao keygen-round1 --name treasury --threshold 2 --n-parties 3 --my-index 2
```

Device 3:

```bash
frostdao keygen-round1 --name treasury --threshold 2 --n-parties 3 --my-index 3
```

Collect all round 1 JSON outputs, then run on each device:

```bash
frostdao keygen-round2 --name treasury --data '<all_round1_outputs>' --encrypt
```

Collect all round 2 JSON outputs intended for each party, then run on each device:

```bash
frostdao keygen-finalize --name treasury --data '<round2_outputs_for_this_party>'
```

Check the shared group address:

```bash
frostdao dkg-address --name treasury
```

List wallets:

```bash
frostdao dkg-list
frostdao dkg-address
```

## 4. Create An HTSS Wallet

HTSS adds ranks. Rank `0` is highest authority; larger numbers are lower authority.

Example 3-of-5 setup:

```bash
frostdao keygen-round1 --name org --threshold 3 --n-parties 5 --my-index 1 --rank 0 --hierarchical
frostdao keygen-round1 --name org --threshold 3 --n-parties 5 --my-index 2 --rank 1 --hierarchical
frostdao keygen-round1 --name org --threshold 3 --n-parties 5 --my-index 3 --rank 1 --hierarchical
frostdao keygen-round1 --name org --threshold 3 --n-parties 5 --my-index 4 --rank 2 --hierarchical
frostdao keygen-round1 --name org --threshold 3 --n-parties 5 --my-index 5 --rank 2 --hierarchical
```

Then run the same `keygen-round2` and `keygen-finalize` flow as TSS.

## 5. Derive Receive Addresses

Generate a deterministic receive address from the same threshold wallet:

```bash
frostdao dkg-derive-address --name treasury --change 0 --index 0 --network testnet
```

List the next addresses:

```bash
frostdao dkg-list-addresses --name treasury --count 10 --network testnet
```

These addresses are deterministic tweaks from the same root threshold key. You do not rerun DKG for every new receive address.

## 6. Back Up Your Share

Generate the local party backup:

```bash
frostdao dkg-generate-mnemonic --name treasury
```

Write down the 24 words and keep the printed backup manifest. Verify the written words before relying on them:

```bash
frostdao dkg-verify-mnemonic --name treasury --words '<24 words>'
```

The mnemonic is secret. The manifest is public metadata for checking wallet, party, rank, public key, address, and backup ID.

## 7. Build And Sign A Testnet Transaction

Build an unsigned transaction:

```bash
frostdao dkg-build-tx \
  --name treasury \
  --to <recipient_testnet_address> \
  --amount <satoshis> \
  --fee-rate <sats_per_vbyte> \
  --network testnet
```

Use the returned `session_id` and `sighash` to generate nonces:

Before signing, compare the returned `review` fields on every device: network, source path, source address, destination address, amount, fee, fee rate, and `sighash_fingerprint`.

```bash
frostdao dkg-nonce --name treasury --session <session_id>
```

After collecting nonce JSON from the signing parties:

```bash
frostdao dkg-sign \
  --name treasury \
  --session <session_id> \
  --sighash <hex_sighash> \
  --data '<nonce_outputs>'
```

After collecting enough signature shares:

```bash
frostdao dkg-broadcast \
  --name treasury \
  --session <session_id> \
  --unsigned-tx <unsigned_tx_hex> \
  --data '<signature_share_outputs>' \
  --network testnet
```

DKG transaction build and broadcast commands accept `testnet`, `testnet3`, `signet`, `regtest`, `local`, or `mainnet`. Testnet remains the default. `regtest`/`local` are for local-node rehearsal; mempool.space balance, UTXO, and broadcast APIs are not available for regtest. Mainnet DKG transaction commands are blocked unless `FROSTDAO_ENABLE_MAINNET_BITCOIN=1` is set for that command.

Never reuse a signing session nonce.

In the TUI send flow, `Enter` prepares the transaction review screen. Press `y` only after the review fields match what every signer expects. In Nostr proposal review, `y` publishes consent and `r` publishes an explicit rejection tied to the reviewed proposal fingerprint.

## 8. Reshare Without Changing Address

Old parties generate reshare outputs:

```bash
frostdao reshare-round1 --source treasury --new-threshold 2 --new-n-parties 3 --my-index 1
frostdao reshare-round1 --source treasury --new-threshold 2 --new-n-parties 3 --my-index 2
```

New parties finalize:

```bash
frostdao reshare-finalize \
  --source treasury \
  --target treasury_v2 \
  --my-index 1 \
  --data '<reshare_round1_outputs>'
```

For HTSS reshare, add `--hierarchical --rank <rank>` to `reshare-finalize`.

Verify the address stayed the same:

```bash
frostdao dkg-address --name treasury
frostdao dkg-address --name treasury_v2
```

## 9. Recover A Lost Share

Helper parties create recovery outputs:

```bash
frostdao recover-round1 --name treasury --lost-index 3
```

The lost party finalizes into a new local wallet name:

```bash
frostdao recover-finalize \
  --source treasury \
  --target treasury_recovered \
  --my-index 3 \
  --data '<recovery_outputs>'
```

For HTSS recovery, add `--hierarchical --rank <original_rank>`.

Verify the recovered address:

```bash
frostdao dkg-address --name treasury
frostdao dkg-address --name treasury_recovered
```

## 10. Miniscript Agent Payment Policy

Miniscript support is optional and disabled in default builds.

```bash
cargo run --features miniscript-policy -- policy-compile \
  --policy 'thresh(2,pk(A),pk(B),pk(C))' \
  --internal-key INTERNAL
```

TUI agent payment draft:

```bash
cargo run --features miniscript-policy -- tui
```

Select a wallet, press `p` on the home screen, then fill the agent payment fields.

- `[` / `]` cycles common policy templates.
- `Tab` moves through agent label, agent pubkey, recipient, amount, daily limit, agent index, and policy.
- The policy field is editable for custom Miniscript.
- `Enter` initializes a JSON payment draft using the selected wallet's derived agent address.
- `c` copies the draft.

This initializes a reviewable draft. Script-path transaction signing is not wired yet.

## 11. Nostr Multi-Device Protocol

The current Nostr protocol foundation is documented in [Nostr Protocol](NOSTR_PROTOCOL.md). The envelope supports:

- room join and ready messages
- TSS and HTSS policy metadata
- encrypted keygen round 2 payloads
- transaction proposal and consent
- encrypted signing nonce/share exchange
- reshare messages
- recovery messages
- message replay, recipient, room, timestamp, and expiry checks

The TUI room screen creates a `NostrRoomRuntime` when joining a room. The current TUI runtime uses deterministic in-memory transport for local testnet/demo flows, while still validating versioned envelopes and persisting replay-cache IDs under `.frost_state/nostr_replay/`. Joining or leaving a room clears volatile participants, pending proposals, nonce/share inboxes, broadcast announcements, protocol signing coordinators, and active TUI ceremony states so a previous room cannot leak stale session state into the next one. Room joins are accepted only when the payload party, threshold, party count, scheme, and rank shape match the active room. TUI transaction proposals require a non-empty recipient address valid for the selected network, a nonzero amount, and a loaded wallet with a known source address; accepted outbound proposals are built through the DKG transaction builder so the proposal carries a real unsigned transaction session, BIP341 sighash, and review fingerprint before publishing versioned `tx_proposal` and `tx_consent` messages through this runtime. Runtime polling also ingests incoming `tx_proposal` messages into the pending proposal list only when proposer identity, active room range, network, addresses, amount, fee, and sighash fingerprint match the review payload; incoming `tx_consent` messages are applied to the proposer consent state only when the signer is inside the active room and the reviewed fingerprint matches the active proposal. Outbound encrypted nonce/share messages require a wallet, session, non-empty ciphertext, and a recipient party inside the active room that is not the sender. Encrypted `signing_nonce_encrypted` and `signing_share_encrypted` messages can be published through the same runtime and are ingested into session-keyed TUI inboxes only when the ciphertext payload party and recipient match the envelope sender and local party; share ciphertext is ignored until nonce ciphertext from the same session and sender has already been accepted. When the TUI enters share collection it starts a `protocol::SigningCoordinator`, replays already accepted session nonces into that coordinator, shows a party-by-party nonce/share progress table, and only advances to combining after the coordinator reports threshold-valid shares. NIP-44 signing plaintext helpers encrypt typed nonce/share payloads and reject decrypted plaintext unless it matches the typed wallet, session, attempt, signer-set, sender, recipient, and fingerprint schema before a coordinator may use it. `protocol::SigningCoordinator` state validates the TSS or HTSS signer policy, tracks one active attempt, waits for nonce threshold before share collection, and refuses shares before the matching signer nonce. Session-scoped signing messages without an explicit envelope `session` are ignored, and pending proposals are filtered by their envelope `wallet` before display. `tx_broadcast` announcements are also published and ingested through the runtime; outbound and inbound broadcasts require wallet, session, sender room range, selected network, raw transaction hex, and txid to match. Accepted inbound signing messages append audit metadata such as room, transport, wallet, session, party index, fingerprint, network, and txid as applicable; they never log ciphertext, raw transaction hex, full sighashes, or secret material. The TUI does not simulate final broadcast completion: it waits for a real matching `tx_broadcast` announcement, so use the CLI transaction broadcast flow until relay-backed TUI signing is fully wired.

Reshare and recovery relay parser helpers reject versioned messages whose payload party fields do not match the envelope sender or direct recipient. This protects the copy/paste and future relay-backed ceremony paths from accepting sub-shares under the wrong party identity.

For relay-backed testnet experiments, the codebase exposes `RelayRoomTransport` and `NostrClient::connect_with_relays` so non-demo room wiring can reuse the same runtime checks with explicit relay URLs. The TUI stays in demo transport by default. Set comma-separated relays to opt into relay transport on testnet/signet:

```bash
FROSTDAO_TUI_NOSTR_RELAYS=wss://relay.damus.io cargo run
```

Mainnet relay rooms are blocked unless `FROSTDAO_ENABLE_MAINNET_NOSTR=1` is also set. Keep mainnet disabled until the relay integration tests and remaining live ceremony gates in [Production Readiness](PRODUCTION_READINESS.md) are complete.

Run the opt-in relay smoke test before a relay-backed release candidate:

```bash
./scripts/nostr-relay-smoke.sh ws://127.0.0.1:8080
./scripts/nostr-relay-smoke.sh wss://relay.damus.io
```

Sensitive payloads must be NIP-44 encrypted before relay publishing. The relay should only see public status messages and ciphertext.

## 12. Useful Commands

```bash
frostdao --help
frostdao <command> --help
./scripts/demo.sh
./scripts/demo-reshare.sh
./scripts/quality.sh --full
```

## Current Production Note

The CLI flows, protocol types, guarded TUI Nostr room runtime, runtime-published and runtime-ingested TUI signing messages, deterministic in-memory Nostr transport, opt-in relay TUI transport, relay transport adapter, and quality gate are implemented and tested. Before mainnet production use, finish the remaining live ceremony wiring and relay release testing listed in [Production Readiness](PRODUCTION_READINESS.md).
