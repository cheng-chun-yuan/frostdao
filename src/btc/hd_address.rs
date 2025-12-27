//! HD-Derived Bitcoin Address Generation
//!
//! This module generates Bitcoin Taproot addresses at BIP-44 derivation paths
//! for threshold wallets. Each party can independently derive addresses using
//! the shared chain code without coordination.
//!
//! ## Usage
//!
//! ```ignore
//! let addresses = list_derived_addresses(&storage, 10, Network::Testnet)?;
//! for (addr, pubkey, index) in addresses {
//!     println!("m/44'/0'/0'/0/{}: {}", index, addr);
//! }
//! ```

use crate::crypto::hd::{derive_at_path, DerivationPath, HdContext};
use crate::protocol::keygen::HdMetadata;
use crate::storage::Storage;
use crate::CommandResult;
use anyhow::{Context, Result};
use bitcoin::{Address, Network, XOnlyPublicKey};
use schnorr_fun::frost::SharedKey;
use secp256kfun::prelude::*;

// ============================================================================
// Address Derivation
// ============================================================================

/// Derive Taproot address at a specific BIP-44 path
///
/// Returns (address, pubkey_hex)
pub fn derive_taproot_address(
    context: &HdContext,
    path: &DerivationPath,
    network: Network,
) -> Result<(Address, String)> {
    let derived = derive_at_path(context, path)?;
    let pubkey_bytes = derived.public_key.to_xonly_bytes();

    let xonly_pk = XOnlyPublicKey::from_slice(&pubkey_bytes)
        .map_err(|e| anyhow::anyhow!("Invalid derived public key: {}", e))?;

    let secp = bitcoin::secp256k1::Secp256k1::new();
    let address = Address::p2tr(&secp, xonly_pk, None, network);

    let pubkey_hex = hex::encode(pubkey_bytes);
    Ok((address, pubkey_hex))
}

/// List multiple derived addresses (external chain, change=0)
///
/// Returns Vec of (address_string, pubkey_hex, index)
pub fn list_derived_addresses(
    storage: &dyn Storage,
    count: u32,
    network: Network,
) -> Result<Vec<(String, String, u32)>> {
    let context = load_hd_context(storage)?;

    let mut addresses = Vec::new();

    // External addresses (change = 0)
    for i in 0..count {
        let path = DerivationPath::receive(i);
        let (addr, pubkey) = derive_taproot_address(&context, &path, network)?;
        addresses.push((addr.to_string(), pubkey, i));
    }

    Ok(addresses)
}

/// List change addresses (internal chain, change=1)
pub fn list_change_addresses(
    storage: &dyn Storage,
    count: u32,
    network: Network,
) -> Result<Vec<(String, String, u32)>> {
    let context = load_hd_context(storage)?;

    let mut addresses = Vec::new();

    for i in 0..count {
        let path = DerivationPath::change(i);
        let (addr, pubkey) = derive_taproot_address(&context, &path, network)?;
        addresses.push((addr.to_string(), pubkey, i));
    }

    Ok(addresses)
}

/// Derive address at specific path
pub fn derive_address_at_path(
    storage: &dyn Storage,
    change: u32,
    index: u32,
    network: Network,
) -> Result<(String, String)> {
    let context = load_hd_context(storage)?;
    let path = DerivationPath {
        change,
        address_index: index,
    };
    let (addr, pubkey) = derive_taproot_address(&context, &path, network)?;
    Ok((addr.to_string(), pubkey))
}

// ============================================================================
// Context Loading
// ============================================================================

/// Load HD context from storage
pub fn load_hd_context(storage: &dyn Storage) -> Result<HdContext> {
    // Load HD metadata
    let hd_json = String::from_utf8(storage.read("hd_metadata.json")?)
        .context("Failed to read hd_metadata.json")?;
    let hd_metadata: HdMetadata =
        serde_json::from_str(&hd_json).context("Failed to parse hd_metadata.json")?;

    if !hd_metadata.hd_enabled {
        anyhow::bail!("HD derivation is not enabled for this wallet");
    }

    // Load shared key for master pubkey
    let shared_key_bytes = storage.read("shared_key.bin")?;
    let shared_key: SharedKey<EvenY> =
        bincode::deserialize(&shared_key_bytes).context("Failed to deserialize shared key")?;

    // Parse chain code from hex
    let chain_code_bytes =
        hex::decode(&hd_metadata.chain_code).context("Invalid chain code hex")?;
    let chain_code: [u8; 32] = chain_code_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Chain code must be 32 bytes"))?;

    Ok(HdContext {
        chain_code,
        master_pubkey_bytes: shared_key.public_key().to_xonly_bytes(),
    })
}

// ============================================================================
// CLI Core Functions
// ============================================================================

