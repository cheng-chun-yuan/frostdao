# HD Key Derivation (BIP-32/44)

Generate multiple Bitcoin addresses from a single DKG wallet using hierarchical deterministic derivation.

## How It Works

```
Master Public Key (from DKG)
         │
    Chain Code (deterministic from group pubkey)
         │
    ┌────┴────┐
    │ BIP-32  │  Non-hardened derivation
    │ Child   │  child_pubkey = master + tweak * G
    └────┬────┘
         │
    m/44'/0'/0'/change/index
         │
    ┌────┴────────────┐
    │                 │
  change=0          change=1
  (receive)         (change)
    │                 │
  index: 0,1,2...   index: 0,1,2...
```

## Benefits

- **Multiple addresses** - Unlimited addresses from one wallet
- **Threshold compatible** - Each party derives locally
- **Privacy** - Fresh address for each transaction
- **Organized** - Separate receive vs change addresses

## Commands

### Derive Single Address

```bash
frostdao dkg-derive-address \
  --name treasury \
  --index 5 \
  --network testnet
```

### List Derived Addresses

```bash
frostdao dkg-list-addresses \
  --name treasury \
  --count 10 \
  --network testnet
```

### TUI

In TUI, navigate to wallet → View HD Addresses

## Path Structure

| Component | Value | Description |
|-----------|-------|-------------|
| Purpose | 44' | BIP-44 standard |
| Coin Type | 0' | Bitcoin |
| Account | 0' | Default account |
| Change | 0/1 | 0=receive, 1=change |
| Index | 0+ | Address index |

Full path: `m/44'/0'/0'/change/index`

## BIP-39 Mnemonic Backup

Backup your secret share as 24 words:

```bash
frostdao dkg-generate-mnemonic --name treasury

# Output:
# Your 24-word backup phrase (KEEP SECRET):
#  1. abandon   7. deputy   13. laptop   19. stone
#  2. ability   8. desert   14. left     20. stove
#  ...
```

**Important**: The mnemonic backs up your **share**, not the group key. Recovery still requires threshold cooperation.

## Signing with HD Addresses

When sending from an HD-derived address:

```bash
# TUI automatically handles HD path selection
frostdao tui
# Wallet → Send → Select HD address
```

The signing process:
1. Derives child public key at path
2. Each party derives their share tweak locally
3. Signs with derived share
4. Same threshold requirement applies

## Implementation

Key derivation in `src/crypto/hd.rs`:

```rust
pub fn derive_at_path(context: &HdContext, path: &DerivationPath) -> Result<DerivedKeyInfo> {
    // Non-hardened derivation: child = parent + tweak * G
    let tweak = tagged_hash("BIP0032/derive", &data);
    let child_pubkey = parent_pubkey + tweak * G;
    // ...
}
```

## Security

- Non-hardened derivation only (public derivation)
- Chain code is deterministic from group public key
- Each party derives independently using same public data
- Threshold requirement unchanged for all derived addresses
