# Distributed Key Generation (DKG)

DKG creates a shared wallet where `t-of-n` parties must cooperate to sign. No single party ever learns the full private key.

## Protocol Flow

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

## Commands

### Round 1: Generate Commitments

```bash
frostdao keygen-round1 \
  --name my_wallet \
  --threshold 2 \
  --n-parties 3 \
  --my-index 1
```

Each party generates:
- Random polynomial of degree `t-1`
- Commitments to polynomial coefficients
- Proof of possession (PoP)

### Round 2: Exchange Shares

```bash
frostdao keygen-round2 \
  --name my_wallet \
  --data '<all_round1_outputs>'
```

Parties:
- Verify all PoPs
- Exchange encrypted share fragments

### Finalize: Derive Keys

```bash
frostdao keygen-finalize \
  --name my_wallet \
  --data '<all_round2_outputs>'
```

Each party:
- Verifies received shares
- Computes their secret share
- Derives group public key

## Example: 2-of-3 Wallet

```bash
# Party 1
P1_R1=$(frostdao keygen-round1 --name treasury --threshold 2 --n-parties 3 --my-index 1)

# Party 2
P2_R1=$(frostdao keygen-round1 --name treasury --threshold 2 --n-parties 3 --my-index 2)

# Party 3
P3_R1=$(frostdao keygen-round1 --name treasury --threshold 2 --n-parties 3 --my-index 3)

# Combine Round 1 outputs and run Round 2
ALL_R1="$P1_R1 $P2_R1 $P3_R1"
frostdao keygen-round2 --name treasury --data "$ALL_R1"

# ... exchange Round 2 outputs and finalize
```

## Threshold Signing

Once the wallet is created, signing requires `t` parties:

```bash
# 1. Each signer generates nonce
frostdao dkg-nonce --name treasury --session "tx-001"

# 2. Create signature shares (exchange nonces first)
frostdao dkg-sign \
  --name treasury \
  --session "tx-001" \
  --sighash <hex> \
  --data '<all_nonces>'

# 3. Combine into final signature
frostdao dkg-broadcast \
  --name treasury \
  --unsigned-tx <hex> \
  --data '<all_shares>'
```

## Storage

Wallet data stored in `~/.frostdao/wallets/<name>/`:

```
treasury/
├── shared_key.bin           # Group public key
├── hd_metadata.json         # HD derivation info
├── party1/
│   ├── paired_secret_share.bin  # Party 1's secret
│   └── htss_metadata.json       # Party 1's config
├── party2/
│   └── ...
└── party3/
    └── ...
```

## Mathematical Foundation

### Polynomial Secret Sharing

Each party `i` generates a random polynomial of degree `t-1`:

```
f_i(x) = a_{i,0} + a_{i,1}·x + a_{i,2}·x² + ... + a_{i,t-1}·xᵗ⁻¹

where:
  - a_{i,0} = party i's secret contribution (random scalar)
  - a_{i,1}...a_{i,t-1} = random coefficients
  - t = threshold (minimum signers needed)
```

### Share Computation

Party `j`'s final secret share combines evaluations from all parties:

```
s_j = Σᵢ f_i(j)

Expanded:
  s_j = f_1(j) + f_2(j) + ... + f_n(j)

This is equivalent to evaluating a combined polynomial F(x) at x=j:
  F(x) = Σᵢ f_i(x)
  s_j = F(j)
```

### Group Key Derivation

The group public key is the sum of all parties' first coefficients times generator:

```
PK = Σᵢ (a_{i,0} · G)

The corresponding private key (never computed) would be:
  sk = Σᵢ a_{i,0} = F(0)
```

### Lagrange Reconstruction

Given `t` shares `{(j₁, s_{j₁}), ..., (jₜ, s_{jₜ})}`, the secret can be reconstructed:

```
sk = F(0) = Σⱼ λⱼ · sⱼ

where λⱼ = Π_{k≠j} (0 - k)/(j - k)  (Lagrange coefficient at x=0)
```

### Proof of Possession (PoP)

Each party proves knowledge of their secret contribution without revealing it:

```
PoP = Sign(a_{i,0}, pubkey_i)

Verification proves party actually generated the polynomial
(prevents rogue-key attacks)
```

## Implementation

| Component | File | Line |
|-----------|------|------|
| Round 1 core logic | `src/protocol/keygen.rs` | 370 |
| Round 2 share exchange | `src/protocol/keygen.rs` | 581 |
| Finalize & derive keys | `src/protocol/keygen.rs` | 727 |
| Lagrange coefficients | `src/crypto/helpers.rs` | 59 |
| Storage helpers | `src/storage.rs` | - |

See [CRYPTOGRAPHIC_ANALYSIS.md](CRYPTOGRAPHIC_ANALYSIS.md) for detailed security analysis.