/// Core function for dkg-derive-address command
pub fn derive_address_core(
    change: u32,
    index: u32,
    network_str: &str,
    storage: &dyn Storage,
) -> Result<CommandResult> {
    let mut out = String::new();

    let network = parse_network(network_str)?;
    let context = load_hd_context(storage)?;
    let path = DerivationPath {
        change,
        address_index: index,
    };

    out.push_str("HD Address Derivation\n\n");
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    out.push_str(&format!("Path: {}\n", path.to_full_string()));
    out.push_str(&format!("Network: {}\n\n", network_str));

    let (addr, pubkey) = derive_taproot_address(&context, &path, network)?;

    out.push_str(&format!("Public Key: {}\n", pubkey));
    out.push_str(&format!("Address: {}\n", addr));
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    Ok(CommandResult {
        output: out,
        result: addr.to_string(),
    })
}

/// Core function for dkg-list-addresses command
pub fn list_addresses_core(
    count: u32,
    network_str: &str,
    storage: &dyn Storage,
) -> Result<CommandResult> {
    let mut out = String::new();

    let network = parse_network(network_str)?;

    out.push_str("HD Derived Addresses\n\n");
    out.push_str(
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n",
    );
    out.push_str(&format!(
        "Network: {}  |  Showing first {} addresses\n\n",
        network_str, count
    ));

    let addresses = list_derived_addresses(storage, count, network)?;

    out.push_str("External Addresses (m/44'/0'/0'/0/*):\n");
    out.push_str("─────────────────────────────────────────────────────────────────────────────\n");

    for (addr, pubkey, idx) in &addresses {
        out.push_str(&format!("  {:>3}  {}  ({}...)\n", idx, addr, &pubkey[..16]));
    }

    out.push_str(
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n",
    );

    // Return first address as result
    let result = addresses
        .first()
        .map(|(addr, _, _)| addr.clone())
        .unwrap_or_default();

    Ok(CommandResult {
        output: out,
        result,
    })
}

// ============================================================================
// Address Count Management
// ============================================================================

/// Get current derived address count from HD metadata
pub fn get_derived_count(storage: &dyn Storage) -> Result<u32> {
    let hd_json = String::from_utf8(storage.read("hd_metadata.json")?)
        .context("Failed to read hd_metadata.json")?;
    let hd_metadata: HdMetadata =
        serde_json::from_str(&hd_json).context("Failed to parse hd_metadata.json")?;
    Ok(hd_metadata.derived_count)
}

/// Update derived address count (add or remove addresses)
pub fn update_derived_count(storage: &dyn Storage, new_count: u32) -> Result<()> {
    let hd_json = String::from_utf8(storage.read("hd_metadata.json")?)
        .context("Failed to read hd_metadata.json")?;
    let mut hd_metadata: HdMetadata =
        serde_json::from_str(&hd_json).context("Failed to parse hd_metadata.json")?;

    hd_metadata.derived_count = new_count.max(1); // Minimum 1 address

    storage.write(
        "hd_metadata.json",
        serde_json::to_string_pretty(&hd_metadata)?.as_bytes(),
    )?;

    Ok(())
}

/// Add a new derived address (increment count)
pub fn add_address(storage: &dyn Storage) -> Result<u32> {
    let current = get_derived_count(storage)?;
    let new_count = current + 1;
    update_derived_count(storage, new_count)?;
    Ok(new_count)
}

/// Remove the last derived address (decrement count, minimum 1)
pub fn remove_address(storage: &dyn Storage) -> Result<u32> {
    let current = get_derived_count(storage)?;
    if current > 1 {
        let new_count = current - 1;
        update_derived_count(storage, new_count)?;
        Ok(new_count)
    } else {
        Ok(1) // Keep at least 1 address
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Parse network string to bitcoin::Network
pub fn parse_network(network_str: &str) -> Result<Network> {
    match network_str.to_lowercase().as_str() {
        "mainnet" | "main" | "bitcoin" => Ok(Network::Bitcoin),
        "testnet" | "test" | "testnet3" => Ok(Network::Testnet),
        "signet" => Ok(Network::Signet),
        "regtest" | "local" => Ok(Network::Regtest),
        _ => anyhow::bail!(
            "Unknown network '{}'. Use: mainnet, testnet, signet, or regtest",
            network_str
        ),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_network() {
        assert!(matches!(
            parse_network("mainnet").unwrap(),
            Network::Bitcoin
        ));
        assert!(matches!(
            parse_network("testnet").unwrap(),
            Network::Testnet
        ));
        assert!(matches!(
            parse_network("MAINNET").unwrap(),
            Network::Bitcoin
        ));
        assert!(parse_network("invalid").is_err());
    }

    #[test]
    fn test_derivation_path_helpers() {
        let receive = DerivationPath::receive(5);
        assert_eq!(receive.change, 0);
        assert_eq!(receive.address_index, 5);

        let change = DerivationPath::change(3);
        assert_eq!(change.change, 1);
        assert_eq!(change.address_index, 3);
    }
}
