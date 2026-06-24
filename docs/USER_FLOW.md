# FrostDAO User Flow

This document defines the recommended product flow for multi-device FrostDAO use. It is written from a user perspective, then maps each step to the protocol messages and commands that already exist.

## Core Model

FrostDAO should feel like five clear jobs:

1. Create or join a room.
2. Create a wallet with TSS or HTSS.
3. Derive receive addresses from the wallet.
4. Propose and sign transactions.
5. Reshare or recover when membership changes.

Nostr is the message transport. NIP-44 protects sensitive messages. FROST, Birkhoff, and Bitcoin signing remain local cryptographic operations.

```text
Device A         Nostr relay          Device B
--------         -----------          --------
local share  ->  encrypted message -> local share
local check  <-  public status     <- local check
```

The relay should never receive private shares, nonces, or signature shares in plaintext.

## Flow 1: Join Room

Goal: get all devices into the same coordination space.

User steps:

1. Organizer creates a room ID.
2. Each device opens FrostDAO and chooses `Join room`.
3. Each signer enters:
   - room ID
   - party index
   - threshold
   - total parties
   - scheme: `TSS` or `HTSS`
   - rank, only for HTSS
4. Each device publishes `room_join`.
5. The room becomes ready when all expected parties are present.

UX requirements:

- Show `Party X of N` on every device.
- Show scheme clearly: `TSS` or `HTSS`.
- For HTSS, show rank and authority meaning.
- Show relay status separately from protocol readiness.
- Allow copying the room ID and showing it as a QR code later.

Protocol:

```text
room_join
room_ready
```

## Flow 2: Create Wallet

Goal: create one threshold wallet without any complete private key existing on one device.

User steps:

1. Choose wallet name.
2. Confirm scheme:
   - TSS: any `t` of `n` parties can sign.
   - HTSS: signers must satisfy rank policy.
3. Each device runs DKG round 1 locally.
4. Devices publish public commitments and encryption pubkeys.
5. Each device encrypts round 2 shares per recipient.
6. Each device receives enough encrypted shares.
7. Each device finalizes locally.
8. The app shows the shared root Taproot address and first derived receive address.

UX requirements:

- Split the screen by rounds: Round 1, Round 2, Finalize.
- Label messages:
  - Round 1 commitment: public
  - Round 2 share: encrypted to one party
  - Final wallet share: local only
- Show per-party progress.
- Keep a visible warning: never paste plaintext shares into public chat.
- For HTSS, validate rank policy before finalization.

Protocol:

```text
keygen_round1
keygen_round2_encrypted
```

CLI equivalent:

```bash
frostdao keygen-round1 --name treasury --threshold 2 --n-parties 3 --my-index 1
frostdao keygen-round2 --name treasury --data '<round1_outputs>' --encrypt
frostdao keygen-finalize --name treasury --data '<round2_outputs>'
```

For HTSS:

```bash
frostdao keygen-round1 --name treasury --threshold 3 --n-parties 5 --my-index 1 --rank 0 --hierarchical
```

## Flow 3: Derive Addresses

Goal: generate new receive addresses from the same threshold wallet.

User steps:

1. Open wallet.
2. Choose `Receive`.
3. Pick address index or use next unused index.
4. Show derived Taproot address.
5. Optionally show path, QR, and copy button.

Important behavior:

- This does not rerun DKG.
- This does not reuse the exact same private key.
- Each derived address is a deterministic tweak of the root threshold key.
- Parties can later sign for that derived path with threshold shares.

CLI equivalent:

```bash
frostdao dkg-derive-address --name treasury --change 0 --index 0
frostdao dkg-list-addresses --name treasury --count 10
```

UX requirements:

- Use language like `Derived address`, not `new wallet`.
- Show path with the selected network's BIP-86 coin type: `m/86'/0'/0'/0/index` on mainnet, `m/86'/1'/0'/0/index` on testnet, signet, and regtest.
- Explain that the same MPC threshold signer set controls all derived addresses.
- Explain that each derived address is controlled by tweaked threshold shares for the selected path, not by a newly reconstructed private key.

## Flow 4: Propose Transaction

Goal: one signer creates a transaction proposal for the group to review.

User steps:

1. Proposer selects wallet and derived address or root address.
2. Proposer enters recipient, amount, fee rate, and note.
3. App builds unsigned transaction and sighash.
4. App publishes transaction proposal.
5. Other devices receive proposal and review details.

UX requirements:

- Review screen must show:
  - wallet
  - source address/path
  - recipient
  - amount
  - fee
  - network
  - sighash fingerprint
  - proposer party
- Mainnet requires a stronger confirmation step.
- Consent should be explicit: `Approve proposal` or `Reject proposal`.

Protocol:

