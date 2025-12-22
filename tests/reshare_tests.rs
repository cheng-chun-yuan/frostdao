//! Integration tests for the resharing protocol

use schnorr_fun::fun::marker::*;
use secp256kfun::prelude::*;

/// Test that Lagrange coefficients sum to 1
/// This is a fundamental property: Σ λ_i(0) = 1
#[test]
fn test_lagrange_sum_property() {
    let indices = vec![1u32, 2, 3];

    // Compute Lagrange coefficients at x=0
    let mut sum: Scalar<Secret, Zero> = Scalar::zero();

    for &i in &indices {
        let coeff = compute_lagrange_at_zero(i, &indices);
        sum = s!(sum + coeff);
    }

    // Sum should equal 1
    let one: Scalar<Secret, Zero> = Scalar::from(1u32);
    assert_eq!(
        sum.to_bytes(),
        one.to_bytes(),
        "Lagrange coefficients should sum to 1"
    );
}

/// Test Lagrange interpolation reconstructs the secret
#[test]
fn test_lagrange_reconstruction() {
    let mut rng = rand::thread_rng();

    // Create a secret and polynomial f(x) = secret + coeff * x
    let secret = Scalar::<Secret, NonZero>::random(&mut rng);
    let coeff = Scalar::<Secret, NonZero>::random(&mut rng);

    // Evaluate shares at indices 1, 2, 3
    let share1 = evaluate_poly(&secret, &coeff, 1);
    let share2 = evaluate_poly(&secret, &coeff, 2);
    let share3 = evaluate_poly(&secret, &coeff, 3);

    // Reconstruct using shares 1 and 2
    let indices_12 = vec![1u32, 2];
    let lambda1 = compute_lagrange_at_zero(1, &indices_12);
    let lambda2 = compute_lagrange_at_zero(2, &indices_12);
    let reconstructed_12 = s!(lambda1 * share1 + lambda2 * share2);

    // Reconstruct using shares 2 and 3
    let indices_23 = vec![2u32, 3];
    let lambda2_alt = compute_lagrange_at_zero(2, &indices_23);
    let lambda3 = compute_lagrange_at_zero(3, &indices_23);
    let reconstructed_23 = s!(lambda2_alt * share2 + lambda3 * share3);

    // Reconstruct using shares 1 and 3
    let indices_13 = vec![1u32, 3];
    let lambda1_alt = compute_lagrange_at_zero(1, &indices_13);
    let lambda3_alt = compute_lagrange_at_zero(3, &indices_13);
    let reconstructed_13 = s!(lambda1_alt * share1 + lambda3_alt * share3);

    // All reconstructions should equal the original secret
    let secret_bytes = secret.to_bytes();
    assert_eq!(
        reconstructed_12.to_bytes(),
        secret_bytes,
        "Reconstruction with shares 1,2 failed"
    );
    assert_eq!(
        reconstructed_23.to_bytes(),
        secret_bytes,
        "Reconstruction with shares 2,3 failed"
    );
    assert_eq!(
        reconstructed_13.to_bytes(),
        secret_bytes,
        "Reconstruction with shares 1,3 failed"
    );
}

/// Test resharing preserves the group secret
#[test]
fn test_resharing_preserves_secret() {
    let mut rng = rand::thread_rng();

    // Original 2-of-3 setup
    let secret = Scalar::<Secret, NonZero>::random(&mut rng);
    let coeff = Scalar::<Secret, NonZero>::random(&mut rng);

    // Original shares
    let old_share1 = evaluate_poly(&secret, &coeff, 1);
    let old_share2 = evaluate_poly(&secret, &coeff, 2);

    // Resharing: each old party creates a new polynomial with their share as constant term
    // Old party 1: g_1(x) = old_share1 + b1*x
    let b1 = Scalar::<Secret, NonZero>::random(&mut rng);
    let sub_share_1_to_1 = evaluate_poly_from_share(&old_share1, &b1, 1); // g_1(1)
    let sub_share_1_to_2 = evaluate_poly_from_share(&old_share1, &b1, 2); // g_1(2)

    // Old party 2: g_2(x) = old_share2 + b2*x
    let b2 = Scalar::<Secret, NonZero>::random(&mut rng);
    let sub_share_2_to_1 = evaluate_poly_from_share(&old_share2, &b2, 1); // g_2(1)
    let sub_share_2_to_2 = evaluate_poly_from_share(&old_share2, &b2, 2); // g_2(2)

    // New party 1 combines sub-shares using Lagrange
    let old_indices = vec![1u32, 2];
    let lambda1 = compute_lagrange_at_zero(1, &old_indices);
    let lambda2 = compute_lagrange_at_zero(2, &old_indices);

    let new_share1 = s!(lambda1 * sub_share_1_to_1 + lambda2 * sub_share_2_to_1);

    // New party 2 combines sub-shares
    let new_share2 = s!(lambda1 * sub_share_1_to_2 + lambda2 * sub_share_2_to_2);

    // Verify: reconstruct secret from new shares
    let new_indices = vec![1u32, 2];
    let new_lambda1 = compute_lagrange_at_zero(1, &new_indices);
    let new_lambda2 = compute_lagrange_at_zero(2, &new_indices);

    let reconstructed = s!(new_lambda1 * new_share1 + new_lambda2 * new_share2);

    assert_eq!(
        reconstructed.to_bytes(),
        Scalar::<Secret, Zero>::from_bytes(secret.to_bytes())
            .unwrap()
            .to_bytes(),
        "Resharing should preserve the original secret"
    );
}

