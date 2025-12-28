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
}

/// Main application state
#[derive(Clone, Default)]
pub enum AppState {
    /// Home screen with wallet list
    #[default]
    Home,

    /// Wallet details with action menu
    WalletDetails(WalletDetailsState),

    /// Chain/network selection popup
    ChainSelect,

    /// Keygen wizard
    Keygen(KeygenState),

    /// Reshare wizard
    Reshare(ReshareState),

    /// Send wizard
    Send(SendState),

    /// HD Address list
    AddressList(AddressListState),

    /// Mnemonic backup screen
    MnemonicBackup(MnemonicState),
}

/// Available wallet actions
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WalletAction {
    Send,
    ViewAddresses,
    BackupMnemonic,
    Reshare,
    DeleteWallet,
}

impl WalletAction {
    pub fn all() -> &'static [WalletAction] {
        &[
            WalletAction::Send,
            WalletAction::ViewAddresses,
            WalletAction::BackupMnemonic,
            WalletAction::Reshare,
            WalletAction::DeleteWallet,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            WalletAction::Send => "Send Transaction",
            WalletAction::ViewAddresses => "View HD Addresses",
            WalletAction::BackupMnemonic => "Backup Mnemonic",
            WalletAction::Reshare => "Reshare Keys",
            WalletAction::DeleteWallet => "⚠ Delete Wallet",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            WalletAction::Send => "Sign and broadcast a Bitcoin transaction",
            WalletAction::ViewAddresses => "View derived HD addresses",
            WalletAction::BackupMnemonic => "Backup your secret share as 24 words",
            WalletAction::Reshare => "Proactively refresh secret shares",
            WalletAction::DeleteWallet => "Permanently delete this wallet (cannot undo!)",
        }
    }
}

/// Wallet details state
#[derive(Clone, Default)]
pub struct WalletDetailsState {
    /// Wallet name
    pub wallet_name: String,
    /// Selected action index
    pub selected_action: usize,
    /// Confirm delete mode
    pub confirm_delete: bool,
    /// Show QR code popup
    pub show_qr: bool,
}

/// HD Address list state
#[derive(Clone, Default)]
pub struct AddressListState {
    /// Wallet name
    pub wallet_name: String,
    /// Addresses loaded (address, pubkey_hex, index)
    pub addresses: Vec<(String, String, u32)>,
    /// Currently selected index
    pub selected: usize,
    /// Error message if any
    pub error: Option<String>,
    /// Is HD enabled for this wallet
    pub hd_enabled: bool,
    /// Balance cache for addresses (index -> (balance_sats, utxo_count))
    pub balance_cache: std::collections::HashMap<u32, (u64, usize)>,
}

/// Mnemonic backup state
#[derive(Clone, Default)]
pub struct MnemonicState {
    /// Wallet name
    pub wallet_name: String,
    /// Available party indices (e.g., [1, 2, 3])
    pub available_parties: Vec<u32>,
    /// Selected party index
    pub selected_party: usize,
    /// Generated mnemonic words (24)
    pub words: Vec<String>,
    /// Error message if any
    pub error: Option<String>,
    /// Whether party selection is done
    pub party_selected: bool,
    /// Whether to show the mnemonic (security confirmation)
    pub revealed: bool,
}

/// Keygen wizard state
#[derive(Clone, Default)]
pub enum KeygenState {
    /// Choose TSS or HTSS mode
    #[default]
    ModeSelect,
    /// Setup params based on mode
    ParamsSetup,
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
    /// Select which parties will sign
    SelectSigners { wallet_name: String },
    /// Select HD address to send from (optional)
    SelectAddress { wallet_name: String },
    /// Configure script options (timelock, recovery, HTLC)
    ConfigureScript { wallet_name: String },
    /// Enter recipient and amount
    EnterDetails { wallet_name: String },
    /// Show sighash for signing
    ShowSighash {
        wallet_name: String,
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
    },
    /// Generate signature share
    GenerateShare {
        wallet_name: String,
        share_output: String,
    },
    /// Combine shares (aggregator)
    CombineShares { wallet_name: String },
    /// Transaction complete
    Complete { txid: String },
}

/// Form field focus for multi-field forms
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum KeygenFormField {
    #[default]
    Name,
    Threshold,
    NParties,
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
