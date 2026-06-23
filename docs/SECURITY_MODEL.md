# FrostDAO Security Model

This document is the maintained security summary for the current FrostDAO codebase. It is not a formal audit.

## Main Invariants

- No single party should learn the full wallet private key during DKG, signing, reshare, or recovery.
- Private shares stay local to each device.
- Share mnemonics are backed by a public manifest that binds them to wallet metadata and a share fingerprint.
- DKG round 2 shares, signing nonces, signing shares, reshare sub-shares, and recovery sub-shares must be encrypted before relay transport.
- Signing nonces are single-use.
- Reshare should preserve the group public key/address when the goal is device rotation.
- Recovery reconstructs only the lost party share.
- HTSS signing and recovery must preserve rank policy.

## Threats Covered

| Threat | Mitigation |
|--------|------------|
| One compromised device | Threshold signing requires enough valid parties |
| Relay sees messages | Sensitive payloads are NIP-44 encrypted |
| Relay replays messages | Nostr envelope has `message_id`, timestamps, expiry, and replay cache checks |
| Wrong recipient consumes direct message | Envelope `to` is checked against local party index |
| Wrong room/session message accepted | Envelope `room` and session metadata are checked by receivers |
| Minority signing attempt | TSS threshold and HTSS rank validation reject invalid signer sets |
| Device replacement changes wallet | Reshare verifies address/public-key invariants |
| Wrong mnemonic used for wallet | `dkg-verify-mnemonic` checks manifest and share fingerprint |

## TSS And HTSS Policy

For standard TSS, any `t` valid parties can sign.

For HTSS, signer ranks must satisfy:

```text
sort ranks ascending
rank[i] <= i
```

This prevents lower-authority-only groups from satisfying a high-authority policy.

## Nonce Safety

Nonce reuse can leak signing key material. Treat every signing session as one-time:

- Generate new nonces per session.
- Bind nonces to a session ID.
- Delete nonce state after successful signing.
- Do not retry a failed signing session with the same nonce output.

## Audit Logging

FrostDAO writes metadata-only JSONL audit events to `.frost_state/audit.jsonl` by default. Set `FROSTDAO_AUDIT_LOG=/path/to/audit.jsonl` to choose another path.

Audit events may include wallet name, event type, status, session ID, room ID, transport mode, network, source path, addresses, amount, fee, fee rate, signer indexes, txid, and sighash fingerprint.

Audit events must not include mnemonics, secret shares, private keys, nonces, signature shares, ciphertext payloads, or raw transactions.

## Recovery Caution

The current recovery flow is useful for restoring a lost party share, but it should be handled as a high-risk ceremony:

- Run recovery over encrypted channels.
- Verify the recovered wallet address before deleting any backup state.
- For HTSS, pass the original rank of the recovered party.
- Do not let helpers recover their own missing share.

## Backup Caution

The 24-word share mnemonic is equivalent to that party's local secret share. The backup manifest is intentionally public and does not contain the secret.

Before relying on a backup:

```bash
frostdao dkg-verify-mnemonic --name <wallet> --words '<24 words>'
```

Do not store all party mnemonics together. That defeats the threshold model.

## Mainnet Requirements

Before mainnet use:

- Complete the gates in [Production Readiness](PRODUCTION_READINESS.md).
- Test against a local relay and at least one independent relay.
- Run the full quality gate.
- Perform external cryptography and implementation review.
- Keep mainnet disabled by default in user-facing flows until the operator deliberately enables it.

## Verification Commands

```bash
./scripts/quality.sh
./scripts/quality.sh --full
```

Use [Run Guide](RUN_GUIDE.md) for end-to-end TSS, HTSS, reshare, and recovery dry runs.
