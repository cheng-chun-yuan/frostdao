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

## Mathematical Foundation

### Birkhoff Interpolation

HTSS uses **Birkhoff interpolation** instead of standard Lagrange interpolation.

In standard Shamir, party `i` holds `sᵢ = f(xᵢ)` (function value).

In HTSS, party `i` with rank `rᵢ` holds the **rᵢ-th derivative**:

```
sᵢ = f^(rᵢ)(xᵢ)

Rank 0: sᵢ = f(xᵢ)      — function value
Rank 1: sᵢ = f'(xᵢ)     — first derivative
Rank 2: sᵢ = f''(xᵢ)    — second derivative
```

### Birkhoff Matrix

For polynomial `f(x) = a₀ + a₁x + a₂x² + ... + aₜ₋₁xᵗ⁻¹`:

The k-th derivative: `f^(k)(x) = Σⱼ₌ₖ (j!/(j-k)!) · aⱼ · xʲ⁻ᵏ`

Given signers at indices `{x₁,...,xₜ}` with ranks `{r₁,...,rₜ}`:

```
        ┌                                        ┐   ┌    ┐   ┌    ┐
        │ B₁,₀  B₁,₁  B₁,₂  ...  B₁,ₜ₋₁        │   │ a₀ │   │ v₁ │
        │ B₂,₀  B₂,₁  B₂,₂  ...  B₂,ₜ₋₁        │   │ a₁ │   │ v₂ │
    B = │  ...                                  │ · │ .. │ = │ .. │
        │ Bₜ,₀  Bₜ,₁  Bₜ,₂  ...  Bₜ,ₜ₋₁        │   │aₜ₋₁│   │ vₜ │
        └                                        ┘   └    ┘   └    ┘

where: Bᵢ,ⱼ = (j!/(j-rᵢ)!) · xᵢʲ⁻ʳⁱ  if j ≥ rᵢ, else 0
```

### Example: 3-of-5 with ranks [0,1,2]

Signers: Party 1 (rank 0), Party 2 (rank 1), Party 4 (rank 2)

```
Polynomial: f(x) = a₀ + a₁x + a₂x²

Known values:
  v₁ = f(1)   = a₀ + a₁ + a₂       (rank 0 at x=1)
  v₂ = f'(2)  = a₁ + 4a₂           (rank 1 at x=2)
  v₄ = f''(4) = 2a₂                (rank 2 at x=4)

Birkhoff Matrix:
    ┌         ┐
B = │ 1  1  1 │  ← rank 0 at x=1
    │ 0  1  4 │  ← rank 1 at x=2
    │ 0  0  2 │  ← rank 2 at x=4
    └         ┘
```

### Birkhoff Coefficients

To recover the secret `a₀`, compute first row of `B⁻¹`:

```
[β₁, β₂, β₄] = first row of B⁻¹

Secret: s = β₁·v₁ + β₂·v₂ + β₄·v₄
```

### Pólya Condition (Validity Rule)

The Birkhoff matrix is invertible **iff** the Pólya condition holds:

```
For sorted ranks [r₀, r₁, ..., rₜ₋₁]: rᵢ ≤ i for all i
```

This is why higher-ranked parties (lower rank number) are required!

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

| Component | File | Line |
|-----------|------|------|
| Signer set validation | `src/crypto/birkhoff.rs` | 41 |
| Birkhoff matrix computation | `src/crypto/birkhoff.rs` | 78 |
| Birkhoff coefficient to scalar | `src/crypto/birkhoff.rs` | 325 |
| HTSS keygen (with ranks) | `src/protocol/keygen.rs` | 370 |
| HTSS signing | `src/protocol/signing.rs` | - |
| Lagrange helpers | `src/crypto/helpers.rs` | 59 |

### Validation Code

The signing validation in `src/crypto/birkhoff.rs:41`:

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
