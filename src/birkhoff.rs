//! Birkhoff Interpolation for Hierarchical Threshold Secret Sharing (HTSS)
//!
//! This module implements Birkhoff interpolation, which extends Lagrange interpolation
//! by incorporating derivative information (ranks). In HTSS, each share has both an
//! x-coordinate and a rank (derivative order).
//!
//! When all ranks are 0, Birkhoff interpolation reduces to Lagrange interpolation,
//! making HTSS a generalization of classical TSS.

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
    let pseudo_inverse = svd.pseudo_inverse(tolerance).map_err(|e| {
        anyhow::anyhow!("Failed to compute pseudoinverse: {}", e)
    })?;

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
            let lagrange_coeff =
                compute_lagrange_coefficient(param.x, &[1, 2, 3]);
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
}
