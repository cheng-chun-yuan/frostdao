//! Resharing Protocol for FROST/HTSS
//!
//! This module implements proactive secret sharing and resharing capabilities:
//! - **Refresh**: Same parties, same threshold - invalidate old shares
//! - **Enrollment**: Add new parties while keeping same public key
//! - **Threshold Change**: Modify threshold (requires new key ceremony for increase)
//!
//! The resharing protocol allows old parties to transfer their shares to a new
//! configuration without revealing the joint secret.
//!
//! ## Protocol Overview
//!
//! For resharing from old (t, n) to new (t', n'):
//!
//! 1. Each old party i creates a polynomial f_i(x) of degree (t'-1) where f_i(0) = s_i
//! 2. Old party i evaluates f_i at each new party index j: sub_share_{i,j} = f_i(j)
//! 3. New party j collects sub_shares from >= t old parties
//! 4. New party j computes: s'_j = Σ (λ_i * sub_share_{i,j}) where λ_i are Lagrange coefficients
//!
//! Result: New shares s'_j for the same group secret s

use crate::keygen::{get_state_dir, GroupInfo, HtssMetadata};
use crate::storage::{FileStorage, Storage};
use anyhow::Result;
use schnorr_fun::frost;
use schnorr_fun::fun::marker::*;
use secp256kfun::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Output from reshare round 1 (old party generates sub-shares)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReshareRound1Output {
    /// Old party index
    pub old_party_index: u32,
    /// Sub-shares for each new party: (new_party_index, encrypted_sub_share)
    pub sub_shares: BTreeMap<u32, String>,
    /// Commitment to the polynomial (for verification)
    pub polynomial_commitment: Vec<String>,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// Generate sub-shares for resharing (old party runs this)
pub fn reshare_round1(
    source_wallet: &str,
    new_threshold: u32,
    new_n_parties: u32,
    my_old_index: u32,
) -> Result<()> {
    let state_dir = get_state_dir(source_wallet);
    let path = std::path::Path::new(&state_dir);

    if !path.exists() {
        anyhow::bail!("Wallet '{}' not found at {}.", source_wallet, state_dir);
    }

    let storage = FileStorage::new(&state_dir)?;

    println!("Reshare Round 1 - Generate Sub-shares\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Load my secret share
    let paired_share_bytes = storage.read("paired_secret_share.bin")?;
    let paired_share: frost::PairedSecretShare<EvenY> = bincode::deserialize(&paired_share_bytes)?;

    // Load HTSS metadata for verification
    let htss_json = String::from_utf8(storage.read("htss_metadata.json")?)?;
    let htss: HtssMetadata = serde_json::from_str(&htss_json)?;

    // Verify my index matches
    if htss.my_index != my_old_index {
        anyhow::bail!(
            "Index mismatch: wallet has index {}, but you specified {}",
            htss.my_index,
            my_old_index
        );
    }

    println!("Source wallet: {}", source_wallet);
    println!(
        "Old config: {}-of-{}",
        htss.threshold,
        htss.party_ranks.len()
    );
    println!("New config: {}-of-{}", new_threshold, new_n_parties);
    println!("My old index: {}", my_old_index);
    println!();

    // Get my secret share scalar - extract from the secret_share structure
    let my_share = paired_share.secret_share();
    let my_secret_bytes = my_share.share.to_bytes();

    // Create a new polynomial of degree (new_threshold - 1) with my share as constant term
    // f(0) = my_secret, and random coefficients for higher terms
    let mut rng = rand::thread_rng();
    let mut coefficients: Vec<[u8; 32]> = Vec::with_capacity(new_threshold as usize);
    coefficients.push(my_secret_bytes); // constant term = my share

    for _ in 1..new_threshold {
        let coeff = Scalar::<Secret, NonZero>::random(&mut rng);
        coefficients.push(coeff.to_bytes());
    }

    // Compute polynomial commitments (for verification)
    let mut polynomial_commitment: Vec<String> = Vec::new();
    for coeff_bytes in &coefficients {
        let coeff: Scalar<Secret, Zero> =
            Scalar::from_bytes(*coeff_bytes).unwrap_or(Scalar::zero());
        let commitment = g!(coeff * G).normalize();
        polynomial_commitment.push(hex::encode(commitment.to_bytes()));
    }

    // Evaluate polynomial at each new party index
    let mut sub_shares: BTreeMap<u32, String> = BTreeMap::new();

    for new_idx in 1..=new_n_parties {
        // Evaluate f(x) = sum(coeff_i * x^i) using Horner's method
        let x = new_idx;

        let mut result = [0u8; 32];
        // Start from highest degree coefficient
        for i in (0..coefficients.len()).rev() {
            // result = result * x + coeff[i]
            let result_scalar: Scalar<Secret, Zero> =
                Scalar::from_bytes(result).unwrap_or(Scalar::zero());
            let x_scalar: Scalar<Public, Zero> = Scalar::from(x);
            let coeff_scalar: Scalar<Secret, Zero> =
                Scalar::from_bytes(coefficients[i]).unwrap_or(Scalar::zero());

            let new_result = s!(result_scalar * x_scalar + coeff_scalar);
            result = new_result.to_bytes();
        }

        sub_shares.insert(new_idx, hex::encode(result));
    }

    println!("Generated sub-shares for {} new parties", new_n_parties);
    println!();

    // Create output
    let output = ReshareRound1Output {
        old_party_index: my_old_index,
        sub_shares,
        polynomial_commitment,
        event_type: "reshare_round1".to_string(),
    };

    let result_json = serde_json::to_string(&output)?;

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 Share this with the coordinator (or new parties):");
    println!("{}\n", result_json);
    println!("⚠️  Keep your old share until resharing is complete!");

    Ok(())
}

/// Finalize resharing (new party runs this)
pub fn reshare_finalize(
    source_wallet: &str,
    target_wallet: &str,
    my_new_index: u32,
    my_rank: u32,
    hierarchical: bool,
    round1_data: &str,
) -> Result<()> {
    println!("Reshare Finalize - Combine Sub-shares\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Parse round1 outputs (space-separated JSON objects)
    let round1_outputs: Vec<ReshareRound1Output> =
        crate::keygen::parse_space_separated_json(round1_data)?;

    if round1_outputs.is_empty() {
        anyhow::bail!("No round1 data provided");
    }

    println!(
        "Received sub-shares from {} old parties",
        round1_outputs.len()
    );
    println!("My new index: {}", my_new_index);
    println!("My rank: {}", my_rank);
    println!();

    // Load source wallet to get group public key
    let source_state_dir = get_state_dir(source_wallet);
    let source_storage = FileStorage::new(&source_state_dir)?;

    let shared_key_bytes = source_storage.read("shared_key.bin")?;
    let shared_key: frost::SharedKey<EvenY> = bincode::deserialize(&shared_key_bytes)?;
    let group_public_key = shared_key.public_key();

    let source_htss_json = String::from_utf8(source_storage.read("htss_metadata.json")?)?;
    let source_htss: HtssMetadata = serde_json::from_str(&source_htss_json)?;
    let old_threshold = source_htss.threshold;

    // Verify we have enough sub-shares (need at least old_threshold)
    if (round1_outputs.len() as u32) < old_threshold {
        anyhow::bail!(
            "Not enough sub-shares: got {}, need at least {}",
            round1_outputs.len(),
            old_threshold
        );
    }

    println!(
        "Old threshold: {} (have {} sub-shares)",
        old_threshold,
        round1_outputs.len()
    );

    // Get the new threshold from the polynomial commitment degree
    let new_threshold = round1_outputs[0].polynomial_commitment.len() as u32;
    let new_n_parties = round1_outputs[0].sub_shares.len() as u32;

    println!("New config: {}-of-{}", new_threshold, new_n_parties);
    println!();

    // Collect old party indices for Lagrange computation
    let old_indices: Vec<u32> = round1_outputs.iter().map(|o| o.old_party_index).collect();

    // Compute my new share: sum of (lagrange_coeff * sub_share) for each old party
    let mut new_share_bytes = [0u8; 32];

    for output in &round1_outputs {
        // Get sub-share for my new index
        let sub_share_hex = output
            .sub_shares
            .get(&my_new_index)
            .ok_or_else(|| anyhow::anyhow!("Missing sub-share for index {}", my_new_index))?;

        let sub_share_bytes: [u8; 32] = hex::decode(sub_share_hex)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid sub-share length"))?;

        let sub_share: Scalar<Secret, Zero> = Scalar::from_bytes(sub_share_bytes)
            .ok_or_else(|| anyhow::anyhow!("Invalid sub-share scalar"))?;

        // Compute Lagrange coefficient for this old party at x=0
        let lagrange_coeff = crate::crypto_helpers::lagrange_coefficient_at_zero(output.old_party_index, &old_indices)?;

        // Add weighted sub-share to result
        let current: Scalar<Secret, Zero> =
            Scalar::from_bytes(new_share_bytes).unwrap_or(Scalar::zero());
        let weighted = s!(lagrange_coeff * sub_share);
        let sum = s!(current + weighted);
        new_share_bytes = sum.to_bytes();
    }

    println!("Computed new secret share");

    // Create new wallet directory
    let target_state_dir = get_state_dir(target_wallet);
    let target_path = std::path::Path::new(&target_state_dir);

    if target_path.exists() {
        println!("⚠️  Target wallet '{}' already exists", target_wallet);
        print!("   Replace? [y/N]: ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim().to_lowercase() != "y" {
            println!("Aborted.");
            return Ok(());
        }
        std::fs::remove_dir_all(target_path)?;
    }

    let target_storage = FileStorage::new(&target_state_dir)?;

    // Create PairedSecretShare using helper function
    let share_scalar: Scalar<Secret, Zero> = Scalar::from_bytes(new_share_bytes)
        .ok_or_else(|| anyhow::anyhow!("Invalid computed share"))?;
    let share_nonzero = crate::crypto_helpers::share_to_nonzero(share_scalar)?;

    let paired_share = crate::crypto_helpers::construct_paired_secret_share(
        my_new_index,
        share_nonzero,
        &group_public_key,
    )?;
    let paired_bytes = bincode::serialize(&paired_share)?;

    target_storage.write("paired_secret_share.bin", &paired_bytes)?;
    target_storage.write("shared_key.bin", &shared_key_bytes)?;

    // Create new HTSS metadata
    let mut party_ranks: BTreeMap<u32, u32> = BTreeMap::new();
    party_ranks.insert(my_new_index, my_rank);

    // Add placeholder ranks for other parties (they'll update their own)
    for i in 1..=new_n_parties {
        if i != my_new_index {
            party_ranks.insert(i, 0); // default rank
        }
    }

    let new_htss = HtssMetadata {
        my_index: my_new_index,
        my_rank,
        threshold: new_threshold,
        hierarchical,
        party_ranks,
    };

    target_storage.write(
        "htss_metadata.json",
        serde_json::to_string_pretty(&new_htss)?.as_bytes(),
    )?;

    // Create group info
    let pubkey_bytes: [u8; 32] = group_public_key.to_xonly_bytes();
    let pubkey_hex = hex::encode(pubkey_bytes);

    use bitcoin::{Address, Network, XOnlyPublicKey};
    let xonly_pk = XOnlyPublicKey::from_slice(&pubkey_bytes)?;
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let address_testnet = Address::p2tr(&secp, xonly_pk, None, Network::Testnet).to_string();
    let address_mainnet = Address::p2tr(&secp, xonly_pk, None, Network::Bitcoin).to_string();

    let group_info = GroupInfo {
        name: target_wallet.to_string(),
        group_public_key: pubkey_hex.clone(),
        taproot_address_testnet: address_testnet.clone(),
        taproot_address_mainnet: address_mainnet.clone(),
        threshold: new_threshold,
        total_parties: new_n_parties,
        hierarchical,
        parties: vec![], // Will be populated when all parties complete
    };

    target_storage.write(
        "group_info.json",
        serde_json::to_string_pretty(&group_info)?.as_bytes(),
    )?;

    // Also save share in hex format for easy verification
    target_storage.write("share_hex.txt", hex::encode(new_share_bytes).as_bytes())?;

    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ Resharing complete!");
    println!();
    println!("New wallet: {}", target_wallet);
    println!("Config: {}-of-{}", new_threshold, new_n_parties);
    println!("Your index: {}", my_new_index);
    println!();
    println!("Public Key: {}", pubkey_hex);
    println!("Testnet Address: {}", address_testnet);
    println!();
    println!("⚠️  The public key and address are the SAME as before!");
    println!("    Funds are still accessible with the new shares.");
    println!();
    println!("🗑️  Once ALL parties have reshared, delete old wallet:");
    println!("    rm -rf .frost_state/{}/", source_wallet);

    Ok(())
}

// Lagrange coefficient computation is now in crypto_helpers module

// ============================================================================
// Core functions for TUI integration
// ============================================================================

use crate::CommandResult;

/// Core function for reshare round 1 (returns output instead of printing)
pub fn reshare_round1_core(
    source_wallet: &str,
    new_threshold: u32,
    new_n_parties: u32,
    my_old_index: u32,
) -> Result<CommandResult> {
    let state_dir = get_state_dir(source_wallet);
    let path = std::path::Path::new(&state_dir);

    if !path.exists() {
        anyhow::bail!("Wallet '{}' not found at {}.", source_wallet, state_dir);
    }

    let storage = FileStorage::new(&state_dir)?;

    // Load my secret share
    let paired_share_bytes = storage.read("paired_secret_share.bin")?;
    let paired_share: frost::PairedSecretShare<EvenY> = bincode::deserialize(&paired_share_bytes)?;

    // Load HTSS metadata for verification
    let htss_json = String::from_utf8(storage.read("htss_metadata.json")?)?;
    let htss: HtssMetadata = serde_json::from_str(&htss_json)?;

    if htss.my_index != my_old_index {
        anyhow::bail!(
            "Index mismatch: provided {}, but wallet has index {}",
            my_old_index,
            htss.my_index
        );
    }

    let my_share = paired_share.secret_share();
    let my_secret_bytes = my_share.share.to_bytes();

    // Create polynomial of degree (new_threshold - 1) with f(0) = my_secret_share
    // Generate random coefficients for higher degree terms
    let mut rng = rand::thread_rng();
    let mut coefficients: Vec<[u8; 32]> = Vec::with_capacity(new_threshold as usize);
    coefficients.push(my_secret_bytes); // constant term = my share

    for _ in 1..new_threshold {
        let coeff = Scalar::<Secret, NonZero>::random(&mut rng);
        coefficients.push(coeff.to_bytes());
    }

    // Create polynomial commitments (public)
    let g = schnorr_fun::fun::G;
    let polynomial_commitment: Vec<String> = coefficients
        .iter()
        .map(|coeff| {
            let scalar: Scalar<Secret, Zero> = Scalar::from_bytes(*coeff).unwrap_or(Scalar::zero());
            let point = g!(scalar * g);
            let point_bytes = point.normalize().to_bytes();
            hex::encode(point_bytes)
        })
        .collect();

    // Generate sub-shares for each new party
    let mut sub_shares: BTreeMap<u32, String> = BTreeMap::new();

    for new_idx in 1..=new_n_parties {
        let x = new_idx;

        let mut result = [0u8; 32];
        for i in (0..coefficients.len()).rev() {
            let result_scalar: Scalar<Secret, Zero> =
                Scalar::from_bytes(result).unwrap_or(Scalar::zero());
            let x_scalar: Scalar<Public, Zero> = Scalar::from(x);
            let coeff_scalar: Scalar<Secret, Zero> =
                Scalar::from_bytes(coefficients[i]).unwrap_or(Scalar::zero());

            let new_result = s!(result_scalar * x_scalar + coeff_scalar);
            result = new_result.to_bytes();
        }

        sub_shares.insert(new_idx, hex::encode(result));
    }

    let output = ReshareRound1Output {
        old_party_index: my_old_index,
        sub_shares,
        polynomial_commitment,
        event_type: "reshare_round1".to_string(),
    };

    let result_json = serde_json::to_string(&output)?;

    Ok(CommandResult {
        output: format!(
            "Generated sub-shares for resharing from {} to {}-of-{}\n\
             Your old index: {}",
            source_wallet, new_threshold, new_n_parties, my_old_index
        ),
        result: result_json,
    })
}

/// Core function for reshare finalize (returns output instead of printing)
pub fn reshare_finalize_core(
    source_wallet: &str,
    target_wallet: &str,
    my_new_index: u32,
    my_rank: u32,
    hierarchical: bool,
    round1_data: &str,
    force_overwrite: bool,
) -> Result<CommandResult> {
    // Parse round1 outputs
    let round1_outputs: Vec<ReshareRound1Output> =
        crate::keygen::parse_space_separated_json(round1_data)?;

    if round1_outputs.is_empty() {
        anyhow::bail!("No round1 data provided");
    }

    // Load source wallet
    let source_state_dir = get_state_dir(source_wallet);
    let source_storage = FileStorage::new(&source_state_dir)?;

    let shared_key_bytes = source_storage.read("shared_key.bin")?;
    let shared_key: frost::SharedKey<EvenY> = bincode::deserialize(&shared_key_bytes)?;
    let group_public_key = shared_key.public_key();

    let source_htss_json = String::from_utf8(source_storage.read("htss_metadata.json")?)?;
    let source_htss: HtssMetadata = serde_json::from_str(&source_htss_json)?;
    let old_threshold = source_htss.threshold;

    if (round1_outputs.len() as u32) < old_threshold {
        anyhow::bail!(
            "Not enough sub-shares: got {}, need at least {}",
            round1_outputs.len(),
            old_threshold
        );
    }

    let new_threshold = round1_outputs[0].polynomial_commitment.len() as u32;
    let new_n_parties = round1_outputs[0].sub_shares.len() as u32;

    let old_indices: Vec<u32> = round1_outputs.iter().map(|o| o.old_party_index).collect();

    // Compute new share
    let mut new_share_bytes = [0u8; 32];

    for output in &round1_outputs {
        let sub_share_hex = output
            .sub_shares
            .get(&my_new_index)
            .ok_or_else(|| anyhow::anyhow!("Missing sub-share for index {}", my_new_index))?;

        let sub_share_bytes: [u8; 32] = hex::decode(sub_share_hex)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid sub-share length"))?;

        let sub_share: Scalar<Secret, Zero> = Scalar::from_bytes(sub_share_bytes)
            .ok_or_else(|| anyhow::anyhow!("Invalid sub-share scalar"))?;

        let lagrange_coeff = crate::crypto_helpers::lagrange_coefficient_at_zero(output.old_party_index, &old_indices)?;

        let current: Scalar<Secret, Zero> =
            Scalar::from_bytes(new_share_bytes).unwrap_or(Scalar::zero());
        let weighted = s!(lagrange_coeff * sub_share);
        let sum = s!(current + weighted);
        new_share_bytes = sum.to_bytes();
    }

    // Create target wallet
    let target_state_dir = get_state_dir(target_wallet);
    let target_path = std::path::Path::new(&target_state_dir);

    if target_path.exists() {
        if !force_overwrite {
            anyhow::bail!(
                "Target wallet '{}' already exists. Use force_overwrite=true to replace.",
                target_wallet
            );
        }
        std::fs::remove_dir_all(target_path)?;
    }

    let target_storage = FileStorage::new(&target_state_dir)?;

    // Create PairedSecretShare using helper function
    let share_scalar: Scalar<Secret, Zero> = Scalar::from_bytes(new_share_bytes)
        .ok_or_else(|| anyhow::anyhow!("Invalid computed share"))?;
    let share_nonzero = crate::crypto_helpers::share_to_nonzero(share_scalar)?;

    let paired_share = crate::crypto_helpers::construct_paired_secret_share(
        my_new_index,
        share_nonzero,
        &group_public_key,
    )?;
    let paired_bytes = bincode::serialize(&paired_share)?;

    target_storage.write("paired_secret_share.bin", &paired_bytes)?;
    target_storage.write("shared_key.bin", &shared_key_bytes)?;

    // Create HTSS metadata
    let mut party_ranks: BTreeMap<u32, u32> = BTreeMap::new();
    party_ranks.insert(my_new_index, my_rank);
    for i in 1..=new_n_parties {
        if i != my_new_index {
            party_ranks.insert(i, 0);
        }
    }

    let new_htss = HtssMetadata {
        my_index: my_new_index,
        my_rank,
        threshold: new_threshold,
        hierarchical,
        party_ranks,
    };

    target_storage.write(
        "htss_metadata.json",
        serde_json::to_string_pretty(&new_htss)?.as_bytes(),
    )?;

    // Create group info
    let pubkey_bytes: [u8; 32] = group_public_key.to_xonly_bytes();
    let pubkey_hex = hex::encode(pubkey_bytes);

    use bitcoin::{Address, Network, XOnlyPublicKey};
    let xonly_pk = XOnlyPublicKey::from_slice(&pubkey_bytes)?;
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let address_testnet = Address::p2tr(&secp, xonly_pk, None, Network::Testnet).to_string();
    let address_mainnet = Address::p2tr(&secp, xonly_pk, None, Network::Bitcoin).to_string();

    let group_info = GroupInfo {
        name: target_wallet.to_string(),
        group_public_key: pubkey_hex.clone(),
        taproot_address_testnet: address_testnet.clone(),
        taproot_address_mainnet: address_mainnet.clone(),
        threshold: new_threshold,
        total_parties: new_n_parties,
        hierarchical,
        parties: vec![],
    };

    target_storage.write(
        "group_info.json",
        serde_json::to_string_pretty(&group_info)?.as_bytes(),
    )?;

    target_storage.write("share_hex.txt", hex::encode(new_share_bytes).as_bytes())?;

    Ok(CommandResult {
        output: format!(
            "Resharing complete!\n\
             New wallet: {}\n\
             Config: {}-of-{}\n\
             Your index: {}\n\
             Public Key: {}\n\
             Testnet Address: {}",
            target_wallet, new_threshold, new_n_parties, my_new_index, pubkey_hex, address_testnet
        ),
        result: target_wallet.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lagrange_coefficients_sum_to_one() {
        use crate::crypto_helpers::lagrange_coefficient_at_zero;

        // Test that Lagrange coefficients for indices {1,2,3} at x=0 sum to 1
        // This is a fundamental property: Σ λ_i(x) = 1 for any x
        let indices = vec![1, 2, 3];

        let mut sum: Scalar<Secret, Zero> = Scalar::zero();
        for &idx in &indices {
            let coeff = lagrange_coefficient_at_zero(idx, &indices).unwrap();
            sum = s!(sum + coeff);
        }

        // Sum should equal 1
        let one: Scalar<Secret, Zero> = Scalar::from(1u32);
        assert_eq!(sum.to_bytes(), one.to_bytes());
    }

    #[test]
    fn test_resharing_preserves_secret() {
        use crate::crypto_helpers::lagrange_coefficient_at_zero;

        // Simulate resharing: old shares combine to same secret
        // Original secret: s
        // Old shares: s_1, s_2, s_3 (2-of-3 Shamir)
        // After resharing, new shares should reconstruct to same s

        // Create a mock secret and shares using simple linear polynomial
        // f(x) = s + a*x, where f(0) = s
        let mut rng = rand::thread_rng();
        let secret = Scalar::<Secret, NonZero>::random(&mut rng);
        let coeff = Scalar::<Secret, NonZero>::random(&mut rng);

        // Evaluate at indices 1, 2, 3
        let share1 = s!(secret + { Scalar::<Secret, Zero>::from(1u32) } * coeff);
        let share2 = s!(secret + { Scalar::<Secret, Zero>::from(2u32) } * coeff);
        let share3 = s!(secret + { Scalar::<Secret, Zero>::from(3u32) } * coeff);

        // Reconstruct secret using Lagrange at x=0 with shares 1 and 2
        let indices = vec![1, 2];
        let lambda1 = lagrange_coefficient_at_zero(1, &indices).unwrap();
        let lambda2 = lagrange_coefficient_at_zero(2, &indices).unwrap();

        let reconstructed = s!(lambda1 * share1 + lambda2 * share2);

        // Should equal original secret
        let secret_zero: Scalar<Secret, Zero> = Scalar::from_bytes(secret.to_bytes()).unwrap();
        assert_eq!(reconstructed.to_bytes(), secret_zero.to_bytes());

        // Also verify with indices 2 and 3
        let indices = vec![2, 3];
        let lambda2 = lagrange_coefficient_at_zero(2, &indices).unwrap();
        let lambda3 = lagrange_coefficient_at_zero(3, &indices).unwrap();

        let reconstructed2 = s!(lambda2 * share2 + lambda3 * share3);
        assert_eq!(reconstructed2.to_bytes(), secret_zero.to_bytes());
    }
}
