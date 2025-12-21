//! Bitcoin Integration
//!
//! This module provides Bitcoin-specific functionality:
//!
//! - **schnorr**: BIP-340 Schnorr signatures and Taproot addresses
//! - **transaction**: Transaction building, signing, and broadcasting

pub mod schnorr;
pub mod transaction;
