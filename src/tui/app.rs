//! TUI application state and logic

use anyhow::Result;
use bitcoin::{Address, XOnlyPublicKey};
use ratatui::widgets::ListState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use crate::tui::components::TextInput;
#[cfg(feature = "miniscript-policy")]
use crate::tui::screens::PolicyPreviewFormData;
use crate::tui::screens::{KeygenFormData, ReshareFormData, SendFormData};
use crate::tui::state::{
    AppState, NetworkSelection, NostrKeygenState, NostrRoomField, NostrRoomPhase, NostrSignState,
    NostrTxField, TxProposal,
};
use crate::tui::REFRESH_KEY_LABEL;
use frostdao::nostr::RoomMessageTransport;
use frostdao::protocol::keygen::{get_state_dir, list_wallets, WalletSummary};
use frostdao::protocol::{
    SigningCoordinator, SigningNonceInput, SigningSchemePolicy, SigningShareInput,
};
use frostdao::storage::{FileStorage, Storage};

const TUI_NOSTR_RELAYS_ENV: &str = "FROSTDAO_TUI_NOSTR_RELAYS";
const TUI_MAINNET_NOSTR_ENV: &str = "FROSTDAO_ENABLE_MAINNET_NOSTR";

pub(crate) fn wallet_address_for_network(
    wallet: &WalletSummary,
    network: NetworkSelection,
) -> Option<&str> {
    match network {
        NetworkSelection::Mainnet => wallet.address_mainnet.as_deref(),
        NetworkSelection::Regtest => wallet.address_regtest.as_deref(),
        NetworkSelection::Testnet4 | NetworkSelection::Testnet3 | NetworkSelection::Signet => {
            wallet
                .address_testnet
                .as_deref()
                .or(wallet.address.as_deref())
        }
    }
}

pub(crate) fn missing_network_address_message(network: NetworkSelection) -> String {
    format!(
        "missing {} source address; select a wallet/network with an address before send, copy, or QR",
        network.display_name()
    )
}

pub(crate) fn balance_cache_key(wallet_name: &str, network: NetworkSelection) -> String {
    format!("{}:{:?}", wallet_name, network)
}

fn wallet_balance_updated_message(wallet_name: &str, network: NetworkSelection) -> String {
    format!(
        "{} balance updated for {}",
        network.display_name(),
        wallet_name
    )
}

fn copied_preview_message(text: &str) -> String {
    // Use char_indices for safe UTF-8 slicing.
    let preview = if text.chars().count() > 20 {
        let end_byte = text
            .char_indices()
            .nth(20)
            .map(|(i, _)| i)
            .unwrap_or(text.len());
        format!("{}...", &text[..end_byte])
    } else {
        text.to_string()
    };
    format!("Copied: {}", preview)
}

pub enum TuiNostrRuntime {
    LocalSimulation(frostdao::nostr::NostrRoomRuntime<frostdao::nostr::InMemoryRoomTransport>),
    Relay(frostdao::nostr::NostrRoomRuntime<frostdao::nostr::RelayRoomTransport>),
}

impl TuiNostrRuntime {
    fn publish(&mut self, message: frostdao::nostr::NostrProtocolMessage) -> Result<()> {
        match self {
            Self::LocalSimulation(runtime) => runtime.publish(message),
            Self::Relay(runtime) => runtime.publish(message),
        }
    }

    fn receive(&mut self, now: u64) -> Result<Vec<frostdao::nostr::NostrProtocolMessage>> {
        match self {
            Self::LocalSimulation(runtime) => runtime.receive(now),
            Self::Relay(runtime) => runtime.receive(now),
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::LocalSimulation(_) => "local simulation",
            Self::Relay(_) => "relay",
        }
    }

    fn is_local_simulation(&self) -> bool {
        matches!(self, Self::LocalSimulation(_))
    }

    fn room_join_pubkey(&self, party_index: u32) -> String {
        match self {
            Self::LocalSimulation(_) => format!("tui-party-{}", party_index),
            Self::Relay(runtime) => runtime.transport().client().my_pubkey(),
        }
    }

    fn conversation_key_with(&self, peer_pubkey: &str) -> Result<Option<[u8; 32]>> {
        match self {
            Self::LocalSimulation(_) => Ok(None),
            Self::Relay(runtime) => Ok(Some(
                runtime
                    .transport()
                    .client()
                    .conversation_key_with(peer_pubkey)?,
            )),
        }
    }

    #[cfg(test)]
    fn local_simulation_room_len(&self, room: &str) -> Option<usize> {
        match self {
            Self::LocalSimulation(runtime) => Some(runtime.transport().room_len(room)),
            Self::Relay(_) => None,
        }
    }

    fn publish_local_simulation_message(
        &mut self,
        message: frostdao::nostr::NostrProtocolMessage,
    ) -> Result<()> {
        match self {
            Self::LocalSimulation(runtime) => runtime.transport_mut().publish(message),
            Self::Relay(_) => {
                anyhow::bail!("local participant simulation is unavailable in relay mode")
            }
        }
    }
}

/// Balance information for a wallet
#[derive(Clone)]
pub struct BalanceInfo {
    pub balance_sats: u64,
    pub utxo_count: usize,
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

    /// Show global help overlay (F1).
    pub show_global_help: bool,

    /// Loading state
    pub loading: bool,

    /// Chain selector index (for popup)
    pub chain_selector_index: usize,

    /// Keygen wizard form data
    pub keygen_form: KeygenFormData,

    /// Reshare wizard form data
    pub reshare_form: ReshareFormData,

    /// Send wizard form data
    pub send_form: SendFormData,

    /// Miniscript-backed agent payment draft form
    #[cfg(feature = "miniscript-policy")]
    pub policy_preview_form: PolicyPreviewFormData,

    // Nostr room configuration
    /// Current Nostr room ID
    pub nostr_room_id: String,
    /// My participant index (1-based)
    pub nostr_my_index: u32,
    /// Signing threshold
    pub nostr_threshold: u32,
    /// Total number of parties
    pub nostr_n_parties: u32,
    /// Whether connected to relay
    pub nostr_connected: bool,
    /// Current focused field in room config
    pub nostr_room_focus: NostrRoomField,
    /// Current room phase
    pub nostr_room_phase: NostrRoomPhase,
    /// Participants who have joined (party_index -> pubkey/name)
    pub nostr_participants: HashMap<u32, String>,
    /// Pending transaction proposals received through the room runtime
    pub nostr_pending_proposals: HashMap<String, TxProposal>,
    /// Encrypted signing nonces received through the room runtime, keyed by session and party
    pub nostr_received_nonces: HashMap<String, HashMap<u32, String>>,
    /// Encrypted signing shares received through the room runtime, keyed by session and party
    pub nostr_received_shares: HashMap<String, HashMap<u32, String>>,
    /// Transaction broadcasts received through the room runtime, keyed by session
    pub nostr_broadcasts: HashMap<String, frostdao::nostr::TxBroadcastEvent>,
    /// Protocol-level signing coordinators, keyed by session
    pub nostr_signing_coordinators: HashMap<String, SigningCoordinator>,
    /// Hardened room runtime for TUI relay flows
    pub nostr_runtime: Option<TuiNostrRuntime>,

    // Nostr DKG/signing state
    /// Current keygen state
    pub nostr_keygen_state: NostrKeygenState,
    /// Current signing state
    pub nostr_sign_state: NostrSignState,

    /// Test-only transport override so TUI tests do not mutate process env.
    #[cfg(test)]
    pub force_relay_transport_for_tests: bool,

    // Nostr signing transaction data
    /// Current focused Nostr transaction draft field
    pub nostr_tx_focus: NostrTxField,
    /// Editable recipient address for transaction proposals
    pub nostr_to_address_input: TextInput,
    /// Editable amount in satoshis for transaction proposals
    pub nostr_amount_input: TextInput,
    /// Recipient address for transaction
    pub nostr_to_address: String,
    /// Amount in satoshis
    pub nostr_amount_sats: u64,

