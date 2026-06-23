//! NIP-44 v2 Encryption
//!
//! Implements NIP-44 v2 encrypted direct messages for E2E encryption of DKG shares.
//! Uses ChaCha20-Poly1305 with HMAC-SHA256 and HKDF key derivation.
//!
//! References:
//! - https://github.com/nostr-protocol/nips/blob/master/44.md

use anyhow::{Context, Result};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use hkdf::Hkdf;
use rand::RngCore;
use secp256kfun::prelude::*;
use sha2::{Digest, Sha256};

/// NIP-44 v2 version byte
const NIP44_VERSION: u8 = 2;

/// Salt for HKDF key derivation (from NIP-44 spec)
const HKDF_SALT: &[u8] = b"nip44-v2";

/// Derives a shared secret using ECDH
/// Takes a secret scalar and a public key (x-only 32 bytes)
pub fn ecdh_shared_secret(secret: &[u8; 32], pubkey: &[u8; 32]) -> Result<[u8; 32]> {
    use secp256kfun::marker::NonZero;

    // Parse secret scalar (must be non-zero for valid key)
    let secret_scalar: Scalar<Secret, NonZero> =
        Scalar::from_bytes(*secret).context("Invalid secret scalar")?;

    // Parse x-only pubkey and lift to full point
    // We need to get the even-y version of the point
    let pubkey_point =
        Point::<EvenY>::from_xonly_bytes(*pubkey).context("Invalid x-only public key")?;

    // Compute ECDH: shared = secret * pubkey
    let shared_point = g!(secret_scalar * pubkey_point);

    // Get x-coordinate of shared point
    let shared_point_norm = shared_point.normalize();
    Ok(shared_point_norm.to_xonly_bytes())
}

/// Derives the conversation key from shared secret using HKDF
pub fn derive_conversation_key(shared_secret: &[u8; 32]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), shared_secret);
    let mut conversation_key = [0u8; 32];
    hk.expand(b"", &mut conversation_key)
        .expect("HKDF expand should not fail with 32-byte output");
    conversation_key
}

/// Derives the message keys (chacha key + nonce) from conversation key and nonce
fn derive_message_keys(conversation_key: &[u8; 32], nonce: &[u8; 32]) -> ([u8; 32], [u8; 12]) {
    // Concatenate conversation key and nonce for HKDF input
    let mut input = [0u8; 64];
    input[..32].copy_from_slice(conversation_key);
    input[32..].copy_from_slice(nonce);

    let hk = Hkdf::<Sha256>::new(None, &input);

    let mut chacha_key = [0u8; 32];
    let mut chacha_nonce = [0u8; 12];

    hk.expand(b"nip44-chacha", &mut chacha_key)
        .expect("HKDF expand should not fail");
    hk.expand(b"nip44-nonce", &mut chacha_nonce)
        .expect("HKDF expand should not fail");

    (chacha_key, chacha_nonce)
}

/// Calculate padding length per NIP-44 spec
fn calc_padded_len(unpadded_len: usize) -> usize {
    if unpadded_len <= 32 {
        return 32;
    }
    let next_power = (unpadded_len as u32).next_power_of_two();
    let chunk = (next_power / 8).max(32) as usize;
    ((unpadded_len + chunk - 1) / chunk) * chunk
}

/// Pads plaintext per NIP-44 spec
fn pad_plaintext(plaintext: &[u8]) -> Vec<u8> {
    let len = plaintext.len();
    let padded_len = calc_padded_len(len);

    // 2-byte big-endian length prefix
    let mut result = Vec::with_capacity(2 + padded_len);
    result.push((len >> 8) as u8);
    result.push(len as u8);
    result.extend_from_slice(plaintext);
    result.resize(2 + padded_len, 0); // Zero padding
    result
}

/// Unpads plaintext per NIP-44 spec
fn unpad_plaintext(padded: &[u8]) -> Result<Vec<u8>> {
    if padded.len() < 2 {
        anyhow::bail!("Padded plaintext too short");
    }

    let len = ((padded[0] as usize) << 8) | (padded[1] as usize);
    if len > padded.len() - 2 {
        anyhow::bail!("Invalid plaintext length in padding");
    }

    Ok(padded[2..2 + len].to_vec())
}

