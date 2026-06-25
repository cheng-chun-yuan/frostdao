# FrostDAO Run Guide

This guide matches the current CLI version. Use it for local testing and multi-device dry runs.

## 1. Build And Verify

```bash
cargo build
./scripts/quality.sh
```

Before a release or recorded walkthrough:

```bash
./scripts/quality.sh --full
```

This runs formatting checks, strict Clippy, tests, a debug build, and rustdoc warnings.

Transaction build, broadcast, local auto-sign, and TUI Nostr proposal/consent flows append metadata-only audit events to `.frost_state/audit.jsonl`. Accepted inbound TUI Nostr signing messages are also audited with metadata only. Audit field insertion drops sensitive field names such as ciphertext, raw transactions, full sighashes, nonces, shares, and mnemonics. Set `FROSTDAO_AUDIT_LOG=/path/to/audit.jsonl` to move the log for walk-throughs or CI runs.

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
frostdao dkg-derive-address --name treasury --change 0 --index 0 --network testnet4
```

List the next addresses:

```bash
frostdao dkg-list-addresses --name treasury --count 10 --network testnet4
```

These addresses are deterministic BIP-86 tweaks from the same root threshold key. You do not rerun DKG for every new receive address.

Control model:

- `change=0` is the external receive chain.
- `change=1` is the internal change chain.
- Only non-hardened child levels are supported after the account key.
- A new address does not create a single-device private key.
- Every signer derives the same public tweak and applies it to their own local share.
- Spending from the derived address still requires a valid TSS or HTSS signer set for that wallet.
- The TUI send source selector, send review, and Nostr proposal review show the selected source path and source address so all devices can compare the same HD child before signing.
- The TUI send flow also shows a child x-only pubkey fingerprint for HD sources. The path, address, and child pubkey fingerprint should match on every signer device before anyone confirms.
- The TUI HD address-list details panel shows the selected network policy and exact `UTXO API` endpoint before the derived address. The QR area labels each QR as a public derived receive address and reminds the operator to verify the selected network and BIP-86 path before funding.
- If no HD source is available in the TUI send address selector, the screen names the active network and says whether the wallet is root-source-only or needs an HD-derived address added first.

## 6. Back Up Your Share

Generate the local party backup:

```bash
frostdao dkg-generate-mnemonic --name treasury
```

Write down the 24 words and keep the printed backup manifest. Verify the written words before relying on them:

```bash
frostdao dkg-verify-mnemonic --name treasury --words '<24 words>'
```

The mnemonic is secret. `frostdao dkg-restore-mnemonic` restores only that party's local share; it does not reconstruct all shares or the full wallet key. The manifest is public metadata for checking wallet, party, rank, public key, address, and backup ID.

## 7. Build And Sign A Testnet Transaction

Build an unsigned transaction:

```bash
frostdao dkg-build-tx \
  --name treasury \
  --to <recipient_testnet4_address> \
  --amount <satoshis> \
  --fee-rate <sats_per_vbyte> \
  --network testnet4
```

Use the returned `session_id` and `sighash` to generate nonces:

Before signing, compare the returned `review` fields on every device: network, source path, source address, destination address, amount, fee, fee rate, and `sighash_fingerprint`.
The sighash is a public transaction digest to compare after review; it is not a nonce, signature share, or private key material.

```bash
frostdao dkg-nonce --name treasury --session <session_id>
```

After collecting nonce JSON from the signing parties:
Nonce JSON is session-bound and single-use; never reuse it for another wallet, transaction, or signing attempt.

```bash
frostdao dkg-sign \
  --name treasury \
  --session <session_id> \
  --sighash <hex_sighash> \
  --data '<nonce_outputs>'
```

After collecting enough signature shares:
Signature share JSON goes to the aggregator only and is not a complete signature by itself.

```bash
frostdao dkg-broadcast \
  --name treasury \
  --session <session_id> \
  --unsigned-tx <unsigned_tx_hex> \
  --data '<signature_share_outputs>' \
  --network testnet4
