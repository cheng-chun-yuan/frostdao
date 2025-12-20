//! Bitcoin Transaction Building and Broadcasting
//!
//! This module implements full Bitcoin transaction lifecycle for Taproot:
//! - UTXO fetching from mempool.space API
//! - Transaction construction
//! - BIP341 sighash computation
//! - Schnorr signing
//! - Transaction broadcasting

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
use reqwest::blocking::Client;
use secp256kfun::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::str::FromStr;

const STATE_DIR: &str = ".frost_state";

// Mempool.space API endpoints
const MEMPOOL_TESTNET_API: &str = "https://mempool.space/testnet/api";
const MEMPOOL_SIGNET_API: &str = "https://mempool.space/signet/api";
const MEMPOOL_MAINNET_API: &str = "https://mempool.space/api";

// ============================================================================
// API Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct UtxoResponse {
    pub txid: String,
    pub vout: u32,
    pub status: UtxoStatus,
    pub value: u64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct UtxoStatus {
    pub confirmed: bool,
    pub block_height: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct FeeEstimate {
    #[serde(rename = "fastestFee")]
    pub fastest_fee: u64,
    #[serde(rename = "halfHourFee")]
    pub half_hour_fee: u64,
    #[serde(rename = "hourFee")]
    pub hour_fee: u64,
    #[serde(rename = "economyFee")]
    pub economy_fee: u64,
    #[serde(rename = "minimumFee")]
    pub minimum_fee: u64,
}

// ============================================================================
// Output Types
// ============================================================================

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SendTransactionOutput {
    pub txid: String,
    pub raw_tx: String,
    pub from_address: String,
    pub to_address: String,
    pub amount_sats: u64,
    pub fee_sats: u64,
    pub network: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BalanceOutput {
    pub address: String,
    pub balance_sats: u64,
    pub utxo_count: usize,
    pub network: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

// ============================================================================
// Stored Key Structure (must match bitcoin_schnorr.rs)
// ============================================================================

#[derive(Serialize, Deserialize, Debug)]
struct StoredBitcoinKey {
    secret_key_bytes: Vec<u8>,
    public_key_bytes: Vec<u8>,
}

// ============================================================================
// Helper Functions
// ============================================================================

fn get_api_base(network: Network) -> &'static str {
    match network {
        Network::Bitcoin => MEMPOOL_MAINNET_API,
        Network::Testnet => MEMPOOL_TESTNET_API,
        Network::Signet => MEMPOOL_SIGNET_API,
        _ => MEMPOOL_TESTNET_API,
    }
}

fn network_name(network: Network) -> &'static str {
    match network {
        Network::Bitcoin => "mainnet",
        Network::Testnet => "testnet",
        Network::Signet => "signet",
        Network::Regtest => "regtest",
        _ => "unknown",
    }
}

/// BIP340 tagged hash
fn tagged_hash(tag: &str, msg: &[u8]) -> [u8; 32] {
    let tag_hash = Sha256::digest(tag.as_bytes());
    let mut hasher = Sha256::new();
    hasher.update(&tag_hash);
    hasher.update(&tag_hash);
    hasher.update(msg);
    hasher.finalize().into()
}

/// BIP340/challenge hash
fn challenge_hash(r_bytes: &[u8; 32], pubkey_bytes: &[u8; 32], message: &[u8]) -> [u8; 32] {
    let mut data = Vec::with_capacity(32 + 32 + message.len());
    data.extend_from_slice(r_bytes);
    data.extend_from_slice(pubkey_bytes);
    data.extend_from_slice(message);
    tagged_hash("BIP0340/challenge", &data)
}

/// BIP340/aux hash
fn aux_hash(aux: &[u8; 32]) -> [u8; 32] {
    tagged_hash("BIP0340/aux", aux)
}

/// BIP340/nonce hash
fn nonce_hash(masked_secret: &[u8; 32], pubkey_bytes: &[u8; 32], message: &[u8]) -> [u8; 32] {
    let mut data = Vec::with_capacity(32 + 32 + message.len());
    data.extend_from_slice(masked_secret);
    data.extend_from_slice(pubkey_bytes);
    data.extend_from_slice(message);
    tagged_hash("BIP0340/nonce", &data)
}

/// Sign a message using BIP340 (returns 64-byte signature)
fn sign_bip340(secret_bytes: &[u8; 32], pubkey_bytes: &[u8; 32], message: &[u8]) -> Result<[u8; 64]> {
    let secret_scalar: Scalar<Secret, NonZero> = Scalar::from_bytes(*secret_bytes)
        .ok_or_else(|| anyhow::anyhow!("Invalid secret key bytes"))?
        .non_zero()
        .ok_or_else(|| anyhow::anyhow!("Secret key is zero"))?;

    // Generate deterministic nonce (no aux randomness for simplicity)
    let aux = [0u8; 32];
    let aux_hashed = aux_hash(&aux);
    let mut masked_secret = *secret_bytes;
    for i in 0..32 {
        masked_secret[i] ^= aux_hashed[i];
    }

    let k_bytes = nonce_hash(&masked_secret, pubkey_bytes, message);
    let mut k_scalar: Scalar<Secret, NonZero> = Scalar::from_bytes(k_bytes)
        .ok_or_else(|| anyhow::anyhow!("Invalid nonce bytes"))?
        .non_zero()
        .ok_or_else(|| anyhow::anyhow!("Nonce is zero"))?;

    let mut r_point = g!(k_scalar * G).normalize();

    // Ensure R has even Y
    if !r_point.is_y_even() {
        k_scalar = -k_scalar;
        r_point = -r_point;
    }

    let r_bytes: [u8; 32] = r_point.to_xonly_bytes();

    // Compute challenge
    let e_bytes = challenge_hash(&r_bytes, pubkey_bytes, message);
    let e_scalar: Scalar<Public, Zero> = Scalar::from_bytes(e_bytes)
        .ok_or_else(|| anyhow::anyhow!("Invalid challenge bytes"))?;

    // Compute s = k + e * d
    let s_scalar = s!(k_scalar + e_scalar * secret_scalar);
    let s_bytes = s_scalar.to_bytes();

    // Create 64-byte signature
    let mut signature = [0u8; 64];
    signature[..32].copy_from_slice(&r_bytes);
    signature[32..].copy_from_slice(&s_bytes);

    Ok(signature)
}

// ============================================================================
// API Functions
// ============================================================================

/// Fetch UTXOs for an address
pub fn fetch_utxos(address: &str, network: Network) -> Result<Vec<UtxoResponse>> {
    let client = Client::new();
    let api_base = get_api_base(network);
    let url = format!("{}/address/{}/utxo", api_base, address);

    let response = client
        .get(&url)
        .send()
        .context("Failed to fetch UTXOs from mempool.space")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        anyhow::bail!("API error {}: {}", status, body);
    }

    let utxos: Vec<UtxoResponse> = response.json().context("Failed to parse UTXO response")?;
    Ok(utxos)
}

