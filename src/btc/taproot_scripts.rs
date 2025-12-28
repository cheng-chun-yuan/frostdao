//! Taproot Script Building
//!
//! This module provides builders for Taproot spending conditions:
//! - Timelocks (absolute CLTV and relative CSV)
//! - Recovery scripts (fallback after timeout)
//! - HTLC (Hash Time-Locked Contracts)

use anyhow::{Context, Result};
use bitcoin::key::XOnlyPublicKey;
use bitcoin::opcodes::all::*;
use bitcoin::script::Builder;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::taproot::{TaprootBuilder, TaprootSpendInfo};
use bitcoin::{Address, Network, ScriptBuf};

/// Script configuration for spending conditions
#[derive(Clone, Debug)]
pub enum SpendingCondition {
    /// No script - key path only
    KeyPathOnly,

    /// Absolute timelock (CHECKLOCKTIMEVERIFY)
    /// Funds can only be spent after block height
    TimelockAbsolute {
        /// Block height when funds become spendable
        lock_height: u32,
        /// Recipient public key (x-only, 32 bytes)
        recipient_pubkey: [u8; 32],
    },

    /// Relative timelock (CHECKSEQUENCEVERIFY)
    /// Funds can only be spent N blocks after UTXO confirmation
    TimelockRelative {
        /// Number of blocks to wait
        blocks: u16,
        /// Recipient public key (x-only, 32 bytes)
        recipient_pubkey: [u8; 32],
    },

    /// Recovery script with timeout fallback
    /// Primary owner can spend anytime, recovery key can spend after timeout
    Recovery {
        /// Primary owner public key
        owner_pubkey: [u8; 32],
        /// Recovery public key (backup/inheritance)
        recovery_pubkey: [u8; 32],
        /// Block height when recovery becomes active
        timeout_height: u32,
    },

    /// Hash Time-Locked Contract
    /// Recipient can claim with preimage, sender can refund after timeout
    Htlc {
        /// Hash of the preimage (SHA256)
        hash: [u8; 32],
        /// Recipient public key (claims with preimage)
        recipient_pubkey: [u8; 32],
        /// Refund public key (claims after timeout)
        refund_pubkey: [u8; 32],
        /// Block height when refund becomes possible
        timeout_height: u32,
    },
}

impl SpendingCondition {
    /// Build a timelock script with CHECKLOCKTIMEVERIFY
    /// Script: <height> OP_CLTV OP_DROP <pubkey> OP_CHECKSIG
    pub fn build_cltv_script(lock_height: u32, pubkey: &[u8; 32]) -> ScriptBuf {
        Builder::new()
            .push_int(lock_height as i64)
            .push_opcode(OP_CLTV)
            .push_opcode(OP_DROP)
            .push_slice(pubkey)
            .push_opcode(OP_CHECKSIG)
            .into_script()
    }

    /// Build a relative timelock script with CHECKSEQUENCEVERIFY
    /// Script: <blocks> OP_CSV OP_DROP <pubkey> OP_CHECKSIG
    pub fn build_csv_script(blocks: u16, pubkey: &[u8; 32]) -> ScriptBuf {
        Builder::new()
            .push_int(blocks as i64)
            .push_opcode(OP_CSV)
            .push_opcode(OP_DROP)
            .push_slice(pubkey)
            .push_opcode(OP_CHECKSIG)
            .into_script()
    }

    /// Build a recovery script (owner can spend anytime)
    /// Script: <owner_pubkey> OP_CHECKSIG
    pub fn build_owner_script(owner_pubkey: &[u8; 32]) -> ScriptBuf {
        Builder::new()
            .push_slice(owner_pubkey)
            .push_opcode(OP_CHECKSIG)
            .into_script()
    }