/// Encrypts a message using NIP-44 v2
/// Returns base64-encoded ciphertext
pub fn encrypt(plaintext: &[u8], conversation_key: &[u8; 32]) -> Result<String> {
    // Generate random nonce
    let mut nonce = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut nonce);

    encrypt_with_nonce(plaintext, conversation_key, &nonce)
}

/// Encrypts a message using NIP-44 v2 with specified nonce (for testing)
pub fn encrypt_with_nonce(
    plaintext: &[u8],
    conversation_key: &[u8; 32],
    nonce: &[u8; 32],
) -> Result<String> {
    // Derive message keys
    let (chacha_key, chacha_nonce) = derive_message_keys(conversation_key, nonce);

    // Pad plaintext
    let padded = pad_plaintext(plaintext);

    // Encrypt with ChaCha20-Poly1305
    let cipher =
        ChaCha20Poly1305::new_from_slice(&chacha_key).context("Invalid ChaCha20-Poly1305 key")?;
    let nonce_array = Nonce::from_slice(&chacha_nonce);
    let ciphertext = cipher
        .encrypt(nonce_array, padded.as_ref())
        .map_err(|e| anyhow::anyhow!("Encryption failed: {:?}", e))?;

    // Compute HMAC for authentication
    let mut hmac_input = Vec::new();
    hmac_input.push(NIP44_VERSION);
    hmac_input.extend_from_slice(nonce);
    hmac_input.extend_from_slice(&ciphertext);

    let mut hmac_hasher = Sha256::new();
    hmac_hasher.update(conversation_key);
    hmac_hasher.update(&hmac_input);
    let hmac = hmac_hasher.finalize();

    // Construct final payload: version + nonce + ciphertext + hmac
    let mut payload = Vec::new();
    payload.push(NIP44_VERSION);
    payload.extend_from_slice(nonce);
    payload.extend_from_slice(&ciphertext);
    payload.extend_from_slice(&hmac);

    Ok(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &payload,
    ))
}

/// Decrypts a NIP-44 v2 message
pub fn decrypt(ciphertext_b64: &str, conversation_key: &[u8; 32]) -> Result<Vec<u8>> {
    // Decode base64
    let payload =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, ciphertext_b64)
            .context("Invalid base64 encoding")?;

    // Check minimum length: version(1) + nonce(32) + min_ciphertext(32+16) + hmac(32) = 113
    if payload.len() < 113 {
        anyhow::bail!("Ciphertext too short");
    }

    // Parse payload
    let version = payload[0];
    if version != NIP44_VERSION {
        anyhow::bail!("Unsupported NIP-44 version: {}", version);
    }

    let nonce = &payload[1..33];
    let ciphertext = &payload[33..payload.len() - 32];
    let expected_hmac = &payload[payload.len() - 32..];

    // Verify HMAC
    let mut hmac_input = Vec::new();
    hmac_input.push(NIP44_VERSION);
    hmac_input.extend_from_slice(nonce);
    hmac_input.extend_from_slice(ciphertext);

    let mut hmac_hasher = Sha256::new();
    hmac_hasher.update(conversation_key);
    hmac_hasher.update(&hmac_input);
    let computed_hmac = hmac_hasher.finalize();

    if computed_hmac.as_slice() != expected_hmac {
        anyhow::bail!("HMAC verification failed");
    }

    // Derive message keys
    let mut nonce_arr = [0u8; 32];
    nonce_arr.copy_from_slice(nonce);
    let (chacha_key, chacha_nonce) = derive_message_keys(conversation_key, &nonce_arr);

    // Decrypt with ChaCha20-Poly1305
    let cipher =
        ChaCha20Poly1305::new_from_slice(&chacha_key).context("Invalid ChaCha20-Poly1305 key")?;
    let nonce_array = Nonce::from_slice(&chacha_nonce);
    let padded = cipher
        .decrypt(nonce_array, ciphertext)
        .map_err(|e| anyhow::anyhow!("Decryption failed: {:?}", e))?;

    // Unpad plaintext
    unpad_plaintext(&padded)
}

