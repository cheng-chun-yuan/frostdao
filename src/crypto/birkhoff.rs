//! Birkhoff Interpolation for Hierarchical Threshold Secret Sharing (HTSS)
//!
//! # Overview
//!
//! HTSS extends traditional threshold signatures (TSS) with a **hierarchy of authority**.
//! Each party has a **rank** (0 = highest authority, higher = lower authority).
//!
//! # TSS vs HTSS
//!
//! ```text
//! TSS (2-of-3):
//!   Any 2 of {Alice, Bob, Carol} can sign
//!   All parties have equal authority
//!
//! HTSS (2-of-3 with ranks):
//!   Party 1 (CEO):     rank 0  ← highest authority
//!   Party 2 (Manager): rank 1
//!   Party 3 (Employee): rank 2  ← lowest authority
//!
//!   Valid signer combinations:
//!   ✅ {CEO, Manager}      - ranks [0,1] → sorted [0,1] → 0≤0, 1≤1 ✓
//!   ✅ {CEO, Employee}     - ranks [0,2] → sorted [0,2] → 0≤0, 2≤1? NO → need 3rd
//!   ✅ {CEO, Manager, Emp} - ranks [0,1,2] → all valid with t=2
//!   ❌ {Manager, Employee} - ranks [1,2] → sorted [1,2] → 1>0 at position 0 ✗
//! ```
//!
//! # The HTSS Validity Rule
//!
//! For threshold `t`, a signer set with ranks `[r₀, r₁, ..., rₖ]` is valid iff:
//! **After sorting ranks ascending: `rank[i] ≤ i` for all i < t**
//!
//! This means:
//! - Position 0 must have rank 0 (need at least one highest-authority party)
//! - Position 1 can have rank 0 or 1
//! - Position i can have rank 0, 1, ..., or i
//!
//! # Mathematical Foundation
//!
//! Birkhoff interpolation generalizes Lagrange by using **derivatives**:
//!
//! ```text
//! Lagrange (rank=0 only):
//!   f(x₁), f(x₂), ..., f(xₜ) → recover f(0) = secret
//!
//! Birkhoff (with ranks):
//!   f^(r₁)(x₁), f^(r₂)(x₂), ..., f^(rₜ)(xₜ) → recover f(0) = secret
//!   where f^(r) denotes the r-th derivative
//! ```
//!
//! When all ranks = 0, Birkhoff reduces to Lagrange (TSS is a special case of HTSS).
//!
//! # Security Model for HTSS Messages
//!
//! ```text
//! | Message Type              | Channel    | Reason                           |
//! |---------------------------|------------|----------------------------------|
//! | Round 1: Commitments      | BROADCAST  | Public, includes rank info       |
//! | Round 2: Secret Shares    | E2E ENCRYPT| Contains derivative evaluations! |
//! | Signing: Nonces           | BROADCAST  | Ephemeral, includes rank         |
//! | Signing: Signature Shares | BROADCAST  | Partial sigs with Birkhoff coeff |
//! ```
//!
//! Round 2 shares in HTSS contain `f^(rank)(index)` - the rank-th derivative of the
//! polynomial evaluated at the party's index. These MUST be encrypted just like TSS.

#![allow(dead_code)] // Birkhoff functions will be used for full HTSS signing integration

use anyhow::{bail, Result};
use nalgebra::DMatrix;
use secp256kfun::prelude::*;
use serde::{Deserialize, Serialize};

/// A Birkhoff parameter represents a share's position in the interpolation.
/// - `x`: The x-coordinate (party index as scalar)
/// - `rank`: The derivative order (0 = value, 1 = first derivative, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BirkhoffParameter {
    pub x: u32,
    pub rank: u32,
}

impl BirkhoffParameter {
    pub fn new(x: u32, rank: u32) -> Self {
        Self { x, rank }
    }
}

