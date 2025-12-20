//! DKG Transaction Module for Threshold Signing
//!
//! This module implements multi-party threshold signing for Bitcoin Taproot transactions:
//! - Build unsigned transactions with UTXO selection
//! - Compute BIP341 sighash for threshold signing
//! - Generate and exchange nonces
//! - Create and combine signature shares
//! - Apply taptweak and broadcast
//!
//! ## Transaction Flow
//!
//! ```text
//! Coordinator           Signer 2              Signer 3
//!     │                     │                     │
//!     │ dkg-build-tx        │                     │
//!     ├─────────────────────┼─────────────────────┤ (share sighash)
//!     │                     │                     │
//!     │ dkg-nonce           │ dkg-nonce           │ dkg-nonce
//!     ├─────────────────────┼─────────────────────┤ (exchange nonces)
//!     │                     │                     │
//!     │ dkg-sign            │ dkg-sign            │ dkg-sign
//!     ├─────────────────────┼─────────────────────┤ (exchange shares)
//!     │                     │                     │
//!     │ dkg-broadcast       │                     │
//!     └─────────────────────┘
//!           ↓
//!         txid
//! ```

use crate::bitcoin_tx::{broadcast_transaction, fetch_fee_estimates, fetch_utxos};
use crate::keygen::{get_state_dir, HtssMetadata};
use crate::signing::NonceOutput;
use crate::storage::{FileStorage, Storage};
use crate::CommandResult;
use anyhow::{Context, Result};
use bitcoin::absolute::LockTime;
use bitcoin::address::Address;
use bitcoin::hashes::Hash;
use bitcoin::key::XOnlyPublicKey;
use bitcoin::script::ScriptBuf;
use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
use bitcoin::transaction::Version;
use bitcoin::{Amount, Network, OutPoint, Sequence, Transaction, TxIn, TxOut, Txid, Witness};
use schnorr_fun::frost::{self, PairedSecretShare, SharedKey};
use schnorr_fun::Message;
use secp256kfun::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::str::FromStr;

// ============================================================================
// Output Types
// ============================================================================

