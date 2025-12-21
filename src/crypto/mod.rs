//! Cryptographic Primitives
//!
//! This module provides the core cryptographic building blocks for FROST/HTSS:
//!
//! - **birkhoff**: Birkhoff interpolation for hierarchical threshold schemes
//! - **helpers**: Utility functions (tagged hash, Lagrange coefficients, etc.)

pub mod birkhoff;
pub mod helpers;
