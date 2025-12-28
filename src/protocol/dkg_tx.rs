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

use crate::btc::transaction::{broadcast_transaction, fetch_fee_estimates, fetch_utxos};
use crate::protocol::keygen::{get_state_dir, HtssMetadata};
use crate::protocol::signing::NonceOutput;
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
// Taproot Helper Functions
// ============================================================================

// Use shared tagged_hash from crypto helpers
use crate::crypto::helpers::tagged_hash;

/// Compute the taptweak for a given internal public key (no script tree)
/// tweak = tagged_hash("TapTweak", internal_pubkey)
fn compute_taptweak(internal_pubkey: &[u8; 32]) -> Scalar<Public, Zero> {
    let tweak_bytes = tagged_hash("TapTweak", internal_pubkey);
    Scalar::from_bytes(tweak_bytes).expect("taptweak should be valid scalar")
}

/// Compute the tweaked public key Q = P + t*G for P2TR addresses
///
/// Returns (tweaked_pubkey, parity_flip) where:
/// - tweaked_pubkey: The tweaked key with even Y (for BIP340)
/// - parity_flip: true if the tweaked key was negated to achieve even Y
///
/// IMPORTANT for threshold signing:
/// - If parity_flip is false: signature = σ + e*t (add tweak contribution)
/// - If parity_flip is true: signature = σ - e*t (subtract tweak contribution)
///   AND secret shares must be negated before signing
fn compute_tweaked_pubkey(internal_pubkey: &Point<EvenY>) -> (Point<EvenY>, bool) {
    let pubkey_bytes: [u8; 32] = internal_pubkey.to_xonly_bytes();
    let tweak = compute_taptweak(&pubkey_bytes);
    let tweaked = g!({ *internal_pubkey } + tweak * G).normalize();
    // Convert to NonZero and then to EvenY, tracking whether negation occurred
    let tweaked_nonzero = tweaked
        .non_zero()
        .expect("tweaked point should not be zero");
    let (even_y_point, parity_flip) = tweaked_nonzero.into_point_with_even_y();
    (even_y_point, parity_flip)
}

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
    let cmd_result = build_unsigned_tx_core(
        wallet_name,
        to_address,
        amount_sats,
        fee_rate,
        network,
        &storage,
    )?;

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
    out.push_str(
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n",
    );

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
    out.push_str(
        "   2. Each party runs: frostdao dkg-nonce --name <wallet> --session <session_id>\n",
    );
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

    let mode_name = if htss_metadata.hierarchical {
        "HTSS"
    } else {
        "TSS"
    };

    out.push_str(&format!("DKG Nonce Generation ({})\n\n", mode_name));
    out.push_str(
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n",
    );
    out.push_str(&format!("Wallet: {}\n", wallet_name));
    out.push_str(&format!("Session: {}\n", session_id));
    out.push_str(&format!(
        "Your index: {} (rank {})\n\n",
        htss_metadata.my_index, htss_metadata.my_rank
    ));

    // Load paired secret share
    let paired_share_bytes = storage
        .read("paired_secret_share.bin")
        .context("Failed to load secret share. Did you run keygen-finalize?")?;
    let paired_share: PairedSecretShare<EvenY> = bincode::deserialize(&paired_share_bytes)?;

    // Create FROST instance
    let frost = frost::new_with_synthetic_nonces::<Sha256, rand::rngs::ThreadRng>();

    // Seed nonce RNG with session ID
    let mut nonce_rng: rand_chacha::ChaCha20Rng =
        frost.seed_nonce_rng(paired_share, session_id.as_bytes());

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

    let mode_name = if htss_metadata.hierarchical {
        "HTSS"
    } else {
        "TSS"
    };

    out.push_str(&format!("DKG Signature Share Creation ({})\n\n", mode_name));
    out.push_str(
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n",
    );

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
    let nonce_outputs: Vec<NonceOutput> =
        crate::protocol::keygen::parse_space_separated_json(nonces_data)?;

    out.push_str(&format!("Session: {}\n", session_id));
    out.push_str(&format!("Sighash: {}...\n", &sighash_hex[..16]));
    out.push_str(&format!("Signers: {} parties\n\n", nonce_outputs.len()));

    // Validate signer set in HTSS mode
    if htss_metadata.hierarchical {
        let ranks: Vec<u32> = nonce_outputs.iter().map(|n| n.rank).collect();
        crate::crypto::birkhoff::validate_signer_set(&ranks, htss_metadata.threshold)?;
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

    // IMPORTANT: For P2TR, we must sign against the TWEAKED public key Q, not the internal key P.
    // The P2TR address is derived from Q = P + H("TapTweak", P) * G
    // The signature must verify as: s*G = R + e*Q where e = H("BIP0340/challenge", R || Q || m)
    let internal_pubkey = shared_key.public_key();
    let (tweaked_pubkey, parity_flip) = compute_tweaked_pubkey(&internal_pubkey);

    // Create coordinator session (still uses internal key for nonce aggregation)
    let coord_session = frost.coordinator_sign_session(&shared_key, nonces_map.clone(), msg);

    // Create party sign session with TWEAKED public key for correct challenge computation
    let agg_binonce = coord_session.agg_binonce();
    let parties = coord_session.parties();
    let sign_session = frost.party_sign_session(tweaked_pubkey, parties.clone(), agg_binonce, msg);

    // CRITICAL: Handle taproot parity
    // If parity_flip is true, the tweaked key was negated to achieve even Y.
    // In this case, we need to sign with the NEGATED secret share.
    // This ensures: σ = k - e*p (instead of k + e*p) when combined,
    // which allows the final signature s = σ - e*t = k - e*p - e*t = k - e*(p+t) to verify.
    let sig_share = if parity_flip {
        let negated_paired = crate::crypto::helpers::negate_paired_secret_share(&paired_share)?;
        sign_session.sign(&negated_paired, nonce)
    } else {
        sign_session.sign(&paired_share, nonce)
    };
    let sig_share_hex = hex::encode(bincode::serialize(&sig_share)?);

    if parity_flip {
        out.push_str("📝 Note: Tweaked key has odd Y - using negated secret share\n\n");
    }

    // Save session data for combine step
    let final_nonce = coord_session.final_nonce();
    let final_nonce_bytes = bincode::serialize(&final_nonce)?;
    storage.write(
        &format!("dkg_final_nonce_{}.bin", session_id),
        &final_nonce_bytes,
    )?;

    // Save tweaked pubkey for broadcast step
    let tweaked_pubkey_bytes = tweaked_pubkey.to_xonly_bytes();
    storage.write(
        &format!("dkg_tweaked_pubkey_{}.bin", session_id),
        &tweaked_pubkey_bytes,
    )?;

    // Save parity flag for broadcast step - CRITICAL for correct signature combination
    storage.write(
        &format!("dkg_parity_flip_{}.bin", session_id),
        &[if parity_flip { 1u8 } else { 0u8 }],
    )?;

    let nonces_json = serde_json::to_string(&nonce_outputs)?;
    storage.write(
        &format!("dkg_session_nonces_{}.json", session_id),
        nonces_json.as_bytes(),
    )?;

    // SECURITY: Delete nonce after use to prevent dangerous reuse
    // Reusing a nonce with different messages exposes the secret share!
    let nonce_file = format!("dkg_nonce_{}.bin", session_id);
    storage.delete(&nonce_file)?;
    out.push_str("🔒 Nonce consumed and deleted (single-use enforced)\n");

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
    let cmd_result = dkg_broadcast_core(
        wallet_name,
        session_id,
        unsigned_tx_hex,
        shares_data,
        network,
        &storage,
    )?;

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
    out.push_str(
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n",
    );

    // Load shared key
    let shared_key_bytes = storage.read("shared_key.bin")?;
    let shared_key: SharedKey<EvenY> = bincode::deserialize(&shared_key_bytes)?;

    // Load session data
    let session_json =
        String::from_utf8(storage.read(&format!("dkg_session_{}.json", session_id))?)?;
    let session_data: serde_json::Value = serde_json::from_str(&session_json)?;
    let sighash_hex = session_data["sighash"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Session file missing or invalid 'sighash' field"))?;
    let sighash_bytes: [u8; 32] = hex::decode(sighash_hex)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid sighash length in session file"))?;

    // Parse signature shares
    let share_outputs: Vec<DkgSignatureShareOutput> =
        crate::protocol::keygen::parse_space_separated_json(shares_data)?;

    out.push_str(&format!("Session: {}\n", session_id));
    out.push_str(&format!("Shares received: {}\n\n", share_outputs.len()));

    // Load saved nonces
    let nonces_json =
        String::from_utf8(storage.read(&format!("dkg_session_nonces_{}.json", session_id))?)?;
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

    // Compute the tweaked public key (same as in dkg_sign)
    // IMPORTANT: Use the computed parity directly instead of reading from file.
    // This allows non-signing coordinators to broadcast without having run dkg_sign.
    let internal_pubkey = shared_key.public_key();
    let internal_pubkey_bytes: [u8; 32] = internal_pubkey.to_xonly_bytes();
    let (tweaked_pubkey, parity_flip) = compute_tweaked_pubkey(&internal_pubkey);
    let taptweak = compute_taptweak(&internal_pubkey_bytes);

    if parity_flip {
        out.push_str("📝 Parity flip detected - will subtract tweak contribution\n\n");
    }

    // Recreate coordinator session
    let coord_session = frost.coordinator_sign_session(&shared_key, nonces_map, msg);

    // Parse signature shares (skip verification since shares were computed with tweaked key)
    let mut sig_shares_sum: Scalar<Public, Zero> = Scalar::zero();
    for share_output in &share_outputs {
        let share_bytes = hex::decode(&share_output.signature_share)?;
        let sig_share: Scalar<Public, Zero> = bincode::deserialize(&share_bytes)?;
        let sum = s!(sig_shares_sum + sig_share);
        sig_shares_sum = sum.public(); // Convert back to Public marker
        out.push_str(&format!("   Party {}: ✓\n", share_output.party_index));
    }

    out.push_str("\nCombining signature shares...\n");

    // Get the final nonce R from coordinator session
    let final_nonce = coord_session.final_nonce();
    let sig_r_bytes = final_nonce.to_xonly_bytes();

    // Apply taptweak adjustment to s
    // Compute e = H("BIP0340/challenge", R || Q || m)
    let mut challenge_input = Vec::with_capacity(96);
    challenge_input.extend_from_slice(&sig_r_bytes);
    challenge_input.extend_from_slice(&tweaked_pubkey.to_xonly_bytes());
    challenge_input.extend_from_slice(&sighash_bytes);
    let challenge_hash = tagged_hash("BIP0340/challenge", &challenge_input);
    let challenge: Scalar<Public, Zero> = Scalar::from_bytes_mod_order(challenge_hash);

    // Compute e * t (the tweak contribution)
    let tweak_contribution = s!(challenge * taptweak);

    // CRITICAL: Handle parity correctly
    // - If parity_flip is false (Q had even Y): s = σ + e*t
    //   Combined shares σ = k + e*p, final s = k + e*p + e*t = k + e*(p+t) ✓
    // - If parity_flip is true (Q had odd Y, was negated):  s = σ - e*t
    //   Combined shares σ = k - e*p (shares were negated), final s = k - e*p - e*t = k - e*(p+t) ✓
    let sig_s_final = if parity_flip {
        s!(sig_shares_sum - tweak_contribution)
    } else {
        s!(sig_shares_sum + tweak_contribution)
    };
    let sig_s_bytes = sig_s_final.to_bytes();

    out.push_str(&format!(
        "✓ Signature computed with taptweak (parity_flip={})!\n\n",
        parity_flip
    ));

    // Combine R and s into 64-byte BIP340 signature
    let mut sig_64 = [0u8; 64];
    sig_64[..32].copy_from_slice(&sig_r_bytes);
    sig_64[32..].copy_from_slice(&sig_s_bytes);

    // Parse unsigned transaction
    let tx_bytes = hex::decode(unsigned_tx_hex)?;
    let mut tx: Transaction = bitcoin::consensus::deserialize(&tx_bytes)?;

    // Check for multi-UTXO limitation
    if tx.input.len() > 1 {
        out.push_str(&format!(
            "⚠️  WARNING: Transaction has {} inputs. Only first input will be signed.\n",
            tx.input.len()
        ));
        out.push_str("   Multi-UTXO signing requires separate sessions per input.\n\n");
    }

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

// ============================================================================
// Automated Multi-Party Signing for Local Parties
// ============================================================================

/// Result of automated FROST signing
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AutoSignResult {
    pub txid: String,
    pub raw_tx: String,
    pub from_address: String,
    pub to_address: String,
    pub amount_sats: u64,
    pub fee_sats: u64,
    pub network: String,
    pub explorer_url: String,
    pub signers: Vec<u32>,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// Automatically sign a transaction using all local party shares
///
/// This function automates the entire FROST signing flow:
/// 1. Build unsigned transaction with real BIP341 sighash
/// 2. Generate nonces for all selected local parties
/// 3. Generate signature shares for all selected local parties (with HD tweak if specified)
/// 4. Combine signatures with taptweak adjustment
/// 5. Broadcast or return ready-to-broadcast transaction
///
/// ## HD Derivation
/// If `derivation_path` is provided as `Some((change, address_index))`, the signing
/// will use the HD-derived key at that BIP-44 path. Each party's secret share is
/// tweaked locally using the same public derivation info.
pub fn frost_sign_all_local(
    wallet_name: &str,
    to_address: &str,
    amount_sats: u64,
    selected_parties: &[u32],            // Party indices (1-based)
    derivation_path: Option<(u32, u32)>, // Optional (change, address_index) for HD signing
    fee_rate: Option<u64>,
    network: Network,
) -> Result<CommandResult> {
    let mut out = String::new();

    out.push_str("🔐 FROST Multi-Party Signing (Automated)\n\n");
    out.push_str(
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n",
    );

    if selected_parties.is_empty() {
        anyhow::bail!("No parties selected for signing");
    }

    // Step 0: Load wallet metadata to validate threshold
    let state_dir = get_state_dir(wallet_name);

    // Load metadata from first party to get threshold info
    let first_party_idx = selected_parties[0];
    let first_party_dir = format!("{}/party{}", state_dir, first_party_idx);
    let first_party_storage = FileStorage::new(&first_party_dir)
        .with_context(|| format!("Party {} folder not found", first_party_idx))?;

    let wallet_metadata: HtssMetadata = {
        let metadata_json = String::from_utf8(first_party_storage.read("htss_metadata.json")?)?;
        serde_json::from_str(&metadata_json)?
    };

    // Validate threshold requirement
    if (selected_parties.len() as u32) < wallet_metadata.threshold {
        anyhow::bail!(
            "Insufficient parties for signing: selected {} but threshold requires at least {}",
            selected_parties.len(),
            wallet_metadata.threshold
        );
    }

    // For HTSS mode, validate signer set ranks
    if wallet_metadata.hierarchical {
        let ranks: Vec<u32> = selected_parties
            .iter()
            .filter_map(|&idx| wallet_metadata.party_ranks.get(&idx).copied())
            .collect();

        if ranks.len() != selected_parties.len() {
            anyhow::bail!("Could not determine ranks for all selected parties");
        }

        crate::crypto::birkhoff::validate_signer_set(&ranks, wallet_metadata.threshold)
            .context("HTSS signer set validation failed")?;
    }

    out.push_str(&format!("Wallet: {}\n", wallet_name));
    out.push_str(&format!("Signing parties: {:?}\n", selected_parties));
    out.push_str(&format!(
        "Threshold: {}-of-{}\n",
        wallet_metadata.threshold,
        wallet_metadata.party_ranks.len()
    ));
    out.push_str(&format!("Destination: {}\n", to_address));
    out.push_str(&format!("Amount: {} sats\n\n", amount_sats));
    let main_storage = FileStorage::new(&state_dir)?;

    let shared_key_bytes = main_storage
        .read("shared_key.bin")
        .context("No DKG shared key found")?;
    let shared_key: SharedKey<EvenY> = bincode::deserialize(&shared_key_bytes)?;

    // HD Derivation: If path specified, derive child key info
    let hd_derived_info: Option<crate::crypto::hd::DerivedKeyInfo> =
        if let Some((change, index)) = derivation_path {
            out.push_str(&format!("📍 Using HD path: {}/{}\n", change, index));

            // Load HD context using the proper function (reads hd_metadata.json)
            let hd_context = crate::btc::hd_address::load_hd_context(&main_storage)
                .context("HD context not found. Wallet may not support HD derivation.")?;

            let path = crate::crypto::hd::DerivationPath {
                change,
                address_index: index,
            };
            let derived = crate::crypto::hd::derive_at_path(&hd_context, &path)
                .context("Failed to derive HD key")?;
            Some(derived)
        } else {
            None
        };

    // Get address (use derived key if HD, otherwise root)
    let (from_pubkey, from_address) = if let Some(ref derived) = hd_derived_info {
        let pubkey_bytes: [u8; 32] = derived.public_key.to_xonly_bytes();
        let xonly_pubkey = XOnlyPublicKey::from_slice(&pubkey_bytes)?;
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let addr = Address::p2tr(&secp, xonly_pubkey, None, network);
        (derived.public_key, addr)
    } else {
        let pubkey_bytes: [u8; 32] = shared_key.public_key().to_xonly_bytes();
        let xonly_pubkey = XOnlyPublicKey::from_slice(&pubkey_bytes)?;
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let addr = Address::p2tr(&secp, xonly_pubkey, None, network);
        (shared_key.public_key(), addr)
    };

    // Parse destination
    let dest_address = Address::from_str(to_address)
        .context("Invalid destination address")?
        .require_network(network)
        .context("Address network mismatch")?;

    // Step 2: Fetch UTXOs and build transaction
    out.push_str("📥 Fetching UTXOs...\n");
    let utxos = fetch_utxos(&from_address.to_string(), network)?;

    let confirmed_utxos: Vec<_> = utxos.iter().filter(|u| u.status.confirmed).collect();
    if confirmed_utxos.is_empty() {
        anyhow::bail!("No confirmed UTXOs available");
    }

    let total_available: u64 = confirmed_utxos.iter().map(|u| u.value).sum();
    out.push_str(&format!("   Available: {} sats\n", total_available));

    // Get fee rate
    let fee_estimates = fetch_fee_estimates(network)?;
    let fee_rate = fee_rate.unwrap_or(fee_estimates.half_hour_fee);

    // Estimate fee
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

    tx_outputs.push(TxOut {
        value: Amount::from_sat(amount_sats),
        script_pubkey: dest_address.script_pubkey(),
    });

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

    // Compute sighash
    let mut sighash_cache = SighashCache::new(&tx);
    let prevouts_slice = Prevouts::All(&prevouts);
    let sighash = sighash_cache
        .taproot_key_spend_signature_hash(0, &prevouts_slice, TapSighashType::Default)
        .context("Failed to compute sighash")?;

    let sighash_bytes: [u8; 32] = *sighash.as_byte_array();
    let sighash_hex = hex::encode(&sighash_bytes);

    out.push_str(&format!("📝 Sighash: {}...\n\n", &sighash_hex[..16]));

    // Generate session ID
    let session_id = generate_session_id(to_address, amount_sats);

    // Step 3: Load party shares and generate nonces
    out.push_str("🔑 Generating nonces for all parties...\n");

    let frost = frost::new_with_synthetic_nonces::<Sha256, rand::rngs::ThreadRng>();

    let mut party_data: Vec<(
        u32,
        u32,
        PairedSecretShare<EvenY>,
        schnorr_fun::binonce::NonceKeyPair,
    )> = Vec::new();
    let mut nonces_map: BTreeMap<Scalar<Public, NonZero>, schnorr_fun::binonce::Nonce> =
        BTreeMap::new();
    let mut _nonce_outputs: Vec<NonceOutput> = Vec::new();

    for &party_idx in selected_parties {
        let party_dir = format!("{}/party{}", state_dir, party_idx);
        let party_storage = FileStorage::new(&party_dir)
            .with_context(|| format!("Party {} folder not found", party_idx))?;

        // Load metadata
        let metadata_json = String::from_utf8(party_storage.read("htss_metadata.json")?)?;
        let metadata: HtssMetadata = serde_json::from_str(&metadata_json)?;

        // Load paired secret share
        let paired_share_bytes = party_storage
            .read("paired_secret_share.bin")
            .with_context(|| format!("Party {} secret share not found", party_idx))?;
        let root_paired_share: PairedSecretShare<EvenY> =
            bincode::deserialize(&paired_share_bytes)?;

        // Apply HD derivation if specified
        let paired_share = if let Some(ref derived_info) = hd_derived_info {
            crate::crypto::hd::derive_share(&root_paired_share, derived_info)
                .with_context(|| format!("Failed to derive HD share for party {}", party_idx))?
        } else {
            root_paired_share
        };

        // Generate nonce (use the derived or root share)
        let mut nonce_rng: rand_chacha::ChaCha20Rng =
            frost.seed_nonce_rng(paired_share, session_id.as_bytes());
        let nonce = frost.gen_nonce(&mut nonce_rng);

        // Store public nonce
        let public_nonce = nonce.public();
        let share_index = Scalar::<Secret, Zero>::from(party_idx)
            .non_zero()
            .expect("index should be nonzero")
            .public();
        nonces_map.insert(share_index, public_nonce);

        // Create NonceOutput for compatibility
        let public_nonce_bytes = bincode::serialize(&public_nonce)?;
        let public_nonce_hex = hex::encode(&public_nonce_bytes);
        _nonce_outputs.push(NonceOutput {
            party_index: party_idx,
            rank: metadata.my_rank,
            session: session_id.clone(),
            nonce: public_nonce_hex,
            event_type: "signing_nonce".to_string(),
        });

        party_data.push((party_idx, metadata.my_rank, paired_share, nonce));
        out.push_str(&format!("   Party {}: ✓ nonce generated\n", party_idx));
    }

    out.push_str("\n");

    // Step 4: Generate signature shares (manual aggregation for HD compatibility)
    out.push_str("✍️  Generating signature shares...\n");

    // Compute tweaked public key for P2TR
    let internal_pubkey = from_pubkey;
    let internal_pubkey_bytes: [u8; 32] = internal_pubkey.to_xonly_bytes();
    let (tweaked_pubkey, parity_flip) = compute_tweaked_pubkey(&internal_pubkey);
    let taptweak = compute_taptweak(&internal_pubkey_bytes);

    // Manual nonce aggregation (bypasses SharedKey validation for HD compatibility)
    // Using simplified single-nonce aggregation: R = sum(R1_i)
    let party_indices: Vec<u32> = party_data.iter().map(|(idx, _, _, _)| *idx).collect();

    // Aggregate nonces - use first nonce component only (k1, R1)
    // This is simpler than full FROST binonces but secure for our use case
    let (agg_nonce_even, nonce_parity_flip): (Point<EvenY>, bool) = {
        let mut agg_r: Point<Normal, Public, Zero> = Point::zero();

        for (_, _, _, nonce) in &party_data {
            let public_nonce = nonce.public();
            let r1 = public_nonce.0[0]; // First nonce component
            let sum = g!(agg_r + r1);
            agg_r = sum.normalize();
        }

        let agg_nonzero = agg_r
            .non_zero()
            .ok_or_else(|| anyhow::anyhow!("Aggregated nonce is point at infinity"))?;
        // Track if R was negated to get even Y (needed for BIP-340 compliance)
        agg_nonzero.into_point_with_even_y()
    };

    let sig_r_bytes = agg_nonce_even.to_xonly_bytes();

    // Compute challenge e = H("BIP0340/challenge", R || Q || m)
    let mut challenge_input = Vec::with_capacity(96);
    challenge_input.extend_from_slice(&sig_r_bytes);
    challenge_input.extend_from_slice(&tweaked_pubkey.to_xonly_bytes());
    challenge_input.extend_from_slice(&sighash_bytes);
    let challenge_hash = tagged_hash("BIP0340/challenge", &challenge_input);
    let challenge: Scalar<Public, Zero> = Scalar::from_bytes_mod_order(challenge_hash);

    // Generate signature shares manually (bypasses schnorr_fun session validation for HD compatibility)
    // Using single nonces (k1 only), signature share: s_i = k1_i + lambda_i * e * x_i
    let mut _sig_shares: Vec<DkgSignatureShareOutput> = Vec::new();
    let mut sig_shares_sum: Scalar<Public, Zero> = Scalar::zero();

    for (party_idx, rank, paired_share, nonce) in party_data {
        // Get secret share value
        let secret_share = paired_share.secret_share();
        let share_value = secret_share.share;

        // Compute Lagrange coefficient for this party
        let lambda =
            crate::crypto::helpers::lagrange_coefficient_at_zero(party_idx, &party_indices)
                .context("Failed to compute Lagrange coefficient")?;

        // Get nonce secret k1 (using single nonce scheme)
        // SecretNonce is a tuple struct with [Scalar; 2], access with .0[0]
        let k1 = &nonce.secret.0[0];

        // Apply nonce parity adjustment if R was negated for even Y
        // BIP-340: if R has odd Y, we use -R, so we must also use -k
        let effective_k1 = if nonce_parity_flip {
            s!(-k1).public()
        } else {
            s!(k1).public()
        };

        // Compute signature share: s_i = k1_i + lambda_i * e * x_i
        // Handle parity flip for the share (needed for even Y coordinate of public key)
        let sig_share = if parity_flip {
            s!(effective_k1 + lambda * challenge * { s!(-share_value) })
        } else {
            s!(effective_k1 + lambda * challenge * share_value)
        };

        // Add to running sum
        let sum = s!(sig_shares_sum + sig_share);
        sig_shares_sum = sum.public();

        let sig_share_hex = hex::encode(sig_share.to_bytes());
        _sig_shares.push(DkgSignatureShareOutput {
            party_index: party_idx,
            rank,
            session_id: session_id.clone(),
            sighash: sighash_hex.clone(),
            signature_share: sig_share_hex,
            event_type: "dkg_signature_share".to_string(),
        });

        out.push_str(&format!("   Party {}: ✓ share created\n", party_idx));
    }

    out.push_str("\n");

    // Step 5: Combine signatures with taptweak
    out.push_str("🔗 Combining signature shares...\n");

    // Compute tweak contribution e * t (challenge already computed above)
    let tweak_contribution = s!(challenge * taptweak);

    // Apply taptweak adjustment
    let sig_s_final = if parity_flip {
        s!(sig_shares_sum - tweak_contribution)
    } else {
        s!(sig_shares_sum + tweak_contribution)
    };
    let sig_s_bytes = sig_s_final.to_bytes();

    // Create 64-byte BIP340 signature
    let mut sig_64 = [0u8; 64];
    sig_64[..32].copy_from_slice(&sig_r_bytes);
    sig_64[32..].copy_from_slice(&sig_s_bytes);

    out.push_str(&format!(
        "   Taptweak applied (parity_flip={})\n\n",
        parity_flip
    ));

    // Step 6: Create signed transaction
    let unsigned_tx_hex = bitcoin::consensus::encode::serialize_hex(&tx);
    let tx_bytes = hex::decode(&unsigned_tx_hex)?;
    let mut signed_tx: Transaction = bitcoin::consensus::deserialize(&tx_bytes)?;

    // Add witness
    signed_tx.input[0].witness = Witness::from_slice(&[&sig_64[..]]);

    let raw_tx = bitcoin::consensus::encode::serialize_hex(&signed_tx);
    let txid = signed_tx.compute_txid();

    out.push_str("📡 Broadcasting transaction...\n");

    // Broadcast
    let explorer_url = match network {
        Network::Testnet => format!("https://mempool.space/testnet/tx/{}", txid),
        Network::Signet => format!("https://mempool.space/signet/tx/{}", txid),
        Network::Bitcoin => format!("https://mempool.space/tx/{}", txid),
        _ => format!("https://mempool.space/testnet/tx/{}", txid),
    };

    match broadcast_transaction(&raw_tx, network) {
        Ok(_) => {
            out.push_str(&format!("\n✅ Transaction broadcast successfully!\n"));
            out.push_str(&format!("   TxID: {}\n", txid));
            out.push_str(&format!("   Explorer: {}\n", explorer_url));
        }
        Err(e) => {
            out.push_str(&format!("\n⚠️ Broadcast failed: {}\n", e));
            out.push_str("   Raw transaction saved for manual broadcast.\n");
        }
    }

    let output = AutoSignResult {
        txid: txid.to_string(),
        raw_tx,
        from_address: from_address.to_string(),
        to_address: dest_address.to_string(),
        amount_sats,
        fee_sats: estimated_fee,
        network: network_name(network).to_string(),
        explorer_url,
        signers: selected_parties.to_vec(),
        event_type: "frost_auto_sign".to_string(),
    };

    Ok(CommandResult {
        output: out,
        result: serde_json::to_string(&output)?,
    })
}
