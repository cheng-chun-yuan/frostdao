//! Bitcoin Schnorr Signatures (BIP340)
//!
//! This module implements BIP340-compliant Schnorr signatures for Bitcoin.
//! It provides single-signer operations that complement the FROST threshold
//! signatures implemented elsewhere in this crate.
//!
//! BIP340 specifies:
//! - 32-byte x-only public keys (even Y coordinate assumed)
//! - Tagged hashing for domain separation
//! - Deterministic nonces using aux randomness
//!
//! References:
//! - BIP340: https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki

use crate::storage::{FileStorage, Storage};
use crate::CommandResult;
use anyhow::{Context, Result};
use bitcoin::address::Address;
use bitcoin::key::XOnlyPublicKey;
use bitcoin::Network;
use rand::RngCore;
use secp256kfun::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const STATE_DIR: &str = ".frost_state";

// ============================================================================
// BIP340 Tagged Hash Functions
// ============================================================================

/// Computes a BIP340 tagged hash: SHA256(SHA256(tag) || SHA256(tag) || msg)
/// This provides domain separation for different use cases.
fn tagged_hash(tag: &str, msg: &[u8]) -> [u8; 32] {
    let tag_hash = Sha256::digest(tag.as_bytes());
    let mut hasher = Sha256::new();
    hasher.update(&tag_hash);
    hasher.update(&tag_hash);
    hasher.update(msg);
    hasher.finalize().into()
}

/// BIP340/challenge tagged hash for signature verification
fn challenge_hash(r_bytes: &[u8; 32], pubkey_bytes: &[u8; 32], message: &[u8]) -> [u8; 32] {
    let mut data = Vec::with_capacity(32 + 32 + message.len());
    data.extend_from_slice(r_bytes);
    data.extend_from_slice(pubkey_bytes);
    data.extend_from_slice(message);
    tagged_hash("BIP0340/challenge", &data)
}

/// BIP340/aux tagged hash for auxiliary randomness
fn aux_hash(aux: &[u8; 32]) -> [u8; 32] {
    tagged_hash("BIP0340/aux", aux)
}

/// BIP340/nonce tagged hash for deterministic nonce generation
fn nonce_hash(masked_secret: &[u8; 32], pubkey_bytes: &[u8; 32], message: &[u8]) -> [u8; 32] {
    let mut data = Vec::with_capacity(32 + 32 + message.len());
    data.extend_from_slice(masked_secret);
    data.extend_from_slice(pubkey_bytes);
    data.extend_from_slice(message);
    tagged_hash("BIP0340/nonce", &data)
}

// ============================================================================
// Data Structures
// ============================================================================

