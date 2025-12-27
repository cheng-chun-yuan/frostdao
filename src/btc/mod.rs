//! Bitcoin Integration
//!
//! This module provides Bitcoin-specific functionality:
//!
//! - **hd_address**: BIP-32/BIP-44 HD address derivation
//! - **schnorr**: BIP-340 Schnorr signatures and Taproot addresses
//! - **transaction**: Transaction building, signing, and broadcasting

pub mod hd_address;
pub mod schnorr;
pub mod transaction;
