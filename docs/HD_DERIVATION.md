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

## Mathematical Foundation

### BIP-32 Non-Hardened Derivation

For a parent public key `P` and chain code `c`, child derivation at index `i`:

```
data = P || i            (33-byte compressed pubkey + 4-byte index)
I = HMAC-SHA512(c, data)
I_L = first 32 bytes    (tweak scalar)
I_R = last 32 bytes     (child chain code)

child_pubkey = P + I_L·G
child_chaincode = I_R
```

### FROST HD Derivation

In threshold setting, no party has the full private key. Derivation works on public data:

```
Given:
  - Group public key: PK = Σᵢ sᵢ·G
  - Chain code: c (derived from group pubkey)

Child derivation:
  tweak = BIP32_derive(PK, c, path)
  child_PK = PK + tweak·G

For signing at derived path:
  Each party's effective share: s'ᵢ = sᵢ + tweak
  Combined: Σᵢ λᵢ·s'ᵢ = Σᵢ λᵢ·sᵢ + tweak = sk + tweak
```

### Tweak Accumulation

For multi-level paths (e.g., m/44'/0'/0'/0/5):

```
tweak_total = 0
For each level:
  tweak_level = derive(current_pubkey, chain_code, index)
  tweak_total += tweak_level
  current_pubkey = current_pubkey + tweak_level·G

Final: child_pubkey = PK + tweak_total·G
```

### BIP-340 Parity Handling

For Taproot (x-only pubkeys), if derived key has odd Y:

```
If child_pubkey.y is odd:
  child_pubkey = -child_pubkey  (negate to get even Y)
  tweak_total = -tweak_total    (track sign flip for signing)
```

## Implementation

| Component | File | Line |
|-----------|------|------|
| Path-based derivation | `src/crypto/hd.rs` | 180 |
| Child tweak computation | `src/crypto/hd.rs` | 116 |
| Child pubkey derivation | `src/crypto/hd.rs` | 156 |
| Share tweak for signing | `src/crypto/hd.rs` | 246 |
| BIP-340 tagged hash | `src/crypto/helpers.rs` | 31 |
| Address derivation CLI | `src/btc/hd_address.rs` | - |

## Security

- **Non-hardened only**: Hardened derivation requires private key (impossible in threshold)
- **Deterministic chain code**: Derived from group pubkey hash (consistent across parties)
- **Independent derivation**: Each party derives locally using same public data
- **Threshold preserved**: Same t-of-n requirement for all derived addresses
- **No key exposure**: Derivation never reveals individual shares or group secret
