# ROAST And Hardware Add-Ons

This document records the production direction for robust signing coordination and hardware-wallet interoperability.

## ROAST Recommendation

ROAST is a good fit for FrostDAO, but not as a Nostr feature.

ROAST should be an optional signing coordinator layer above FROST/HTSS and below the TUI. Nostr should remain a transport for room messages, direct encrypted payloads, replay protection, and relay delivery.

Recommended placement:

- `src/protocol/roast.rs` or `src/protocol/signing_coordinator.rs`
- use `src/protocol/dkg_tx.rs` for transaction build, nonce generation, share generation, share combination, and broadcast
- use `src/nostr/transport.rs` only as a mailbox/transport boundary
- keep TUI code as a thin driver that shows coordinator state and asks for user consent

The ROAST paper describes ROAST as a wrapper around a semi-interactive threshold signature scheme such as FROST. The wrapper relies on identifiable aborts and concurrent-session unforgeability, and uses a semi-trusted coordinator for robustness, not for custody or unforgeability.

## Coordinator Scope

The coordinator should own:

- signer set selection
- signing attempt IDs
- nonce collection per attempt
- signature share collection per attempt
- exclusion of failed or disruptive signers
- retry policy
- final combine and broadcast handoff

The coordinator must not own:

- private shares
- plaintext DKG shares
- plaintext signing nonces or signing shares in relay logs
- wallet custody decisions
- Nostr relay behavior

## First Useful ROAST Milestone

Implement a testnet/signet-only coordinator for single-input Taproot key-path spends:

1. Build a real unsigned transaction with `dkg_tx`.
2. Create a real transaction review and sighash fingerprint.
3. Select the first valid signer set.
4. Collect encrypted nonce envelopes for one attempt.
5. Collect encrypted signature share envelopes for that same attempt.
6. Combine and broadcast only after threshold-valid shares are present.
7. Record metadata-only audit events.

Do not claim ROAST robustness until bad-share identification and signer exclusion are implemented and tested. A simple coordinator state machine is still useful before full ROAST because it prevents mixing nonces or shares between signing attempts.

## Nostr Message Changes

Existing `signing_nonce_encrypted` and `signing_share_encrypted` messages should keep carrying ciphertext, but the encrypted plaintext uses stable schemas in `src/nostr/events.rs`:

- wallet
- session
- attempt ID
- signer set
- sender party
- recipient party
- sighash fingerprint
- nonce or signature share

The envelope already carries room, wallet, session, sender, recipient, timestamp, expiry, and replay protection. Keep those checks in the transport/runtime layer.

## Hardware Recommendation

Direct hardware-wallet signing is not realistic for the FrostDAO group key because normal hardware wallets do not hold a FROST/HTSS share or participate in FrostDAO DKG.

Use hardware support in this order:

1. PSBT export/import for FrostDAO-built transactions.
2. QR display for addresses, fingerprints, session IDs, and small review payloads.
3. Optional external process integration with HWI for script-path or separate single-signer keys.
4. Hardware-backed Miniscript script paths after script-path spending is implemented.

This keeps dependencies low and avoids pretending a hardware wallet can sign the threshold key-path spend directly.

## PSBT First

The smallest useful hardware-adjacent feature is single-input PSBT export/finalize. Proposed command names:

```bash
dkg-build-psbt --name treasury --to <address> --amount <sats> --network signet
dkg-finalize-psbt --name treasury --session <id> --psbt <base64-or-file> --data '<signature_shares>'
```

Initial PSBT support should reject multi-input spends until FrostDAO supports one signing session per input.

## HWI Boundary

HWI should be treated as an optional external command, not a Rust dependency.

Good uses:

- enumerate hardware devices for policy-key registration
- display a hardware-owned address
- sign PSBTs for hardware-owned script-path keys after script-path spending exists

Not supported:

- signing the FrostDAO FROST/HTSS group key directly
- storing FrostDAO private shares on normal hardware wallets

## References

- ROAST paper: <https://eprint.iacr.org/2022/550.pdf>
- Bitcoin Core external signer documentation: <https://github.com/bitcoin/bitcoin/blob/master/doc/external-signer.md>
- Bitcoin Optech HWI topic: <https://bitcoinops.org/en/topics/hwi/>