/// Validates whether a set of signers can recover the secret in HTSS.
///
/// For threshold `t`, signers with ranks must satisfy:
/// After sorting by rank, `rank[i] <= i` for all positions i.
///
/// # Examples
/// - ranks [0,1,1] with t=3: Valid (0<=0, 1<=1, 1<=2)
/// - ranks [0,1,2] with t=3: Valid (0<=0, 1<=1, 2<=2)
/// - ranks [1,1,2] with t=3: Invalid (1>0 at position 0)
pub fn validate_signer_set(ranks: &[u32], threshold: u32) -> Result<()> {
    if ranks.len() < threshold as usize {
        bail!(
            "Not enough signers: have {} but need at least {}",
            ranks.len(),
            threshold
        );
    }

    // Sort ranks for validation
    let mut sorted_ranks = ranks.to_vec();
    sorted_ranks.sort();

    // Check n_i <= i for all positions
    for (i, &rank) in sorted_ranks.iter().take(threshold as usize).enumerate() {
        if rank > i as u32 {
            bail!(
                "Invalid HTSS signer set: rank {} at position {} violates n_i <= i rule.\n\
                 Sorted ranks: {:?}\n\
                 Hint: You need signers with lower ranks (higher authority) to meet threshold.",
                rank,
                i,
                sorted_ranks
            );
        }
    }

    Ok(())
}

/// Computes the factorial coefficient for Birkhoff matrix entry.
/// Returns n! / (n-k)! = n * (n-1) * ... * (n-k+1)
fn falling_factorial(n: u32, k: u32) -> f64 {
    if k > n {
        return 0.0;
    }
    let mut result = 1.0;
    for i in 0..k {
        result *= (n - i) as f64;
    }
    result
}

/// Builds the Birkhoff coefficient matrix.
///
/// For each parameter (x, rank), row i corresponds to the rank-th derivative
/// evaluated at x. The matrix entry (i, j) is:
///   coefficient = j! / (j-rank)! * x^(j-rank)
///
/// This comes from taking the rank-th derivative of the polynomial term x^j.
fn build_birkhoff_matrix(params: &[BirkhoffParameter]) -> DMatrix<f64> {
    let n = params.len();
    let mut matrix = DMatrix::zeros(n, n);

    for (row, param) in params.iter().enumerate() {
        let x = param.x as f64;
        let rank = param.rank;

        for col in 0..n {
            let degree = col as u32;

            if degree < rank {
                // Derivative is zero if degree < rank
                matrix[(row, col)] = 0.0;
            } else {
                // Coefficient = falling_factorial(degree, rank) * x^(degree-rank)
                let coeff = falling_factorial(degree, rank);
                let power = (degree - rank) as i32;
                let x_power = if power == 0 { 1.0 } else { x.powi(power) };
                matrix[(row, col)] = coeff * x_power;
            }
        }
    }

    matrix
}

/// Computes Birkhoff interpolation coefficients.
///
/// These coefficients, when multiplied by the respective shares and summed,
/// recover the secret (constant term of the polynomial).
///
/// Returns a vector of coefficients, one for each parameter.
pub fn compute_birkhoff_coefficients(params: &[BirkhoffParameter]) -> Result<Vec<f64>> {
    if params.is_empty() {
        bail!("No parameters provided for Birkhoff interpolation");
    }

    let n = params.len();
    let matrix = build_birkhoff_matrix(params);

    // We want to find coefficients c such that sum(c_i * share_i) = secret
    // The secret is the constant term (coefficient of x^0) of the polynomial
    // Using pseudoinverse: c = matrix^(-1) * e_0 where e_0 = [1, 0, 0, ...]

    // Try to compute the inverse (or pseudoinverse for robustness)
    let svd = matrix.svd(true, true);

    // Check if matrix is singular
    let tolerance = 1e-10;
    let rank = svd
        .singular_values
        .iter()
        .filter(|&&s| s > tolerance)
        .count();

    if rank < n {
        bail!(
            "Birkhoff matrix is singular (rank {} < {}). \
             This means the signer set cannot recover the secret. \
             Check that ranks satisfy the HTSS validity rule.",
            rank,
            n
        );
    }

    // Compute pseudoinverse
    let pseudo_inverse = svd
        .pseudo_inverse(tolerance)
        .map_err(|e| anyhow::anyhow!("Failed to compute pseudoinverse: {}", e))?;

    // Extract first row (coefficients for recovering constant term a_0)
    // The inverse maps [y_1, y_2, ..., y_n] -> [a_0, a_1, ..., a_{n-1}]
    // So a_0 = first_row(V^{-1}) · [y_1, ..., y_n]
    let mut coefficients = Vec::with_capacity(n);
    for j in 0..n {
        coefficients.push(pseudo_inverse[(0, j)]);
    }

    Ok(coefficients)
}

