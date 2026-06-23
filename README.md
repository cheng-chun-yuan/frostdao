# FrostDAO - Hierarchical Threshold Signatures for Bitcoin

> **Fork from [nickfarrow/yushan](https://github.com/nickfarrow/yushan)**

> **🏆 Winner at [BTC++ Taipei 2025](https://devpost.com/software/frostdao)**
> - **1st Place Overall**
> - **Best Use of Cryptography**

**No single party ever knows the full private key.**

FrostDAO implements FROST threshold signatures with **Hierarchical TSS (HTSS)** for Bitcoin Taproot. Create `t-of-n` multisig wallets with optional rank-based access control.

## Features

- **Threshold Signatures** - FROST-based t-of-n without trusted dealer
- **Hierarchical TSS** - Rank-based signing (CEO must approve)
- **HD Derivation** - BIP-32/44 addresses from one DKG wallet
- **Resharing** - Refresh shares without changing address
- **Share Recovery** - Reconstruct lost shares from t parties
- **BIP-39 Backup** - 24-word mnemonic for share recovery

## Installation

```bash
git clone https://github.com/anthropics/frostdao.git
cd frostdao
cargo install --path .
```

## Quick Start

### Terminal UI (Recommended)

```bash
frostdao tui
```

### CLI: Create 2-of-3 Wallet

```bash
# Each party runs Round 1
frostdao keygen-round1 --name treasury --threshold 2 --n-parties 3 --my-index 1

# Exchange outputs, run Round 2
frostdao keygen-round2 --name treasury --data '<all_round1_outputs>' --encrypt

# Finalize
frostdao keygen-finalize --name treasury --data '<round2_outputs_for_this_party>'

# Check address and balance
frostdao dkg-address --name treasury
frostdao dkg-balance --name treasury
```

### Send Transaction

```bash
# TUI handles multi-party signing automatically
frostdao tui
# Navigate to wallet → Send Transaction
```

## Documentation

| Document | Description |
|----------|-------------|
| [Showcase](docs/SHOWCASE.md) | Demo path, code tour, and verification map |
| [Run Guide](docs/RUN_GUIDE.md) | Current build, test, TUI, CLI, TSS, HTSS, reshare, recovery, and Nostr usage |
| [CLI Reference](docs/CLI.md) | All CLI commands |
| [TUI Keymap](docs/TUI_KEYMAP.md) | Unified terminal keyboard shortcuts |
| [Backup Guide](docs/BACKUP.md) | Share mnemonic, manifest, verification, and storage guidance |
| [Protocols](docs/PROTOCOLS.md) | Current TSS, HTSS, DKG, signing, derivation, reshare, and recovery overview |
| [Nostr Protocol](docs/NOSTR_PROTOCOL.md) | Relay message protocol for DKG, signing, and resharing |
| [Miniscript](docs/MINISCRIPT.md) | Optional Taproot policy and agent payment draft support |
| [ROAST And Hardware](docs/ROAST_HARDWARE.md) | Robust coordinator and hardware/PSBT integration direction |
| [Security Model](docs/SECURITY_MODEL.md) | Security invariants, threat model, and mainnet cautions |
| [Production Readiness](docs/PRODUCTION_READINESS.md) | Release gates, runbooks, and mainnet criteria |
| [User Flow](docs/USER_FLOW.md) | Multi-device UX flow for TSS, HTSS, signing, reshare, and recovery |

## Architecture

```
frostdao/
├── src/
│   ├── protocol/     # DKG, signing, reshare, recovery
│   ├── crypto/       # Birkhoff, HD, helpers
│   ├── btc/          # Bitcoin, Schnorr, addresses
│   └── tui/          # Terminal UI
├── docs/             # Documentation
└── tests/            # Integration tests
```

## Quality

```bash
./scripts/doctor.sh
./scripts/quality.sh
./scripts/quality.sh --full
```

## Security

- Keys stored in `~/.frostdao/` (not in repo)
- Choose `t > n/2` to prevent minority attacks
- **Never reuse nonces** - causes key leakage
- Security audit recommended before production

## References

- [FROST Paper](https://eprint.iacr.org/2020/852)
- [BIP-32 - HD Wallets](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki)
- [BIP-39 - Mnemonic](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki)
- [BIP-44 - Multi-Account](https://github.com/bitcoin/bips/blob/master/bip-0044.mediawiki)
- [BIP-340 - Schnorr](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki)
- [BIP-341 - Taproot](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki)

## License

MIT