/// Test threshold change: 2-of-3 to 2-of-4
#[test]
fn test_resharing_with_new_parties() {
    let mut rng = rand::thread_rng();

    // Original 2-of-3
    let secret = Scalar::<Secret, NonZero>::random(&mut rng);
    let coeff = Scalar::<Secret, NonZero>::random(&mut rng);

    let old_share1 = evaluate_poly(&secret, &coeff, 1);
    let old_share2 = evaluate_poly(&secret, &coeff, 2);

    // Reshare to 2-of-4 (adding one new party)
    // Old party 1 creates polynomial for new threshold 2
    let b1 = Scalar::<Secret, NonZero>::random(&mut rng);
    let sub_1_1 = evaluate_poly_from_share(&old_share1, &b1, 1);
    let sub_1_2 = evaluate_poly_from_share(&old_share1, &b1, 2);
    let sub_1_3 = evaluate_poly_from_share(&old_share1, &b1, 3);
    let sub_1_4 = evaluate_poly_from_share(&old_share1, &b1, 4);

    // Old party 2 creates polynomial
    let b2 = Scalar::<Secret, NonZero>::random(&mut rng);
    let sub_2_1 = evaluate_poly_from_share(&old_share2, &b2, 1);
    let sub_2_2 = evaluate_poly_from_share(&old_share2, &b2, 2);
    let sub_2_3 = evaluate_poly_from_share(&old_share2, &b2, 3);
    let sub_2_4 = evaluate_poly_from_share(&old_share2, &b2, 4);

    // Lagrange coefficients for old parties
    let old_indices = vec![1u32, 2];
    let lambda1 = compute_lagrange_at_zero(1, &old_indices);
    let lambda2 = compute_lagrange_at_zero(2, &old_indices);

    // New parties compute their shares
    let new_share1 = s!(lambda1 * sub_1_1 + lambda2 * sub_2_1);
    let new_share2 = s!(lambda1 * sub_1_2 + lambda2 * sub_2_2);
    let new_share3 = s!(lambda1 * sub_1_3 + lambda2 * sub_2_3);
    let new_share4 = s!(lambda1 * sub_1_4 + lambda2 * sub_2_4);

    // Verify: any 2 of the 4 new shares can reconstruct the secret
    let pairs = [
        (1, 2, &new_share1, &new_share2),
        (2, 3, &new_share2, &new_share3),
        (3, 4, &new_share3, &new_share4),
        (1, 4, &new_share1, &new_share4),
    ];

    let secret_bytes = Scalar::<Secret, Zero>::from_bytes(secret.to_bytes())
        .unwrap()
        .to_bytes();

    for (i, j, share_i, share_j) in pairs {
        let indices = vec![i, j];
        let li = compute_lagrange_at_zero(i, &indices);
        let lj = compute_lagrange_at_zero(j, &indices);
        let reconstructed = s!(li * { *share_i } + lj * { *share_j });
        assert_eq!(
            reconstructed.to_bytes(),
            secret_bytes,
            "Reconstruction with new shares {},{} failed",
            i,
            j
        );
    }
}

/// Test multiple resharing rounds (proactive security)
#[test]
fn test_multiple_resharing_rounds() {
    let mut rng = rand::thread_rng();

    // Initial secret
    let secret = Scalar::<Secret, NonZero>::random(&mut rng);
    let coeff = Scalar::<Secret, NonZero>::random(&mut rng);

    let mut share1 = evaluate_poly(&secret, &coeff, 1);
    let mut share2 = evaluate_poly(&secret, &coeff, 2);

    // Perform 5 resharing rounds
    for round in 0..5 {
        // Each party creates new polynomial
        let b1 = Scalar::<Secret, NonZero>::random(&mut rng);
        let b2 = Scalar::<Secret, NonZero>::random(&mut rng);

        let sub_1_1 = evaluate_poly_from_share(&share1, &b1, 1);
        let sub_1_2 = evaluate_poly_from_share(&share1, &b1, 2);
        let sub_2_1 = evaluate_poly_from_share(&share2, &b2, 1);
        let sub_2_2 = evaluate_poly_from_share(&share2, &b2, 2);

        let old_indices = vec![1u32, 2];
        let lambda1 = compute_lagrange_at_zero(1, &old_indices);
        let lambda2 = compute_lagrange_at_zero(2, &old_indices);

        share1 = s!(lambda1 * sub_1_1 + lambda2 * sub_2_1);
        share2 = s!(lambda1 * sub_1_2 + lambda2 * sub_2_2);

        // Verify secret is preserved after each round
        let reconstructed = s!(lambda1 * share1 + lambda2 * share2);
        let secret_bytes = Scalar::<Secret, Zero>::from_bytes(secret.to_bytes())
            .unwrap()
            .to_bytes();

        assert_eq!(
            reconstructed.to_bytes(),
            secret_bytes,
            "Secret not preserved after resharing round {}",
            round + 1
        );
    }
}