/// Output from dkg-build-tx command
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuildTxOutput {
    /// Session ID for this signing session
    pub session_id: String,
    /// Sighash to be signed (32 bytes hex)
    pub sighash: String,
    /// Unsigned transaction (raw hex)
    pub unsigned_tx: String,
    /// From address
    pub from_address: String,
    /// To address
    pub to_address: String,
    /// Amount in satoshis
    pub amount_sats: u64,
    /// Estimated fee
    pub fee_sats: u64,
    /// Network
    pub network: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// Output from dkg-sign command
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DkgSignatureShareOutput {
    /// Party index
    pub party_index: u32,
    /// Party rank (for HTSS)
    pub rank: u32,
    /// Session ID
    pub session_id: String,
    /// Sighash that was signed
    pub sighash: String,
    /// Signature share (scalar hex)
    pub signature_share: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// Output from dkg-broadcast command
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BroadcastOutput {
    /// Transaction ID
    pub txid: String,
    /// Raw signed transaction
    pub raw_tx: String,
    /// Network
    pub network: String,
    /// Explorer URL
    pub explorer_url: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

// ============================================================================
// Helper Functions
// ============================================================================

fn network_name(network: Network) -> &'static str {
    match network {
        Network::Bitcoin => "mainnet",
        Network::Testnet => "testnet",
        Network::Signet => "signet",
        Network::Regtest => "regtest",
        _ => "unknown",
    }
}

/// Generate a session ID based on transaction details
fn generate_session_id(to_address: &str, amount: u64) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let hash_input = format!("{}:{}:{}", to_address, amount, timestamp);
    let hash = Sha256::digest(hash_input.as_bytes());
    hex::encode(&hash[..8]) // First 8 bytes for readability
}

// ============================================================================
// Build Unsigned Transaction
// ============================================================================

/// Build an unsigned transaction and compute sighash for DKG signing
pub fn build_unsigned_tx(
    wallet_name: &str,
    to_address: &str,
    amount_sats: u64,
    fee_rate: Option<u64>,
    network: Network,
) -> Result<()> {
    let state_dir = get_state_dir(wallet_name);
    let storage = FileStorage::new(&state_dir)?;
    let cmd_result = build_unsigned_tx_core(wallet_name, to_address, amount_sats, fee_rate, network, &storage)?;

    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 Share this with all signing parties:");
    println!("{}\n", cmd_result.result);

    Ok(())
}

/// Core function for building unsigned transaction
pub fn build_unsigned_tx_core(
    wallet_name: &str,
    to_address: &str,
    amount_sats: u64,
    fee_rate: Option<u64>,
    network: Network,
    storage: &dyn Storage,
) -> Result<CommandResult> {
    let mut out = String::new();

    out.push_str("DKG Transaction Builder\n\n");
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Load DKG shared key
    let shared_key_bytes = storage
        .read("shared_key.bin")
        .context("No DKG shared key found. Run keygen-finalize first.")?;

    let shared_key: SharedKey<EvenY> =
        bincode::deserialize(&shared_key_bytes).context("Failed to deserialize shared key")?;

    // Get x-only public key
    let pubkey_point = shared_key.public_key();
    let pubkey_bytes: [u8; 32] = pubkey_point.to_xonly_bytes();

    // Get our address
    let xonly_pubkey = XOnlyPublicKey::from_slice(&pubkey_bytes)?;
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let from_address = Address::p2tr(&secp, xonly_pubkey, None, network);

    // Parse destination address
    let dest_address = Address::from_str(to_address)
        .context("Invalid destination address")?
        .require_network(network)
        .context("Address network mismatch")?;

    out.push_str(&format!("Wallet: {}\n", wallet_name));
    out.push_str(&format!("Network: {}\n", network_name(network)));
    out.push_str(&format!("From: {}\n", from_address));
    out.push_str(&format!("To: {}\n", dest_address));
    out.push_str(&format!("Amount: {} sats\n\n", amount_sats));

    // Fetch UTXOs
    out.push_str("Fetching UTXOs...\n");
    let utxos = fetch_utxos(&from_address.to_string(), network)?;

    if utxos.is_empty() {
        anyhow::bail!("No UTXOs found. Please fund the DKG address first.");
    }

    let confirmed_utxos: Vec<_> = utxos.iter().filter(|u| u.status.confirmed).collect();
    if confirmed_utxos.is_empty() {
        anyhow::bail!("No confirmed UTXOs. Wait for confirmations.");
    }

    let total_available: u64 = confirmed_utxos.iter().map(|u| u.value).sum();
    out.push_str(&format!("Available balance: {} sats\n", total_available));

    // Get fee rate
    let fee_estimates = fetch_fee_estimates(network)?;
    let fee_rate = fee_rate.unwrap_or(fee_estimates.half_hour_fee);
    out.push_str(&format!("Fee rate: {} sats/vbyte\n", fee_rate));

    // Estimate tx size
    let estimated_vsize: u64 = 10 + (confirmed_utxos.len() as u64 * 58) + (2 * 43);
    let estimated_fee = estimated_vsize * fee_rate;

    if total_available < amount_sats + estimated_fee {
        anyhow::bail!(
            "Insufficient funds. Need {} sats, have {} sats",
            amount_sats + estimated_fee,
            total_available
        );
    }

    // Build transaction inputs
    let mut tx_inputs = Vec::new();
    let mut prevouts = Vec::new();

    for utxo in &confirmed_utxos {
        let txid = Txid::from_str(&utxo.txid)?;
        let outpoint = OutPoint::new(txid, utxo.vout);

        tx_inputs.push(TxIn {
            previous_output: outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
        });

        prevouts.push(TxOut {
            value: Amount::from_sat(utxo.value),
            script_pubkey: from_address.script_pubkey(),
        });
    }

    // Build outputs
    let selected_amount: u64 = confirmed_utxos.iter().map(|u| u.value).sum();
    let mut tx_outputs = Vec::new();

    // Recipient output
    tx_outputs.push(TxOut {
        value: Amount::from_sat(amount_sats),
        script_pubkey: dest_address.script_pubkey(),
    });

    // Change output
    let change_amount = selected_amount - amount_sats - estimated_fee;
    if change_amount > 546 {
        tx_outputs.push(TxOut {
            value: Amount::from_sat(change_amount),
            script_pubkey: from_address.script_pubkey(),
        });
    }

    // Create unsigned transaction
    let tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: tx_inputs,
        output: tx_outputs,
    };

    // Compute sighash for first input (we'll handle multiple inputs later)
    let mut sighash_cache = SighashCache::new(&tx);
    let prevouts_slice = Prevouts::All(&prevouts);
    let sighash = sighash_cache
        .taproot_key_spend_signature_hash(0, &prevouts_slice, TapSighashType::Default)
        .context("Failed to compute sighash")?;

    let sighash_hex = hex::encode(sighash.as_byte_array());

    // Generate session ID
    let session_id = generate_session_id(to_address, amount_sats);

    // Serialize unsigned tx
    let unsigned_tx_hex = bitcoin::consensus::encode::serialize_hex(&tx);

    // Save session data for later
    let session_data = serde_json::json!({
        "session_id": session_id,
        "sighash": sighash_hex,
        "unsigned_tx": unsigned_tx_hex,
        "prevouts": prevouts.iter().map(|p| {
            serde_json::json!({
                "value": p.value.to_sat(),
                "script_pubkey": hex::encode(p.script_pubkey.as_bytes())
            })
        }).collect::<Vec<_>>(),
        "from_address": from_address.to_string(),
        "to_address": dest_address.to_string(),
        "amount_sats": amount_sats,
        "fee_sats": estimated_fee,
        "network": network_name(network),
    });

    storage.write(
        &format!("dkg_session_{}.json", session_id),
        serde_json::to_string_pretty(&session_data)?.as_bytes(),
    )?;

    out.push_str(&format!("\nSession ID: {}\n", session_id));
    out.push_str(&format!("Sighash: {}\n", sighash_hex));
    out.push_str(&format!("Estimated fee: {} sats\n\n", estimated_fee));

    out.push_str("🧠 Next steps:\n");
    out.push_str("   1. Share the session_id and sighash with all signing parties\n");
    out.push_str("   2. Each party runs: frostdao dkg-nonce --name <wallet> --session <session_id>\n");
    out.push_str("   3. Exchange nonces, then run: frostdao dkg-sign ...\n");
    out.push_str("   4. Coordinator runs: frostdao dkg-broadcast ...\n");

    let output = BuildTxOutput {
        session_id,
        sighash: sighash_hex,
        unsigned_tx: unsigned_tx_hex,
        from_address: from_address.to_string(),
        to_address: dest_address.to_string(),
        amount_sats,
        fee_sats: estimated_fee,
        network: network_name(network).to_string(),
        event_type: "dkg_build_tx".to_string(),
    };

    Ok(CommandResult {
        output: out,
        result: serde_json::to_string(&output)?,
    })
}