/// Fetch recommended fees
pub fn fetch_fee_estimates(network: Network) -> Result<FeeEstimate> {
    let client = Client::new();
    let api_base = get_api_base(network);
    let url = format!("{}/v1/fees/recommended", api_base);

    let response = client
        .get(&url)
        .send()
        .context("Failed to fetch fee estimates")?;

    if !response.status().is_success() {
        // Return default fees if API fails
        return Ok(FeeEstimate {
            fastest_fee: 10,
            half_hour_fee: 5,
            hour_fee: 3,
            economy_fee: 2,
            minimum_fee: 1,
        });
    }

    let fees: FeeEstimate = response.json().context("Failed to parse fee response")?;
    Ok(fees)
}

/// Broadcast a transaction
pub fn broadcast_transaction(raw_tx_hex: &str, network: Network) -> Result<String> {
    let client = Client::new();
    let api_base = get_api_base(network);
    let url = format!("{}/tx", api_base);

    let response = client
        .post(&url)
        .body(raw_tx_hex.to_string())
        .send()
        .context("Failed to broadcast transaction")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        anyhow::bail!("Broadcast failed {}: {}", status, body);
    }

    let txid = response.text().context("Failed to get txid from response")?;
    Ok(txid.trim().to_string())
}

// ============================================================================
// Balance Check
// ============================================================================