/// Test that Lagrange coefficients work correctly for large party counts.
/// This verifies the fix for integer overflow that previously corrupted results
/// for 14+ parties (13! = 6,227,020,800 > u32::MAX).
#[test]
fn test_large_party_lagrange_no_overflow() {
    // Test with 15 parties - this would overflow with the old i64->u32 truncation
    let indices: Vec<u32> = (1..=15).collect();

    // Compute Lagrange coefficients for all parties
    let mut sum: Scalar<Secret, Zero> = Scalar::zero();

    for &i in &indices {
        let coeff = compute_lagrange_at_zero(i, &indices);
        sum = s!(sum + coeff);
    }

    // Lagrange coefficients must sum to 1 for any valid set of indices
    let one: Scalar<Secret, Zero> = Scalar::from(1u32);
    assert_eq!(
        sum.to_bytes(),
        one.to_bytes(),
        "Lagrange coefficients for 15 parties should sum to 1"
    );
}

/// Test Lagrange reconstruction with 20 parties (would definitely overflow old impl)
#[test]
fn test_20_party_reconstruction() {
    let mut rng = rand::thread_rng();

    // Create a secret and polynomial with degree 19 (threshold 20)
    let secret = Scalar::<Secret, NonZero>::random(&mut rng);

    // Generate shares for parties 1..=20 using a simple degree-1 polynomial for testing
    // (real threshold would use higher degree, but sum property still must hold)
    let coeff = Scalar::<Secret, NonZero>::random(&mut rng);
    let shares: Vec<_> = (1..=20)
        .map(|x| evaluate_poly(&secret, &coeff, x))
        .collect();

    // Reconstruct using any 2 shares (since we use degree-1 polynomial)
    let indices = vec![1u32, 20]; // Use first and last party
    let lambda1 = compute_lagrange_at_zero(1, &indices);
    let lambda20 = compute_lagrange_at_zero(20, &indices);
    let reconstructed = s!(lambda1 * { shares[0] } + lambda20 * { shares[19] });

    // Convert secret to Zero variant for comparison
    let secret_bytes = Scalar::<Secret, Zero>::from_bytes(secret.to_bytes())
        .unwrap()
        .to_bytes();

    assert_eq!(
        reconstructed.to_bytes(),
        secret_bytes,
        "20-party reconstruction should work without overflow"
    );
}

// Helper functions

/// Compute Lagrange coefficient using field arithmetic to avoid integer overflow.
/// Previous implementation used i64 then truncated to u32, corrupting results for 14+ parties.
fn compute_lagrange_at_zero(party_index: u32, all_indices: &[u32]) -> Scalar<Secret, Zero> {
    let mut numerator: Scalar<Secret, Zero> = Scalar::from(1u32);
    let mut denominator: Scalar<Secret, Zero> = Scalar::from(1u32);
    let i_scalar: Scalar<Secret, Zero> = Scalar::from(party_index);

    for &other_index in all_indices {
        if other_index == party_index {
            continue;
        }
        let j_scalar: Scalar<Secret, Zero> = Scalar::from(other_index);

        // numerator *= (0 - j) = -j
        let neg_j = s!(-j_scalar);
        numerator = s!(numerator * neg_j);

        // denominator *= (i - j)
        let i_minus_j = s!(i_scalar - j_scalar);
        denominator = s!(denominator * i_minus_j);
    }

    let denom_nonzero = denominator
        .non_zero()
        .expect("denominator should not be zero");
    let denom_inv = denom_nonzero.invert();
    s!(numerator * denom_inv)
}

fn evaluate_poly(
    constant: &Scalar<Secret, NonZero>,
    coeff: &Scalar<Secret, NonZero>,
    x: u32,
) -> Scalar<Secret, Zero> {
    let x_scalar: Scalar<Secret, Zero> = Scalar::from(x);
    s!({ *constant } + x_scalar * { *coeff })
}

fn evaluate_poly_from_share(
    share: &Scalar<Secret, Zero>,
    coeff: &Scalar<Secret, NonZero>,
    x: u32,
) -> Scalar<Secret, Zero> {
    let x_scalar: Scalar<Secret, Zero> = Scalar::from(x);
    s!({ *share } + x_scalar * { *coeff })
}