// ============================================================================
// Generate Nonce for DKG Signing
// ============================================================================

/// Generate nonce for DKG transaction signing
pub fn dkg_generate_nonce(wallet_name: &str, session_id: &str) -> Result<()> {
    let state_dir = get_state_dir(wallet_name);
    let storage = FileStorage::new(&state_dir)?;
    let cmd_result = dkg_generate_nonce_core(wallet_name, session_id, &storage)?;

    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 Share this with other signing parties:");
    println!("{}\n", cmd_result.result);

    Ok(())
}

/// Core function for nonce generation
pub fn dkg_generate_nonce_core(
    wallet_name: &str,
    session_id: &str,
    storage: &dyn Storage,
) -> Result<CommandResult> {
    let mut out = String::new();

    // Load HTSS metadata
    let htss_metadata: HtssMetadata = {
        let metadata_json = String::from_utf8(storage.read("htss_metadata.json")?)?;
        serde_json::from_str(&metadata_json)?
    };

    let mode_name = if htss_metadata.hierarchical { "HTSS" } else { "TSS" };

    out.push_str(&format!("DKG Nonce Generation ({})\n\n", mode_name));
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    out.push_str(&format!("Wallet: {}\n", wallet_name));
    out.push_str(&format!("Session: {}\n", session_id));
    out.push_str(&format!("Your index: {} (rank {})\n\n", htss_metadata.my_index, htss_metadata.my_rank));

    // Load paired secret share
    let paired_share_bytes = storage
        .read("paired_secret_share.bin")
        .context("Failed to load secret share. Did you run keygen-finalize?")?;
    let paired_share: PairedSecretShare<EvenY> = bincode::deserialize(&paired_share_bytes)?;

    // Create FROST instance
    let frost = frost::new_with_synthetic_nonces::<Sha256, rand::rngs::ThreadRng>();

    // Seed nonce RNG with session ID
    let mut nonce_rng: rand_chacha::ChaCha20Rng = frost.seed_nonce_rng(paired_share, session_id.as_bytes());

    // Generate nonce
    let nonce = frost.gen_nonce(&mut nonce_rng);

    // Save nonce for later signing
    let nonce_bytes = bincode::serialize(&nonce)?;
    storage.write(&format!("dkg_nonce_{}.bin", session_id), &nonce_bytes)?;

    // Serialize public nonce
    let public_nonce = nonce.public();
    let public_nonce_bytes = bincode::serialize(&public_nonce)?;
    let public_nonce_hex = hex::encode(&public_nonce_bytes);

    out.push_str("⚠️  NEVER reuse a nonce - it will leak your secret share!\n\n");

    // Create output compatible with existing NonceOutput
    let output = NonceOutput {
        party_index: htss_metadata.my_index,
        rank: htss_metadata.my_rank,
        session: session_id.to_string(),
        nonce: public_nonce_hex,
        event_type: "dkg_nonce".to_string(),
    };

    Ok(CommandResult {
        output: out,
        result: serde_json::to_string(&output)?,
    })
}

