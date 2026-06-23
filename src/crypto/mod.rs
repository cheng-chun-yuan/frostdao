//! Cryptographic Primitives
//!
//! This module provides the core cryptographic building blocks for FROST/HTSS:
//!
//! - **birkhoff**: Birkhoff interpolation for hierarchical threshold schemes
//! - **hd**: BIP-32/BIP-86 hierarchical deterministic key derivation (Taproot)
//! - **helpers**: Utility functions (tagged hash, Lagrange coefficients, etc.)
//! - **mnemonic**: BIP-39 mnemonic seed phrase generation and parsing
//! - **nip44**: NIP-44 v2 E2E encryption for secure DKG share transmission

pub mod birkhoff;
pub mod hd;
pub mod helpers;
pub mod mnemonic;
pub mod nip44;
