//! BIP-39 Mnemonic Backup for FROST Master Keys
//!
//! Generates 24-word seed phrases for backing up secret shares.
//!
//! ## Important Security Note
//!
//! In threshold mode, each party backs up their OWN share independently.
//! The mnemonic encodes the share value, NOT the group private key.
//! Recovery still requires threshold shares to reconstruct the key.
//!
//! ## Usage
//!
//! ```ignore
//! // Generate mnemonic for a share
//! let mnemonic = share_to_mnemonic(&share_bytes)?;
//! println!("Backup: {}", mnemonic.to_string());
//!
//! // Restore share from mnemonic
//! let restored = mnemonic_to_share(&mnemonic)?;
//! ```

use anyhow::Result;
use bip39::{Language, Mnemonic};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha512;
use zeroize::Zeroize;

// ============================================================================
// Mnemonic Generation
// ============================================================================

/// Generate a new random 24-word BIP-39 mnemonic
///
/// Returns a cryptographically secure mnemonic with 256 bits of entropy.
/// Entropy is securely zeroized after mnemonic generation.
pub fn generate_mnemonic() -> Result<Mnemonic> {
    let mut entropy = [0u8; 32]; // 256 bits for 24 words
    rand::thread_rng().fill_bytes(&mut entropy);
    let result = Mnemonic::from_entropy_in(Language::English, &entropy)
        .map_err(|e| anyhow::anyhow!("Failed to generate mnemonic: {}", e));
    entropy.zeroize(); // Clear entropy from memory
    result
}

/// Generate a 12-word BIP-39 mnemonic (128 bits entropy)
///
/// Less secure but easier to write down. Use 24 words for production.
/// Entropy is securely zeroized after mnemonic generation.
pub fn generate_mnemonic_12() -> Result<Mnemonic> {
    let mut entropy = [0u8; 16]; // 128 bits for 12 words
    rand::thread_rng().fill_bytes(&mut entropy);
    let result = Mnemonic::from_entropy_in(Language::English, &entropy)
        .map_err(|e| anyhow::anyhow!("Failed to generate mnemonic: {}", e));
    entropy.zeroize(); // Clear entropy from memory
    result
}

// ============================================================================
// Mnemonic Conversion
// ============================================================================

/// Convert 32-byte secret share to 24-word mnemonic
///
/// The share bytes are used directly as entropy for BIP-39.
/// This allows backing up a scalar share as human-readable words.
pub fn share_to_mnemonic(share_bytes: &[u8; 32]) -> Result<Mnemonic> {
    Mnemonic::from_entropy_in(Language::English, share_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to create mnemonic from share: {}", e))
}

/// Convert 24-word mnemonic back to 32-byte share
///
/// Extracts the original entropy (share bytes) from the mnemonic.
pub fn mnemonic_to_share(mnemonic: &Mnemonic) -> Result<[u8; 32]> {
    let entropy = mnemonic.to_entropy();
    if entropy.len() != 32 {
        anyhow::bail!(
            "Expected 32-byte entropy (24 words), got {} bytes",
            entropy.len()
        );
    }
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&entropy);
    Ok(bytes)
}

/// Parse mnemonic from space-separated words
pub fn parse_mnemonic(words: &str) -> Result<Mnemonic> {
    Mnemonic::parse_in(Language::English, words)
        .map_err(|e| anyhow::anyhow!("Invalid mnemonic: {}", e))
}

/// Validate mnemonic words (checksum and wordlist)
pub fn validate_mnemonic(words: &str) -> bool {
    Mnemonic::parse_in(Language::English, words).is_ok()
}

/// Get word count from mnemonic
pub fn word_count(mnemonic: &Mnemonic) -> usize {
    mnemonic.word_count()
}

// ============================================================================
// BIP-39 Seed Derivation
// ============================================================================

/// Convert mnemonic to 512-bit seed using BIP-39 PBKDF2
///
/// The optional passphrase adds a second factor (the "25th word").
/// Different passphrases produce completely different seeds.
pub fn mnemonic_to_seed(mnemonic: &Mnemonic, passphrase: &str) -> [u8; 64] {
    mnemonic.to_seed(passphrase)
}

/// Derive master key and chain code from BIP-39 seed
///
/// Follows BIP-32: HMAC-SHA512("Bitcoin seed", seed)
/// Returns (master_key: 32 bytes, chain_code: 32 bytes)
pub fn seed_to_master_key(seed: &[u8; 64]) -> Result<([u8; 32], [u8; 32])> {
    let mut hmac =
        Hmac::<Sha512>::new_from_slice(b"Bitcoin seed").expect("HMAC accepts any key length");
    hmac.update(seed);
    let result = hmac.finalize().into_bytes();

    let master_key: [u8; 32] = result[..32].try_into().unwrap();
    let chain_code: [u8; 32] = result[32..].try_into().unwrap();

    Ok((master_key, chain_code))
}

// ============================================================================
// Full Workflow Helpers
// ============================================================================

/// Generate mnemonic and derive master key + chain code
///
/// Convenience function for new wallet creation.
pub fn generate_master_from_mnemonic(passphrase: &str) -> Result<(Mnemonic, [u8; 32], [u8; 32])> {
    let mnemonic = generate_mnemonic()?;
    let seed = mnemonic_to_seed(&mnemonic, passphrase);
    let (master_key, chain_code) = seed_to_master_key(&seed)?;
    Ok((mnemonic, master_key, chain_code))
}

