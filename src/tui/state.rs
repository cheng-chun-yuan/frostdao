//! TUI state definitions
//!
//! Defines all states for the TUI state machine.

use bitcoin::Network;

/// Network selection for the TUI
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum NetworkSelection {
    #[default]
    Testnet4,
    Testnet3,
    Signet,
    Regtest,
    Mainnet,
}

impl NetworkSelection {
    pub fn to_bitcoin_network(self) -> Network {
        match self {
            Self::Testnet4 => Network::Testnet4,
            Self::Testnet3 => Network::Testnet,
            Self::Signet => Network::Signet,
            Self::Regtest => Network::Regtest,
            Self::Mainnet => Network::Bitcoin,
        }
    }

    pub fn mempool_api_base(&self) -> anyhow::Result<String> {
        frostdao::btc::transaction::mempool_api_base_url(self.to_bitcoin_network())
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Testnet4 => "Testnet4",
            Self::Testnet3 => "Testnet3",
            Self::Signet => "Signet",
            Self::Regtest => "Regtest",
            Self::Mainnet => "Mainnet",
        }
    }

    pub fn policy_hint(&self) -> &'static str {
        match self {
            Self::Testnet4 => "testnet4 remote UTXOs via mempool.space",
            Self::Testnet3 => "testnet3 remote UTXOs via mempool.space",
            Self::Signet => "signet remote UTXOs via mempool.space",
            Self::Regtest => {
                "regtest uses local Esplora/mempool API from FROSTDAO_REGTEST_MEMPOOL_API"
            }
            Self::Mainnet => "MAINNET real funds; guarded commands require explicit opt-in",
        }
    }

    pub fn address_scope_hint(&self) -> &'static str {
        match self {
            Self::Mainnet => "Bitcoin mainnet root address",
            Self::Testnet4 | Self::Testnet3 | Self::Signet | Self::Regtest => {
                "test-chain root address for testnet/signet/regtest"
            }
        }
    }

    pub fn all() -> &'static [NetworkSelection] {
        &[
            Self::Testnet4,
            Self::Testnet3,
            Self::Signet,
            Self::Regtest,
            Self::Mainnet,
        ]
    }

    /// Convert a network selection to its index in [`NetworkSelection::all`].
    pub fn index(self) -> usize {
        Self::all()
            .iter()
            .position(|network| *network == self)
            .unwrap_or(0)
    }

    /// Convert an index to a network selection, defaulting to the configured default.
    pub fn from_index(index: usize) -> Self {
        Self::all().get(index).copied().unwrap_or_default()
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

    /// Nostr room configuration
    NostrRoom,

    /// Nostr DKG keygen
    NostrKeygen,

    /// Nostr signing
    NostrSign,

    /// Miniscript-backed agent payment draft
    #[cfg(feature = "miniscript-policy")]
    PolicyPreview,
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
    /// Wallet name typed by the operator to confirm deletion
    pub delete_confirmation_input: String,
    /// Show QR code popup
    pub show_qr: bool,
}

/// HD Address list state
#[derive(Clone, Default)]
pub struct AddressListState {
    /// Wallet name
    pub wallet_name: String,
    /// Network used for address/path display
    pub network: NetworkSelection,
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
    /// Whether this is an HTSS wallet
    pub hierarchical: bool,
    /// Party ranks for HTSS (party_index -> rank)
    pub party_ranks: std::collections::BTreeMap<u32, u32>,
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

/// Reshare mode selection
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ReshareMode {
    /// Local reshare - all shares on same machine
    #[default]
    Local,
    /// Distributed reshare - multi-party protocol
    Distributed,
}

impl ReshareMode {
    pub fn all() -> &'static [ReshareMode] {
        &[ReshareMode::Local, ReshareMode::Distributed]
    }

    pub fn label(&self) -> &'static str {
        match self {
            ReshareMode::Local => "Local Refresh",
            ReshareMode::Distributed => "Distributed Reshare",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ReshareMode::Local => "Refresh all shares locally (single machine)",
            ReshareMode::Distributed => "Multi-party reshare protocol (copy/paste)",
        }
    }
}

/// Reshare wizard state
#[derive(Clone, Default)]
pub enum ReshareState {
    /// Select reshare mode (Local vs Distributed)
    #[default]
    ModeSelect,
    /// Local reshare setup
    LocalSetup,
    /// Local reshare complete
    LocalComplete { wallet_name: String },
    /// Distributed: Select source wallet and configure
    Round1Setup,
    /// Distributed: Display round 1 output
    Round1Output { output_json: String },
    /// Distributed: Input for finalize (as new party)
    FinalizeInput,
    /// Distributed: Complete
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
    /// Review transaction details before local signing/broadcast
    ReviewTransaction { wallet_name: String },
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
    Complete {
        txid: String,
        broadcast_status: Option<String>,
        raw_tx: Option<String>,
    },
}

