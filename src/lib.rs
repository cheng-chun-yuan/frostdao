use wasm_bindgen::prelude::*;

pub mod btc;
pub mod crypto;
pub mod protocol;
pub mod storage;
pub mod wasm;

// Nostr module (not available in WASM)
#[cfg(not(target_arch = "wasm32"))]
pub mod nostr;

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