```text
tx_proposal
tx_consent
```

CLI equivalent:

```bash
frostdao dkg-build-tx --name treasury --to <address> --amount <sats> --network testnet
```

## Flow 5: Sign Transaction

Goal: collect enough valid signing material and broadcast a transaction.

User steps:

1. After enough consents, each signing device generates a nonce locally.
2. Nonces are encrypted per recipient or signing group policy.
3. Each signer creates a signature share locally.
4. Signature shares are encrypted and sent.
5. Aggregator combines enough valid shares.
6. App broadcasts transaction.
7. All devices receive broadcast result.

UX requirements:

- Warn that nonces are single-use.
- Show valid signer set status:
  - TSS: `2 of 3 collected`
  - HTSS: `rank policy satisfied` or specific missing authority
- Do not show raw secret material unless user explicitly opens advanced details.
- If combine fails, show whether the problem is missing shares, invalid ranks, invalid share, or wrong session.

Protocol:

```text
signing_nonce_encrypted
signing_share_encrypted
tx_broadcast
```

CLI equivalent:

```bash
frostdao dkg-nonce --name treasury --session <session>
frostdao dkg-sign --name treasury --session <session> --sighash <hex> --data '<nonces>'
frostdao dkg-broadcast --name treasury --session <session> --unsigned-tx <hex> --data '<shares>'
```

## Flow 6: Reshare

Goal: change or refresh shares without changing the wallet identity.

Use cases:

- Replace a lost or retired device.
- Rotate shares after suspected exposure.
- Change from one party set to another.
- Preserve the same root public key and derived addresses.

User steps:

1. Choose source wallet.
2. Choose target wallet name.
3. Choose new threshold and party count.
4. Choose scheme:
   - TSS: all new parties rank 0.
   - HTSS: define new party ranks.
5. Existing parties publish reshare round 1 outputs.
6. Existing parties encrypt sub-shares to new parties.
7. New parties finalize locally.
8. App compares old and new address fingerprints.

UX requirements:

- Make the invariant visible: `Address stays the same`.
- Show old party set and new party set side by side.
- For HTSS, show old rank map and new rank map.
- Prevent accidental privilege escalation during recovery.
- Require confirmation if target wallet already exists.

Protocol:

```text
reshare_round1
reshare_subshare_encrypted
reshare_finalize
```

CLI equivalent:

```bash
frostdao reshare-round1 --source treasury --new-threshold 2 --new-n-parties 3 --my-index 1
frostdao reshare-finalize --source treasury --target treasury_v2 --my-index 1 --data '<round1_outputs>'
```

## Flow 7: Recover Share

Goal: reconstruct a lost party share without exposing the full wallet secret.

User steps:

1. Lost party creates target wallet name.
2. Helper parties generate recovery sub-shares.
3. Lost party receives enough recovery data.
4. App reconstructs the lost share locally.
5. App verifies the recovered wallet has the same public key/address.

UX requirements:

- Make helper role and lost-party role separate.
- Preserve original rank for HTSS recovery.
- Show that helpers cannot recover their own missing share.
- Show final verification before marking recovery complete.

Protocol:

```text
recovery_round1
recovery_subshare_encrypted
recovery_finalize
```

CLI equivalent:

```bash
frostdao recover-round1 --name treasury --lost-index 2
frostdao recover-finalize --source treasury --target treasury_recovered --my-index 2 --data '<recovery_outputs>'
```

## Recommended First Screen

The best UX entry point is not a command list. It should be a task picker:

```text
FrostDAO

What do you want to do?

[ Create wallet ]       DKG with TSS or HTSS
[ Join signing room ]   Review and sign a proposal
[ Receive bitcoin ]     Derive a new address
[ Reshare wallet ]      Rotate shares, keep address
[ Recover share ]       Restore a lost party
```

Each task should show:

- current network
- local party identity
- wallet selected
- relay state
- security status

## Multi-Device State Model

Every device should track the same high-level state:

| State | Meaning |
|-------|---------|
| `local_ready` | Local wallet/share material is available |
| `relay_connected` | Nostr relay is reachable |
| `room_ready` | Expected parties have joined |
| `round_ready` | Required messages for current round are present |
| `policy_valid` | TSS threshold or HTSS rank rule is satisfied |
| `finalized` | Local ceremony step is complete |

This state model is more useful to users than raw protocol names alone.

## UX Priority

Build in this order:

1. Room join and participant readiness.
2. TSS DKG over Nostr.
3. HTSS DKG over Nostr with rank map.
4. Transaction proposal and consent.
5. Signing nonce/share exchange.
6. Reshare over Nostr.
7. Recovery over Nostr.

This order gives users a working multi-device story before adding every advanced ceremony.
