# FrostDAO Terminal UI (TUI) Documentation

The FrostDAO TUI provides an interactive terminal-based interface for managing DKG wallets, including full wizards for key generation, resharing, and threshold signing.

## Quick Start

```bash
# Build and run the TUI
cargo build --release
./target/release/frostdao tui

# Or directly with cargo
cargo run -- tui
```

## Features

### 1. Wallet Management
- View all DKG wallets in `.frost_state/` directory
- See threshold configuration (e.g., "2-of-3")
- See mode (TSS or HTSS)
- Check balances on Testnet/Signet/Mainnet

### 2. Network Selection
Press `n` to switch between:
- **Testnet** - Bitcoin testnet for testing
- **Signet** - Bitcoin signet for testing
- **Mainnet** - Real Bitcoin (use with caution)

### 3. Keygen Wizard
Press `g` to create a new DKG wallet:
- **Round 1**: Set wallet name, threshold, parties, your index/rank
- **Round 2**: Exchange commitments between parties
- **Finalize**: Exchange shares to complete key generation

### 4. Reshare Wizard
Press `h` (with wallet selected) to reshare:
- **Round 1**: Generate sub-shares for new configuration
- **Finalize**: New parties combine sub-shares to create new shares
- Maintains the same public key and address

### 5. Demo-Send (Threshold Signing)
Press `s` (with wallet selected) to sign:
- **Step 1**: Select wallet
- **Step 2**: Enter destination and amount
- **Step 3**: View sighash (message to sign)
- **Step 4**: Generate nonce
- **Step 5**: Collect all nonces
- **Step 6**: Generate signature share
- **Step 7**: Combine shares (aggregator)

## Keyboard Controls

### Home Screen
| Key | Action |
|-----|--------|
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `Enter` / `r` | Refresh balance |
| `R` | Reload wallet list |
| `n` | Network/chain selector |
| `g` | Keygen wizard |
| `h` | Reshare wizard |
| `s` | Send/Sign wizard |
| `q` | Quit |

### Wizard Screens
| Key | Action |
|-----|--------|
| `Tab` | Next field |
| `Shift+Tab` | Previous field |
| `Space` | Toggle checkbox |
| `Enter` | Confirm / Next step |
| `Esc` | Back / Cancel |
| `c` | Copy output to clipboard |

## Screen Layout

```
┌──────────────────────────────────────────────────────────────────┐
│ FrostDAO - DKG Wallet Manager  [Testnet]                         │
├────────────────────────────┬─────────────────────────────────────┤
│ Wallets                    │ Details                             │
│                            │                                     │
│ >> my_wallet (2-of-3 TSS)  │ Name: my_wallet                     │
│    backup (3-of-5 HTSS) $  │ Threshold: 2-of-3                   │
│    test (1-of-1 TSS)       │ Mode: Hierarchical (HTSS)           │
│                            │                                     │
│                            │ Address:                            │
│                            │ tb1p7sray...zpvhs4x7ehm             │
│                            │                                     │
│                            │ Balance: 10000 sats (0.0001 BTC)    │
│                            │                                     │
├────────────────────────────┴─────────────────────────────────────┤
│ ↑/↓:Navigate | Enter:Balance | n:Network | g:Keygen | s:Send     │
└──────────────────────────────────────────────────────────────────┘
```

## Workflow Examples

### Creating a 2-of-3 Wallet

1. Launch TUI: `frostdao tui`
2. Press `g` to start Keygen wizard
3. Enter wallet name, threshold=2, parties=3, your index
4. Press Enter to generate Round 1 output
5. Copy output (press `c`) and share with other parties
6. Press Enter to continue to Round 2
7. Paste all Round 1 outputs from other parties
8. Press Enter to generate Round 2 output
9. Copy and share Round 2 output
10. Press Enter to continue to Finalize
11. Paste all Round 2 outputs
12. Press Enter to complete

### Signing a Transaction

1. Select a wallet and press `s`
2. Enter destination address and amount
3. Share the sighash with signing parties
4. Each party generates their nonce
5. Collect all nonces and generate signature shares
6. Aggregator combines shares to create final signature

### Resharing to New Parties

1. Select source wallet and press `h`
2. Configure new threshold and party count
3. Generate Round 1 sub-shares
4. New parties collect sub-shares and finalize
5. New wallet is created with same address

## Module Structure

```
src/tui/
├── mod.rs              # Entry point, event loop, key handlers
├── app.rs              # App state and logic
├── state.rs            # State machine definitions
├── components/
│   ├── text_input.rs   # Single-line input widget
│   └── text_area.rs    # Multi-line text area widget
└── screens/
    ├── home.rs         # Wallet list and details
    ├── chain_select.rs # Network selector popup
    ├── keygen.rs       # Keygen wizard screens
    ├── reshare.rs      # Reshare wizard screens
    └── send.rs         # Send wizard screens
```

## State Machine

```
AppState
├── Home                    # Wallet list view
├── ChainSelect             # Network selector popup
├── Keygen
│   ├── Round1Setup         # Configuration form
│   ├── Round1Output        # Display commitment
│   ├── Round2Input         # Paste commitments
│   ├── Round2Output        # Display shares
│   ├── FinalizeInput       # Paste shares
│   └── Complete            # Success screen
├── Reshare
│   ├── Round1Setup         # Source wallet + config
│   ├── Round1Output        # Display sub-shares
│   ├── FinalizeInput       # New party setup
│   └── Complete            # Success screen
└── Send
    ├── SelectWallet        # Choose wallet
    ├── EnterDetails        # Address + amount
    ├── ShowSighash         # Display message
    ├── GenerateNonce       # Display nonce
    ├── EnterNonces         # Paste nonces
    ├── GenerateShare       # Display share
    ├── CombineShares       # Aggregator input
    └── Complete            # Success screen
```

## Dependencies

```toml
[dependencies]
ratatui = "0.29"
crossterm = "0.28"
reqwest = { version = "0.12", features = ["blocking", "json"] }
```

## Troubleshooting

### No Wallets Displayed
```bash
# Check if wallets exist
ls -la .frost_state/

# Ensure wallets are complete
ls .frost_state/*/shared_key.bin
```

### Balance Fetch Fails
```bash
# Check network connectivity
curl https://mempool.space/testnet/api/blocks/tip/height
```

### Terminal Issues
```bash
# Reset terminal if corrupted
reset
# or
stty sane
```