pub fn check_balance_core(network: Network, storage: &dyn Storage) -> Result<CommandResult> {
    let mut out = String::new();

    out.push_str("Bitcoin Balance Check\n\n");
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Load keypair
    let stored_json = String::from_utf8(
        storage
            .read("bitcoin_keypair.json")
            .context("No keypair found. Run btc-keygen first.")?,
    )?;
    let stored_key: StoredBitcoinKey = serde_json::from_str(&stored_json)?;

    let pubkey_bytes: [u8; 32] = stored_key
        .public_key_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid public key length"))?;

    // Get address
    let xonly_pubkey = XOnlyPublicKey::from_slice(&pubkey_bytes)?;
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let address = Address::p2tr(&secp, xonly_pubkey, None, network);

    out.push_str(&format!("Network: {}\n", network_name(network)));
    out.push_str(&format!("Address: {}\n\n", address));

    out.push_str("Fetching UTXOs from mempool.space...\n");

    let utxos = fetch_utxos(&address.to_string(), network)?;

    let total_balance: u64 = utxos.iter().map(|u| u.value).sum();
    let confirmed_utxos: Vec<_> = utxos.iter().filter(|u| u.status.confirmed).collect();
    let confirmed_balance: u64 = confirmed_utxos.iter().map(|u| u.value).sum();

    out.push_str(&format!("\nTotal UTXOs: {}\n", utxos.len()));
    out.push_str(&format!("Confirmed UTXOs: {}\n", confirmed_utxos.len()));
    out.push_str(&format!("\nTotal Balance: {} sats ({:.8} BTC)\n", total_balance, total_balance as f64 / 100_000_000.0));
    out.push_str(&format!("Confirmed Balance: {} sats ({:.8} BTC)\n", confirmed_balance, confirmed_balance as f64 / 100_000_000.0));

    if !utxos.is_empty() {
        out.push_str("\nUTXO Details:\n");
        for utxo in &utxos {
            let status = if utxo.status.confirmed { "confirmed" } else { "unconfirmed" };
            out.push_str(&format!("  {} sats - {}:{} ({})\n", utxo.value, &utxo.txid[..16], utxo.vout, status));
        }
    }

    let output = BalanceOutput {
        address: address.to_string(),
        balance_sats: total_balance,
        utxo_count: utxos.len(),
        network: network_name(network).to_string(),
        event_type: "bitcoin_balance".to_string(),
    };
    let result = serde_json::to_string(&output)?;

    Ok(CommandResult {
        output: out,
        result,
    })
}

/// CLI wrapper for testnet balance
pub fn check_balance_testnet() -> Result<()> {
    let storage = FileStorage::new(STATE_DIR)?;
    let cmd_result = check_balance_core(Network::Testnet, &storage)?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}\n", cmd_result.result);
    Ok(())
}

// ============================================================================
// DKG Balance Check
// ============================================================================

