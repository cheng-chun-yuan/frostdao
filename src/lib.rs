use wasm_bindgen::prelude::*;

pub mod birkhoff;
pub mod bitcoin_schnorr;
pub mod bitcoin_tx;
pub mod dkg_tx;
pub mod keygen;
pub mod recovery;
pub mod reshare;
pub mod signing;
pub mod storage;
pub mod wasm;

// Re-export WASM functions
pub use wasm::*;

/// Result from a command, separating educational output from copy-paste result
#[derive(Debug, Clone)]
pub struct CommandResult {
    /// Educational output with explanations (🧠, ⚙️, ❄️, etc.)
    pub output: String,
    /// Clean JSON result for copy-pasting
    pub result: String,
}

// Test function to verify WASM compilation works
#[wasm_bindgen]
pub fn test_wasm() -> String {
    "WASM is working!".to_string()
}