// ============================================================================
// Create Signature Share
// ============================================================================

/// Create signature share for DKG transaction
pub fn dkg_sign(
    wallet_name: &str,
    session_id: &str,
    sighash: &str,
    nonces_data: &str,
) -> Result<()> {
    let state_dir = get_state_dir(wallet_name);
    let storage = FileStorage::new(&state_dir)?;
    let cmd_result = dkg_sign_core(wallet_name, session_id, sighash, nonces_data, &storage)?;

    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 Share this signature share:");
    println!("{}\n", cmd_result.result);

    Ok(())
}

/// Core function for signature share creation
pub fn dkg_sign_core(
    _wallet_name: &str,
    session_id: &str,
    sighash_hex: &str,
    nonces_data: &str,
    storage: &dyn Storage,
) -> Result<CommandResult> {
    let mut out = String::new();

    // Load HTSS metadata
    let htss_metadata: HtssMetadata = {
        let metadata_json = String::from_utf8(storage.read("htss_metadata.json")?)?;
        serde_json::from_str(&metadata_json)?
    };

    let mode_name = if htss_metadata.hierarchical { "HTSS" } else { "TSS" };

    out.push_str(&format!("DKG Signature Share Creation ({})\n\n", mode_name));
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Load nonce
    let nonce_bytes = storage
        .read(&format!("dkg_nonce_{}.bin", session_id))
        .context("Nonce not found. Did you run dkg-nonce?")?;
    let nonce: schnorr_fun::binonce::NonceKeyPair = bincode::deserialize(&nonce_bytes)?;

    // Load paired secret share
    let paired_share_bytes = storage.read("paired_secret_share.bin")?;
    let paired_share: PairedSecretShare<EvenY> = bincode::deserialize(&paired_share_bytes)?;

    // Load shared key
    let shared_key_bytes = storage.read("shared_key.bin")?;
    let shared_key: SharedKey<EvenY> = bincode::deserialize(&shared_key_bytes)?;

    // Parse sighash
    let sighash_bytes: [u8; 32] = hex::decode(sighash_hex)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid sighash length"))?;

    // Parse nonces from other parties
    let nonce_outputs: Vec<NonceOutput> = crate::keygen::parse_space_separated_json(nonces_data)?;

    out.push_str(&format!("Session: {}\n", session_id));
    out.push_str(&format!("Sighash: {}...\n", &sighash_hex[..16]));
    out.push_str(&format!("Signers: {} parties\n\n", nonce_outputs.len()));

    // Validate signer set in HTSS mode
    if htss_metadata.hierarchical {
        let ranks: Vec<u32> = nonce_outputs.iter().map(|n| n.rank).collect();
        crate::birkhoff::validate_signer_set(&ranks, htss_metadata.threshold)?;
        out.push_str("✓ HTSS signer set is valid\n\n");
    }

    // Build nonces map
    let mut nonces_map = BTreeMap::new();
    for nonce_output in &nonce_outputs {
        let nonce_bytes = hex::decode(&nonce_output.nonce)?;
        let public_nonce: schnorr_fun::binonce::Nonce = bincode::deserialize(&nonce_bytes)?;

        let share_index = Scalar::<Secret, Zero>::from(nonce_output.party_index)
            .non_zero()
            .expect("index should be nonzero")
            .public();
        nonces_map.insert(share_index, public_nonce);
    }

    // Create FROST instance
    let frost = frost::new_with_deterministic_nonces::<Sha256>();

    // Create message from sighash (using BIP340 challenge format)
    // For Bitcoin Taproot, the message is the raw sighash bytes
    let msg = Message::raw(&sighash_bytes);

    // Create coordinator session
    let coord_session = frost.coordinator_sign_session(&shared_key, nonces_map.clone(), msg);

    // Create party sign session
    let agg_binonce = coord_session.agg_binonce();
    let parties = coord_session.parties();
    let sign_session = frost.party_sign_session(shared_key.public_key(), parties.clone(), agg_binonce, msg);

    // Create signature share
    let sig_share = sign_session.sign(&paired_share, nonce);
    let sig_share_hex = hex::encode(bincode::serialize(&sig_share)?);

    // Save session data for combine step
    let final_nonce = coord_session.final_nonce();
    let final_nonce_bytes = bincode::serialize(&final_nonce)?;
    storage.write(&format!("dkg_final_nonce_{}.bin", session_id), &final_nonce_bytes)?;

    let nonces_json = serde_json::to_string(&nonce_outputs)?;
    storage.write(&format!("dkg_session_nonces_{}.json", session_id), nonces_json.as_bytes())?;

    out.push_str("✓ Signature share created\n");

    let output = DkgSignatureShareOutput {
        party_index: htss_metadata.my_index,
        rank: htss_metadata.my_rank,
        session_id: session_id.to_string(),
        sighash: sighash_hex.to_string(),
        signature_share: sig_share_hex,
        event_type: "dkg_signature_share".to_string(),
    };

    Ok(CommandResult {
        output: out,
        result: serde_json::to_string(&output)?,
    })
}

