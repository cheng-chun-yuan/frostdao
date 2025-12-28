//! Share Recovery Protocol for FROST/HTSS
//!
//! This module implements share recovery - reconstructing a lost party's share
//! from the remaining threshold parties WITHOUT changing the group public key.
//!
//! ## Protocol Overview
//!
//! For a t-of-n wallet where party j has lost their share:
//!
//! 1. At least t helper parties (who still have shares) participate
//! 2. Each helper party i shares their share value as sub_share
//! 3. Lost party j collects sub_shares from >= t helpers
//! 4. Lost party j computes their share using interpolation
//!
//! ## Interpolation Methods
//!
//! - **Standard TSS (all ranks = 0)**: Uses Lagrange interpolation at x=j
//!   s_j = Σ (λ_i(j) * sub_share_i) where λ_i(j) = Π_{k≠i} (j-k)/(i-k)
//!
//! - **HTSS with mixed ranks**: Uses Birkhoff interpolation
//!   Birkhoff generalizes Lagrange by incorporating derivative information (ranks).
//!   When recovering a rank-r share at index j, we compute coefficients that
//!   evaluate f^(r)(j) from the helper shares with their respective ranks.
//!
//! Key insight: When all ranks are 0, Birkhoff reduces to Lagrange!
//!
//! Result: The lost party gets their original share s_j back!