/// Encrypts a message for a recipient given sender's secret and recipient's pubkey
pub fn encrypt_for_recipient(
    plaintext: &[u8],
    sender_secret: &[u8; 32],
    recipient_pubkey: &[u8; 32],
) -> Result<String> {
    let shared_secret = ecdh_shared_secret(sender_secret, recipient_pubkey)?;
    let conversation_key = derive_conversation_key(&shared_secret);
    encrypt(plaintext, &conversation_key)
}

/// Decrypts a message from a sender given recipient's secret and sender's pubkey
pub fn decrypt_from_sender(
    ciphertext_b64: &str,
    recipient_secret: &[u8; 32],
    sender_pubkey: &[u8; 32],
) -> Result<Vec<u8>> {
    let shared_secret = ecdh_shared_secret(recipient_secret, sender_pubkey)?;
    let conversation_key = derive_conversation_key(&shared_secret);
    decrypt(ciphertext_b64, &conversation_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_padding() {
        // Test small message
        let small = b"hello";
        let padded = pad_plaintext(small);
        assert_eq!(padded.len(), 2 + 32); // 2-byte length + 32 min padding
        let unpadded = unpad_plaintext(&padded).unwrap();
        assert_eq!(unpadded, small);

        // Test exact 32-byte message
        let exact = [0u8; 32];
        let padded = pad_plaintext(&exact);
        assert_eq!(padded.len(), 2 + 32);
        let unpadded = unpad_plaintext(&padded).unwrap();
        assert_eq!(unpadded, exact);

        // Test larger message
        let large = [0u8; 100];
        let padded = pad_plaintext(&large);
        assert!(padded.len() > 2 + 100);
        let unpadded = unpad_plaintext(&padded).unwrap();
        assert_eq!(unpadded, large);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let plaintext = b"Hello, NIP-44!";
        let conversation_key = [42u8; 32];

        let ciphertext = encrypt(plaintext, &conversation_key).unwrap();
        let decrypted = decrypt(&ciphertext, &conversation_key).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_ecdh_key_exchange() {
        // Generate two key pairs
        use rand::RngCore;
        use secp256kfun::marker::NonZero;
        let mut rng = rand::thread_rng();

        let mut secret_a = [0u8; 32];
        let mut secret_b = [0u8; 32];
        rng.fill_bytes(&mut secret_a);
        rng.fill_bytes(&mut secret_b);

        // Derive public keys
        let scalar_a: Scalar<Secret, NonZero> = Scalar::from_bytes(secret_a).unwrap();
        let scalar_b: Scalar<Secret, NonZero> = Scalar::from_bytes(secret_b).unwrap();

        let pubkey_a = g!(scalar_a * G).normalize().to_xonly_bytes();
        let pubkey_b = g!(scalar_b * G).normalize().to_xonly_bytes();

        // ECDH should produce same shared secret both ways
        let shared_ab = ecdh_shared_secret(&secret_a, &pubkey_b).unwrap();
        let shared_ba = ecdh_shared_secret(&secret_b, &pubkey_a).unwrap();

        assert_eq!(shared_ab, shared_ba);
    }

    #[test]
    fn test_e2e_encryption() {
        // Simulate two parties
        use rand::RngCore;
        use secp256kfun::marker::NonZero;
        let mut rng = rand::thread_rng();

        let mut secret_sender = [0u8; 32];
        let mut secret_recipient = [0u8; 32];
        rng.fill_bytes(&mut secret_sender);
        rng.fill_bytes(&mut secret_recipient);

        // Derive public keys
        let scalar_sender: Scalar<Secret, NonZero> = Scalar::from_bytes(secret_sender).unwrap();
        let scalar_recipient: Scalar<Secret, NonZero> =
            Scalar::from_bytes(secret_recipient).unwrap();

        let pubkey_sender = g!(scalar_sender * G).normalize().to_xonly_bytes();
        let pubkey_recipient = g!(scalar_recipient * G).normalize().to_xonly_bytes();

        // Encrypt from sender to recipient
        let message = b"Secret share data";
        let ciphertext = encrypt_for_recipient(message, &secret_sender, &pubkey_recipient).unwrap();

        // Decrypt at recipient
        let decrypted =
            decrypt_from_sender(&ciphertext, &secret_recipient, &pubkey_sender).unwrap();

        assert_eq!(decrypted, message);
    }
}
