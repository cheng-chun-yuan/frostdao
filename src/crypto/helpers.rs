//! Cryptographic Helper Functions
//!
//! This module provides shared utilities for cryptographic operations
//! used across keygen, signing, recovery, and resharing protocols.
//!
//! ## Functions
//!
//! - **PairedSecretShare helpers**: construct, negate, convert shares
//! - **Lagrange interpolation**: field-safe computation for threshold schemes
//! - **Tagged hash**: BIP340-style tagged hashing for Bitcoin protocols

use anyhow::Result;
use schnorr_fun::frost::{PairedSecretShare, SharedKey};
use schnorr_fun::fun::marker::*;
use secp256kfun::prelude::*;
use sha2::{Digest, Sha256};

// ============================================================================
// Tagged Hash (BIP340)
// ============================================================================

/// Compute BIP340 tagged hash: SHA256(SHA256(tag) || SHA256(tag) || data)
///
/// This is the standard tagged hash construction used throughout Bitcoin's
/// Taproot/Schnorr implementation for domain separation.
///
/// # Examples
/// - "TapTweak" tag for Taproot key tweaking
/// - "BIP0340/challenge" for Schnorr signature challenges
/// - "TapSighash" for Taproot sighash computation
pub fn tagged_hash(tag: &str, data: &[u8]) -> [u8; 32] {
    let tag_hash = Sha256::digest(tag.as_bytes());
    let mut hasher = Sha256::new();
    hasher.update(&tag_hash);
    hasher.update(&tag_hash);
    hasher.update(data);
    hasher.finalize().into()
}

// ============================================================================
// Lagrange Interpolation
// ============================================================================

/// Compute Lagrange coefficient for party_index at target_x.
///
/// λ_i(x) = Π_{j≠i} (x - j) / (i - j)
///
/// Uses field arithmetic directly to avoid integer overflow for large party counts.
/// Previous implementations using i64 accumulation then truncating to u32 silently
/// corrupted results for 14+ parties (13! = 6,227,020,800 > u32::MAX).
///
/// # Arguments
/// * `party_index` - The index i for which to compute the coefficient
/// * `all_indices` - All party indices participating in interpolation
/// * `target_x` - The x-coordinate at which to evaluate (0 for secret recovery)
///
/// # Returns
/// The Lagrange coefficient as a field scalar
pub fn lagrange_coefficient_at(
    party_index: u32,
    all_indices: &[u32],
    target_x: u32,
) -> Result<Scalar<Secret, Zero>> {
    let mut numerator: Scalar<Secret, Zero> = Scalar::from(1u32);
    let mut denominator: Scalar<Secret, Zero> = Scalar::from(1u32);

    let i_scalar: Scalar<Secret, Zero> = Scalar::from(party_index);
    let x_scalar: Scalar<Secret, Zero> = Scalar::from(target_x);

    for &other_index in all_indices {
        if other_index == party_index {
            continue;
        }

        let j_scalar: Scalar<Secret, Zero> = Scalar::from(other_index);

        // numerator *= (x - j)
        let x_minus_j = s!(x_scalar - j_scalar);
        numerator = s!(numerator * x_minus_j);

        // denominator *= (i - j)
        let i_minus_j = s!(i_scalar - j_scalar);
        denominator = s!(denominator * i_minus_j);
    }

    // Invert denominator and multiply
    let denom_nonzero = denominator
        .non_zero()
        .ok_or_else(|| anyhow::anyhow!("Lagrange denominator is zero - duplicate indices?"))?;
    let denom_inv = denom_nonzero.invert();
    let result = s!(numerator * denom_inv);

    Ok(result)
}

/// Compute Lagrange coefficient at x=0 (for secret/share reconstruction).
///
/// This is the common case for Shamir's Secret Sharing and FROST protocols
/// where we reconstruct the constant term (secret) of the polynomial.
///
/// λ_i(0) = Π_{j≠i} (-j) / (i - j) = Π_{j≠i} j / (j - i)
pub fn lagrange_coefficient_at_zero(
    party_index: u32,
    all_indices: &[u32],
) -> Result<Scalar<Secret, Zero>> {
    lagrange_coefficient_at(party_index, all_indices, 0)
}

// ============================================================================
// PairedSecretShare Helpers
// ============================================================================