pub fn check_dkg_balance_core(network: Network, storage: &dyn Storage) -> Result<CommandResult> {
    use schnorr_fun::frost::SharedKey;

    let mut out = String::new();

    out.push_str("DKG Group Balance Check\n\n");
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Load DKG shared key
    let shared_key_bytes = storage
        .read("shared_key.bin")
        .context("No DKG shared key found. Run keygen-finalize first.")?;

    let shared_key: SharedKey<secp256kfun::marker::EvenY> =
        bincode::deserialize(&shared_key_bytes).context("Failed to deserialize shared key")?;

    // Get x-only public key bytes
    let pubkey_point = shared_key.public_key();
    let pubkey_bytes: [u8; 32] = pubkey_point.to_xonly_bytes();

    // Get address
    let xonly_pubkey = XOnlyPublicKey::from_slice(&pubkey_bytes)?;
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let address = Address::p2tr(&secp, xonly_pubkey, None, network);

    out.push_str(&format!("Network: {}\n", network_name(network)));
    out.push_str(&format!("DKG Address: {}\n\n", address));

    out.push_str("Fetching UTXOs from mempool.space...\n");

    let utxos = fetch_utxos(&address.to_string(), network)?;

    let total_balance: u64 = utxos.iter().map(|u| u.value).sum();
    let confirmed_utxos: Vec<_> = utxos.iter().filter(|u| u.status.confirmed).collect();
    let confirmed_balance: u64 = confirmed_utxos.iter().map(|u| u.value).sum();

    out.push_str(&format!("\nTotal UTXOs: {}\n", utxos.len()));
    out.push_str(&format!("Confirmed UTXOs: {}\n", confirmed_utxos.len()));
    out.push_str(&format!("\nTotal Balance: {} sats ({:.8} BTC)\n", total_balance, total_balance as f64 / 100_000_000.0));
    out.push_str(&format!("Confirmed Balance: {} sats ({:.8} BTC)\n", confirmed_balance, confirmed_balance as f64 / 100_000_000.0));

    if !utxos.is_empty() {
        out.push_str("\nUTXO Details:\n");
        for utxo in &utxos {
            let status = if utxo.status.confirmed { "confirmed" } else { "unconfirmed" };
            out.push_str(&format!("  {} sats - {}:{} ({})\n", utxo.value, &utxo.txid[..16], utxo.vout, status));
        }
    }

    out.push_str("\nNote: Spending requires threshold signatures (2-of-3)\n");

    let output = BalanceOutput {
        address: address.to_string(),
        balance_sats: total_balance,
        utxo_count: utxos.len(),
        network: network_name(network).to_string(),
        event_type: "dkg_balance".to_string(),
    };
    let result = serde_json::to_string(&output)?;

    Ok(CommandResult {
        output: out,
        result,
    })
}

/// CLI wrapper for DKG testnet balance
pub fn check_dkg_balance_testnet() -> Result<()> {
    let storage = FileStorage::new(STATE_DIR)?;
    let cmd_result = check_dkg_balance_core(Network::Testnet, &storage)?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}\n", cmd_result.result);
    Ok(())
}

// ============================================================================
// Send Transaction
// ============================================================================