    /// Build a recovery fallback script (recovery key after timeout)
    /// Script: <timeout> OP_CLTV OP_DROP <recovery_pubkey> OP_CHECKSIG
    pub fn build_recovery_script(timeout_height: u32, recovery_pubkey: &[u8; 32]) -> ScriptBuf {
        Builder::new()
            .push_int(timeout_height as i64)
            .push_opcode(OP_CLTV)
            .push_opcode(OP_DROP)
            .push_slice(recovery_pubkey)
            .push_opcode(OP_CHECKSIG)
            .into_script()
    }

    /// Build HTLC claim script (recipient claims with preimage)
    /// Script: OP_SHA256 <hash> OP_EQUALVERIFY <recipient_pubkey> OP_CHECKSIG
    pub fn build_htlc_claim_script(hash: &[u8; 32], recipient_pubkey: &[u8; 32]) -> ScriptBuf {
        Builder::new()
            .push_opcode(OP_SHA256)
            .push_slice(hash)
            .push_opcode(OP_EQUALVERIFY)
            .push_slice(recipient_pubkey)
            .push_opcode(OP_CHECKSIG)
            .into_script()
    }

    /// Build HTLC refund script (sender refunds after timeout)
    /// Script: <timeout> OP_CLTV OP_DROP <refund_pubkey> OP_CHECKSIG
    pub fn build_htlc_refund_script(timeout_height: u32, refund_pubkey: &[u8; 32]) -> ScriptBuf {
        Builder::new()
            .push_int(timeout_height as i64)
            .push_opcode(OP_CLTV)
            .push_opcode(OP_DROP)
            .push_slice(refund_pubkey)
            .push_opcode(OP_CHECKSIG)
            .into_script()
    }

    /// Build Taproot spend info with the configured scripts
    ///
    /// Returns the TaprootSpendInfo containing:
    /// - Internal key (for key path spending if enabled)
    /// - Script tree with all configured spending conditions
    pub fn build_taproot_spend_info(
        &self,
        internal_key: &XOnlyPublicKey,
    ) -> Result<TaprootSpendInfo> {
        let secp = Secp256k1::new();

        match self {
            SpendingCondition::KeyPathOnly => {
                // No scripts, just key path
                TaprootBuilder::new()
                    .finalize(&secp, *internal_key)
                    .map_err(|e| anyhow::anyhow!("Failed to finalize taproot: {:?}", e))
            }

            SpendingCondition::TimelockAbsolute {
                lock_height,
                recipient_pubkey,
            } => {
                let script = Self::build_cltv_script(*lock_height, recipient_pubkey);
                TaprootBuilder::new()
                    .add_leaf(0, script)
                    .map_err(|e| anyhow::anyhow!("Failed to add leaf: {:?}", e))?
                    .finalize(&secp, *internal_key)
                    .map_err(|e| anyhow::anyhow!("Failed to finalize taproot: {:?}", e))
            }

            SpendingCondition::TimelockRelative {
                blocks,
                recipient_pubkey,
            } => {
                let script = Self::build_csv_script(*blocks, recipient_pubkey);
                TaprootBuilder::new()
                    .add_leaf(0, script)
                    .map_err(|e| anyhow::anyhow!("Failed to add leaf: {:?}", e))?
                    .finalize(&secp, *internal_key)
                    .map_err(|e| anyhow::anyhow!("Failed to finalize taproot: {:?}", e))
            }

            SpendingCondition::Recovery {
                owner_pubkey,
                recovery_pubkey,
                timeout_height,
            } => {
                // Two leaves: owner (no timelock) and recovery (with timelock)
                let owner_script = Self::build_owner_script(owner_pubkey);
                let recovery_script = Self::build_recovery_script(*timeout_height, recovery_pubkey);

                TaprootBuilder::new()
                    .add_leaf(1, owner_script)
                    .map_err(|e| anyhow::anyhow!("Failed to add owner leaf: {:?}", e))?
                    .add_leaf(1, recovery_script)
                    .map_err(|e| anyhow::anyhow!("Failed to add recovery leaf: {:?}", e))?
                    .finalize(&secp, *internal_key)
                    .map_err(|e| anyhow::anyhow!("Failed to finalize taproot: {:?}", e))
            }

            SpendingCondition::Htlc {
                hash,
                recipient_pubkey,
                refund_pubkey,
                timeout_height,
            } => {
                // Two leaves: claim (with preimage) and refund (after timeout)
                let claim_script = Self::build_htlc_claim_script(hash, recipient_pubkey);
                let refund_script = Self::build_htlc_refund_script(*timeout_height, refund_pubkey);

                TaprootBuilder::new()
                    .add_leaf(1, claim_script)
                    .map_err(|e| anyhow::anyhow!("Failed to add claim leaf: {:?}", e))?
                    .add_leaf(1, refund_script)
                    .map_err(|e| anyhow::anyhow!("Failed to add refund leaf: {:?}", e))?
                    .finalize(&secp, *internal_key)
                    .map_err(|e| anyhow::anyhow!("Failed to finalize taproot: {:?}", e))
            }
        }
    }