/// Restore master key + chain code from mnemonic words
pub fn restore_master_from_words(
    words: &str,
    passphrase: &str,
) -> Result<(Mnemonic, [u8; 32], [u8; 32])> {
    let mnemonic = parse_mnemonic(words)?;
    let seed = mnemonic_to_seed(&mnemonic, passphrase);
    let (master_key, chain_code) = seed_to_master_key(&seed)?;
    Ok((mnemonic, master_key, chain_code))
}

// ============================================================================
// Display Helpers
// ============================================================================

/// Format mnemonic as numbered word list for display
pub fn format_mnemonic_numbered(mnemonic: &Mnemonic) -> String {
    mnemonic
        .words()
        .enumerate()
        .map(|(i, word)| format!("{:2}. {}", i + 1, word))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format mnemonic in 4-column grid
pub fn format_mnemonic_grid(mnemonic: &Mnemonic) -> String {
    let words: Vec<&str> = mnemonic.words().collect();
    let mut lines = Vec::new();

    for row in 0..6 {
        let cols: Vec<String> = (0..4)
            .filter_map(|col| {
                let idx = row + col * 6;
                words.get(idx).map(|w| format!("{:2}. {:12}", idx + 1, w))
            })
            .collect();
        lines.push(cols.join("  "));
    }

    lines.join("\n")
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mnemonic_24() {
        let mnemonic = generate_mnemonic().unwrap();
        assert_eq!(mnemonic.word_count(), 24);
    }

    #[test]
    fn test_generate_mnemonic_12() {
        let mnemonic = generate_mnemonic_12().unwrap();
        assert_eq!(mnemonic.word_count(), 12);
    }

    #[test]
    fn test_share_to_mnemonic_roundtrip() {
        let original_share = [0x42u8; 32];

        let mnemonic = share_to_mnemonic(&original_share).unwrap();
        assert_eq!(mnemonic.word_count(), 24);

        let restored = mnemonic_to_share(&mnemonic).unwrap();
        assert_eq!(restored, original_share);
    }

    #[test]
    fn test_parse_valid_mnemonic() {
        // Standard BIP-39 test vector
        let words = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let result = parse_mnemonic(words);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().word_count(), 12);
    }

    #[test]
    fn test_parse_invalid_mnemonic() {
        let words = "invalid words that are not in the bip39 wordlist";
        let result = parse_mnemonic(words);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_mnemonic() {
        let valid =
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        assert!(validate_mnemonic(valid));

        let invalid = "not a valid mnemonic phrase at all";
        assert!(!validate_mnemonic(invalid));
    }

    #[test]
    fn test_mnemonic_to_seed_deterministic() {
        let words = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic = parse_mnemonic(words).unwrap();

        let seed1 = mnemonic_to_seed(&mnemonic, "");
        let seed2 = mnemonic_to_seed(&mnemonic, "");

        assert_eq!(seed1, seed2);
    }

    #[test]
    fn test_passphrase_changes_seed() {
        let words = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic = parse_mnemonic(words).unwrap();

        let seed_no_pass = mnemonic_to_seed(&mnemonic, "");
        let seed_with_pass = mnemonic_to_seed(&mnemonic, "secret");

        assert_ne!(seed_no_pass, seed_with_pass);
    }

    #[test]
    fn test_seed_to_master_key() {
        let words = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic = parse_mnemonic(words).unwrap();
        let seed = mnemonic_to_seed(&mnemonic, "");

        let (master_key, chain_code) = seed_to_master_key(&seed).unwrap();

        assert_eq!(master_key.len(), 32);
        assert_eq!(chain_code.len(), 32);
        // Keys and chain codes should be different
        assert_ne!(master_key, chain_code);
    }

    #[test]
    fn test_format_mnemonic_numbered() {
        let mnemonic = generate_mnemonic_12().unwrap();
        let formatted = format_mnemonic_numbered(&mnemonic);

        assert!(formatted.contains(" 1. "));
        assert!(formatted.contains("12. "));
        assert_eq!(formatted.lines().count(), 12);
    }

    #[test]
    fn test_format_mnemonic_grid() {
        let mnemonic = generate_mnemonic().unwrap();
        let formatted = format_mnemonic_grid(&mnemonic);

        // Should have 6 rows
        assert_eq!(formatted.lines().count(), 6);
    }

    #[test]
    fn test_generate_master_from_mnemonic() {
        let (mnemonic, master_key, chain_code) = generate_master_from_mnemonic("").unwrap();

        assert_eq!(mnemonic.word_count(), 24);
        assert_eq!(master_key.len(), 32);
        assert_eq!(chain_code.len(), 32);
    }

    #[test]
    fn test_restore_master_from_words() {
        // First generate
        let (original_mnemonic, _, _) = generate_master_from_mnemonic("test").unwrap();
        let words = original_mnemonic.to_string();

        // Then restore
        let (restored_mnemonic, restored_key, restored_chain) =
            restore_master_from_words(&words, "test").unwrap();

        assert_eq!(original_mnemonic.to_string(), restored_mnemonic.to_string());

        // Generate again with same words should give same result
        let (_, expected_key, expected_chain) = generate_master_from_mnemonic("test").unwrap();

        // Note: These won't match because generate creates NEW random mnemonic
        // But restore should be deterministic
        let (_, key2, chain2) = restore_master_from_words(&words, "test").unwrap();
        assert_eq!(restored_key, key2);
        assert_eq!(restored_chain, chain2);
    }
}