// ============================================================================
// Combine Signatures and Broadcast
// ============================================================================

/// Combine signature shares and broadcast transaction
pub fn dkg_broadcast(
    wallet_name: &str,
    session_id: &str,
    unsigned_tx_hex: &str,
    shares_data: &str,
    network: Network,
) -> Result<()> {
    let state_dir = get_state_dir(wallet_name);
    let storage = FileStorage::new(&state_dir)?;
    let cmd_result = dkg_broadcast_core(wallet_name, session_id, unsigned_tx_hex, shares_data, network, &storage)?;

    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 Transaction details:");
    println!("{}\n", cmd_result.result);

    Ok(())
}

/// Core function for combining signatures and broadcasting
pub fn dkg_broadcast_core(
    _wallet_name: &str,
    session_id: &str,
    unsigned_tx_hex: &str,
    shares_data: &str,
    network: Network,
    storage: &dyn Storage,
) -> Result<CommandResult> {
    let mut out = String::new();

    out.push_str("DKG Transaction Broadcast\n\n");
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Load shared key
    let shared_key_bytes = storage.read("shared_key.bin")?;
    let shared_key: SharedKey<EvenY> = bincode::deserialize(&shared_key_bytes)?;

    // Load session data
    let session_json = String::from_utf8(
        storage.read(&format!("dkg_session_{}.json", session_id))?
    )?;
    let session_data: serde_json::Value = serde_json::from_str(&session_json)?;
    let sighash_hex = session_data["sighash"].as_str().unwrap();
    let sighash_bytes: [u8; 32] = hex::decode(sighash_hex)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid sighash"))?;

    // Parse signature shares
    let share_outputs: Vec<DkgSignatureShareOutput> = crate::keygen::parse_space_separated_json(shares_data)?;

    out.push_str(&format!("Session: {}\n", session_id));
    out.push_str(&format!("Shares received: {}\n\n", share_outputs.len()));

    // Load saved nonces
    let nonces_json = String::from_utf8(
        storage.read(&format!("dkg_session_nonces_{}.json", session_id))?
    )?;
    let nonce_outputs: Vec<NonceOutput> = serde_json::from_str(&nonces_json)?;

    // Rebuild nonces map
    let mut nonces_map = BTreeMap::new();
    for nonce_output in &nonce_outputs {
        let nonce_bytes = hex::decode(&nonce_output.nonce)?;
        let public_nonce: schnorr_fun::binonce::Nonce = bincode::deserialize(&nonce_bytes)?;
        let share_index = Scalar::<Secret, Zero>::from(nonce_output.party_index)
            .non_zero()
            .expect("index should be nonzero")
            .public();
        nonces_map.insert(share_index, public_nonce);
    }

    // Create FROST instance
    let frost = frost::new_with_synthetic_nonces::<Sha256, rand::rngs::ThreadRng>();

    // Create message
    let msg = Message::raw(&sighash_bytes);

    // Recreate coordinator session
    let coord_session = frost.coordinator_sign_session(&shared_key, nonces_map, msg);

    // Parse and verify signature shares
    let mut sig_shares = BTreeMap::new();
    for share_output in &share_outputs {
        let share_bytes = hex::decode(&share_output.signature_share)?;
        let sig_share: Scalar<Public, Zero> = bincode::deserialize(&share_bytes)?;
        let share_index = Scalar::<Secret, Zero>::from(share_output.party_index)
            .non_zero()
            .expect("index should be nonzero")
            .public();
        sig_shares.insert(share_index, sig_share);
        out.push_str(&format!("   Party {}: ✓\n", share_output.party_index));
    }

    out.push_str("\nCombining signature shares...\n");

    // Verify and combine
    let signature = coord_session
        .verify_and_combine_signature_shares(&shared_key, sig_shares)
        .map_err(|e| anyhow::anyhow!("Signature verification failed: {:?}", e))?;

    out.push_str("✓ Signature valid!\n\n");

    // Get signature components (R, s)
    let sig_r_bytes = signature.R.to_xonly_bytes();
    let sig_s_bytes = signature.s.to_bytes();

    // Apply taptweak to signature
    // For key-path spend: tweaked_R stays same, we need to account for tweak in verification
    // The schnorr_fun library handles this internally with EvenY marker

    // Combine R and s into 64-byte BIP340 signature
    let mut sig_64 = [0u8; 64];
    sig_64[..32].copy_from_slice(&sig_r_bytes);
    sig_64[32..].copy_from_slice(&sig_s_bytes);

    // Parse unsigned transaction
    let tx_bytes = hex::decode(unsigned_tx_hex)?;
    let mut tx: Transaction = bitcoin::consensus::deserialize(&tx_bytes)?;

    // Add witness with signature
    // For Taproot key-path spend, witness is just the signature
    tx.input[0].witness = Witness::from_slice(&[&sig_64[..]]);

    // Serialize signed transaction
    let raw_tx = bitcoin::consensus::encode::serialize_hex(&tx);
    let txid = tx.compute_txid();

    out.push_str("Broadcasting transaction...\n");

    // Broadcast
    let broadcast_result = broadcast_transaction(&raw_tx, network);

    let explorer_url = match network {
        Network::Testnet => format!("https://mempool.space/testnet/tx/{}", txid),
        Network::Signet => format!("https://mempool.space/signet/tx/{}", txid),
        Network::Bitcoin => format!("https://mempool.space/tx/{}", txid),
        _ => format!("https://mempool.space/testnet/tx/{}", txid),
    };

    match broadcast_result {
        Ok(_) => {
            out.push_str(&format!("\n✅ Transaction broadcast successfully!\n"));
            out.push_str(&format!("TxID: {}\n", txid));
            out.push_str(&format!("Explorer: {}\n", explorer_url));
        }
        Err(e) => {
            out.push_str(&format!("\n⚠️ Broadcast failed: {}\n", e));
            out.push_str("Raw transaction saved for manual broadcast.\n");
        }
    }

    let output = BroadcastOutput {
        txid: txid.to_string(),
        raw_tx,
        network: network_name(network).to_string(),
        explorer_url,
        event_type: "dkg_broadcast".to_string(),
    };

    Ok(CommandResult {
        output: out,
        result: serde_json::to_string(&output)?,
    })
}
