# Hierarchical Threshold Secret Sharing (HTSS)

HTSS adds rank-based access control to threshold signatures. Higher ranks (lower numbers) have more authority.

## Rank System

| Rank | Authority | Example Role |
|------|-----------|--------------|
| 0 | Highest | CEO, Board |
| 1 | High | C-Suite, Directors |
| 2 | Medium | Managers |
| 3+ | Lower | Staff |

## Signing Rule

For signers with sorted ranks `[r₀, r₁, ..., r_{t-1}]`:

```
Valid if: rᵢ ≤ i for all positions i
```

This ensures higher-ranked members must participate.

## Example: 3-of-4 Corporate Treasury

```bash
# CEO (rank 0)
frostdao keygen-round1 --name corp --threshold 3 --n-parties 4 \
  --my-index 1 --rank 0 --hierarchical

# CFO (rank 1)
frostdao keygen-round1 --name corp --threshold 3 --n-parties 4 \
  --my-index 2 --rank 1 --hierarchical

# COO (rank 1)
frostdao keygen-round1 --name corp --threshold 3 --n-parties 4 \
  --my-index 3 --rank 1 --hierarchical

# Manager (rank 2)
frostdao keygen-round1 --name corp --threshold 3 --n-parties 4 \
  --my-index 4 --rank 2 --hierarchical
```

## Valid Combinations

| Signers | Ranks | Valid? | Reason |
|---------|-------|--------|--------|
| CEO + CFO + COO | [0,1,1] | Yes | 0≤0, 1≤1, 1≤2 |
| CEO + CFO + Manager | [0,1,2] | Yes | 0≤0, 1≤1, 2≤2 |
| CFO + COO + Manager | [1,1,2] | No | 1>0 at position 0 |

**CEO must always be involved!**

## Birkhoff Interpolation

HTSS uses Birkhoff interpolation instead of Lagrange:

- Rank 0: Evaluates polynomial value `f(x)`
- Rank 1: Evaluates first derivative `f'(x)`
- Rank k: Evaluates k-th derivative `f^(k)(x)`

This mathematical property enforces the rank constraint.

## Use Cases

### Corporate Treasury
```
3-of-5 with ranks: CEO(0), CFO(1), Treasurer(1), Director(2), Accountant(2)
Policy: CEO approval required for all movements
```

### DAO Multi-Sig
```
4-of-7 with ranks: Core Devs(0), Community Leads(1), Advisors(2), Reps(3)
Policy: At least one core dev required
```

### Family Trust
```
2-of-4 with ranks: Parents(0), Adult Children(1)
Policy: Children cannot act alone
```

### Exchange Hot Wallet
```
3-of-6 with ranks: Security Officer(0), CTO(0), Sr DevOps(1), Engineers(2)
Policy: Security/CTO approval required
```

## Implementation

The signing validation is in `src/crypto/birkhoff.rs`:

```rust
pub fn validate_signer_set(ranks: &[u32], threshold: u32) -> Result<()> {
    let mut sorted_ranks = ranks.to_vec();
    sorted_ranks.sort();

    for (i, &rank) in sorted_ranks.iter().enumerate() {
        if rank > i as u32 {
            anyhow::bail!("Invalid signer set: rank {} at position {}", rank, i);
        }
    }
    Ok(())
}
```

See [CRYPTOGRAPHIC_ANALYSIS.md](CRYPTOGRAPHIC_ANALYSIS.md) for security proofs.