    /// Generate the P2TR address for this spending condition
    pub fn to_address(&self, internal_key: &XOnlyPublicKey, network: Network) -> Result<Address> {
        let secp = Secp256k1::new();
        let spend_info = self.build_taproot_spend_info(internal_key)?;
        let output_key = spend_info.output_key();

        Ok(Address::p2tr(
            &secp,
            output_key.to_x_only_public_key(),
            None,
            network,
        ))
    }

    /// Get the script pubkey for this spending condition
    pub fn script_pubkey(&self, internal_key: &XOnlyPublicKey) -> Result<ScriptBuf> {
        let spend_info = self.build_taproot_spend_info(internal_key)?;
        let output_key = spend_info.output_key();
        Ok(ScriptBuf::new_p2tr_tweaked(output_key))
    }
}

/// Parse a hex public key string into 32-byte array
pub fn parse_pubkey_hex(hex_str: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(hex_str).context("Invalid hex")?;
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Public key must be 32 bytes"))
}

/// Parse a hex hash string into 32-byte array
pub fn parse_hash_hex(hex_str: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(hex_str).context("Invalid hex")?;
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Hash must be 32 bytes"))
}

/// Simple script type for input parsing (avoids TUI dependency)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScriptTypeInput {
    /// Standard key path spending (no script)
    #[default]
    None,
    /// Absolute timelock (CLTV)
    TimelockAbsolute,
    /// Relative timelock (CSV)
    TimelockRelative,
    /// Recovery script
    Recovery,
    /// Hash Time-Locked Contract
    Htlc,
}

/// Parameters for building spending conditions
#[derive(Clone, Debug, Default)]
pub struct ScriptParams {
    /// Script type
    pub script_type: ScriptTypeInput,
    /// Absolute timelock: block height
    pub timelock_height: Option<u32>,
    /// Relative timelock: number of blocks
    pub timelock_blocks: Option<u16>,
    /// Recovery/HTLC: timeout in blocks
    pub timeout: Option<u32>,
    /// Recovery: pubkey (x-only, 32 bytes)
    pub recovery_pubkey: Option<[u8; 32]>,
    /// HTLC: hash (SHA256, 32 bytes)
    pub htlc_hash: Option<[u8; 32]>,
    /// HTLC: refund pubkey (x-only, 32 bytes)
    pub htlc_refund_pubkey: Option<[u8; 32]>,
}

