# FrostDAO Protocols

This is the maintained protocol overview for the current FrostDAO codebase. For exact CLI syntax, use [CLI Reference](CLI.md) or `frostdao <command> --help`.

## Core Concepts

- **TSS**: standard `t-of-n` FROST threshold signing. Every party has rank `0`.
- **HTSS**: hierarchical threshold signing using Birkhoff interpolation. Lower rank numbers have higher authority.
- **DKG wallet**: a threshold wallet produced by distributed key generation. No device holds the full private key.
- **Derived address**: a deterministic BIP-86 Taproot child address from the same DKG root key.
- **Backup manifest**: public metadata that verifies a share mnemonic belongs to the intended wallet and party.
- **Reshare**: rotate or change shares while preserving the wallet identity when the group secret is preserved.
- **Recovery**: reconstruct one lost party share with helper participation.

## DKG

DKG creates a wallet without a trusted dealer.

1. Each party runs `keygen-round1`.
2. Parties exchange round 1 public commitments and encryption pubkeys.
3. Each party runs `keygen-round2`, preferably with `--encrypt`.
4. Parties exchange round 2 payloads intended for each recipient.
5. Each party runs `keygen-finalize`.

Current commands:

```bash
frostdao keygen-round1 --name treasury --threshold 2 --n-parties 3 --my-index 1
frostdao keygen-round2 --name treasury --data '<all_round1_outputs>' --encrypt
frostdao keygen-finalize --name treasury --data '<round2_outputs_for_this_party>'
```

## HTSS

HTSS adds rank-aware authorization.

```bash
frostdao keygen-round1 --name org --threshold 3 --n-parties 5 --my-index 1 --rank 0 --hierarchical
```

Signer set validation uses the Birkhoff rank rule:

```text
sort ranks ascending
valid when rank[i] <= i for every required signer position
```

Example:

- ranks `[0, 1, 1]` satisfy a 3-party requirement.
- ranks `[1, 1, 2]` do not satisfy a 3-party requirement because no rank `0` signer is present.

## HD Address Derivation

FrostDAO derives receive addresses from the same threshold wallet. This does not rerun DKG and does not create a new root wallet.
Each party applies the same public tweak to their own FROST share, so no full private key is reconstructed and no single device can take control of the derived address.
When the group signs for a derived address, every signer must use the selected derivation path and the resulting tweaked shares.
The invariant is: any valid threshold signer set can interpolate the tweaked shares to the HD child key, while fewer than threshold parties still cannot reconstruct or sign for that child key.

```bash
frostdao dkg-derive-address --name treasury --change 0 --index 0 --network testnet
frostdao dkg-list-addresses --name treasury --count 10 --network testnet
```

Only non-hardened child derivation is supported in the threshold setting. Hardened child derivation would require private parent material that the threshold wallet intentionally never reconstructs.

## Share Backup

Each party backs up only their local share.

```bash
frostdao dkg-generate-mnemonic --name treasury
frostdao dkg-verify-mnemonic --name treasury --words '<24 words>'
```

The mnemonic is secret. The backup manifest is public metadata containing wallet, party, rank, threshold, group public key, address, share fingerprint, and backup ID.

## Threshold Transaction Signing

1. Build an unsigned transaction.
2. Generate one-use nonces for the signing session.
3. Create signature shares.
4. Combine and broadcast.

```bash
frostdao dkg-build-tx --name treasury --to <address> --amount <sats> --network testnet
frostdao dkg-nonce --name treasury --session <session_id>
frostdao dkg-sign --name treasury --session <session_id> --sighash <hex> --data '<nonce_outputs>'
frostdao dkg-broadcast --name treasury --session <session_id> --unsigned-tx <hex> --data '<signature_share_outputs>' --network testnet
```

Never reuse a nonce or signing session nonce output.

## Reshare

Reshare is used to rotate shares, replace devices, or change the party set.

```bash
frostdao reshare-round1 --source treasury --new-threshold 2 --new-n-parties 3 --my-index 1
frostdao reshare-finalize --source treasury --target treasury_v2 --my-index 1 --data '<reshare_round1_outputs>'
```

For HTSS reshare finalization, pass `--hierarchical --rank <rank>`.

Expected invariant:

```bash
frostdao dkg-address --name treasury
frostdao dkg-address --name treasury_v2
```

The addresses should match when the reshare preserves the group secret.

## Recovery

Recovery reconstructs one lost party share. Helper parties generate recovery outputs, and the lost party finalizes into a new local wallet name.

```bash
frostdao recover-round1 --name treasury --lost-index 3
frostdao recover-finalize --source treasury --target treasury_recovered --my-index 3 --data '<recovery_outputs>'
```

For HTSS recovery, pass `--hierarchical --rank <original_rank>`.

Expected invariant:

```bash
frostdao dkg-address --name treasury
frostdao dkg-address --name treasury_recovered
```

The addresses should match after successful recovery.

## Nostr Coordination

Nostr is the relay transport for multi-device coordination. It carries FrostDAO protocol envelopes, not raw cryptographic authority.

Use [Nostr Protocol](NOSTR_PROTOCOL.md) for message kinds, envelope fields, replay checks, expiry checks, and TSS/HTSS metadata.

Sensitive payloads must be encrypted with NIP-44 before publishing to relays.

## Miniscript Policy Preview

Miniscript support is optional and is intended for Taproot script-path fallback policies.

```bash
cargo run --features miniscript-policy -- policy-compile \
  --policy 'thresh(2,pk(A),pk(B),pk(C))' \
  --internal-key INTERNAL
```

In the TUI, run with `--features miniscript-policy` and press `p` on the home screen to preview policies.

This does not replace FROST/HTSS key-path signing. Script-path spending needs additional witness/control-block integration.
