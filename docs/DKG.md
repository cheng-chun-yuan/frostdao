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

Each party `i` generates polynomial:
```
f_i(x) = a_{i,0} + a_{i,1}x + ... + a_{i,t-1}x^{t-1}
```

Final secret share for party `j`:
```
s_j = Σ f_i(j) for all i
```

Group public key:
```
PK = Σ a_{i,0} * G for all i
```

See [CRYPTOGRAPHIC_ANALYSIS.md](CRYPTOGRAPHIC_ANALYSIS.md) for detailed security analysis.
