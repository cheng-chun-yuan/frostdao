# FrostDAO Cryptographic Security Analysis

**Version:** 1.0
**Date:** December 2024
**Analyst:** Cryptographic Expert Review

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Protocol Overview](#protocol-overview)
3. [Mathematical Foundations](#mathematical-foundations)
4. [Security Analysis](#security-analysis)
5. [Implementation Review](#implementation-review)
6. [Best Practices & Recommendations](#best-practices--recommendations)
7. [Known Limitations](#known-limitations)

---

## Executive Summary

FrostDAO implements a sophisticated threshold signature system combining:

- **FROST (Flexible Round-Optimized Schnorr Threshold Signatures)** - The core threshold signing protocol
- **HTSS (Hierarchical Threshold Secret Sharing)** - Rank-based access control via Birkhoff interpolation
- **Bitcoin Taproot Integration** - BIP340/BIP341 compliant Schnorr signatures

### Key Security Properties

| Property | Status | Notes |
|----------|--------|-------|
| **Key Secrecy** | ✅ Secure | No single party learns the full secret |
| **Threshold Security** | ✅ Secure | Requires t parties to sign |
| **HTSS Hierarchy** | ✅ Secure | Rank constraint: r_i ≤ i enforced |
| **Nonce Security** | ⚠️ Critical | Reuse = key leakage (well-documented) |
| **Taproot Parity** | ✅ Fixed | Parity handling implemented correctly |
| **Recovery Protocol** | ⚠️ Caution | Exposes raw shares (documented) |

---

## Protocol Overview

### 1. Distributed Key Generation (DKG)

The DKG follows the SimplePedPop protocol from schnorr_fun:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    DKG Protocol Flow                                 │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Round 1: Each party i                                               │
│  ├─ Generate random polynomial: f_i(x) = a_{i,0} + a_{i,1}x + ...   │
│  ├─ Compute commitments: C_{i,j} = a_{i,j} * G  (j = 0..t-1)        │
│  ├─ Generate Proof of Possession (PoP): σ_i = Sign(sk_i, pk_i)      │
│  └─ Broadcast: {C_{i,0}, ..., C_{i,t-1}, PoP_i}                     │
│                                                                      │
│  Round 2: Each party i                                               │
│  ├─ Verify all other parties' PoPs                                  │
│  └─ Send encrypted share f_i(j) to each party j                     │
│                                                                      │
│  Finalize: Each party j                                              │
│  ├─ Verify received shares against commitments                       │
│  ├─ Compute secret share: s_j = Σ f_i(j) for all i                  │
│  └─ Compute group public key: PK = Σ C_{i,0} for all i              │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

**Mathematical Verification:**

For share verification, party j checks:
```
s_j * G == Σ_{k=0}^{t-1} (j^k * C_{i,k})  for each party i
```

This ensures `s_j` lies on party i's committed polynomial.

### 2. FROST Signing Protocol

```
┌─────────────────────────────────────────────────────────────────────┐
│                    FROST Signing Flow                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Nonce Generation: Each signer i                                     │
│  ├─ Generate nonce pair: (k_i, K_i) where K_i = k_i * G             │
│  ├─ Binonce: (k_i1, k_i2) with K_i = (K_i1, K_i2)                   │
│  └─ Broadcast K_i to all signers                                     │
│                                                                      │
│  Signature Share Creation:                                           │
│  ├─ Aggregate nonces: R = Σ (K_i1 + ρ * K_i2)                       │
│  │   where ρ = H("frost/binding", i, K_i, PK, msg, ...)             │
│  ├─ Compute challenge: e = H("BIP0340/challenge", R || PK || msg)   │
│  ├─ Compute Lagrange coefficient: λ_i                                │
│  └─ Signature share: σ_i = k_i + e * λ_i * s_i                      │
│                                                                      │
│  Combination:                                                        │
│  ├─ Final signature: s = Σ σ_i                                      │
│  └─ Output: (R, s) - standard BIP340 Schnorr signature              │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 3. Resharing Protocol

The resharing protocol enables proactive security and key management:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Resharing Protocol Flow                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Old Party i (runs round1):                                          │
│  ├─ Load secret share s_i                                           │
│  ├─ Create polynomial: g_i(x) = s_i + b_{i,1}*x + ... + b_{i,t'-1}*x^{t'-1}  │
│  │  (note: g_i(0) = s_i, degree t'-1 for new threshold t')          │
│  ├─ Compute commitments: [s_i*G, b_{i,1}*G, ..., b_{i,t'-1}*G]      │
│  └─ Broadcast: sub_share_{i,j} = g_i(j) for each new party j        │
│                                                                      │
│  New Party j (runs finalize):                                        │
│  ├─ Collect sub_shares from ≥ t old parties                         │
│  ├─ Compute Lagrange coefficients λ_i at x=0 for old party indices  │
│  └─ Compute: s'_j = Σ λ_i(0) * sub_share_{i,j}                      │
│                                                                      │
│  Result: s'_j is new share for SAME secret s                         │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

**Mathematical Proof:**

```
s'_j = Σ_i λ_i(0) * sub_share_{i,j}
     = Σ_i λ_i(0) * g_i(j)
     = Σ_i λ_i(0) * (s_i + b_{i,1}*j + ... + b_{i,t'-1}*j^{t'-1})
     = Σ_i λ_i(0) * s_i   +   j * Σ_i λ_i(0) * b_{i,1}   + ...
     = s                   +   j * (random)               + ...
     = s + (new polynomial terms)
```

The first term equals s because Σ λ_i(0) * s_i is exactly Lagrange reconstruction!

### 4. Hierarchical TSS (HTSS)

HTSS extends FROST with rank-based access control using Birkhoff interpolation.

**Key Insight:** In HTSS, shares encode not just polynomial values f(x) but also derivatives f'(x), f''(x), etc.

```
Standard TSS:  Share_i = f(i)           (all rank 0)
HTSS:          Share_i = f^(r_i)(i)     (r_i = party's rank/derivative order)
```

**Validity Rule:** For threshold t, signers with sorted ranks [r_0, r_1, ..., r_{t-1}] are valid iff:
```
r_i ≤ i  for all i ∈ [0, t-1]
```

**Example (3-of-4 with ranks [0,1,1,2]):**
- [0,1,1] → Valid: 0≤0, 1≤1, 1≤2 ✓
- [0,1,2] → Valid: 0≤0, 1≤1, 2≤2 ✓
- [1,1,2] → Invalid: 1>0 at position 0 ✗

This ensures CEO (rank 0) must always participate!

---

## Mathematical Foundations

### Lagrange Interpolation

For secret reconstruction from t shares at indices {x_1, ..., x_t}:

```
Secret = f(0) = Σ_{i=1}^{t} λ_i(0) * f(x_i)

where λ_i(0) = Π_{j≠i} (0 - x_j) / (x_i - x_j)
             = Π_{j≠i} x_j / (x_j - x_i)
```

**Implementation in `crypto_helpers.rs`:**

```rust
pub fn lagrange_coefficient_at(
    party_index: u32,
    all_indices: &[u32],
    target_x: u32,
) -> Result<Scalar<Secret, Zero>> {
    // λ_i(x) = Π_{j≠i} (x - j) / (i - j)
    // Uses field arithmetic to avoid integer overflow
    // for large party counts (13! > u32::MAX)
}
```

**Critical Fix:** Previous implementations using i64 arithmetic silently corrupted results for 14+ parties due to factorial overflow. The current implementation uses direct scalar field arithmetic.

### Birkhoff Interpolation

Generalizes Lagrange by incorporating derivative constraints.

**Birkhoff Matrix Construction:**

For shares with parameters [(x_1, r_1), ..., (x_n, r_n)] where r_i is the derivative order:

```
B[i,j] = d^{r_i}/dx^{r_i} [x^j] evaluated at x_i
       = j! / (j-r_i)! * x_i^{j-r_i}  if j ≥ r_i
       = 0                             if j < r_i
```

**Recovery Formula:**

To recover f^{(r)}(x) from helper shares:
```
coefficients = eval_vector · B^{-1}

where eval_vector[j] = j! / (j-r)! * x^{j-r}
```

**Implementation Note:** Uses `nalgebra` SVD for matrix inversion with tolerance 1e-10.

### Taproot Key Tweaking (BIP341)

For a P2TR address, the tweaked public key Q is:

```
t = H("TapTweak", P)           // tweak scalar
Q = P + t*G                    // tweaked public key

where P is the internal (group) public key
```

**Parity Handling (Critical Fix):**

BIP340 requires x-only public keys (even Y coordinate). If Q has odd Y:
1. Negate Q to get even Y: Q' = -Q
2. To sign with Q', we need to negate secret shares
3. Signature computation: s = σ - e*t (subtract, not add)

```rust
fn compute_tweaked_pubkey(internal_pubkey: &Point<EvenY>) -> (Point<EvenY>, bool) {
    let tweak = compute_taptweak(&pubkey_bytes);
    let tweaked = g!({ *internal_pubkey } + tweak * G).normalize();
    let (even_y_point, parity_flip) = tweaked_nonzero.into_point_with_even_y();
    (even_y_point, parity_flip)  // parity_flip indicates if negation occurred
}
```

### Tagged Hash (BIP340)

Domain separation via tagged hashing:

```
tagged_hash(tag, data) = SHA256(SHA256(tag) || SHA256(tag) || data)
```

Used for:
- `"TapTweak"` - Key tweaking
- `"BIP0340/challenge"` - Signature challenge
- `"TapSighash"` - Transaction sighash (computed by bitcoin library)

---

## Security Analysis

### Threat Model

| Threat | Mitigation | Status |
|--------|------------|--------|
| Malicious DKG participant | PoP verification, share verification against commitments | ✅ |
| Rogue key attack | Proof of Possession (PoP) signatures | ✅ |
| Nonce reuse | Session-based nonce generation, warnings | ⚠️ User responsibility |
| < t colluding parties | Mathematical guarantee from Shamir's Secret Sharing | ✅ |
| HTSS rank bypass | Birkhoff matrix singularity check | ✅ |
| Privilege escalation in recovery | Original rank preserved from source wallet | ✅ |

### Nonce Security (Critical)

**The Golden Rule:** NEVER reuse nonces!

If a party uses the same nonce k for two different messages m₁ and m₂:
```
σ₁ = k + e₁ * λ * s
σ₂ = k + e₂ * λ * s

⟹ s = (σ₁ - σ₂) / (λ * (e₁ - e₂))
```

The secret share is immediately recoverable!

**Implementation Safeguards:**
1. Synthetic nonces: `frost.seed_nonce_rng(paired_share, session_id.as_bytes())`
2. Session-based storage: `nonce_{session}.bin`
3. Clear warnings in output

### Share Recovery Security

The recovery protocol has an inherent trade-off:

**Pro:** Allows reconstruction of lost shares without changing the group key.

**Con:** Helpers expose their raw share values to the recovering party.

After recovery, the recovering party knows t shares (theirs + t-1 helpers), which is exactly the threshold. They could theoretically reconstruct the full secret.

**Mitigation:**
- Clear security warnings in output
- Recommendation: Use resharing (which uses blinded sub-shares) for production

### HTSS Signer Validation

```rust
pub fn validate_signer_set(ranks: &[u32], threshold: u32) -> Result<()> {
    let mut sorted_ranks = ranks.to_vec();
    sorted_ranks.sort();

    for (i, &rank) in sorted_ranks.iter().take(threshold as usize).enumerate() {
        if rank > i as u32 {
            bail!("Invalid HTSS signer set: rank {} at position {} violates n_i <= i rule");
        }
    }
    Ok(())
}
```

This enforces the hierarchical constraint mathematically.

---

## Implementation Review

### Field Arithmetic Safety

The codebase correctly uses `secp256kfun` scalar arithmetic:

```rust
// Safe: Operations in finite field, no overflow
let x_minus_j = s!(x_scalar - j_scalar);
numerator = s!(numerator * x_minus_j);
```

### Type Safety

Uses Rust's type system for cryptographic correctness:

```rust
// Marker types prevent mixing:
Scalar<Secret, Zero>    // May be zero
Scalar<Secret, NonZero> // Guaranteed non-zero
Point<EvenY>            // BIP340-compatible public key
```

### PairedSecretShare Construction

```rust
pub fn construct_paired_secret_share(
    index: u32,
    share: Scalar<Secret, NonZero>,
    group_public_key: &Point<EvenY>,
) -> Result<PairedSecretShare<EvenY>> {
    // 96-byte bincode format:
    // [index:32][share:32][pubkey:32]
    let mut paired_bytes = Vec::with_capacity(96);
    paired_bytes.extend_from_slice(&index_scalar.to_bytes());
    paired_bytes.extend_from_slice(&share.to_bytes());
    paired_bytes.extend_from_slice(&group_public_key.to_xonly_bytes());

    Ok(bincode::deserialize(&paired_bytes)?)
}
```

### Test Coverage

Strong test coverage for cryptographic invariants:

```rust
#[test]
fn test_lagrange_coefficient_at_zero_sum() {
    // Fundamental property: Lagrange coefficients sum to 1
    let indices = vec![1u32, 2, 3];
    let mut sum: Scalar<Secret, Zero> = Scalar::zero();
    for &i in &indices {
        let coeff = lagrange_coefficient_at_zero(i, &indices).unwrap();
        sum = s!(sum + coeff);
    }
    let one: Scalar<Secret, Zero> = Scalar::from(1u32);
    assert_eq!(sum.to_bytes(), one.to_bytes());
}

#[test]
fn test_recovery_math() {
    // Verifies: s_3 = λ_1(3) * s_1 + λ_2(3) * s_2
    // For polynomial f(x) = s + a*x
}
```

---

## Best Practices & Recommendations

### Production Deployment

| Recommendation | Priority | Status |
|----------------|----------|--------|
| HSM integration for key storage | High | Not implemented |
| Encrypted share storage at rest | High | Not implemented |
| Authenticated communication channels | High | User responsibility |
| Nonce pre-generation for 1-round signing | Medium | Not implemented |
| Rate limiting on signing operations | Medium | Not implemented |
| Audit logging | Medium | Partial (file-based) |

### Code Quality

1. **Error Handling:** Uses `anyhow` for contextual errors - good for debugging
2. **Type Safety:** Leverages Rust's type system effectively
3. **Documentation:** Well-documented module headers and function docs
4. **Test Coverage:** Core cryptographic functions well-tested

### Security Hardening

```rust
// RECOMMENDED: Add constant-time comparison for secrets
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// RECOMMENDED: Zeroize secrets on drop
use zeroize::Zeroize;
struct SecretWrapper(Scalar<Secret, NonZero>);
impl Drop for SecretWrapper {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}
```

---

## Known Limitations

### 1. Recovery Protocol Exposure

The simplified recovery exposes raw helper shares. For production:
- Use blinded sub-shares (like resharing)
- Or implement DKG-based recovery with fresh polynomials

### 2. Multi-UTXO Signing

Current DKG transaction signing only handles the first input:
```rust
if tx.input.len() > 1 {
    out.push_str("⚠️ WARNING: Only first input will be signed.");
}
```

For multi-UTXO transactions, each input needs a separate signing session.

### 3. Birkhoff Floating-Point Precision

The Birkhoff implementation uses f64 for matrix operations with proper field arithmetic:
```rust
const SCALE: u64 = 1_000_000_000_000; // 10^12
let scaled = (abs_coeff * SCALE as f64).round() as u64;

// Modular inverse is applied internally to produce correct field elements
let scale_inverse = scale_nonzero.invert();
let result = s!(scaled_scalar * scale_inverse);
```

For binary-representable fractions (1/2, 1/4, etc.), results are exact. For others (1/3),
precision is limited by floating-point representation. Practical Birkhoff coefficients
for small party counts are typically integers, avoiding this limitation.

### 4. Session State Management

Session data is stored in plaintext JSON files. For production:
- Encrypt session files
- Implement session expiration
- Consider secure memory for ephemeral data

---

## Appendix: Key Formulas Reference

### Schnorr Signature
```
s = k + e * x
where e = H("BIP0340/challenge", R || P || m)
Verify: s*G == R + e*P
```

### FROST Threshold Signature
```
σ_i = k_i + e * λ_i * s_i    (each signer)
s = Σ σ_i                     (combined)
```

### Taproot Tweak
```
Q = P + H("TapTweak", P) * G
Sign with d' = d + t (or d' = -d - t if Q negated)
```

### Lagrange Coefficient
```
λ_i(x) = Π_{j≠i} (x - x_j) / (x_i - x_j)
```

### Birkhoff Matrix Entry
```
B[i,j] = (j)_{r_i} * x_i^{j-r_i}
where (j)_r = j! / (j-r)! is falling factorial
```

### HTSS Validity
```
Valid iff: sorted_ranks[i] ≤ i for all i < t
```

---

*This analysis is based on code review as of December 2024. Cryptographic implementations should be audited by professional security firms before production deployment.*
