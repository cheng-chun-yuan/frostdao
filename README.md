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
frostdao keygen-round2 --name treasury --data '<round1_outputs>'

# Finalize
frostdao keygen-finalize --name treasury --data '<round2_outputs>'

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
| [CLI Reference](docs/CLI.md) | All CLI commands |
| [TUI Guide](docs/TUI.md) | Terminal UI usage |
| [DKG Protocol](docs/DKG.md) | Distributed key generation |
| [HTSS Guide](docs/HTSS.md) | Hierarchical threshold signatures |
| [Resharing](docs/RESHARE.md) | Proactive share refresh |
| [Recovery](docs/RECOVERY.md) | Share recovery protocol |
| [HD Derivation](docs/HD_DERIVATION.md) | BIP-32/44 key derivation |
| [Cryptographic Analysis](docs/CRYPTOGRAPHIC_ANALYSIS.md) | Security analysis |
| [Bitcoin Guide](docs/BITCOIN_GUIDE.md) | Bitcoin transaction details |

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
