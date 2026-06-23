//! TUI application state and logic

use anyhow::Result;
use bitcoin::{Address, XOnlyPublicKey};
use ratatui::widgets::ListState;
use std::collections::HashMap;
use std::path::PathBuf;

#[cfg(feature = "miniscript-policy")]
use crate::tui::screens::PolicyPreviewFormData;
use crate::tui::screens::{KeygenFormData, ReshareFormData, SendFormData};
use crate::tui::state::{
    AppState, NetworkSelection, NostrKeygenState, NostrRoomField, NostrRoomPhase, NostrSignState,
    TxProposal,
};
use frostdao::nostr::RoomMessageTransport;
use frostdao::protocol::keygen::{list_wallets, WalletSummary};
use frostdao::storage::{FileStorage, Storage};

const TUI_NOSTR_RELAYS_ENV: &str = "FROSTDAO_TUI_NOSTR_RELAYS";
const TUI_MAINNET_NOSTR_ENV: &str = "FROSTDAO_ENABLE_MAINNET_NOSTR";

pub enum TuiNostrRuntime {
    Demo(frostdao::nostr::NostrRoomRuntime<frostdao::nostr::InMemoryRoomTransport>),
    Relay(frostdao::nostr::NostrRoomRuntime<frostdao::nostr::RelayRoomTransport>),
}

impl TuiNostrRuntime {
    fn publish(&mut self, message: frostdao::nostr::NostrProtocolMessage) -> Result<()> {
        match self {
            Self::Demo(runtime) => runtime.publish(message),
            Self::Relay(runtime) => runtime.publish(message),
        }
    }

    fn receive(&mut self, now: u64) -> Result<Vec<frostdao::nostr::NostrProtocolMessage>> {
        match self {
            Self::Demo(runtime) => runtime.receive(now),
            Self::Relay(runtime) => runtime.receive(now),
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Demo(_) => "demo",
            Self::Relay(_) => "relay",
        }
    }

    #[cfg(test)]
    fn demo_room_len(&self, room: &str) -> Option<usize> {
        match self {
            Self::Demo(runtime) => Some(runtime.transport().room_len(room)),
            Self::Relay(_) => None,
        }
    }

    fn publish_demo_message(
        &mut self,
        message: frostdao::nostr::NostrProtocolMessage,
    ) -> Result<()> {
        match self {
            Self::Demo(runtime) => runtime.transport_mut().publish(message),
            Self::Relay(_) => {
                anyhow::bail!("demo participant simulation is unavailable in relay mode")
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
    /// Hardened room runtime for TUI relay flows
    pub nostr_runtime: Option<TuiNostrRuntime>,

    // Nostr DKG/signing state
    /// Current keygen state
    pub nostr_keygen_state: NostrKeygenState,
    /// Current signing state
    pub nostr_sign_state: NostrSignState,

    // Nostr signing transaction data
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
            nostr_runtime: None,
            nostr_keygen_state: NostrKeygenState::ModeSelect,
            nostr_sign_state: NostrSignState::SelectWallet,
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
        })
    }

    /// Fetch UTXOs and recent transactions for send form
    pub fn fetch_utxos_for_send(&mut self, address: &str) {
        use super::screens::{TxDisplay, UtxoDisplay};

        let api_base = self.network.mempool_api_base();
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
        }

        // Fetch UTXOs
        let utxo_url = format!("{}/address/{}/utxo", api_base, address);
        if let Ok(response) = client.get(&utxo_url).send() {
            if let Ok(utxos) = response.json::<Vec<serde_json::Value>>() {
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
            }
        }