/// Form field focus for multi-field forms
#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)] // MyRank used in distributed Nostr keygen
pub enum KeygenFormField {
    #[default]
    Name,
    Threshold,
    NParties,
    MyRank,
    RankDistribution, // HTSS: e.g., "2,3,3" = 2 at rank 0, 3 at rank 1, 3 at rank 2
    SigningRequirement, // HTSS: e.g., "1,2,2" = need 1 rank-0, 2 rank-1, 2 rank-2
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

/// Local reshare form field focus
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ReshareLocalField {
    #[default]
    SourceWallet,
    TargetName,
    NewThreshold,
    NewNParties,
}

impl ReshareLocalField {
    pub fn next(&self) -> Self {
        match self {
            Self::SourceWallet => Self::TargetName,
            Self::TargetName => Self::NewThreshold,
            Self::NewThreshold => Self::NewNParties,
            Self::NewNParties => Self::SourceWallet,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::SourceWallet => Self::NewNParties,
            Self::TargetName => Self::SourceWallet,
            Self::NewThreshold => Self::TargetName,
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

// ============================================================================
// Nostr States
// ============================================================================

/// Nostr room phase
#[derive(Clone, Default, PartialEq, Eq)]
pub enum NostrRoomPhase {
    /// Configuring room parameters
    #[default]
    Configure,
    /// Waiting for participants to join
    WaitingForParticipants,
    /// Ready to start (enough participants)
    Ready,
}

/// Nostr room form fields
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum NostrRoomField {
    #[default]
    RoomId,
    MyIndex,
    Threshold,
    NParties,
}

/// Nostr DKG keygen state
#[derive(Clone, Default)]
pub enum NostrKeygenState {
    /// Waiting to start / select mode
    #[default]
    ModeSelect,
    /// Waiting for parties to join
    WaitingForParties {
        received_round1: std::collections::HashMap<u32, String>,
    },
    /// Round 1 complete, processing Round 2
    Round2 {
        received_round2: std::collections::HashMap<u32, Vec<String>>,
    },
    /// Finalizing
    Finalizing,
}

/// Nostr signing state - Propose/Consent/Execute flow
#[derive(Clone, Default)]
pub enum NostrSignState {
    /// Select wallet and role (Propose or Consent)
    #[default]
    SelectWallet,
    /// Choose role: Propose new tx or Consent to existing
    SelectRole { wallet_name: String },

    // === Proposer Flow ===
    /// Configure transaction details
    ConfigureTx { wallet_name: String },
    /// Transaction proposed, waiting for consents
    WaitingForConsent {
        wallet_name: String,
        session_id: String,
        proposal: TxProposal,
        consents: std::collections::HashMap<u32, String>, // party -> reviewed sighash fingerprint
        rejections: std::collections::HashMap<u32, String>, // party -> rejection reason
    },

    // === Consenter Flow ===
    /// View pending proposals for this wallet
    ViewProposals { wallet_name: String },
    /// Review a specific proposal
    ReviewProposal {
        wallet_name: String,
        proposal: TxProposal,
    },
    /// Consent given, waiting for execution
    WaitingForExecution {
        wallet_name: String,
        session_id: String,
    },

    // === Shared Final States ===
    /// Collecting signature shares
    CollectingShares {
        wallet_name: String,
        session_id: String,
        received_shares: std::collections::HashMap<u32, String>,
    },
    /// Combining and waiting for real broadcast confirmation
    Combining {
        wallet_name: String,
        session_id: String,
    },
    /// Transaction broadcast
    Complete { txid: String },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NostrTxField {
    #[default]
    Recipient,
    Amount,
}

impl NostrTxField {
    pub fn next(self) -> Self {
        match self {
            Self::Recipient => Self::Amount,
            Self::Amount => Self::Recipient,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Recipient => Self::Amount,
            Self::Amount => Self::Recipient,
        }
    }
}

/// Transaction proposal for Nostr signing
#[derive(Clone, Default, Debug)]
pub struct TxProposal {
    /// Unique session ID
    pub session_id: String,
    /// Wallet this proposal belongs to
    pub wallet_name: String,
    /// Proposer's party index
    pub proposer_index: u32,
    /// Recipient address
    pub to_address: String,
    /// Amount in satoshis
    pub amount_sats: u64,
    /// Fee rate (sat/vB)
    pub fee_rate: u64,
    /// Transaction sighash to sign
    pub sighash: String,
    /// Unsigned transaction hex for later combine/broadcast handoff
    pub unsigned_tx: String,
    /// Human-checkable transaction review data
    pub review: frostdao::nostr::TxReviewPayload,
    /// Human-readable description
    pub description: String,
    /// Timestamp when proposed
    pub timestamp: u64,
}

#[cfg(test)]
mod tests {
    use super::NetworkSelection;

    #[test]
    fn network_selection_index_roundtrip() {
        let networks = NetworkSelection::all();

        assert_eq!(networks.len(), 5);

        for (index, network) in networks.iter().copied().enumerate() {
            assert_eq!(NetworkSelection::from_index(index), network);
            assert_eq!(network.index(), index);
        }
    }

    #[test]
    fn network_selection_from_index_out_of_bounds_defaults_to_testnet4() {
        assert_eq!(
            NetworkSelection::from_index(usize::MAX),
            NetworkSelection::Testnet4
        );
    }
}
