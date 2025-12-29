//! Bitcoin Integration
//!
//! This module provides Bitcoin-specific functionality:
//!
//! - **hd_address**: BIP-32/BIP-86 HD address derivation (Taproot)
//! - **schnorr**: BIP-340 Schnorr signatures and Taproot addresses
//! - **taproot_scripts**: Taproot script building (timelocks, HTLC, recovery)
//! - **transaction**: Transaction building, signing, and broadcasting

pub mod hd_address;
pub mod schnorr;
pub mod taproot_scripts;
pub mod transaction;