/// Computes the Birkhoff coefficient for a single party in a signing session.
///
/// This is the multiplier that should be applied to a party's signature share
/// to correctly combine shares in HTSS mode.
pub fn compute_single_birkhoff_coefficient(
    my_index: u32,
    _my_rank: u32,
    all_signers: &[(u32, u32)], // (index, rank) pairs
) -> Result<f64> {
    // Find my position in the signer list
    let my_position = all_signers
        .iter()
        .position(|(idx, _)| *idx == my_index)
        .ok_or_else(|| anyhow::anyhow!("My index {} not found in signers list", my_index))?;

    // Build parameters
    let params: Vec<BirkhoffParameter> = all_signers
        .iter()
        .map(|(x, rank)| BirkhoffParameter::new(*x, *rank))
        .collect();

    // Compute all coefficients
    let coefficients = compute_birkhoff_coefficients(&params)?;

    Ok(coefficients[my_position])
}

/// Converts a floating-point Birkhoff coefficient to a Scalar.
///
/// Since we work in a finite field, we need to handle the conversion carefully.
/// The coefficient is first scaled and then converted to an integer representation.
pub fn coefficient_to_scalar(coeff: f64) -> Scalar<Secret, Zero> {
    // Handle sign
    let is_negative = coeff < 0.0;
    let abs_coeff = coeff.abs();

    // Scale to get more precision (we'll use fixed-point arithmetic)
    // The scaling factor should be inverted when computing the final result
    const SCALE: f64 = 1e18;
    let scaled = (abs_coeff * SCALE).round() as u64;

    // Create scalar from the scaled value
    let mut scalar = Scalar::from(scaled);

    if is_negative {
        scalar = -scalar;
    }

    // Note: The caller needs to divide by SCALE in the field
    // This is done by multiplying by the modular inverse of SCALE
    scalar
}

/// Computes Lagrange coefficient for standard TSS (when all ranks are 0).
///
/// This is provided for comparison and backwards compatibility.
/// λ_i = Π_{j≠i} (x_j / (x_j - x_i))
pub fn compute_lagrange_coefficient(my_index: u32, all_indices: &[u32]) -> f64 {
    let xi = my_index as f64;
    let mut lambda = 1.0;

    for &idx in all_indices {
        if idx != my_index {
            let xj = idx as f64;
            lambda *= xj / (xj - xi);
        }
    }

    lambda
}

/// Computes Birkhoff interpolation coefficients for recovering a specific point.
///
/// Given helper shares with their (index, rank) pairs, this computes coefficients
/// that allow recovering the `target_rank`-th derivative evaluated at `target_x`.
///
/// For recovery of a lost party's share:
/// - `target_x`: The lost party's index
/// - `target_rank`: The lost party's rank (0 for value, 1 for first derivative, etc.)
/// - `params`: The helper parties' (index, rank) pairs
///
/// Returns coefficients such that:
/// `recovered_value = sum(coeff[i] * helper_share[i])`
pub fn compute_birkhoff_recovery_coefficients(
    target_x: u32,
    target_rank: u32,
    params: &[BirkhoffParameter],
) -> Result<Vec<f64>> {
    if params.is_empty() {
        bail!("No parameters provided for Birkhoff interpolation");
    }

    let n = params.len();
    let matrix = build_birkhoff_matrix(params);

    // Compute pseudoinverse
    let svd = matrix.svd(true, true);
    let tolerance = 1e-10;
    let rank = svd
        .singular_values
        .iter()
        .filter(|&&s| s > tolerance)
        .count();

    if rank < n {
        bail!(
            "Birkhoff matrix is singular (rank {} < {}). \
             This means the helper set cannot recover the target share. \
             Check that ranks satisfy the HTSS validity rule.",
            rank,
            n
        );
    }

    let pseudo_inverse = svd
        .pseudo_inverse(tolerance)
        .map_err(|e| anyhow::anyhow!("Failed to compute pseudoinverse: {}", e))?;

    // Build evaluation vector for f^{(target_rank)}(target_x)
    // Entry j = falling_factorial(j, target_rank) * target_x^(j - target_rank)
    let x = target_x as f64;
    let mut eval_vector = Vec::with_capacity(n);
    for j in 0..n {
        let degree = j as u32;
        if degree < target_rank {
            eval_vector.push(0.0);
        } else {
            let coeff = falling_factorial(degree, target_rank);
            let power = (degree - target_rank) as i32;
            let x_power = if power == 0 { 1.0 } else { x.powi(power) };
            eval_vector.push(coeff * x_power);
        }
    }

    // Recovery coefficients = eval_vector · pseudo_inverse
    // This gives us coefficients for each helper share
    let mut coefficients = Vec::with_capacity(n);
    for j in 0..n {
        let mut sum = 0.0;
        for k in 0..n {
            sum += eval_vector[k] * pseudo_inverse[(k, j)];
        }
        coefficients.push(sum);
    }

    Ok(coefficients)
}

