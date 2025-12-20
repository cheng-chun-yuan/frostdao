# FrostDAO Terminal UI (TUI) Documentation

The FrostDAO TUI provides an interactive terminal-based interface for managing DKG wallets, including full wizards for key generation, resharing, and threshold signing.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Features Overview](#features-overview)
3. [TSS vs HTSS Modes](#tss-vs-htss-modes)
4. [Keyboard Controls](#keyboard-controls)
5. [Workflow Examples](#workflow-examples)
6. [Wizards Reference](#wizards-reference)
7. [Architecture](#architecture)
8. [Troubleshooting](#troubleshooting)

---

## Quick Start

```bash
# Build and run the TUI
cargo build --release
./target/release/frostdao tui

# Or directly with cargo
cargo run -- tui
```

### Prerequisites

- Rust toolchain (cargo)
- Terminal with Unicode support
- Network access for balance checking (optional)

---

## Features Overview

### 1. Wallet Management
- View all DKG wallets in `.frost_state/` directory
- See threshold configuration (e.g., "2-of-3")
- See mode (TSS or HTSS - Hierarchical)
- Check balances on Testnet/Signet/Mainnet

### 2. Network Selection
Press `n` to switch between:
- **Testnet** - Bitcoin testnet for testing
- **Signet** - Bitcoin signet for testing
- **Mainnet** - Real Bitcoin (use with caution!)

### 3. Keygen Wizard (`g`)
Create new threshold wallets with full DKG:
- Configure threshold, party count, and your index
- Support for both TSS and HTSS modes
- Three-round protocol for secure key generation

### 4. Reshare Wizard (`h`)
Proactive secret sharing without changing the public key:
- Transfer shares to new party configurations
- Change threshold requirements
- Maintain same Bitcoin address

### 5. Demo-Send Wizard (`s`)
Multi-party threshold signing demonstration:
- Generate and exchange nonces
- Create partial signatures
- Combine signatures (aggregator role)

---

## TSS vs HTSS Modes

FrostDAO supports two threshold signature schemes:

### Standard TSS (Threshold Signature Scheme)
- All parties are equal - any `t` out of `n` can sign
- Uses standard Lagrange interpolation
- Simpler setup, no hierarchy

**Example: 2-of-3 TSS**
```
Party 1 (index=1)  ──┐
Party 2 (index=2)  ──┼── Any 2 can sign
Party 3 (index=3)  ──┘
```

### HTSS (Hierarchical Threshold Signature Scheme)
- Parties have different authority levels (ranks)
- Uses Birkhoff interpolation with derivatives
- Enables organizational hierarchies

**Example: 2-of-3 HTSS with Director**
```
Director (rank=0)   ── Can sign with any 1 other party
Manager  (rank=1)   ── Needs Director or another Manager
Staff    (rank=1)   ── Needs Director or another Manager
```

**Rank Rules:**
- Lower rank number = higher authority
- Rank 0 parties are "Directors" with highest authority
- Same rank parties are peers
- Higher ranks need more participants to meet threshold

### Choosing Between TSS and HTSS

| Use Case | Recommended |
|----------|-------------|
| Equal partners | TSS |
| Corporate hierarchy | HTSS |
| Multisig escrow | TSS |
| Tiered access control | HTSS |
| Simple backup | TSS |
| Succession planning | HTSS |

---

## Keyboard Controls

### Home Screen
| Key | Action |
|-----|--------|
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `Enter` / `r` | Refresh balance |
| `R` | Reload wallet list from disk |
| `n` | Network/chain selector popup |
| `g` | Start Keygen wizard |
| `h` | Start Reshare wizard (requires wallet) |
| `s` | Start Send/Sign wizard (requires wallet) |
| `q` | Quit TUI |

### Wizard Navigation
| Key | Action |
|-----|--------|
| `Tab` | Next field |
| `Shift+Tab` | Previous field |
| `Space` | Toggle checkbox (HTSS mode) |
| `Enter` | Confirm / Execute / Next step |
| `Esc` | Back / Cancel |
| `c` | Copy output to clipboard |

### Text Input
| Key | Action |
|-----|--------|
| Any character | Insert at cursor |
| `Backspace` | Delete before cursor |
| `Delete` | Delete at cursor |
| `←` / `→` | Move cursor |
| `Home` | Move to start |
| `End` | Move to end |
| `Ctrl+U` | Clear input |

---

## Workflow Examples

### Creating a 2-of-3 TSS Wallet

**Party 1:**
```bash
frostdao tui
# Press 'g' for Keygen
# Enter: name=vault, threshold=2, parties=3, index=1
# Press Enter to generate Round 1
# Copy the JSON output (press 'c')
```

**Party 2:** Same process with `index=2`

**Party 3:** Same process with `index=3`

**All parties:** Exchange Round 1 outputs, paste them, continue through Round 2 and Finalize.

### Creating a Hierarchical HTSS Wallet

**Director (Party 1):**
```bash
frostdao tui
# Press 'g' for Keygen
# Enter: name=corp_vault, threshold=2, parties=3, index=1, rank=0
# Enable "HTSS" checkbox (press Space)
# Press Enter to generate
```

**Manager (Party 2):** `index=2, rank=1, HTSS enabled`

**Staff (Party 3):** `index=3, rank=1, HTSS enabled`

With this setup:
- Director + Manager can sign (ranks 0+1)
- Director + Staff can sign (ranks 0+1)
- Manager + Staff can sign (ranks 1+1)

### Signing a Transaction (Demo-Send)

1. Select wallet and press `s`
2. Enter destination address and amount
3. **Sighash Display**: Share this with all signing parties
4. **Generate Nonce**: Each party generates and shares their nonce
5. **Collect Nonces**: Paste all nonces (space-separated JSON)
6. **Generate Share**: Create your signature share
7. **Combine (Aggregator)**: One party collects all shares to produce final signature

### Resharing to Add a New Party

**Scenario:** Change 2-of-2 to 2-of-3 (add new backup holder)

**Old parties (1 and 2):**
```bash
frostdao tui
# Select wallet, press 'h' for Reshare
# New threshold=2, New parties=3
# Generate and share Round 1 output
```

**New party (3):**
```bash
frostdao tui
# Press 'h', go to Finalize step directly
# Enter: target_name, my_index=3, my_rank=0 (or 1 for HTSS)
# Paste Round 1 outputs from old parties
# Press Enter to complete
```

Result: Same public key and address, but now 2-of-3!

---

## Wizards Reference

### Keygen Wizard

Creates a new DKG wallet through three rounds.

**Round 1 Setup:**
| Field | Description |
|-------|-------------|
| Wallet Name | Unique identifier for this wallet |
| Threshold | Minimum signers needed (t) |
| Total Parties | Total participants (n) |
| My Index | Your party number (1 to n) |
| My Rank | HTSS authority level (0=highest) |
| Enable HTSS | Toggle hierarchical mode |

**Round 1 Output:** Commitment JSON to share

**Round 2 Input:** All parties' Round 1 outputs (space-separated)

**Round 2 Output:** Share JSON for other parties

**Finalize Input:** All parties' Round 2 outputs

**Complete:** Wallet created with Bitcoin address

### Reshare Wizard

Transfers secret shares to new configuration.

**Round 1 Setup:**
| Field | Description |
|-------|-------------|
| Source Wallet | Existing wallet to reshare |
| New Threshold | New minimum signers |
| New Total Parties | New party count |

**Round 1 Output:** Sub-shares for new parties

**Finalize Input (New Party):**
| Field | Description |
|-------|-------------|
| Target Name | New wallet name |
| My New Index | Your index in new setup |
| My Rank | Your rank in new setup |
| Enable HTSS | Use hierarchical mode |
| Round 1 Data | Sub-shares from old parties |

### Send Wizard (Demo-Send)

Interactive threshold signing flow.

**Steps:**
1. **Select Wallet** - Choose which wallet to sign with
2. **Enter Details** - Destination address, amount in sats
3. **Show Sighash** - Message to be signed (share with parties)
4. **Generate Nonce** - Your ephemeral nonce (share it)
5. **Enter Nonces** - Collect all parties' nonces
6. **Generate Share** - Your partial signature
7. **Combine Shares** - (Aggregator) Produce final signature

---

## Architecture

### Module Structure
```
src/tui/
├── mod.rs              # Entry point, event loop, key handlers
├── app.rs              # App state and business logic
├── state.rs            # State machine definitions
├── components/
│   ├── mod.rs
│   ├── text_input.rs   # Single-line input widget
│   └── text_area.rs    # Multi-line text area widget
└── screens/
    ├── mod.rs
    ├── home.rs         # Wallet list and details
    ├── chain_select.rs # Network selector popup
    ├── keygen.rs       # Keygen wizard screens
    ├── reshare.rs      # Reshare wizard screens
    └── send.rs         # Send wizard screens
```

### State Machine
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

### Data Flow
```
User Input → Event Handler → State Transition → Re-render
                ↓
        Core Function Call (keygen, reshare, signing)
                ↓
        File Storage (.frost_state/<wallet>/)
```

### Storage Format
```
.frost_state/
└── <wallet_name>/
    ├── htss_metadata.json      # Threshold, parties, ranks
    ├── paired_secret_share.bin # Your secret share
    ├── shared_key.bin          # Group public key
    ├── group_info.json         # Verification data
    └── nonces/                 # Signing nonces
        └── <session_id>.bin
```

---

## Troubleshooting

### TUI Won't Start
```bash
# Check terminal type
echo $TERM

# Try with explicit terminal type
TERM=xterm-256color frostdao tui

# Verify binary exists
ls -la ./target/release/frostdao
```

### No Wallets Displayed
```bash
# Check if wallets exist
ls -la .frost_state/

# Ensure wallets are complete (have shared_key.bin)
ls .frost_state/*/shared_key.bin
```

### Balance Fetch Fails
```bash
# Check network connectivity
curl https://mempool.space/testnet/api/blocks/tip/height

# Try different network (press 'n' in TUI)
```

### "Not enough sub-shares" in Reshare
- Ensure you have outputs from at least `t` old parties
- Verify old threshold matches source wallet

### Terminal Corrupted After Exit
```bash
# Reset terminal
reset

# Or
stty sane
```

### Invalid Signature During Signing
- Verify all parties used the same sighash/message
- Ensure nonces are from the same session
- Check that threshold parties are participating

---

## Security Considerations

### Key Material
- Secret shares are stored in `paired_secret_share.bin`
- Never share your secret share or nonce secrets
- Only share public commitments and signature shares

### Nonce Safety
- Nonces are single-use; never reuse for different messages
- If a nonce is reused, the secret key can be recovered

### Network Selection
- Use Testnet/Signet for testing
- Double-check when on Mainnet (real funds at risk!)

### Resharing
- Keep old shares until resharing is fully complete
- Verify new wallet has same address before deleting old

---

## Dependencies

```toml
[dependencies]
ratatui = "0.29"
crossterm = "0.28"
reqwest = { version = "0.12", features = ["blocking", "json"] }
schnorr_fun = "0.11"
bitcoin = "0.32"
```

---

## Testing

Run all tests including HTSS:
```bash
cargo test --release
```

Specific test categories:
```bash
# DKG flow tests (TSS and HTSS)
cargo test --release test_full_2_of_3

# Resharing tests
cargo test --release reshare

# Signing tests
cargo test --release signing
```