    #[cfg(test)]
    pub audit_events: Vec<frostdao::audit::AuditEvent>,
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
            show_global_help: false,
            loading: false,
            chain_selector_index: 0,
            keygen_form: KeygenFormData::new(),
            reshare_form: ReshareFormData::new(),
            send_form: SendFormData::new(),
            #[cfg(feature = "miniscript-policy")]
            policy_preview_form: PolicyPreviewFormData::new(),
            // Nostr defaults
            nostr_room_id: String::new(),
            nostr_my_index: 1,
            nostr_threshold: 2,
            nostr_n_parties: 3,
            nostr_connected: false,
            nostr_room_focus: NostrRoomField::RoomId,
            nostr_room_phase: NostrRoomPhase::Configure,
            nostr_participants: HashMap::new(),
            nostr_pending_proposals: HashMap::new(),
            nostr_received_nonces: HashMap::new(),
            nostr_received_shares: HashMap::new(),
            nostr_broadcasts: HashMap::new(),
            nostr_signing_coordinators: HashMap::new(),
            nostr_runtime: None,
            nostr_keygen_state: NostrKeygenState::ModeSelect,
            nostr_sign_state: NostrSignState::SelectWallet,
            #[cfg(test)]
            force_relay_transport_for_tests: false,
            nostr_tx_focus: NostrTxField::Recipient,
            nostr_to_address_input: TextInput::new("Recipient Address").with_placeholder("tb1p..."),
            nostr_amount_input: TextInput::new("Amount (sats)")
                .with_placeholder("50000")
                .numeric(),
            nostr_to_address: String::new(),
            nostr_amount_sats: 0,
            #[cfg(test)]
            audit_events: Vec::new(),
        })
    }

    /// Get selected wallet
    pub fn selected_wallet(&self) -> Option<&WalletSummary> {
        self.wallet_list_state
            .selected()
            .and_then(|i| self.wallets.get(i))
    }

    /// Validate the active Nostr room form before joining a ceremony.
    pub fn nostr_room_config_error(&self) -> Option<&'static str> {
        if self.nostr_room_id.trim().is_empty() {
            return Some("Enter a room ID first");
        }
        if self.nostr_n_parties < 2 {
            return Some("Parties must be at least 2");
        }
        if self.nostr_my_index == 0 || self.nostr_my_index > self.nostr_n_parties {
            return Some("My Index must be between 1 and Parties");
        }
        if self.nostr_threshold == 0 || self.nostr_threshold > self.nostr_n_parties {
            return Some("Threshold must be between 1 and Parties");
        }
        None
    }

    /// Build a locally proposed TUI Nostr transaction after validating the draft fields.
    pub fn build_nostr_tx_proposal(&self, wallet_name: &str, timestamp: u64) -> Result<TxProposal> {
        let amount_sats = self.nostr_amount_sats()?;
        let to_address = parse_tui_recipient_address(
            self.nostr_to_address_value().trim(),
            self.network.to_bitcoin_network(),
            self.network.display_name(),
        )?;
        if amount_sats == 0 {
            anyhow::bail!("amount must be greater than zero");
        }
        self.ensure_nostr_proposal_network_available()?;

        let wallet = self
            .wallets
            .iter()
            .find(|wallet| wallet.name == wallet_name)
            .ok_or_else(|| anyhow::anyhow!("wallet '{wallet_name}' is not loaded"))?;
        wallet_address_for_network(wallet, self.network)
            .map(str::trim)
            .filter(|address| !address.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "wallet '{wallet_name}' has no known {} source address",
                    self.network.display_name()
                )
            })?;

        let state_dir = get_state_dir(wallet_name);
        let storage = FileStorage::new(&state_dir)?;
        let derivation_path = self.nostr_source_derivation_path();
        let build = frostdao::protocol::dkg_tx::build_unsigned_tx_core_with_source_path(
            wallet_name,
            &to_address,
            amount_sats,
            Some(10),
            self.network.to_bitcoin_network(),
            &storage,
            derivation_path,
        )?;
        let build_output: frostdao::protocol::dkg_tx::BuildTxOutput =
            serde_json::from_str(&build.result)?;

        Ok(self.nostr_tx_proposal_from_build_output(wallet_name, timestamp, build_output))
    }

    pub(crate) fn ensure_nostr_proposal_network_available(&self) -> Result<()> {
        self.network.mempool_api_base().map(|_| ()).map_err(|_| {
            anyhow::anyhow!(
                "Nostr transaction proposals need a UTXO API on {}; {}. For regtest, set {} to your local Esplora/mempool API.",
                self.network.display_name(),
                self.network.policy_hint(),
                frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV
            )
        })
    }

    pub(crate) fn nostr_to_address_value(&self) -> &str {
        let input_value = self.nostr_to_address_input.value().trim();
        if input_value.is_empty() {
            self.nostr_to_address.as_str()
        } else {
            self.nostr_to_address_input.value()
        }
    }

    pub(crate) fn nostr_amount_sats(&self) -> Result<u64> {
        let input_value = self.nostr_amount_input.value().trim();
        if input_value.is_empty() {
            Ok(self.nostr_amount_sats)
        } else {
            input_value
                .parse::<u64>()
                .map_err(|_| anyhow::anyhow!("amount must be a whole number of sats"))
        }
    }

    pub(crate) fn nostr_source_derivation_path(&self) -> Option<(u32, u32)> {
        self.send_form.get_derivation_path()
    }

    fn nostr_tx_proposal_from_build_output(
        &self,
        wallet_name: &str,
        timestamp: u64,
        build: frostdao::protocol::dkg_tx::BuildTxOutput,
    ) -> TxProposal {
        TxProposal {
            session_id: build.session_id,
            wallet_name: wallet_name.to_string(),
            proposer_index: self.nostr_my_index,
            to_address: build.to_address.clone(),
            amount_sats: build.amount_sats,
            fee_rate: build.review.fee_rate_sats_vb,
            sighash: build.sighash,
            unsigned_tx: build.unsigned_tx,
            review: frostdao::nostr::TxReviewPayload {
                network: self.network.display_name().to_string(),
                source_path: build.review.source_path,
                from_address: build.from_address,
                to_address: build.to_address,
                amount_sats: build.amount_sats,
                fee_rate_sats_vb: build.review.fee_rate_sats_vb,
                sighash_fingerprint: build.review.sighash_fingerprint,
            },
            description: format!("Send {} sats", build.amount_sats),
            timestamp,
        }
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
                    let cache_key = balance_cache_key(&wallet.name, self.network);
                    self.balance_cache.insert(cache_key, info);
                    self.message = Some(wallet_balance_updated_message(&wallet.name, self.network));
                }
                Err(e) => {
                    self.message = Some(format!("Error: {}", e));
                }
            }
            self.loading = false;
        }
    }

    pub(crate) fn utxo_api_base_for_fetch(&self, what: &str) -> Result<String> {
        self.network.mempool_api_base().map_err(|err| {
            anyhow::anyhow!(
                "Cannot fetch {what} on {}: {}",
                self.network.display_name(),
                err
            )
        })
    }

    /// Fetch balance for a wallet on the current network
    fn fetch_balance(&self, wallet_name: &str) -> Result<BalanceInfo> {
        let state_dir = frostdao::protocol::keygen::get_state_dir(wallet_name);
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

        // Fetch UTXOs from mempool.space where the selected network supports it.
        let client = reqwest::blocking::Client::new();
        let api_base = self.utxo_api_base_for_fetch("wallet balance")?;
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
        })
    }

    /// Fetch UTXOs and recent transactions for send form
    pub fn fetch_utxos_for_send(&mut self, address: &str) {
        use super::screens::{TxDisplay, UtxoDisplay};

        let clear_send_refresh_state = |send_form: &mut crate::tui::screens::SendFormData| {
            send_form.utxos.clear();
            send_form.recent_txs.clear();
            send_form.total_balance = 0;
            send_form.estimated_fee = 0;
            send_form.utxos_needed = 0;
            send_form.fee_rate = 1;
        };

        let set_fetch_error = |app: &mut App, message: &str| {
            let message = message.to_string();
            app.set_message(&message);
            app.send_form.error_message = Some(message.clone());
            app.send_form.utxo_fetch_error = Some(message);
            clear_send_refresh_state(&mut app.send_form);
        };

        self.send_form.utxo_fetch_error = None;
        self.send_form.error_message = None;
        self.send_form.fee_rate = 1;
        self.send_form.utxos.clear();
        self.send_form.recent_txs.clear();

        let api_base = match self.utxo_source_unavailable_message() {
            Some(message) => {
                set_fetch_error(self, &message);
                return;
            }
            None => match self.utxo_api_base_for_fetch("UTXOs") {
                Ok(api_base) => api_base,
                Err(err) => {
                    set_fetch_error(self, &err.to_string());
                    return;
                }
            },
        };
        let client = reqwest::blocking::Client::new();

        // Fetch fee estimates
        let fee_url = format!("{}/v1/fees/recommended", api_base);
        if let Ok(response) = client.get(&fee_url).send() {
            if let Ok(fees) = response.json::<serde_json::Value>() {
                // Use half hour fee as default (reasonable balance of speed/cost)
                self.send_form.fee_rate = fees
                    .get("halfHourFee")
                    .and_then(|f| f.as_u64())
                    .unwrap_or(1);
            }
        } else {
            set_fetch_error(
                self,
                &format!(
                    "Cannot fetch fee estimates on {}",
                    self.network.display_name()
                ),
            );
            return;
        }

        // Fetch UTXOs
        let utxo_url = format!("{}/address/{}/utxo", api_base, address);
        let response = match client.get(&utxo_url).send() {
            Ok(response) => response,
            Err(_) => {
                set_fetch_error(
                    self,
                    &format!("Cannot fetch UTXOs on {}", self.network.display_name()),
                );
                return;
            }
        };
        let utxos = match response.json::<Vec<serde_json::Value>>() {
            Ok(utxos) => utxos,
            Err(_) => {
                set_fetch_error(
                    self,
                    &format!(
                        "Cannot parse UTXO data from {} explorer",
                        self.network.display_name()
                    ),
                );
                return;
            }
        };

        self.send_form.utxos = utxos
            .iter()
            .filter_map(|u| {
                Some(UtxoDisplay {
                    txid: u.get("txid")?.as_str()?.to_string(),
                    vout: u.get("vout")?.as_u64()? as u32,
                    value: u.get("value")?.as_u64()?,
                    confirmed: u
                        .get("status")
                        .and_then(|s| s.get("confirmed"))
                        .and_then(|c| c.as_bool())
                        .unwrap_or(false),
                })
            })
            .collect();

        self.send_form.total_balance = self.send_form.utxos.iter().map(|u| u.value).sum();
        // Update fee estimate
        self.send_form.estimate_fee();

        // Fetch recent transactions
        let txs_url = format!("{}/address/{}/txs", api_base, address);
        let response = match client.get(&txs_url).send() {
            Ok(response) => response,
            Err(_) => {
                set_fetch_error(
                    self,
                    &format!(
                        "Cannot fetch recent transactions on {}",
                        self.network.display_name()
                    ),
                );
                return;
            }
        };
        match response.json::<Vec<serde_json::Value>>() {
            Ok(txs) => {
                self.send_form.recent_txs = txs
                    .iter()
                    .take(10)
                    .filter_map(|tx| {
                        let txid = tx.get("txid")?.as_str()?.to_string();
                        let confirmed = tx
                            .get("status")
                            .and_then(|s| s.get("confirmed"))
                            .and_then(|c| c.as_bool())
                            .unwrap_or(false);
                        let time = tx
                            .get("status")
                            .and_then(|s| s.get("block_time"))
                            .and_then(|t| t.as_u64());

                        // Calculate net amount for this address
                        let mut received: i64 = 0;
                        let mut sent: i64 = 0;

                        if let Some(vout) = tx.get("vout").and_then(|v| v.as_array()) {
                            for out in vout {
                                if let Some(scriptpubkey_address) =
                                    out.get("scriptpubkey_address").and_then(|a| a.as_str())
                                {
                                    if scriptpubkey_address == address {
                                        received +=
                                            out.get("value").and_then(|v| v.as_i64()).unwrap_or(0);
                                    }
                                }
                            }
                        }

                        if let Some(vin) = tx.get("vin").and_then(|v| v.as_array()) {
                            for inp in vin {
                                if let Some(prevout) = inp.get("prevout") {
                                    if let Some(scriptpubkey_address) =
                                        prevout.get("scriptpubkey_address").and_then(|a| a.as_str())
                                    {
                                        if scriptpubkey_address == address {
                                            sent += prevout
                                                .get("value")
                                                .and_then(|v| v.as_i64())
                                                .unwrap_or(0);
                                        }
                                    }
                                }
                            }
                        }

                        Some(TxDisplay {
                            txid,
                            amount: received - sent,
                            confirmed,
                            time,
                        })
                    })
                    .collect();
            }
            Err(_) => {
                set_fetch_error(
                    self,
                    &format!(
                        "Cannot parse recent transaction data on {}",
                        self.network.display_name()
                    ),
                );
            }
        }
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

    /// Join the configured Nostr room using the hardened runtime wrapper.
    pub fn join_nostr_room_runtime(&mut self) -> Result<()> {
        let relay_urls = self.nostr_relay_urls_from_env();
        self.join_nostr_room_runtime_with_relays(relay_urls)
    }

    /// Join the configured room with explicit relay URLs. Empty relays use local simulation mode.
    pub fn join_nostr_room_runtime_with_relays(&mut self, relay_urls: Vec<String>) -> Result<()> {
        let cache_path = self.nostr_replay_cache_path();
        let mut runtime = if relay_urls.is_empty() {
            let transport = frostdao::nostr::InMemoryRoomTransport::new();
            TuiNostrRuntime::LocalSimulation(frostdao::nostr::NostrRoomRuntime::load(
                self.nostr_room_id.clone(),
                self.nostr_my_index,
                transport,
                cache_path,
            )?)
        } else {
            if self.network == NetworkSelection::Mainnet && !mainnet_nostr_enabled() {
                anyhow::bail!(
                    "mainnet relay rooms require {}=1; use testnet or signet by default",
                    TUI_MAINNET_NOSTR_ENV
                );
            }
            let transport = frostdao::nostr::RelayRoomTransport::connect(
                &self.nostr_room_id,
                self.nostr_my_index,
                &relay_urls,
            )?;
            TuiNostrRuntime::Relay(frostdao::nostr::NostrRoomRuntime::load(
                self.nostr_room_id.clone(),
                self.nostr_my_index,
                transport,
                cache_path,
            )?)
        };

        let payload = frostdao::nostr::RoomJoinPayload {
            party_index: self.nostr_my_index,
            nostr_pubkey: runtime.room_join_pubkey(self.nostr_my_index),
            threshold: self.nostr_threshold,
            n_parties: self.nostr_n_parties,
            scheme: frostdao::nostr::ThresholdScheme::Tss,
            rank: None,
        };
        let message = frostdao::nostr::NostrProtocolMessage::new(
            self.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::RoomJoin,
            self.nostr_my_index,
            &payload,
        )?
        .with_tss();

        runtime.publish(message)?;
        self.clear_nostr_room_session_state();
        self.nostr_runtime = Some(runtime);
        self.nostr_connected = true;
        self.poll_nostr_room_runtime()?;
        Ok(())
    }

    pub fn nostr_transport_label(&self) -> String {
        if let Some(runtime) = &self.nostr_runtime {
            return runtime.label().to_string();
        }

        let relays = self.nostr_relay_urls_from_env();
        if relays.is_empty() {
            "local simulation".to_string()
        } else {
            format!("relay ({})", relays.join(","))
        }
    }

    pub fn nostr_local_simulation_transport_active(&self) -> bool {
        #[cfg(test)]
        if self.force_relay_transport_for_tests {
            return false;
        }

        self.nostr_runtime
            .as_ref()
            .map(TuiNostrRuntime::is_local_simulation)
            .unwrap_or_else(|| self.nostr_relay_urls_from_env().is_empty())
    }

    fn nostr_relay_urls_from_env(&self) -> Vec<String> {
        nostr_relay_urls_from_env()
    }

    /// Leave the current TUI Nostr room.
    pub fn leave_nostr_room_runtime(&mut self) {
        self.nostr_runtime = None;
        self.nostr_connected = false;
        self.clear_nostr_room_session_state();
    }

    /// Replay-cache path for the current room and party.
    pub fn nostr_replay_cache_path(&self) -> PathBuf {
        let safe_room: String = self
            .nostr_room_id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        PathBuf::from(".frost_state")
            .join("nostr_replay")
            .join(format!("{}-party-{}.json", safe_room, self.nostr_my_index))
    }

    /// Receive validated room messages through the replay cache.
    pub fn poll_nostr_room_runtime(&mut self) -> Result<usize> {
        let now = unix_timestamp_secs();
        let messages = match self.nostr_runtime.as_mut() {
            Some(runtime) => runtime.receive(now)?,
            None => return Ok(0),
        };
        let count = messages.len();

        for message in messages {
            match message.kind {
                frostdao::nostr::NostrMessageKind::RoomJoin => {
                    let payload: frostdao::nostr::RoomJoinPayload = message.payload_as()?;
                    if self.accept_nostr_room_join(&message, &payload) {
                        self.nostr_participants
                            .insert(payload.party_index, payload.nostr_pubkey);
                    }
                }
                frostdao::nostr::NostrMessageKind::TxProposal
                    if message.from != self.nostr_my_index =>
                {
                    let payload: frostdao::nostr::TxProposalEvent = message.payload_as()?;
                    let Some(session_id) = nonempty_message_session(&message) else {
                        continue;
                    };
                    let Some(wallet_name) = nonempty_message_wallet(&message) else {
                        continue;
                    };
                    if !self.accept_nostr_tx_proposal(&message, &payload) {
                        continue;
                    }
                    self.append_nostr_audit_event(
                        frostdao::audit::AuditEvent::new(
                            "nostr_tx_proposal_received",
                            &wallet_name,
                            "accepted",
                        )
                        .with_field("room", self.nostr_room_id.clone())
                        .with_field("transport", self.nostr_transport_label())
                        .with_field("session_id", session_id.clone())
                        .with_field("party_index", message.from)
                        .with_field("network", payload.review.network.clone())
                        .with_field("to_address", payload.to_address.clone())
                        .with_field("amount_sats", payload.amount_sats)
                        .with_field("fee_rate_sats_vb", payload.fee_rate)
                        .with_field(
                            "sighash_fingerprint",
                            payload.review.sighash_fingerprint.clone(),
                        ),
                    );
                    self.nostr_pending_proposals.insert(
                        session_id.clone(),
                        TxProposal {
                            session_id,
                            wallet_name,
                            proposer_index: payload.proposer_index,
                            to_address: payload.to_address,
                            amount_sats: payload.amount_sats,
                            fee_rate: payload.fee_rate,
                            sighash: payload.sighash,
                            unsigned_tx: payload.unsigned_tx,
                            review: payload.review,
                            description: payload.description,
                            timestamp: payload.timestamp,
                        },
                    );
                }
                frostdao::nostr::NostrMessageKind::TxConsent => {
                    let payload: frostdao::nostr::TxConsentEvent = message.payload_as()?;
                    let Some(session_id) = nonempty_message_session(&message) else {
                        continue;
                    };
                    let Some(message_wallet) = nonempty_message_wallet(&message) else {
                        continue;
                    };
                    if session_id != payload.proposal_session {
                        continue;
                    }
                    let mut accepted_consent = None;
                    if let NostrSignState::WaitingForConsent {
                        wallet_name,
                        session_id,
                        proposal,
                        consents,
                        rejections,
                    } = &mut self.nostr_sign_state
                    {
                        if *wallet_name == message_wallet
                            && *session_id == payload.proposal_session
                            && message.from > 0
                            && message.from <= self.nostr_n_parties
                            && message.from != self.nostr_my_index
                            && payload.reviewed_sighash_fingerprint
                                == proposal.review.sighash_fingerprint
                        {
                            if payload.consent {
                                rejections.remove(&message.from);
                                consents.insert(
                                    message.from,
                                    payload.reviewed_sighash_fingerprint.clone(),
                                );
                                accepted_consent = Some((
                                    "accepted",
                                    payload.reviewed_sighash_fingerprint.clone(),
                                ));
                            } else {
                                consents.remove(&message.from);
                                let reason = payload
                                    .reason
                                    .clone()
                                    .filter(|reason| !reason.trim().is_empty())
                                    .unwrap_or_else(|| "Rejected without reason".to_string());
                                rejections.insert(message.from, reason);
                                accepted_consent = Some((
                                    "rejected",
                                    payload.reviewed_sighash_fingerprint.clone(),
                                ));
                            }
                        }
                    }
                    if let Some((status, fingerprint)) = accepted_consent {
                        self.append_nostr_audit_event(
                            frostdao::audit::AuditEvent::new(
                                "nostr_tx_consent_received",
                                &message_wallet,
                                status,
                            )
                            .with_field("room", self.nostr_room_id.clone())
                            .with_field("transport", self.nostr_transport_label())
                            .with_field("session_id", session_id)
                            .with_field("party_index", message.from)
                            .with_field("sighash_fingerprint", fingerprint),
                        );
                    }
                }
                frostdao::nostr::NostrMessageKind::SigningNonceEncrypted => {
                    let payload: frostdao::nostr::SigningNonceEvent = message.payload_as()?;
                    let Some(session_id) = nonempty_message_session(&message) else {
                        continue;
                    };
                    let Some(message_wallet) = nonempty_message_wallet(&message) else {
                        continue;
                    };
                    if !self.accept_nostr_party_ciphertext(
                        &message,
                        payload.party_index,
                        payload.to_index,
                    ) {
                        continue;
                    }
                    let audit_session_id = session_id.clone();
                    let Ok(nonce_input) = self.nostr_signing_nonce_input(
                        &message_wallet,
                        &session_id,
                        &message,
                        &payload,
                    ) else {
                        continue;
                    };
                    if self
                        .accept_nostr_signing_nonce_for_coordinator(
                            &session_id,
                            nonce_input.clone(),
                        )
                        .is_err()
                    {
                        continue;
                    }
                    let party_index = nonce_input.party_index;
                    self.nostr_received_nonces
                        .entry(session_id.clone())
                        .or_default()
                        .insert(party_index, nonce_input.public_nonce);
                    self.append_nostr_audit_event(
                        frostdao::audit::AuditEvent::new(
                            "nostr_signing_nonce_received",
                            &message_wallet,
                            "accepted",
                        )
                        .with_field("room", self.nostr_room_id.clone())
                        .with_field("transport", self.nostr_transport_label())
                        .with_field("session_id", audit_session_id)
                        .with_field("party_index", party_index)
                        .with_field("to_index", payload.to_index),
                    );
                }
                frostdao::nostr::NostrMessageKind::SigningShareEncrypted => {
                    let payload: frostdao::nostr::SigningShareEvent = message.payload_as()?;
                    let Some(session_id) = nonempty_message_session(&message) else {
                        continue;
                    };
                    let Some(message_wallet) = nonempty_message_wallet(&message) else {
                        continue;
                    };
                    if !self.accept_nostr_party_ciphertext(
                        &message,
                        payload.party_index,
                        payload.to_index,
                    ) {
                        continue;
                    }
                    if !self
                        .nostr_received_nonces
                        .get(&session_id)
                        .is_some_and(|nonces| nonces.contains_key(&payload.party_index))
                    {
                        continue;
                    }
                    let Ok(share_input) = self.nostr_signing_share_input(
                        &message_wallet,
                        &session_id,
                        &message,
                        &payload,
                    ) else {
                        continue;
                    };
                    let party_index = share_input.party_index;
                    let signature_share = share_input.signature_share.clone();
                    let Ok(accepted_by_coordinator) =
                        self.accept_nostr_signing_share_for_coordinator(&session_id, share_input)
                    else {
                        continue;
                    };
                    if !accepted_by_coordinator {
                        continue;
                    }
                    self.nostr_received_shares
                        .entry(session_id.clone())
                        .or_default()
                        .insert(party_index, signature_share.clone());
                    self.append_nostr_audit_event(
                        frostdao::audit::AuditEvent::new(
                            "nostr_signing_share_received",
                            &message_wallet,
                            "accepted",
                        )
                        .with_field("room", self.nostr_room_id.clone())
                        .with_field("transport", self.nostr_transport_label())
                        .with_field("session_id", session_id.clone())
                        .with_field("party_index", party_index)
                        .with_field("to_index", payload.to_index),
                    );
                    if let NostrSignState::CollectingShares {
                        session_id: active_session,
                        received_shares,
                        ..
                    } = &mut self.nostr_sign_state
                    {
                        if *active_session == session_id {
                            received_shares.insert(party_index, signature_share);
                        }
                    }
                }
                frostdao::nostr::NostrMessageKind::TxBroadcast => {
                    let payload: frostdao::nostr::TxBroadcastEvent = message.payload_as()?;
                    let Some(session_id) = nonempty_message_session(&message) else {
                        continue;
                    };
                    let Some(message_wallet) = nonempty_message_wallet(&message) else {
                        continue;
                    };
                    if !self.accept_nostr_tx_broadcast(
                        &message,
                        &payload,
                        &session_id,
                        &message_wallet,
                    ) {
                        continue;
                    }
                    self.nostr_broadcasts
                        .insert(session_id.clone(), payload.clone());
                    self.append_nostr_audit_event(
                        frostdao::audit::AuditEvent::new(
                            "nostr_tx_broadcast_received",
                            &message_wallet,
                            "accepted",
                        )
                        .with_field("room", self.nostr_room_id.clone())
                        .with_field("transport", self.nostr_transport_label())
                        .with_field("session_id", session_id.clone())
                        .with_field("party_index", message.from)
                        .with_field("network", payload.network.clone())
                        .with_field("txid", payload.txid.clone()),
                    );
                    if matches!(
                        &self.nostr_sign_state,
                        NostrSignState::WaitingForExecution {
                            wallet_name: active_wallet,
                            session_id: active_session,
                            ..
                        } | NostrSignState::CollectingShares {
                            wallet_name: active_wallet,
                            session_id: active_session,
                            ..
                        } | NostrSignState::Combining {
                            wallet_name: active_wallet,
                            session_id: active_session,
                            ..
                        } if *active_wallet == message_wallet && *active_session == session_id
                    ) {
                        self.nostr_sign_state = NostrSignState::Complete { txid: payload.txid };
                    }
                }
                _ => {}
            }
        }

        Ok(count)
    }

    /// Publish a reviewed transaction proposal through the active room runtime.
    pub fn publish_nostr_tx_proposal(
        &mut self,
        wallet_name: &str,
        proposal: &TxProposal,
    ) -> Result<()> {
        let Some(runtime) = self.nostr_runtime.as_mut() else {
            anyhow::bail!("join a Nostr room before publishing a transaction proposal");
        };

        let payload = frostdao::nostr::TxProposalEvent {
            proposer_index: proposal.proposer_index,
            to_address: proposal.to_address.clone(),
            amount_sats: proposal.amount_sats,
            fee_rate: proposal.fee_rate,
            sighash: proposal.sighash.clone(),
            unsigned_tx: proposal.unsigned_tx.clone(),
            review: proposal.review.clone(),
            description: proposal.description.clone(),
            timestamp: proposal.timestamp,
        };
        let message = frostdao::nostr::NostrProtocolMessage::new(
            self.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxProposal,
            self.nostr_my_index,
            &payload,
        )?
        .with_wallet(wallet_name)
        .with_session(proposal.session_id.clone())
        .with_tss();

        runtime.publish(message)?;
        self.append_nostr_audit_event(
            frostdao::audit::AuditEvent::new("nostr_tx_proposal", wallet_name, "published")
                .with_field("room", self.nostr_room_id.clone())
                .with_field("transport", self.nostr_transport_label())
                .with_field("session_id", proposal.session_id.clone())
                .with_field("party_index", self.nostr_my_index)
                .with_field("network", proposal.review.network.clone())
                .with_field("source_path", proposal.review.source_path.clone())
                .with_field("from_address", proposal.review.from_address.clone())
                .with_field("to_address", proposal.to_address.clone())
                .with_field("amount_sats", proposal.amount_sats)
                .with_field("fee_rate_sats_vb", proposal.fee_rate)
                .with_field(
                    "sighash_fingerprint",
                    proposal.review.sighash_fingerprint.clone(),
                ),
        );
        Ok(())
    }

    /// Publish reviewed consent through the active room runtime.
    pub fn publish_nostr_tx_consent(
        &mut self,
        wallet_name: &str,
        proposal: &TxProposal,
        consent: bool,
        reason: Option<String>,
    ) -> Result<()> {
        let Some(runtime) = self.nostr_runtime.as_mut() else {
            anyhow::bail!("join a Nostr room before publishing transaction consent");
        };

        let payload = frostdao::nostr::TxConsentEvent {
            proposal_session: proposal.session_id.clone(),
            consent,
            reviewed_sighash_fingerprint: proposal.review.sighash_fingerprint.clone(),
            reason,
        };
        let message = frostdao::nostr::NostrProtocolMessage::new(
            self.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxConsent,
            self.nostr_my_index,
            &payload,
        )?
        .with_wallet(wallet_name)
        .with_session(proposal.session_id.clone())
        .with_tss();

        runtime.publish(message)?;
        self.append_nostr_audit_event(
            frostdao::audit::AuditEvent::new(
                "nostr_tx_consent",
                wallet_name,
                if consent { "consented" } else { "rejected" },
            )
            .with_field("room", self.nostr_room_id.clone())
            .with_field("transport", self.nostr_transport_label())
            .with_field("session_id", proposal.session_id.clone())
            .with_field("party_index", self.nostr_my_index)
            .with_field("network", proposal.review.network.clone())
            .with_field(
                "sighash_fingerprint",
                proposal.review.sighash_fingerprint.clone(),
            ),
        );
        Ok(())
    }

    /// Publish an encrypted signing nonce to another party through the active runtime.
    #[allow(dead_code)]
    pub fn publish_nostr_signing_nonce(
        &mut self,
        wallet_name: &str,
        session_id: &str,
        to_index: u32,
        ciphertext: String,
    ) -> Result<()> {
        self.validate_nostr_direct_publish(wallet_name, session_id, to_index, &ciphertext)?;
        let Some(runtime) = self.nostr_runtime.as_mut() else {
            anyhow::bail!("join a Nostr room before publishing signing nonce");
        };

        let payload =
            frostdao::nostr::SigningNonceEvent::new(self.nostr_my_index, to_index, ciphertext);
        let message = frostdao::nostr::NostrProtocolMessage::new(
            self.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::SigningNonceEncrypted,
            self.nostr_my_index,
            &payload,
        )?
        .with_wallet(wallet_name)
        .with_session(session_id)
        .with_tss()
        .to_party(to_index)?;

        runtime.publish(message)?;
        self.append_nostr_audit_event(
            frostdao::audit::AuditEvent::new("nostr_signing_nonce", wallet_name, "published")
                .with_field("room", self.nostr_room_id.clone())
                .with_field("transport", self.nostr_transport_label())
                .with_field("session_id", session_id)
                .with_field("party_index", self.nostr_my_index)
                .with_field("to_index", to_index),
        );
        Ok(())
    }

    /// Publish an encrypted signing share to another party through the active runtime.
    #[allow(dead_code)]
    pub fn publish_nostr_signing_share(
        &mut self,
        wallet_name: &str,
        session_id: &str,
        to_index: u32,
        ciphertext: String,
    ) -> Result<()> {
        self.validate_nostr_direct_publish(wallet_name, session_id, to_index, &ciphertext)?;
        let Some(runtime) = self.nostr_runtime.as_mut() else {
            anyhow::bail!("join a Nostr room before publishing signing share");
        };

        let payload =
            frostdao::nostr::SigningShareEvent::new(self.nostr_my_index, to_index, ciphertext);
        let message = frostdao::nostr::NostrProtocolMessage::new(
            self.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::SigningShareEncrypted,
            self.nostr_my_index,
            &payload,
        )?
        .with_wallet(wallet_name)
        .with_session(session_id)
        .with_tss()
        .to_party(to_index)?;

        runtime.publish(message)?;
        self.append_nostr_audit_event(
            frostdao::audit::AuditEvent::new("nostr_signing_share", wallet_name, "published")
                .with_field("room", self.nostr_room_id.clone())
                .with_field("transport", self.nostr_transport_label())
                .with_field("session_id", session_id)
                .with_field("party_index", self.nostr_my_index)
                .with_field("to_index", to_index),
        );
        Ok(())
    }

    /// Publish a transaction broadcast announcement through the active runtime.
    #[allow(dead_code)]
    pub fn publish_nostr_tx_broadcast(
        &mut self,
        wallet_name: &str,
        session_id: &str,
        txid: String,
        raw_tx: String,
        network: String,
    ) -> Result<()> {
        self.validate_nostr_broadcast_publish(wallet_name, session_id, &txid, &raw_tx, &network)?;
        let Some(runtime) = self.nostr_runtime.as_mut() else {
            anyhow::bail!("join a Nostr room before publishing transaction broadcast");
        };

        let payload = frostdao::nostr::TxBroadcastEvent {
            txid: txid.clone(),
            raw_tx,
            network: network.clone(),
        };
        let message = frostdao::nostr::NostrProtocolMessage::new(
            self.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxBroadcast,
            self.nostr_my_index,
            &payload,
        )?
        .with_wallet(wallet_name)
        .with_session(session_id)
        .with_tss();

        runtime.publish(message)?;
        self.append_nostr_audit_event(
            frostdao::audit::AuditEvent::new("nostr_tx_broadcast", wallet_name, "published")
                .with_field("room", self.nostr_room_id.clone())
                .with_field("transport", self.nostr_transport_label())
                .with_field("session_id", session_id)
                .with_field("party_index", self.nostr_my_index)
                .with_field("network", network)
                .with_field("txid", txid),
        );
        Ok(())
    }

    /// Publish a local-simulation participant join into the active room transport.
    pub fn simulate_nostr_participant_join(&mut self, party_index: u32) -> Result<()> {
        let Some(runtime) = self.nostr_runtime.as_mut() else {
            anyhow::bail!("join a room before simulating participants");
        };

        let payload = frostdao::nostr::RoomJoinPayload {
            party_index,
            nostr_pubkey: format!("npub-local-sim-{}", party_index),
            threshold: self.nostr_threshold,
            n_parties: self.nostr_n_parties,
            scheme: frostdao::nostr::ThresholdScheme::Tss,
            rank: None,
        };
        let message = frostdao::nostr::NostrProtocolMessage::new(
            self.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::RoomJoin,
            party_index,
            &payload,
        )?
        .with_tss();

        runtime.publish_local_simulation_message(message)?;
        self.poll_nostr_room_runtime()?;
        Ok(())
    }

    /// Toggle to next network in chain selector
    pub fn next_network(&mut self) {
        self.chain_selector_index = (self.chain_selector_index + 1) % NetworkSelection::all().len();
    }

    /// Toggle to previous network in chain selector
    pub fn prev_network(&mut self) {
        self.chain_selector_index = if self.chain_selector_index == 0 {
            NetworkSelection::all().len() - 1
        } else {
            self.chain_selector_index - 1
        };
    }

    /// Confirm network selection
    pub fn confirm_network(&mut self) {
        let selected_network = NetworkSelection::from_index(self.chain_selector_index);
        if selected_network == self.network {
            self.state = AppState::Home;
            self.message = Some(format!(
                "Already on {}; no pending send, reshare, or Nostr ceremony state was changed",
                self.network.display_name()
            ));
            return;
        }

        self.network = selected_network;
        self.clear_network_volatile_state();
        self.state = AppState::Home;
        self.message = Some(format!(
            "Switched to {}; {}; cleared pending send, reshare, and Nostr ceremony state",
            self.network.display_name(),
            self.network.policy_hint()
        ));
    }

    fn clear_network_volatile_state(&mut self) {
        self.send_form = SendFormData::new();
        self.reshare_form = crate::tui::screens::ReshareFormData::new();
        #[cfg(feature = "miniscript-policy")]
        {
            self.policy_preview_form = crate::tui::screens::PolicyPreviewFormData::new();
        }
        self.nostr_runtime = None;
        self.nostr_connected = false;
        self.nostr_room_phase = NostrRoomPhase::Configure;
        self.clear_nostr_room_session_state();
    }

    /// Helper message for UTXO-dependent actions when the current network has no fetch source.
    pub(crate) fn utxo_source_unavailable_message(&self) -> Option<String> {
        self.network.mempool_api_base().err().map(|err| {
            format!(
                "Cannot fetch UTXOs on {}: {}",
                self.network.display_name(),
                err
            )
        })
    }

    /// Balance fetch hint shown in wallet list/details views.
    pub(crate) fn balance_fetch_hint(&self) -> String {
        match self.utxo_source_unavailable_message() {
            Some(error) => error,
            None => format!("Press {REFRESH_KEY_LABEL} to fetch"),
        }
    }

    /// Set status message
    pub fn set_message(&mut self, msg: &str) {
        self.message = Some(msg.to_string());
    }

    /// Copy text to clipboard
    pub fn copy_to_clipboard(&mut self, text: &str) {
        let success_message = copied_preview_message(text);
        self.copy_to_clipboard_with_message(text, success_message);
    }

    /// Copy text to clipboard with a caller-provided success message.
    pub fn copy_to_clipboard_with_message(&mut self, text: &str, success_message: String) {
        if text.trim().is_empty() {
            self.message = Some("Nothing to copy".to_string());
            return;
        }

        match arboard::Clipboard::new() {
            Ok(mut clipboard) => match clipboard.set_text(text) {
                Ok(_) => {
                    self.message = Some(success_message);
                }
                Err(e) => {
                    self.message = Some(format!("Clipboard error: {}", e));
                }
            },
            Err(e) => {
                self.message = Some(format!("Clipboard unavailable: {}", e));
            }
        }
    }

    /// Load HD addresses for a wallet
    pub fn load_hd_addresses(&mut self, wallet_name: &str) {
        let btc_network = self.network.to_bitcoin_network();
        let state_dir = frostdao::protocol::keygen::get_state_dir(wallet_name);

        match FileStorage::new(&state_dir) {
            Ok(storage) => {
                // Check if HD is enabled
                match storage.read("hd_metadata.json") {
                    Ok(bytes) => {
                        let hd_json = String::from_utf8_lossy(&bytes);
                        match serde_json::from_str::<frostdao::protocol::keygen::HdMetadata>(
                            &hd_json,
                        ) {
                            Ok(metadata) => {
                                if metadata.hd_enabled {
                                    // Load addresses using stored derived_count
                                    match frostdao::btc::hd_address::list_derived_addresses(
                                        &storage,
                                        metadata.derived_count,
                                        btc_network,
                                    ) {
                                        Ok(addresses) => {
                                            if let AppState::AddressList(ref mut state) = self.state
                                            {
                                                state.addresses = addresses;
                                                state.hd_enabled = true;
                                            }
                                        }
                                        Err(e) => {
                                            if let AppState::AddressList(ref mut state) = self.state
                                            {
                                                state.error = Some(format!("Error loading: {}", e));
                                            }
                                        }
                                    }
                                } else {
                                    if let AppState::AddressList(ref mut state) = self.state {
                                        state.error =
                                            Some("HD not enabled for this wallet".to_string());
                                    }
                                }
                            }
                            Err(e) => {
                                if let AppState::AddressList(ref mut state) = self.state {
                                    state.error =
                                        Some(format!("Invalid HD metadata format: {}", e));
                                }
                            }
                        }
                    }
                    Err(_) => {
                        if let AppState::AddressList(ref mut state) = self.state {
                            state.error = Some("HD not enabled for this wallet".to_string());
                        }
                    }
                }
            }
            Err(e) => {
                if let AppState::AddressList(ref mut state) = self.state {
                    state.error = Some(format!("Storage error: {}", e));
                }
            }
        }
    }

    /// Add a new HD address (derive next index)
    pub fn add_hd_address(&mut self, wallet_name: &str) {
        let state_dir = frostdao::protocol::keygen::get_state_dir(wallet_name);
        if let Ok(storage) = FileStorage::new(&state_dir) {
            match frostdao::btc::hd_address::add_address(&storage) {
                Ok(new_count) => {
                    self.message = Some(format!("Added address {}", new_count - 1));
                    // Reload addresses
                    self.load_hd_addresses(wallet_name);
                }
                Err(e) => {
                    self.message = Some(format!("Error adding address: {}", e));
                }
            }
        }
    }

    /// Remove the last HD address
    pub fn remove_hd_address(&mut self, wallet_name: &str) {
        let state_dir = frostdao::protocol::keygen::get_state_dir(wallet_name);
        if let Ok(storage) = FileStorage::new(&state_dir) {
            // Get current count first
            if let Ok(current) = frostdao::btc::hd_address::get_derived_count(&storage) {
                if current <= 1 {
                    self.message = Some("Cannot remove: minimum 1 address required".to_string());
                    return;
                }
            }

            match frostdao::btc::hd_address::remove_address(&storage) {
                Ok(new_count) => {
                    self.message = Some(format!(
                        "Removed address. Now showing {} addresses",
                        new_count
                    ));
                    // Reload addresses and adjust selection if needed
                    self.load_hd_addresses(wallet_name);
                    if let AppState::AddressList(ref mut state) = self.state {
                        if state.selected >= state.addresses.len() && !state.addresses.is_empty() {
                            state.selected = state.addresses.len() - 1;
                        }
                    }
                }
                Err(e) => {
                    self.message = Some(format!("Error removing address: {}", e));
                }
            }
        }
    }

    fn append_nostr_audit_event(&mut self, event: frostdao::audit::AuditEvent) {
        #[cfg(test)]
        {
            self.audit_events.push(event);
        }
        #[cfg(not(test))]
        {
            let _ = frostdao::audit::append(&event);
        }
    }

    fn clear_nostr_room_session_state(&mut self) {
        self.nostr_participants.clear();
        self.nostr_pending_proposals.clear();
        self.nostr_received_nonces.clear();
        self.nostr_received_shares.clear();
        self.nostr_broadcasts.clear();
        self.nostr_signing_coordinators.clear();
        self.nostr_keygen_state = NostrKeygenState::ModeSelect;
        self.nostr_sign_state = NostrSignState::SelectWallet;
    }

    pub fn start_nostr_signing_attempt(
        &mut self,
        wallet_name: &str,
        session_id: &str,
        sighash_fingerprint: &str,
    ) -> Result<()> {
        let signer_set: Vec<u32> = (1..=self.nostr_n_parties).collect();
        let config = frostdao::protocol::SigningAttemptConfig::new(
            wallet_name,
            session_id,
            signer_set,
            self.nostr_threshold,
            sighash_fingerprint,
            SigningSchemePolicy::Tss,
        )?;
        let mut coordinator = SigningCoordinator::new(config)?;
        if let Some(nonces) = self.nostr_received_nonces.get(session_id) {
            for (party_index, public_nonce) in nonces {
                let config = coordinator.config().clone();
                coordinator.accept_nonce(SigningNonceInput {
                    wallet: wallet_name.to_string(),
                    session: session_id.to_string(),
                    attempt_id: config.attempt_id,
                    signer_set: config.signer_set,
                    party_index: *party_index,
                    sighash_fingerprint: config.sighash_fingerprint,
                    public_nonce: public_nonce.clone(),
                })?;
            }
        }
        self.nostr_signing_coordinators
            .insert(session_id.to_string(), coordinator);
        Ok(())
    }

    fn nostr_signing_nonce_input(
        &self,
        wallet_name: &str,
        session_id: &str,
        message: &frostdao::nostr::NostrProtocolMessage,
        payload: &frostdao::nostr::SigningNonceEvent,
    ) -> Result<SigningNonceInput> {
        if let Some(conversation_key) =
            self.nostr_conversation_key_for_party(payload.party_index)?
        {
            return Ok(frostdao::nostr::decrypt_signing_nonce_plaintext(
                &payload.ciphertext,
                &conversation_key,
                message,
                payload,
            )?
            .into());
        }

        Ok(SigningNonceInput {
            wallet: wallet_name.to_string(),
            session: session_id.to_string(),
            attempt_id: nostr_session_attempt_id(session_id),
            signer_set: self.nostr_signer_set(),
            party_index: payload.party_index,
            sighash_fingerprint: nostr_active_sighash_fingerprint(&self.nostr_sign_state)
                .unwrap_or_default(),
            public_nonce: payload.ciphertext.clone(),
        })
    }

    fn nostr_signing_share_input(
        &self,
        wallet_name: &str,
        session_id: &str,
        message: &frostdao::nostr::NostrProtocolMessage,
        payload: &frostdao::nostr::SigningShareEvent,
    ) -> Result<SigningShareInput> {
        if let Some(conversation_key) =
            self.nostr_conversation_key_for_party(payload.party_index)?
        {
            return Ok(frostdao::nostr::decrypt_signing_share_plaintext(
                &payload.ciphertext,
                &conversation_key,
                message,
                payload,
            )?
            .into());
        }

        Ok(SigningShareInput {
            wallet: wallet_name.to_string(),
            session: session_id.to_string(),
            attempt_id: nostr_session_attempt_id(session_id),
            signer_set: self.nostr_signer_set(),
            party_index: payload.party_index,
            sighash_fingerprint: nostr_active_sighash_fingerprint(&self.nostr_sign_state)
                .unwrap_or_default(),
            signature_share: payload.ciphertext.clone(),
        })
    }

    fn nostr_conversation_key_for_party(&self, party_index: u32) -> Result<Option<[u8; 32]>> {
        let Some(peer_pubkey) = self.nostr_participants.get(&party_index) else {
            return Ok(None);
        };
        let Some(runtime) = &self.nostr_runtime else {
            return Ok(None);
        };
        runtime.conversation_key_with(peer_pubkey)
    }

    fn nostr_signer_set(&self) -> Vec<u32> {
        (1..=self.nostr_n_parties).collect()
    }

    fn accept_nostr_signing_nonce_for_coordinator(
        &mut self,
        session_id: &str,
        input: SigningNonceInput,
    ) -> Result<()> {
        let Some(coordinator) = self.nostr_signing_coordinators.get_mut(session_id) else {
            return Ok(());
        };
        let config = coordinator.config().clone();
        let input = SigningNonceInput {
            attempt_id: config.attempt_id,
            signer_set: config.signer_set,
            sighash_fingerprint: config.sighash_fingerprint,
            ..input
        };
        coordinator.accept_nonce(input)?;
        Ok(())
    }

    fn accept_nostr_signing_share_for_coordinator(
        &mut self,
        session_id: &str,
        input: SigningShareInput,
    ) -> Result<bool> {
        let Some(coordinator) = self.nostr_signing_coordinators.get_mut(session_id) else {
            return Ok(false);
        };
        let config = coordinator.config().clone();
        let input = SigningShareInput {
            attempt_id: config.attempt_id,
            signer_set: config.signer_set,
            sighash_fingerprint: config.sighash_fingerprint,
            ..input
        };
        let progress = coordinator.accept_share(input)?;
        Ok(progress.accepted_new)
    }

    fn accept_nostr_room_join(
        &self,
        message: &frostdao::nostr::NostrProtocolMessage,
        payload: &frostdao::nostr::RoomJoinPayload,
    ) -> bool {
        payload.party_index == message.from
            && payload.party_index > 0
            && payload.party_index <= self.nostr_n_parties
            && payload.threshold == self.nostr_threshold
            && payload.n_parties == self.nostr_n_parties
            && payload.scheme == frostdao::nostr::ThresholdScheme::Tss
            && payload.rank.is_none()
    }

    fn validate_nostr_direct_publish(
        &self,
        wallet_name: &str,
        session_id: &str,
        to_index: u32,
        ciphertext: &str,
    ) -> Result<()> {
        if wallet_name.trim().is_empty() {
            anyhow::bail!("wallet name is required");
        }
        if session_id.trim().is_empty() {
            anyhow::bail!("signing session is required");
        }
        if to_index == 0 || to_index > self.nostr_n_parties {
            anyhow::bail!("recipient party must be inside the active room");
        }
        if to_index == self.nostr_my_index {
            anyhow::bail!("direct signing messages must target another party");
        }
        if self.nostr_connected && !self.nostr_participants.contains_key(&to_index) {
            anyhow::bail!("recipient party has not joined the active room");
        }
        if ciphertext.trim().is_empty() {
            anyhow::bail!("encrypted payload is required");
        }
        Ok(())
    }

    fn validate_nostr_broadcast_publish(
        &self,
        wallet_name: &str,
        session_id: &str,
        txid: &str,
        raw_tx: &str,
        network: &str,
    ) -> Result<()> {
        if wallet_name.trim().is_empty() {
            anyhow::bail!("wallet name is required");
        }
        if session_id.trim().is_empty() {
            anyhow::bail!("signing session is required");
        }
        if txid.trim().is_empty() {
            anyhow::bail!("transaction id is required");
        }
        if raw_tx.trim().is_empty() {
            anyhow::bail!("raw transaction hex is required");
        }
        if network != self.network.display_name() {
            anyhow::bail!("broadcast network must match the selected TUI network");
        }
        let tx_bytes = hex::decode(raw_tx.trim()).map_err(|err| anyhow::anyhow!("{err}"))?;
        let tx = bitcoin::consensus::deserialize::<bitcoin::Transaction>(&tx_bytes)
            .map_err(|err| anyhow::anyhow!("{err}"))?;
        if tx.compute_txid().to_string() != txid {
            anyhow::bail!("broadcast txid does not match raw transaction");
        }
        Ok(())
    }

    fn accept_nostr_party_ciphertext(
        &self,
        message: &frostdao::nostr::NostrProtocolMessage,
        party_index: u32,
        to_index: u32,
    ) -> bool {
        party_index == message.from
            && party_index > 0
            && party_index <= self.nostr_n_parties
            && to_index == self.nostr_my_index
            && message.to == Some(self.nostr_my_index)
    }

    fn accept_nostr_tx_proposal(
        &self,
        message: &frostdao::nostr::NostrProtocolMessage,
        payload: &frostdao::nostr::TxProposalEvent,
    ) -> bool {
        if payload.proposer_index != message.from
            || payload.proposer_index == 0
            || payload.proposer_index > self.nostr_n_parties
            || payload.amount_sats == 0
            || payload.fee_rate == 0
            || payload.sighash.trim().is_empty()
            || payload.unsigned_tx.trim().is_empty()
        {
            return false;
        }

        let Ok(tx_bytes) = hex::decode(payload.unsigned_tx.trim()) else {
            return false;
        };
        if bitcoin::consensus::deserialize::<bitcoin::Transaction>(&tx_bytes).is_err() {
            return false;
        }

        if payload.review.network != self.network.display_name()
            || payload.review.source_path.trim().is_empty()
            || payload.review.from_address.trim().is_empty()
            || payload.review.to_address.trim().is_empty()
            || payload.review.amount_sats != payload.amount_sats
            || payload.review.fee_rate_sats_vb != payload.fee_rate
            || payload.review.sighash_fingerprint
                != frostdao::protocol::dkg_tx::sighash_fingerprint(&payload.sighash)
        {
            return false;
        }

        let network = self.network.to_bitcoin_network();
        let network_name = self.network.display_name();
        let Ok(to_address) =
            parse_tui_recipient_address(payload.to_address.trim(), network, network_name)
        else {
            return false;
        };
        if payload.review.to_address != to_address {
            return false;
        }

        parse_tui_recipient_address(payload.review.from_address.trim(), network, network_name)
            .is_ok()
    }

    fn accept_nostr_tx_broadcast(
        &self,
        message: &frostdao::nostr::NostrProtocolMessage,
        payload: &frostdao::nostr::TxBroadcastEvent,
        session_id: &str,
        message_wallet: &str,
    ) -> bool {
        if message.from == 0
            || message.from > self.nostr_n_parties
            || payload.txid.trim().is_empty()
            || payload.raw_tx.trim().is_empty()
            || payload.network != self.network.display_name()
        {
            return false;
        }

        let active_matches = matches!(
            &self.nostr_sign_state,
            NostrSignState::WaitingForExecution {
                wallet_name,
                session_id: active_session,
            } | NostrSignState::CollectingShares {
                wallet_name,
                session_id: active_session,
                ..
            } | NostrSignState::Combining {
                wallet_name,
                session_id: active_session,
            } if wallet_name == message_wallet && active_session == session_id
        );
        if !active_matches {
            return false;
        }

        let Ok(tx_bytes) = hex::decode(payload.raw_tx.trim()) else {
            return false;
        };
        let Ok(tx) = bitcoin::consensus::deserialize::<bitcoin::Transaction>(&tx_bytes) else {
            return false;
        };
        tx.compute_txid().to_string() == payload.txid
    }
}