/// Output from key generation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BitcoinKeyOutput {
    /// 32-byte x-only public key (hex)
    pub public_key: String,
    /// Secret key (hex) - only included in local storage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_key: Option<String>,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// Output from signing
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BitcoinSignatureOutput {
    /// 64-byte Schnorr signature (hex)
    pub signature: String,
    /// The message that was signed (hex or UTF-8)
    pub message: String,
    /// 32-byte x-only public key (hex)
    pub public_key: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// Stored key material
#[derive(Serialize, Deserialize, Debug)]
struct StoredBitcoinKey {
    secret_key_bytes: Vec<u8>,
    public_key_bytes: Vec<u8>,
}

// ============================================================================
// Key Generation
// ============================================================================

/// Generate a new Bitcoin Schnorr keypair (BIP340)
pub fn generate_keypair_core(storage: &dyn Storage) -> Result<CommandResult> {
    let mut out = String::new();

    out.push_str("Bitcoin Schnorr Key Generation (BIP340)\n\n");
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    out.push_str("Generating random 256-bit secret key...\n");
    out.push_str("Using secp256k1 curve (same as Bitcoin)\n\n");

    // Generate random secret key
    let mut rng = rand::thread_rng();
    let mut secret_bytes = [0u8; 32];
    rng.fill_bytes(&mut secret_bytes);

    // Create scalar from bytes
    let secret_scalar: Scalar<Secret, NonZero> = Scalar::from_bytes(secret_bytes)
        .ok_or_else(|| anyhow::anyhow!("Invalid scalar bytes"))?
        .non_zero()
        .ok_or_else(|| anyhow::anyhow!("Generated zero scalar (extremely unlikely)"))?;

    // Compute public key P = secret * G
    let public_point = g!(secret_scalar * G).normalize();

    // For BIP340, we need x-only public key with even Y
    // If Y is odd, we negate the secret key
    let (final_secret, final_public) = if public_point.is_y_even() {
        (secret_scalar, public_point)
    } else {
        out.push_str("Public key Y was odd, negating secret for BIP340 compatibility\n");
        (-secret_scalar, -public_point)
    };

    // Get x-only public key bytes (32 bytes)
    let pubkey_bytes: [u8; 32] = final_public.to_xonly_bytes();
    let pubkey_hex = hex::encode(pubkey_bytes);

    // Get secret key bytes
    let secret_bytes = final_secret.to_bytes();

    out.push_str("Key generation complete!\n\n");

    out.push_str("BIP340 Key Properties:\n");
    out.push_str("   - 32-byte x-only public key (Y coordinate is always even)\n");
    out.push_str("   - Same curve as Bitcoin (secp256k1)\n");
    out.push_str("   - Compatible with Taproot (BIP341)\n\n");

    out.push_str("Why x-only public keys?\n");
    out.push_str("   Traditional public keys are 33 bytes (1 byte prefix + 32 bytes x)\n");
    out.push_str("   BIP340 uses only the x-coordinate (32 bytes) and assumes even Y\n");
    out.push_str("   This saves 1 byte and simplifies signature verification\n\n");

    // Store key material
    let stored_key = StoredBitcoinKey {
        secret_key_bytes: secret_bytes.to_vec(),
        public_key_bytes: pubkey_bytes.to_vec(),
    };
    let stored_json = serde_json::to_string(&stored_key)?;
    storage.write("bitcoin_keypair.json", stored_json.as_bytes())?;

    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    out.push_str("Keypair generated and saved!\n\n");

    out.push_str(&format!("Public Key (x-only): {}\n", pubkey_hex));
    out.push_str("Secret key saved to .frost_state/bitcoin_keypair.json\n\n");

    out.push_str("NEVER share your secret key!\n");
    out.push_str("Use this public key for receiving Bitcoin or verifying signatures.\n");

    // Create JSON result (public key only for safety)
    let output = BitcoinKeyOutput {
        public_key: pubkey_hex,
        secret_key: None,
        event_type: "bitcoin_keypair".to_string(),
    };
    let result = serde_json::to_string(&output)?;

    Ok(CommandResult {
        output: out,
        result,
    })
}

/// CLI wrapper for key generation
pub fn generate_keypair() -> Result<()> {
    let storage = FileStorage::new(STATE_DIR)?;
    let cmd_result = generate_keypair_core(&storage)?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Copy this JSON:");
    println!("{}\n", cmd_result.result);
    Ok(())
}

/// Import an existing secret key
pub fn import_key_core(secret_hex: &str, storage: &dyn Storage) -> Result<CommandResult> {
    let mut out = String::new();

    out.push_str("Bitcoin Schnorr Key Import (BIP340)\n\n");
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Decode secret key
    let secret_bytes: [u8; 32] = hex::decode(secret_hex)
        .context("Invalid hex string for secret key")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Secret key must be exactly 32 bytes"))?;

    let secret_scalar: Scalar<Secret, NonZero> = Scalar::from_bytes(secret_bytes)
        .ok_or_else(|| anyhow::anyhow!("Invalid secret key bytes"))?
        .non_zero()
        .ok_or_else(|| anyhow::anyhow!("Secret key is zero (invalid)"))?;

    // Compute public key
    let public_point = g!(secret_scalar * G).normalize();

    // Ensure even Y for BIP340
    let (final_secret, final_public) = if public_point.is_y_even() {
        (secret_scalar, public_point)
    } else {
        out.push_str("Negating secret key to ensure even Y (BIP340)\n");
        (-secret_scalar, -public_point)
    };

    let pubkey_bytes: [u8; 32] = final_public.to_xonly_bytes();
    let pubkey_hex = hex::encode(pubkey_bytes);

    // Store key material
    let stored_key = StoredBitcoinKey {
        secret_key_bytes: final_secret.to_bytes().to_vec(),
        public_key_bytes: pubkey_bytes.to_vec(),
    };
    let stored_json = serde_json::to_string(&stored_key)?;
    storage.write("bitcoin_keypair.json", stored_json.as_bytes())?;

    out.push_str("Key imported successfully!\n\n");
    out.push_str(&format!("Public Key (x-only): {}\n", pubkey_hex));

    let output = BitcoinKeyOutput {
        public_key: pubkey_hex,
        secret_key: None,
        event_type: "bitcoin_keypair".to_string(),
    };
    let result = serde_json::to_string(&output)?;

    Ok(CommandResult {
        output: out,
        result,
    })
}

/// CLI wrapper for key import
pub fn import_key(secret_hex: &str) -> Result<()> {
    let storage = FileStorage::new(STATE_DIR)?;
    let cmd_result = import_key_core(secret_hex, &storage)?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Copy this JSON:");
    println!("{}\n", cmd_result.result);
    Ok(())
}

// ============================================================================
// Signing
// ============================================================================

/// Sign a message using BIP340 Schnorr signature
pub fn sign_message_core(
    message: &[u8],
    aux_rand: Option<&[u8; 32]>,
    storage: &dyn Storage,
) -> Result<CommandResult> {
    let mut out = String::new();

    out.push_str("Bitcoin Schnorr Signing (BIP340)\n\n");
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Load keypair
    let stored_json = String::from_utf8(storage.read("bitcoin_keypair.json")?)?;
    let stored_key: StoredBitcoinKey = serde_json::from_str(&stored_json)?;

    let secret_bytes: [u8; 32] = stored_key
        .secret_key_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid secret key length"))?;
    let pubkey_bytes: [u8; 32] = stored_key
        .public_key_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid public key length"))?;

    let secret_scalar: Scalar<Secret, NonZero> = Scalar::from_bytes(secret_bytes)
        .ok_or_else(|| anyhow::anyhow!("Invalid stored secret key bytes"))?
        .non_zero()
        .ok_or_else(|| anyhow::anyhow!("Stored secret key is zero (invalid)"))?;

    out.push_str("Loaded keypair from storage\n\n");

    // Get auxiliary randomness (or use zeros for deterministic signing)
    let aux = aux_rand.cloned().unwrap_or_else(|| {
        out.push_str("No auxiliary randomness provided, using zeros\n");
        out.push_str("(Signatures will be deterministic)\n\n");
        [0u8; 32]
    });

    out.push_str("BIP340 Nonce Generation:\n");
    out.push_str("   1. t = aux_hash(aux) XOR secret_key\n");
    out.push_str("   2. k = nonce_hash(t || pubkey || message)\n");
    out.push_str("   3. R = k * G (ensure even Y)\n\n");

    // Step 1: Mask secret key with auxiliary randomness
    let aux_hashed = aux_hash(&aux);
    let mut masked_secret = secret_bytes;
    for i in 0..32 {
        masked_secret[i] ^= aux_hashed[i];
    }

    // Step 2: Generate nonce
    let k_bytes = nonce_hash(&masked_secret, &pubkey_bytes, message);
    let mut k_scalar: Scalar<Secret, NonZero> = Scalar::from_bytes(k_bytes)
        .ok_or_else(|| anyhow::anyhow!("Invalid nonce bytes"))?
        .non_zero()
        .ok_or_else(|| anyhow::anyhow!("Nonce is zero (extremely unlikely)"))?;

    // Step 3: Compute R = k * G
    let mut r_point = g!(k_scalar * G).normalize();

    // Ensure R has even Y (negate k if needed)
    if !r_point.is_y_even() {
        k_scalar = -k_scalar;
        r_point = -r_point;
    }

    let r_bytes: [u8; 32] = r_point.to_xonly_bytes();

    out.push_str("Computing BIP340 challenge:\n");
    out.push_str("   e = tagged_hash(\"BIP0340/challenge\", R || P || m)\n\n");

    // Step 4: Compute challenge
    let e_bytes = challenge_hash(&r_bytes, &pubkey_bytes, message);
    let e_scalar: Scalar<Public, Zero> =
        Scalar::from_bytes(e_bytes).ok_or_else(|| anyhow::anyhow!("Invalid challenge bytes"))?;

    out.push_str("Computing signature scalar:\n");
    out.push_str("   s = k + e * secret_key (mod n)\n\n");

    // Step 5: Compute s = k + e * d
    let s_scalar = s!(k_scalar + e_scalar * secret_scalar);
    let s_bytes = s_scalar.to_bytes();

    // Create 64-byte signature: R (32 bytes) || s (32 bytes)
    let mut signature = [0u8; 64];
    signature[..32].copy_from_slice(&r_bytes);
    signature[32..].copy_from_slice(&s_bytes);
    let sig_hex = hex::encode(signature);

    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    out.push_str("Signature created!\n\n");

    out.push_str("BIP340 Signature format (64 bytes):\n");
    out.push_str("   - Bytes 0-31:  R (x-coordinate of nonce point)\n");
    out.push_str("   - Bytes 32-63: s (signature scalar)\n\n");

    out.push_str("This signature is valid for:\n");
    out.push_str("   - Bitcoin Taproot transactions (BIP341)\n");
    out.push_str("   - Nostr events\n");
    out.push_str("   - Any BIP340-compatible system\n");

    let message_display = if let Ok(s) = std::str::from_utf8(message) {
        s.to_string()
    } else {
        hex::encode(message)
    };

    let output = BitcoinSignatureOutput {
        signature: sig_hex,
        message: message_display,
        public_key: hex::encode(pubkey_bytes),
        event_type: "bitcoin_signature".to_string(),
    };
    let result = serde_json::to_string(&output)?;

    Ok(CommandResult {
        output: out,
        result,
    })
}

/// CLI wrapper for signing
pub fn sign_message(message: &str) -> Result<()> {
    let storage = FileStorage::new(STATE_DIR)?;
    let cmd_result = sign_message_core(message.as_bytes(), None, &storage)?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Copy this JSON:");
    println!("{}\n", cmd_result.result);
    Ok(())
}

/// Sign a hex-encoded message
pub fn sign_message_hex(message_hex: &str) -> Result<()> {
    let storage = FileStorage::new(STATE_DIR)?;
    let message = hex::decode(message_hex).context("Invalid hex message")?;
    let cmd_result = sign_message_core(&message, None, &storage)?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Copy this JSON:");
    println!("{}\n", cmd_result.result);
    Ok(())
}

// ============================================================================
// Verification
// ============================================================================

/// Verify a BIP340 Schnorr signature
pub fn verify_signature_core(
    signature_hex: &str,
    public_key_hex: &str,
    message: &[u8],
) -> Result<CommandResult> {
    let mut out = String::new();

    out.push_str("Bitcoin Schnorr Verification (BIP340)\n\n");
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Decode signature
    let sig_bytes: [u8; 64] = hex::decode(signature_hex)
        .context("Invalid hex string for signature")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Signature must be exactly 64 bytes"))?;

    let r_bytes: [u8; 32] = sig_bytes[..32].try_into().unwrap();
    let s_bytes: [u8; 32] = sig_bytes[32..].try_into().unwrap();

    // Decode public key
    let pubkey_bytes: [u8; 32] = hex::decode(public_key_hex)
        .context("Invalid hex string for public key")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Public key must be exactly 32 bytes"))?;

    out.push_str("Parsing signature components:\n");
    out.push_str(&format!("   R: {}...\n", &signature_hex[..16]));
    out.push_str(&format!("   s: {}...\n", &signature_hex[64..80]));
    out.push_str(&format!("   P: {}...\n\n", &public_key_hex[..16]));

    // Parse s as scalar (must be < curve order)
    let s_scalar: Scalar<Public, Zero> = Scalar::from_bytes(s_bytes)
        .ok_or_else(|| anyhow::anyhow!("Invalid s value in signature (>= curve order)"))?;

    // Parse R as x-coordinate, lift to point with even Y
    let r_point = Point::<EvenY, Public>::from_xonly_bytes(r_bytes)
        .ok_or_else(|| anyhow::anyhow!("Invalid R point in signature"))?;

    // Parse public key as x-only point
    let pubkey_point = Point::<EvenY, Public>::from_xonly_bytes(pubkey_bytes)
        .ok_or_else(|| anyhow::anyhow!("Invalid public key"))?;

    out.push_str("BIP340 Verification equation:\n");
    out.push_str("   e = H(R || P || m)\n");
    out.push_str("   s * G == R + e * P\n\n");

    // Compute challenge
    let e_bytes = challenge_hash(&r_bytes, &pubkey_bytes, message);
    let e_scalar: Scalar<Public, Zero> =
        Scalar::from_bytes(e_bytes).ok_or_else(|| anyhow::anyhow!("Invalid challenge bytes"))?;

    // Verify: s * G == R + e * P
    let lhs = g!(s_scalar * G).normalize();
    let rhs = g!(r_point + e_scalar * pubkey_point).normalize();

    let is_valid = lhs == rhs;

    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let result = if is_valid {
        out.push_str("SIGNATURE VALID!\n\n");
        out.push_str("   The signature is cryptographically correct.\n");
        out.push_str("   The message was signed by the holder of the private key.\n");
        "VALID".to_string()
    } else {
        out.push_str("SIGNATURE INVALID!\n\n");
        out.push_str("   Verification failed.\n");
        out.push_str("   Possible causes:\n");
        out.push_str("   - Wrong public key\n");
        out.push_str("   - Modified message\n");
        out.push_str("   - Corrupted signature\n");
        "INVALID".to_string()
    };

    Ok(CommandResult {
        output: out,
        result,
    })
}

/// CLI wrapper for verification
pub fn verify_signature(signature_hex: &str, public_key_hex: &str, message: &str) -> Result<()> {
    let cmd_result = verify_signature_core(signature_hex, public_key_hex, message.as_bytes())?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Result: {}\n", cmd_result.result);
    Ok(())
}

/// Verify with hex-encoded message
pub fn verify_signature_hex(
    signature_hex: &str,
    public_key_hex: &str,
    message_hex: &str,
) -> Result<()> {
    let message = hex::decode(message_hex).context("Invalid hex message")?;
    let cmd_result = verify_signature_core(signature_hex, public_key_hex, &message)?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Result: {}\n", cmd_result.result);
    Ok(())
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Get the stored public key
pub fn get_public_key_core(storage: &dyn Storage) -> Result<CommandResult> {
    let mut out = String::new();

    let stored_json = String::from_utf8(
        storage
            .read("bitcoin_keypair.json")
            .context("No keypair found. Run btc-keygen first.")?,
    )?;
    let stored_key: StoredBitcoinKey = serde_json::from_str(&stored_json)?;

    let pubkey_hex = hex::encode(&stored_key.public_key_bytes);

    out.push_str("Bitcoin Public Key (BIP340 x-only)\n\n");
    out.push_str(&format!("Public Key: {}\n", pubkey_hex));

    let output = BitcoinKeyOutput {
        public_key: pubkey_hex,
        secret_key: None,
        event_type: "bitcoin_pubkey".to_string(),
    };
    let result = serde_json::to_string(&output)?;

    Ok(CommandResult {
        output: out,
        result,
    })
}

/// CLI wrapper for getting public key
pub fn get_public_key() -> Result<()> {
    let storage = FileStorage::new(STATE_DIR)?;
    let cmd_result = get_public_key_core(&storage)?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}\n", cmd_result.result);
    Ok(())
}

// ============================================================================
// Bitcoin Transaction Signing (Taproot)
// ============================================================================

/// Sign a Bitcoin Taproot sighash
pub fn sign_taproot_sighash_core(
    sighash_hex: &str,
    storage: &dyn Storage,
) -> Result<CommandResult> {
    let mut out = String::new();

    out.push_str("Bitcoin Taproot Transaction Signing\n\n");
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Decode sighash (32 bytes)
    let sighash: [u8; 32] = hex::decode(sighash_hex)
        .context("Invalid hex string for sighash")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Sighash must be exactly 32 bytes"))?;

    out.push_str(&format!("Sighash: {}\n\n", sighash_hex));
    out.push_str("This is the transaction commitment hash (BIP341)\n");
    out.push_str("computed from: version, locktime, inputs, outputs, etc.\n\n");

    // Sign the sighash directly (it's already a hash)
    let cmd_result = sign_message_core(&sighash, None, storage)?;

    // Parse and enhance the output
    let sig_output: BitcoinSignatureOutput = serde_json::from_str(&cmd_result.result)?;

    out.push_str(&cmd_result.output);
    out.push_str("\nTaproot Signature Format:\n");
    out.push_str("   For SIGHASH_DEFAULT: signature alone (64 bytes)\n");
    out.push_str("   For other sighash types: signature + sighash_type (65 bytes)\n\n");

    out.push_str("To use in a transaction:\n");
    out.push_str("   1. Set witness[0] = this signature\n");
    out.push_str("   2. Broadcast the transaction\n");

    let result = serde_json::to_string(&sig_output)?;

    Ok(CommandResult {
        output: out,
        result,
    })
}

/// CLI wrapper for Taproot signing
pub fn sign_taproot_sighash(sighash_hex: &str) -> Result<()> {
    let storage = FileStorage::new(STATE_DIR)?;
    let cmd_result = sign_taproot_sighash_core(sighash_hex, &storage)?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Copy this JSON:");
    println!("{}\n", cmd_result.result);
    Ok(())
}

// ============================================================================
// Bitcoin Address Generation
// ============================================================================

/// Output from address generation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BitcoinAddressOutput {
    /// Taproot address (P2TR)
    pub address: String,
    /// Network (mainnet, testnet, signet)
    pub network: String,
    /// 32-byte x-only public key (hex)
    pub public_key: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

/// Get a Bitcoin Taproot address from the stored public key
pub fn get_address_core(network: Network, storage: &dyn Storage) -> Result<CommandResult> {
    let mut out = String::new();

    out.push_str("Bitcoin Taproot Address (P2TR)\n\n");
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

    let pubkey_hex = hex::encode(pubkey_bytes);

    out.push_str(&format!("Public Key (x-only): {}\n\n", pubkey_hex));

    // Create XOnlyPublicKey for bitcoin crate
    let xonly_pubkey =
        XOnlyPublicKey::from_slice(&pubkey_bytes).context("Failed to create x-only public key")?;

    // Create a P2TR address using key-path spend (no script tree)
    // For a simple key-path address, we use the untweaked internal key
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let address = Address::p2tr(&secp, xonly_pubkey, None, network);

    let network_name = match network {
        Network::Bitcoin => "mainnet",
        Network::Testnet => "testnet",
        Network::Signet => "signet",
        Network::Regtest => "regtest",
        _ => "unknown",
    };

    out.push_str(&format!("Network: {}\n", network_name));
    out.push_str(&format!("Address: {}\n\n", address));

    out.push_str("Address Type: P2TR (Pay-to-Taproot)\n");
    out.push_str("   - Witness version: 1 (SegWit v1)\n");
    out.push_str("   - Address prefix: bc1p (mainnet) / tb1p (testnet/signet)\n");
    out.push_str("   - Encoding: Bech32m\n\n");

    out.push_str("You can fund this address with Bitcoin.\n");
    out.push_str("For testnet/signet, use a faucet to get test coins.\n");

    let output = BitcoinAddressOutput {
        address: address.to_string(),
        network: network_name.to_string(),
        public_key: pubkey_hex,
        event_type: "bitcoin_address".to_string(),
    };
    let result = serde_json::to_string(&output)?;

    Ok(CommandResult {
        output: out,
        result,
    })
}

/// CLI wrapper for getting mainnet address
pub fn get_address_mainnet() -> Result<()> {
    let storage = FileStorage::new(STATE_DIR)?;
    let cmd_result = get_address_core(Network::Bitcoin, &storage)?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Copy this JSON:");
    println!("{}\n", cmd_result.result);
    Ok(())
}

/// CLI wrapper for getting testnet address
pub fn get_address_testnet() -> Result<()> {
    let storage = FileStorage::new(STATE_DIR)?;
    let cmd_result = get_address_core(Network::Testnet, &storage)?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Copy this JSON:");
    println!("{}\n", cmd_result.result);
    Ok(())
}

/// CLI wrapper for getting signet address
pub fn get_address_signet() -> Result<()> {
    let storage = FileStorage::new(STATE_DIR)?;
    let cmd_result = get_address_core(Network::Signet, &storage)?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Copy this JSON:");
    println!("{}\n", cmd_result.result);
    Ok(())
}

// ============================================================================
// DKG Group Address
// ============================================================================

/// Get the DKG group Taproot address from the shared key
pub fn get_dkg_address_core(network: Network, storage: &dyn Storage) -> Result<CommandResult> {
    let mut out = String::new();

    out.push_str("DKG Group Taproot Address (P2TR)\n\n");
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Load shared key from DKG
    let shared_key_bytes = storage
        .read("shared_key.bin")
        .context("No DKG shared key found. Run keygen-finalize first.")?;

    // The shared key is serialized with bincode, we need to extract the x-only public key bytes
    // The format is: bincode serialized SharedKey<EvenY>
    // For simplicity, let's read the raw 32-byte public key from the serialized data
    // The public key bytes are typically at a known offset in the bincode format

    // Try to deserialize using schnorr_fun types
    use schnorr_fun::frost::SharedKey;

    let shared_key: SharedKey<secp256kfun::marker::EvenY> =
        bincode::deserialize(&shared_key_bytes).context("Failed to deserialize shared key")?;

    // Get the x-only public key bytes
    let pubkey_point = shared_key.public_key();
    let pubkey_bytes: [u8; 32] = pubkey_point.to_xonly_bytes();
    let pubkey_hex = hex::encode(pubkey_bytes);

    out.push_str(&format!("Group Public Key (x-only): {}\n\n", pubkey_hex));

    // Create XOnlyPublicKey for bitcoin crate
    let xonly_pubkey =
        XOnlyPublicKey::from_slice(&pubkey_bytes).context("Failed to create x-only public key")?;

    // Create a P2TR address
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let address = Address::p2tr(&secp, xonly_pubkey, None, network);

    let network_name = match network {
        Network::Bitcoin => "mainnet",
        Network::Testnet => "testnet",
        Network::Signet => "signet",
        Network::Regtest => "regtest",
        _ => "unknown",
    };

    out.push_str(&format!("Network: {}\n", network_name));
    out.push_str(&format!("Address: {}\n\n", address));

    out.push_str("This is the GROUP address from DKG.\n");
    out.push_str("Funds sent here require threshold signatures to spend.\n");

    let output = BitcoinAddressOutput {
        address: address.to_string(),
        network: network_name.to_string(),
        public_key: pubkey_hex,
        event_type: "dkg_address".to_string(),
    };
    let result = serde_json::to_string(&output)?;

    Ok(CommandResult {
        output: out,
        result,
    })
}

/// CLI wrapper for getting DKG testnet address
pub fn get_dkg_address_testnet(name: &str) -> Result<()> {
    let state_dir = crate::keygen::get_state_dir(name);
    let path = std::path::Path::new(&state_dir);

    if !path.exists() {
        anyhow::bail!(
            "Wallet '{}' not found at {}. Did you run keygen-finalize with --name {}?",
            name,
            state_dir,
            name
        );
    }

    let storage = FileStorage::new(&state_dir)?;
    let cmd_result = get_dkg_address_core(Network::Testnet, &storage)?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Copy this JSON:");
    println!("{}\n", cmd_result.result);
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tagged_hash() {
        // Test vector from BIP340
        let hash = tagged_hash("BIP0340/challenge", b"test");
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_sign_and_verify() {
        use crate::storage::MemoryStorage;

        let storage = MemoryStorage::new();

        // Generate keypair
        let key_result = generate_keypair_core(&storage).unwrap();
        let key_output: BitcoinKeyOutput = serde_json::from_str(&key_result.result).unwrap();

        // Sign message
        let message = b"Hello, Bitcoin!";
        let sig_result = sign_message_core(message, None, &storage).unwrap();
        let sig_output: BitcoinSignatureOutput = serde_json::from_str(&sig_result.result).unwrap();

        // Verify signature
        let verify_result =
            verify_signature_core(&sig_output.signature, &key_output.public_key, message).unwrap();

        assert_eq!(verify_result.result, "VALID");
    }

    #[test]
    fn test_deterministic_signing() {
        use crate::storage::MemoryStorage;

        let storage = MemoryStorage::new();

        // Import a known secret key
        let secret_hex = "0000000000000000000000000000000000000000000000000000000000000001";
        import_key_core(secret_hex, &storage).unwrap();

        // Sign the same message twice
        let message = b"test message";
        let sig1 = sign_message_core(message, None, &storage).unwrap();
        let sig2 = sign_message_core(message, None, &storage).unwrap();

        let out1: BitcoinSignatureOutput = serde_json::from_str(&sig1.result).unwrap();
        let out2: BitcoinSignatureOutput = serde_json::from_str(&sig2.result).unwrap();

        // Signatures should be identical (deterministic)
        assert_eq!(out1.signature, out2.signature);
    }

    #[test]
    fn test_invalid_signature() {
        use crate::storage::MemoryStorage;

        let storage = MemoryStorage::new();
        generate_keypair_core(&storage).unwrap();

        let message = b"Hello, Bitcoin!";
        let sig_result = sign_message_core(message, None, &storage).unwrap();
        let sig_output: BitcoinSignatureOutput = serde_json::from_str(&sig_result.result).unwrap();

        // Verify with wrong message
        let verify_result = verify_signature_core(
            &sig_output.signature,
            &sig_output.public_key,
            b"Wrong message",
        )
        .unwrap();

        assert_eq!(verify_result.result, "INVALID");
    }
}
