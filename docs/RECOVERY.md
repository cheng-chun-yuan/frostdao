# Share Recovery

If a party loses their share, it can be reconstructed from `t` other parties without changing the group public key or Bitcoin address.

## How It Works

```
Helper Party 1  ──→  sub_share_{1→lost}  ─┐
Helper Party 2  ──→  sub_share_{2→lost}  ─┼─→  Lost Party combines  ──→  Recovered Share
Helper Party 3  ──→  sub_share_{3→lost}  ─┘

                     (need t helpers)           s_lost = Σ λᵢ * sub_shareᵢ
```

Each helper evaluates their share polynomial at the lost party's index using Lagrange interpolation (or Birkhoff interpolation for HTSS).

## Commands

### Step 1: Helper Generates Sub-Share

```bash
frostdao recover-round1 \
  --name treasury \
  --lost-index 3
```

### Step 2: Lost Party Combines

```bash
frostdao recover-finalize \
  --source treasury \
  --target treasury_recovered \
  --my-index 3 \
  --data '<sub-shares JSON>'
```

## Example: Recover Party 3 in 2-of-3 Wallet

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

## HTSS Recovery with Birkhoff

For hierarchical wallets with mixed ranks, recovery uses Birkhoff interpolation:

```bash
# HTSS wallet: CEO(rank 0), CFO(rank 1), COO(rank 1)
# If CFO loses share, CEO and COO can help recover

# CEO generates sub-share (rank 0)
CEO_SUB=$(frostdao recover-round1 --name corp --lost-index 2)

# COO generates sub-share (rank 1)
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

## Security Considerations

- Recovery reveals the recovered share to the recovering party
- The recovered share is mathematically identical to the original
- Group public key remains unchanged
- Other parties' shares remain secure

## Mathematical Foundation

### Standard Lagrange Recovery

For a polynomial `f(x)` of degree `t-1`, any `t` points can reconstruct any other point.

To recover share at index `j` from helpers at indices `{i₁, i₂, ..., iₜ}`:

```
s_j = f(j) = Σₖ λₖ(j) · sₖ

where λₖ(j) = Π_{m≠k} (j - iₘ)/(iₖ - iₘ)  (Lagrange coefficient at x=j)
```

### Example: Recover Party 3 from Parties 1 and 2

```
Helpers: {1, 2}, Lost index: 3

λ₁(3) = (3 - 2)/(1 - 2) = 1/(-1) = -1
λ₂(3) = (3 - 1)/(2 - 1) = 2/1 = 2

Recovered share:
  s₃ = λ₁(3)·s₁ + λ₂(3)·s₂
     = (-1)·s₁ + 2·s₂
```

### Sub-Share Protocol

To avoid exposing helper shares directly, each helper computes a sub-share:

```
Helper i computes: sub_share_i = λᵢ(lost_index) · sᵢ

Lost party combines: s_lost = Σᵢ sub_share_i
```

This ensures:
- Helpers don't reveal their actual shares
- Lost party only learns their own share

### HTSS Recovery with Birkhoff

For hierarchical wallets, recovery uses Birkhoff interpolation with derivatives:

```
Party with rank r holds: f^(r)(xᵢ) (r-th derivative at their index)

Birkhoff coefficient βᵢⱼ depends on:
  - Helper ranks
  - Lost party's rank
  - All party indices

sub_share_i = βᵢ,lost · share_i

Recovered: s_lost = Σᵢ sub_share_i
```

The Birkhoff matrix must satisfy the Pólya condition for recovery to work.

## Implementation

| Component | File | Line |
|-----------|------|------|
| Recovery round 1 (CLI) | `src/protocol/recovery.rs` | 63 |
| Recovery round 1 (core) | `src/protocol/recovery.rs` | 106 |
| Recovery finalize | `src/protocol/recovery.rs` | - |
| Lagrange at target x | `src/crypto/helpers.rs` | 59 |
| Birkhoff coefficients | `src/crypto/birkhoff.rs` | 325 |
| Signer set validation | `src/crypto/birkhoff.rs` | 41 |

See [CRYPTOGRAPHIC_ANALYSIS.md](CRYPTOGRAPHIC_ANALYSIS.md) for security proofs and [HTSS.md](HTSS.md) for Birkhoff details.
