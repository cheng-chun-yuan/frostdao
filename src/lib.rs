pub mod audit;
pub mod btc;
pub mod crypto;
pub mod protocol;
pub mod storage;

pub mod nostr;

/// Result from a command, separating educational output from copy-paste result
#[derive(Debug, Clone)]
pub struct CommandResult {
    /// Educational output with explanations (🧠, ⚙️, ❄️, etc.)
    pub output: String,
    /// Clean JSON result for copy-pasting
    pub result: String,
}