        // Fetch recent transactions
        let txs_url = format!("{}/address/{}/txs", api_base, address);
        if let Ok(response) = client.get(&txs_url).send() {
            if let Ok(txs) = response.json::<Vec<serde_json::Value>>() {
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

    /// Join the configured room with explicit relay URLs. Empty relays use demo mode.
    pub fn join_nostr_room_runtime_with_relays(&mut self, relay_urls: Vec<String>) -> Result<()> {
        let cache_path = self.nostr_replay_cache_path();
        let mut runtime = if relay_urls.is_empty() {
            let transport = frostdao::nostr::InMemoryRoomTransport::new();
            TuiNostrRuntime::Demo(frostdao::nostr::NostrRoomRuntime::load(
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
            nostr_pubkey: format!("tui-party-{}", self.nostr_my_index),
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
        self.nostr_runtime = Some(runtime);
        self.nostr_connected = true;
        self.nostr_participants.clear();
        self.poll_nostr_room_runtime()?;
        Ok(())
    }

    pub fn nostr_transport_label(&self) -> String {
        if let Some(runtime) = &self.nostr_runtime {
            return runtime.label().to_string();
        }

        let relays = self.nostr_relay_urls_from_env();
        if relays.is_empty() {
            "demo".to_string()
        } else {
            format!("relay ({})", relays.join(","))
        }
    }

    fn nostr_relay_urls_from_env(&self) -> Vec<String> {
        nostr_relay_urls_from_env()
    }

    /// Leave the current TUI Nostr room.
    pub fn leave_nostr_room_runtime(&mut self) {
        self.nostr_runtime = None;
        self.nostr_connected = false;
        self.nostr_participants.clear();
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
                    self.nostr_participants
                        .insert(payload.party_index, payload.nostr_pubkey);
                }
                frostdao::nostr::NostrMessageKind::TxProposal
                    if message.from != self.nostr_my_index =>
                {
                    let payload: frostdao::nostr::TxProposalEvent = message.payload_as()?;
                    let session_id = message.session.clone().unwrap_or_else(|| {
                        format!("proposal-{}-{}", payload.proposer_index, payload.timestamp)
                    });
                    self.nostr_pending_proposals.insert(
                        session_id.clone(),
                        TxProposal {
                            session_id,
                            proposer_index: payload.proposer_index,
                            to_address: payload.to_address,
                            amount_sats: payload.amount_sats,
                            fee_rate: payload.fee_rate,
                            sighash: payload.sighash,
                            review: payload.review,
                            description: payload.description,
                            timestamp: payload.timestamp,
                        },
                    );
                }
                frostdao::nostr::NostrMessageKind::TxConsent => {
                    let payload: frostdao::nostr::TxConsentEvent = message.payload_as()?;
                    if payload.consent {
                        if let NostrSignState::WaitingForConsent {
                            session_id,
                            consents,
                            ..
                        } = &mut self.nostr_sign_state
                        {
                            if *session_id == payload.proposal_session {
                                consents.insert(
                                    message.from,
                                    payload.reviewed_sighash_fingerprint.clone(),
                                );
                            }
                        }
                    }
                }
                frostdao::nostr::NostrMessageKind::SigningNonceEncrypted => {
                    let payload: frostdao::nostr::SigningNonceEvent = message.payload_as()?;
                    let session_id = message
                        .session
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());
                    self.nostr_received_nonces
                        .entry(session_id)
                        .or_default()
                        .insert(payload.party_index, payload.ciphertext);
                }
                frostdao::nostr::NostrMessageKind::SigningShareEncrypted => {
                    let payload: frostdao::nostr::SigningShareEvent = message.payload_as()?;
                    let session_id = message
                        .session
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());
                    self.nostr_received_shares
                        .entry(session_id.clone())
                        .or_default()
                        .insert(payload.party_index, payload.ciphertext.clone());
                    if let NostrSignState::CollectingShares {
                        session_id: active_session,
                        received_shares,
                        ..
                    } = &mut self.nostr_sign_state
                    {
                        if *active_session == session_id {
                            received_shares.insert(payload.party_index, payload.ciphertext);
                        }
                    }
                }
                frostdao::nostr::NostrMessageKind::TxBroadcast => {
                    let payload: frostdao::nostr::TxBroadcastEvent = message.payload_as()?;
                    let session_id = message
                        .session
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());
                    self.nostr_broadcasts
                        .insert(session_id.clone(), payload.clone());
                    if matches!(
                        &self.nostr_sign_state,
                        NostrSignState::WaitingForExecution {
                            session_id: active_session,
                            ..
                        } | NostrSignState::CollectingShares {
                            session_id: active_session,
                            ..
                        } | NostrSignState::Combining {
                            session_id: active_session,
                            ..
                        } if *active_session == session_id
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

    /// Publish a demo participant join into the active room transport.
    pub fn simulate_nostr_participant_join(&mut self, party_index: u32) -> Result<()> {
        let Some(runtime) = self.nostr_runtime.as_mut() else {
            anyhow::bail!("join a room before simulating participants");
        };

        let payload = frostdao::nostr::RoomJoinPayload {
            party_index,
            nostr_pubkey: format!("npub-demo-{}", party_index),
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

        runtime.publish_demo_message(message)?;
        self.poll_nostr_room_runtime()?;
        Ok(())
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
            0 => NetworkSelection::Testnet4,
            1 => NetworkSelection::Testnet3,
            2 => NetworkSelection::Signet,
            3 => NetworkSelection::Mainnet,
            _ => NetworkSelection::Testnet4,
        };
        self.state = AppState::Home;
        self.message = Some(format!("Switched to {}", self.network.display_name()));
    }

    /// Set status message
    pub fn set_message(&mut self, msg: &str) {
        self.message = Some(msg.to_string());
    }

    /// Copy text to clipboard
    pub fn copy_to_clipboard(&mut self, text: &str) {
        match arboard::Clipboard::new() {
            Ok(mut clipboard) => match clipboard.set_text(text) {
                Ok(_) => {
                    // Use char_indices for safe UTF-8 slicing (avoids panic on multi-byte chars)
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
                    self.message = Some(format!("Copied: {}", preview));
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
}

fn unix_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(1)
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
    use super::App;
    use crate::tui::state::{NetworkSelection, NostrSignState};

    #[test]
    fn tui_nostr_room_uses_runtime_and_replay_cache() {
        let mut app = App::new().unwrap();
        app.nostr_room_id = format!("tui-runtime-test-{}", std::process::id());
        app.nostr_my_index = 1;
        app.nostr_threshold = 2;
        app.nostr_n_parties = 2;
        let cache_path = app.nostr_replay_cache_path();
        let _ = std::fs::remove_file(&cache_path);

        app.join_nostr_room_runtime_with_relays(Vec::new()).unwrap();
        assert!(app.nostr_connected);
        assert!(app.nostr_runtime.is_some());
        assert_eq!(app.nostr_transport_label(), "demo");
        assert_eq!(app.nostr_participants.get(&1).unwrap(), "tui-party-1");
        assert!(cache_path.exists());

        app.simulate_nostr_participant_join(2).unwrap();
        assert_eq!(app.nostr_participants.get(&2).unwrap(), "npub-demo-2");

        app.leave_nostr_room_runtime();
        assert!(!app.nostr_connected);
        assert!(app.nostr_runtime.is_none());
        assert!(app.nostr_participants.is_empty());

        let _ = std::fs::remove_file(&cache_path);
    }

    #[test]
    fn tui_nostr_signing_publishes_runtime_messages() {
        let mut app = App::new().unwrap();
        app.nostr_room_id = format!("tui-signing-runtime-test-{}", std::process::id());
        app.nostr_my_index = 1;
        app.nostr_threshold = 2;
        app.nostr_n_parties = 2;
        let cache_path = app.nostr_replay_cache_path();
        let _ = std::fs::remove_file(&cache_path);

        app.join_nostr_room_runtime_with_relays(Vec::new()).unwrap();
        let proposal = crate::tui::state::TxProposal {
            session_id: "session-test".to_string(),
            proposer_index: 1,
            to_address: "tb1qrecipient".to_string(),
            amount_sats: 50_000,
            fee_rate: 10,
            sighash: "abc123".to_string(),
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
                .demo_room_len(&app.nostr_room_id),
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
                .demo_room_len(&app.nostr_room_id),
            Some(3)
        );

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
        app.publish_nostr_tx_broadcast(
            "wallet-test",
            "session-test",
            "txid-test".to_string(),
            "raw-transaction-hex".to_string(),
            "testnet".to_string(),
        )
        .unwrap();
        assert_eq!(app.audit_events[4].event, "nostr_tx_broadcast");
        assert_eq!(app.audit_events[4].fields["txid"], "txid-test");
        assert!(app.audit_events[4].fields.get("raw_tx").is_none());
        assert_eq!(
            app.nostr_runtime
                .as_ref()
                .unwrap()
                .demo_room_len(&app.nostr_room_id),
            Some(6)
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
    fn tui_nostr_poll_ingests_proposals_and_consents() {
        let mut app = App::new().unwrap();
        app.nostr_room_id = format!("tui-ingest-runtime-test-{}", std::process::id());
        app.nostr_my_index = 1;
        app.nostr_threshold = 2;
        app.nostr_n_parties = 2;
        let cache_path = app.nostr_replay_cache_path();
        let _ = std::fs::remove_file(&cache_path);

        app.join_nostr_room_runtime_with_relays(Vec::new()).unwrap();

        let proposal_event = frostdao::nostr::TxProposalEvent {
            proposer_index: 2,
            to_address: "tb1qrecipient".to_string(),
            amount_sats: 25_000,
            fee_rate: 8,
            sighash: "remote-sighash".to_string(),
            review: frostdao::nostr::TxReviewPayload {
                network: "testnet".to_string(),
                source_path: "m/86'/1'/0'/0/1".to_string(),
                from_address: "tb1qremote".to_string(),
                to_address: "tb1qrecipient".to_string(),
                amount_sats: 25_000,
                fee_rate_sats_vb: 8,
                sighash_fingerprint: "remote123".to_string(),
            },
            description: "remote proposal".to_string(),
            timestamp: 1_700_000_010,
        };
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
            .publish_demo_message(proposal_message)
            .unwrap();

        app.poll_nostr_room_runtime().unwrap();
        let pending = app.nostr_pending_proposals.get("session-remote").unwrap();
        assert_eq!(pending.proposer_index, 2);
        assert_eq!(pending.amount_sats, 25_000);
        assert_eq!(pending.review.sighash_fingerprint, "remote123");

        app.nostr_sign_state = NostrSignState::WaitingForConsent {
            wallet_name: "wallet-test".to_string(),
            session_id: "session-remote".to_string(),
            proposal: pending.clone(),
            consents: std::collections::HashMap::new(),
        };
        let consent_event = frostdao::nostr::TxConsentEvent {
            proposal_session: "session-remote".to_string(),
            consent: true,
            reviewed_sighash_fingerprint: "remote123".to_string(),
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
            .publish_demo_message(consent_message)
            .unwrap();

        app.poll_nostr_room_runtime().unwrap();
        if let NostrSignState::WaitingForConsent { consents, .. } = &app.nostr_sign_state {
            assert_eq!(consents.get(&2).unwrap(), "remote123");
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
            .publish_demo_message(nonce_message)
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
            .publish_demo_message(share_message)
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
        let broadcast_event = frostdao::nostr::TxBroadcastEvent {
            txid: "txid-remote".to_string(),
            raw_tx: "raw-remote-transaction".to_string(),
            network: "testnet".to_string(),
        };
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
            .publish_demo_message(broadcast_message)
            .unwrap();
        app.poll_nostr_room_runtime().unwrap();
        assert_eq!(
            app.nostr_broadcasts.get("session-remote").unwrap().txid,
            "txid-remote"
        );
        assert!(matches!(
            &app.nostr_sign_state,
            NostrSignState::Complete { txid } if txid == "txid-remote"
        ));

        let _ = std::fs::remove_file(&cache_path);
    }
}
