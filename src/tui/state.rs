//! TUI state definitions
//!
//! Defines all states for the TUI state machine.

use bitcoin::Network;

/// Network selection for the TUI
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum NetworkSelection {
    #[default]
    Testnet,
    Signet,
    Mainnet,
}

impl NetworkSelection {
    pub fn to_bitcoin_network(&self) -> Network {
        match self {
            Self::Testnet => Network::Testnet,
            Self::Signet => Network::Signet,
            Self::Mainnet => Network::Bitcoin,
        }
    }

    pub fn mempool_api_base(&self) -> &'static str {
        match self {
            Self::Testnet => "https://mempool.space/testnet/api",
            Self::Signet => "https://mempool.space/signet/api",
            Self::Mainnet => "https://mempool.space/api",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Testnet => "Testnet",
            Self::Signet => "Signet",
            Self::Mainnet => "Mainnet",
        }
    }

    pub fn all() -> &'static [NetworkSelection] {
        &[Self::Testnet, Self::Signet, Self::Mainnet]
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Testnet => Self::Signet,
            Self::Signet => Self::Mainnet,
            Self::Mainnet => Self::Testnet,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Testnet => Self::Mainnet,
            Self::Signet => Self::Testnet,
            Self::Mainnet => Self::Signet,
        }
    }
}

/// Main application state
#[derive(Clone, Default)]
pub enum AppState {
    /// Home screen with wallet list
    #[default]
    Home,

    /// Chain/network selection popup
    ChainSelect,

    /// Keygen wizard
    Keygen(KeygenState),

    /// Reshare wizard
    Reshare(ReshareState),

    /// Send wizard
    Send(SendState),
}

/// Keygen wizard state
#[derive(Clone, Default)]
pub enum KeygenState {
    /// Initial setup form
    #[default]
    Round1Setup,
    /// Display round 1 output
    Round1Output { output_json: String },
    /// Input round 2 data
    Round2Input,
    /// Display round 2 output
    Round2Output { output_json: String },
    /// Input finalize data
    FinalizeInput,
    /// Complete
    Complete { wallet_name: String },
}

/// Reshare wizard state
#[derive(Clone, Default)]
pub enum ReshareState {
    /// Select source wallet and configure
    #[default]
    Round1Setup,
    /// Display round 1 output
    Round1Output { output_json: String },
    /// Input for finalize (as new party)
    FinalizeInput,
    /// Complete
    Complete { wallet_name: String },
}

/// Send wizard state
#[derive(Clone, Default)]
pub enum SendState {
    /// Select wallet to send from
    #[default]
    SelectWallet,
    /// Enter recipient and amount
    EnterDetails {
        wallet_name: String,
    },
    /// Show sighash for signing
    ShowSighash {
        wallet_name: String,
        to_address: String,
        amount: u64,
        sighash: String,
        session_id: String,
    },
    /// Generate nonce
    GenerateNonce {
        wallet_name: String,
        session_id: String,
        sighash: String,
        nonce_output: String,
    },
    /// Enter other nonces
    EnterNonces {
        wallet_name: String,
        session_id: String,
        sighash: String,
        my_nonce: String,
    },
    /// Generate signature share
    GenerateShare {
        wallet_name: String,
        session_id: String,
        sighash: String,
        share_output: String,
    },
    /// Combine shares (aggregator)
    CombineShares {
        wallet_name: String,
        session_id: String,
        sighash: String,
    },
    /// Transaction complete
    Complete {
        txid: String,
    },
}

/// Form field focus for multi-field forms
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum KeygenFormField {
    #[default]
    Name,
    Threshold,
    NParties,
    MyIndex,
    MyRank,
    Hierarchical,
}

impl KeygenFormField {
    pub fn next(&self) -> Self {
        match self {
            Self::Name => Self::Threshold,
            Self::Threshold => Self::NParties,
            Self::NParties => Self::MyIndex,
            Self::MyIndex => Self::MyRank,
            Self::MyRank => Self::Hierarchical,
            Self::Hierarchical => Self::Name,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Name => Self::Hierarchical,
            Self::Threshold => Self::Name,
            Self::NParties => Self::Threshold,
            Self::MyIndex => Self::NParties,
            Self::MyRank => Self::MyIndex,
            Self::Hierarchical => Self::MyRank,
        }
    }
}

/// Reshare form field focus
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ReshareFormField {
    #[default]
    SourceWallet,
    NewThreshold,
    NewNParties,
}

impl ReshareFormField {
    pub fn next(&self) -> Self {
        match self {
            Self::SourceWallet => Self::NewThreshold,
            Self::NewThreshold => Self::NewNParties,
            Self::NewNParties => Self::SourceWallet,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::SourceWallet => Self::NewNParties,
            Self::NewThreshold => Self::SourceWallet,
            Self::NewNParties => Self::NewThreshold,
        }
    }
}

/// Reshare finalize form field focus
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ReshareFinalizeField {
    #[default]
    TargetName,
    MyIndex,
    MyRank,
    Hierarchical,
    DataInput,
}

impl ReshareFinalizeField {
    pub fn next(&self) -> Self {
        match self {
            Self::TargetName => Self::MyIndex,
            Self::MyIndex => Self::MyRank,
            Self::MyRank => Self::Hierarchical,
            Self::Hierarchical => Self::DataInput,
            Self::DataInput => Self::TargetName,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::TargetName => Self::DataInput,
            Self::MyIndex => Self::TargetName,
            Self::MyRank => Self::MyIndex,
            Self::Hierarchical => Self::MyRank,
            Self::DataInput => Self::Hierarchical,
        }
    }
}

/// Send form field focus
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum SendFormField {
    #[default]
    ToAddress,
    Amount,
}

impl SendFormField {
    pub fn next(&self) -> Self {
        match self {
            Self::ToAddress => Self::Amount,
            Self::Amount => Self::ToAddress,
        }
    }

    pub fn prev(&self) -> Self {
        self.next()
    }
}
