# FrostDAO Showcase

This page is a guided map of the project for demos, reviews, and technical walkthroughs.

## What To Show First

1. **Terminal UI**: start with `frostdao tui` for the most complete interactive flow.
2. **2-of-3 DKG**: show distributed key generation with `keygen-round1`, `keygen-round2`, and `keygen-finalize`.
3. **Taproot address output**: show that all parties derive the same Bitcoin address with `dkg-address`.
4. **Resharing**: show that shares can rotate while the Bitcoin address stays the same.
5. **NIP-44 encrypted exchange**: show encrypted round 2 share delivery for safer coordination.

## Core Capabilities

| Area | What It Demonstrates | Start Here |
|------|----------------------|------------|
| CLI | Scriptable wallet creation, signing, address, balance, and recovery workflows | [CLI Reference](CLI.md) |
| TUI Keymap | Unified terminal keyboard shortcuts | [TUI Keymap](TUI_KEYMAP.md) |
| Backup | Share mnemonic, manifest, verification, and storage guidance | [Backup Guide](BACKUP.md) |
| Run Guide | Current build, test, TSS, HTSS, reshare, recovery, and Nostr usage | [Run Guide](RUN_GUIDE.md) |
| User Flow | Multi-device product flow from room join to signing and reshare | [User Flow](USER_FLOW.md) |
| Production Readiness | Release gates, runbooks, and mainnet criteria | [Production Readiness](PRODUCTION_READINESS.md) |
| Protocols | TSS, HTSS, DKG, signing, derivation, reshare, and recovery overview | [Protocols](PROTOCOLS.md) |
| Nostr | Relay-based message protocol for DKG, signing, and resharing | [Nostr Protocol](NOSTR_PROTOCOL.md) |
| Miniscript | Optional Taproot policy preview for fallback script paths | [Miniscript](MINISCRIPT.md) |
| Security | Security invariants, threat model, and mainnet cautions | [Security Model](SECURITY_MODEL.md) |

## Demo Commands

Run the fast quality gate before demos:

```bash
./scripts/quality.sh
```

Run a simple DKG demo:

```bash
./scripts/demo.sh
```

Run a resharing demo:

```bash
./scripts/demo-reshare.sh
```

Build the CLI:

```bash
./scripts/build.sh --release
```

Build the browser package:

```bash
./scripts/wasm-build.sh
```

Serve the static frontend:

```bash
./scripts/serve-frontend.sh
```

## Code Tour

| Path | Purpose |
|------|---------|
| `src/protocol/keygen.rs` | DKG round orchestration and wallet finalization |
| `src/protocol/signing.rs` | FROST signing flow |
| `src/protocol/reshare.rs` | Share refresh and migration |
| `src/protocol/recovery.rs` | Share recovery |
| `src/crypto/birkhoff.rs` | HTSS interpolation math |
| `src/crypto/hd.rs` | BIP-86 compatible threshold HD derivation |
| `src/crypto/nip44.rs` | Encrypted party-to-party payload support |
| `src/btc/transaction.rs` | Bitcoin transaction construction and broadcast |
| `src/tui/` | Terminal application state, screens, and components |
| `frontend/` | Static browser demo and WASM integration |
| `tests/` | Unit and integration coverage for crypto, DKG, signing, resharing, and encryption |

## Verification Coverage

The test suite covers:

- Birkhoff interpolation and HTSS signer validation.
- HD child derivation and threshold derivation consistency.
- BIP-39 mnemonic backup and restore flows.
- NIP-44 encryption and encrypted DKG exchange.
- CLI-driven DKG, address, wallet listing, signing, and resharing flows.
- Taproot script construction and Bitcoin signing helpers.

Use the full gate when preparing a release or recorded demo:

```bash
./scripts/quality.sh --full
```