fn unix_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(1)
}

fn parse_tui_recipient_address(
    recipient: &str,
    network: bitcoin::Network,
    network_name: &str,
) -> Result<String> {
    if recipient.is_empty() {
        anyhow::bail!("recipient address is required");
    }

    match Address::from_str(recipient) {
        Ok(address) => address
            .require_network(network)
            .map(|address| address.to_string())
            .map_err(|err| {
                anyhow::anyhow!("recipient address is invalid for {network_name}: {err}")
            }),
        Err(err) => Err(anyhow::anyhow!("invalid recipient address: {err}")),
    }
}

fn nonempty_message_session(message: &frostdao::nostr::NostrProtocolMessage) -> Option<String> {
    message
        .session
        .as_deref()
        .map(str::trim)
        .filter(|session| !session.is_empty())
        .map(str::to_string)
}

fn nonempty_message_wallet(message: &frostdao::nostr::NostrProtocolMessage) -> Option<String> {
    message
        .wallet
        .as_deref()
        .map(str::trim)
        .filter(|wallet| !wallet.is_empty())
        .map(str::to_string)
}

fn nostr_session_attempt_id(session_id: &str) -> String {
    format!("nostr-session-{session_id}")
}

fn nostr_active_sighash_fingerprint(state: &NostrSignState) -> Option<String> {
    match state {
        NostrSignState::WaitingForConsent { proposal, .. }
        | NostrSignState::ReviewProposal { proposal, .. } => {
            Some(proposal.review.sighash_fingerprint.clone())
        }
        _ => None,
    }
}