/// Construct a PairedSecretShare from its components.
///
/// This creates the 96-byte bincode format:
/// - index: 32 bytes (Scalar)
/// - share: 32 bytes (Scalar)
/// - public_key: 32 bytes (Point x-only)
pub fn construct_paired_secret_share(
    index: u32,
    share: Scalar<Secret, NonZero>,
    group_public_key: &Point<EvenY>,
) -> Result<PairedSecretShare<EvenY>> {
    let index_scalar = Scalar::<Secret, Zero>::from(index)
        .non_zero()
        .ok_or_else(|| anyhow::anyhow!("Party index cannot be zero"))?;

    let mut paired_bytes = Vec::with_capacity(96);
    paired_bytes.extend_from_slice(&index_scalar.to_bytes()); // index: 32 bytes
    paired_bytes.extend_from_slice(&share.to_bytes()); // share: 32 bytes
    paired_bytes.extend_from_slice(&group_public_key.to_xonly_bytes()); // pubkey: 32 bytes

    let paired: PairedSecretShare<EvenY> = bincode::deserialize(&paired_bytes)?;
    Ok(paired)
}

/// Create a negated version of a PairedSecretShare.
///
/// This is needed for Taproot parity handling when the tweaked public key
/// has odd Y coordinate (requiring negation to achieve even Y for BIP340).
///
/// The negated share allows computing signature shares that will combine
/// correctly for the negated key.
pub fn negate_paired_secret_share(
    paired_share: &PairedSecretShare<EvenY>,
) -> Result<PairedSecretShare<EvenY>> {
    let secret_share = paired_share.secret_share();

    // Negate the share value
    let negated_share = s!(-{ secret_share.share });
    let negated_share_nonzero = negated_share
        .non_zero()
        .ok_or_else(|| anyhow::anyhow!("Negated share should be nonzero"))?;

    // Reconstruct with negated share
    let mut negated_bytes = Vec::with_capacity(96);
    negated_bytes.extend_from_slice(&secret_share.index.to_bytes()); // index unchanged
    negated_bytes.extend_from_slice(&negated_share_nonzero.to_bytes()); // negated share
    negated_bytes.extend_from_slice(&paired_share.public_key().to_xonly_bytes()); // pubkey unchanged

    let negated_paired: PairedSecretShare<EvenY> = bincode::deserialize(&negated_bytes)?;
    Ok(negated_paired)
}

/// Convert a Zero-variant scalar share to NonZero for use in PairedSecretShare.
///
/// Returns an error if the share is actually zero (extremely unlikely in practice).
pub fn share_to_nonzero(share: Scalar<Secret, Zero>) -> Result<Scalar<Secret, NonZero>> {
    share
        .non_zero()
        .ok_or_else(|| anyhow::anyhow!("Share value is zero (extremely unlikely)"))
}

// ============================================================================
// SharedKey Helpers
// ============================================================================

/// Construct a SharedKey from a public key point.
///
/// This is used for HD derivation where we need a SharedKey with the derived
/// public key but don't have the full polynomial info.
pub fn construct_shared_key(public_key: &Point<EvenY>) -> Result<SharedKey<EvenY>> {
    // SharedKey is serialized as just the 32-byte x-only public key
    let shared_key: SharedKey<EvenY> = bincode::deserialize(&public_key.to_xonly_bytes())?;
    Ok(shared_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_helpers() {
        // Tagged hash determinism
        let hash1 = tagged_hash("TestTag", b"data");
        let hash2 = tagged_hash("TestTag", b"data");
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, tagged_hash("Other", b"data"));

        // Lagrange coefficients sum to 1 at x=0
        let indices = vec![1u32, 2, 3];
        let one: Scalar<Secret, Zero> = Scalar::from(1u32);
        let mut sum: Scalar<Secret, Zero> = Scalar::zero();
        for &i in &indices {
            sum = s!(sum + lagrange_coefficient_at_zero(i, &indices).unwrap());
        }
        assert_eq!(sum.to_bytes(), one.to_bytes());

        // Large party count (15 parties)
        let large_indices: Vec<u32> = (1..=15).collect();
        let mut sum15: Scalar<Secret, Zero> = Scalar::zero();
        for &i in &large_indices {
            sum15 = s!(sum15 + lagrange_coefficient_at_zero(i, &large_indices).unwrap());
        }
        assert_eq!(sum15.to_bytes(), one.to_bytes());

        // Paired secret share construction and negation
        let mut rng = rand::thread_rng();
        let share = Scalar::<Secret, NonZero>::random(&mut rng);
        let sk: Scalar<Secret, NonZero> = Scalar::random(&mut rng);
        let pubkey = g!(sk * G)
            .normalize()
            .non_zero()
            .unwrap()
            .into_point_with_even_y()
            .0;
        let paired = construct_paired_secret_share(1, share, &pubkey).unwrap();
        assert_eq!(paired.secret_share().share.to_bytes(), share.to_bytes());

        let negated = negate_paired_secret_share(&paired).unwrap();
        let sum_neg = s!(paired.secret_share().share + negated.secret_share().share);
        assert_eq!(
            sum_neg.to_bytes(),
            Scalar::<Secret, Zero>::zero().to_bytes()
        );
    }
}
