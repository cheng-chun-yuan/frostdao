# FrostDAO - Hierarchical Threshold Signatures for Organizations

## Fork from -> (https://github.com/nickfarrow/yushan)

> **🏆 Winner at [BTC++ Taipei 2025](https://devpost.com/software/frostdao)**
> - **1st Place Overall**
> - **Best Use of Cryptography**

FrostDAO implements FROST threshold signatures with **Hierarchical Threshold Secret Sharing (HTSS)** for Bitcoin Taproot. It enables organizational hierarchies where `t-of-n` parties must cooperate to sign, with rank-based access control.

**No single party ever knows the full private key.**

---

## Table of Contents

1. [Features](#features)
2. [Installation](#installation)
3. [Quick Start](#quick-start)
4. [Single-Signer Wallet](#single-signer-wallet)
5. [Threshold Signatures (DKG)](#threshold-signatures-dkg)
6. [Hierarchical TSS (HTSS)](#hierarchical-tss-htss)
7. [Bitcoin Transactions](#bitcoin-transactions)
8. [CLI Reference](#cli-reference)
9. [Web UI](#web-ui)
10. [Architecture](#architecture)
11. [Use Cases](#use-cases)
12. [Security](#security)

---

## Features

| Feature | Description |
|---------|-------------|
| **Single-Signer Wallet** | BIP340 Schnorr signatures for Bitcoin Taproot |
| **Threshold Signatures** | FROST-based t-of-n multisig without trusted dealer |
| **Hierarchical TSS** | Rank-based signing authority (CEO must approve) |
| **Bitcoin Integration** | Taproot addresses, transaction building, broadcasting |
| **On-Chain Transactions** | UTXO fetching, signing, and broadcasting via mempool.space |

---

## Installation

```bash
# Clone and install
git clone https://github.com/anthropics/frostdao.git
cd frostdao
cargo install --path .

# Verify
frostdao --help
```

---

## Quick Start

### Single-Signer (Simple Wallet)

```bash
# 1. Generate wallet
frostdao btc-keygen

# 2. Get testnet address
frostdao btc-address-testnet
# Output: tb1pqjuvav27udjjufh8z8pt6873myjlrgjmelfx62f7xhkl4rrrw5vsw2r2um

# 3. Fund via faucet: https://bitcoinfaucet.uo1.net/

# 4. Check balance
frostdao btc-balance

# 5. Send Bitcoin
frostdao btc-send --to <recipient> --amount 10000
```

### Threshold Wallet (2-of-3)

```bash
# Each party runs Round 1
frostdao keygen-round1 --threshold 2 --n-parties 3 --my-index 1
frostdao keygen-round1 --threshold 2 --n-parties 3 --my-index 2
frostdao keygen-round1 --threshold 2 --n-parties 3 --my-index 3

# Exchange commitments, run Round 2
frostdao keygen-round2 --data '<commitments_json>'

# Finalize
frostdao keygen-finalize --data '<shares_json>'

# Get group address
frostdao dkg-address
```

---

## Single-Signer Wallet

Standard BIP340 Schnorr signatures for Bitcoin Taproot.

### Key Management

```bash
frostdao btc-keygen              # Generate new keypair
frostdao btc-import-key --secret <hex>  # Import existing
frostdao btc-pubkey              # Show public key
```

### Addresses

```bash
frostdao btc-address             # Mainnet (bc1p...)
frostdao btc-address-testnet     # Testnet (tb1p...)
frostdao btc-address-signet      # Signet
```

### Signing

```bash
frostdao btc-sign --message "Hello"              # Sign message
frostdao btc-sign-taproot --sighash <hex>        # Sign sighash
frostdao btc-verify --signature <sig> --public-key <pk> --message "Hello"
```

---

## Threshold Signatures (DKG)

Distributed Key Generation creates a shared wallet where `t-of-n` parties must cooperate to sign.

### DKG Workflow

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   Party 1   │    │   Party 2   │    │   Party 3   │
└──────┬──────┘    └──────┬──────┘    └──────┬──────┘
       │                  │                  │
       ▼                  ▼                  ▼
   Round 1            Round 1            Round 1
   (Generate)         (Generate)         (Generate)
       │                  │                  │
       └────────┬─────────┴─────────┬────────┘
                │  Share Commitments │
                ▼                    ▼
            Round 2              Round 2
            (Exchange)           (Exchange)
                │                    │
                └─────────┬──────────┘
                          │
                    ┌─────▼─────┐
                    │  Finalize │
                    └─────┬─────┘
                          │
              ┌───────────┴───────────┐
              │                       │
         Group Public Key      Secret Shares
         (Bitcoin Address)     (Each party)
```

### Commands

```bash
# Round 1: Generate polynomial and commitments
frostdao keygen-round1 --threshold 2 --n-parties 3 --my-index 1

# Round 2: Exchange encrypted shares
frostdao keygen-round2 --data '{"commitments": [...]}'

# Finalize: Derive group key and personal share
frostdao keygen-finalize --data '{"shares": [...]}'

# Get group address
frostdao dkg-address
```

### Threshold Signing

```bash
# 1. Each signer generates nonce
frostdao generate-nonce --session "tx-001"

# 2. Create signature shares
frostdao sign --session "tx-001" --message "<data>" --data '<nonces>'

# 3. Combine into final signature
frostdao combine --data '<signature_shares>'
```

---

## Hierarchical TSS (HTSS)

HTSS adds rank-based access control. Higher ranks (lower numbers) have more authority.

### Rank System

| Rank | Authority | Example Role |
|------|-----------|--------------|
| 0 | Highest | CEO, Board |
| 1 | High | C-Suite, Directors |
| 2 | Medium | Managers |
| 3+ | Lower | Staff |

### Signing Rule

For signers with sorted ranks `[r₀, r₁, ..., r_{t-1}]`:

```
Valid if: rᵢ ≤ i for all positions i
```

### Example: 3-of-4 with HTSS

```bash
# CEO (rank 0)
frostdao keygen-round1 --threshold 3 --n-parties 4 --my-index 1 --rank 0 --hierarchical

# CFO (rank 1)
frostdao keygen-round1 --threshold 3 --n-parties 4 --my-index 2 --rank 1 --hierarchical

# COO (rank 1)
frostdao keygen-round1 --threshold 3 --n-parties 4 --my-index 3 --rank 1 --hierarchical

# Manager (rank 2)
frostdao keygen-round1 --threshold 3 --n-parties 4 --my-index 4 --rank 2 --hierarchical
```

### Valid Combinations

| Signers | Ranks | Valid? | Reason |
|---------|-------|--------|--------|
| CEO + CFO + COO | [0,1,1] | Yes | 0≤0, 1≤1, 1≤2 |
| CEO + CFO + Manager | [0,1,2] | Yes | 0≤0, 1≤1, 2≤2 |
| CFO + COO + Manager | [1,1,2] | No | 1>0 fails |

**CEO must always be involved!**

---

## Bitcoin Transactions

### Check Balance

```bash
frostdao btc-balance
```

Output:
```
Network: testnet
Address: tb1pqjuvav27...

Total UTXOs: 1
Total Balance: 100000 sats (0.00100000 BTC)
```

### Send Transaction

```bash
frostdao btc-send \
  --to tb1p3e44guscrytuum9q36tlx5kez9zvdheuwxlq9k9y4kud3hyckhtq63fz34 \
  --amount 10000 \
  --fee-rate 2  # optional
```

### Transaction Flow

```
┌──────────────┐
│ Fetch UTXOs  │ ─── mempool.space API
└──────┬───────┘
       ▼
┌──────────────┐
│ Build TX     │ ─── Taproot (P2TR)
└──────┬───────┘
       ▼
┌──────────────┐
│ Sign         │ ─── BIP340 Schnorr
└──────┬───────┘
       ▼
┌──────────────┐
│ Broadcast    │ ─── mempool.space API
└──────────────┘
```

---

## CLI Reference

### Key Management

| Command | Description |
|---------|-------------|
| `btc-keygen` | Generate BIP340 keypair |
| `btc-import-key --secret <hex>` | Import secret key |
| `btc-pubkey` | Show public key |

### Addresses

| Command | Description |
|---------|-------------|
| `btc-address` | Mainnet address |
| `btc-address-testnet` | Testnet address |
| `btc-address-signet` | Signet address |
| `dkg-address` | DKG group address |
| `dkg-balance` | DKG group balance |

### Transactions

| Command | Description |
|---------|-------------|
| `btc-balance` | Check single-signer balance |
| `dkg-balance` | Check DKG group balance |
| `btc-send --to <addr> --amount <sats>` | Send on testnet |
| `btc-send-signet --to <addr> --amount <sats>` | Send on signet |

### Signing

| Command | Description |
|---------|-------------|
| `btc-sign --message <text>` | Sign message |
| `btc-sign-hex --message <hex>` | Sign hex data |
| `btc-sign-taproot --sighash <hex>` | Sign sighash |
| `btc-verify` | Verify signature |

### DKG

| Command | Description |
|---------|-------------|
| `keygen-round1` | Generate commitments |
| `keygen-round2 --data <json>` | Exchange shares |
| `keygen-finalize --data <json>` | Derive keys |

### Threshold Signing

| Command | Description |
|---------|-------------|
| `generate-nonce --session <id>` | Generate nonce |
| `sign --session <id> --message <msg> --data <json>` | Create share |
| `combine --data <json>` | Combine signatures |
| `verify` | Verify signature |

---

## Web UI

Open `frontend/btc-send.html` for a visual interface:

```bash
open frontend/btc-send.html
```

Features:
- **Two tabs**: Single Signer & Threshold (DKG)
- **Balance display** with auto-refresh
- **Fund button** opens faucet
- **Amount input** with quick-select (1k, 5k, 10k, MAX)
- **Command generator** for CLI

---

## Architecture

### Project Structure

```
frostdao/
├── src/                      # Rust source code
│   ├── main.rs               # CLI entry point
│   ├── lib.rs                # Library exports
│   ├── keygen.rs             # DKG implementation
│   ├── signing.rs            # Threshold signing
│   ├── birkhoff.rs           # HTSS Birkhoff interpolation
│   ├── bitcoin_schnorr.rs    # BIP340 & addresses
│   ├── bitcoin_tx.rs         # Transactions & broadcasting
│   ├── storage.rs            # Key storage
│   └── wasm.rs               # WebAssembly bindings
├── frontend/                 # Web UI
│   ├── btc-send.html         # Send BTC interface
│   ├── index.html            # DKG demo
│   └── pkg/                  # WASM bindings
├── docs/                     # Documentation
│   ├── CLI.md                # CLI reference
│   └── BITCOIN_GUIDE.md      # Bitcoin guide
├── .frost_state/             # Key storage (gitignored)
├── README.md
└── Cargo.toml
```

### Storage

Keys stored in `.frost_state/` (gitignored):

| File | Contents |
|------|----------|
| `bitcoin_keypair.json` | Single-signer keys |
| `shared_key.bin` | DKG group public key |
| `paired_secret_share.bin` | Your DKG share |
| `htss_metadata.json` | HTSS config |

---

## Use Cases

### 1. Corporate Treasury

```
Threshold: 3-of-5
Ranks:
  CEO (0), CFO (1), Treasurer (1), Director (2), Accountant (2)

Policy: CEO must always approve treasury movements
```

### 2. DAO Multi-Sig

```
Threshold: 4-of-7
Ranks:
  Core Devs (0), Community Leads (1), Advisors (2), Reps (3)

Policy: At least one core dev required
```

### 3. Family Trust

```
Threshold: 2-of-4
Ranks:
  Parents (0), Adult Children (1)

Policy: Children cannot act alone
```

### 4. Exchange Hot Wallet

```
Threshold: 3-of-6
Ranks:
  Security Officer (0), CTO (0), Senior DevOps (1), Engineers (2)

Policy: Security/CTO approval required for all withdrawals
```

---

## Security

### Key Protection

- Keys stored in `.frost_state/` (gitignored)
- **Never commit keys to version control**
- Consider encryption at rest for production

### Threshold Security

- Choose `t > n/2` to prevent minority attacks
- HTSS adds hierarchy but same threshold applies

### Nonce Security

- **Never reuse nonces** - causes key leakage
- Session IDs must be unique per signature

### Production Recommendations

- Security audit before production use
- Secure communication channels (TLS)
- Hardware security modules (HSM)
- Comprehensive testing

---

## References

- [FROST Paper](https://eprint.iacr.org/2020/852)
- [Hierarchical TSS](https://www.cs.umd.edu/~gasMDa/htss.pdf)
- [BIP340 - Schnorr Signatures](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki)
- [BIP341 - Taproot](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki)
- [schnorr_fun Library](https://github.com/LLFourn/secp256kfun)

---

## Acknowledgments

- **[Frostsnap Team](https://frostsnap.com/)** - `schnorr_fun` and `secp256kfun` libraries
- **[Nick Farrow](https://github.com/nickfarrow)** - Original Yushan workshop codebase

---

## License

MIT