```

DKG transaction build and broadcast commands accept `testnet`/`testnet3`, `testnet4`/`test4`, `signet`, `regtest`/`local`, or `mainnet`. `testnet` remains the CLI default and means testnet3; the TUI defaults to explicit testnet4. `regtest`/`local` are for local-node rehearsal; set `FROSTDAO_REGTEST_MEMPOOL_API` to a local Esplora/mempool API endpoint such as `http://127.0.0.1:3002/api` before fetching UTXOs, building transactions, or broadcasting on regtest. Mainnet DKG transaction commands are blocked unless `FROSTDAO_ENABLE_MAINNET_BITCOIN=1` is set for that command.

Never reuse a signing session nonce.

The TUI home and wallet details screens show the selected network policy before the wallet address. Testnet, testnet4, and signet show the remote mempool.space UTXO source; regtest uses the local Esplora/mempool API from `FROSTDAO_REGTEST_MEMPOOL_API`; mainnet shows the real-funds opt-in warning. The Home network panel, Wallet Details, and network selector also show the exact `UTXO API` endpoint or missing regtest configuration that balance, send, and Nostr proposal flows will use. Copy address, QR display, wallet details, and Nostr proposal validation all use the selected network's known root address. The QR popup labels the address as a public receive address and reminds the operator to verify the selected network before funding. If a wallet lacks an address for the active network, Home and Wallet Details name the missing network address and tell the user to select a wallet/network with an address before send, copy, or QR. The TUI must not fake a mainnet address by rewriting a testnet prefix, and it must not reuse a `tb...` testnet address as a regtest `bcrt...` address.
Home balance status is network-scoped. Press `b`, `r`, or `F5` from Home to fetch the selected wallet balance for the selected network; the wallet list only shows a cached-balance marker when that same wallet/network pair has been fetched. Home send readiness means the source address and UTXO API are available to fetch UTXOs; transaction review still requires confirmed UTXOs and enough confirmed balance. The persistent Home help bar includes `b/r/F5 (Refresh):Balance` so the fetch path remains visible. Home Nostr readiness stays in setup mode until a room is joined, then labels the active room as local rehearsal or relay signing.
Confirming a different TUI network clears pending send form data, reshare draft data, and volatile Nostr ceremony state, including room participants, pending proposals, nonce/share inboxes, broadcasts, and signing coordinators. Confirming the current network leaves those drafts unchanged. Network-scoped balance cache entries may remain because their keys include the selected network.

In the TUI send flow, `Enter` prepares the transaction review screen only after the selected source address has confirmed UTXOs and enough confirmed balance for the amount plus estimated fee. Signer selection must match exactly the required threshold before review; adding another signer after threshold requires deselecting one first. The recipient form previews the expected address prefix for the selected network (`tb1...`, `bcrt1...`, or `bc1...`). Recipient validation names the active network, and amount validation requires sats greater than 0 before a review can be prepared. If the selected network cannot provide UTXOs, such as regtest without `FROSTDAO_REGTEST_MEMPOOL_API`, the TUI keeps the user on the transaction-details step instead of allowing a misleading review. Press `y` only after the review fields match what every signer expects, and the Send review help bar repeats that `y` confirms only after every signer sees the same review. The local send review shows wallet name, network, source path and address, destination, amount/fee, total debit, remaining balance, and signers, and labels the local-only boundary: share material stays on this device, and only the signed raw transaction or TXID leaves the signing flow. TUI local sign and broadcast is blocked on mainnet; use CLI commands with explicit mainnet opt-in after offline review. On test chains, the local auto-sign path reports whether the selected network API accepted the broadcast; if broadcast fails or is unavailable, the completion screen keeps the raw transaction copy path available for manual broadcast. In Nostr proposal review, the consent checklist shows wallet name, network/session, source path and address, destination, amount/fee, sighash fingerprint, unsigned transaction fingerprint, and proposer before `y` publishes consent; the help bar says `y` consents only after every signer matches the review. Press `r` to publish an explicit rejection tied to the reviewed proposal fingerprint. Proposers see approved, rejected, and pending parties in the consent list.

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

In every reshare ceremony:

