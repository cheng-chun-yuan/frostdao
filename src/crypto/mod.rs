//! Cryptographic Primitives
//!
//! This module provides the core cryptographic building blocks for FROST/HTSS:
//!
//! - **birkhoff**: Birkhoff interpolation for hierarchical threshold schemes
//! - **hd**: BIP-32/BIP-44 hierarchical deterministic key derivation
//! - **helpers**: Utility functions (tagged hash, Lagrange coefficients, etc.)
//! - **mnemonic**: BIP-39 mnemonic seed phrase generation and parsing

pub mod birkhoff;
pub mod hd;
pub mod helpers;
pub mod mnemonic;
