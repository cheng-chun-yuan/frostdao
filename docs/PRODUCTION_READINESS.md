# Production Readiness

This document defines the release bar for FrostDAO multi-device use. The current codebase has protocol foundations, deterministic transport tests, CLI flows, TUI screens, and documentation. Treat live relay orchestration as production-bound only after the gates below pass.

## Quality Gate

Run before every release candidate:

```bash
./scripts/quality.sh --full
```

Run when touching browser or WASM behavior:

```bash
./scripts/quality.sh --with-wasm
```

The full gate checks formatting, strict Clippy, all tests, release build, and rustdoc warnings.

## Security Invariants

- Private shares, DKG round 2 shares, signing nonces, signing shares, reshare sub-shares, and recovery sub-shares are never published in plaintext.
- Nonces are generated for one signing session and are never reused.
- Nostr messages are accepted only for the active room and intended party.
- Nostr messages include `message_id`, `created_at`, and optional `expires_at`; receivers keep a replay cache.
- Every party verifies their written share mnemonic with `dkg-verify-mnemonic`.
- TSS signer sets satisfy `t-of-n`.
- HTSS signer sets satisfy the Birkhoff rank rule: sorted ranks must have `rank[i] <= i`.
- Reshare preserves the group secret when the goal is device rotation without address rotation.
- Recovery reconstructs one missing party share, not the full wallet secret.

## Release Runbook

1. Start from a clean test profile and run `./scripts/quality.sh --full`.
2. Create a 2-of-3 TSS wallet with CLI commands from [User Flow](USER_FLOW.md).
3. Derive at least two addresses and verify they remain deterministic across devices.
4. Generate and verify a backup mnemonic for each party.
5. Build and sign a testnet transaction using an explicit session ID.
6. Reshare the wallet to a new party set and verify the address fingerprint is unchanged.
7. Recover one lost party share and verify the recovered wallet public key/address.
8. Repeat steps 2-7 with HTSS ranks and verify invalid rank sets are rejected.
9. Run the same ceremonies through the Nostr message envelope and relay transport.
10. Record relay failures, duplicate messages, expired messages, and wrong-recipient messages as rejected.

## Required Before Mainnet

- Wire the hardened Nostr envelope into the full interactive TUI room runtime.
- Add relay integration tests against at least one local relay and one public relay.
- Add persisted replay-cache behavior for long-running ceremonies.
- Add explicit transaction review for amount, fee, network, source path, destination, and sighash fingerprint.
- Add structured audit logs that exclude secret material.
- Complete an external cryptography/security review for FROST, HTSS, NIP-44 use, reshare, and recovery.
- Default to testnet/signet until the user deliberately enables mainnet.

## Operational Defaults

- Default message TTL: 1 hour.
- Maximum accepted future clock skew: 5 minutes.
- Default relay: `wss://relay.damus.io`.
- Recommended production relay setup: at least two relays, one controlled by the operator and one independent fallback.
- Recommended threshold: choose `t > n / 2` unless the organization has a documented reason not to.

## Documentation Map

- [Showcase](SHOWCASE.md): demo and code tour.
- [Run Guide](RUN_GUIDE.md): current commands and end-to-end dry runs.
- [Backup Guide](BACKUP.md): share mnemonic, manifest, verification, and storage guidance.
- [User Flow](USER_FLOW.md): task-oriented multi-device flow.
- [Protocols](PROTOCOLS.md): TSS, HTSS, DKG, signing, derivation, reshare, and recovery overview.
- [Nostr Protocol](NOSTR_PROTOCOL.md): message envelope, kinds, and validation.
- [Security Model](SECURITY_MODEL.md): invariants, threat model, and mainnet cautions.
