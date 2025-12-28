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

For Lagrange recovery at index `j`:

```
s_j = Σᵢ λᵢ(j) * sᵢ

where λᵢ(j) = Π_{k≠i} (j - k)/(i - k)
```

For HTSS with Birkhoff, derivatives are used based on rank.

See [CRYPTOGRAPHIC_ANALYSIS.md](CRYPTOGRAPHIC_ANALYSIS.md) for details.
