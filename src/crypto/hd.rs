//! BIP-32/BIP-44 HD Key Derivation for FROST Threshold Signatures
//!
//! This module implements non-hardened HD key derivation compatible with
//! threshold signing. Each party derives child keys locally using the
//! public chain_code, allowing coordination-free address generation.
//!
//! ## Key Insight
//!
//! BIP-32 non-hardened derivation: `child_key = parent_key + IL`
//! This is structurally identical to Taproot tweaking, allowing each
//! threshold party to independently derive child keys.
//!
//! ## Usage
//!
//! ```ignore
//! let context = HdContext { chain_code, master_pubkey };
//! let path = DerivationPath { change: 0, address_index: 5 };
//! let derived = derive_at_path(&context, &path)?;
//! let child_share = derive_share(&paired_share, &derived)?;
//! ```

use crate::crypto::helpers::{construct_paired_secret_share, negate_paired_secret_share};
use anyhow::Result;
use hmac::{Hmac, Mac};
use schnorr_fun::frost::PairedSecretShare;
use secp256kfun::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::Sha512;

// ============================================================================
// Data Structures
// ============================================================================

/// HD derivation context storing master key info and chain code
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HdContext {
    /// 32-byte chain code for derivation (hex encoded for JSON)
    #[serde(with = "hex_bytes")]
    pub chain_code: [u8; 32],
    /// The master (root) public key bytes
    #[serde(with = "hex_bytes")]
    pub master_pubkey_bytes: [u8; 32],
}

/// BIP-44 derivation path components
///
/// Full path: m/44'/0'/0'/change/address_index
/// We only support non-hardened derivation for the last two levels.
/// The hardened portion (m/44'/0'/0') is implicit in the DKG setup.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DerivationPath {
    /// 0 for external (receive), 1 for internal (change)
    pub change: u32,
    /// Address index within the change level
    pub address_index: u32,
}

impl DerivationPath {
    /// Create path for receiving address at index
    pub fn receive(index: u32) -> Self {
        Self {
            change: 0,
            address_index: index,
        }
    }

    /// Create path for change address at index
    pub fn change(index: u32) -> Self {
        Self {
            change: 1,
            address_index: index,
        }
    }

    /// Format as BIP-44 style string (relative to account)
    pub fn to_string(&self) -> String {
        format!("{}/{}", self.change, self.address_index)
    }

    /// Format as full BIP-44 path (assuming Bitcoin mainnet account 0)
    pub fn to_full_string(&self) -> String {
        format!("m/44'/0'/0'/{}/{}", self.change, self.address_index)
    }
}

/// Derived key information
#[derive(Clone, Debug)]
pub struct DerivedKeyInfo {
    /// The derived public key with even Y (BIP-340 compatible)
    pub public_key: Point<EvenY>,
    /// The accumulated tweak from derivation
    pub tweak: Scalar<Public, Zero>,
    /// The chain code at this level (for further derivation)
    pub chain_code: [u8; 32],
    /// Whether the key was negated for even Y
    pub parity_flip: bool,
}

// ============================================================================
// Core Derivation Functions
// ============================================================================

/// Compute BIP-32 child key derivation tweak (non-hardened only)
///
/// IL || IR = HMAC-SHA512(chain_code, 0x02 || parent_pubkey || index)
///
/// Returns (IL as scalar tweak, IR as new chain_code)
///
/// # Arguments
/// * `chain_code` - 32-byte chain code from parent
/// * `parent_pubkey` - Parent public key (EvenY format)
/// * `index` - Child index (must be < 2^31 for non-hardened)
///
/// # Errors
/// Returns error if index is hardened (>= 2^31)
pub fn derive_child_tweak(
    chain_code: &[u8; 32],
    parent_pubkey: &Point<EvenY>,
    index: u32,
) -> Result<(Scalar<Public, Zero>, [u8; 32])> {
    // Ensure index is non-hardened (< 2^31)
    if index >= 0x80000000 {
        anyhow::bail!(
            "Hardened derivation (index >= 2^31) not supported in threshold mode. \
             Use non-hardened indices only."
        );
    }

    // HMAC-SHA512(chain_code, 0x02 || compressed_pubkey || index)
    // For EvenY points, the prefix is always 0x02
    let mut hmac = Hmac::<Sha512>::new_from_slice(chain_code).expect("HMAC accepts any key length");

    hmac.update(&[0x02]); // Compressed pubkey prefix for even Y
    hmac.update(&parent_pubkey.to_xonly_bytes());
    hmac.update(&index.to_be_bytes());

    let result = hmac.finalize().into_bytes();

    // Split into IL (tweak) and IR (new chain code)
    let il: [u8; 32] = result[..32].try_into().unwrap();
    let ir: [u8; 32] = result[32..].try_into().unwrap();

    // Convert IL to scalar - if invalid (>= curve order), derivation fails
    let tweak: Scalar<Public, Zero> = Scalar::from_bytes(il).ok_or_else(|| {
        anyhow::anyhow!("Invalid tweak scalar (>= curve order) at index {}", index)
    })?;

    Ok((tweak, ir))
}

