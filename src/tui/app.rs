//! TUI application state and logic

use anyhow::Result;
use bitcoin::{Address, XOnlyPublicKey};
use ratatui::widgets::ListState;
use std::collections::HashMap;

use crate::keygen::{list_wallets, WalletSummary};
use crate::storage::{FileStorage, Storage};
use crate::tui::state::{AppState, NetworkSelection};

/// Balance information for a wallet
#[derive(Clone)]
pub struct BalanceInfo {
    pub balance_sats: u64,
    pub utxo_count: usize,
    pub address: String,
}

/// Main application state
pub struct App {
    /// Current application state
    pub state: AppState,

    /// List of wallets
    pub wallets: Vec<WalletSummary>,

    /// Wallet list selection state
    pub wallet_list_state: ListState,

    /// Balance cache (key: "wallet_name:network")
    pub balance_cache: HashMap<String, BalanceInfo>,

    /// Currently selected network
    pub network: NetworkSelection,

    /// Status message
    pub message: Option<String>,

    /// Loading state
    pub loading: bool,

    /// Chain selector index (for popup)
    pub chain_selector_index: usize,
}

impl App {
    /// Create a new App instance
    pub fn new() -> Result<Self> {
        let wallets = list_wallets()?;
        let mut wallet_list_state = ListState::default();
        if !wallets.is_empty() {
            wallet_list_state.select(Some(0));
        }

        Ok(Self {
            state: AppState::Home,
            wallets,
            wallet_list_state,
            balance_cache: HashMap::new(),
            network: NetworkSelection::default(),
            message: None,
            loading: false,
            chain_selector_index: 0,
        })
    }

    /// Get selected wallet
    pub fn selected_wallet(&self) -> Option<&WalletSummary> {
        self.wallet_list_state
            .selected()
            .and_then(|i| self.wallets.get(i))
    }

    /// Navigate to next wallet
    pub fn next_wallet(&mut self) {
        if self.wallets.is_empty() {
            return;
        }
        let i = match self.wallet_list_state.selected() {
            Some(i) => {
                if i >= self.wallets.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.wallet_list_state.select(Some(i));
    }

    /// Navigate to previous wallet
    pub fn prev_wallet(&mut self) {
        if self.wallets.is_empty() {
            return;
        }
        let i = match self.wallet_list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.wallets.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.wallet_list_state.select(Some(i));
    }

    /// Refresh balance for selected wallet
    pub fn refresh_balance(&mut self) {
        if let Some(wallet) = self.selected_wallet().cloned() {
            self.loading = true;
            self.message = Some(format!(
                "Fetching {} balance for {}...",
                self.network.display_name(),
                wallet.name
            ));

            match self.fetch_balance(&wallet.name) {
                Ok(info) => {
                    let cache_key = format!("{}:{:?}", wallet.name, self.network);
                    self.balance_cache.insert(cache_key, info);
                    self.message = Some(format!("Balance updated for {}", wallet.name));
                }
                Err(e) => {
                    self.message = Some(format!("Error: {}", e));
                }
            }
            self.loading = false;
        }
    }

    /// Fetch balance for a wallet on the current network
    fn fetch_balance(&self, wallet_name: &str) -> Result<BalanceInfo> {
        let state_dir = crate::keygen::get_state_dir(wallet_name);
        let storage = FileStorage::new(&state_dir)?;

        // Load shared key
        let shared_key_bytes = storage.read("shared_key.bin")?;
        let shared_key: schnorr_fun::frost::SharedKey<schnorr_fun::fun::marker::EvenY> =
            bincode::deserialize(&shared_key_bytes)?;

        let pubkey_bytes: [u8; 32] = shared_key.public_key().to_xonly_bytes();
        let xonly_pubkey = XOnlyPublicKey::from_slice(&pubkey_bytes)?;

        let secp = bitcoin::secp256k1::Secp256k1::new();
        let btc_network = self.network.to_bitcoin_network();
        let address = Address::p2tr(&secp, xonly_pubkey, None, btc_network).to_string();

        // Fetch UTXOs from mempool.space
        let client = reqwest::blocking::Client::new();
        let api_base = self.network.mempool_api_base();
        let url = format!("{}/address/{}/utxo", api_base, address);
        let response = client.get(&url).send()?;
        let utxos: Vec<serde_json::Value> = response.json()?;

        let balance_sats: u64 = utxos
            .iter()
            .filter_map(|u| u.get("value").and_then(|v| v.as_u64()))
            .sum();

        Ok(BalanceInfo {
            balance_sats,
            utxo_count: utxos.len(),
            address,
        })
    }

    /// Reload wallet list
    pub fn reload_wallets(&mut self) {
        if let Ok(wallets) = list_wallets() {
            self.wallets = wallets;
            if self.wallets.is_empty() {
                self.wallet_list_state.select(None);
            } else if self
                .wallet_list_state
                .selected()
                .map(|i| i >= self.wallets.len())
                .unwrap_or(true)
            {
                self.wallet_list_state.select(Some(0));
            }
            self.message = Some("Wallet list refreshed".to_string());
        }
    }

    /// Toggle to next network in chain selector
    pub fn next_network(&mut self) {
        self.chain_selector_index = (self.chain_selector_index + 1) % 3;
    }

    /// Toggle to previous network in chain selector
    pub fn prev_network(&mut self) {
        self.chain_selector_index = if self.chain_selector_index == 0 {
            2
        } else {
            self.chain_selector_index - 1
        };
    }

    /// Confirm network selection
    pub fn confirm_network(&mut self) {
        self.network = match self.chain_selector_index {
            0 => NetworkSelection::Testnet,
            1 => NetworkSelection::Signet,
            2 => NetworkSelection::Mainnet,
            _ => NetworkSelection::Testnet,
        };
        self.state = AppState::Home;
        self.message = Some(format!("Switched to {}", self.network.display_name()));
    }

    /// Set status message
    pub fn set_message(&mut self, msg: &str) {
        self.message = Some(msg.to_string());
    }

    /// Clear status message
    pub fn clear_message(&mut self) {
        self.message = None;
    }
}