impl ScriptParams {
    /// Create script params from string inputs (from TUI)
    pub fn from_strings(
        script_type: ScriptTypeInput,
        timelock_height: &str,
        timelock_blocks: &str,
        timeout: &str,
        recovery_pubkey: &str,
        htlc_hash: &str,
        htlc_refund_pubkey: &str,
    ) -> Result<Self> {
        Ok(Self {
            script_type,
            timelock_height: if timelock_height.is_empty() {
                None
            } else {
                Some(timelock_height.parse().context("Invalid block height")?)
            },
            timelock_blocks: if timelock_blocks.is_empty() {
                None
            } else {
                Some(timelock_blocks.parse().context("Invalid block count")?)
            },
            timeout: if timeout.is_empty() {
                None
            } else {
                Some(timeout.parse().context("Invalid timeout")?)
            },
            recovery_pubkey: if recovery_pubkey.is_empty() {
                None
            } else {
                Some(parse_pubkey_hex(recovery_pubkey)?)
            },
            htlc_hash: if htlc_hash.is_empty() {
                None
            } else {
                Some(parse_hash_hex(htlc_hash)?)
            },
            htlc_refund_pubkey: if htlc_refund_pubkey.is_empty() {
                None
            } else {
                Some(parse_pubkey_hex(htlc_refund_pubkey)?)
            },
        })
    }

    /// Convert to SpendingCondition
    ///
    /// The recipient_pubkey is the key that will be able to spend under the conditions.
    pub fn to_spending_condition(&self, recipient_pubkey: &[u8; 32]) -> Result<SpendingCondition> {
        match self.script_type {
            ScriptTypeInput::None => Ok(SpendingCondition::KeyPathOnly),

            ScriptTypeInput::TimelockAbsolute => {
                let height = self.timelock_height.ok_or_else(|| {
                    anyhow::anyhow!("Block height required for absolute timelock")
                })?;
                Ok(SpendingCondition::TimelockAbsolute {
                    lock_height: height,
                    recipient_pubkey: *recipient_pubkey,
                })
            }

            ScriptTypeInput::TimelockRelative => {
                let blocks = self
                    .timelock_blocks
                    .ok_or_else(|| anyhow::anyhow!("Block count required for relative timelock"))?;
                Ok(SpendingCondition::TimelockRelative {
                    blocks,
                    recipient_pubkey: *recipient_pubkey,
                })
            }

            ScriptTypeInput::Recovery => {
                let timeout = self
                    .timeout
                    .ok_or_else(|| anyhow::anyhow!("Timeout required for recovery script"))?;
                let recovery_pk = self
                    .recovery_pubkey
                    .ok_or_else(|| anyhow::anyhow!("Recovery pubkey required"))?;
                Ok(SpendingCondition::Recovery {
                    owner_pubkey: *recipient_pubkey,
                    recovery_pubkey: recovery_pk,
                    timeout_height: timeout,
                })
            }

            ScriptTypeInput::Htlc => {
                let hash = self
                    .htlc_hash
                    .ok_or_else(|| anyhow::anyhow!("Hash required for HTLC"))?;
                let timeout = self
                    .timeout
                    .ok_or_else(|| anyhow::anyhow!("Timeout required for HTLC"))?;
                let refund_pk = self
                    .htlc_refund_pubkey
                    .ok_or_else(|| anyhow::anyhow!("Refund pubkey required for HTLC"))?;
                Ok(SpendingCondition::Htlc {
                    hash,
                    recipient_pubkey: *recipient_pubkey,
                    refund_pubkey: refund_pk,
                    timeout_height: timeout,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cltv_script() {
        let pubkey = [0u8; 32];
        let script = SpendingCondition::build_cltv_script(800000, &pubkey);
        assert!(!script.is_empty());
        // Script should contain CLTV opcode
        assert!(script.as_bytes().contains(&0xb1)); // OP_CLTV
    }

    #[test]
    fn test_csv_script() {
        let pubkey = [0u8; 32];
        let script = SpendingCondition::build_csv_script(144, &pubkey);
        assert!(!script.is_empty());
        // Script should contain CSV opcode
        assert!(script.as_bytes().contains(&0xb2)); // OP_CSV
    }

    #[test]
    fn test_htlc_claim_script() {
        let hash = [0u8; 32];
        let pubkey = [0u8; 32];
        let script = SpendingCondition::build_htlc_claim_script(&hash, &pubkey);
        assert!(!script.is_empty());
        // Script should contain SHA256 opcode
        assert!(script.as_bytes().contains(&0xa8)); // OP_SHA256
    }
}