fn nostr_relay_urls_from_env() -> Vec<String> {
    std::env::var(TUI_NOSTR_RELAYS_ENV)
        .ok()
        .into_iter()
        .flat_map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|relay| !relay.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn mainnet_nostr_enabled() -> bool {
    std::env::var(TUI_MAINNET_NOSTR_ENV).as_deref() == Ok("1")
}

#[cfg(test)]
mod tests {
    use super::{App, BalanceInfo};
    use crate::tui::screens::{TxDisplay, UtxoDisplay};
    use crate::tui::state::{
        AppState, NetworkSelection, NostrKeygenState, NostrRoomPhase, NostrSignState, TxProposal,
    };
    use bitcoin::absolute::LockTime;
    use bitcoin::secp256k1::{Keypair, Secp256k1, SecretKey};
    use bitcoin::transaction::Version;
    use bitcoin::{
        Address, Amount, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
        XOnlyPublicKey,
    };
    use frostdao::protocol::keygen::WalletSummary;
    use serial_test::serial;
    use std::collections::BTreeMap;
    use std::collections::HashMap;

    fn wallet_summary(name: &str, address: Option<String>) -> WalletSummary {
        let address_mainnet = address.as_ref().map(|_| test_address(Network::Bitcoin));
        let address_regtest = address.as_ref().map(|_| test_address(Network::Regtest));
        WalletSummary {
            name: name.to_string(),
            threshold: Some(2),
            total_parties: Some(3),
            hierarchical: Some(false),
            address: address.clone(),
            address_testnet: address,
            address_mainnet,
            address_regtest,
            signing_requirement: None,
            party_ranks: None::<BTreeMap<u32, u32>>,
        }
    }

    fn test_address(network: Network) -> String {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let (xonly_pubkey, _) = XOnlyPublicKey::from_keypair(&keypair);
        Address::p2tr(&secp, xonly_pubkey, None, network).to_string()
    }

    fn app_with_wallet(address: Option<String>) -> App {
        let mut app = App::new().unwrap();
        app.wallets = vec![wallet_summary("wallet-test", address)];
        app.wallet_list_state.select(Some(0));
        app.network = NetworkSelection::Testnet3;
        app.nostr_my_index = 2;
        app.nostr_to_address = test_address(Network::Testnet);
        app.nostr_amount_sats = 50_000;
        app
    }

    #[test]
    fn wallet_address_for_network_uses_explicit_mainnet_address() {
        let testnet = test_address(Network::Testnet);
        let mainnet = test_address(Network::Bitcoin);
        let regtest = test_address(Network::Regtest);
        let wallet = WalletSummary {
            name: "wallet-test".to_string(),
            threshold: Some(2),
            total_parties: Some(3),
            hierarchical: Some(false),
            address: Some(testnet.clone()),
            address_testnet: Some(testnet.clone()),
            address_mainnet: Some(mainnet.clone()),
            address_regtest: Some(regtest.clone()),
            signing_requirement: None,
            party_ranks: None::<BTreeMap<u32, u32>>,
        };

        assert_eq!(
            super::wallet_address_for_network(&wallet, NetworkSelection::Testnet3),
            Some(testnet.as_str())
        );
        assert_eq!(
            super::wallet_address_for_network(&wallet, NetworkSelection::Signet),
            Some(testnet.as_str())
        );
        assert_eq!(
            super::wallet_address_for_network(&wallet, NetworkSelection::Regtest),
            Some(regtest.as_str())
        );
        assert_eq!(
            super::wallet_address_for_network(&wallet, NetworkSelection::Mainnet),
            Some(mainnet.as_str())
        );
    }

    #[test]
    fn wallet_address_for_network_does_not_fake_mainnet_from_testnet_address() {
        let wallet = WalletSummary {
            name: "wallet-test".to_string(),
            threshold: Some(2),
            total_parties: Some(3),
            hierarchical: Some(false),
            address: Some(test_address(Network::Testnet)),
            address_testnet: None,
            address_mainnet: None,
            address_regtest: None,
            signing_requirement: None,
            party_ranks: None::<BTreeMap<u32, u32>>,
        };

        assert!(super::wallet_address_for_network(&wallet, NetworkSelection::Mainnet).is_none());
    }

    #[test]
    fn wallet_address_for_network_does_not_reuse_testnet_address_for_regtest() {
        let testnet = test_address(Network::Testnet);
        let wallet = WalletSummary {
            name: "wallet-test".to_string(),
            threshold: Some(2),
            total_parties: Some(3),
            hierarchical: Some(false),
            address: Some(testnet.clone()),
            address_testnet: Some(testnet),
            address_mainnet: None,
            address_regtest: None,
            signing_requirement: None,
            party_ranks: None::<BTreeMap<u32, u32>>,
        };

        assert!(super::wallet_address_for_network(&wallet, NetworkSelection::Regtest).is_none());
    }

    #[test]
    fn balance_cache_key_is_network_scoped() {
        assert_eq!(
            super::balance_cache_key("treasury", NetworkSelection::Testnet3),
            "treasury:Testnet3"
        );
        assert_eq!(
            super::balance_cache_key("treasury", NetworkSelection::Signet),
            "treasury:Signet"
        );
        assert_ne!(
            super::balance_cache_key("treasury", NetworkSelection::Testnet3),
            super::balance_cache_key("treasury", NetworkSelection::Mainnet)
        );
    }

    #[test]
    fn missing_network_address_message_names_network_and_blocked_actions() {
        let message = super::missing_network_address_message(NetworkSelection::Mainnet);

        assert!(message.contains("missing Mainnet source address"));
        assert!(message.contains("before send, copy, or QR"));
    }

    #[test]
    fn wallet_balance_updated_message_includes_network() {
        assert_eq!(
            super::wallet_balance_updated_message("treasury", NetworkSelection::Signet),
            "Signet balance updated for treasury"
        );
    }

    #[test]
    fn chain_selector_cycles_all_supported_networks() {
        let mut app = App::new().unwrap();
        let network_count = NetworkSelection::all().len();

        for _ in 0..network_count {
            app.next_network();
        }
        assert_eq!(app.chain_selector_index, 0);

        app.prev_network();
        assert_eq!(app.chain_selector_index, network_count - 1);
    }

    #[test]
    fn chain_selector_can_confirm_regtest() {
        let mut app = App::new().unwrap();
        app.chain_selector_index = 3;

        app.confirm_network();

        assert_eq!(app.network, NetworkSelection::Regtest);
        assert_eq!(app.network.to_bitcoin_network(), Network::Regtest);
        let message = app.message.as_deref().unwrap_or_default();
        assert!(message.contains("regtest uses local Esplora/mempool API"));
        assert!(message.contains("cleared pending send, reshare, and Nostr ceremony state"));
    }

    #[test]
    fn copy_to_clipboard_shows_helpful_message_for_empty_text() {
        let mut app = App::new().unwrap();

        app.copy_to_clipboard("");

        assert_eq!(app.message.as_deref(), Some("Nothing to copy"));
    }

    #[test]
    fn copy_to_clipboard_shows_helpful_message_for_whitespace_only_text() {
        let mut app = App::new().unwrap();

        app.copy_to_clipboard("   ");

        assert_eq!(app.message.as_deref(), Some("Nothing to copy"));
    }

    #[test]
    fn network_switch_clears_volatile_send_and_nostr_state() {
        let mut app = App::new().unwrap();
        app.chain_selector_index = 2;
        app.state = AppState::NostrSign;
        app.send_form.to_address.set_value("tb1qstale");
        app.send_form.total_balance = 50_000;
        app.send_form.error_message = Some("stale send error".to_string());
        app.reshare_form.round1_output = "stale reshare round".to_string();
        app.reshare_form.error_message = Some("stale reshare error".to_string());
        app.nostr_connected = true;
        app.nostr_room_phase = NostrRoomPhase::Ready;
        app.nostr_participants.insert(1, "npub-local-1".to_string());
        app.nostr_pending_proposals.insert(
            "session-stale".to_string(),
            TxProposal {
                session_id: "session-stale".to_string(),
                wallet_name: "wallet-test".to_string(),
                proposer_index: 2,
                to_address: test_address(Network::Testnet),
                amount_sats: 10_000,
                fee_rate: 2,
                sighash: "stale-sighash".to_string(),
                unsigned_tx: valid_transaction_hex(2_000),
                review: frostdao::nostr::TxReviewPayload {
                    network: "Testnet3".to_string(),
                    source_path: "root key-path".to_string(),
                    from_address: test_address(Network::Testnet),
                    to_address: test_address(Network::Testnet),
                    amount_sats: 10_000,
                    fee_rate_sats_vb: 2,
                    sighash_fingerprint: "stale-fingerprint".to_string(),
                },
                description: "stale proposal".to_string(),
                timestamp: 1_700_000_000,
            },
        );

        app.confirm_network();

        assert_eq!(app.network, NetworkSelection::Signet);
        assert!(matches!(app.state, AppState::Home));
        assert_eq!(app.send_form.to_address.value(), "");
        assert_eq!(app.send_form.total_balance, 0);
        assert!(app.send_form.error_message.is_none());
        assert!(app.reshare_form.round1_output.is_empty());
        assert!(app.reshare_form.error_message.is_none());
        assert!(!app.nostr_connected);
        assert!(matches!(app.nostr_room_phase, NostrRoomPhase::Configure));
        assert!(app.nostr_participants.is_empty());
        assert!(app.nostr_pending_proposals.is_empty());
        assert!(matches!(app.nostr_sign_state, NostrSignState::SelectWallet));
        assert!(app
            .message
            .as_deref()
            .unwrap_or("")
            .contains("cleared pending send, reshare, and Nostr ceremony state"));
    }

    #[test]
    fn confirming_current_network_preserves_volatile_ceremony_state() {
        let mut app = App::new().unwrap();
        app.chain_selector_index = app.network.index();
        app.state = AppState::NostrSign;
        app.send_form.to_address.set_value("tb1qdraft");
        app.send_form.total_balance = 50_000;
        app.reshare_form.round1_output = "draft reshare round".to_string();
        app.nostr_connected = true;
        app.nostr_room_phase = NostrRoomPhase::Ready;
        app.nostr_participants.insert(1, "npub-local-1".to_string());
        app.nostr_pending_proposals.insert(
            "session-draft".to_string(),
            TxProposal {
                session_id: "session-draft".to_string(),
                wallet_name: "wallet-test".to_string(),
                proposer_index: 2,
                to_address: test_address(Network::Testnet),
                amount_sats: 10_000,
                fee_rate: 2,
                sighash: "draft-sighash".to_string(),
                unsigned_tx: valid_transaction_hex(2_001),
                review: frostdao::nostr::TxReviewPayload {
                    network: "Testnet4".to_string(),
                    source_path: "root key-path".to_string(),
                    from_address: test_address(Network::Testnet4),
                    to_address: test_address(Network::Testnet4),
                    amount_sats: 10_000,
                    fee_rate_sats_vb: 2,
                    sighash_fingerprint: "draft-fingerprint".to_string(),
                },
                description: "draft proposal".to_string(),
                timestamp: 1_700_000_000,
            },
        );

        app.confirm_network();

        assert_eq!(app.network, NetworkSelection::Testnet4);
        assert!(matches!(app.state, AppState::Home));
        assert_eq!(app.send_form.to_address.value(), "tb1qdraft");
        assert_eq!(app.send_form.total_balance, 50_000);
        assert_eq!(app.reshare_form.round1_output, "draft reshare round");
        assert!(app.nostr_connected);
        assert!(matches!(app.nostr_room_phase, NostrRoomPhase::Ready));
        assert_eq!(app.nostr_participants.len(), 1);
        assert_eq!(app.nostr_pending_proposals.len(), 1);
        let message = app.message.as_deref().unwrap_or_default();
        assert!(message.contains("Already on Testnet4"));
        assert!(message.contains("no pending send, reshare, or Nostr ceremony state was changed"));
    }

    #[test]
    fn network_switch_preserves_network_scoped_balance_cache() {
        let mut app = App::new().unwrap();
        let cache_key = super::balance_cache_key("treasury", NetworkSelection::Testnet3);
        app.balance_cache.insert(
            cache_key.clone(),
            BalanceInfo {
                balance_sats: 12_345,
                utxo_count: 2,
            },
        );
        app.chain_selector_index = 4;

        app.confirm_network();

        assert_eq!(app.network, NetworkSelection::Mainnet);
        assert!(app.balance_cache.contains_key(&cache_key));
        assert_ne!(
            cache_key,
            super::balance_cache_key("treasury", NetworkSelection::Mainnet)
        );
    }

    #[test]
    #[serial]
    fn regtest_requires_configured_mempool_api_fetches() {
        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);

        let err = NetworkSelection::Regtest.mempool_api_base().unwrap_err();

        assert!(err.to_string().contains("local Esplora/mempool API"));
        assert!(err
            .to_string()
            .contains(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV));
    }

    #[test]
    #[serial]
    fn regtest_uses_configured_mempool_api_fetches() {
        std::env::set_var(
            frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV,
            "http://127.0.0.1:3002/api/",
        );

        assert_eq!(
            NetworkSelection::Regtest.mempool_api_base().unwrap(),
            "http://127.0.0.1:3002/api"
        );

        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);
    }

    #[test]
    #[serial]
    fn regtest_utxo_fetch_marks_send_form_unavailable() {
        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);
        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Regtest;

        app.fetch_utxos_for_send("bcrt1qexample");

        assert!(app.send_form.utxos.is_empty());
        assert_eq!(app.send_form.total_balance, 0);
        assert_eq!(app.send_form.estimated_fee, 0);
        assert_eq!(app.send_form.utxos_needed, 0);
        assert!(app
            .send_form
            .utxo_fetch_error
            .as_deref()
            .unwrap_or("")
            .contains("local Esplora/mempool API"));
        assert!(app
            .send_form
            .error_message
            .as_deref()
            .unwrap_or("")
            .contains("Cannot fetch UTXOs on Regtest"));
    }

    #[test]
    #[serial]
    fn fetch_utxos_clears_stale_data_on_transport_failure() {
        std::env::set_var(
            frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV,
            "http://127.0.0.1:1/api/",
        );

        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Regtest;
        app.send_form.utxos = vec![UtxoDisplay {
            txid: "00".repeat(32),
            vout: 0,
            value: 10_000,
            confirmed: true,
        }];
        app.send_form.recent_txs = vec![TxDisplay {
            txid: "11".repeat(32),
            amount: 1_000,
            confirmed: true,
            time: None,
        }];
        app.send_form.total_balance = 10_000;
        app.send_form.estimated_fee = 100;
        app.send_form.utxos_needed = 1;
        app.send_form.error_message = Some("stale send error".to_string());

        app.fetch_utxos_for_send("bcrt1qexample");

        assert!(app.send_form.utxos.is_empty());
        assert!(app.send_form.recent_txs.is_empty());
        assert_eq!(app.send_form.total_balance, 0);
        assert_eq!(app.send_form.estimated_fee, 0);
        assert_eq!(app.send_form.utxos_needed, 0);
        assert_eq!(app.send_form.fee_rate, 1);
        assert_eq!(
            app.send_form.utxo_fetch_error.as_deref().unwrap_or(""),
            "Cannot fetch fee estimates on Regtest"
        );
        assert_eq!(
            app.send_form.error_message.as_deref().unwrap_or(""),
            "Cannot fetch fee estimates on Regtest"
        );

        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);
    }

    fn valid_remote_proposal_event(proposer_index: u32) -> frostdao::nostr::TxProposalEvent {
        let to_address = test_address(Network::Testnet);
        let from_address = test_address(Network::Testnet);
        let sighash = "remote-sighash".to_string();
        frostdao::nostr::TxProposalEvent {
            proposer_index,
            to_address: to_address.clone(),
            amount_sats: 25_000,
            fee_rate: 8,
            sighash: sighash.clone(),
            unsigned_tx: valid_transaction_hex(2_000),
            review: frostdao::nostr::TxReviewPayload {
                network: "Testnet3".to_string(),
                source_path: "m/86'/1'/0'/0/1".to_string(),
                from_address,
                to_address,
                amount_sats: 25_000,
                fee_rate_sats_vb: 8,
                sighash_fingerprint: frostdao::protocol::dkg_tx::sighash_fingerprint(&sighash),
            },
            description: "remote proposal".to_string(),
            timestamp: 1_700_000_010,
        }
    }

    fn valid_broadcast_event_with_value(value_sats: u64) -> frostdao::nostr::TxBroadcastEvent {
        let raw_tx = valid_transaction_hex(value_sats);
        let tx_bytes = hex::decode(&raw_tx).unwrap();
        let tx: Transaction = bitcoin::consensus::deserialize(&tx_bytes).unwrap();
        frostdao::nostr::TxBroadcastEvent {
            txid: tx.compute_txid().to_string(),
            raw_tx,
            network: "Testnet3".to_string(),
        }
    }

    fn valid_transaction_hex(value_sats: u64) -> String {
        let tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(),
            }],
            output: vec![TxOut {
                value: Amount::from_sat(value_sats),
                script_pubkey: ScriptBuf::new(),
            }],
        };
        bitcoin::consensus::encode::serialize_hex(&tx)
    }

    #[test]
    fn tui_nostr_tx_proposal_conversion_populates_review() {
        let app = app_with_wallet(Some(test_address(Network::Testnet)));
        let sighash = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
        let build = frostdao::protocol::dkg_tx::BuildTxOutput {
            session_id: "real-session".to_string(),
            sighash: sighash.to_string(),
            unsigned_tx: "02000000000100".to_string(),
            from_address: test_address(Network::Testnet),
            to_address: app.nostr_to_address.clone(),
            amount_sats: 50_000,
            fee_sats: 1_000,
            network: "testnet".to_string(),
            review: frostdao::protocol::dkg_tx::TransactionReview {
                network: "testnet".to_string(),
                source_path: "root key-path".to_string(),
                from_address: test_address(Network::Testnet),
                to_address: app.nostr_to_address.clone(),
                amount_sats: 50_000,
                fee_sats: 1_000,
                fee_rate_sats_vb: 10,
                sighash_fingerprint: frostdao::protocol::dkg_tx::sighash_fingerprint(sighash),
            },
            event_type: "dkg_build_tx".to_string(),
        };

        let proposal = app.nostr_tx_proposal_from_build_output("wallet-test", 1_700_000_000, build);

        assert_eq!(proposal.session_id, "real-session");
        assert_eq!(proposal.wallet_name, "wallet-test");
        assert_eq!(proposal.proposer_index, 2);
        assert_eq!(proposal.to_address, app.nostr_to_address);
        assert_eq!(proposal.amount_sats, 50_000);
        assert_eq!(proposal.fee_rate, 10);
        assert_eq!(proposal.sighash, sighash);
        assert_eq!(proposal.unsigned_tx, "02000000000100");
        assert_eq!(proposal.review.network, "Testnet3");
        assert_eq!(proposal.review.from_address, test_address(Network::Testnet));
        assert_eq!(proposal.review.to_address, app.nostr_to_address);
        assert_eq!(proposal.review.amount_sats, 50_000);
        assert_eq!(proposal.review.fee_rate_sats_vb, 10);
        assert_eq!(
            proposal.review.sighash_fingerprint,
            "001122334455...aabbccddeeff"
        );
    }

    #[test]
    fn tui_nostr_tx_proposal_builder_rejects_invalid_drafts() {
        let mut app = app_with_wallet(Some(test_address(Network::Testnet)));

        app.nostr_to_address = "   ".to_string();
        assert!(app
            .build_nostr_tx_proposal("wallet-test", 1_700_000_000)
            .unwrap_err()
            .to_string()
            .contains("recipient address is required"));

        app.nostr_to_address = test_address(Network::Testnet);
        app.nostr_amount_sats = 0;
        assert!(app
            .build_nostr_tx_proposal("wallet-test", 1_700_000_000)
            .unwrap_err()
            .to_string()
            .contains("amount must be greater than zero"));

        app.nostr_amount_sats = 50_000;
        app.nostr_to_address = test_address(Network::Bitcoin);
        assert!(app
            .build_nostr_tx_proposal("wallet-test", 1_700_000_000)
            .unwrap_err()
            .to_string()
            .contains("invalid for Testnet3"));

        app.nostr_to_address = test_address(Network::Testnet);
        assert!(app
            .build_nostr_tx_proposal("wallet-missing", 1_700_000_000)
            .unwrap_err()
            .to_string()
            .contains("is not loaded"));

        let app = app_with_wallet(None);
        assert!(app
            .build_nostr_tx_proposal("wallet-test", 1_700_000_000)
            .unwrap_err()
            .to_string()
            .contains("has no known Testnet3 source address"));
    }

    #[test]
    #[serial]
    fn tui_nostr_tx_proposal_requires_regtest_api_configuration() {
        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);
        let mut app = app_with_wallet(Some(test_address(Network::Regtest)));
        app.network = NetworkSelection::Regtest;
        app.nostr_to_address = test_address(Network::Regtest);
        app.nostr_amount_sats = 50_000;

        let err = app
            .build_nostr_tx_proposal("wallet-test", 1_700_000_000)
            .unwrap_err()
            .to_string();

        assert!(err.contains("need a UTXO API on Regtest"));
        assert!(err.contains("local Esplora/mempool API"));
        assert!(err.contains(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV));
    }

    #[test]
    #[serial]
    fn tui_nostr_tx_proposal_allows_configured_regtest_api() {
        std::env::set_var(
            frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV,
            "http://127.0.0.1:3002/api/",
        );
        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Regtest;

        assert!(app.ensure_nostr_proposal_network_available().is_ok());

        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);
    }

    #[test]
    fn tui_nostr_source_uses_selected_hd_path_when_enabled() {
        let mut app = app_with_wallet(Some(test_address(Network::Testnet)));
        app.send_form.hd_enabled = true;
        app.send_form.use_hd_address = true;
        app.send_form.hd_selected_index = 0;
        app.send_form.hd_addresses =
            vec![("tb1pagentderived".to_string(), "pubkey".to_string(), 42)];

        assert_eq!(app.nostr_source_derivation_path(), Some((0, 42)));

        app.send_form.use_hd_address = false;
        assert_eq!(app.nostr_source_derivation_path(), None);
    }

    #[test]
    fn tui_nostr_room_uses_runtime_and_replay_cache() {
        let mut app = App::new().unwrap();
        app.nostr_room_id = format!("tui-runtime-test-{}", std::process::id());
        app.nostr_my_index = 1;
        app.nostr_threshold = 2;
        app.nostr_n_parties = 2;
        let cache_path = app.nostr_replay_cache_path();
        let _ = std::fs::remove_file(&cache_path);

        app.nostr_participants.insert(9, "stale-party".to_string());
        app.nostr_pending_proposals
            .insert("stale-session".to_string(), TxProposal::default());
        app.nostr_received_nonces.insert(
            "stale-session".to_string(),
            std::collections::HashMap::new(),
        );
        app.nostr_received_shares.insert(
            "stale-session".to_string(),
            std::collections::HashMap::new(),
        );
        app.nostr_broadcasts.insert(
            "stale-session".to_string(),
            frostdao::nostr::TxBroadcastEvent {
                txid: "stale-txid".to_string(),
                raw_tx: "stale-raw-tx".to_string(),
                network: "Testnet3".to_string(),
            },
        );
        app.nostr_keygen_state = NostrKeygenState::Finalizing;
        app.nostr_sign_state = NostrSignState::Complete {
            txid: "stale-txid".to_string(),
        };

        app.join_nostr_room_runtime_with_relays(Vec::new()).unwrap();
        assert!(app.nostr_connected);
        assert!(app.nostr_runtime.is_some());
        assert_eq!(app.nostr_transport_label(), "local simulation");
        assert!(app.nostr_local_simulation_transport_active());
        assert_eq!(app.nostr_participants.get(&1).unwrap(), "tui-party-1");
        assert!(!app.nostr_participants.contains_key(&9));
        assert!(app.nostr_pending_proposals.is_empty());
        assert!(app.nostr_received_nonces.is_empty());
        assert!(app.nostr_received_shares.is_empty());
        assert!(app.nostr_broadcasts.is_empty());
        assert!(matches!(
            app.nostr_keygen_state,
            NostrKeygenState::ModeSelect
        ));
        assert!(matches!(app.nostr_sign_state, NostrSignState::SelectWallet));
        assert!(cache_path.exists());

        app.simulate_nostr_participant_join(2).unwrap();
        assert_eq!(app.nostr_participants.get(&2).unwrap(), "npub-local-sim-2");
        app.nostr_pending_proposals
            .insert("active-session".to_string(), TxProposal::default());
        app.nostr_received_nonces.insert(
            "active-session".to_string(),
            std::collections::HashMap::new(),
        );
        app.nostr_received_shares.insert(
            "active-session".to_string(),
            std::collections::HashMap::new(),
        );
        app.nostr_broadcasts.insert(
            "active-session".to_string(),
            frostdao::nostr::TxBroadcastEvent {
                txid: "active-txid".to_string(),
                raw_tx: "active-raw-tx".to_string(),
                network: "Testnet3".to_string(),
            },
        );
        app.nostr_keygen_state = NostrKeygenState::Finalizing;
        app.nostr_sign_state = NostrSignState::Complete {
            txid: "active-txid".to_string(),
        };

        app.leave_nostr_room_runtime();
        assert!(!app.nostr_connected);
        assert!(app.nostr_runtime.is_none());
        assert!(app.nostr_participants.is_empty());
        assert!(app.nostr_pending_proposals.is_empty());
        assert!(app.nostr_received_nonces.is_empty());
        assert!(app.nostr_received_shares.is_empty());
        assert!(app.nostr_broadcasts.is_empty());
        assert!(matches!(
            app.nostr_keygen_state,
            NostrKeygenState::ModeSelect
        ));
        assert!(matches!(app.nostr_sign_state, NostrSignState::SelectWallet));

        let _ = std::fs::remove_file(&cache_path);
    }

    #[test]
    fn tui_nostr_room_rejects_malformed_join_payloads() {
        let mut app = App::new().unwrap();
        app.nostr_room_id = format!("tui-join-validation-test-{}", std::process::id());
        app.nostr_my_index = 1;
        app.nostr_threshold = 2;
        app.nostr_n_parties = 3;
        let cache_path = app.nostr_replay_cache_path();
        let _ = std::fs::remove_file(&cache_path);

        app.join_nostr_room_runtime_with_relays(Vec::new()).unwrap();

        let mismatched_party = frostdao::nostr::RoomJoinPayload {
            party_index: 3,
            nostr_pubkey: "npub-claimed-party-3".to_string(),
            threshold: 2,
            n_parties: 3,
            scheme: frostdao::nostr::ThresholdScheme::Tss,
            rank: None,
        };
        let mismatched_message = frostdao::nostr::NostrProtocolMessage::new_at(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::RoomJoin,
            2,
            &mismatched_party,
            1_700_001_000,
        )
        .unwrap()
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(mismatched_message)
            .unwrap();

        let wrong_threshold = frostdao::nostr::RoomJoinPayload {
            party_index: 2,
            nostr_pubkey: "npub-wrong-threshold".to_string(),
            threshold: 1,
            n_parties: 3,
            scheme: frostdao::nostr::ThresholdScheme::Tss,
            rank: None,
        };
        let wrong_threshold_message = frostdao::nostr::NostrProtocolMessage::new_at(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::RoomJoin,
            2,
            &wrong_threshold,
            1_700_001_001,
        )
        .unwrap()
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(wrong_threshold_message)
            .unwrap();

        let htss_join = frostdao::nostr::RoomJoinPayload {
            party_index: 2,
            nostr_pubkey: "npub-htss".to_string(),
            threshold: 2,
            n_parties: 3,
            scheme: frostdao::nostr::ThresholdScheme::Htss,
            rank: Some(0),
        };
        let htss_message = frostdao::nostr::NostrProtocolMessage::new_at(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::RoomJoin,
            2,
            &htss_join,
            1_700_001_002,
        )
        .unwrap()
        .with_htss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(htss_message)
            .unwrap();

        app.poll_nostr_room_runtime().unwrap();
        assert!(!app.nostr_participants.contains_key(&2));
        assert!(!app.nostr_participants.contains_key(&3));

        app.simulate_nostr_participant_join(2).unwrap();
        assert_eq!(app.nostr_participants.get(&2).unwrap(), "npub-local-sim-2");

        let _ = std::fs::remove_file(&cache_path);
    }

    #[test]
    fn tui_nostr_signing_publishes_runtime_messages() {
        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Testnet3;
        app.nostr_room_id = format!("tui-signing-runtime-test-{}", std::process::id());
        app.nostr_my_index = 1;
        app.nostr_threshold = 2;
        app.nostr_n_parties = 2;
        let cache_path = app.nostr_replay_cache_path();
        let _ = std::fs::remove_file(&cache_path);

        app.join_nostr_room_runtime_with_relays(Vec::new()).unwrap();
        let proposal = crate::tui::state::TxProposal {
            session_id: "session-test".to_string(),
            wallet_name: "wallet-test".to_string(),
            proposer_index: 1,
            to_address: "tb1qrecipient".to_string(),
            amount_sats: 50_000,
            fee_rate: 10,
            sighash: "abc123".to_string(),
            unsigned_tx: valid_transaction_hex(2_000),
            review: frostdao::nostr::TxReviewPayload {
                network: "testnet".to_string(),
                source_path: "m/86'/1'/0'/0/0".to_string(),
                from_address: "tb1qfrom".to_string(),
                to_address: "tb1qrecipient".to_string(),
                amount_sats: 50_000,
                fee_rate_sats_vb: 10,
                sighash_fingerprint: "abc12345".to_string(),
            },
            description: "test proposal".to_string(),
            timestamp: 1_700_000_000,
        };

        app.publish_nostr_tx_proposal("wallet-test", &proposal)
            .unwrap();
        assert_eq!(app.audit_events[0].event, "nostr_tx_proposal");
        assert_eq!(app.audit_events[0].wallet, "wallet-test");
        assert_eq!(app.audit_events[0].fields["session_id"], "session-test");
        assert_eq!(
            app.audit_events[0].fields["sighash_fingerprint"],
            "abc12345"
        );
        assert!(app.audit_events[0].fields.get("sighash").is_none());
        assert_eq!(
            app.nostr_runtime
                .as_ref()
                .unwrap()
                .local_simulation_room_len(&app.nostr_room_id),
            Some(2)
        );

        app.publish_nostr_tx_consent("wallet-test", &proposal, true, None)
            .unwrap();
        assert_eq!(app.audit_events[1].event, "nostr_tx_consent");
        assert_eq!(app.audit_events[1].status, "consented");
        assert_eq!(app.audit_events[1].fields["session_id"], "session-test");
        assert!(app.audit_events[1].fields.get("sighash").is_none());
        assert_eq!(
            app.nostr_runtime
                .as_ref()
                .unwrap()
                .local_simulation_room_len(&app.nostr_room_id),
            Some(3)
        );

        app.simulate_nostr_participant_join(2).unwrap();
        app.publish_nostr_signing_nonce(
            "wallet-test",
            "session-test",
            2,
            "encrypted-nonce".to_string(),
        )
        .unwrap();
        assert_eq!(app.audit_events[2].event, "nostr_signing_nonce");
        assert_eq!(app.audit_events[2].fields["to_index"], 2);
        assert!(app.audit_events[2].fields.get("ciphertext").is_none());

        app.publish_nostr_signing_share(
            "wallet-test",
            "session-test",
            2,
            "encrypted-share".to_string(),
        )
        .unwrap();
        assert_eq!(app.audit_events[3].event, "nostr_signing_share");
        assert_eq!(app.audit_events[3].fields["to_index"], 2);
        assert!(app.audit_events[3].fields.get("ciphertext").is_none());
        let broadcast_event = valid_broadcast_event_with_value(2_000);
        app.publish_nostr_tx_broadcast(
            "wallet-test",
            "session-test",
            broadcast_event.txid.clone(),
            broadcast_event.raw_tx.clone(),
            broadcast_event.network.clone(),
        )
        .unwrap();
        assert_eq!(app.audit_events[4].event, "nostr_tx_broadcast");
        assert_eq!(app.audit_events[4].fields["txid"], broadcast_event.txid);
        assert!(app.audit_events[4].fields.get("raw_tx").is_none());
        assert_eq!(
            app.nostr_runtime
                .as_ref()
                .unwrap()
                .local_simulation_room_len(&app.nostr_room_id),
            Some(7)
        );

        let _ = std::fs::remove_file(&cache_path);
    }

    #[test]
    fn tui_nostr_signing_rejects_malformed_outbound_messages() {
        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Testnet3;
        app.nostr_room_id = format!("tui-outbound-validation-test-{}", std::process::id());
        app.nostr_my_index = 1;
        app.nostr_threshold = 2;
        app.nostr_n_parties = 2;
        let cache_path = app.nostr_replay_cache_path();
        let _ = std::fs::remove_file(&cache_path);

        app.join_nostr_room_runtime_with_relays(Vec::new()).unwrap();

        assert!(app
            .publish_nostr_signing_nonce("wallet-test", "session-test", 0, "cipher".to_string())
            .unwrap_err()
            .to_string()
            .contains("recipient party"));
        assert!(app
            .publish_nostr_signing_nonce("wallet-test", "session-test", 1, "cipher".to_string())
            .unwrap_err()
            .to_string()
            .contains("another party"));
        assert!(app
            .publish_nostr_signing_nonce("wallet-test", "session-test", 2, "cipher".to_string())
            .unwrap_err()
            .to_string()
            .contains("recipient party has not joined"));
        app.simulate_nostr_participant_join(2).unwrap();
        assert!(app
            .publish_nostr_signing_share("wallet-test", "", 2, "cipher".to_string())
            .unwrap_err()
            .to_string()
            .contains("signing session"));
        assert!(app
            .publish_nostr_signing_share("wallet-test", "session-test", 2, "   ".to_string())
            .unwrap_err()
            .to_string()
            .contains("encrypted payload"));

        let broadcast = valid_broadcast_event_with_value(2_001);
        assert!(app
            .publish_nostr_tx_broadcast(
                "wallet-test",
                "session-test",
                broadcast.txid.clone(),
                broadcast.raw_tx.clone(),
                "Mainnet".to_string(),
            )
            .unwrap_err()
            .to_string()
            .contains("selected TUI network"));
        assert!(app
            .publish_nostr_tx_broadcast(
                "wallet-test",
                "session-test",
                "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                broadcast.raw_tx,
                broadcast.network,
            )
            .unwrap_err()
            .to_string()
            .contains("txid does not match"));

        assert!(app.audit_events.is_empty());
        assert_eq!(
            app.nostr_runtime
                .as_ref()
                .unwrap()
                .local_simulation_room_len(&app.nostr_room_id),
            Some(2)
        );

        let _ = std::fs::remove_file(&cache_path);
    }

    #[test]
    fn tui_nostr_relay_mode_is_guarded_on_mainnet() {
        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Mainnet;
        app.nostr_room_id = format!("tui-mainnet-relay-guard-test-{}", std::process::id());
        app.nostr_my_index = 1;
        app.nostr_threshold = 2;
        app.nostr_n_parties = 2;

        let err = app
            .join_nostr_room_runtime_with_relays(vec!["wss://relay.example.invalid".to_string()])
            .unwrap_err();
        assert!(err.to_string().contains("mainnet relay rooms require"));
    }

    #[test]
    fn tui_nostr_poll_rejects_sessionless_signing_messages() {
        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Testnet3;
        app.nostr_room_id = format!("tui-session-required-test-{}", std::process::id());
        app.nostr_my_index = 1;
        app.nostr_threshold = 2;
        app.nostr_n_parties = 2;
        let cache_path = app.nostr_replay_cache_path();
        let _ = std::fs::remove_file(&cache_path);

        app.join_nostr_room_runtime_with_relays(Vec::new()).unwrap();

        let proposal_event = valid_remote_proposal_event(2);
        let walletless_proposal = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxProposal,
            2,
            &proposal_event,
        )
        .unwrap()
        .with_session("session-walletless")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(walletless_proposal)
            .unwrap();
        let mut other_wallet_proposal_event = proposal_event.clone();
        other_wallet_proposal_event.timestamp += 1;
        let other_wallet_proposal = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxProposal,
            2,
            &other_wallet_proposal_event,
        )
        .unwrap()
        .with_wallet("wallet-other")
        .with_session("session-other-wallet")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(other_wallet_proposal)
            .unwrap();
        app.poll_nostr_room_runtime().unwrap();
        assert!(!app
            .nostr_pending_proposals
            .contains_key("session-walletless"));
        assert_eq!(
            app.nostr_pending_proposals
                .get("session-other-wallet")
                .unwrap()
                .wallet_name,
            "wallet-other"
        );
        assert_eq!(
            app.nostr_pending_proposals
                .values()
                .filter(|proposal| proposal.wallet_name == "wallet-test")
                .count(),
            0
        );

        let proposal = crate::tui::state::TxProposal {
            session_id: "session-expected".to_string(),
            wallet_name: "wallet-test".to_string(),
            proposer_index: 2,
            to_address: proposal_event.to_address.clone(),
            amount_sats: proposal_event.amount_sats,
            fee_rate: proposal_event.fee_rate,
            sighash: proposal_event.sighash.clone(),
            unsigned_tx: proposal_event.unsigned_tx.clone(),
            review: proposal_event.review.clone(),
            description: "remote proposal".to_string(),
            timestamp: 1_700_000_010,
        };
        app.nostr_sign_state = NostrSignState::WaitingForConsent {
            wallet_name: "wallet-test".to_string(),
            session_id: proposal.session_id.clone(),
            proposal: proposal.clone(),
            consents: std::collections::HashMap::new(),
            rejections: std::collections::HashMap::new(),
        };

        let consent_event = frostdao::nostr::TxConsentEvent {
            proposal_session: proposal.session_id.clone(),
            consent: true,
            reviewed_sighash_fingerprint: proposal.review.sighash_fingerprint.clone(),
            reason: None,
        };
        let sessionless_consent = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxConsent,
            2,
            &consent_event,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(sessionless_consent)
            .unwrap();

        let mismatched_consent = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxConsent,
            2,
            &consent_event,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-other")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(mismatched_consent)
            .unwrap();
        let wrong_wallet_consent = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxConsent,
            2,
            &consent_event,
        )
        .unwrap()
        .with_wallet("wallet-other")
        .with_session("session-expected")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(wrong_wallet_consent)
            .unwrap();
        let wrong_fingerprint_consent_event = frostdao::nostr::TxConsentEvent {
            proposal_session: proposal.session_id.clone(),
            consent: true,
            reviewed_sighash_fingerprint: "wrong-fingerprint".to_string(),
            reason: None,
        };
        let wrong_fingerprint_consent = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxConsent,
            2,
            &wrong_fingerprint_consent_event,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-expected")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(wrong_fingerprint_consent)
            .unwrap();
        let out_of_room_consent = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxConsent,
            4,
            &consent_event,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-expected")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(out_of_room_consent)
            .unwrap();
        app.poll_nostr_room_runtime().unwrap();
        if let NostrSignState::WaitingForConsent {
            consents,
            rejections,
            ..
        } = &app.nostr_sign_state
        {
            assert!(consents.is_empty());
            assert!(rejections.is_empty());
        } else {
            panic!("expected WaitingForConsent");
        }

        let nonce_event =
            frostdao::nostr::SigningNonceEvent::new(2, 1, "encrypted-nonce".to_string());
        let sessionless_nonce = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::SigningNonceEncrypted,
            2,
            &nonce_event,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_tss()
        .to_party(1)
        .unwrap();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(sessionless_nonce)
            .unwrap();
        app.poll_nostr_room_runtime().unwrap();
        assert!(app.nostr_received_nonces.is_empty());
        assert!(!app.nostr_received_nonces.contains_key("unknown"));

        app.nostr_sign_state = NostrSignState::CollectingShares {
            wallet_name: "wallet-test".to_string(),
            session_id: proposal.session_id.clone(),
            received_shares: std::collections::HashMap::new(),
        };
        let share_event =
            frostdao::nostr::SigningShareEvent::new(2, 1, "encrypted-share".to_string());
        let sessionless_share = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::SigningShareEncrypted,
            2,
            &share_event,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_tss()
        .to_party(1)
        .unwrap();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(sessionless_share)
            .unwrap();
        app.poll_nostr_room_runtime().unwrap();
        assert!(app.nostr_received_shares.is_empty());
        if let NostrSignState::CollectingShares {
            received_shares, ..
        } = &app.nostr_sign_state
        {
            assert!(received_shares.is_empty());
        } else {
            panic!("expected CollectingShares");
        }

        app.nostr_sign_state = NostrSignState::Combining {
            wallet_name: "wallet-test".to_string(),
            session_id: proposal.session_id.clone(),
        };
        let broadcast_event = frostdao::nostr::TxBroadcastEvent {
            txid: "txid-sessionless".to_string(),
            raw_tx: "raw-remote-transaction".to_string(),
            network: "testnet".to_string(),
        };
        let sessionless_broadcast = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxBroadcast,
            2,
            &broadcast_event,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(sessionless_broadcast)
            .unwrap();
        app.poll_nostr_room_runtime().unwrap();
        assert!(app.nostr_broadcasts.is_empty());
        assert!(matches!(
            &app.nostr_sign_state,
            NostrSignState::Combining { session_id, .. } if session_id == "session-expected"
        ));

        let _ = std::fs::remove_file(&cache_path);
    }

    #[test]
    fn tui_nostr_poll_rejects_malformed_tx_proposals() {
        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Testnet3;
        app.nostr_room_id = format!("tui-proposal-validation-test-{}", std::process::id());
        app.nostr_my_index = 1;
        app.nostr_threshold = 2;
        app.nostr_n_parties = 3;
        let cache_path = app.nostr_replay_cache_path();
        let _ = std::fs::remove_file(&cache_path);

        app.join_nostr_room_runtime_with_relays(Vec::new()).unwrap();

        let mismatched_proposer = valid_remote_proposal_event(3);
        let mismatched_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxProposal,
            2,
            &mismatched_proposer,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-mismatched-proposer")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(mismatched_message)
            .unwrap();

        let mut zero_amount = valid_remote_proposal_event(2);
        zero_amount.amount_sats = 0;
        zero_amount.review.amount_sats = 0;
        let zero_amount_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxProposal,
            2,
            &zero_amount,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-zero-amount")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(zero_amount_message)
            .unwrap();

        let mut wrong_network = valid_remote_proposal_event(2);
        wrong_network.review.network = "Mainnet".to_string();
        let wrong_network_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxProposal,
            2,
            &wrong_network,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-wrong-network")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(wrong_network_message)
            .unwrap();

        let mut wrong_fingerprint = valid_remote_proposal_event(2);
        wrong_fingerprint.review.sighash_fingerprint = "wrong-fingerprint".to_string();
        let wrong_fingerprint_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxProposal,
            2,
            &wrong_fingerprint,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-wrong-fingerprint")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(wrong_fingerprint_message)
            .unwrap();

        let mut missing_unsigned_tx = valid_remote_proposal_event(2);
        missing_unsigned_tx.unsigned_tx.clear();
        let missing_unsigned_tx_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxProposal,
            2,
            &missing_unsigned_tx,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-missing-unsigned-tx")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(missing_unsigned_tx_message)
            .unwrap();

        let mut invalid_unsigned_tx = valid_remote_proposal_event(2);
        invalid_unsigned_tx.unsigned_tx = "not-hex".to_string();
        let invalid_unsigned_tx_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxProposal,
            2,
            &invalid_unsigned_tx,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-invalid-unsigned-tx")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(invalid_unsigned_tx_message)
            .unwrap();

        let valid = valid_remote_proposal_event(2);
        let valid_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxProposal,
            2,
            &valid,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-valid")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(valid_message)
            .unwrap();

        app.poll_nostr_room_runtime().unwrap();
        assert!(!app
            .nostr_pending_proposals
            .contains_key("session-mismatched-proposer"));
        assert!(!app
            .nostr_pending_proposals
            .contains_key("session-zero-amount"));
        assert!(!app
            .nostr_pending_proposals
            .contains_key("session-wrong-network"));
        assert!(!app
            .nostr_pending_proposals
            .contains_key("session-wrong-fingerprint"));
        assert!(!app
            .nostr_pending_proposals
            .contains_key("session-missing-unsigned-tx"));
        assert!(!app
            .nostr_pending_proposals
            .contains_key("session-invalid-unsigned-tx"));
        assert_eq!(
            app.nostr_pending_proposals
                .get("session-valid")
                .unwrap()
                .review
                .sighash_fingerprint,
            frostdao::protocol::dkg_tx::sighash_fingerprint("remote-sighash")
        );
        assert!(!app
            .nostr_pending_proposals
            .get("session-valid")
            .unwrap()
            .unsigned_tx
            .is_empty());

        let _ = std::fs::remove_file(&cache_path);
    }

    #[test]
    fn tui_nostr_poll_rejects_mismatched_ciphertext_payload_parties() {
        let mut app = App::new().unwrap();
        app.nostr_room_id = format!("tui-ciphertext-binding-test-{}", std::process::id());
        app.nostr_my_index = 1;
        app.nostr_threshold = 2;
        app.nostr_n_parties = 3;
        let cache_path = app.nostr_replay_cache_path();
        let _ = std::fs::remove_file(&cache_path);

        app.join_nostr_room_runtime_with_relays(Vec::new()).unwrap();

        let mismatched_nonce =
            frostdao::nostr::SigningNonceEvent::new(3, 1, "bad-nonce".to_string());
        let mismatched_nonce_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::SigningNonceEncrypted,
            2,
            &mismatched_nonce,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-ciphertext")
        .with_tss()
        .to_party(1)
        .unwrap();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(mismatched_nonce_message)
            .unwrap();

        let wrong_payload_recipient =
            frostdao::nostr::SigningNonceEvent::new(2, 3, "wrong-recipient".to_string());
        let wrong_payload_recipient_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::SigningNonceEncrypted,
            2,
            &wrong_payload_recipient,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-ciphertext")
        .with_tss()
        .to_party(1)
        .unwrap();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(wrong_payload_recipient_message)
            .unwrap();

        let out_of_room_share =
            frostdao::nostr::SigningShareEvent::new(4, 1, "out-of-room-share".to_string());
        let out_of_room_share_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::SigningShareEncrypted,
            4,
            &out_of_room_share,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-ciphertext")
        .with_tss()
        .to_party(1)
        .unwrap();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(out_of_room_share_message)
            .unwrap();

        app.nostr_sign_state = NostrSignState::CollectingShares {
            wallet_name: "wallet-test".to_string(),
            session_id: "session-ciphertext".to_string(),
            received_shares: std::collections::HashMap::new(),
        };
        app.start_nostr_signing_attempt("wallet-test", "session-ciphertext", "fingerprint-test")
            .unwrap();
        app.poll_nostr_room_runtime().unwrap();
        assert!(app.nostr_received_nonces.is_empty());
        assert!(app.nostr_received_shares.is_empty());
        if let NostrSignState::CollectingShares {
            received_shares, ..
        } = &app.nostr_sign_state
        {
            assert!(received_shares.is_empty());
        } else {
            panic!("expected CollectingShares");
        }

        let early_share = frostdao::nostr::SigningShareEvent::new(2, 1, "early-share".to_string());
        let early_share_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::SigningShareEncrypted,
            2,
            &early_share,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-ciphertext")
        .with_tss()
        .to_party(1)
        .unwrap();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(early_share_message)
            .unwrap();
        app.poll_nostr_room_runtime().unwrap();
        assert!(app.nostr_received_shares.is_empty());
        if let NostrSignState::CollectingShares {
            received_shares, ..
        } = &app.nostr_sign_state
        {
            assert!(received_shares.is_empty());
        } else {
            panic!("expected CollectingShares");
        }

        let valid_nonce = frostdao::nostr::SigningNonceEvent::new(2, 1, "good-nonce".to_string());
        let valid_nonce_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::SigningNonceEncrypted,
            2,
            &valid_nonce,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-ciphertext")
        .with_tss()
        .to_party(1)
        .unwrap();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(valid_nonce_message)
            .unwrap();

        let second_valid_nonce =
            frostdao::nostr::SigningNonceEvent::new(3, 1, "good-nonce-3".to_string());
        let second_valid_nonce_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::SigningNonceEncrypted,
            3,
            &second_valid_nonce,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-ciphertext")
        .with_tss()
        .to_party(1)
        .unwrap();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(second_valid_nonce_message)
            .unwrap();

        let valid_share = frostdao::nostr::SigningShareEvent::new(2, 1, "good-share".to_string());
        let valid_share_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::SigningShareEncrypted,
            2,
            &valid_share,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-ciphertext")
        .with_tss()
        .to_party(1)
        .unwrap();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(valid_share_message)
            .unwrap();

        app.poll_nostr_room_runtime().unwrap();
        assert_eq!(
            app.nostr_received_nonces
                .get("session-ciphertext")
                .unwrap()
                .get(&2)
                .unwrap(),
            "good-nonce"
        );
        assert_eq!(
            app.nostr_received_shares
                .get("session-ciphertext")
                .unwrap()
                .get(&2)
                .unwrap(),
            "good-share"
        );
        if let NostrSignState::CollectingShares {
            received_shares, ..
        } = &app.nostr_sign_state
        {
            assert_eq!(received_shares.get(&2).unwrap(), "good-share");
        } else {
            panic!("expected CollectingShares");
        }

        let _ = std::fs::remove_file(&cache_path);
    }

    #[test]
    fn tui_nostr_signing_attempt_replays_prior_session_nonces() {
        let mut app = App::new().unwrap();
        app.nostr_threshold = 2;
        app.nostr_n_parties = 3;
        app.nostr_received_nonces.insert(
            "session-replay".to_string(),
            HashMap::from([
                (2, "nonce-before-start-2".to_string()),
                (3, "nonce-before-start-3".to_string()),
            ]),
        );

        app.start_nostr_signing_attempt("wallet-test", "session-replay", "fingerprint-replay")
            .unwrap();

        let coordinator = app
            .nostr_signing_coordinators
            .get("session-replay")
            .unwrap();
        assert!(coordinator.ready_to_request_shares());
        assert_eq!(coordinator.collector().nonce_count(), 2);
    }

    #[test]
    fn tui_nostr_poll_ingests_proposals_and_consents() {
        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Testnet3;
        app.nostr_room_id = format!("tui-ingest-runtime-test-{}", std::process::id());
        app.nostr_my_index = 1;
        app.nostr_threshold = 2;
        app.nostr_n_parties = 2;
        let cache_path = app.nostr_replay_cache_path();
        let _ = std::fs::remove_file(&cache_path);

        app.join_nostr_room_runtime_with_relays(Vec::new()).unwrap();

        let proposal_event = valid_remote_proposal_event(2);
        let proposal_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxProposal,
            2,
            &proposal_event,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-remote")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(proposal_message)
            .unwrap();

        app.poll_nostr_room_runtime().unwrap();
        let pending = app.nostr_pending_proposals.get("session-remote").unwrap();
        assert_eq!(pending.proposer_index, 2);
        assert_eq!(pending.amount_sats, 25_000);
        assert_eq!(
            pending.review.sighash_fingerprint,
            frostdao::protocol::dkg_tx::sighash_fingerprint("remote-sighash")
        );
        let pending_fingerprint = pending.review.sighash_fingerprint.clone();

        app.nostr_sign_state = NostrSignState::WaitingForConsent {
            wallet_name: "wallet-test".to_string(),
            session_id: "session-remote".to_string(),
            proposal: pending.clone(),
            consents: std::collections::HashMap::new(),
            rejections: std::collections::HashMap::new(),
        };
        let consent_event = frostdao::nostr::TxConsentEvent {
            proposal_session: "session-remote".to_string(),
            consent: true,
            reviewed_sighash_fingerprint: pending.review.sighash_fingerprint.clone(),
            reason: None,
        };
        let consent_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxConsent,
            2,
            &consent_event,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-remote")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(consent_message)
            .unwrap();

        app.poll_nostr_room_runtime().unwrap();
        if let NostrSignState::WaitingForConsent { consents, .. } = &app.nostr_sign_state {
            assert_eq!(
                consents.get(&2).unwrap(),
                &frostdao::protocol::dkg_tx::sighash_fingerprint("remote-sighash")
            );
        } else {
            panic!("expected WaitingForConsent");
        }

        let nonce_event =
            frostdao::nostr::SigningNonceEvent::new(2, 1, "encrypted-nonce".to_string());
        let nonce_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::SigningNonceEncrypted,
            2,
            &nonce_event,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-remote")
        .with_tss()
        .to_party(1)
        .unwrap();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(nonce_message)
            .unwrap();
        app.poll_nostr_room_runtime().unwrap();
        assert_eq!(
            app.nostr_received_nonces
                .get("session-remote")
                .unwrap()
                .get(&2)
                .unwrap(),
            "encrypted-nonce"
        );

        app.nostr_sign_state = NostrSignState::CollectingShares {
            wallet_name: "wallet-test".to_string(),
            session_id: "session-remote".to_string(),
            received_shares: std::collections::HashMap::new(),
        };
        app.nostr_received_nonces
            .entry("session-remote".to_string())
            .or_default()
            .insert(1, "local-nonce".to_string());
        app.start_nostr_signing_attempt("wallet-test", "session-remote", &pending_fingerprint)
            .unwrap();
        assert!(app
            .nostr_signing_coordinators
            .get("session-remote")
            .unwrap()
            .ready_to_request_shares());
        let share_event =
            frostdao::nostr::SigningShareEvent::new(2, 1, "encrypted-share".to_string());
        let share_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::SigningShareEncrypted,
            2,
            &share_event,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-remote")
        .with_tss()
        .to_party(1)
        .unwrap();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(share_message)
            .unwrap();
        app.poll_nostr_room_runtime().unwrap();
        assert_eq!(
            app.nostr_received_shares
                .get("session-remote")
                .unwrap()
                .get(&2)
                .unwrap(),
            "encrypted-share"
        );
        if let NostrSignState::CollectingShares {
            received_shares, ..
        } = &app.nostr_sign_state
        {
            assert_eq!(received_shares.get(&2).unwrap(), "encrypted-share");
        } else {
            panic!("expected CollectingShares");
        }

        app.nostr_sign_state = NostrSignState::Combining {
            wallet_name: "wallet-test".to_string(),
            session_id: "session-remote".to_string(),
        };
        let wrong_wallet_broadcast = valid_broadcast_event_with_value(1_001);
        let wrong_wallet_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxBroadcast,
            2,
            &wrong_wallet_broadcast,
        )
        .unwrap()
        .with_wallet("wallet-other")
        .with_session("session-remote")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(wrong_wallet_message)
            .unwrap();

        let mut wrong_network_broadcast = valid_broadcast_event_with_value(1_002);
        wrong_network_broadcast.network = "Mainnet".to_string();
        let wrong_network_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxBroadcast,
            2,
            &wrong_network_broadcast,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-remote")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(wrong_network_message)
            .unwrap();

        let mut wrong_txid_broadcast = valid_broadcast_event_with_value(1_003);
        wrong_txid_broadcast.txid =
            "0000000000000000000000000000000000000000000000000000000000000000".to_string();
        let wrong_txid_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxBroadcast,
            2,
            &wrong_txid_broadcast,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-remote")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(wrong_txid_message)
            .unwrap();

        let mut invalid_raw_broadcast = valid_broadcast_event_with_value(1_004);
        invalid_raw_broadcast.raw_tx = "not-hex".to_string();
        let invalid_raw_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxBroadcast,
            2,
            &invalid_raw_broadcast,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-remote")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(invalid_raw_message)
            .unwrap();

        app.poll_nostr_room_runtime().unwrap();
        assert!(app.nostr_broadcasts.is_empty());
        assert!(matches!(
            &app.nostr_sign_state,
            NostrSignState::Combining { session_id, .. } if session_id == "session-remote"
        ));

        let broadcast_event = valid_broadcast_event_with_value(1_005);
        let broadcast_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxBroadcast,
            2,
            &broadcast_event,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session("session-remote")
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(broadcast_message)
            .unwrap();
        app.poll_nostr_room_runtime().unwrap();
        assert_eq!(
            app.nostr_broadcasts.get("session-remote").unwrap().txid,
            broadcast_event.txid
        );
        assert!(matches!(
            &app.nostr_sign_state,
            NostrSignState::Complete { txid } if txid == &broadcast_event.txid
        ));
        let audit_event_names = app
            .audit_events
            .iter()
            .map(|event| event.event.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            audit_event_names,
            vec![
                "nostr_tx_proposal_received",
                "nostr_tx_consent_received",
                "nostr_signing_nonce_received",
                "nostr_signing_share_received",
                "nostr_tx_broadcast_received",
            ]
        );
        for event in &app.audit_events {
            assert!(event.fields.get("ciphertext").is_none());
            assert!(event.fields.get("raw_tx").is_none());
            assert!(event.fields.get("sighash").is_none());
        }

        let _ = std::fs::remove_file(&cache_path);
    }

    #[test]
    fn tui_nostr_poll_tracks_rejected_proposal_consents() {
        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Testnet3;
        app.nostr_room_id = format!("tui-consent-reject-test-{}", std::process::id());
        app.nostr_my_index = 1;
        app.nostr_threshold = 2;
        app.nostr_n_parties = 3;
        let cache_path = app.nostr_replay_cache_path();
        let _ = std::fs::remove_file(&cache_path);

        app.join_nostr_room_runtime_with_relays(Vec::new()).unwrap();
        let proposal_event = valid_remote_proposal_event(2);
        let proposal = crate::tui::state::TxProposal {
            session_id: "session-rejected".to_string(),
            wallet_name: "wallet-test".to_string(),
            proposer_index: 1,
            to_address: proposal_event.to_address.clone(),
            amount_sats: proposal_event.amount_sats,
            fee_rate: proposal_event.fee_rate,
            sighash: proposal_event.sighash.clone(),
            unsigned_tx: proposal_event.unsigned_tx.clone(),
            review: proposal_event.review.clone(),
            description: "proposal with rejection".to_string(),
            timestamp: 1_700_000_010,
        };
        app.nostr_sign_state = NostrSignState::WaitingForConsent {
            wallet_name: "wallet-test".to_string(),
            session_id: proposal.session_id.clone(),
            proposal: proposal.clone(),
            consents: std::collections::HashMap::new(),
            rejections: std::collections::HashMap::new(),
        };

        let rejection_event = frostdao::nostr::TxConsentEvent {
            proposal_session: proposal.session_id.clone(),
            consent: false,
            reviewed_sighash_fingerprint: proposal.review.sighash_fingerprint.clone(),
            reason: Some("amount mismatch".to_string()),
        };
        let rejection_message = frostdao::nostr::NostrProtocolMessage::new(
            app.nostr_room_id.clone(),
            frostdao::nostr::NostrMessageKind::TxConsent,
            2,
            &rejection_event,
        )
        .unwrap()
        .with_wallet("wallet-test")
        .with_session(proposal.session_id.clone())
        .with_tss();
        app.nostr_runtime
            .as_mut()
            .unwrap()
            .publish_local_simulation_message(rejection_message)
            .unwrap();

        app.poll_nostr_room_runtime().unwrap();

        if let NostrSignState::WaitingForConsent {
            consents,
            rejections,
            ..
        } = &app.nostr_sign_state
        {
            assert!(consents.is_empty());
            assert_eq!(
                rejections.get(&2).map(String::as_str),
                Some("amount mismatch")
            );
        } else {
            panic!("expected WaitingForConsent");
        }
        assert_eq!(app.audit_events[0].event, "nostr_tx_consent_received");
        assert_eq!(app.audit_events[0].status, "rejected");
        assert_eq!(app.audit_events[0].fields["party_index"], 2);

        let _ = std::fs::remove_file(&cache_path);
    }

    #[test]
    fn nostr_room_config_validation_blocks_invalid_ceremony_shape() {
        let mut app = App::new().unwrap();
        app.nostr_room_id.clear();
        assert_eq!(app.nostr_room_config_error(), Some("Enter a room ID first"));

        app.nostr_room_id = "treasury-room".to_string();
        app.nostr_n_parties = 1;
        assert_eq!(
            app.nostr_room_config_error(),
            Some("Parties must be at least 2")
        );

        app.nostr_n_parties = 3;
        app.nostr_my_index = 4;
        assert_eq!(
            app.nostr_room_config_error(),
            Some("My Index must be between 1 and Parties")
        );

        app.nostr_my_index = 2;
        app.nostr_threshold = 4;
        assert_eq!(
            app.nostr_room_config_error(),
            Some("Threshold must be between 1 and Parties")
        );

        app.nostr_threshold = 2;
        assert_eq!(app.nostr_room_config_error(), None);
    }
}