/// Converts Birkhoff coefficient to field scalar with proper precision.
///
/// Uses scaling internally for non-integer coefficients and applies
/// the modular inverse to produce correct field elements.
pub fn birkhoff_coefficient_to_scalar(coeff: f64) -> Scalar<Secret, Zero> {
    // For coefficients that are likely rational, round to nearest rational
    // with small denominator and convert
    let is_negative = coeff < 0.0;
    let abs_coeff = coeff.abs();

    // Check for small integer (most common case - avoids scaling overhead)
    let rounded = abs_coeff.round();
    if (abs_coeff - rounded).abs() < 1e-9 && rounded < 1e9 {
        let value = rounded as u64;
        let scalar: Scalar<Secret, Zero> = Scalar::from(value);
        return if is_negative { s!(-scalar) } else { scalar };
    }

    // For non-integer coefficients, use scaling approach
    // Scale up to preserve precision, then divide by SCALE in the field
    const SCALE: u64 = 1_000_000_000_000; // 10^12

    let scaled = (abs_coeff * SCALE as f64).round() as u64;
    let scaled_scalar: Scalar<Secret, Zero> = Scalar::from(scaled);

    // Compute modular inverse of SCALE and multiply to get correct coefficient
    // This is equivalent to dividing by SCALE in the finite field
    let scale_scalar: Scalar<Secret, Zero> = Scalar::from(SCALE);
    let scale_nonzero = scale_scalar.non_zero().expect("SCALE is non-zero constant");
    let scale_inverse = scale_nonzero.invert();

    // result = scaled_value / SCALE = scaled_value * SCALE^(-1)
    let result = s!(scaled_scalar * scale_inverse);

    if is_negative {
        s!(-result)
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_signer_set_valid() {
        // Valid: ranks [0,1,1] for threshold 3
        assert!(validate_signer_set(&[0, 1, 1], 3).is_ok());

        // Valid: ranks [0,1,2] for threshold 3
        assert!(validate_signer_set(&[0, 1, 2], 3).is_ok());

        // Valid: ranks [0,0,0] for threshold 3 (classical TSS)
        assert!(validate_signer_set(&[0, 0, 0], 3).is_ok());

        // Valid: more signers than threshold
        assert!(validate_signer_set(&[0, 1, 1, 2], 3).is_ok());
    }

    #[test]
    fn test_validate_signer_set_invalid() {
        // Invalid: ranks [1,1,2] for threshold 3 (rank 1 > position 0)
        assert!(validate_signer_set(&[1, 1, 2], 3).is_err());

        // Invalid: ranks [2,2,2] for threshold 3
        assert!(validate_signer_set(&[2, 2, 2], 3).is_err());

        // Invalid: not enough signers
        assert!(validate_signer_set(&[0, 1], 3).is_err());
    }

    #[test]
    fn test_birkhoff_reduces_to_lagrange() {
        // When all ranks are 0, Birkhoff should equal Lagrange
        let params = vec![
            BirkhoffParameter::new(1, 0),
            BirkhoffParameter::new(2, 0),
            BirkhoffParameter::new(3, 0),
        ];

        let birkhoff_coeffs = compute_birkhoff_coefficients(&params).unwrap();

        for (i, param) in params.iter().enumerate() {
            let lagrange_coeff = compute_lagrange_coefficient(param.x, &[1, 2, 3]);
            println!(
                "Party {}: Birkhoff={:.6}, Lagrange={:.6}",
                param.x, birkhoff_coeffs[i], lagrange_coeff
            );
            let diff = (birkhoff_coeffs[i] - lagrange_coeff).abs();
            assert!(
                diff < 1e-6,
                "Birkhoff ({}) should equal Lagrange ({}) when all ranks are 0",
                birkhoff_coeffs[i],
                lagrange_coeff
            );
        }
    }

    #[test]
    fn test_falling_factorial() {
        assert_eq!(falling_factorial(5, 0), 1.0);
        assert_eq!(falling_factorial(5, 1), 5.0);
        assert_eq!(falling_factorial(5, 2), 20.0); // 5 * 4
        assert_eq!(falling_factorial(5, 3), 60.0); // 5 * 4 * 3
        assert_eq!(falling_factorial(3, 5), 0.0); // k > n
    }

    #[test]
    fn test_birkhoff_recovery_at_different_x() {
        // When all ranks are 0, recovery coefficients should match Lagrange at target_x
        // For helpers at x=1, x=2 recovering x=3:
        // λ_1(3) = (3-2)/(1-2) = -1
        // λ_2(3) = (3-1)/(2-1) = 2
        let params = vec![BirkhoffParameter::new(1, 0), BirkhoffParameter::new(2, 0)];

        let coeffs = compute_birkhoff_recovery_coefficients(3, 0, &params).unwrap();

        println!(
            "Recovery at x=3 from x=1,2: [{:.6}, {:.6}]",
            coeffs[0], coeffs[1]
        );

        // λ_1(3) should be -1
        assert!(
            (coeffs[0] - (-1.0)).abs() < 1e-6,
            "λ_1(3) should be -1, got {}",
            coeffs[0]
        );

        // λ_2(3) should be 2
        assert!(
            (coeffs[1] - 2.0).abs() < 1e-6,
            "λ_2(3) should be 2, got {}",
            coeffs[1]
        );

        // Sum should be 1 (fundamental Lagrange property)
        let sum: f64 = coeffs.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-6,
            "Recovery coefficients should sum to 1, got {}",
            sum
        );
    }

    #[test]
    fn test_birkhoff_recovery_with_mixed_ranks() {
        // HTSS with mixed ranks: recover a rank-0 share using rank-0 and rank-1 helpers
        // Helper 1: index=1, rank=0 (has polynomial value f(1))
        // Helper 2: index=2, rank=1 (has first derivative f'(2))
        // Target: index=3, rank=0 (want to recover f(3))
        let params = vec![
            BirkhoffParameter::new(1, 0), // f(1)
            BirkhoffParameter::new(2, 1), // f'(2)
        ];

        let coeffs = compute_birkhoff_recovery_coefficients(3, 0, &params);
        assert!(
            coeffs.is_ok(),
            "Should be able to compute recovery coefficients for valid HTSS helper set"
        );

        let coeffs = coeffs.unwrap();
        println!(
            "HTSS recovery at x=3,rank=0 from (1,0),(2,1): [{:.6}, {:.6}]",
            coeffs[0], coeffs[1]
        );

        // Verify coefficients are finite and reasonable
        for (i, c) in coeffs.iter().enumerate() {
            assert!(
                c.is_finite(),
                "Coefficient {} should be finite, got {}",
                i,
                c
            );
        }
    }

    #[test]
    fn test_birkhoff_recovery_rank1_target() {
        // Recover a rank-1 share (derivative) using rank-0 helpers
        // Helper 1: index=1, rank=0 (has f(1))
        // Helper 2: index=2, rank=0 (has f(2))
        // Target: index=3, rank=1 (want to recover f'(3))
        let params = vec![BirkhoffParameter::new(1, 0), BirkhoffParameter::new(2, 0)];

        let coeffs = compute_birkhoff_recovery_coefficients(3, 1, &params).unwrap();
        println!(
            "Recover f'(3) from f(1),f(2): [{:.6}, {:.6}]",
            coeffs[0], coeffs[1]
        );

        // For a degree-1 polynomial f(x) = a + bx:
        // f(1) = a + b, f(2) = a + 2b
        // The derivative f'(x) = b (constant)
        // So f'(3) = b
        // Solve: c1*(a+b) + c2*(a+2b) = b
        // (c1+c2)*a + (c1+2*c2)*b = b
        // c1 + c2 = 0 and c1 + 2*c2 = 1
        // => c2 = 1, c1 = -1
        assert!(
            (coeffs[0] - (-1.0)).abs() < 1e-6,
            "Expected c1=-1, got {}",
            coeffs[0]
        );
        assert!(
            (coeffs[1] - 1.0).abs() < 1e-6,
            "Expected c2=1, got {}",
            coeffs[1]
        );
    }

    #[test]
    fn test_birkhoff_coefficient_to_scalar_integer() {
        // Integer coefficients should work directly
        let coeff = birkhoff_coefficient_to_scalar(3.0);
        let three: Scalar<Secret, Zero> = Scalar::from(3u32);
        assert_eq!(coeff.to_bytes(), three.to_bytes());

        // Negative integer
        let neg_coeff = birkhoff_coefficient_to_scalar(-2.0);
        let two: Scalar<Secret, Zero> = Scalar::from(2u32);
        let neg_two = s!(-two);
        assert_eq!(neg_coeff.to_bytes(), neg_two.to_bytes());
    }

    #[test]
    fn test_birkhoff_coefficient_to_scalar_fraction() {
        // Test that 0.5 * 2 = 1 in the field
        // This verifies the modular inverse is applied correctly
        let half = birkhoff_coefficient_to_scalar(0.5);
        let two: Scalar<Secret, Zero> = Scalar::from(2u32);
        let result = s!(half * two);

        let one: Scalar<Secret, Zero> = Scalar::from(1u32);
        assert_eq!(
            result.to_bytes(),
            one.to_bytes(),
            "0.5 * 2 should equal 1 in the field"
        );
    }

    #[test]
    fn test_birkhoff_coefficient_to_scalar_quarter() {
        // Test that 0.25 * 4 = 1 in the field
        // Using 0.25 because it's exactly representable in binary floating point
        let quarter = birkhoff_coefficient_to_scalar(0.25);
        let four: Scalar<Secret, Zero> = Scalar::from(4u32);
        let result = s!(quarter * four);

        let one: Scalar<Secret, Zero> = Scalar::from(1u32);
        assert_eq!(
            result.to_bytes(),
            one.to_bytes(),
            "0.25 * 4 should equal 1 in the field"
        );
    }

    #[test]
    fn test_birkhoff_coefficient_negative_fraction() {
        // Test that -0.5 * -2 = 1 in the field
        let neg_half = birkhoff_coefficient_to_scalar(-0.5);
        let neg_two: Scalar<Secret, Zero> = {
            let two: Scalar<Secret, Zero> = Scalar::from(2u32);
            s!(-two)
        };
        let result = s!(neg_half * neg_two);

        let one: Scalar<Secret, Zero> = Scalar::from(1u32);
        assert_eq!(
            result.to_bytes(),
            one.to_bytes(),
            "-0.5 * -2 should equal 1 in the field"
        );
    }

    #[test]
    fn test_birkhoff_htss_recovery_correctness() {
        // Simulate HTSS recovery with mixed ranks
        // Helper 1: index=1, rank=0 (has f(1))
        // Helper 2: index=2, rank=1 (has f'(2))
        // Target: index=3, rank=0 (recover f(3))

        // For a degree-1 polynomial f(x) = a + bx:
        // f(1) = a + b
        // f'(2) = b (derivative is constant)
        // f(3) = a + 3b

        // Using Birkhoff to recover f(3):
        // We need coefficients c1, c2 such that:
        // c1 * f(1) + c2 * f'(2) = f(3)
        // c1 * (a + b) + c2 * b = a + 3b
        // c1*a + c1*b + c2*b = a + 3b
        // c1 = 1 (coefficient of a)
        // c1 + c2 = 3 => c2 = 2

        let params = vec![
            BirkhoffParameter::new(1, 0), // f(1)
            BirkhoffParameter::new(2, 1), // f'(2)
        ];

        let coeffs = compute_birkhoff_recovery_coefficients(3, 0, &params).unwrap();

        // Verify coefficients
        assert!(
            (coeffs[0] - 1.0).abs() < 1e-6,
            "c1 should be 1, got {}",
            coeffs[0]
        );
        assert!(
            (coeffs[1] - 2.0).abs() < 1e-6,
            "c2 should be 2, got {}",
            coeffs[1]
        );

        // Now test the full recovery with actual scalars
        let mut rng = rand::thread_rng();
        let a = Scalar::<Secret, NonZero>::random(&mut rng);
        let b = Scalar::<Secret, NonZero>::random(&mut rng);

        // Compute shares
        let one: Scalar<Secret, Zero> = Scalar::from(1u32);
        let three: Scalar<Secret, Zero> = Scalar::from(3u32);

        let f_1 = s!(a + one * b); // f(1) = a + b
        let f_prime_2 = s!(b); // f'(2) = b (constant for linear polynomial)
        let f_3_expected = s!(a + three * b); // f(3) = a + 3b

        // Recover using Birkhoff coefficients
        let c1 = birkhoff_coefficient_to_scalar(coeffs[0]);
        let c2 = birkhoff_coefficient_to_scalar(coeffs[1]);

        let f_3_recovered = s!(c1 * f_1 + c2 * f_prime_2);

        assert_eq!(
            f_3_recovered.to_bytes(),
            f_3_expected.to_bytes(),
            "Birkhoff recovery should produce correct f(3)"
        );
    }
}