- Local refresh mode keeps secret share material on each signing device and rewrites that device's full local share set.
- Distributed round1 outputs are sub-shares only and are required for one target wallet and one recipient party index.
- The TUI finalize paste area shows the target party index and HTSS rank context; reject outputs for another party index, rank, or wallet.
- The target wallet root public key and root address remain the same as the source wallet, so verify the shared key/address on every participant before signing after the operation.
- The TUI address check is network-scoped. It names the selected network for match, mismatch, and missing-address results. If the source or target address is unavailable on the selected network, compare the named source and target wallets manually before using the reshared wallet.

## 9. Recover A Lost Share

Recovery is currently a CLI ceremony, not a TUI wizard. The TUI keymap documents
this boundary so operators do not look for a hidden recovery path.

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

In every recovery ceremony:

- Recovery reconstructs one lost party share only, not the full wallet private key.
- Helper outputs are intended for the recovering party and should not be pasted into unrelated ceremonies.
- For HTSS, the recovered party must use the original rank, not a new rank chosen during recovery.
- The recovered wallet root public key and root address should match the source wallet on the selected network before anyone signs with it.

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

This initializes a reviewable draft. Script-path transaction signing is not available in the current TUI flow.

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

The TUI room screen creates a `NostrRoomRuntime` when joining a room. The current TUI runtime uses deterministic in-memory transport for local simulation flows, while still validating versioned envelopes and persisting replay-cache IDs under `.frost_state/nostr_replay/`. Joining or leaving a room clears volatile participants, pending proposals, nonce/share inboxes, broadcast announcements, protocol signing coordinators, and active TUI ceremony states so a previous room cannot leak stale session state into the next one. The room configure status line shows `Blocked` until the room ID, party index, threshold, and party count form a valid room. TUI Nostr room configure screen labels this as a multi-device signing room. The room info line shows room ID, party index, threshold, scheme, rank, and transport; current TUI room joins are guarded as `Scheme: TSS` with `Rank: n/a`, and the configure screen says the current room join shape is TSS-only so HTSS users do not assume rank-aware room setup is exposed yet. Room joins are public metadata, while signing nonce/share payloads are encrypted. Room joins are accepted only when the payload party, threshold, party count, scheme, and rank shape match the active room. When all parties have joined, local simulation labels the room as ready for local rehearsal, while relay transport labels it as ready for signing only. Nostr keygen status keeps room, party, threshold, scheme, rank, and transport visible across setup, round 1, round 2, and finalizing, and labels public commitments, encrypted share payloads, and local-only final share material. TUI relay keygen is intentionally unavailable; local simulation can still start a keygen rehearsal, and relay users should create keys with CLI keygen before using the room for signing. Nostr signing status keeps room, party, threshold, scheme, rank, and transport visible across proposal, consent, nonce, share, combine, and broadcast phases. In Nostr signing, the proposer configure screen has editable recipient and amount fields; `Tab` moves between fields and `Enter` publishes only after those fields validate. Regtest proposals use the same transaction builder when `FROSTDAO_REGTEST_MEMPOOL_API` points at a local Esplora/mempool API; without that endpoint the configure screen explains the missing setting before publishing. TUI transaction proposals require a non-empty recipient address valid for the selected network, a nonzero amount, and a loaded wallet with a known source address. If the proposer has selected an HD-derived source address, the proposal is built from that selected source path instead of silently falling back to the root key-path address. Accepted outbound proposals are built through the DKG transaction builder so the proposal carries a real unsigned transaction hex, transaction session, source path, source address, BIP341 sighash, and review fingerprint before publishing versioned `tx_proposal` and `tx_consent` messages through this runtime. Runtime polling also ingests incoming `tx_proposal` messages into the pending proposal list only when proposer identity, active room range, network, addresses, amount, fee, unsigned transaction hex, and sighash fingerprint match the review payload; the pending proposal list shows only proposals for the selected wallet and active room, and displays wallet, network, session, proposer, amount, destination, and fingerprint before the operator opens the full review. Incoming `tx_consent` messages are applied to the proposer consent state only when the signer is inside the active room and the reviewed fingerprint matches the active proposal, with approvals and rejections tracked separately in the party list. After a signer consents, the waiting state tells them to keep the room open until threshold consent lets parties exchange encrypted nonces and signature shares. Pressing `Enter` in that state only polls the room; it does not start signing before threshold consent. Outbound encrypted nonce/share messages require a wallet, session, non-empty ciphertext, and a recipient party inside the active room that is not the sender. Encrypted `signing_nonce_encrypted` and `signing_share_encrypted` messages can be published through the same runtime and are ingested into session-keyed TUI inboxes only when the ciphertext payload party and recipient match the envelope sender and local party; share ciphertext is ignored until nonce ciphertext from the same session and sender has already been accepted. When the TUI enters share collection it starts a `protocol::SigningCoordinator`, replays already accepted session nonces into that coordinator, shows a party-by-party nonce/share progress table, and only advances to a CLI handoff after the coordinator reports threshold-valid shares. The handoff caches the reviewed proposal and lets the operator press `c` to copy a `frostdao dkg-broadcast` command with the wallet, session, unsigned transaction, and selected network already filled in; replace `<shares-json>` with the collected threshold shares. After CLI broadcast, the room waits for a matching `tx_broadcast` announcement. A matching `tx_broadcast` is a room announcement, not an on-chain confirmation. NIP-44 signing plaintext helpers encrypt typed nonce/share payloads and reject decrypted plaintext unless it matches the typed wallet, session, attempt, signer-set, sender, recipient, and fingerprint schema before a coordinator may use it. `protocol::SigningCoordinator` state validates the TSS or HTSS signer policy, tracks one active attempt, waits for nonce threshold before share collection, and refuses shares before the matching signer nonce. Session-scoped signing messages without an explicit envelope `session` are ignored, and pending proposals are filtered by their envelope `wallet` before display. `tx_broadcast` announcements are also published and ingested through the runtime; outbound and inbound broadcasts require wallet, session, sender room range, selected network, raw transaction hex, and txid to match. Accepted inbound signing messages append audit metadata such as room, transport, wallet, session, party index, fingerprint, network, and txid as applicable; they never log ciphertext, raw transactions, full sighashes, nonces, shares, or secret material. The TUI does not simulate final broadcast completion: it waits for a real matching `tx_broadcast` announcement, so use the CLI transaction broadcast flow when no relay broadcast announcement is available.