/// Derive a child public key from parent public key
///
/// child_pubkey = parent_pubkey + tweak * G
///
/// Returns (child_pubkey, new_chain_code, tweak, parity_flip)
pub fn derive_child_pubkey(
    parent_pubkey: &Point<EvenY>,
    chain_code: &[u8; 32],
    index: u32,
) -> Result<(Point<EvenY>, [u8; 32], Scalar<Public, Zero>, bool)> {
    let (tweak, new_chain_code) = derive_child_tweak(chain_code, parent_pubkey, index)?;

    // child_pubkey = parent_pubkey + tweak * G
    let child_point = g!({ *parent_pubkey } + tweak * G).normalize();

    let child_nonzero = child_point
        .non_zero()
        .ok_or_else(|| anyhow::anyhow!("Derived point at infinity at index {}", index))?;

    // Ensure even Y for BIP-340 compatibility
    let (child_even_y, parity_flip) = child_nonzero.into_point_with_even_y();

    Ok((child_even_y, new_chain_code, tweak, parity_flip))
}

/// Derive public key at full BIP-44 path: m/44'/0'/0'/change/address_index
///
/// NOTE: We assume the account-level key (m/44'/0'/0') is the master key
/// stored after keygen. The hardened levels are implicit in the DKG setup.
pub fn derive_at_path(context: &HdContext, path: &DerivationPath) -> Result<DerivedKeyInfo> {
    // Reconstruct master pubkey from bytes
    let master_pubkey = Point::<EvenY>::from_xonly_bytes(context.master_pubkey_bytes)
        .ok_or_else(|| anyhow::anyhow!("Invalid master public key bytes"))?;

    // First derivation: m/.../change
    let (change_pubkey, change_chain_code, tweak1, flip1) =
        derive_child_pubkey(&master_pubkey, &context.chain_code, path.change)?;

    // Second derivation: m/.../change/address_index
    let (final_pubkey, final_chain_code, tweak2, flip2) =
        derive_child_pubkey(&change_pubkey, &change_chain_code, path.address_index)?;

    // Accumulate tweaks (accounting for parity flips)
    // If the intermediate key was flipped, the tweak relationship changes
    let accumulated_tweak = if flip1 {
        // When change level flipped, we need tweak1 - tweak2 so that after
        // the parity flip negation in derive_share, the final result is correct:
        // -(s + tweak1 - tweak2) = -s - tweak1 + tweak2, matching the derived key
        s!(tweak1 - tweak2).public()
    } else {
        s!(tweak1 + tweak2).public()
    };

    Ok(DerivedKeyInfo {
        public_key: final_pubkey,
        tweak: accumulated_tweak,
        chain_code: final_chain_code,
        parity_flip: flip1 ^ flip2, // XOR for combined parity
    })
}

/// Derive at a single level (for more granular control)
pub fn derive_single_level(
    pubkey: &Point<EvenY>,
    chain_code: &[u8; 32],
    index: u32,
) -> Result<DerivedKeyInfo> {
    let (child_pubkey, new_chain_code, tweak, parity_flip) =
        derive_child_pubkey(pubkey, chain_code, index)?;

    Ok(DerivedKeyInfo {
        public_key: child_pubkey,
        tweak,
        chain_code: new_chain_code,
        parity_flip,
    })
}

// ============================================================================
// Share Derivation (Threshold-Compatible)
// ============================================================================

/// Apply HD derivation tweak to a secret share
///
/// This is the threshold-compatible operation: each party applies the same
/// public tweak to their share locally, resulting in shares for the derived key.
///
/// derived_share = original_share + tweak (mod curve_order)
///
/// # Arguments
/// * `paired_share` - Original secret share from DKG
/// * `derived_info` - Derivation result containing tweak and target pubkey
///
/// # Returns
/// New PairedSecretShare for the derived key
pub fn derive_share(
    paired_share: &PairedSecretShare<EvenY>,
    derived_info: &DerivedKeyInfo,
) -> Result<PairedSecretShare<EvenY>> {
    // Get original share components
    let secret_share = paired_share.secret_share();
    let index_bytes = secret_share.index.to_bytes();
    // Scalars are stored big-endian; extract u32 from last 4 bytes
    let index = u32::from_be_bytes(index_bytes[28..32].try_into().unwrap());

    // Apply tweak: derived_share = original_share + tweak
    let original_share = secret_share.share;
    let derived_share_value = s!(original_share + { derived_info.tweak });

    let derived_share_nonzero = derived_share_value
        .non_zero()
        .ok_or_else(|| anyhow::anyhow!("Derived share is zero (extremely unlikely)"))?;

    // Construct new paired share with derived values
    let mut derived_paired =
        construct_paired_secret_share(index, derived_share_nonzero, &derived_info.public_key)?;

    // Handle parity: if derived key needed negation for even Y, negate the share
    if derived_info.parity_flip {
        derived_paired = negate_paired_secret_share(&derived_paired)?;
    }

    Ok(derived_paired)
}

// ============================================================================
// Helper: Hex Serialization for [u8; 32]
// ============================================================================

mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
        bytes
            .try_into()
            .map_err(|_| serde::de::Error::custom("expected 32 bytes"))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn random_keypair() -> (Scalar<Secret, NonZero>, Point<EvenY>) {
        let mut rng = rand::thread_rng();
        let secret = Scalar::<Secret, NonZero>::random(&mut rng);
        let pubkey = g!(secret * G)
            .normalize()
            .non_zero()
            .unwrap()
            .into_point_with_even_y()
            .0;
        (secret, pubkey)
    }

    #[test]
    fn test_child_derivation_deterministic() {
        let chain_code = [42u8; 32];
        let (_, pubkey) = random_keypair();

        let (tweak1, cc1) = derive_child_tweak(&chain_code, &pubkey, 0).unwrap();
        let (tweak2, cc2) = derive_child_tweak(&chain_code, &pubkey, 0).unwrap();

        assert_eq!(tweak1.to_bytes(), tweak2.to_bytes());
        assert_eq!(cc1, cc2);
    }

    #[test]
    fn test_different_indices_produce_different_keys() {
        let chain_code = [42u8; 32];
        let (_, pubkey) = random_keypair();

        let (tweak0, _) = derive_child_tweak(&chain_code, &pubkey, 0).unwrap();
        let (tweak1, _) = derive_child_tweak(&chain_code, &pubkey, 1).unwrap();

        assert_ne!(tweak0.to_bytes(), tweak1.to_bytes());
    }

    #[test]
    fn test_hardened_derivation_rejected() {
        let chain_code = [42u8; 32];
        let (_, pubkey) = random_keypair();

        // Hardened index should fail
        let result = derive_child_tweak(&chain_code, &pubkey, 0x80000000);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Hardened"));
    }

    #[test]
    fn test_threshold_derivation_consistency() {
        // Verify that the derived pubkey can be computed from master + tweak*G
        // This tests the core property needed for threshold signing:
        // child_pubkey = master_pubkey + tweak * G (then normalized to EvenY)
        let (_, master_pubkey) = random_keypair();
        let chain_code = [42u8; 32];

        // Derive child pubkey through our function
        let (child_pubkey, _, tweak, _parity_flip) =
            derive_child_pubkey(&master_pubkey, &chain_code, 5).unwrap();

        // The child pubkey should be master_pubkey + tweak*G (normalized to EvenY)
        let expected_child_point = g!({ master_pubkey } + tweak * G).normalize();
        let expected_child_nonzero = expected_child_point.non_zero().unwrap();
        let (expected_child_even_y, _) = expected_child_nonzero.into_point_with_even_y();

        // Both should produce the same x-only pubkey
        assert_eq!(
            child_pubkey.to_xonly_bytes(),
            expected_child_even_y.to_xonly_bytes(),
            "Derived pubkey should match expected (master + tweak*G)"
        );

        // Verify derivation is consistent (same inputs = same outputs)
        let (child_pubkey2, chain2, tweak2, parity2) =
            derive_child_pubkey(&master_pubkey, &chain_code, 5).unwrap();
        assert_eq!(
            child_pubkey.to_xonly_bytes(),
            child_pubkey2.to_xonly_bytes()
        );
        assert_eq!(tweak.to_bytes(), tweak2.to_bytes());

        // Different index produces different result
        let (child_pubkey3, _, _, _) = derive_child_pubkey(&master_pubkey, &chain_code, 6).unwrap();
        assert_ne!(
            child_pubkey.to_xonly_bytes(),
            child_pubkey3.to_xonly_bytes()
        );
    }

    #[test]
    fn test_derive_at_path() {
        let (_, master_pubkey) = random_keypair();
        let chain_code = [1u8; 32];

        let context = HdContext {
            chain_code,
            master_pubkey_bytes: master_pubkey.to_xonly_bytes(),
        };

        // Derive at path 0/0
        let path = DerivationPath::receive(0);
        let derived = derive_at_path(&context, &path).unwrap();

        // Should produce a valid pubkey
        assert!(derived.public_key.to_xonly_bytes().len() == 32);

        // Different paths should produce different keys
        let path2 = DerivationPath::receive(1);
        let derived2 = derive_at_path(&context, &path2).unwrap();

        assert_ne!(
            derived.public_key.to_xonly_bytes(),
            derived2.public_key.to_xonly_bytes()
        );
    }

    #[test]
    fn test_derivation_path_formatting() {
        let path = DerivationPath {
            change: 0,
            address_index: 5,
        };
        assert_eq!(path.to_string(), "0/5");
        assert_eq!(path.to_full_string(), "m/44'/0'/0'/0/5");

        let change_path = DerivationPath::change(3);
        assert_eq!(change_path.to_full_string(), "m/44'/0'/0'/1/3");
    }

    #[test]
    fn test_hd_context_serialization() {
        let context = HdContext {
            chain_code: [0xab; 32],
            master_pubkey_bytes: [0xcd; 32],
        };

        let json = serde_json::to_string(&context).unwrap();
        assert!(json.contains("abab")); // hex encoding

        let parsed: HdContext = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.chain_code, context.chain_code);
        assert_eq!(parsed.master_pubkey_bytes, context.master_pubkey_bytes);
    }
}
