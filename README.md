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
7. [Resharing (Proactive Security)](#resharing-proactive-security)
8. [Share Recovery](#share-recovery)
9. [Bitcoin Transactions](#bitcoin-transactions)
10. [DKG Threshold Transactions](#dkg-threshold-transactions)
11. [Terminal UI (TUI)](#terminal-ui-tui)
12. [CLI Reference](#cli-reference)
13. [Web UI](#web-ui)
14. [Architecture](#architecture)
15. [Use Cases](#use-cases)
16. [Security](#security)

---

## Features

| Feature | Description |
|---------|-------------|
| **Single-Signer Wallet** | BIP340 Schnorr signatures for Bitcoin Taproot |
| **Threshold Signatures** | FROST-based t-of-n multisig without trusted dealer |
| **Hierarchical TSS** | Rank-based signing authority (CEO must approve) |
| **Resharing** | Proactive secret sharing - refresh shares without changing address |
| **Share Recovery** | Reconstruct lost party's share from t other parties |
| **DKG Transactions** | Multi-party threshold signing for Bitcoin transactions |
| **Bitcoin Integration** | Taproot addresses, transaction building, broadcasting |
| **On-Chain Transactions** | UTXO fetching, signing, and broadcasting via mempool.space |
| **Terminal UI** | Interactive TUI for wallet management and balance checking |

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
# Each party runs Round 1 (use same --name for the wallet)
frostdao keygen-round1 --name treasury --threshold 2 --n-parties 3 --my-index 1
frostdao keygen-round1 --name treasury --threshold 2 --n-parties 3 --my-index 2
frostdao keygen-round1 --name treasury --threshold 2 --n-parties 3 --my-index 3

# Exchange commitments, run Round 2
frostdao keygen-round2 --name treasury --data '<commitments_json>'

# Finalize
frostdao keygen-finalize --name treasury --data '<shares_json>'

# Get group address and balance
frostdao dkg-address --name treasury
frostdao dkg-balance --name treasury
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
frostdao keygen-round1 --name my_wallet --threshold 2 --n-parties 3 --my-index 1

# Round 2: Exchange encrypted shares
frostdao keygen-round2 --name my_wallet --data '{"commitments": [...]}'

# Finalize: Derive group key and personal share
frostdao keygen-finalize --name my_wallet --data '{"shares": [...]}'

# Get group address and balance
frostdao dkg-address --name my_wallet
frostdao dkg-balance --name my_wallet
```

**Note:** The `--name` parameter creates a folder `.frost_state/<name>/` to store all wallet data. If a wallet with the same name exists, you'll be prompted to confirm replacement.

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
frostdao keygen-round1 --name corp_treasury --threshold 3 --n-parties 4 --my-index 1 --rank 0 --hierarchical

# CFO (rank 1)
frostdao keygen-round1 --name corp_treasury --threshold 3 --n-parties 4 --my-index 2 --rank 1 --hierarchical

# COO (rank 1)
frostdao keygen-round1 --name corp_treasury --threshold 3 --n-parties 4 --my-index 3 --rank 1 --hierarchical

# Manager (rank 2)
frostdao keygen-round1 --name corp_treasury --threshold 3 --n-parties 4 --my-index 4 --rank 2 --hierarchical
```

### Valid Combinations

| Signers | Ranks | Valid? | Reason |
|---------|-------|--------|--------|
| CEO + CFO + COO | [0,1,1] | Yes | 0≤0, 1≤1, 1≤2 |
| CEO + CFO + Manager | [0,1,2] | Yes | 0≤0, 1≤1, 2≤2 |
| CFO + COO + Manager | [1,1,2] | No | 1>0 fails |

**CEO must always be involved!**

---

## Resharing (Proactive Security)

Resharing allows you to refresh secret shares while keeping the same group public key and Bitcoin address. This is useful for:

- **Proactive security**: Invalidate potentially compromised shares
- **Party replacement**: Add/remove parties without changing the address
- **Threshold changes**: Modify the signing threshold

### How Resharing Works

```
Old Configuration (2-of-3)          New Configuration (2-of-3)
┌─────────┐ ┌─────────┐ ┌─────────┐     ┌─────────┐ ┌─────────┐ ┌─────────┐
│ Party 1 │ │ Party 2 │ │ Party 3 │     │ Party A │ │ Party B │ │ Party C │
│ Share_1 │ │ Share_2 │ │ Share_3 │     │ Share_A │ │ Share_B │ │ Share_C │
└────┬────┘ └────┬────┘ └────┬────┘     └────┬────┘ └────┬────┘ └────┬────┘
     │           │           │               │           │           │
     └───────────┼───────────┘               └───────────┼───────────┘
                 │                                       │
         ┌───────▼───────┐                       ┌───────▼───────┐
         │ Group Secret  │  ═══════════════════▶ │ Group Secret  │
         │     (same)    │                       │     (same)    │
         └───────────────┘                       └───────────────┘
                 │                                       │
         ┌───────▼───────┐                       ┌───────▼───────┐
         │ Public Key    │  ═══════════════════▶ │ Public Key    │
         │ (same)        │                       │ (same)        │
         └───────────────┘                       └───────────────┘
```

### Commands

```bash
# Step 1: Old parties generate sub-shares (need at least t old parties)
frostdao reshare-round1 \
  --source treasury \
  --new-threshold 2 \
  --new-n-parties 3 \
  --my-index 1

# Step 2: New parties combine sub-shares
frostdao reshare-finalize \
  --source treasury \
  --target treasury_v2 \
  --my-index 1 \
  --data '<round1_outputs_json>'
```

### Example: Refresh a 2-of-3 Wallet

```bash
# Party 1 (old) generates sub-shares
P1_SUB=$(frostdao reshare-round1 --source wallet --new-threshold 2 --new-n-parties 3 --my-index 1)

# Party 2 (old) generates sub-shares
P2_SUB=$(frostdao reshare-round1 --source wallet --new-threshold 2 --new-n-parties 3 --my-index 2)

# New party 1 combines (needs at least 2 old party outputs)
frostdao reshare-finalize \
  --source wallet \
  --target wallet_refreshed \
  --my-index 1 \
  --data "$P1_SUB $P2_SUB"

# Verify: addresses should match!
frostdao dkg-address --name wallet
frostdao dkg-address --name wallet_refreshed
```

### Security Note

- Old shares become invalid after resharing
- Delete old wallet folders once all parties have reshared
- The same Bitcoin address remains valid for receiving funds

---

## Share Recovery

If a party loses their share, it can be reconstructed from `t` other parties without changing the group public key or Bitcoin address.

### How Recovery Works

```
Helper Party 1  ──→  sub_share_{1→lost}  ─┐
Helper Party 2  ──→  sub_share_{2→lost}  ─┼─→  Lost Party combines  ──→  Recovered Share
Helper Party 3  ──→  sub_share_{3→lost}  ─┘

                     (need t helpers)           s_lost = Σ λᵢ * sub_shareᵢ
```

Each helper evaluates their share polynomial at the lost party's index using **Lagrange interpolation** (or **Birkhoff interpolation** for HTSS with mixed ranks).

### Commands

```bash
# Step 1: Each helper party generates sub-share for the lost party
frostdao recover-round1 \
  --name treasury \
  --lost-index 3

# Step 2: Lost party combines sub-shares to recover
frostdao recover-finalize \
  --source treasury \
  --target treasury_recovered \
  --my-index 3 \
  --data '<sub-shares JSON>'
```

### Example: Recover Party 3 in a 2-of-3 Wallet

```bash
# Party 1 (helper) generates sub-share for party 3
P1_SUB=$(frostdao recover-round1 --name wallet --lost-index 3)

# Party 2 (helper) generates sub-share for party 3
P2_SUB=$(frostdao recover-round1 --name wallet --lost-index 3)

# Party 3 (lost) combines sub-shares to recover
frostdao recover-finalize \
  --source wallet \
  --target wallet_recovered \
  --my-index 3 \
  --data "$P1_SUB $P2_SUB"

# Verify: addresses should match!
frostdao dkg-address --name wallet
frostdao dkg-address --name wallet_recovered
```

### HTSS Recovery with Birkhoff

For hierarchical wallets with mixed ranks, recovery uses **Birkhoff interpolation** which accounts for rank derivatives:

```bash
# HTSS wallet: CEO(rank 0), CFO(rank 1), COO(rank 1)
# If CFO loses share, CEO and COO can help recover

# CEO generates sub-share (rank 0 → evaluates value)
CEO_SUB=$(frostdao recover-round1 --name corp --lost-index 2)

# COO generates sub-share (rank 1 → evaluates derivative)
COO_SUB=$(frostdao recover-round1 --name corp --lost-index 2)

# CFO recovers (needs rank 1 at their index)
frostdao recover-finalize \
  --source corp \
  --target corp_recovered \
  --my-index 2 \
  --rank 1 \
  --hierarchical \
  --data "$CEO_SUB $COO_SUB"
```

---

## Terminal UI (TUI)

FrostDAO includes an interactive terminal UI for wallet management.

### Running the TUI

```bash
frostdao tui
```

### Screenshot

```
┌──────────────────────────────────────────────────────────────────┐
│ FrostDAO - DKG Wallet Manager                                    │
├────────────────────────────┬─────────────────────────────────────┤
│ Wallets                    │ Details                             │
│                            │                                     │
│ >> treasury (2-of-3 TSS)   │ Name: treasury                      │
│    backup (3-of-5 HTSS) $  │                                     │
│    test (1-of-1 TSS)       │ Threshold: 2-of-3                   │
│                            │ Mode: Standard (TSS)                │
│                            │                                     │
│                            │ Address:                            │
│                            │ tb1p7sray...zpvhs4x7ehm             │
│                            │                                     │
│                            │ Balance: 50000 sats                 │
│                            │          (0.00050000 BTC)           │
│                            │ UTXOs: 2                            │
├────────────────────────────┴─────────────────────────────────────┤
│ Help: ↑/↓: Navigate | Enter/r: Refresh Balance | R: Reload | q  │
└──────────────────────────────────────────────────────────────────┘
```

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `Enter` / `r` | Refresh balance for selected wallet |
| `R` | Reload wallet list |
| `q` / `Esc` | Quit |

### Features

- View all DKG wallets
- Check real-time balances (via mempool.space API)
- See threshold configuration and mode (TSS/HTSS)
- Navigate with vim-style keys

See [docs/TUI.md](docs/TUI.md) for full documentation.

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

## DKG Threshold Transactions

Send Bitcoin from a DKG threshold wallet. Requires `t` parties to cooperate for signing.

### Transaction Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         DKG Threshold Transaction                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  Coordinator              Signer 2                Signer 3               │
│       │                       │                       │                  │
│  1. dkg-build-tx              │                       │                  │
│       │ (sighash, session)    │                       │                  │
│       ├───────────────────────┼───────────────────────┤ share sighash    │
│       ▼                       ▼                       ▼                  │
│  2. dkg-nonce             dkg-nonce               dkg-nonce              │
│       │                       │                       │                  │
│       └───────────────────────┼───────────────────────┘ exchange nonces  │
│                               ▼                                          │
│  3. dkg-sign              dkg-sign                dkg-sign               │
│       │                       │                       │                  │
│       └───────────────────────┼───────────────────────┘ exchange shares  │
│                               ▼                                          │
│  4. dkg-broadcast             │                       │                  │
│       │                       │                       │                  │
│       ▼                       │                       │                  │
│     txid                      │                       │                  │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Commands

```bash
# Step 1: Coordinator builds unsigned transaction
frostdao dkg-build-tx \
  --name treasury \
  --to tb1p... \
  --amount 10000 \
  --fee-rate 2

# Output: session_id, sighash, unsigned_tx

# Step 2: Each signer generates nonce (share session_id)
frostdao dkg-nonce --name treasury --session <session_id>

# Step 3: Each signer creates signature share (need all nonces)
frostdao dkg-sign \
  --name treasury \
  --session <session_id> \
  --sighash <sighash_hex> \
  --data '<nonces_json>'

# Step 4: Coordinator combines shares and broadcasts
frostdao dkg-broadcast \
  --name treasury \
  --unsigned-tx <tx_hex> \
  --data '<signature_shares_json>'
```

### Example: 2-of-3 Threshold Send

```bash
# 1. Party 1 (coordinator) builds the transaction
BUILD_OUT=$(frostdao dkg-build-tx --name treasury --to tb1p... --amount 10000)
SESSION=$(echo $BUILD_OUT | jq -r '.session_id')
SIGHASH=$(echo $BUILD_OUT | jq -r '.sighash')
UNSIGNED_TX=$(echo $BUILD_OUT | jq -r '.unsigned_tx')

# 2. Parties 1 and 2 generate nonces (share session_id)
NONCE1=$(frostdao dkg-nonce --name treasury --session $SESSION)
NONCE2=$(frostdao dkg-nonce --name treasury --session $SESSION)

# 3. Parties 1 and 2 create signature shares (exchange nonces)
NONCES="[$NONCE1, $NONCE2]"
SHARE1=$(frostdao dkg-sign --name treasury --session $SESSION --sighash $SIGHASH --data "$NONCES")
SHARE2=$(frostdao dkg-sign --name treasury --session $SESSION --sighash $SIGHASH --data "$NONCES")

# 4. Coordinator broadcasts
SHARES="[$SHARE1, $SHARE2]"
frostdao dkg-broadcast --name treasury --unsigned-tx $UNSIGNED_TX --data "$SHARES"
# Output: txid
```

### Taproot Parity Handling

The DKG signing automatically handles Taproot key tweaking and parity. When the tweaked public key has odd Y coordinate, the signing logic:
1. Negates secret shares during signing
2. Adjusts the tweak contribution in the final signature

This ensures ~100% signature success rate (previously ~50% when parity was ignored).

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
| `dkg-address --name <wallet>` | DKG group address |
| `dkg-balance --name <wallet>` | DKG group balance |

### Transactions

| Command | Description |
|---------|-------------|
| `btc-balance` | Check single-signer balance |
| `dkg-balance --name <wallet>` | Check DKG group balance |
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
| `keygen-round1 --name <wallet> ...` | Generate commitments |
| `keygen-round2 --name <wallet> --data <json>` | Exchange shares |
| `keygen-finalize --name <wallet> --data <json>` | Derive keys + group_info.json |

### Threshold Signing

| Command | Description |
|---------|-------------|
| `generate-nonce --session <id>` | Generate nonce |
| `sign --session <id> --message <msg> --data <json>` | Create share |
| `combine --data <json>` | Combine signatures |
| `verify` | Verify signature |

### Resharing

| Command | Description |
|---------|-------------|
| `reshare-round1 --source <wallet> ...` | Generate sub-shares for resharing |
| `reshare-finalize --source <old> --target <new> ...` | Combine sub-shares into new wallet |

### Share Recovery

| Command | Description |
|---------|-------------|
| `recover-round1 --name <wallet> --lost-index <i>` | Helper generates sub-share for lost party |
| `recover-finalize --source <wallet> --target <new> ...` | Lost party combines sub-shares to recover |

### DKG Transactions

| Command | Description |
|---------|-------------|
| `dkg-build-tx --name <wallet> --to <addr> --amount <sats>` | Build unsigned transaction |
| `dkg-nonce --name <wallet> --session <id>` | Generate signing nonce |
| `dkg-sign --name <wallet> --session <id> --sighash <hex> --data <json>` | Create signature share |
| `dkg-broadcast --name <wallet> --unsigned-tx <hex> --data <json>` | Combine and broadcast |

### Wallet Management

| Command | Description |
|---------|-------------|
| `dkg-list` | List all DKG wallets |
| `dkg-info --name <wallet>` | Regenerate group_info.json |
| `tui` | Launch interactive Terminal UI |

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
│   ├── reshare.rs            # Resharing protocol
│   ├── recovery.rs           # Share recovery protocol
│   ├── dkg_tx.rs             # DKG threshold transactions
│   ├── crypto_helpers.rs     # Shared cryptographic utilities
│   ├── birkhoff.rs           # HTSS Birkhoff interpolation
│   ├── bitcoin_schnorr.rs    # BIP340 & addresses
│   ├── bitcoin_tx.rs         # Single-signer transactions
│   ├── storage.rs            # Key storage
│   ├── tui.rs                # Terminal UI entry
│   ├── tui/                  # Terminal UI components
│   └── wasm.rs               # WebAssembly bindings
├── tests/                    # Integration tests
│   ├── reshare_tests.rs      # Resharing protocol tests
│   ├── dkg_flow_tests.rs     # DKG flow tests
│   └── signing_tests.rs      # Signing tests
├── frontend/                 # Web UI
│   ├── btc-send.html         # Send BTC interface
│   ├── index.html            # DKG demo
│   └── pkg/                  # WASM bindings
├── docs/                     # Documentation
│   ├── CLI.md                # CLI reference
│   ├── TUI.md                # Terminal UI guide
│   ├── BITCOIN_GUIDE.md      # Bitcoin guide
│   └── demo-reshare.sh       # Resharing demo script
├── .frost_state/             # Key storage (gitignored)
├── README.md
└── Cargo.toml
```

### Core Modules

| Module | Purpose |
|--------|---------|
| `crypto_helpers.rs` | Shared utilities: tagged_hash, Lagrange coefficients, taproot parity |
| `birkhoff.rs` | Birkhoff interpolation for HTSS (rank-based recovery) |
| `keygen.rs` | Distributed Key Generation (DKG) protocol |
| `signing.rs` | FROST threshold signing |
| `reshare.rs` | Proactive share refresh without changing address |
| `recovery.rs` | Recover lost party's share from t other parties |
| `dkg_tx.rs` | Multi-party Bitcoin transactions with threshold signing |
| `bitcoin_schnorr.rs` | BIP340 Schnorr signatures and Taproot addresses |
| `bitcoin_tx.rs` | Single-signer transaction building and broadcasting |

### Storage

Keys stored in `.frost_state/` (gitignored):

```
.frost_state/
├── <wallet_name>/             # Each DKG wallet in its own folder
│   ├── group_info.json        # Public info (shareable, parties by rank)
│   ├── shared_key.bin         # Group public key
│   ├── paired_secret_share.bin  # Your secret share
│   └── htss_metadata.json     # HTSS config
└── bitcoin_keypair.json       # Single-signer keys
```

The `group_info.json` contains public wallet info sorted by rank - useful for coordination.

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