use crate::crypto::birkhoff::{
    birkhoff_coefficient_to_scalar, compute_birkhoff_recovery_coefficients, BirkhoffParameter,
};
use crate::protocol::keygen::{get_state_dir, GroupInfo, HtssMetadata};
use crate::storage::{FileStorage, Storage};
use crate::CommandResult;
use anyhow::Result;
use schnorr_fun::frost::{PairedSecretShare, SharedKey};
use schnorr_fun::fun::marker::*;
use secp256kfun::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Output from recovery round 1 (helper party generates sub-share for lost party)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RecoveryRound1Output {
    /// Helper party index
    pub helper_index: u32,
    /// Helper party's rank (for HTSS)
    pub helper_rank: u32,
    /// Sub-share for the lost party (this is just the helper's share value)
    pub sub_share: String,
    /// Lost party index (for verification)
    pub lost_index: u32,
    /// Wallet name (for verification)
    pub wallet_name: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// Generate sub-share to help recover a lost party's share
///
/// This is simpler than resharing - we just output our share value.
/// The lost party will use Lagrange interpolation at their index to reconstruct.
pub fn recover_round1(source_wallet: &str, lost_index: u32) -> Result<()> {
    let state_dir = get_state_dir(source_wallet);
    let path = std::path::Path::new(&state_dir);

    if !path.exists() {
        anyhow::bail!("Wallet '{}' not found at {}.", source_wallet, state_dir);
    }

    let storage = FileStorage::new(&state_dir)?;
    let cmd_result = recover_round1_core(source_wallet, lost_index, &storage)?;

    // Parse result to get threshold for message
    let result: RecoveryRound1Output = serde_json::from_str(&cmd_result.result)?;

    // Load HTSS metadata to get threshold
    let state_dir = get_state_dir(source_wallet);
    let storage = FileStorage::new(&state_dir)?;
    let htss_json = String::from_utf8(storage.read("htss_metadata.json")?)?;
    let htss: HtssMetadata = serde_json::from_str(&htss_json)?;

    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 Share this with the recovering party:");
    println!("{}\n", cmd_result.result);
    println!("⚠️  SECURITY WARNING: This protocol exposes your raw share value!");
    println!(
        "    After recovery, party {} will know {} shares (theirs + helpers').",
        result.lost_index, htss.threshold
    );
    println!(
        "    With {} shares, they could theoretically reconstruct the group secret.",
        htss.threshold
    );
    println!("    Only use this with TRUSTED parties who were already part of the group.\n");
    println!(
        "    The lost party needs {} helper outputs to recover.",
        htss.threshold
    );

    Ok(())
}

/// Core function for recovery round 1 (returns structured output)
pub fn recover_round1_core(
    source_wallet: &str,
    lost_index: u32,
    storage: &dyn Storage,
) -> Result<CommandResult> {
    let mut out = String::new();

    out.push_str("Share Recovery - Generate Helper Sub-share\n\n");
    out.push_str(
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n",
    );

    // Load HTSS metadata
    let htss_json = String::from_utf8(storage.read("htss_metadata.json")?)?;
    let htss: HtssMetadata = serde_json::from_str(&htss_json)?;

    let my_index = htss.my_index;
    let my_rank = htss.my_rank;
    let threshold = htss.threshold;
    let n_parties = htss.party_ranks.len() as u32;

    // Validate
    if lost_index == my_index {
        anyhow::bail!("You cannot help recover your own share! You ARE the lost party.");
    }

    if lost_index < 1 || lost_index > n_parties {
        anyhow::bail!(
            "Invalid lost_index {}. Must be 1 to {}.",
            lost_index,
            n_parties
        );
    }

    out.push_str(&format!("Wallet: {}\n", source_wallet));
    out.push_str(&format!("Config: {}-of-{}\n", threshold, n_parties));
    out.push_str(&format!("Your index: {} (rank {})\n", my_index, my_rank));
    out.push_str(&format!("Lost party index: {}\n\n", lost_index));

    // Load secret share
    let paired_share_bytes = storage.read("paired_secret_share.bin")?;
    let paired_share: PairedSecretShare<EvenY> = bincode::deserialize(&paired_share_bytes)?;

    // Get the share value
    let my_share = paired_share.secret_share();
    let share_bytes = my_share.share.to_bytes();
    let share_hex = hex::encode(share_bytes);

    out.push_str("🧠 How recovery works:\n");
    out.push_str("   1. Each helper shares their share value (what you're doing now)\n");
    out.push_str("   2. Lost party collects >= threshold helper outputs\n");
    out.push_str("   3. Lost party uses Lagrange interpolation at their index\n");
    out.push_str("   4. Result: Original share reconstructed!\n\n");

    out.push_str(&format!(
        "Generated sub-share for party {} to recover\n",
        lost_index
    ));

    // Create output
    let output = RecoveryRound1Output {
        helper_index: my_index,
        helper_rank: my_rank,
        sub_share: share_hex,
        lost_index,
        wallet_name: source_wallet.to_string(),
        event_type: "recovery_round1".to_string(),
    };

    let result_json = serde_json::to_string(&output)?;

    Ok(CommandResult {
        output: out,
        result: result_json,
    })
}

/// Finalize recovery - lost party combines sub-shares to reconstruct their share
pub fn recover_finalize(
    source_wallet: &str,
    target_wallet: &str,
    my_index: u32,
    my_rank: u32,
    hierarchical: bool,
    round1_data: &str,
    force: bool,
) -> Result<()> {
    let cmd_result = recover_finalize_core(
        source_wallet,
        target_wallet,
        my_index,
        my_rank,
        hierarchical,
        round1_data,
        force,
    )?;

    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 Recovery complete!");
    println!("   Your recovered wallet: {}", cmd_result.result);

    Ok(())
}

/// Core function for recovery finalize
///
/// Note: my_rank and hierarchical parameters are IGNORED for security.
/// The original rank and hierarchical setting are preserved from the source wallet
/// to prevent privilege escalation attacks.
pub fn recover_finalize_core(
    source_wallet: &str,
    target_wallet: &str,
    my_index: u32,
    _my_rank: u32, // IGNORED - use source wallet's rank to prevent privilege escalation
    _hierarchical: bool, // IGNORED - use source wallet's setting to prevent tampering
    round1_data: &str,
    force_overwrite: bool,
) -> Result<CommandResult> {
    let mut out = String::new();

    out.push_str("Share Recovery - Combine Sub-shares\n\n");
    out.push_str(
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n",
    );

    // Parse round1 outputs
    let round1_outputs: Vec<RecoveryRound1Output> =
        crate::protocol::keygen::parse_space_separated_json(round1_data)?;

    if round1_outputs.is_empty() {
        anyhow::bail!("No recovery round1 data provided");
    }

    // Load source wallet metadata FIRST to get original configuration
    let source_state_dir = get_state_dir(source_wallet);
    let source_storage = FileStorage::new(&source_state_dir)?;

    let shared_key_bytes = source_storage.read("shared_key.bin")?;
    let shared_key: SharedKey<EvenY> = bincode::deserialize(&shared_key_bytes)?;
    let group_public_key = shared_key.public_key();

    let source_htss_json = String::from_utf8(source_storage.read("htss_metadata.json")?)?;
    let source_htss: HtssMetadata = serde_json::from_str(&source_htss_json)?;
    let threshold = source_htss.threshold;
    let n_parties = source_htss.party_ranks.len() as u32;

    // SECURITY: Get the original rank from source wallet to prevent privilege escalation
    // If party wasn't in original config, this is a security violation - reject
    let original_rank = source_htss
        .party_ranks
        .get(&my_index)
        .copied()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "SECURITY ERROR: Party index {} not found in original wallet configuration.\n\
             Recovery is only allowed for parties that were part of the original group.\n\
             Original party indices: {:?}",
                my_index,
                source_htss.party_ranks.keys().collect::<Vec<_>>()
            )
        })?;

    // SECURITY: Use hierarchical setting from source wallet, not user input
    let hierarchical = source_htss.hierarchical;

    // Verify all outputs are for the same lost index and wallet
    let expected_lost_index = my_index;
    let expected_wallet = source_wallet;

    for output in &round1_outputs {
        if output.lost_index != expected_lost_index {
            anyhow::bail!(
                "Mismatched lost_index: expected {}, got {} from helper {}",
                expected_lost_index,
                output.lost_index,
                output.helper_index
            );
        }
        if output.wallet_name != expected_wallet {
            anyhow::bail!(
                "Mismatched wallet: expected '{}', got '{}' from helper {}",
                expected_wallet,
                output.wallet_name,
                output.helper_index
            );
        }
    }

    out.push_str(&format!(
        "Received sub-shares from {} helper parties\n",
        round1_outputs.len()
    ));
    out.push_str(&format!(
        "Recovering index: {} (original rank: {})\n\n",
        my_index, original_rank
    ));

    // Verify we have enough sub-shares
    if (round1_outputs.len() as u32) < threshold {
        anyhow::bail!(
            "Not enough sub-shares: got {}, need at least {}",
            round1_outputs.len(),
            threshold
        );
    }

    out.push_str(&format!(
        "Threshold: {} (have {} sub-shares) ✓\n",
        threshold,
        round1_outputs.len()
    ));
    out.push_str(&format!("Total parties: {}\n\n", n_parties));

    // Collect helper info (indices and ranks)
    let helper_indices: Vec<u32> = round1_outputs.iter().map(|o| o.helper_index).collect();
    let helper_ranks: Vec<u32> = round1_outputs.iter().map(|o| o.helper_rank).collect();

    // Check if we need Birkhoff (HTSS with any non-zero ranks) or can use Lagrange (all rank 0)
    let any_nonzero_rank = helper_ranks.iter().any(|&r| r > 0) || original_rank > 0;
    let use_birkhoff = hierarchical && any_nonzero_rank;

    if use_birkhoff {
        out.push_str("🧠 Birkhoff interpolation for HTSS recovery:\n");
        out.push_str(&format!("   Helpers: {:?}\n", helper_indices));
        out.push_str(&format!("   Helper ranks: {:?}\n", helper_ranks));
        out.push_str(&format!(
            "   Target index: {}, rank: {}\n\n",
            my_index, original_rank
        ));
    } else {
        out.push_str("🧠 Lagrange interpolation at x = your_index:\n");
        out.push_str(&format!("   Helpers: {:?}\n", helper_indices));
        out.push_str(&format!("   Target x: {}\n\n", my_index));
    }

    // Compute recovered share using appropriate interpolation method
    let recovered_share_bytes = if use_birkhoff {
        // Build Birkhoff parameters from helper data
        let params: Vec<BirkhoffParameter> = round1_outputs
            .iter()
            .map(|o| BirkhoffParameter::new(o.helper_index, o.helper_rank))
            .collect();

        // Compute Birkhoff recovery coefficients
        let birkhoff_coeffs =
            compute_birkhoff_recovery_coefficients(my_index, original_rank, &params)?;

        out.push_str("   Birkhoff coefficients: ");
        for (i, c) in birkhoff_coeffs.iter().enumerate() {
            out.push_str(&format!("{:.4} ", c));
            if i < birkhoff_coeffs.len() - 1 {
                out.push_str(", ");
            }
        }
        out.push_str("\n\n");

        // Combine sub-shares using Birkhoff coefficients
        let mut recovered: Scalar<Secret, Zero> = Scalar::zero();
        for (i, output) in round1_outputs.iter().enumerate() {
            let sub_share_bytes: [u8; 32] = hex::decode(&output.sub_share)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("Invalid sub-share length"))?;

            let sub_share: Scalar<Secret, Zero> = Scalar::from_bytes(sub_share_bytes)
                .ok_or_else(|| anyhow::anyhow!("Invalid sub-share scalar"))?;

            let coeff = birkhoff_coefficient_to_scalar(birkhoff_coeffs[i]);
            let weighted = s!(coeff * sub_share);
            recovered = s!(recovered + weighted);
        }

        recovered.to_bytes()
    } else {
        // Standard Lagrange interpolation at x = my_index
        // s_j = Σ λ_i(j) * s_i  where λ_i(j) = Π_{k≠i} (j - k) / (i - k)
        let mut share_bytes = [0u8; 32];

        for output in &round1_outputs {
            // Parse sub-share
            let sub_share_bytes: [u8; 32] = hex::decode(&output.sub_share)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("Invalid sub-share length"))?;

            let sub_share: Scalar<Secret, Zero> = Scalar::from_bytes(sub_share_bytes)
                .ok_or_else(|| anyhow::anyhow!("Invalid sub-share scalar"))?;

            // Compute Lagrange coefficient at x = my_index
            let lagrange_coeff = crate::crypto::helpers::lagrange_coefficient_at(
                output.helper_index,
                &helper_indices,
                my_index,
            )?;

            // Add weighted sub-share
            let current: Scalar<Secret, Zero> =
                Scalar::from_bytes(share_bytes).unwrap_or(Scalar::zero());
            let weighted = s!(lagrange_coeff * sub_share);
            let sum = s!(current + weighted);
            share_bytes = sum.to_bytes();
        }

        share_bytes
    };

    out.push_str("✓ Computed recovered share\n\n");

    // Create target wallet directory
    let target_state_dir = get_state_dir(target_wallet);
    let target_path = std::path::Path::new(&target_state_dir);

    if target_path.exists() {
        if !force_overwrite {
            anyhow::bail!(
                "Target wallet '{}' already exists. Use --force to overwrite.",
                target_wallet
            );
        }
        std::fs::remove_dir_all(target_path)?;
    }

    let target_storage = FileStorage::new(&target_state_dir)?;

    // Create PairedSecretShare using helper function
    let share_scalar: Scalar<Secret, Zero> = Scalar::from_bytes(recovered_share_bytes)
        .ok_or_else(|| anyhow::anyhow!("Invalid recovered share bytes"))?;
    let share_nonzero = crate::crypto::helpers::share_to_nonzero(share_scalar)?;

    let paired_share = crate::crypto::helpers::construct_paired_secret_share(
        my_index,
        share_nonzero,
        &group_public_key,
    )?;
    let paired_bytes = bincode::serialize(&paired_share)?;

    target_storage.write("paired_secret_share.bin", &paired_bytes)?;
    target_storage.write("shared_key.bin", &shared_key_bytes)?;

    // Create HTSS metadata preserving original configuration
    // Use source wallet's party_ranks (already includes this party's original rank)
    let party_ranks: BTreeMap<u32, u32> = source_htss.party_ranks.clone();

    let new_htss = HtssMetadata {
        my_index,
        my_rank: original_rank, // Use original rank from source wallet
        threshold,
        hierarchical, // Already set from source_htss.hierarchical
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
        threshold,
        total_parties: n_parties,
        hierarchical,
        parties: vec![],
    };

    target_storage.write(
        "group_info.json",
        serde_json::to_string_pretty(&group_info)?.as_bytes(),
    )?;

    // Save share in hex for verification
    target_storage.write(
        "share_hex.txt",
        hex::encode(recovered_share_bytes).as_bytes(),
    )?;

    out.push_str(
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n",
    );
    out.push_str("✅ Share recovery complete!\n\n");
    out.push_str(&format!("Recovered wallet: {}\n", target_wallet));
    out.push_str(&format!(
        "Config: {}-of-{} ({})\n",
        threshold,
        n_parties,
        if hierarchical { "HTSS" } else { "TSS" }
    ));
    out.push_str(&format!(
        "Your index: {} (rank {} - preserved from original)\n\n",
        my_index, original_rank
    ));
    out.push_str(&format!("Public Key: {}\n", pubkey_hex));
    out.push_str(&format!("Testnet Address: {}\n\n", address_testnet));
    out.push_str("⚠️  The public key and address are the SAME as the original wallet!\n");
    out.push_str("    Your recovered share is now compatible with the group.\n\n");
    out.push_str("🔐 SECURITY NOTE: This simplified recovery protocol exposed helper shares.\n");
    out.push_str("    You now know enough shares to reconstruct the group secret.\n");
    out.push_str("    A production system should use blinded sub-shares (like resharing).\n");

    Ok(CommandResult {
        output: out,
        result: target_wallet.to_string(),
    })
}

