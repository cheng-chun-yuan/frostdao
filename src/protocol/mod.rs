//! FROST/HTSS Protocol Implementation
//!
//! This module implements the threshold signature protocols:
//!
//! - **keygen**: Distributed Key Generation (DKG)
//! - **signing**: Threshold signature creation and verification
//! - **reshare**: Key resharing to new party sets
//! - **recovery**: Lost share recovery
//! - **dkg_tx**: DKG-based Bitcoin transaction signing
//! - **signing_coordinator**: attempt-scoped nonce/share collection state

pub mod dkg_tx;
pub mod keygen;
pub mod recovery;
pub mod reshare;
pub mod signing;
pub mod signing_coordinator;

pub use signing_coordinator::{
    SigningAttemptCollector, SigningAttemptConfig, SigningCoordinator, SigningCoordinatorProgress,
    SigningCoordinatorStatus, SigningNonceInput, SigningSchemePolicy, SigningShareInput,
};