pub fn send_transaction_core(
    to_address: &str,
    amount_sats: u64,
    fee_rate: Option<u64>, // sats/vbyte
    network: Network,
    storage: &dyn Storage,
) -> Result<CommandResult> {
    let mut out = String::new();

    out.push_str("Bitcoin Taproot Transaction\n\n");
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Load keypair
    let stored_json = String::from_utf8(
        storage
            .read("bitcoin_keypair.json")
            .context("No keypair found. Run btc-keygen first.")?,
    )?;
    let stored_key: StoredBitcoinKey = serde_json::from_str(&stored_json)?;

    let secret_bytes: [u8; 32] = stored_key
        .secret_key_bytes
        .clone()
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid secret key length"))?;
    let pubkey_bytes: [u8; 32] = stored_key
        .public_key_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid public key length"))?;

    // Get our address
    let xonly_pubkey = XOnlyPublicKey::from_slice(&pubkey_bytes)?;
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let from_address = Address::p2tr(&secp, xonly_pubkey, None, network);

    // Parse destination address
    let dest_address = Address::from_str(to_address)
        .context("Invalid destination address")?
        .require_network(network)
        .context("Address network mismatch")?;

    out.push_str(&format!("Network: {}\n", network_name(network)));
    out.push_str(&format!("From: {}\n", from_address));
    out.push_str(&format!("To: {}\n", dest_address));
    out.push_str(&format!("Amount: {} sats\n\n", amount_sats));

    // Fetch UTXOs
    out.push_str("Fetching UTXOs...\n");
    let utxos = fetch_utxos(&from_address.to_string(), network)?;

    if utxos.is_empty() {
        anyhow::bail!("No UTXOs found. Please fund the address first.");
    }

    // Filter confirmed UTXOs
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

    // Estimate transaction size for P2TR -> P2TR
    // Input: ~58 vbytes (witness ~64 bytes / 4 = 16 vbytes + 41 vbytes base)
    // Output: ~43 vbytes per P2TR output
    // Overhead: ~10 vbytes
    let estimated_vsize: u64 = 10 + (confirmed_utxos.len() as u64 * 58) + (2 * 43); // 2 outputs (recipient + change)
    let estimated_fee = estimated_vsize * fee_rate;

    out.push_str(&format!("Estimated fee: {} sats\n\n", estimated_fee));

    if total_available < amount_sats + estimated_fee {
        anyhow::bail!(
            "Insufficient funds. Need {} sats (amount + fee), have {} sats",
            amount_sats + estimated_fee,
            total_available
        );
    }

    // Select UTXOs (simple: use all for now)
    let selected_utxos = confirmed_utxos.clone();
    let selected_amount: u64 = selected_utxos.iter().map(|u| u.value).sum();

    // Build transaction
    out.push_str("Building transaction...\n");

    let mut tx_inputs = Vec::new();
    let mut prevouts = Vec::new();

    for utxo in &selected_utxos {
        let txid = Txid::from_str(&utxo.txid)?;
        let outpoint = OutPoint::new(txid, utxo.vout);

        tx_inputs.push(TxIn {
            previous_output: outpoint,
            script_sig: ScriptBuf::new(), // Empty for SegWit
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(), // Will fill later
        });

        // Create prevout for sighash calculation
        // For P2TR, the scriptPubKey is OP_1 <32-byte-pubkey>
        let script_pubkey = from_address.script_pubkey();
        prevouts.push(TxOut {
            value: Amount::from_sat(utxo.value),
            script_pubkey,
        });
    }

    // Create outputs
    let mut tx_outputs = Vec::new();

    // Recipient output
    tx_outputs.push(TxOut {
        value: Amount::from_sat(amount_sats),
        script_pubkey: dest_address.script_pubkey(),
    });

    // Change output (if needed)
    let change_amount = selected_amount - amount_sats - estimated_fee;
    if change_amount > 546 {
        // Dust threshold
        tx_outputs.push(TxOut {
            value: Amount::from_sat(change_amount),
            script_pubkey: from_address.script_pubkey(),
        });
    }

    // Create transaction
    let mut tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: tx_inputs,
        output: tx_outputs,
    };

    // Sign each input
    out.push_str("Signing transaction...\n");

    let prevouts_ref: Vec<TxOut> = prevouts.clone();
    let prevouts_slice = Prevouts::All(&prevouts_ref);

    for i in 0..tx.input.len() {
        // Compute sighash
        let mut sighash_cache = SighashCache::new(&tx);
        let sighash = sighash_cache
            .taproot_key_spend_signature_hash(i, &prevouts_slice, TapSighashType::Default)
            .context("Failed to compute sighash")?;

        let sighash_bytes: [u8; 32] = *sighash.as_byte_array();

        // We need to sign with the tweaked key for P2TR key-path spend
        // The tweaked secret key is: d' = d + H(P||m) where m is empty for key-only spend
        // For simplicity, let's compute the tweak

        // Compute the taptweak
        let tap_tweak_hash = tagged_hash("TapTweak", &pubkey_bytes);
        let tweak_scalar: Scalar<Public, Zero> = Scalar::from_bytes(tap_tweak_hash)
            .ok_or_else(|| anyhow::anyhow!("Invalid tweak"))?;

        // Load secret as scalar
        let secret_scalar: Scalar<Secret, NonZero> = Scalar::from_bytes(secret_bytes)
            .ok_or_else(|| anyhow::anyhow!("Invalid secret bytes"))?
            .non_zero()
            .ok_or_else(|| anyhow::anyhow!("Zero secret"))?;

        // Compute public key to check parity
        let public_point = g!(secret_scalar * G).normalize();

        // If Y is odd, negate secret before tweaking
        let secret_for_tweak = if public_point.is_y_even() {
            secret_scalar
        } else {
            -secret_scalar
        };

        // Tweaked secret: d' = d + tweak
        let tweaked_secret = s!(secret_for_tweak + tweak_scalar);
        let tweaked_secret_nonzero: Scalar<Secret, NonZero> = tweaked_secret
            .non_zero()
            .ok_or_else(|| anyhow::anyhow!("Tweaked secret is zero (extremely unlikely)"))?;

        // Compute tweaked public key for signing
        let tweaked_public = g!(tweaked_secret_nonzero * G).normalize();

        // BIP340 requires the public key to have even Y coordinate
        // If the tweaked public key has odd Y, we must negate the secret for signing
        let final_secret = if tweaked_public.is_y_even() {
            tweaked_secret_nonzero
        } else {
            -tweaked_secret_nonzero
        };
        let final_secret_bytes = final_secret.to_bytes();

        // The x-only public key bytes are the same regardless of Y parity
        let tweaked_pubkey_bytes: [u8; 32] = tweaked_public.to_xonly_bytes();

        // Sign with final (potentially negated) secret
        let signature = sign_bip340(&final_secret_bytes, &tweaked_pubkey_bytes, &sighash_bytes)?;

        // Set witness (just the signature for key-path spend)
        tx.input[i].witness = Witness::from_slice(&[&signature[..]]);
    }

    // Serialize transaction
    let raw_tx = bitcoin::consensus::encode::serialize_hex(&tx);
    let txid = tx.compute_txid();

    out.push_str(&format!("\nTransaction built successfully!\n"));
    out.push_str(&format!("TxID: {}\n", txid));
    out.push_str(&format!("Size: {} bytes\n", raw_tx.len() / 2));

    // Calculate actual fee
    let actual_fee = selected_amount - amount_sats - change_amount.max(0);

    out.push_str(&format!("Actual fee: {} sats\n\n", actual_fee));

    // Broadcast
    out.push_str("Broadcasting transaction...\n");

    match broadcast_transaction(&raw_tx, network) {
        Ok(broadcast_txid) => {
            out.push_str(&format!("\nTransaction broadcast successfully!\n"));
            out.push_str(&format!("TxID: {}\n", broadcast_txid));

            let explorer_url = match network {
                Network::Testnet => format!("https://mempool.space/testnet/tx/{}", broadcast_txid),
                Network::Signet => format!("https://mempool.space/signet/tx/{}", broadcast_txid),
                Network::Bitcoin => format!("https://mempool.space/tx/{}", broadcast_txid),
                _ => format!("https://mempool.space/testnet/tx/{}", broadcast_txid),
            };
            out.push_str(&format!("Explorer: {}\n", explorer_url));
        }
        Err(e) => {
            out.push_str(&format!("\nBroadcast failed: {}\n", e));
            out.push_str("Raw transaction (for manual broadcast):\n");
            out.push_str(&format!("{}\n", raw_tx));
        }
    }

    let output = SendTransactionOutput {
        txid: txid.to_string(),
        raw_tx,
        from_address: from_address.to_string(),
        to_address: dest_address.to_string(),
        amount_sats,
        fee_sats: actual_fee,
        network: network_name(network).to_string(),
        event_type: "bitcoin_transaction".to_string(),
    };
    let result = serde_json::to_string(&output)?;

    Ok(CommandResult {
        output: out,
        result,
    })
}

/// CLI wrapper for sending on testnet
pub fn send_testnet(to_address: &str, amount_sats: u64, fee_rate: Option<u64>) -> Result<()> {
    let storage = FileStorage::new(STATE_DIR)?;
    let cmd_result = send_transaction_core(to_address, amount_sats, fee_rate, Network::Testnet, &storage)?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Copy this JSON:");
    println!("{}\n", cmd_result.result);
    Ok(())
}

/// CLI wrapper for sending on signet
pub fn send_signet(to_address: &str, amount_sats: u64, fee_rate: Option<u64>) -> Result<()> {
    let storage = FileStorage::new(STATE_DIR)?;
    let cmd_result = send_transaction_core(to_address, amount_sats, fee_rate, Network::Signet, &storage)?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Copy this JSON:");
    println!("{}\n", cmd_result.result);
    Ok(())
}