// Lagrange coefficient computation is now in crypto_helpers module

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::helpers::lagrange_coefficient_at;

    #[test]
    fn test_lagrange_at_different_x() {
        // For indices {1, 2} evaluating at x=3:
        // λ_1(3) = (3-2)/(1-2) = 1/(-1) = -1
        // λ_2(3) = (3-1)/(2-1) = 2/1 = 2
        // Check: -1 + 2 = 1 ✓ (Lagrange coefficients always sum to 1)

        let indices = vec![1u32, 2];

        let lambda1 = lagrange_coefficient_at(1, &indices, 3).unwrap();
        let lambda2 = lagrange_coefficient_at(2, &indices, 3).unwrap();

        // Sum should equal 1
        let sum = s!(lambda1 + lambda2);
        let one: Scalar<Secret, Zero> = Scalar::from(1u32);
        assert_eq!(sum.to_bytes(), one.to_bytes());

        // Verify λ_1(3) = -1
        let neg_one: Scalar<Secret, Zero> = {
            let pos: Scalar<Secret, Zero> = Scalar::from(1u32);
            s!(-pos)
        };
        assert_eq!(lambda1.to_bytes(), neg_one.to_bytes());

        // Verify λ_2(3) = 2
        let two: Scalar<Secret, Zero> = Scalar::from(2u32);
        assert_eq!(lambda2.to_bytes(), two.to_bytes());
    }

    #[test]
    fn test_recovery_math() {
        // Simulate recovery:
        // Original polynomial: f(x) = s + a*x (degree 1, threshold 2)
        // Shares: s_1 = f(1), s_2 = f(2), s_3 = f(3)
        //
        // If party 3 loses their share, parties 1 and 2 help recover:
        // s_3 = λ_1(3) * s_1 + λ_2(3) * s_2
        //     = (-1) * s_1 + 2 * s_2
        //     = (-1) * (s + a) + 2 * (s + 2a)
        //     = -s - a + 2s + 4a
        //     = s + 3a
        //     = f(3) ✓

        let mut rng = rand::thread_rng();

        // Create polynomial f(x) = s + a*x
        let secret = Scalar::<Secret, NonZero>::random(&mut rng);
        let coeff = Scalar::<Secret, NonZero>::random(&mut rng);

        // Compute shares
        let one: Scalar<Secret, Zero> = Scalar::from(1u32);
        let two: Scalar<Secret, Zero> = Scalar::from(2u32);
        let three: Scalar<Secret, Zero> = Scalar::from(3u32);

        let share1 = s!(secret + one * coeff); // f(1)
        let share2 = s!(secret + two * coeff); // f(2)
        let share3 = s!(secret + three * coeff); // f(3) - the one we want to recover

        // Recover share 3 using shares 1 and 2
        let indices = vec![1u32, 2];
        let lambda1 = lagrange_coefficient_at(1, &indices, 3).unwrap();
        let lambda2 = lagrange_coefficient_at(2, &indices, 3).unwrap();

        let recovered = s!(lambda1 * share1 + lambda2 * share2);

        // Should equal original share3
        assert_eq!(
            recovered.to_bytes(),
            share3.to_bytes(),
            "Recovered share should match original"
        );
    }
}
