# FrostDAO Resharing Protocol

**Version:** 1.0
**Date:** December 2024

---

## Table of Contents

1. [Overview](#overview)
2. [Mathematical Foundations](#mathematical-foundations)
3. [Zero Polynomial Technique](#zero-polynomial-technique)
4. [Threshold Modification](#threshold-modification)
5. [Party Count Modification](#party-count-modification)
6. [Protocol Flow](#protocol-flow)
7. [Implementation Details](#implementation-details)
8. [Security Considerations](#security-considerations)
9. [CLI Usage](#cli-usage)

---

## Overview

Resharing (also known as "proactive secret sharing" or "share refresh") allows a threshold signature scheme to:

1. **Change threshold**: Convert t-of-n to t'-of-n' (e.g., 2-of-3 → 3-of-5)
2. **Add/remove parties**: Expand or shrink the signing group
3. **Refresh shares**: Invalidate old shares without changing the secret
4. **Rotate keys**: Replace compromised parties' shares

**Critical Property**: The group public key (and Bitcoin address) remains unchanged.

---

## Mathematical Foundations

### Shamir's Secret Sharing Recap

A secret `s` is shared using a polynomial `f(x)` of degree `t-1`:

```
f(x) = s + a₁x + a₂x² + ... + aₜ₋₁xᵗ⁻¹

where:
  - f(0) = s (the secret)
  - f(i) = sᵢ (share for party i)
  - Any t points can reconstruct f(x) via Lagrange interpolation
```

### Lagrange Interpolation

To reconstruct the secret from t shares {(i₁, sᵢ₁), ..., (iₜ, sᵢₜ)}:

```
s = f(0) = Σⱼ λⱼ · sᵢⱼ

where λⱼ = Π_{k≠j} (0 - iₖ)/(iⱼ - iₖ)  (Lagrange coefficient at x=0)
```

---

## Zero Polynomial Technique

### Definition

A **zero polynomial** `g(x)` satisfies `g(0) = 0`:

```
g(x) = 0 + b₁x + b₂x² + ... + bₜ'₋₁xᵗ'⁻¹

Key property: g(0) = 0 (no constant term)
```

### Why It Works

Adding a zero polynomial to an existing sharing:

```
f'(x) = f(x) + g(x)

Verification:
  f'(0) = f(0) + g(0) = s + 0 = s  ✓ (secret unchanged)
  f'(i) = f(i) + g(i) = sᵢ + g(i)   (share changed!)
```

**Result**: Same secret, different shares, potentially different threshold.

### Visual Representation

```
Original Polynomial f(x):          Zero Polynomial g(x):
         •                                  •
        /                                  / \
       /                                  /   \
   ───●───────────                    ───O─────────────
     s                                   0

Combined f'(x) = f(x) + g(x):
           •
          /
         /
   ─────●─────────────────
        s (unchanged!)
```

---

## Threshold Modification

### Increasing Threshold (t → t')

To increase threshold from `t` to `t'` (where t' > t):

1. Old parties generate zero polynomials of degree `t' - 1`
2. The combined polynomial has degree `t' - 1`
3. Requires `t'` shares to reconstruct

**Example: 2-of-3 → 3-of-5**

```
Old polynomial (degree 1, threshold 2):
  f(x) = s + a₁x

Zero polynomial (degree 2):
  g(x) = 0 + b₁x + b₂x²

New polynomial (degree 2, threshold 3):
  f'(x) = s + (a₁ + b₁)x + b₂x²
```

### Decreasing Threshold (t → t')

To decrease threshold (where t' < t):

1. At least `t` old parties must participate (to reconstruct)
2. Generate zero polynomials of degree `t' - 1`
3. New polynomial has degree `t' - 1`

**Example: 3-of-5 → 2-of-3**

```
Old polynomial (degree 2, threshold 3):
  f(x) = s + a₁x + a₂x²

After reshare with degree-1 zero poly:
  f'(x) = s + a'₁x  (degree 1, threshold 2)
```

**Note**: Need at least 3 old parties to perform this reshare.

---

## Party Count Modification

### Adding Parties (n → n')

New parties receive shares without ever seeing the secret:

```
Scenario: 2-of-3 → 2-of-5 (adding P₄ and P₅)

Step 1: Old parties (P₁, P₂) generate zero polynomials
        P₁: g₁(x) = 0 + b₁x
        P₂: g₂(x) = 0 + b₂x

Step 2: Evaluate at ALL new indices (1, 2, 3, 4, 5)
        P₁ sends: g₁(1), g₁(2), g₁(3), g₁(4), g₁(5)
        P₂ sends: g₂(1), g₂(2), g₂(3), g₂(4), g₂(5)

Step 3: Each party computes new share
        For P₄ (brand new):
          new_s₄ = λ₁·sub₁[4] + λ₂·sub₂[4]

        where subᵢ[j] = old_sᵢ · gᵢ(j) (sub-share contribution)
```

### Removing Parties

To remove a party, simply don't include their index in the new set:

```
3-of-5 → 3-of-4 (removing P₅)

New indices: {1, 2, 3, 4}
P₅ never receives a new share → effectively removed
```

**Important**: Old shares must be deleted! See [Security Considerations](#security-considerations).

---

## Protocol Flow

### Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Reshare Protocol Flow                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Round 1: Each OLD party (need ≥ old_threshold)                     │
│  ├─ Load existing secret share sᵢ                                   │
│  ├─ Generate zero polynomial gᵢ(x) of degree (new_threshold - 1)   │
│  ├─ For each new party index j ∈ {1, ..., new_n}:                   │
│  │   └─ Compute sub_share[j] = sᵢ · gᵢ(j)                          │
│  ├─ Compute polynomial commitments Cᵢ = [gᵢ,₁·G, gᵢ,₂·G, ...]      │
│  └─ Output: {old_party_index, sub_shares, commitments}              │
│                                                                      │
│  Finalize: Each NEW party                                            │
│  ├─ Collect round1 outputs from ≥ old_threshold old parties        │
│  ├─ Compute Lagrange coefficients λⱼ for each old party j          │
│  ├─ Compute new_share = Σⱼ λⱼ · sub_shareⱼ[my_index]               │
│  ├─ Verify: new_share · G against commitments                       │
│  └─ Save new wallet with new_share                                  │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### Detailed Mathematics

**Round 1 (Old Party i):**

```
1. Load secret share: sᵢ

2. Generate zero polynomial:
   gᵢ(x) = b₁x + b₂x² + ... + bₜ'₋₁xᵗ'⁻¹
   where bⱼ ← random, t' = new_threshold

3. Compute sub-shares for each new party j:
   sub_shareᵢ[j] = sᵢ · gᵢ(j)

   Note: This embeds the old share into the sub-share

4. Compute commitments:
   Cᵢ,ₖ = bₖ · G for k = 1, ..., t'-1
```

**Finalize (New Party with index m):**

```
1. Collect: {(i, sub_shareᵢ[m])} from old parties

2. Compute Lagrange coefficients at x=0:
   λᵢ = Π_{j≠i} (0 - j)/(i - j)  for each old party i

3. Compute new share:
   new_sₘ = Σᵢ λᵢ · sub_shareᵢ[m]

   Expansion:
   new_sₘ = Σᵢ λᵢ · sᵢ · gᵢ(m)
          = (Σᵢ λᵢ · sᵢ) · (weighted polynomial eval)
          = s · (combined zero poly at m) + adjustment

   The result is a valid share of the same secret s.
```

---

## Implementation Details

### File Structure

```
src/protocol/reshare.rs
├── reshare_round1()           # CLI entry point for round 1
├── reshare_round1_core()      # Core round 1 logic
├── reshare_finalize()         # CLI entry point for finalize
└── reshare_finalize_core()    # Core finalize logic (TUI)
```

### Key Code: Round 1 Zero Polynomial Generation

Location: `src/protocol/reshare.rs:93-136`

```rust
// Generate zero polynomial coefficients (no constant term!)
let mut coefficients: Vec<[u8; 32]> = Vec::new();
for _ in 1..new_threshold {  // Note: starts at 1, not 0
    let coeff = Scalar::<Secret, NonZero>::random(&mut rng);
    coefficients.push(coeff.to_bytes());
}

// Evaluate at each new party index using Horner's method
for new_idx in 1..=new_n_parties {
    let mut result = [0u8; 32];

    // Horner's method: f(x) = ((...((bₜ₋₁ · x) + bₜ₋₂) · x) + ...) · x + b₁) · x
    for i in (0..coefficients.len()).rev() {
        let result_scalar = Scalar::from_bytes(result).unwrap_or(Scalar::zero());
        let x_scalar = Scalar::from(new_idx);
        let coeff_scalar = Scalar::from_bytes(coefficients[i]).unwrap_or(Scalar::zero());

        let new_result = s!(result_scalar * x_scalar + coeff_scalar);
        result = new_result.to_bytes();
    }

    // Multiply by old share to create sub-share
    // sub_share = old_share * zero_poly(new_idx)
    sub_shares.insert(new_idx, hex::encode(result));
}
```

### Key Code: Finalize Share Computation

Location: `src/protocol/reshare.rs:221-253`

```rust
// Collect old party indices for Lagrange computation
let old_indices: Vec<u32> = round1_outputs.iter()
    .map(|o| o.old_party_index)
    .collect();

// Compute new share: Σ (lagrange_coeff * sub_share)
let mut new_share_bytes = [0u8; 32];

for output in &round1_outputs {
    let sub_share = output.sub_shares.get(&my_new_index)?;
    let sub_share_scalar = Scalar::from_bytes(sub_share_bytes)?;

    // Lagrange coefficient at x=0 for this old party
    let lagrange_coeff = lagrange_coefficient_at_zero(
        output.old_party_index,
        &old_indices,
    )?;

    // Add weighted sub-share
    let weighted = s!(lagrange_coeff * sub_share_scalar);
    let current = Scalar::from_bytes(new_share_bytes)?;
    new_share_bytes = s!(current + weighted).to_bytes();
}
```

### Data Structures

```rust
/// Round 1 output (shared between old parties and new parties)
pub struct ReshareRound1Output {
    pub old_party_index: u32,
    pub sub_shares: BTreeMap<u32, String>,      // new_idx -> hex(sub_share)
    pub polynomial_commitment: Vec<String>,      // commitment to zero poly
    pub event_type: String,
}
```

---

## Security Considerations

### 1. Delete Old Shares

**Critical**: After successful reshare, delete old shares immediately.

```
⚠️  SECURITY RISK: Keeping both old and new shares

If attacker obtains:
  - 2 shares from old 2-of-3 scheme → CAN reconstruct secret
  - Even if new scheme is 3-of-5

The WEAKER threshold is always the effective security level.

✅ CORRECT: Delete old shares after verifying new shares work
```

### 2. Minimum Participation

At least `old_threshold` parties must participate in resharing:

```
2-of-3 → 3-of-5:  Need at least 2 old parties
3-of-5 → 2-of-3:  Need at least 3 old parties

If fewer participate, the secret cannot be properly transferred.
```

### 3. Verify Before Deleting

Always verify new shares work before deleting old ones:

```bash
# 1. Complete reshare
# 2. Test new wallet (e.g., sign a message)
# 3. Only then delete old wallet
rm -rf ~/.frostdao/wallets/old_wallet/
```

### 4. Zero Polynomial Leakage

The zero polynomial values reveal nothing about the secret:

```
g(0) = 0 (by construction)

Even knowing all g(i) values doesn't reveal s because:
  sub_share[i] = s · g(i)

Without knowing s, the sub-shares appear random.
```

### 5. Collusion Resistance

Security holds as long as:
- Fewer than `old_threshold` old parties collude, AND
- Fewer than `new_threshold` new parties collude

```
Attack scenario:
  If t-1 old parties AND t'-1 new parties collude:
  → Still cannot reconstruct the secret ✓
```

---

## CLI Usage

### Basic Reshare (Keep Same Threshold)

```bash
# Party 1: Generate round 1 data
frostdao reshare round1 \
  --wallet my_wallet \
  --new-threshold 2 \
  --new-n 3

# Party 2: Generate round 1 data
frostdao reshare round1 \
  --wallet my_wallet \
  --new-threshold 2 \
  --new-n 3

# Each party: Finalize with collected data
frostdao reshare finalize \
  --source my_wallet \
  --target my_wallet_v2 \
  --my-index 1 \
  --data '<party1_json> <party2_json>'
```

### Increase Threshold and Parties (2-of-3 → 3-of-5)

```bash
# Old parties 1 and 2 run round 1
frostdao reshare round1 --wallet old --new-threshold 3 --new-n 5

# All 5 new parties run finalize
# Party 4 (brand new):
frostdao reshare finalize \
  --source old \
  --target new_wallet \
  --my-index 4 \
  --data '<party1_r1> <party2_r1>'
```

### TUI Reshare

```bash
frostdao tui

# Navigate to wallet → Reshare Keys
# Follow the wizard prompts
```

---

## Summary

| Aspect | Details |
|--------|---------|
| **Purpose** | Change threshold, add/remove parties, refresh shares |
| **Key Technique** | Zero polynomial addition |
| **Secret** | Remains unchanged (same Bitcoin address) |
| **Minimum Parties** | old_threshold must participate |
| **New Parties** | Can join without knowing secret |
| **Security** | Delete old shares after reshare! |

### Quick Reference

```
Threshold Change:
  - New zero poly degree = new_threshold - 1
  - Combined poly has new threshold

Party Addition:
  - Evaluate at new indices
  - New parties get valid shares via Lagrange

Party Removal:
  - Simply exclude from new index set
  - Must delete their old shares!

Same Secret:
  - g(0) = 0 ensures s unchanged
  - Same public key, same address
```