While a consenter waits after approval, the TUI shows the same session party-by-party nonce/share progress table used by the coordinator so multi-device progress is visible on every signer. Once a room is joined, outbound direct nonce/share publishing also requires the recipient party to have joined the active room, so relay mode has recipient key material before a direct signing envelope is sent.

In relay mode, outbound signing nonces and shares are wrapped in the typed plaintext schema and NIP-44 encrypted with the local relay session key and the recipient's joined Nostr pubkey before publishing. Accepted signing nonce/share ciphertext is decrypted with the local relay session key and the sender's joined Nostr pubkey before it is counted as coordinator input. The decrypted typed plaintext must match the envelope wallet, session, sender, recipient, signer set, attempt, and sighash fingerprint. Local simulation keeps using direct test strings for rehearsal only.

Reshare and recovery relay parser helpers reject versioned messages whose payload party fields do not match the envelope sender or direct recipient. This protects the copy/paste and future relay-backed ceremony paths from accepting sub-shares under the wrong party identity.

For relay-backed testnet experiments, the codebase exposes `RelayRoomTransport` and `NostrClient::connect_with_relays` so relay room wiring can reuse the same runtime checks with explicit relay URLs. In relay mode, TUI room joins advertise `NostrClient::my_pubkey()` instead of a local simulation label, which gives peers the recipient key material needed for encrypted nonce/share exchange. The TUI stays in local simulation transport by default. Set comma-separated relays to opt into relay transport on testnet/signet:

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
./scripts/dkg-walkthrough.sh
./scripts/reshare-walkthrough.sh
./scripts/quality.sh --full
```

## Current Production Note

The CLI flows, protocol types, guarded TUI Nostr room runtime, runtime-published and runtime-ingested TUI signing messages, deterministic in-memory local simulation transport, opt-in relay TUI transport, relay transport adapter, and quality gate are implemented and tested. Before mainnet production use, finish the remaining live ceremony wiring and relay release testing listed in [Production Readiness](PRODUCTION_READINESS.md).
