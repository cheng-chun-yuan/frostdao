//! Terminal UI module for FrostDAO wallet management
//!
//! Provides an interactive terminal interface for:
//! - Viewing and managing DKG wallets
//! - Chain/network selection (Testnet, Signet, Regtest, Mainnet)
//! - Keygen wizard for creating new wallets
//! - Reshare wizard for resharing existing wallets
//! - Send wizard for threshold signing transactions

pub mod app;
pub mod components;
pub mod screens;
pub mod state;

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame, Terminal,
};
use std::io;
#[cfg(feature = "miniscript-policy")]
use std::str::FromStr;

use app::App;
use state::{
    AddressListState, AppState, KeygenState, MnemonicState, NostrKeygenState, NostrRoomField,
    NostrRoomPhase, NostrSignState, NostrTxField, ReshareState, SendState, WalletAction,
    WalletDetailsState,
};

use frostdao::protocol::{keygen, reshare, signing};
use frostdao::storage::{FileStorage, Storage};

pub(crate) const COPY_KEY_LABEL: &str = "c/C";
pub(crate) const REFRESH_KEY_LABEL: &str = "b/r/F5";

/// Run the terminal UI
pub fn run_tui() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new()?;
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn is_copy_key(code: &KeyCode) -> bool {
    matches!(code, KeyCode::Char(c) if c.eq_ignore_ascii_case(&'c'))
}

fn is_refresh_key(code: &KeyCode) -> bool {
    (matches!(code, KeyCode::Char(c) if c.eq_ignore_ascii_case(&'b') || c.eq_ignore_ascii_case(&'r')))
        || matches!(code, KeyCode::F(5))
}

fn is_shortcut_key(code: &KeyCode, target: char) -> bool {
    matches!(code, KeyCode::Char(c) if c.eq_ignore_ascii_case(&target))
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                if key.code == KeyCode::F(1) {
                    app.show_global_help = true;
                    continue;
                }

                if app.show_global_help {
                    if matches!(key.code, KeyCode::Esc | KeyCode::F(1)) {
                        app.show_global_help = false;
                    }
                    continue;
                }

                // Global quit
                if is_shortcut_key(&key.code, 'q') && matches!(app.state, AppState::Home) {
                    return Ok(());
                }

                match &app.state {
                    AppState::Home => handle_home_keys(app, key.code),
                    AppState::WalletDetails(_) => handle_wallet_details_keys(app, key.code),
                    AppState::ChainSelect => handle_chain_select_keys(app, key.code),
                    AppState::Keygen(_) => handle_keygen_keys(app, key),
                    AppState::Reshare(_) => handle_reshare_keys(app, key),
                    AppState::Send(_) => handle_send_keys(app, key),
                    AppState::AddressList(_) => handle_address_list_keys(app, key.code),
                    AppState::MnemonicBackup(_) => handle_mnemonic_keys(app, key.code),
                    AppState::NostrRoom => handle_nostr_room_keys(app, key),
                    AppState::NostrKeygen => handle_nostr_keygen_keys(app, key.code),
                    AppState::NostrSign => handle_nostr_sign_keys(app, key),
                    #[cfg(feature = "miniscript-policy")]
                    AppState::PolicyPreview => handle_policy_preview_keys(app, key),
                }
            }
        }
    }
}

fn handle_home_keys(app: &mut App, code: KeyCode) {
    match code {
        code if code == KeyCode::Down || is_shortcut_key(&code, 'j') => app.next_wallet(),
        code if code == KeyCode::Up || is_shortcut_key(&code, 'k') => app.prev_wallet(),
        KeyCode::Enter => {
            // Go to wallet details
            if let Some(wallet) = app.selected_wallet() {
                app.state = AppState::WalletDetails(WalletDetailsState {
                    wallet_name: wallet.name.clone(),
                    selected_action: 0,
                    confirm_delete: false,
                    delete_confirmation_input: String::new(),
                    show_qr: false,
                });
            } else {
                app.set_message("No wallet selected");
            }
        }
        code if is_refresh_key(&code) => app.refresh_balance(),
        code if is_shortcut_key(&code, 'r') => app.reload_wallets(),
        code if is_shortcut_key(&code, 'n') => {
            app.chain_selector_index = app.network.index();
            app.state = AppState::ChainSelect;
        }
        code if is_shortcut_key(&code, 'g') => {
            app.state = AppState::Keygen(state::KeygenState::default());
        }
        code if is_shortcut_key(&code, 'h') => {
            if app.selected_wallet().is_some() {
                app.state = AppState::Reshare(state::ReshareState::default());
            } else {
                app.set_message("Select a wallet first to reshare");
            }
        }
        code if is_shortcut_key(&code, 's') => {
            if app.selected_wallet().is_some() {
                if let Some(message) = app.utxo_source_unavailable_message() {
                    app.set_message(&message);
                } else {
                    app.state = AppState::Send(state::SendState::default());
                }
            } else {
                app.set_message("Select a wallet first to send");
            }
        }
        code if is_shortcut_key(&code, 'a') => {
            // HD Address list
            if let Some(wallet) = app.selected_wallet() {
                let wallet_name = wallet.name.clone();
                app.state = AppState::AddressList(AddressListState {
                    wallet_name: wallet_name.clone(),
                    network: app.network,
                    addresses: Vec::new(),
                    selected: 0,
                    error: None,
                    hd_enabled: false,
                    balance_cache: std::collections::HashMap::new(),
                });
                // Load addresses
                app.load_hd_addresses(&wallet_name);
            } else {
                app.set_message("Select a wallet first to view addresses");
            }
        }
        code if is_shortcut_key(&code, 'm') => {
            // Mnemonic backup
            if let Some(wallet) = app.selected_wallet() {
                let wallet_name = wallet.name.clone();
                let hierarchical = wallet.hierarchical.unwrap_or(false);
                let party_ranks = wallet.party_ranks.clone().unwrap_or_default();
                let state_dir = keygen::get_state_dir(&wallet_name);

                // Scan for available party folders
                let mut available_parties = Vec::new();
                for i in 1..=10 {
                    // Check up to 10 parties
                    let party_dir = format!("{}/party{}", state_dir, i);
                    let share_path = format!("{}/paired_secret_share.bin", party_dir);
                    if std::path::Path::new(&share_path).exists() {
                        available_parties.push(i);
                    }
                }

                // Check for legacy structure (share directly in wallet folder)
                let legacy_share_path = format!("{}/paired_secret_share.bin", state_dir);
                let has_legacy_share = std::path::Path::new(&legacy_share_path).exists();

                if available_parties.is_empty() && !has_legacy_share {
                    app.set_message("No party shares found in this wallet");
                } else if has_legacy_share && available_parties.is_empty() {
                    // Legacy wallet - use party index 0 to indicate legacy
                    app.state = AppState::MnemonicBackup(MnemonicState {
                        wallet_name: wallet_name.clone(),
                        available_parties: vec![0], // 0 = legacy (direct in wallet folder)
                        selected_party: 0,
                        words: Vec::new(),
                        error: None,
                        party_selected: false,
                        revealed: false,
                        hierarchical: false,
                        party_ranks: std::collections::BTreeMap::new(),
                    });
                } else {
                    app.state = AppState::MnemonicBackup(MnemonicState {
                        wallet_name: wallet_name.clone(),
                        available_parties,
                        selected_party: 0,
                        words: Vec::new(),
                        error: None,
                        party_selected: false,
                        revealed: false,
                        hierarchical,
                        party_ranks,
                    });
                }
            } else {
                app.set_message("Select a wallet first to backup");
            }
        }
        code if is_copy_key(&code) => {
            // Copy wallet address
            let addr = app
                .selected_wallet()
                .and_then(|w| app::wallet_address_for_network(w, app.network))
                .map(str::to_string);
            if let Some(addr) = addr {
                app.copy_to_clipboard_with_message(
                    &addr,
                    root_address_copy_message(app.network, &addr),
                );
            } else {
                app.set_message("Select a wallet first to copy address");
            }
        }
        code if is_shortcut_key(&code, 'o') => {
            // Nostr room for distributed DKG/signing
            app.state = AppState::NostrRoom;
        }
        #[cfg(feature = "miniscript-policy")]
        code if is_shortcut_key(&code, 'p') => {
            app.policy_preview_form = screens::PolicyPreviewFormData::new();
            app.state = AppState::PolicyPreview;
        }
        _ => {}
    }
}

#[cfg(feature = "miniscript-policy")]
fn handle_policy_preview_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.state = AppState::Home;
        }
        KeyCode::Tab => {
            app.policy_preview_form.focused_field = app.policy_preview_form.focused_field.next();
        }
        KeyCode::BackTab => {
            app.policy_preview_form.focused_field = app.policy_preview_form.focused_field.prev();
        }
        KeyCode::Char(']') => {
            app.policy_preview_form.next_preset();
        }
        KeyCode::Char('[') => {
            app.policy_preview_form.prev_preset();
        }
        KeyCode::Enter => {
            initialize_agent_payment_policy(app);
        }
        code if is_copy_key(&code) && !app.policy_preview_form.output.trim().is_empty() => {
            let output = app.policy_preview_form.output.clone();
            app.copy_to_clipboard(&output);
        }
        _ => {
            handle_policy_preview_input(app, key);
        }
    }
}

#[cfg(feature = "miniscript-policy")]
fn handle_policy_preview_input(app: &mut App, key: KeyEvent) {
    use screens::PolicyPreviewField;

    match app.policy_preview_form.focused_field {
        PolicyPreviewField::AgentLabel => {
            app.policy_preview_form.agent_label.handle_key(key);
        }
        PolicyPreviewField::AgentPubkey => {
            app.policy_preview_form.agent_pubkey.handle_key(key);
        }
        PolicyPreviewField::Recipient => {
            app.policy_preview_form.recipient.handle_key(key);
        }
        PolicyPreviewField::Amount => {
            app.policy_preview_form.amount_sats.handle_key(key);
        }
        PolicyPreviewField::DailyLimit => {
            app.policy_preview_form.daily_limit_sats.handle_key(key);
        }
        PolicyPreviewField::AgentIndex => {
            app.policy_preview_form.agent_index.handle_key(key);
        }
        PolicyPreviewField::Policy => {
            app.policy_preview_form.policy_input.handle_key(key);
        }
    }
}

#[cfg(feature = "miniscript-policy")]
fn initialize_agent_payment_policy(app: &mut App) {
    let Some(wallet) = app.selected_wallet().cloned() else {
        app.policy_preview_form.output.clear();
        app.policy_preview_form.error =
            Some("Select a wallet before initializing an agent payment policy".to_string());
        return;
    };

    let agent_label = app.policy_preview_form.agent_label.value().trim();
    let agent_xonly_pubkey = app.policy_preview_form.agent_pubkey.value().trim();
    let recipient = app.policy_preview_form.recipient.value().trim();
    let amount_sats = app
        .policy_preview_form
        .amount_sats
        .value()
        .trim()
        .parse::<u64>()
        .unwrap_or(0);
    let daily_limit_sats = app
        .policy_preview_form
        .daily_limit_sats
        .value()
        .trim()
        .parse::<u64>()
        .unwrap_or(0);
    let agent_index = app
        .policy_preview_form
        .agent_index
        .value()
        .trim()
        .parse::<u32>()
        .unwrap_or(0);

    if agent_label.is_empty() {
        app.policy_preview_form.output.clear();
        app.policy_preview_form.error = Some("Agent label is required".to_string());
        return;
    }
    if agent_xonly_pubkey.is_empty() {
        app.policy_preview_form.output.clear();
        app.policy_preview_form.error = Some("Agent pubkey is required".to_string());
        return;
    }
    let agent_pubkey_bytes = match hex::decode(agent_xonly_pubkey) {
        Ok(bytes) if bytes.len() == 32 => bytes,
        Ok(_) => {
            app.policy_preview_form.output.clear();
            app.policy_preview_form.error =
                Some("Agent pubkey must be 32-byte x-only hex".to_string());
            return;
        }
        Err(err) => {
            app.policy_preview_form.output.clear();
            app.policy_preview_form.error = Some(format!("Invalid agent pubkey hex: {err}"));
            return;
        }
    };
    if let Err(err) = bitcoin::secp256k1::XOnlyPublicKey::from_slice(&agent_pubkey_bytes) {
        app.policy_preview_form.output.clear();
        app.policy_preview_form.error = Some(format!("Invalid agent x-only pubkey: {err}"));
        return;
    }
    if recipient.is_empty() {
        app.policy_preview_form.output.clear();
        app.policy_preview_form.error = Some("Recipient address is required".to_string());
        return;
    }
    if amount_sats == 0 {
        app.policy_preview_form.output.clear();
        app.policy_preview_form.error = Some("Amount must be greater than zero".to_string());
        return;
    }
    if daily_limit_sats == 0 {
        app.policy_preview_form.output.clear();
        app.policy_preview_form.error = Some("Daily limit must be greater than zero".to_string());
        return;
    }

    let state_dir = keygen::get_state_dir(&wallet.name);
    let storage = match FileStorage::new(&state_dir) {
        Ok(storage) => storage,
        Err(err) => {
            app.policy_preview_form.output.clear();
            app.policy_preview_form.error = Some(format!("Cannot open wallet storage: {err}"));
            return;
        }
    };

    let network = app.network.to_bitcoin_network();
    let recipient_address = match bitcoin::Address::from_str(recipient) {
        Ok(address) => match address.require_network(network) {
            Ok(address) => address.to_string(),
            Err(err) => {
                app.policy_preview_form.output.clear();
                app.policy_preview_form.error = Some(format!(
                    "Recipient address is invalid for {}: {err}",
                    app.network.display_name()
                ));
                return;
            }
        },
        Err(err) => {
            app.policy_preview_form.output.clear();
            app.policy_preview_form.error = Some(format!("Invalid recipient address: {err}"));
            return;
        }
    };
    let (frost_key_path_address, frost_control_pubkey) =
        match frostdao::btc::hd_address::derive_address_at_path(&storage, 0, agent_index, network) {
            Ok(result) => result,
            Err(err) => {
                app.policy_preview_form.output.clear();
                app.policy_preview_form.error =
                    Some(format!("Cannot derive agent payment address: {err}"));
                return;
            }
        };

    let policy_template = app.policy_preview_form.policy_input.content();
    let policy_for_compile = policy_template
        .replace("AGENT", agent_xonly_pubkey)
        .replace("DAO", &frost_control_pubkey);
    let compiled = match frostdao::btc::miniscript_policy::compile_taproot_policy(
        &policy_for_compile,
        Some(&frost_control_pubkey),
    ) {
        Ok(result) => result,
        Err(err) => {
            app.policy_preview_form.output.clear();
            app.policy_preview_form.error = Some(err.to_string());
            return;
        }
    };

    let policy_status = if amount_sats <= daily_limit_sats {
        "draft_amount_within_limit"
    } else {
        "draft_needs_dao_approval"
    };
    let derivation_path =
        frostdao::crypto::hd::format_bip86_path(app.network.to_bitcoin_network(), 0, agent_index);
    let draft = serde_json::json!({
        "type": "frostdao.agent_payment_init",
        "version": 1,
        "wallet": wallet.name,
        "network": app.network.display_name(),
        "agent": {
            "label": agent_label,
            "pubkey": agent_xonly_pubkey,
            "index": agent_index,
            "derivation_path": derivation_path,
            "frost_key_path_address": frost_key_path_address,
            "frost_control_pubkey": frost_control_pubkey,
        },
        "payment": {
            "recipient": recipient_address,
            "amount_sats": amount_sats,
            "daily_limit_sats": daily_limit_sats,
            "status": policy_status,
        },
        "miniscript": {
            "scope": "descriptor_preview_only_script_path_spending_not_wired",
            "policy_template": policy_template,
            "compiled_policy": compiled.policy,
            "taproot_descriptor_preview": compiled.descriptor,
            "warning": compiled.warning,
        }
    });

    app.policy_preview_form.output =
        serde_json::to_string_pretty(&draft).unwrap_or_else(|err| err.to_string());
    app.policy_preview_form.error = None;
}

fn handle_chain_select_keys(app: &mut App, code: KeyCode) {
    match code {
        code if code == KeyCode::Up || is_shortcut_key(&code, 'k') => app.prev_network(),
        code if code == KeyCode::Down || is_shortcut_key(&code, 'j') => app.next_network(),
        KeyCode::Enter => app.confirm_network(),
        KeyCode::Esc => app.state = AppState::Home,
        _ => {}
    }
}

fn handle_wallet_details_keys(app: &mut App, code: KeyCode) {
    let state = if let AppState::WalletDetails(s) = &app.state {
        s.clone()
    } else {
        return;
    };

    // Handle QR code popup mode
    if state.show_qr {
        match code {
            code if code == KeyCode::Esc || is_shortcut_key(&code, 'q') => {
                if let AppState::WalletDetails(ref mut s) = app.state {
                    s.show_qr = false;
                }
            }
            _ => {}
        }
        return;
    }

    // Handle confirm delete mode
    if state.confirm_delete {
        match code {
            KeyCode::Esc => {
                if let AppState::WalletDetails(ref mut s) = app.state {
                    s.confirm_delete = false;
                    s.delete_confirmation_input.clear();
                }
            }
            KeyCode::Backspace => {
                if let AppState::WalletDetails(ref mut s) = app.state {
                    s.delete_confirmation_input.pop();
                }
            }
            KeyCode::Enter => {
                let wallet_name = state.wallet_name.clone();
                if state.delete_confirmation_input != wallet_name {
                    app.set_message("Type the wallet name exactly to delete");
                    return;
                }

                let state_dir = keygen::get_state_dir(&wallet_name);
                match std::fs::remove_dir_all(&state_dir) {
                    Ok(_) => {
                        app.set_message(&format!("Wallet '{}' deleted", wallet_name));
                        app.reload_wallets();
                        app.state = AppState::Home;
                    }
                    Err(e) => {
                        app.set_message(&format!("Failed to delete: {}", e));
                        if let AppState::WalletDetails(ref mut s) = app.state {
                            s.confirm_delete = false;
                        }
                    }
                }
            }
            KeyCode::Char(ch) => {
                if let AppState::WalletDetails(ref mut s) = app.state {
                    s.delete_confirmation_input.push(ch);
                }
            }
            _ => {}
        }
        return;
    }

    let actions = WalletAction::all();
    let action_count = actions.len();

    match code {
        KeyCode::Esc => {
            app.state = AppState::Home;
        }
        code if code == KeyCode::Up || is_shortcut_key(&code, 'k') => {
            if let AppState::WalletDetails(ref mut s) = app.state {
                if s.selected_action > 0 {
                    s.selected_action -= 1;
                } else {
                    s.selected_action = action_count - 1;
                }
            }
        }
        code if code == KeyCode::Down || is_shortcut_key(&code, 'j') => {
            if let AppState::WalletDetails(ref mut s) = app.state {
                s.selected_action = (s.selected_action + 1) % action_count;
            }
        }
        KeyCode::Enter => {
            let selected_action = actions[state.selected_action];
            let wallet_name = state.wallet_name.clone();

            match selected_action {
                WalletAction::Send => {
                    // Go to send flow with wallet pre-selected
                    if let Some(message) = app.utxo_source_unavailable_message() {
                        app.set_message(&message);
                    } else {
                        app.send_form = screens::SendFormData::new();
                        // Find wallet index
                        if let Some(idx) = app.wallets.iter().position(|w| w.name == wallet_name) {
                            app.send_form.wallet_index = idx;
                            // Load party info
                            if let Some(wallet) = app.wallets.get(idx) {
                                app.send_form.threshold = wallet.threshold.unwrap_or(2);
                                app.send_form.total_parties = wallet.total_parties.unwrap_or(3);
                                app.send_form.selected_parties =
                                    vec![true; wallet.total_parties.unwrap_or(3) as usize];
                                // Load HTSS info
                                app.send_form.hierarchical = wallet.hierarchical.unwrap_or(false);
                                app.send_form.signing_requirement =
                                    wallet.signing_requirement.clone();
                                app.send_form.party_ranks =
                                    wallet.party_ranks.clone().unwrap_or_default();
                            }
                        }
                        app.state = AppState::Send(SendState::SelectSigners { wallet_name });
                    }
                }
                WalletAction::ViewAddresses => {
                    app.state = AppState::AddressList(AddressListState {
                        wallet_name: wallet_name.clone(),
                        network: app.network,
                        addresses: Vec::new(),
                        selected: 0,
                        error: None,
                        hd_enabled: false,
                        balance_cache: std::collections::HashMap::new(),
                    });
                    app.load_hd_addresses(&wallet_name);
                }
                WalletAction::BackupMnemonic => {
                    let state_dir = keygen::get_state_dir(&wallet_name);

                    // Get HTSS info from wallet
                    let hierarchical = app
                        .wallets
                        .iter()
                        .find(|w| w.name == wallet_name)
                        .and_then(|w| w.hierarchical)
                        .unwrap_or(false);
                    let party_ranks = app
                        .wallets
                        .iter()
                        .find(|w| w.name == wallet_name)
                        .and_then(|w| w.party_ranks.clone())
                        .unwrap_or_default();

                    // Scan for available party folders
                    let mut available_parties = Vec::new();
                    for i in 1..=10 {
                        let party_dir = format!("{}/party{}", state_dir, i);
                        let share_path = format!("{}/paired_secret_share.bin", party_dir);
                        if std::path::Path::new(&share_path).exists() {
                            available_parties.push(i);
                        }
                    }

                    // Check for legacy structure
                    let legacy_share_path = format!("{}/paired_secret_share.bin", state_dir);
                    let has_legacy_share = std::path::Path::new(&legacy_share_path).exists();

                    if available_parties.is_empty() && !has_legacy_share {
                        app.set_message("No party shares found in this wallet");
                        app.state = AppState::Home;
                    } else if has_legacy_share && available_parties.is_empty() {
                        app.state = AppState::MnemonicBackup(MnemonicState {
                            wallet_name,
                            available_parties: vec![0],
                            selected_party: 0,
                            words: Vec::new(),
                            error: None,
                            party_selected: false,
                            revealed: false,
                            hierarchical: false,
                            party_ranks: std::collections::BTreeMap::new(),
                        });
                    } else {
                        app.state = AppState::MnemonicBackup(MnemonicState {
                            wallet_name,
                            available_parties,
                            selected_party: 0,
                            words: Vec::new(),
                            error: None,
                            party_selected: false,
                            revealed: false,
                            hierarchical,
                            party_ranks,
                        });
                    }
                }
                WalletAction::Reshare => {
                    app.state = AppState::Reshare(ReshareState::default());
                }
                WalletAction::DeleteWallet => {
                    // Show confirmation dialog
                    if let AppState::WalletDetails(ref mut s) = app.state {
                        s.confirm_delete = true;
                        s.delete_confirmation_input.clear();
                    }
                }
            }
        }
        code if is_copy_key(&code) => {
            // Copy wallet address to clipboard
            let addr_to_copy = app
                .wallets
                .iter()
                .find(|w| w.name == state.wallet_name)
                .and_then(|w| app::wallet_address_for_network(w, app.network))
                .map(str::to_string);
            if let Some(addr) = addr_to_copy {
                app.copy_to_clipboard_with_message(
                    &addr,
                    root_address_copy_message(app.network, &addr),
                );
            } else {
                app.set_message("No address available to copy for this wallet");
            }
        }
        code if is_refresh_key(&code) => {
            // Quick fetch balance
            let wallet_name = state.wallet_name.clone();
            if let Some(idx) = app.wallets.iter().position(|w| w.name == wallet_name) {
                app.wallet_list_state.select(Some(idx));
                app.refresh_balance();
            }
        }
        code if is_shortcut_key(&code, 'v') => {
            // Show QR code popup
            if let AppState::WalletDetails(ref mut s) = app.state {
                s.show_qr = true;
            }
        }
        _ => {}
    }
}

fn handle_keygen_keys(app: &mut App, key: KeyEvent) {
    use state::KeygenFormField;

    // Helper to get next field based on mode
    fn next_field(current: KeygenFormField, hierarchical: bool) -> KeygenFormField {
        match (current, hierarchical) {
            // TSS mode: Name -> Threshold -> NParties -> Name
            (KeygenFormField::Name, false) => KeygenFormField::Threshold,
            (KeygenFormField::Threshold, false) => KeygenFormField::NParties,
            (KeygenFormField::NParties, false) => KeygenFormField::Name,
            (KeygenFormField::MyRank, false) => KeygenFormField::Name,
            (KeygenFormField::RankDistribution, false) => KeygenFormField::Name,
            (KeygenFormField::SigningRequirement, false) => KeygenFormField::Name,
            // HTSS mode: Name -> RankDistribution -> SigningRequirement -> Name
            (KeygenFormField::Name, true) => KeygenFormField::RankDistribution,
            (KeygenFormField::RankDistribution, true) => KeygenFormField::SigningRequirement,
            (KeygenFormField::SigningRequirement, true) => KeygenFormField::Name,
            (KeygenFormField::Threshold, true) => KeygenFormField::Name,
            (KeygenFormField::NParties, true) => KeygenFormField::Name,
            (KeygenFormField::MyRank, true) => KeygenFormField::Name,
        }
    }

    fn prev_field(current: KeygenFormField, hierarchical: bool) -> KeygenFormField {
        match (current, hierarchical) {
            // TSS mode
            (KeygenFormField::Name, false) => KeygenFormField::NParties,
            (KeygenFormField::Threshold, false) => KeygenFormField::Name,
            (KeygenFormField::NParties, false) => KeygenFormField::Threshold,
            (KeygenFormField::MyRank, false) => KeygenFormField::NParties,
            (KeygenFormField::RankDistribution, false) => KeygenFormField::NParties,
            (KeygenFormField::SigningRequirement, false) => KeygenFormField::NParties,
            // HTSS mode: Name <- RankDistribution <- SigningRequirement <- Name
            (KeygenFormField::Name, true) => KeygenFormField::SigningRequirement,
            (KeygenFormField::RankDistribution, true) => KeygenFormField::Name,
            (KeygenFormField::SigningRequirement, true) => KeygenFormField::RankDistribution,
            (KeygenFormField::Threshold, true) => KeygenFormField::Name,
            (KeygenFormField::NParties, true) => KeygenFormField::Name,
            (KeygenFormField::MyRank, true) => KeygenFormField::RankDistribution,
        }
    }

    let state = app.state.clone();
    match state {
        AppState::Keygen(KeygenState::ModeSelect) => match key.code {
            KeyCode::Esc => {
                app.keygen_form = screens::KeygenFormData::new();
                app.state = AppState::Home;
            }
            code if (code == KeyCode::Up || is_shortcut_key(&code, 'k'))
                || (code == KeyCode::Down || is_shortcut_key(&code, 'j')) =>
            {
                // Toggle between TSS and HTSS
                app.keygen_form.hierarchical = !app.keygen_form.hierarchical;
            }
            KeyCode::Char('1') => {
                app.keygen_form.hierarchical = false; // TSS
            }
            KeyCode::Char('2') => {
                app.keygen_form.hierarchical = true; // HTSS
            }
            KeyCode::Enter => {
                // Proceed to params setup
                app.keygen_form.focused_field = KeygenFormField::Name;
                app.state = AppState::Keygen(KeygenState::ParamsSetup);
            }
            _ => {}
        },
        AppState::Keygen(KeygenState::ParamsSetup) => match key.code {
            KeyCode::Esc => {
                // Go back to mode select
                app.state = AppState::Keygen(KeygenState::ModeSelect);
            }
            KeyCode::Tab | KeyCode::Down => {
                app.keygen_form.focused_field =
                    next_field(app.keygen_form.focused_field, app.keygen_form.hierarchical);
            }
            KeyCode::BackTab | KeyCode::Up => {
                app.keygen_form.focused_field =
                    prev_field(app.keygen_form.focused_field, app.keygen_form.hierarchical);
            }
            KeyCode::Enter => {
                // Validate and run keygen
                let name = app.keygen_form.name.value().to_string();
                let hierarchical = app.keygen_form.hierarchical;

                if name.is_empty() {
                    app.keygen_form.error_message = Some("Wallet name is required".to_string());
                    return;
                }

                let (n_parties, threshold, ranks, signing_req) = if hierarchical {
                    // HTSS: Validate configuration and get threshold
                    match app.keygen_form.validate_htss_config() {
                        Ok(t) => {
                            let parsed_ranks = match app.keygen_form.parse_rank_distribution() {
                                Some(ranks) => ranks,
                                None => {
                                    app.keygen_form.error_message =
                                        Some("Enter parties per rank (e.g., 2,3,3)".to_string());
                                    return;
                                }
                            };

                            let signing_requirement =
                                match app.keygen_form.parse_signing_requirement() {
                                    Some(req) => Some(req),
                                    None => {
                                        app.keygen_form.error_message = Some(
                                            "Enter signing requirement (e.g., 1,2,2)".to_string(),
                                        );
                                        return;
                                    }
                                };

                            let n = parsed_ranks.len() as u32;
                            (n, t, Some(parsed_ranks), signing_requirement)
                        }
                        Err(e) => {
                            app.keygen_form.error_message = Some(e);
                            return;
                        }
                    }
                } else {
                    // TSS: Use threshold and n_parties inputs
                    let n: u32 = app.keygen_form.n_parties.value().parse().unwrap_or(0);
                    let t: u32 = app.keygen_form.threshold.value().parse().unwrap_or(0);

                    if n < 2 {
                        app.keygen_form.error_message = Some("Need at least 2 parties".to_string());
                        return;
                    }
                    if t == 0 || t > n {
                        app.keygen_form.error_message =
                            Some("Invalid threshold (must be 1 ≤ t ≤ n)".to_string());
                        return;
                    }
                    (n, t, None, None)
                };

                match keygen::generate_all_parties(
                    &name,
                    threshold,
                    n_parties,
                    hierarchical,
                    ranks,
                    signing_req,
                ) {
                    Ok(_result) => {
                        app.keygen_form.error_message = None;
                        app.reload_wallets();
                        app.state = AppState::Keygen(KeygenState::Complete { wallet_name: name });
                    }
                    Err(e) => {
                        app.keygen_form.error_message = Some(format!("Error: {}", e));
                    }
                }
            }
            _ => {
                // Handle text input based on focused field
                match app.keygen_form.focused_field {
                    KeygenFormField::Name => {
                        app.keygen_form.name.handle_key(key);
                    }
                    KeygenFormField::Threshold => {
                        // Threshold is used in both TSS and HTSS modes
                        app.keygen_form.threshold.handle_key(key);
                    }
                    KeygenFormField::NParties => {
                        app.keygen_form.n_parties.handle_key(key);
                    }
                    KeygenFormField::MyRank => {
                        if app.keygen_form.hierarchical {
                            app.keygen_form.my_rank.handle_key(key);
                        }
                    }
                    KeygenFormField::RankDistribution => {
                        if app.keygen_form.hierarchical {
                            app.keygen_form.rank_distribution.handle_key(key);
                        }
                    }
                    KeygenFormField::SigningRequirement => {
                        if app.keygen_form.hierarchical {
                            app.keygen_form.signing_requirement.handle_key(key);
                        }
                    }
                }
            }
        },
        AppState::Keygen(KeygenState::Round1Output { .. }) => match key.code {
            KeyCode::Esc => {
                app.keygen_form = screens::KeygenFormData::new();
                app.state = AppState::Home;
            }
            code if is_copy_key(&code) => {
                let output = app.keygen_form.round1_output.clone();
                app.copy_to_clipboard(&output);
            }
            KeyCode::Enter => {
                app.state = AppState::Keygen(KeygenState::Round2Input);
            }
            _ => {}
        },
        AppState::Keygen(KeygenState::Round2Input) => match key.code {
            KeyCode::Esc => {
                app.state = AppState::Keygen(KeygenState::Round1Output {
                    output_json: app.keygen_form.round1_output.clone(),
                });
            }
            KeyCode::Enter => {
                // Run keygen round 2
                let name = app.keygen_form.name.value().to_string();
                let data = app.keygen_form.round2_input.content();

                if data.trim().is_empty() {
                    app.keygen_form.error_message = Some("Paste round 1 outputs first".to_string());
                    return;
                }

                let state_dir = keygen::get_state_dir(&name);
                match FileStorage::new(&state_dir) {
                    Ok(storage) => match keygen::round2_core(&data, &storage, false) {
                        Ok(result) => {
                            app.keygen_form.round2_output = result.result;
                            app.keygen_form.error_message = None;
                            app.state = AppState::Keygen(KeygenState::Round2Output {
                                output_json: app.keygen_form.round2_output.clone(),
                            });
                        }
                        Err(e) => {
                            app.keygen_form.error_message = Some(format!("Error: {}", e));
                        }
                    },
                    Err(e) => {
                        app.keygen_form.error_message = Some(format!("Storage error: {}", e));
                    }
                }
            }
            _ => {
                app.keygen_form.round2_input.handle_key(key);
            }
        },
        AppState::Keygen(KeygenState::Round2Output { .. }) => match key.code {
            KeyCode::Esc => {
                app.state = AppState::Keygen(KeygenState::Round2Input);
            }
            code if is_copy_key(&code) => {
                let output = app.keygen_form.round2_output.clone();
                app.copy_to_clipboard(&output);
            }
            KeyCode::Enter => {
                app.state = AppState::Keygen(KeygenState::FinalizeInput);
            }
            _ => {}
        },
        AppState::Keygen(KeygenState::FinalizeInput) => match key.code {
            KeyCode::Esc => {
                app.state = AppState::Keygen(KeygenState::Round2Output {
                    output_json: app.keygen_form.round2_output.clone(),
                });
            }
            KeyCode::Enter => {
                // Run keygen finalize
                let name = app.keygen_form.name.value().to_string();
                let data = app.keygen_form.finalize_input.content();

                if data.trim().is_empty() {
                    app.keygen_form.error_message = Some("Paste round 2 outputs first".to_string());
                    return;
                }

                let state_dir = keygen::get_state_dir(&name);
                match FileStorage::new(&state_dir) {
                    Ok(storage) => {
                        match keygen::finalize_core(&data, &storage) {
                            Ok(_) => {
                                app.keygen_form.error_message = None;
                                app.state = AppState::Keygen(KeygenState::Complete {
                                    wallet_name: name.clone(),
                                });
                                // Reload wallets
                                app.reload_wallets();
                            }
                            Err(e) => {
                                app.keygen_form.error_message = Some(format!("Error: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        app.keygen_form.error_message = Some(format!("Storage error: {}", e));
                    }
                }
            }
            _ => {
                app.keygen_form.finalize_input.handle_key(key);
            }
        },
        AppState::Keygen(KeygenState::Complete { .. }) => match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                app.keygen_form = screens::KeygenFormData::new();
                app.state = AppState::Home;
            }
            _ => {}
        },
        _ => {}
    }
}

fn handle_reshare_keys(app: &mut App, key: KeyEvent) {
    use screens::ReshareFormData;
    use state::{ReshareFinalizeField, ReshareFormField, ReshareLocalField, ReshareMode};

    let state = app.state.clone();
    match state {
        AppState::Reshare(ReshareState::ModeSelect) => match key.code {
            KeyCode::Esc => {
                app.reshare_form = ReshareFormData::new();
                app.state = AppState::Home;
            }
            code if (code == KeyCode::Up || is_shortcut_key(&code, 'k'))
                && app.reshare_form.mode_selected_index > 0 =>
            {
                app.reshare_form.mode_selected_index -= 1;
            }
            code if code == KeyCode::Up || is_shortcut_key(&code, 'k') => {}
            code if code == KeyCode::Down || is_shortcut_key(&code, 'j') => {
                let modes = ReshareMode::all();
                if app.reshare_form.mode_selected_index < modes.len() - 1 {
                    app.reshare_form.mode_selected_index += 1;
                }
            }
            KeyCode::Enter => {
                let modes = ReshareMode::all();
                app.reshare_form.mode = modes[app.reshare_form.mode_selected_index];
                match app.reshare_form.mode {
                    ReshareMode::Local => {
                        app.state = AppState::Reshare(ReshareState::LocalSetup);
                    }
                    ReshareMode::Distributed => {
                        app.state = AppState::Reshare(ReshareState::Round1Setup);
                    }
                }
            }
            _ => {}
        },
        AppState::Reshare(ReshareState::LocalSetup) => match key.code {
            KeyCode::Esc => {
                app.reshare_form.error_message = None;
                app.state = AppState::Reshare(ReshareState::ModeSelect);
            }
            KeyCode::Tab => {
                app.reshare_form.local_field = app.reshare_form.local_field.next();
            }
            KeyCode::BackTab => {
                app.reshare_form.local_field = app.reshare_form.local_field.prev();
            }
            code if (code == KeyCode::Up || is_shortcut_key(&code, 'k'))
                && app.reshare_form.local_field == ReshareLocalField::SourceWallet =>
            {
                if app.reshare_form.source_wallet_index > 0 {
                    app.reshare_form.source_wallet_index -= 1;
                } else if !app.wallets.is_empty() {
                    app.reshare_form.source_wallet_index = app.wallets.len() - 1;
                }
            }
            code if (code == KeyCode::Down || is_shortcut_key(&code, 'j'))
                && app.reshare_form.local_field == ReshareLocalField::SourceWallet =>
            {
                if !app.wallets.is_empty() {
                    app.reshare_form.source_wallet_index =
                        (app.reshare_form.source_wallet_index + 1) % app.wallets.len();
                }
            }
            KeyCode::Enter => {
                // Run local reshare
                if app.wallets.is_empty() {
                    app.reshare_form.error_message = Some("No wallets available".to_string());
                    return;
                }

                let source_wallet = app.wallets[app.reshare_form.source_wallet_index]
                    .name
                    .clone();
                let target_wallet = app.reshare_form.local_target_name.value().to_string();

                if target_wallet.is_empty() {
                    app.reshare_form.error_message =
                        Some("Target wallet name required".to_string());
                    return;
                }

                // Parse optional threshold/n_parties
                let new_threshold: Option<u32> = {
                    let val = app.reshare_form.local_new_threshold.value();
                    if val.is_empty() {
                        None
                    } else {
                        val.parse().ok()
                    }
                };
                let new_n_parties: Option<u32> = {
                    let val = app.reshare_form.local_new_n_parties.value();
                    if val.is_empty() {
                        None
                    } else {
                        val.parse().ok()
                    }
                };

                // Run local reshare
                match reshare::reshare_local(
                    &source_wallet,
                    &target_wallet,
                    new_threshold,
                    new_n_parties,
                    false, // hierarchical
                ) {
                    Ok(_) => {
                        app.reshare_form.error_message = None;
                        // Reload wallets
                        app.reload_wallets();
                        app.state = AppState::Reshare(ReshareState::LocalComplete {
                            wallet_name: target_wallet,
                        });
                    }
                    Err(e) => {
                        app.reshare_form.error_message = Some(format!("Error: {}", e));
                    }
                }
            }
            _ => match app.reshare_form.local_field {
                ReshareLocalField::TargetName => {
                    app.reshare_form.local_target_name.handle_key(key);
                }
                ReshareLocalField::NewThreshold => {
                    app.reshare_form.local_new_threshold.handle_key(key);
                }
                ReshareLocalField::NewNParties => {
                    app.reshare_form.local_new_n_parties.handle_key(key);
                }
                _ => {}
            },
        },
        AppState::Reshare(ReshareState::LocalComplete { .. }) => match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                app.reshare_form = ReshareFormData::new();
                app.state = AppState::Home;
            }
            _ => {}
        },
        AppState::Reshare(ReshareState::Round1Setup) => match key.code {
            KeyCode::Esc => {
                app.reshare_form = ReshareFormData::new();
                app.state = AppState::Home;
            }
            KeyCode::Tab | KeyCode::Down => {
                app.reshare_form.focused_field = app.reshare_form.focused_field.next();
            }
            KeyCode::BackTab | KeyCode::Up => {
                // For SourceWallet, up/down changes the selection
                if app.reshare_form.focused_field == ReshareFormField::SourceWallet {
                    if app.reshare_form.source_wallet_index > 0 {
                        app.reshare_form.source_wallet_index -= 1;
                    } else if !app.wallets.is_empty() {
                        app.reshare_form.source_wallet_index = app.wallets.len() - 1;
                    }
                } else {
                    app.reshare_form.focused_field = app.reshare_form.focused_field.prev();
                }
            }
            code if (code == KeyCode::Down || is_shortcut_key(&code, 'j'))
                && app.reshare_form.focused_field == ReshareFormField::SourceWallet =>
            {
                if !app.wallets.is_empty() {
                    app.reshare_form.source_wallet_index =
                        (app.reshare_form.source_wallet_index + 1) % app.wallets.len();
                }
            }
            code if (code == KeyCode::Up || is_shortcut_key(&code, 'k'))
                && app.reshare_form.focused_field == ReshareFormField::SourceWallet =>
            {
                if app.reshare_form.source_wallet_index > 0 {
                    app.reshare_form.source_wallet_index -= 1;
                } else if !app.wallets.is_empty() {
                    app.reshare_form.source_wallet_index = app.wallets.len() - 1;
                }
            }
            KeyCode::Enter => {
                // Validate and run reshare round 1
                if app.wallets.is_empty() {
                    app.reshare_form.error_message = Some("No wallets available".to_string());
                    return;
                }

                let wallet_name = app.wallets[app.reshare_form.source_wallet_index]
                    .name
                    .clone();
                let new_threshold: u32 =
                    app.reshare_form.new_threshold.value().parse().unwrap_or(0);
                let new_n_parties: u32 =
                    app.reshare_form.new_n_parties.value().parse().unwrap_or(0);

                if new_threshold == 0 || new_threshold > new_n_parties {
                    app.reshare_form.error_message = Some("Invalid threshold".to_string());
                    return;
                }

                // Get my_old_index from the source wallet
                let state_dir = keygen::get_state_dir(&wallet_name);

                // Find available party folders
                let mut party_path: Option<(String, u32)> = None;

                // Check for party folders first (new structure)
                for i in 1..=10 {
                    let party_dir = format!("{}/party{}", state_dir, i);
                    let share_path = format!("{}/paired_secret_share.bin", party_dir);
                    if std::path::Path::new(&share_path).exists() {
                        party_path = Some((party_dir, i as u32));
                        break;
                    }
                }

                // Check for legacy structure (share directly in wallet folder)
                if party_path.is_none() {
                    let legacy_path = format!("{}/paired_secret_share.bin", state_dir);
                    if std::path::Path::new(&legacy_path).exists() {
                        party_path = Some((state_dir.clone(), 0)); // 0 = will read from share
                    }
                }

                match party_path {
                    Some((path, _party_idx)) => {
                        match FileStorage::new(&path) {
                            Ok(storage) => {
                                // Load paired secret share to get my old index
                                match storage.read("paired_secret_share.bin") {
                                    Ok(bytes) => {
                                        use schnorr_fun::frost::PairedSecretShare;
                                        use schnorr_fun::fun::marker::EvenY;

                                        let paired_share: PairedSecretShare<EvenY> =
                                            match bincode::deserialize(&bytes) {
                                                Ok(share) => share,
                                                Err(e) => {
                                                    app.reshare_form.error_message = Some(format!(
                                                        "Corrupted wallet data: {}",
                                                        e
                                                    ));
                                                    return;
                                                }
                                            };

                                        // Extract party index from scalar (big-endian, last 4 bytes)
                                        let index_bytes = paired_share.index().to_bytes();
                                        let my_old_index = u32::from_be_bytes(
                                            index_bytes[28..32].try_into().unwrap(),
                                        );

                                        match reshare::reshare_round1_core(
                                            &wallet_name,
                                            new_threshold,
                                            new_n_parties,
                                            my_old_index,
                                        ) {
                                            Ok(result) => {
                                                app.reshare_form.round1_output = result.result;
                                                app.reshare_form.error_message = None;
                                                app.state =
                                                    AppState::Reshare(ReshareState::Round1Output {
                                                        output_json: app
                                                            .reshare_form
                                                            .round1_output
                                                            .clone(),
                                                    });
                                            }
                                            Err(e) => {
                                                app.reshare_form.error_message =
                                                    Some(format!("Error: {}", e));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        app.reshare_form.error_message =
                                            Some(format!("Cannot read share: {}", e));
                                    }
                                }
                            }
                            Err(e) => {
                                app.reshare_form.error_message =
                                    Some(format!("Storage error: {}", e));
                            }
                        }
                    }
                    None => {
                        app.reshare_form.error_message =
                            Some("No party shares found in wallet".to_string());
                    }
                }
            }
            _ => {
                // Handle text input based on focused field
                match app.reshare_form.focused_field {
                    ReshareFormField::SourceWallet => {
                        // Arrow keys handled above
                    }
                    ReshareFormField::NewThreshold => {
                        app.reshare_form.new_threshold.handle_key(key);
                    }
                    ReshareFormField::NewNParties => {
                        app.reshare_form.new_n_parties.handle_key(key);
                    }
                }
            }
        },
        AppState::Reshare(ReshareState::Round1Output { .. }) => match key.code {
            KeyCode::Esc => {
                // Old party: done, go home
                app.reshare_form = ReshareFormData::new();
                app.state = AppState::Home;
            }
            code if is_copy_key(&code) => {
                let output = app.reshare_form.round1_output.clone();
                app.copy_to_clipboard(&output);
            }
            KeyCode::Enter => {
                // New party: go to finalize
                app.state = AppState::Reshare(ReshareState::FinalizeInput);
            }
            _ => {}
        },
        AppState::Reshare(ReshareState::FinalizeInput) => match key.code {
            KeyCode::Esc => {
                app.state = AppState::Reshare(ReshareState::Round1Output {
                    output_json: app.reshare_form.round1_output.clone(),
                });
            }
            KeyCode::Tab => {
                app.reshare_form.finalize_field = app.reshare_form.finalize_field.next();
            }
            KeyCode::BackTab => {
                app.reshare_form.finalize_field = app.reshare_form.finalize_field.prev();
            }
            KeyCode::Char(' ')
                if app.reshare_form.finalize_field == ReshareFinalizeField::Hierarchical =>
            {
                app.reshare_form.hierarchical = !app.reshare_form.hierarchical;
            }
            KeyCode::Enter => {
                // Run reshare finalize
                let source_wallet = if !app.wallets.is_empty() {
                    app.wallets[app.reshare_form.source_wallet_index]
                        .name
                        .clone()
                } else {
                    String::new()
                };
                let target_name = app.reshare_form.target_name.value().to_string();
                let my_new_index: u32 = app.reshare_form.my_new_index.value().parse().unwrap_or(0);
                let my_rank: u32 = app.reshare_form.my_rank.value().parse().unwrap_or(0);
                let hierarchical = app.reshare_form.hierarchical;
                let data = app.reshare_form.finalize_input.content();

                if target_name.is_empty() {
                    app.reshare_form.error_message = Some("Wallet name is required".to_string());
                    return;
                }
                if my_new_index == 0 {
                    app.reshare_form.error_message = Some("Invalid new index".to_string());
                    return;
                }
                if data.trim().is_empty() {
                    app.reshare_form.error_message =
                        Some("Paste round 1 outputs first".to_string());
                    return;
                }

                match reshare::reshare_finalize_core(
                    &source_wallet,
                    &target_name,
                    my_new_index,
                    my_rank,
                    hierarchical,
                    &data,
                    false,
                ) {
                    Ok(_) => {
                        app.reshare_form.error_message = None;
                        app.state = AppState::Reshare(ReshareState::Complete {
                            wallet_name: target_name.clone(),
                        });
                        app.reload_wallets();
                    }
                    Err(e) => {
                        app.reshare_form.error_message = Some(format!("Error: {}", e));
                    }
                }
            }
            _ => {
                // Handle text input based on focused field
                match app.reshare_form.finalize_field {
                    ReshareFinalizeField::TargetName => {
                        app.reshare_form.target_name.handle_key(key);
                    }
                    ReshareFinalizeField::MyIndex => {
                        app.reshare_form.my_new_index.handle_key(key);
                    }
                    ReshareFinalizeField::MyRank => {
                        app.reshare_form.my_rank.handle_key(key);
                    }
                    ReshareFinalizeField::Hierarchical => {
                        // Space handled above
                    }
                    ReshareFinalizeField::DataInput => {
                        app.reshare_form.finalize_input.handle_key(key);
                    }
                }
            }
        },
        AppState::Reshare(ReshareState::Complete { .. }) => match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                app.reshare_form = ReshareFormData::new();
                app.state = AppState::Home;
            }
            _ => {}
        },
        _ => {}
    }
}

fn handle_send_keys(app: &mut App, key: KeyEvent) {
    use screens::SendFormData;
    use state::SendFormField;

    let state = app.state.clone();
    match state {
        AppState::Send(SendState::SelectWallet) => match key.code {
            KeyCode::Esc => {
                app.send_form = SendFormData::new();
                app.state = AppState::Home;
            }
            code if code == KeyCode::Up || is_shortcut_key(&code, 'k') => {
                if app.send_form.wallet_index > 0 {
                    app.send_form.wallet_index -= 1;
                } else if !app.wallets.is_empty() {
                    app.send_form.wallet_index = app.wallets.len() - 1;
                }
            }
            code if (code == KeyCode::Down || is_shortcut_key(&code, 'j'))
                && !app.wallets.is_empty() =>
            {
                app.send_form.wallet_index = (app.send_form.wallet_index + 1) % app.wallets.len();
            }
            code if code == KeyCode::Down || is_shortcut_key(&code, 'j') => {}
            KeyCode::Enter => {
                if app.wallets.is_empty() {
                    app.send_form.error_message = Some("No wallets available".to_string());
                    return;
                }
                let wallet = &app.wallets[app.send_form.wallet_index];
                let wallet_name = wallet.name.clone();

                // Load wallet info for party selection
                let threshold = wallet.threshold.unwrap_or(2);
                let total_parties = wallet.total_parties.unwrap_or(3);

                // Load my party index from htss_metadata
                let state_dir = keygen::get_state_dir(&wallet_name);
                let my_index = if let Ok(storage) = FileStorage::new(&state_dir) {
                    if let Ok(bytes) = storage.read("htss_metadata.json") {
                        let json = String::from_utf8_lossy(&bytes);
                        serde_json::from_str::<serde_json::Value>(&json)
                            .ok()
                            .and_then(|v| v.get("my_index").and_then(|i| i.as_u64()))
                            .map(|i| i as u32)
                            .unwrap_or(1)
                    } else {
                        1
                    }
                } else {
                    1
                };

                // Initialize party selection
                app.send_form.threshold = threshold;
                app.send_form.total_parties = total_parties;
                app.send_form.my_party_index = my_index;
                app.send_form.selected_parties = vec![false; total_parties as usize];
                // Auto-select self
                if my_index > 0 && my_index <= total_parties {
                    app.send_form.selected_parties[(my_index - 1) as usize] = true;
                }
                app.send_form.party_selector_index = 0;

                // Load HTSS info from wallet
                app.send_form.hierarchical = wallet.hierarchical.unwrap_or(false);
                app.send_form.signing_requirement = wallet.signing_requirement.clone();
                app.send_form.party_ranks = wallet.party_ranks.clone().unwrap_or_default();

                app.state = AppState::Send(SendState::SelectSigners { wallet_name });
            }
            _ => {}
        },
        AppState::Send(SendState::SelectSigners { wallet_name }) => match key.code {
            KeyCode::Esc => {
                app.state = AppState::Send(SendState::SelectWallet);
            }
            code if code == KeyCode::Up || is_shortcut_key(&code, 'k') => {
                if app.send_form.party_selector_index > 0 {
                    app.send_form.party_selector_index -= 1;
                } else {
                    app.send_form.party_selector_index = app.send_form.total_parties as usize - 1;
                }
            }
            code if code == KeyCode::Down || is_shortcut_key(&code, 'j') => {
                app.send_form.party_selector_index =
                    (app.send_form.party_selector_index + 1) % app.send_form.total_parties as usize;
            }
            KeyCode::Char(' ') => {
                // Toggle party selection
                let idx = app.send_form.party_selector_index;
                if idx < app.send_form.selected_parties.len() {
                    let currently_selected = app.send_form.selected_parties[idx];
                    let selected_count = app.send_form.selected_count();

                    // If trying to select and already at threshold, don't allow
                    if !currently_selected && selected_count >= app.send_form.threshold as usize {
                        app.send_form.error_message = Some(format!(
                            "Cannot select more than {} parties",
                            app.send_form.threshold
                        ));
                        return;
                    }

                    app.send_form.selected_parties[idx] = !currently_selected;
                    app.send_form.error_message = None;
                }
            }
            KeyCode::Enter => {
                if let Some(error) = app.send_form.signer_selection_error() {
                    app.send_form.error_message = Some(error);
                    return;
                }
                app.send_form.error_message = None;

                // Load HD addresses for address selection
                let state_dir = keygen::get_state_dir(&wallet_name);
                let network = app.network.to_bitcoin_network();
                let (hd_enabled, hd_addresses) = match FileStorage::new(&state_dir) {
                    Ok(storage) => {
                        // Check if HD metadata exists and get derived_count
                        match storage.read("hd_metadata.json") {
                            Ok(bytes) => {
                                let hd_json = String::from_utf8_lossy(&bytes);
                                match serde_json::from_str::<keygen::HdMetadata>(&hd_json) {
                                    Ok(metadata) if metadata.hd_enabled => {
                                        match frostdao::btc::hd_address::list_derived_addresses(
                                            &storage,
                                            metadata.derived_count,
                                            network,
                                        ) {
                                            Ok(addrs) => (true, addrs),
                                            Err(_) => (true, Vec::new()),
                                        }
                                    }
                                    _ => (false, Vec::new()),
                                }
                            }
                            Err(_) => (false, Vec::new()),
                        }
                    }
                    Err(_) => (false, Vec::new()),
                };

                app.send_form.hd_enabled = hd_enabled;
                app.send_form.hd_addresses = hd_addresses;
                app.send_form.hd_selected_index = 0;
                app.send_form.use_hd_address = false;

                app.state = AppState::Send(SendState::SelectAddress {
                    wallet_name: wallet_name.clone(),
                });
            }
            _ => {}
        },
        AppState::Send(SendState::SelectAddress { wallet_name }) => match key.code {
            KeyCode::Esc => {
                app.state = AppState::Send(SendState::SelectSigners {
                    wallet_name: wallet_name.clone(),
                });
            }
            code if (code == KeyCode::Up || is_shortcut_key(&code, 'k'))
                && app.send_form.use_hd_address =>
            {
                if app.send_form.hd_selected_index > 0 {
                    app.send_form.hd_selected_index -= 1;
                } else {
                    // Wrap to root address
                    app.send_form.use_hd_address = false;
                }
            }
            code if code == KeyCode::Up || is_shortcut_key(&code, 'k') => {}
            code if code == KeyCode::Down || is_shortcut_key(&code, 'j') => {
                if !app.send_form.use_hd_address {
                    // At root, move to first HD address if available
                    if !app.send_form.hd_addresses.is_empty() {
                        app.send_form.use_hd_address = true;
                        app.send_form.hd_selected_index = 0;
                    }
                } else if app.send_form.hd_selected_index + 1 < app.send_form.hd_addresses.len() {
                    app.send_form.hd_selected_index += 1;
                }
            }
            KeyCode::Enter => {
                app.send_form.error_message = None;

                if let Some(message) = app.utxo_source_unavailable_message() {
                    app.set_message(&message);
                    app.send_form.error_message = Some(message);
                    return;
                }

                // Get the source address to fetch UTXOs
                let source_address = if app.send_form.use_hd_address {
                    app.send_form
                        .hd_addresses
                        .get(app.send_form.hd_selected_index)
                        .map(|(addr, _, _)| addr.clone())
                } else {
                    // Get root address
                    let state_dir = frostdao::protocol::keygen::get_state_dir(&wallet_name);
                    FileStorage::new(&state_dir).ok().and_then(|storage| {
                        storage.read("shared_key.bin").ok().and_then(|bytes| {
                            bincode::deserialize::<
                                schnorr_fun::frost::SharedKey<schnorr_fun::fun::marker::EvenY>,
                            >(&bytes)
                            .ok()
                            .and_then(|sk| {
                                let pubkey_bytes: [u8; 32] = sk.public_key().to_xonly_bytes();
                                let xonly =
                                    bitcoin::secp256k1::XOnlyPublicKey::from_slice(&pubkey_bytes)
                                        .ok()?;
                                let secp = bitcoin::secp256k1::Secp256k1::new();
                                Some(
                                    bitcoin::Address::p2tr(
                                        &secp,
                                        xonly,
                                        None,
                                        app.network.to_bitcoin_network(),
                                    )
                                    .to_string(),
                                )
                            })
                        })
                    })
                };

                // Fetch UTXOs and transactions for the source address
                if let Some(addr) = source_address {
                    app.fetch_utxos_for_send(&addr);
                }

                // Reset script config for new transaction
                app.send_form.script_config = crate::tui::screens::ScriptConfig::new();

                app.state = AppState::Send(SendState::ConfigureScript { wallet_name });
            }
            _ => {}
        },
        AppState::Send(SendState::ConfigureScript { wallet_name }) => match key.code {
            KeyCode::Esc => {
                app.state = AppState::Send(SendState::SelectAddress {
                    wallet_name: wallet_name.clone(),
                });
            }
            code if (code == KeyCode::Up || is_shortcut_key(&code, 'k'))
                && app.send_form.script_config.selected_index > 0 =>
            {
                app.send_form.script_config.selected_index -= 1;
            }
            code if code == KeyCode::Up || is_shortcut_key(&code, 'k') => {}
            code if code == KeyCode::Down || is_shortcut_key(&code, 'j') => {
                let max = crate::tui::screens::ScriptType::all().len();
                if app.send_form.script_config.selected_index + 1 < max {
                    app.send_form.script_config.selected_index += 1;
                }
            }
            KeyCode::Char(' ') => {
                // Toggle/select script type
                let types = crate::tui::screens::ScriptType::all();
                if let Some(selected) = types.get(app.send_form.script_config.selected_index) {
                    app.send_form.script_config.script_type = selected.clone();
                    app.send_form.script_config.focused_field = 0;
                }
            }
            KeyCode::Tab => {
                // Cycle through config fields based on script type
                use crate::tui::screens::{ScriptType, TimelockMode};
                let config = &mut app.send_form.script_config;

                let max_fields = match &config.script_type {
                    ScriptType::None => 0,
                    ScriptType::TimelockAbsolute => 1,
                    ScriptType::TimelockRelative => match config.timelock_mode {
                        TimelockMode::Blocks => 1, // blocks only
                        TimelockMode::Time => 2,   // days + hours
                    },
                    ScriptType::Recovery => 2,
                    ScriptType::Htlc => 3,
                };
                if max_fields > 0 {
                    config.focused_field = (config.focused_field + 1) % max_fields;
                }
            }
            code if is_shortcut_key(&code, 'm') => {
                // Toggle timelock mode (blocks vs time)
                use crate::tui::screens::ScriptType;
                if app.send_form.script_config.script_type == ScriptType::TimelockRelative {
                    app.send_form.script_config.timelock_mode =
                        app.send_form.script_config.timelock_mode.toggle();
                    // Reset to first input field after mode change
                    app.send_form.script_config.focused_field = 0;
                }
            }
            KeyCode::Char(_) | KeyCode::Backspace => {
                // Input to focused field
                use crate::tui::screens::{ScriptType, TimelockMode};
                let config = &mut app.send_form.script_config;
                match &config.script_type {
                    ScriptType::TimelockAbsolute if config.focused_field == 0 => {
                        config.timelock_height.handle_key(key);
                    }
                    ScriptType::TimelockAbsolute => {}
                    ScriptType::TimelockRelative => match config.timelock_mode {
                        TimelockMode::Blocks => {
                            if config.focused_field == 0 {
                                config.timelock_blocks.handle_key(key);
                            }
                        }
                        TimelockMode::Time => match config.focused_field {
                            0 => {
                                config.timelock_days.handle_key(key);
                            }
                            1 => {
                                config.timelock_hours.handle_key(key);
                            }
                            _ => {}
                        },
                    },
                    ScriptType::Recovery => match config.focused_field {
                        0 => {
                            config.recovery_timeout.handle_key(key);
                        }
                        1 => {
                            config.recovery_pubkey.handle_key(key);
                        }
                        _ => {}
                    },
                    ScriptType::Htlc => match config.focused_field {
                        0 => {
                            config.htlc_hash.handle_key(key);
                        }
                        1 => {
                            config.htlc_timeout.handle_key(key);
                        }
                        2 => {
                            config.htlc_refund_pubkey.handle_key(key);
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            KeyCode::Enter => {
                app.send_form.error_message = None;
                app.state = AppState::Send(SendState::EnterDetails { wallet_name });
            }
            _ => {}
        },
        AppState::Send(SendState::EnterDetails { wallet_name }) => match key.code {
            KeyCode::Esc => {
                app.state = AppState::Send(SendState::ConfigureScript {
                    wallet_name: wallet_name.clone(),
                });
            }
            KeyCode::Tab => {
                app.send_form.focused_field = app.send_form.focused_field.next();
            }
            KeyCode::BackTab => {
                app.send_form.focused_field = app.send_form.focused_field.prev();
            }
            KeyCode::Enter => {
                let to_addr = app.send_form.to_address.value().to_string();
                let amount: u64 = app.send_form.amount.value().parse().unwrap_or(0);

                if to_addr.is_empty() {
                    app.send_form.error_message = Some("Enter destination address".to_string());
                    return;
                }
                if amount == 0 {
                    app.send_form.error_message = Some("Enter valid amount".to_string());
                    return;
                }
                if let Some(fetch_error) = app.send_form.utxo_fetch_error.clone() {
                    app.send_form.error_message =
                        Some(format!("Cannot prepare transaction: {}", fetch_error));
                    return;
                }

                // Collect selected party indices (1-based)
                let selected_parties: Vec<u32> = app
                    .send_form
                    .selected_parties
                    .iter()
                    .enumerate()
                    .filter_map(
                        |(i, &selected)| {
                            if selected {
                                Some((i + 1) as u32)
                            } else {
                                None
                            }
                        },
                    )
                    .collect();

                if selected_parties.is_empty() {
                    app.send_form.error_message = Some("No parties selected".to_string());
                    return;
                }

                app.send_form.estimate_fee();
                let confirmed_balance: u64 = app
                    .send_form
                    .utxos
                    .iter()
                    .filter(|utxo| utxo.confirmed)
                    .map(|utxo| utxo.value)
                    .sum();
                if confirmed_balance == 0 {
                    app.send_form.error_message = Some(
                        "No confirmed UTXOs available for the selected source address".to_string(),
                    );
                    return;
                }
                let total_needed = amount.saturating_add(app.send_form.estimated_fee);
                if total_needed > confirmed_balance {
                    app.send_form.error_message = Some(format!(
                        "Insufficient confirmed balance: need {} sats, have {} sats",
                        total_needed, confirmed_balance
                    ));
                    return;
                }
                app.send_form.error_message = None;
                app.state = AppState::Send(SendState::ReviewTransaction { wallet_name });
            }
            _ => match app.send_form.focused_field {
                SendFormField::ToAddress => {
                    app.send_form.to_address.handle_key(key);
                }
                SendFormField::Amount => {
                    app.send_form.amount.handle_key(key);
                    // Recalculate fee estimate when amount changes
                    app.send_form.estimate_fee();
                }
            },
        },
        AppState::Send(SendState::ReviewTransaction { wallet_name }) => match key.code {
            KeyCode::Esc => {
                app.state = AppState::Send(SendState::EnterDetails {
                    wallet_name: wallet_name.clone(),
                });
            }
            code if is_shortcut_key(&code, 'y') => {
                if app.network == state::NetworkSelection::Mainnet {
                    app.send_form.error_message = Some(
                        "Mainnet send is blocked in the TUI; use CLI with explicit mainnet opt-in after offline review"
                            .to_string(),
                    );
                    return;
                }

                let to_addr = app.send_form.to_address.value().to_string();
                let amount: u64 = app.send_form.amount.value().parse().unwrap_or(0);
                let selected_parties = app.send_form.get_selected_indices();
                let network = app.network.to_bitcoin_network();
                let derivation_path = app.send_form.get_derivation_path();

                match frostdao::protocol::dkg_tx::frost_sign_all_local(
                    &wallet_name,
                    &to_addr,
                    amount,
                    &selected_parties,
                    derivation_path,
                    None,
                    network,
                ) {
                    Ok(result) => {
                        app.send_form.error_message = None;
                        let (txid, broadcast_status, raw_tx) = if let Ok(parsed) =
                            serde_json::from_str::<serde_json::Value>(&result.result)
                        {
                            (
                                parsed["txid"].as_str().unwrap_or("unknown").to_string(),
                                parsed["broadcast_status"].as_str().map(str::to_string),
                                parsed["raw_tx"].as_str().map(str::to_string),
                            )
                        } else {
                            (result.result.clone(), None, None)
                        };
                        app.state = AppState::Send(SendState::Complete {
                            txid,
                            broadcast_status,
                            raw_tx,
                        });
                    }
                    Err(e) => {
                        app.send_form.error_message = Some(format!("Error: {}", e));
                    }
                }
            }
            _ => {}
        },
        AppState::Send(SendState::ShowSighash {
            wallet_name,
            sighash,
            session_id,
            ..
        }) => match key.code {
            KeyCode::Esc => {
                app.state = AppState::Send(SendState::EnterDetails {
                    wallet_name: wallet_name.clone(),
                });
            }
            code if is_copy_key(&code) => {
                app.copy_to_clipboard(&sighash);
            }
            KeyCode::Enter => {
                // Generate nonce
                let state_dir = keygen::get_state_dir(&wallet_name);
                match FileStorage::new(&state_dir) {
                    Ok(storage) => match signing::generate_nonce_core(&session_id, &storage) {
                        Ok(result) => {
                            app.send_form.nonce_output = result.result.clone();
                            app.state = AppState::Send(SendState::GenerateNonce {
                                wallet_name,
                                session_id,
                                sighash,
                                nonce_output: result.result,
                            });
                        }
                        Err(e) => {
                            app.send_form.error_message = Some(format!("Error: {}", e));
                        }
                    },
                    Err(e) => {
                        app.send_form.error_message = Some(format!("Storage error: {}", e));
                    }
                }
            }
            _ => {}
        },
        AppState::Send(SendState::GenerateNonce {
            wallet_name,
            session_id,
            sighash,
            nonce_output,
        }) => match key.code {
            KeyCode::Esc => {
                app.state = AppState::Send(SendState::ShowSighash {
                    wallet_name,
                    sighash,
                    session_id,
                });
            }
            code if is_copy_key(&code) => {
                app.copy_to_clipboard(&nonce_output);
            }
            KeyCode::Enter => {
                // Pre-fill with my nonce
                app.send_form.nonces_input =
                    crate::tui::components::TextArea::new("Paste nonces from other parties");
                app.send_form.nonces_input.handle_paste(&nonce_output);
                app.state = AppState::Send(SendState::EnterNonces {
                    wallet_name,
                    session_id,
                    sighash,
                });
            }
            _ => {}
        },
        AppState::Send(SendState::EnterNonces {
            wallet_name,
            session_id,
            sighash,
        }) => match key.code {
            KeyCode::Esc => {
                app.state = AppState::Send(SendState::GenerateNonce {
                    wallet_name,
                    session_id,
                    sighash,
                    nonce_output: app.send_form.nonce_output.clone(),
                });
            }
            KeyCode::Enter => {
                let nonces_data = app.send_form.nonces_input.content();
                if nonces_data.trim().is_empty() {
                    app.send_form.error_message = Some("Paste nonces first".to_string());
                    return;
                }

                // Count nonces by looking for "party_index" occurrences
                let nonce_count = nonces_data.matches("\"party_index\"").count();
                let threshold = app.send_form.threshold as usize;

                if nonce_count < threshold {
                    app.send_form.error_message = Some(format!(
                        "Need {} nonces but only found {}. Collect more nonces from other signers!",
                        threshold, nonce_count
                    ));
                    return;
                }

                // Generate signature share (real FROST)
                let state_dir = keygen::get_state_dir(&wallet_name);
                match FileStorage::new(&state_dir) {
                    Ok(storage) => {
                        match signing::create_signature_share_core(
                            &session_id,
                            &sighash,
                            &nonces_data,
                            &storage,
                        ) {
                            Ok(result) => {
                                app.send_form.share_output = result.result.clone();
                                app.send_form.error_message = None;
                                app.state = AppState::Send(SendState::GenerateShare {
                                    wallet_name,
                                    share_output: result.result,
                                });
                            }
                            Err(e) => {
                                app.send_form.error_message = Some(format!("Error: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        app.send_form.error_message = Some(format!("Storage error: {}", e));
                    }
                }
            }
            _ => {
                app.send_form.nonces_input.handle_key(key);
            }
        },
        AppState::Send(SendState::GenerateShare {
            wallet_name,
            share_output,
        }) => match key.code {
            KeyCode::Esc => {
                // Non-aggregator done
                app.send_form = SendFormData::new();
                app.state = AppState::Home;
            }
            code if is_copy_key(&code) => {
                app.copy_to_clipboard(&share_output);
            }
            KeyCode::Enter => {
                // Go to aggregator mode
                app.send_form.shares_input = crate::tui::components::TextArea::new(
                    "Paste signature shares from other parties",
                );
                app.send_form.shares_input.handle_paste(&share_output);
                app.state = AppState::Send(SendState::CombineShares { wallet_name });
            }
            _ => {}
        },
        AppState::Send(SendState::CombineShares { wallet_name }) => match key.code {
            KeyCode::Esc => {
                app.state = AppState::Send(SendState::GenerateShare {
                    wallet_name,
                    share_output: app.send_form.share_output.clone(),
                });
            }
            KeyCode::Enter => {
                let shares_data = app.send_form.shares_input.content();
                if shares_data.trim().is_empty() {
                    app.send_form.error_message = Some("Paste signature shares first".to_string());
                    return;
                }

                // Count shares
                let share_count = shares_data.matches("\"party_index\"").count();
                let threshold = app.send_form.threshold as usize;

                if share_count < threshold {
                    app.send_form.error_message = Some(format!(
                        "Need {} shares but only found {}. Collect more shares!",
                        threshold, share_count
                    ));
                    return;
                }

                // Combine signatures (real FROST)
                let state_dir = keygen::get_state_dir(&wallet_name);
                match FileStorage::new(&state_dir) {
                    Ok(storage) => match signing::combine_signatures_core(&shares_data, &storage) {
                        Ok(result) => {
                            app.send_form.final_signature = result.result.clone();
                            app.send_form.error_message = None;
                            app.state = AppState::Send(SendState::Complete {
                                txid: result.result,
                                broadcast_status: None,
                                raw_tx: None,
                            });
                        }
                        Err(e) => {
                            app.send_form.error_message = Some(format!("Error: {}", e));
                        }
                    },
                    Err(e) => {
                        app.send_form.error_message = Some(format!("Storage error: {}", e));
                    }
                }
            }
            _ => {
                app.send_form.shares_input.handle_key(key);
            }
        },
        AppState::Send(SendState::Complete { txid, raw_tx, .. }) => match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                app.send_form = SendFormData::new();
                app.state = AppState::Home;
            }
            code if is_copy_key(&code) => {
                app.copy_to_clipboard(raw_tx.as_deref().unwrap_or(&txid));
            }
            _ => {}
        },
        _ => {}
    }
}

fn handle_address_list_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.state = AppState::Home;
        }
        code if code == KeyCode::Up || is_shortcut_key(&code, 'k') => {
            if let AppState::AddressList(ref mut state) = app.state {
                if state.selected > 0 {
                    state.selected -= 1;
                }
            }
        }
        code if code == KeyCode::Down || is_shortcut_key(&code, 'j') => {
            if let AppState::AddressList(ref mut state) = app.state {
                if state.selected + 1 < state.addresses.len() {
                    state.selected += 1;
                }
            }
        }
        code if is_copy_key(&code) => {
            // Copy selected address to clipboard
            let addr_to_copy = if let AppState::AddressList(ref state) = app.state {
                state.addresses.get(state.selected).map(|(addr, _, index)| {
                    (
                        addr.clone(),
                        state.network,
                        frostdao::crypto::hd::format_bip86_path(
                            state.network.to_bitcoin_network(),
                            0,
                            *index,
                        ),
                    )
                })
            } else {
                None
            };
            if let Some((addr, network, path)) = addr_to_copy {
                app.copy_to_clipboard_with_message(
                    &addr,
                    hd_address_copy_message(network, &path, &addr),
                );
            } else {
                app.set_message("No address selected to copy");
            }
        }
        code if is_refresh_key(&code) => {
            // Fetch balance for selected address
            let addr_info = if let AppState::AddressList(ref state) = app.state {
                state
                    .addresses
                    .get(state.selected)
                    .map(|(addr, _, idx)| (addr.clone(), *idx))
            } else {
                None
            };

            if let Some((addr, idx)) = addr_info {
                let network = if let AppState::AddressList(ref state) = app.state {
                    state.network
                } else {
                    app.network
                };
                app.set_message(&address_balance_fetching_message(network, idx));

                // Fetch balance from mempool.space where the selected network supports it.
                let api_base = match app.utxo_api_base_for_fetch("address balance") {
                    Ok(api_base) => api_base,
                    Err(err) => {
                        app.set_message(&err.to_string());
                        return;
                    }
                };
                let url = format!("{}/address/{}/utxo", api_base, addr);

                match reqwest::blocking::Client::new().get(&url).send() {
                    Ok(response) => match response.json::<Vec<serde_json::Value>>() {
                        Ok(utxos) => {
                            let balance: u64 = utxos
                                .iter()
                                .filter_map(|u| u.get("value").and_then(|v| v.as_u64()))
                                .sum();
                            let utxo_count = utxos.len();

                            if let AppState::AddressList(ref mut state) = app.state {
                                state.balance_cache.insert(idx, (balance, utxo_count));
                            }

                            let btc = balance as f64 / 100_000_000.0;
                            app.set_message(&address_balance_updated_message(
                                network, idx, balance, btc, utxo_count,
                            ));
                        }
                        Err(e) => {
                            app.set_message(&format!("Failed to parse response: {}", e));
                        }
                    },
                    Err(e) => {
                        app.set_message(&format!("Failed to fetch balance: {}", e));
                    }
                }
            }
        }
        KeyCode::Char('+') => {
            // Add new HD address
            let wallet_name = if let AppState::AddressList(ref state) = app.state {
                Some(state.wallet_name.clone())
            } else {
                None
            };
            if let Some(name) = wallet_name {
                app.add_hd_address(&name);
            }
        }
        code if is_shortcut_key(&code, 'a') => {
            // Add new HD address
            let wallet_name = if let AppState::AddressList(ref state) = app.state {
                Some(state.wallet_name.clone())
            } else {
                None
            };
            if let Some(name) = wallet_name {
                app.add_hd_address(&name);
            }
        }
        KeyCode::Char('-') => {
            // Remove last HD address
            let wallet_name = if let AppState::AddressList(ref state) = app.state {
                Some(state.wallet_name.clone())
            } else {
                None
            };
            if let Some(name) = wallet_name {
                app.remove_hd_address(&name);
            }
        }
        code if is_shortcut_key(&code, 'x') => {
            // Remove last HD address
            let wallet_name = if let AppState::AddressList(ref state) = app.state {
                Some(state.wallet_name.clone())
            } else {
                None
            };
            if let Some(name) = wallet_name {
                app.remove_hd_address(&name);
            }
        }
        _ => {}
    }
}

fn root_address_copy_message(network: state::NetworkSelection, address: &str) -> String {
    format!(
        "Copied {} root address: {}",
        network.display_name(),
        address
    )
}

fn hd_address_copy_message(network: state::NetworkSelection, path: &str, address: &str) -> String {
    format!(
        "Copied {} HD address {}: {}",
        network.display_name(),
        path,
        address
    )
}

fn address_balance_fetching_message(network: state::NetworkSelection, index: u32) -> String {
    format!(
        "Fetching {} balance for address {}...",
        network.display_name(),
        index
    )
}

fn address_balance_updated_message(
    network: state::NetworkSelection,
    index: u32,
    balance: u64,
    btc: f64,
    utxo_count: usize,
) -> String {
    format!(
        "{} address {}: {} sats ({:.8} BTC), {} UTXOs",
        network.display_name(),
        index,
        balance,
        btc,
        utxo_count
    )
}

fn handle_mnemonic_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.state = AppState::Home;
        }
        code if code == KeyCode::Up || is_shortcut_key(&code, 'k') => {
            if let AppState::MnemonicBackup(ref mut state) = app.state {
                if !state.party_selected && !state.available_parties.is_empty() {
                    if state.selected_party > 0 {
                        state.selected_party -= 1;
                    } else {
                        state.selected_party = state.available_parties.len() - 1;
                    }
                }
            }
        }
        code if code == KeyCode::Down || is_shortcut_key(&code, 'j') => {
            if let AppState::MnemonicBackup(ref mut state) = app.state {
                if !state.party_selected && !state.available_parties.is_empty() {
                    state.selected_party =
                        (state.selected_party + 1) % state.available_parties.len();
                }
            }
        }
        KeyCode::Enter => {
            if let AppState::MnemonicBackup(ref mut state) = app.state {
                if !state.party_selected {
                    // Party selected, show security warning
                    state.party_selected = true;
                } else if !state.revealed {
                    // Generate mnemonic from selected party's share
                    let wallet_name = state.wallet_name.clone();
                    let state_dir = keygen::get_state_dir(&wallet_name);
                    let party_idx = state
                        .available_parties
                        .get(state.selected_party)
                        .copied()
                        .unwrap_or(1);

                    // Party 0 = legacy (share in wallet root), otherwise in party subfolder
                    let share_dir = if party_idx == 0 {
                        state_dir.clone()
                    } else {
                        format!("{}/party{}", state_dir, party_idx)
                    };

                    match FileStorage::new(&share_dir) {
                        Ok(storage) => match storage.read("paired_secret_share.bin") {
                            Ok(bytes) => {
                                use schnorr_fun::frost::PairedSecretShare;
                                use schnorr_fun::fun::marker::EvenY;

                                let paired_share: PairedSecretShare<EvenY> =
                                    match bincode::deserialize(&bytes) {
                                        Ok(share) => share,
                                        Err(e) => {
                                            state.error =
                                                Some(format!("Corrupted wallet data: {}", e));
                                            return;
                                        }
                                    };
                                let share_bytes = paired_share.secret_share().share.to_bytes();

                                match frostdao::crypto::mnemonic::share_to_mnemonic(&share_bytes) {
                                    Ok(mnemonic) => {
                                        state.words =
                                            mnemonic.words().map(|s| s.to_string()).collect();
                                        state.revealed = true;
                                    }
                                    Err(e) => {
                                        state.error = Some(format!("Error: {}", e));
                                    }
                                }
                            }
                            Err(e) => {
                                state.error = Some(format!("Cannot read share: {}", e));
                            }
                        },
                        Err(e) => {
                            state.error = Some(format!("Storage error: {}", e));
                        }
                    }
                } else {
                    // Already revealed, go home
                    app.state = AppState::Home;
                }
            }
        }
        _ => {}
    }
}

fn handle_nostr_room_keys(app: &mut App, key: KeyEvent) {
    match app.nostr_room_phase {
        NostrRoomPhase::Configure => handle_nostr_room_configure(app, key),
        NostrRoomPhase::WaitingForParticipants => handle_nostr_room_waiting(app, key),
        NostrRoomPhase::Ready => handle_nostr_room_ready(app, key),
    }
}

fn handle_nostr_room_configure(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.state = AppState::Home;
        }
        KeyCode::Tab => {
            app.nostr_room_focus = match app.nostr_room_focus {
                NostrRoomField::RoomId => NostrRoomField::MyIndex,
                NostrRoomField::MyIndex => NostrRoomField::Threshold,
                NostrRoomField::Threshold => NostrRoomField::NParties,
                NostrRoomField::NParties => NostrRoomField::RoomId,
            };
        }
        KeyCode::BackTab => {
            app.nostr_room_focus = match app.nostr_room_focus {
                NostrRoomField::RoomId => NostrRoomField::NParties,
                NostrRoomField::MyIndex => NostrRoomField::RoomId,
                NostrRoomField::Threshold => NostrRoomField::MyIndex,
                NostrRoomField::NParties => NostrRoomField::Threshold,
            };
        }
        KeyCode::Enter => {
            // Validate and join room
            if let Some(error) = app.nostr_room_config_error() {
                app.set_message(error);
                return;
            }

            match app.join_nostr_room_runtime() {
                Ok(()) => {
                    app.nostr_room_phase = NostrRoomPhase::WaitingForParticipants;
                    app.message = Some(format!(
                        "Joined room '{}' as Party {} with {} transport",
                        app.nostr_room_id,
                        app.nostr_my_index,
                        app.nostr_transport_label()
                    ));
                }
                Err(e) => {
                    app.message = Some(format!("Nostr room error: {}", e));
                    return;
                }
            }

            // Check if already have all participants in local simulation.
            check_participants_ready(app);
        }
        KeyCode::Char(c) => match app.nostr_room_focus {
            NostrRoomField::RoomId => {
                app.nostr_room_id.push(c);
            }
            NostrRoomField::MyIndex => {
                if c.is_ascii_digit() {
                    let new_val = format!("{}{}", app.nostr_my_index, c);
                    if let Ok(n) = new_val.parse::<u32>() {
                        if n <= 99 {
                            app.nostr_my_index = n;
                        }
                    }
                }
            }
            NostrRoomField::Threshold => {
                if c.is_ascii_digit() {
                    let new_val = format!("{}{}", app.nostr_threshold, c);
                    if let Ok(n) = new_val.parse::<u32>() {
                        if n <= 99 {
                            app.nostr_threshold = n;
                        }
                    }
                }
            }
            NostrRoomField::NParties => {
                if c.is_ascii_digit() {
                    let new_val = format!("{}{}", app.nostr_n_parties, c);
                    if let Ok(n) = new_val.parse::<u32>() {
                        if n <= 99 {
                            app.nostr_n_parties = n;
                        }
                    }
                }
            }
        },
        KeyCode::Backspace => match app.nostr_room_focus {
            NostrRoomField::RoomId => {
                app.nostr_room_id.pop();
            }
            NostrRoomField::MyIndex => {
                let s = app.nostr_my_index.to_string();
                app.nostr_my_index = if s.len() > 1 {
                    s[..s.len() - 1].parse().unwrap_or(1)
                } else {
                    1
                };
            }
            NostrRoomField::Threshold => {
                let s = app.nostr_threshold.to_string();
                app.nostr_threshold = if s.len() > 1 {
                    s[..s.len() - 1].parse().unwrap_or(2)
                } else {
                    2
                };
            }
            NostrRoomField::NParties => {
                let s = app.nostr_n_parties.to_string();
                app.nostr_n_parties = if s.len() > 1 {
                    s[..s.len() - 1].parse().unwrap_or(3)
                } else {
                    3
                };
            }
        },
        _ => {}
    }
}

fn handle_nostr_room_waiting(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            // Leave room, go back to configure
            app.nostr_room_phase = NostrRoomPhase::Configure;
            app.leave_nostr_room_runtime();
            app.set_message("Left room");
        }
        KeyCode::Char(' ') => {
            if app.nostr_local_simulation_transport_active() {
                simulate_participant_join(app);
            } else {
                app.set_message(
                    "Relay room: wait for real participants; local simulation disabled",
                );
            }
        }
        _ => {}
    }
}

fn handle_nostr_room_ready(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            // Leave room
            app.nostr_room_phase = NostrRoomPhase::Configure;
            app.leave_nostr_room_runtime();
            app.set_message("Left room");
        }
        code if is_shortcut_key(&code, 'k') => {
            if !app.nostr_local_simulation_transport_active() {
                app.set_message(nostr_relay_keygen_blocked_message());
                return;
            }
            // Start Nostr keygen
            app.nostr_keygen_state = NostrKeygenState::ModeSelect;
            app.state = AppState::NostrKeygen;
        }
        code if is_shortcut_key(&code, 's') => {
            // Start Nostr signing
            app.nostr_sign_state = NostrSignState::SelectWallet;
            app.state = AppState::NostrSign;
        }
        _ => {}
    }
}

fn nostr_relay_keygen_blocked_message() -> &'static str {
    "Relay keygen is unavailable in this TUI flow; use local rehearsal or CLI keygen"
}

/// Check if all participants have joined, transition to Ready if so
fn check_participants_ready(app: &mut App) {
    if app.nostr_participants.len() >= app.nostr_n_parties as usize {
        app.nostr_room_phase = NostrRoomPhase::Ready;
        app.set_message("All participants ready!");
    }
}

/// Local simulation: simulate a participant joining.
fn simulate_participant_join(app: &mut App) {
    // Find next missing participant
    for i in 1..=app.nostr_n_parties {
        if !app.nostr_participants.contains_key(&i) {
            match app.simulate_nostr_participant_join(i) {
                Ok(()) => {
                    app.message = Some(format!("Party {} joined through runtime", i));
                    check_participants_ready(app);
                }
                Err(e) => {
                    app.message = Some(format!("Nostr runtime error: {}", e));
                }
            }
            return;
        }
    }
}

fn handle_nostr_keygen_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            // Go back to room
            app.nostr_keygen_state = NostrKeygenState::ModeSelect;
            app.state = AppState::NostrRoom;
        }
        KeyCode::Enter => {
            match &app.nostr_keygen_state {
                NostrKeygenState::ModeSelect => {
                    // Start DKG round 1
                    app.nostr_keygen_state = NostrKeygenState::WaitingForParties {
                        received_round1: std::collections::HashMap::new(),
                    };
                    app.set_message("Broadcasting Round 1...");
                }
                NostrKeygenState::WaitingForParties { received_round1 } => {
                    // Check if we have enough round 1 messages
                    if received_round1.len() >= app.nostr_n_parties as usize {
                        app.nostr_keygen_state = NostrKeygenState::Round2 {
                            received_round2: std::collections::HashMap::new(),
                        };
                        app.set_message("Processing Round 2...");
                    }
                }
                NostrKeygenState::Round2 { received_round2 } => {
                    // Check if we have enough round 2 messages
                    if received_round2.len() >= app.nostr_n_parties as usize {
                        app.nostr_keygen_state = NostrKeygenState::Finalizing;
                    }
                }
                NostrKeygenState::Finalizing => {
                    // Done, return to room
                    app.nostr_keygen_state = NostrKeygenState::ModeSelect;
                    app.state = AppState::NostrRoom;
                    app.set_message("DKG complete!");
                }
            }
        }
        code if is_shortcut_key(&code, 'r') => {
            // Retry - reset to mode select
            app.nostr_keygen_state = NostrKeygenState::ModeSelect;
        }
        _ => {}
    }
}

fn handle_nostr_sign_keys(app: &mut App, key: KeyEvent) {
    let code = key.code;
    match code {
        KeyCode::Esc => {
            // Go back based on current state
            match &app.nostr_sign_state {
                NostrSignState::SelectRole { .. } => {
                    app.nostr_sign_state = NostrSignState::SelectWallet;
                }
                NostrSignState::ConfigureTx { .. } | NostrSignState::ViewProposals { .. } => {
                    if let Some(wallet) = app.selected_wallet() {
                        app.nostr_sign_state = NostrSignState::SelectRole {
                            wallet_name: wallet.name.clone(),
                        };
                    } else {
                        app.nostr_sign_state = NostrSignState::SelectWallet;
                    }
                }
                NostrSignState::ReviewProposal { wallet_name, .. } => {
                    app.nostr_sign_state = NostrSignState::ViewProposals {
                        wallet_name: wallet_name.clone(),
                    };
                }
                NostrSignState::Complete { .. } => {
                    app.nostr_sign_state = NostrSignState::SelectWallet;
                    app.state = AppState::NostrRoom;
                }
                _ => {
                    // For other states, go back to room
                    app.nostr_sign_state = NostrSignState::SelectWallet;
                    app.state = AppState::NostrRoom;
                }
            }
        }
        KeyCode::Tab if matches!(app.nostr_sign_state, NostrSignState::ConfigureTx { .. }) => {
            app.nostr_tx_focus = app.nostr_tx_focus.next();
        }
        KeyCode::BackTab if matches!(app.nostr_sign_state, NostrSignState::ConfigureTx { .. }) => {
            app.nostr_tx_focus = app.nostr_tx_focus.prev();
        }
        KeyCode::Char(_)
        | KeyCode::Backspace
        | KeyCode::Delete
        | KeyCode::Left
        | KeyCode::Right
        | KeyCode::Home
        | KeyCode::End
            if matches!(app.nostr_sign_state, NostrSignState::ConfigureTx { .. }) =>
        {
            match app.nostr_tx_focus {
                NostrTxField::Recipient => {
                    app.nostr_to_address_input.handle_key(key);
                }
                NostrTxField::Amount => {
                    app.nostr_amount_input.handle_key(key);
                }
            }
        }
        KeyCode::Enter => {
            if matches!(
                app.nostr_sign_state,
                NostrSignState::WaitingForConsent { .. }
                    | NostrSignState::ViewProposals { .. }
                    | NostrSignState::WaitingForExecution { .. }
                    | NostrSignState::CollectingShares { .. }
                    | NostrSignState::Combining { .. }
            ) {
                if let Err(e) = app.poll_nostr_room_runtime() {
                    app.message = Some(format!("Nostr poll error: {}", e));
                    return;
                }
            }
            match &app.nostr_sign_state {
                NostrSignState::SelectWallet => {
                    // Select wallet and go to role selection
                    if let Some(wallet) = app.selected_wallet() {
                        let wallet_name = wallet.name.clone();
                        app.nostr_sign_state = NostrSignState::SelectRole { wallet_name };
                    } else {
                        app.set_message("Select a wallet first");
                    }
                }
                NostrSignState::SelectRole { wallet_name } => {
                    app.nostr_tx_focus = NostrTxField::Recipient;
                    app.nostr_sign_state = NostrSignState::ConfigureTx {
                        wallet_name: wallet_name.clone(),
                    };
                }
                NostrSignState::ConfigureTx { wallet_name } => {
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let proposal = match app.build_nostr_tx_proposal(wallet_name, timestamp) {
                        Ok(proposal) => proposal,
                        Err(e) => {
                            app.message = Some(format!("Cannot propose transaction: {}", e));
                            return;
                        }
                    };
                    let wallet_name = wallet_name.clone();
                    let session_id = proposal.session_id.clone();
                    if let Err(e) = app.publish_nostr_tx_proposal(&wallet_name, &proposal) {
                        app.message = Some(format!("Nostr proposal publish error: {}", e));
                        return;
                    }
                    app.nostr_sign_state = NostrSignState::WaitingForConsent {
                        wallet_name,
                        session_id,
                        proposal,
                        consents: std::collections::HashMap::new(),
                        rejections: std::collections::HashMap::new(),
                    };
                    app.set_message(
                        "Proposal published through room runtime; waiting for consents...",
                    );
                }
                NostrSignState::WaitingForConsent {
                    wallet_name,
                    session_id,
                    proposal,
                    consents,
                    rejections,
                } => {
                    // Check if we have enough consents (including proposer)
                    if consents.len() + 1 >= app.nostr_threshold as usize {
                        let wallet_name = wallet_name.clone();
                        let session_id = session_id.clone();
                        let sighash_fingerprint = proposal.review.sighash_fingerprint.clone();
                        if let Err(e) = app.start_nostr_signing_attempt(
                            &wallet_name,
                            &session_id,
                            &sighash_fingerprint,
                        ) {
                            app.message = Some(format!("Cannot start signing attempt: {}", e));
                            return;
                        }
                        app.nostr_sign_state = NostrSignState::CollectingShares {
                            wallet_name,
                            session_id,
                            received_shares: std::collections::HashMap::new(),
                        };
                        app.set_message("Threshold reached! Collecting signature shares...");
                    } else if app.nostr_n_parties.saturating_sub(rejections.len() as u32)
                        < app.nostr_threshold
                    {
                        app.set_message(
                            "Proposal cannot reach threshold after recorded rejections",
                        );
                    } else {
                        app.set_message("Waiting for more consents...");
                    }
                }
                NostrSignState::ViewProposals { wallet_name } => {
                    let wallet_name = wallet_name.clone();
                    let Some(proposal) = app
                        .nostr_pending_proposals
                        .values()
                        .find(|proposal| {
                            proposal.wallet_name == wallet_name
                                && proposal.proposer_index != app.nostr_my_index
                        })
                        .cloned()
                    else {
                        app.set_message("No pending proposals received");
                        return;
                    };
                    app.nostr_sign_state = NostrSignState::ReviewProposal {
                        wallet_name,
                        proposal,
                    };
                }
                NostrSignState::ReviewProposal { proposal, .. } => {
                    app.message = Some(format!(
                        "Review fingerprint {}, then press y to consent",
                        proposal.review.sighash_fingerprint
                    ));
                }
                NostrSignState::WaitingForExecution {
                    wallet_name,
                    session_id,
                } => {
                    // Transition to collecting shares when proposer initiates
                    let Some(proposal) = app.nostr_pending_proposals.get(session_id) else {
                        app.set_message("Cannot execute: proposal context is missing");
                        return;
                    };
                    let wallet_name = wallet_name.clone();
                    let session_id = session_id.clone();
                    let sighash_fingerprint = proposal.review.sighash_fingerprint.clone();
                    if let Err(e) = app.start_nostr_signing_attempt(
                        &wallet_name,
                        &session_id,
                        &sighash_fingerprint,
                    ) {
                        app.message = Some(format!("Cannot start signing attempt: {}", e));
                        return;
                    }
                    app.nostr_sign_state = NostrSignState::CollectingShares {
                        wallet_name,
                        session_id,
                        received_shares: std::collections::HashMap::new(),
                    };
                }
                NostrSignState::CollectingShares {
                    wallet_name,
                    session_id,
                    received_shares,
                } => {
                    let ready_to_combine = app
                        .nostr_signing_coordinators
                        .get(session_id)
                        .is_some_and(|coordinator| coordinator.ready_to_combine());
                    if ready_to_combine {
                        app.nostr_sign_state = NostrSignState::Combining {
                            wallet_name: wallet_name.clone(),
                            session_id: session_id.clone(),
                        };
                        app.set_message(
                            "Threshold reached; waiting for real transaction broadcast...",
                        );
                    } else if received_shares.len() >= app.nostr_threshold as usize {
                        app.set_message("Waiting for nonce-checked coordinator threshold...");
                    } else {
                        app.set_message("Waiting for more shares...");
                    }
                }
                NostrSignState::Combining { .. } => {
                    app.set_message(
                        "Waiting for real tx_broadcast; use CLI broadcast until TUI signing is wired",
                    );
                }
                NostrSignState::Complete { .. } => {
                    // Done, go back to room
                    app.nostr_sign_state = NostrSignState::SelectWallet;
                    app.state = AppState::NostrRoom;
                }
            }
        }
        code if code == KeyCode::Up || is_shortcut_key(&code, 'k') => {
            if matches!(app.nostr_sign_state, NostrSignState::SelectWallet) {
                app.prev_wallet();
            }
        }
        code if code == KeyCode::Down || is_shortcut_key(&code, 'j') => {
            if matches!(app.nostr_sign_state, NostrSignState::SelectWallet) {
                app.next_wallet();
            }
        }
        code if is_shortcut_key(&code, 'p') => {
            // Select Propose role
            if let NostrSignState::SelectRole { wallet_name } = &app.nostr_sign_state {
                app.nostr_tx_focus = NostrTxField::Recipient;
                app.nostr_sign_state = NostrSignState::ConfigureTx {
                    wallet_name: wallet_name.clone(),
                };
            }
        }
        code if is_copy_key(&code) => {
            // Select Consent role or copy TXID
            match &app.nostr_sign_state {
                NostrSignState::SelectRole { wallet_name } => {
                    app.nostr_sign_state = NostrSignState::ViewProposals {
                        wallet_name: wallet_name.clone(),
                    };
                }
                NostrSignState::Complete { txid } => {
                    // Copy TXID to clipboard
                    let txid = txid.clone();
                    app.copy_to_clipboard(&txid);
                }
                _ => {}
            }
        }
        code if is_shortcut_key(&code, 'r') => {
            if let NostrSignState::ReviewProposal {
                wallet_name,
                proposal,
            } = &app.nostr_sign_state
            {
                let wallet_name = wallet_name.clone();
                let proposal = proposal.clone();
                let fingerprint = proposal.review.sighash_fingerprint.clone();
                if let Err(e) = app.publish_nostr_tx_consent(
                    &wallet_name,
                    &proposal,
                    false,
                    Some("Rejected in TUI".to_string()),
                ) {
                    app.message = Some(format!("Nostr rejection publish error: {}", e));
                    return;
                }
                app.nostr_sign_state = NostrSignState::ViewProposals { wallet_name };
                app.set_message(&format!(
                    "Rejection sent for proposal fingerprint {}",
                    fingerprint
                ));
            }
        }
        code if is_shortcut_key(&code, 'y') => {
            if let NostrSignState::ReviewProposal {
                wallet_name,
                proposal,
            } = &app.nostr_sign_state
            {
                let wallet_name = wallet_name.clone();
                let proposal = proposal.clone();
                let session_id = proposal.session_id.clone();
                let fingerprint = proposal.review.sighash_fingerprint.clone();
                if let Err(e) = app.publish_nostr_tx_consent(&wallet_name, &proposal, true, None) {
                    app.message = Some(format!("Nostr consent publish error: {}", e));
                    return;
                }
                app.nostr_sign_state = NostrSignState::WaitingForExecution {
                    wallet_name,
                    session_id,
                };
                app.message = Some(format!(
                    "Consent sent after reviewing fingerprint {}",
                    fingerprint
                ));
            }
        }
        _ => {}
    }
}

fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Title + network status
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Help bar
        ])
        .split(frame.area());

    // Title with network indicator
    render_title(frame, app, chunks[0]);

    // Main content based on state
    match &app.state {
        AppState::Home => screens::render_home(frame, app, chunks[1]),
        AppState::WalletDetails(state) => {
            screens::render_wallet_details(frame, app, state, chunks[1])
        }
        AppState::ChainSelect => {
            screens::render_home(frame, app, chunks[1]);
            screens::render_chain_select(frame, app, frame.area());
        }
        AppState::Keygen(_) => screens::render_keygen(frame, app, &app.keygen_form, chunks[1]),
        AppState::Reshare(_) => screens::render_reshare(frame, app, &app.reshare_form, chunks[1]),
        AppState::Send(_) => screens::render_send(frame, app, &app.send_form, chunks[1]),
        AppState::AddressList(state) => screens::render_address_list(frame, state, chunks[1]),
        AppState::MnemonicBackup(state) => screens::render_mnemonic(frame, state, chunks[1]),
        AppState::NostrRoom => screens::render_nostr_room(frame, app, chunks[1]),
        AppState::NostrKeygen => screens::render_nostr_keygen(frame, app, chunks[1]),
        AppState::NostrSign => screens::render_nostr_sign(frame, app, chunks[1]),
        #[cfg(feature = "miniscript-policy")]
        AppState::PolicyPreview => {
            screens::render_policy_preview(frame, app, &app.policy_preview_form, chunks[1])
        }
    }

    // Help bar
    render_help_bar(frame, app, chunks[2]);

    if app.show_global_help {
        render_global_help_overlay(frame, app);
    }
}

fn render_title(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let network_color = match app.network {
        state::NetworkSelection::Testnet4 => Color::Yellow,
        state::NetworkSelection::Testnet3 => Color::LightYellow,
        state::NetworkSelection::Signet => Color::Magenta,
        state::NetworkSelection::Regtest => Color::Cyan,
        state::NetworkSelection::Mainnet => Color::Red,
    };

    let status_line = network_title_status_line(app.network);
    let status_style = if app.network == state::NetworkSelection::Mainnet {
        Color::Red
    } else if app.network == state::NetworkSelection::Regtest {
        Color::Cyan
    } else {
        Color::Yellow
    };

    let title = vec![
        Line::from(vec![
            Span::styled(
                "FrostDAO - DKG Wallet Manager",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("[", Style::default().fg(Color::Gray)),
            Span::styled(
                app.network.display_name(),
                Style::default().fg(network_color),
            ),
            Span::styled("]", Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::styled("Policy: ", Style::default().fg(Color::Gray)),
            Span::styled(status_line, Style::default().fg(status_style)),
        ]),
    ];

    let paragraph = Paragraph::new(title).block(Block::default().borders(Borders::ALL));

    frame.render_widget(paragraph, area);
}

fn network_title_status_line(network: state::NetworkSelection) -> String {
    match network {
        state::NetworkSelection::Regtest => match network.mempool_api_base() {
            Ok(_) => network.policy_hint().to_string(),
            Err(err) => err.to_string(),
        },
        _ => network.policy_hint().to_string(),
    }
}

fn render_help_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let help_text = help_bar_text(app);

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL).title("Help"));

    frame.render_widget(help, area);
}

fn render_global_help_overlay(frame: &mut Frame, app: &App) {
    let popup_area = centered_popup_rect(76, 86, frame.area());

    frame.render_widget(Clear, popup_area);

    let help = Paragraph::new(global_help_lines(app))
        .style(Style::default().fg(Color::Gray))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("FrostDAO Help (F1, Esc)"),
        )
        .wrap(ratatui::widgets::Wrap { trim: true });

    frame.render_widget(help, popup_area);
}

fn centered_popup_rect(
    width_percent: u16,
    height_percent: u16,
    area: ratatui::layout::Rect,
) -> Rect {
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .split(area);
    let body = if horizontal.len() == 3 {
        horizontal[1]
    } else {
        area
    };
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_percent) / 2),
            Constraint::Percentage(height_percent),
            Constraint::Percentage((100 - height_percent) / 2),
        ])
        .split(body);
    if vertical.len() == 3 {
        vertical[1]
    } else {
        area
    }
}

fn global_help_lines(app: &App) -> Vec<Line<'static>> {
    let copy = COPY_KEY_LABEL;
    let refresh = REFRESH_KEY_LABEL;
    #[cfg(feature = "miniscript-policy")]
    let home_policy_line = " | p:Policy".to_string();
    #[cfg(not(feature = "miniscript-policy"))]
    let home_policy_line = String::new();

    let mut lines = vec![
        Line::from(Span::styled(
            "Global",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("F1:Show help | Esc:Close | q:Quit (Home only)"),
        Line::from(""),
        Line::from("Home"),
        Line::from(format!(
            "j/k/↑/↓:Navigate | Enter:Select | g:New wallet | n:Network | h:Reshare | s:Send{home_policy_line}"
        )),
        format!("o:Nostr (or N) | a:Address list | m:Mnemonic backup | {copy}:Copy address").into(),
        format!("{refresh} (Refresh):Refresh balances | R:Reload wallets").into(),
        Line::from(""),
        Line::from("Wallet Details"),
        Line::from("j/k/↑/↓:Select action | Enter:Continue | v:QR"),
        format!("{copy}:Copy selected address | {refresh} (Refresh):Refresh selected").into(),
        Line::from(""),
        Line::from("Address List"),
        format!("j/k/↑/↓:Navigate | a:Add | x:Remove | {copy}:Copy | {refresh} (Refresh):Refresh",)
            .into(),
    ];

    match &app.state {
        AppState::Keygen(state) => {
            lines.push(Line::from("Keygen"));
            lines.push(Line::from(keygen_help_bar_text(app, state)));
            match state {
                KeygenState::ParamsSetup => lines.push(Line::from(
                    "↑/↓/j/k/1/2:Select | Tab/Enter to move | Esc:Back",
                )),
                KeygenState::Round1Output { .. } | KeygenState::Round2Output { .. } => {
                    lines.push(Line::from(format!(
                        "{copy}:Copy outputs/JSON | Ctrl+u:Clear JSON"
                    )));
                }
                _ => {}
            }
        }
        AppState::Reshare(state) => {
            lines.push(Line::from("Reshare"));
            lines.push(Line::from(reshare_help_bar_text(state)));
            lines.push(Line::from("↑/↓/j/k:Select | Tab/Enter/Space/Ctrl+u"));
        }
        AppState::Send(state) => {
            lines.push(Line::from("Send"));
            lines.push(Line::from(send_help_bar_text(state)));
            lines.push(Line::from("↑/↓/j/k:Navigate | Tab/Space/Ctrl+u"));
        }
        _ => {
            lines.push(Line::from("Flow"));
            lines.push(Line::from(
                "Keygen / Reshare / Send are available from home. Enter them to see flow actions.",
            ));
        }
    }

    lines.push(Line::from(format!(
        "{copy}:Copy outputs/JSON | Ctrl+u:Clear form (text fields)"
    )));
    lines.push(Line::from(""));
    lines.push(Line::from("Nostr"));
    lines.push(Line::from(
        "k/K:Start keygen (local rehearsal in local mode) | s/S:Sign | Esc:Leave room",
    ));
    lines.push(Line::from(format!(
        "p/P:Propose | {copy}:Consent role / copy TXID in complete | y:Consent | r/R:Reject"
    )));

    #[cfg(feature = "miniscript-policy")]
    {
        lines.push(Line::from(""));
        lines.push(Line::from("Policy Preview"));
        lines.push(Line::from("Tab:Next field | Enter:Init draft | Esc:Back"));
        lines.push(format!("[/]:Select template | {copy}:Copy draft").into());
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Networks supported"));
    lines.push(Line::from(
        "Testnet4/Testnet3/Signet use remote mempool; Regtest uses FROSTDAO_REGTEST_MEMPOOL_API; Mainnet requires explicit opt-in.",
    ));

    lines
}

fn help_bar_text(app: &App) -> String {
    if let Some(msg) = &app.message {
        return msg.clone();
    }
    let copy = COPY_KEY_LABEL;
    let refresh = REFRESH_KEY_LABEL;

    match &app.state {
        AppState::Home => home_help_bar_text(),
        AppState::WalletDetails(state) => {
            if state.confirm_delete {
                "Type wallet name | Enter:Delete | Backspace:Edit | Esc:Cancel".to_string()
            } else {
                format!(
                    "j/k/↑/↓:Navigate | Enter:Select | {refresh} (Refresh):Balance | {copy}:Copy | v:QR | Esc:Back"
                )
                .to_string()
            }
        }
        AppState::ChainSelect => "j/k/↑/↓:Select | Enter:Confirm | Esc:Cancel".to_string(),
        AppState::Keygen(state) => keygen_help_bar_text(app, state),
        AppState::Reshare(state) => reshare_help_bar_text(state),
        AppState::Send(state) => send_help_bar_text(state),
        AppState::AddressList(_) => {
            format!(
                "j/k/↑/↓:Navigate | {copy}:Copy | {refresh} (Refresh):Refresh | a:Add | x:Remove | Esc:Back"
            )
            .to_string()
        }
        AppState::MnemonicBackup(state) => {
            if state.revealed {
                "Enter:Done | Esc:Back".to_string()
            } else if !state.party_selected {
                "j/k/↑/↓:Select party | Enter:Continue | Esc:Cancel".to_string()
            } else {
                "Enter:Reveal | Esc:Cancel".to_string()
            }
        }
        AppState::NostrRoom => match app.nostr_room_phase {
            NostrRoomPhase::Configure => "Tab:Next | Enter:Join | Esc:Back".to_string(),
            NostrRoomPhase::WaitingForParticipants => {
                "Space:Add local test participant | Esc:Leave".to_string()
            }
            NostrRoomPhase::Ready => {
                if app.nostr_local_simulation_transport_active() {
                    "k:Local keygen | s:Sign | Esc:Leave".to_string()
                } else {
                    "k:Relay keygen unavailable | s:Sign | Esc:Leave".to_string()
                }
            }
        },
        AppState::NostrKeygen => "Enter:Continue | r:Retry | Esc:Cancel".to_string(),
        AppState::NostrSign => screens::nostr_sign_help_text(&app.nostr_sign_state),
        #[cfg(feature = "miniscript-policy")]
        AppState::PolicyPreview => {
            format!("Enter:Init draft | [/]:Template | Tab:Field | {copy}:Copy | Esc:Back").to_string()
        }
    }
}

fn keygen_help_bar_text(app: &App, state: &KeygenState) -> String {
    let copy = COPY_KEY_LABEL;

    match state {
        KeygenState::ModeSelect => "j/k/↑/↓/1/2:Select | Enter:Continue | Esc:Cancel".to_string(),
        KeygenState::ParamsSetup => {
            if app.keygen_form.hierarchical {
                "Tab:Next field | Enter:Generate | Esc:Back | Ctrl+u:Clear field".to_string()
            } else {
                "Tab:Next field | Enter:Generate All Parties | Esc:Back | Ctrl+u:Clear field"
                    .to_string()
            }
        }
        KeygenState::Round1Output { .. } => {
            format!("{copy}:Copy to clipboard | Enter:Continue to Round 2 | Esc:Cancel")
        }
        KeygenState::Round2Input => {
            "Paste/type JSON | Ctrl+u:Clear | Enter:Generate Shares | Esc:Back".to_string()
        }
        KeygenState::Round2Output { .. } => {
            format!("{copy}:Copy to clipboard | Enter:Continue to Finalize | Esc:Cancel")
        }
        KeygenState::FinalizeInput => {
            "Paste/type JSON | Ctrl+u:Clear | Enter:Finalize Wallet | Esc:Back".to_string()
        }
        KeygenState::Complete { .. } => "Enter/Esc:Return to wallet list".to_string(),
    }
}

fn reshare_help_bar_text(state: &ReshareState) -> String {
    let copy = COPY_KEY_LABEL;

    match state {
        ReshareState::ModeSelect => "j/k/↑/↓:Select | Enter:Continue | Esc:Cancel".to_string(),
        ReshareState::LocalSetup => {
            "Tab:Next | j/k/↑/↓:Select wallet | Enter:Reshare | Esc:Back".to_string()
        }
        ReshareState::LocalComplete { .. } => "Enter/Esc:Return to wallet list".to_string(),
        ReshareState::Round1Setup => {
            "Tab:Next | j/k/↑/↓:Select wallet | Enter:Generate | Esc:Cancel".to_string()
        }
        ReshareState::Round1Output { .. } => {
            format!("{copy}:Copy | Enter:Go to Finalize (if new party) | Esc:Done (if old party)")
        }
        ReshareState::FinalizeInput => {
            "Tab:Next | Space:Toggle | Ctrl+u:Clear | Enter:Finalize | Esc:Back".to_string()
        }
        ReshareState::Complete { .. } => "Enter/Esc:Return to wallet list".to_string(),
    }
}

fn send_help_bar_text(state: &SendState) -> String {
    let copy = COPY_KEY_LABEL;

    match state {
        SendState::SelectWallet => {
            "j/k/↑/↓:Select wallet | Enter:Continue | Esc:Cancel".to_string()
        }
        SendState::SelectSigners { .. } => {
            "j/k/↑/↓:Navigate | Space:Toggle | Enter:Continue | Esc:Back".to_string()
        }
        SendState::SelectAddress { .. } => {
            "j/k/↑/↓:Navigate | Enter:Continue | Esc:Back".to_string()
        }
        SendState::ConfigureScript { .. } => {
            "j/k/↑/↓:Select type | Tab:Next field | Space:Toggle | Enter:Continue | Esc:Back"
                .to_string()
        }
        SendState::EnterDetails { .. } => {
            "Tab:Next field | Enter:Prepare TX | Esc:Back".to_string()
        }
        SendState::ReviewTransaction { .. } => "y:Confirm and sign | Esc:Back to edit".to_string(),
        SendState::ShowSighash { .. } => {
            format!("{copy}:Copy | Enter:Generate Nonce | Esc:Back")
        }
        SendState::GenerateNonce { .. } => {
            format!("{copy}:Copy | Enter:Collect nonces from others | Esc:Back")
        }
        SendState::EnterNonces { .. } => {
            "Paste/type JSON | Ctrl+u:Clear | Enter:Generate Share | Esc:Back".to_string()
        }
        SendState::GenerateShare { .. } => {
            format!("{copy}:Copy | Enter:Combine (Aggregator) | Esc:Done")
        }
        SendState::CombineShares { .. } => {
            "Paste/type JSON | Ctrl+u:Clear | Enter:Combine | Esc:Back".to_string()
        }
        SendState::Complete { .. } => {
            format!("Enter/Esc:Return to home | {copy}:Copy raw tx")
        }
    }
}

fn home_help_bar_text() -> String {
    #[cfg(feature = "miniscript-policy")]
    {
        format!(
            "j/k/↑/↓:Navigate | Enter:Select | g:New | n:Network | o:Nostr | p:Policy | {0} (Refresh):Balance | {1}:Copy | q:Quit | F1:Help",
            REFRESH_KEY_LABEL,
            COPY_KEY_LABEL
        )
        .to_string()
    }
    #[cfg(not(feature = "miniscript-policy"))]
    {
        format!(
            "j/k/↑/↓:Navigate | Enter:Select | g:New | n:Network | o:Nostr (or N) | {0} (Refresh):Balance | {1}:Copy | q:Quit | F1:Help",
            REFRESH_KEY_LABEL,
            COPY_KEY_LABEL
        )
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::{NetworkSelection, TxProposal};
    use crossterm::event::KeyModifiers;
    use frostdao::protocol::keygen::WalletSummary;
    use serial_test::serial;
    use std::collections::BTreeMap;

    fn line_to_string(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<Vec<_>>()
            .join("")
    }

    fn lines_to_string(lines: &[Line<'_>]) -> String {
        lines
            .iter()
            .map(line_to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn enter_key() -> KeyEvent {
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
    }

    fn reviewable_proposal() -> TxProposal {
        TxProposal {
            session_id: "session-review-reject".to_string(),
            wallet_name: "wallet-test".to_string(),
            proposer_index: 2,
            to_address: "tb1qrecipient".to_string(),
            amount_sats: 50_000,
            fee_rate: 10,
            sighash: "abc123".to_string(),
            unsigned_tx: "02000000000100".to_string(),
            review: frostdao::nostr::TxReviewPayload {
                network: "Testnet3".to_string(),
                source_path: "m/86'/1'/0'/0/0".to_string(),
                from_address: "tb1qfrom".to_string(),
                to_address: "tb1qrecipient".to_string(),
                amount_sats: 50_000,
                fee_rate_sats_vb: 10,
                sighash_fingerprint: "abc12345".to_string(),
            },
            description: "test proposal".to_string(),
            timestamp: 1_700_000_000,
        }
    }

    fn wallet_summary_no_address(name: &str) -> WalletSummary {
        WalletSummary {
            name: name.to_string(),
            threshold: Some(2),
            total_parties: Some(3),
            hierarchical: Some(false),
            address: None,
            address_testnet: None,
            address_mainnet: None,
            address_regtest: None,
            signing_requirement: None,
            party_ranks: Some(BTreeMap::new()),
        }
    }

    #[test]
    fn home_help_bar_exposes_balance_refresh_shortcut() {
        let app = App::new().unwrap();
        let help = help_bar_text(&app);

        assert!(help.contains("j/k/↑/↓"));
        assert!(help.contains("b/r/F5 (Refresh):Balance"));
        assert!(help.contains("Enter:Select"));
        assert!(help.contains("c/C:Copy"));
        assert!(!help.contains("r:Balance"));
    }

    #[test]
    fn home_help_bar_uses_nostr_aliases() {
        let app = App::new().unwrap();
        let help = help_bar_text(&app);

        assert!(help.contains("o:Nostr"));
        #[cfg(feature = "miniscript-policy")]
        assert!(help.contains("p:Policy"));
        #[cfg(not(feature = "miniscript-policy"))]
        assert!(!help.contains("p:Policy"));
    }

    #[test]
    fn wallet_details_help_bar_includes_refresh_shortcut() {
        let mut app = App::new().unwrap();
        app.state = AppState::WalletDetails(WalletDetailsState {
            wallet_name: "treasury".to_string(),
            selected_action: 0,
            confirm_delete: false,
            delete_confirmation_input: String::new(),
            show_qr: false,
        });

        let help = help_bar_text(&app);

        assert!(help.contains("b/r/F5 (Refresh):Balance"));
        assert!(help.contains("v:QR"));
        assert!(!help.contains("r:Balance"));
    }

    #[test]
    #[serial]
    fn home_send_action_blocks_without_utxo_source() {
        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);

        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Regtest;
        app.wallets = vec![wallet_summary_no_address("treasury")];
        app.wallet_list_state.select(Some(0));

        handle_home_keys(&mut app, KeyCode::Char('s'));

        match app.state {
            AppState::Home => {}
            _ => panic!("expected AppState::Home"),
        }
        assert!(app
            .message
            .as_deref()
            .unwrap_or("")
            .contains("Cannot fetch UTXOs on Regtest"));

        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);
    }

    #[test]
    fn home_and_detail_shortcuts_are_case_insensitive() {
        assert!(is_copy_key(&KeyCode::Char('c')));
        assert!(is_copy_key(&KeyCode::Char('C')));

        assert!(is_refresh_key(&KeyCode::Char('b')));
        assert!(is_refresh_key(&KeyCode::Char('B')));
        assert!(is_refresh_key(&KeyCode::Char('r')));
        assert!(is_refresh_key(&KeyCode::Char('R')));
        assert!(is_refresh_key(&KeyCode::F(5)));

        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Signet;
        app.chain_selector_index = app.network.index();

        handle_home_keys(&mut app, KeyCode::Char('N'));
        assert!(matches!(app.state, AppState::ChainSelect));
        assert_eq!(app.chain_selector_index, NetworkSelection::Signet.index());

        app.network = NetworkSelection::Mainnet;
        app.state = AppState::WalletDetails(WalletDetailsState {
            wallet_name: "treasury".to_string(),
            selected_action: 0,
            confirm_delete: false,
            delete_confirmation_input: String::new(),
            show_qr: false,
        });

        handle_wallet_details_keys(&mut app, KeyCode::Char('V'));
        if let AppState::WalletDetails(ref state) = app.state {
            assert!(state.show_qr);
        } else {
            panic!("expected AppState::WalletDetails");
        }

        handle_wallet_details_keys(&mut app, KeyCode::Char('Q'));
        if let AppState::WalletDetails(ref state) = app.state {
            assert!(!state.show_qr);
        } else {
            panic!("expected AppState::WalletDetails");
        }

        assert!(is_shortcut_key(&KeyCode::Char('Y'), 'y'));
        assert!(is_shortcut_key(&KeyCode::Char('y'), 'Y'));
        assert!(!is_shortcut_key(&KeyCode::Char('2'), 'y'));
    }

    #[test]
    #[serial]
    fn wallet_details_send_action_blocks_without_utxo_source() {
        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);

        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Regtest;
        app.wallets = vec![wallet_summary_no_address("treasury")];
        app.state = AppState::WalletDetails(WalletDetailsState {
            wallet_name: "treasury".to_string(),
            selected_action: 0,
            confirm_delete: false,
            delete_confirmation_input: String::new(),
            show_qr: false,
        });

        handle_wallet_details_keys(&mut app, KeyCode::Enter);

        assert!(matches!(
            app.state,
            AppState::WalletDetails(WalletDetailsState {
                wallet_name, ..
            }) if wallet_name == "treasury"
        ));
        assert!(app
            .message
            .as_deref()
            .unwrap_or("")
            .contains("Cannot fetch UTXOs on Regtest"));

        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);
    }

    #[test]
    fn wallet_details_copy_reports_missing_address() {
        let mut app = App::new().unwrap();
        app.wallets = vec![wallet_summary_no_address("treasury")];
        app.state = AppState::WalletDetails(WalletDetailsState {
            wallet_name: "treasury".to_string(),
            selected_action: 0,
            confirm_delete: false,
            delete_confirmation_input: String::new(),
            show_qr: false,
        });

        handle_wallet_details_keys(&mut app, KeyCode::Char('c'));

        assert_eq!(
            app.message.as_deref(),
            Some("No address available to copy for this wallet")
        );
    }

    #[test]
    #[serial]
    fn address_list_uppercase_add_remove_triggers_same_paths() {
        let original_dir = std::env::current_dir().unwrap();
        let temp_dir =
            std::env::temp_dir().join(format!("frostdao-test-address-list-{}", std::process::id()));

        std::fs::create_dir_all(&temp_dir).unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        let mut app = App::new().unwrap();
        app.state = AppState::AddressList(AddressListState {
            wallet_name: "wallet-case-test".to_string(),
            network: NetworkSelection::Testnet4,
            addresses: vec![("tb1qtest".to_string(), "02".to_string(), 0)],
            selected: 0,
            error: None,
            hd_enabled: true,
            balance_cache: std::collections::HashMap::new(),
        });

        handle_address_list_keys(&mut app, KeyCode::Char('A'));
        assert!(app
            .message
            .as_deref()
            .unwrap_or("")
            .starts_with("Error adding address"));

        handle_address_list_keys(&mut app, KeyCode::Char('X'));
        assert!(app
            .message
            .as_deref()
            .unwrap_or("")
            .starts_with("Error removing address"));

        std::env::set_current_dir(original_dir).unwrap();
        let _ = std::fs::remove_dir_all(temp_dir.join(".frost_state"));
    }

    #[test]
    fn address_list_navigation_is_case_insensitive() {
        let mut app = App::new().unwrap();
        app.state = AppState::AddressList(AddressListState {
            wallet_name: "wallet-case-test".to_string(),
            network: NetworkSelection::Testnet4,
            addresses: vec![
                ("tb1qtest1".to_string(), "02".to_string(), 0),
                ("tb1qtest2".to_string(), "03".to_string(), 1),
            ],
            selected: 1,
            error: None,
            hd_enabled: true,
            balance_cache: std::collections::HashMap::new(),
        });

        handle_address_list_keys(&mut app, KeyCode::Char('K'));
        assert_eq!(
            if let AppState::AddressList(state) = &app.state {
                state.selected
            } else {
                unreachable!()
            },
            0
        );

        handle_address_list_keys(&mut app, KeyCode::Char('J'));
        assert_eq!(
            if let AppState::AddressList(state) = &app.state {
                state.selected
            } else {
                unreachable!()
            },
            1
        );
    }

    #[test]
    fn address_list_help_bar_includes_refresh_shortcut() {
        let mut app = App::new().unwrap();
        app.state = AppState::AddressList(AddressListState {
            wallet_name: "treasury".to_string(),
            network: NetworkSelection::Testnet4,
            addresses: vec![("tb1qtest".to_string(), "02".to_string(), 0)],
            selected: 0,
            error: None,
            hd_enabled: true,
            balance_cache: std::collections::HashMap::new(),
        });

        let help = help_bar_text(&app);

        assert!(help.contains("b/r/F5 (Refresh):Refresh"));
        assert!(help.contains("a:Add"));
        assert!(help.contains("x:Remove"));
        assert!(!help.contains("r:Balance"));
    }

    #[test]
    fn address_list_copy_reports_no_selection() {
        let mut app = App::new().unwrap();
        app.state = AppState::AddressList(AddressListState {
            wallet_name: "treasury".to_string(),
            network: NetworkSelection::Testnet4,
            addresses: Vec::new(),
            selected: 0,
            error: None,
            hd_enabled: true,
            balance_cache: std::collections::HashMap::new(),
        });

        handle_address_list_keys(&mut app, KeyCode::Char('c'));

        assert_eq!(app.message.as_deref(), Some("No address selected to copy"));
    }

    #[test]
    fn address_copy_messages_include_network_and_source_scope() {
        let root = root_address_copy_message(NetworkSelection::Regtest, "bcrt1proot");
        assert_eq!(root, "Copied Regtest root address: bcrt1proot");

        let hd = hd_address_copy_message(NetworkSelection::Signet, "m/86'/1'/0'/0/7", "tb1phd");
        assert_eq!(hd, "Copied Signet HD address m/86'/1'/0'/0/7: tb1phd");
    }

    #[test]
    fn address_balance_messages_include_network() {
        assert_eq!(
            address_balance_fetching_message(NetworkSelection::Testnet4, 3),
            "Fetching Testnet4 balance for address 3..."
        );
        assert_eq!(
            address_balance_updated_message(NetworkSelection::Regtest, 3, 12_345, 0.00012345, 2),
            "Regtest address 3: 12345 sats (0.00012345 BTC), 2 UTXOs"
        );
    }

    #[test]
    #[serial]
    fn address_list_refresh_blocks_when_utxo_source_unavailable() {
        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);

        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Regtest;
        app.state = AppState::AddressList(AddressListState {
            wallet_name: "treasury".to_string(),
            network: NetworkSelection::Regtest,
            addresses: vec![("tb1qaddr".to_string(), "02".to_string(), 0)],
            selected: 0,
            error: None,
            hd_enabled: true,
            balance_cache: std::collections::HashMap::new(),
        });

        handle_address_list_keys(&mut app, KeyCode::Char('b'));

        assert!(matches!(
            app.state,
            AppState::AddressList(AddressListState {
                wallet_name, ..
            }) if wallet_name == "treasury"
        ));
        assert!(app
            .message
            .as_deref()
            .unwrap_or("")
            .contains("Cannot fetch address balance on Regtest"));

        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);
    }

    #[test]
    fn global_help_overlay_lists_network_and_core_actions() {
        let app = App::new().unwrap();
        let rendered = lines_to_string(&global_help_lines(&app));
        assert!(rendered.contains("Networks supported"));
        assert!(rendered.contains("Networks supported"));
        assert!(rendered.contains("Testnet4/Testnet3/Signet"));
        assert!(rendered.contains("FROSTDAO_REGTEST_MEMPOOL_API"));
        assert!(rendered.contains("o:Nostr"));
        assert!(rendered.contains("Global"));
        assert!(rendered.contains("Home"));
        assert!(rendered.contains("Wallet Details"));
    }

    #[test]
    fn global_help_overlay_tracks_keygen_state() {
        let mut app = App::new().unwrap();
        app.state = AppState::Keygen(KeygenState::ParamsSetup);
        app.keygen_form.hierarchical = true;
        app.network = NetworkSelection::Testnet3;

        let rendered = lines_to_string(&global_help_lines(&app));

        assert!(rendered.contains("Keygen"));
        assert!(rendered.contains("Tab:Next field | Enter:Generate"));
        assert!(rendered.contains("Esc:Back"));
        assert!(rendered.contains("↑/↓/j/k/1/2:Select"));
    }

    #[test]
    fn global_help_overlay_tracks_reshare_state() {
        let mut app = App::new().unwrap();
        app.state = AppState::Reshare(ReshareState::ModeSelect);

        let rendered = lines_to_string(&global_help_lines(&app));

        assert!(rendered.contains("Reshare"));
        assert!(rendered.contains("Enter:Continue"));
    }

    #[test]
    fn global_help_overlay_tracks_send_state() {
        let mut app = App::new().unwrap();
        app.state = AppState::Send(SendState::SelectWallet);

        let rendered = lines_to_string(&global_help_lines(&app));

        assert!(rendered.contains("Send"));
        assert!(rendered.contains("Select wallet"));
    }

    #[test]
    fn global_help_overlay_shows_feature_gated_home_shortcuts() {
        let app = App::new().unwrap();
        let rendered = lines_to_string(&global_help_lines(&app));

        assert!(rendered.contains("g:New wallet"));
        assert!(rendered.contains("n:Network"));
        assert!(rendered.contains("m:Mnemonic backup"));
        assert!(rendered.contains("Home"));
        assert!(!rendered.contains("p/P:Paste"));

        #[cfg(feature = "miniscript-policy")]
        assert!(rendered.contains("p:Policy"));

        #[cfg(not(feature = "miniscript-policy"))]
        assert!(!rendered.contains("p:Policy"));
    }

    #[test]
    fn keygen_mode_select_accepts_j_k_shortcuts() {
        let mut app = App::new().unwrap();
        app.state = AppState::Keygen(KeygenState::ModeSelect);
        app.keygen_form.hierarchical = false;

        handle_keygen_keys(
            &mut app,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        );

        assert!(app.keygen_form.hierarchical);

        handle_keygen_keys(
            &mut app,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
        );

        assert!(!app.keygen_form.hierarchical);
    }

    #[test]
    fn send_select_wallet_navigation_is_case_insensitive() {
        let mut app = App::new().unwrap();
        app.wallets = vec![
            wallet_summary_no_address("wallet-a"),
            wallet_summary_no_address("wallet-b"),
        ];
        app.state = AppState::Send(SendState::SelectWallet);

        handle_send_keys(
            &mut app,
            KeyEvent::new(KeyCode::Char('J'), KeyModifiers::NONE),
        );
        assert_eq!(app.send_form.wallet_index, 1);

        handle_send_keys(
            &mut app,
            KeyEvent::new(KeyCode::Char('K'), KeyModifiers::NONE),
        );
        assert_eq!(app.send_form.wallet_index, 0);
    }

    #[test]
    fn keygen_htss_enter_reports_invalid_rank_distribution_without_panic() {
        let mut app = App::new().unwrap();
        app.state = AppState::Keygen(KeygenState::ParamsSetup);
        app.keygen_form.hierarchical = true;
        app.keygen_form.name.set_value("wallet-htss");
        app.keygen_form.rank_distribution.set_value("abc");
        app.keygen_form.signing_requirement.set_value("1,1");

        handle_keygen_keys(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(matches!(
            app.state,
            AppState::Keygen(KeygenState::ParamsSetup)
        ));
        assert_eq!(
            app.keygen_form.error_message.as_deref(),
            Some("Enter parties per rank (e.g., 2,3,3)")
        );
    }

    #[test]
    fn keygen_help_bar_uses_htss_parameters_language() {
        let mut app = App::new().unwrap();
        app.keygen_form.hierarchical = true;
        app.state = AppState::Keygen(KeygenState::ParamsSetup);

        let help = help_bar_text(&app);

        assert!(help.contains("Tab:Next field"));
        assert!(help.contains("Enter:Generate"));
        assert!(help.contains("Esc:Back"));
        assert!(!help.contains("Generate All Parties"));
    }

    #[test]
    fn keygen_help_bar_matches_round_states() {
        let mut app = App::new().unwrap();
        app.state = AppState::Keygen(KeygenState::Round2Input);

        let round2_help = help_bar_text(&app);
        assert!(round2_help.contains("Paste/type JSON"));
        assert!(round2_help.contains("Enter:Generate Shares"));

        app.state = AppState::Keygen(KeygenState::Round1Output {
            output_json: "{}".to_string(),
        });
        let round1_help = help_bar_text(&app);
        assert!(round1_help.contains("Enter:Continue to Round 2"));

        app.state = AppState::Keygen(KeygenState::Complete {
            wallet_name: "wallet-help".to_string(),
        });
        let complete_help = help_bar_text(&app);
        assert!(complete_help.contains("Return to wallet list"));
    }

    #[test]
    fn reshare_help_bar_matches_distributed_local_modes() {
        let mut app = App::new().unwrap();
        app.state = AppState::Reshare(ReshareState::LocalSetup);

        let local_help = help_bar_text(&app);
        assert!(local_help.contains("Tab:Next"));
        assert!(local_help.contains("Enter:Reshare"));
        assert!(local_help.contains("Esc:Back"));

        app.state = AppState::Reshare(ReshareState::FinalizeInput);
        let finalize_help = help_bar_text(&app);
        assert!(finalize_help.contains("Enter:Finalize"));
        assert!(finalize_help.contains("Ctrl+u:Clear"));

        app.state = AppState::Reshare(ReshareState::Round1Output {
            output_json: "{}".to_string(),
        });
        let output_help = help_bar_text(&app);
        assert!(output_help.contains("Enter:Go to Finalize"));
        assert!(output_help.contains("Esc:Done"));
    }

    #[test]
    fn send_help_bar_matches_state_transitions() {
        let mut app = App::new().unwrap();
        app.state = AppState::Send(SendState::SelectSigners {
            wallet_name: "wallet-help".to_string(),
        });

        let select_signers = help_bar_text(&app);
        assert!(select_signers.contains("Space:Toggle"));
        assert!(select_signers.contains("Enter:Continue"));

        app.state = AppState::Send(SendState::ConfigureScript {
            wallet_name: "wallet-help".to_string(),
        });
        let script_help = help_bar_text(&app);
        assert!(script_help.contains("Space:Toggle"));
        assert!(script_help.contains("Esc:Back"));

        app.state = AppState::Send(SendState::ReviewTransaction {
            wallet_name: "wallet-help".to_string(),
        });
        let review_help = help_bar_text(&app);
        assert!(review_help.contains("y:Confirm and sign"));
        assert!(review_help.contains("Esc:Back to edit"));

        app.state = AppState::Send(SendState::ShowSighash {
            wallet_name: "wallet-help".to_string(),
            sighash: "abc".to_string(),
            session_id: "session-help".to_string(),
        });
        let show_sighash_help = help_bar_text(&app);
        assert!(show_sighash_help.contains("Enter:Generate Nonce"));
        assert!(show_sighash_help.contains("c/C:Copy"));
    }

    #[test]
    fn send_help_bar_matches_signature_share_state() {
        let mut app = App::new().unwrap();
        app.state = AppState::Send(SendState::GenerateShare {
            wallet_name: "wallet-help".to_string(),
            share_output: "".to_string(),
        });

        let generate_share = help_bar_text(&app);
        assert!(generate_share.contains("Enter:Combine (Aggregator)"));
        assert!(generate_share.contains("Esc:Done"));

        app.state = AppState::Send(SendState::Complete {
            txid: "abc".to_string(),
            broadcast_status: None,
            raw_tx: None,
        });
        let complete_help = help_bar_text(&app);
        assert!(complete_help.contains("Enter/Esc:Return to home"));
        assert!(complete_help.contains("c/C:Copy raw tx"));
    }

    #[test]
    fn help_bar_prefers_status_message() {
        let mut app = App::new().unwrap();
        app.message = Some("Balance updated for treasury".to_string());

        assert_eq!(help_bar_text(&app), "Balance updated for treasury");
    }

    #[test]
    fn wallet_delete_confirmation_help_requires_typing_wallet_name() {
        let mut app = App::new().unwrap();
        app.state = AppState::WalletDetails(WalletDetailsState {
            wallet_name: "treasury".to_string(),
            selected_action: 0,
            confirm_delete: true,
            delete_confirmation_input: String::new(),
            show_qr: false,
        });

        let help = help_bar_text(&app);

        assert!(help.contains("Type wallet name"));
        assert!(help.contains("Enter:Delete"));
        assert!(help.contains("Backspace:Edit"));
        assert!(help.contains("Esc:Cancel"));
    }

    #[test]
    fn wallet_delete_confirmation_does_not_accept_single_y() {
        let mut app = App::new().unwrap();
        app.state = AppState::WalletDetails(WalletDetailsState {
            wallet_name: "treasury".to_string(),
            selected_action: 0,
            confirm_delete: true,
            delete_confirmation_input: String::new(),
            show_qr: false,
        });

        handle_wallet_details_keys(&mut app, KeyCode::Char('y'));

        let AppState::WalletDetails(state) = &app.state else {
            panic!("single y should not leave wallet details");
        };
        assert!(state.confirm_delete);
        assert_eq!(state.delete_confirmation_input, "y");
    }

    #[test]
    fn wallet_delete_confirmation_rejects_partial_name() {
        let mut app = App::new().unwrap();
        app.state = AppState::WalletDetails(WalletDetailsState {
            wallet_name: "treasury".to_string(),
            selected_action: 0,
            confirm_delete: true,
            delete_confirmation_input: "treas".to_string(),
            show_qr: false,
        });

        handle_wallet_details_keys(&mut app, KeyCode::Enter);

        assert_eq!(
            app.message.as_deref(),
            Some("Type the wallet name exactly to delete")
        );
        assert!(matches!(
            app.state,
            AppState::WalletDetails(WalletDetailsState {
                confirm_delete: true,
                ..
            })
        ));
    }

    #[test]
    fn network_title_status_line_reflects_regtest_requirement() {
        let status = network_title_status_line(NetworkSelection::Regtest);

        assert!(
            status.contains("FROSTDAO_REGTEST_MEMPOOL_API"),
            "unexpected status: {status}"
        );
    }

    #[test]
    fn network_title_status_line_reflects_remote_test_chain_policy() {
        assert!(network_title_status_line(NetworkSelection::Signet)
            .contains("signet remote UTXOs via mempool.space"));
        assert!(network_title_status_line(NetworkSelection::Testnet3)
            .contains("testnet3 remote UTXOs via mempool.space"));
        assert!(network_title_status_line(NetworkSelection::Testnet4)
            .contains("testnet4 remote UTXOs via mempool.space"));
        assert!(network_title_status_line(NetworkSelection::Mainnet)
            .contains("MAINNET real funds; guarded commands require explicit opt-in"));
    }

    fn mnemonic_state_for_help(party_selected: bool, revealed: bool) -> MnemonicState {
        MnemonicState {
            wallet_name: "treasury".to_string(),
            available_parties: vec![1, 2, 3],
            selected_party: 0,
            words: Vec::new(),
            error: None,
            party_selected,
            revealed,
            hierarchical: false,
            party_ranks: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn mnemonic_help_bar_matches_party_selection_stage() {
        let mut app = App::new().unwrap();
        app.state = AppState::MnemonicBackup(mnemonic_state_for_help(false, false));

        let help = help_bar_text(&app);

        assert!(help.contains("Select party"));
        assert!(help.contains("Enter:Continue"));
        assert!(help.contains("Esc:Cancel"));
        assert!(!help.contains("Reveal"));
    }

    #[test]
    fn mnemonic_help_bar_matches_warning_stage() {
        let mut app = App::new().unwrap();
        app.state = AppState::MnemonicBackup(mnemonic_state_for_help(true, false));

        let help = help_bar_text(&app);

        assert_eq!(help, "Enter:Reveal | Esc:Cancel");
    }

    #[test]
    fn mnemonic_help_bar_matches_revealed_stage() {
        let mut app = App::new().unwrap();
        app.state = AppState::MnemonicBackup(mnemonic_state_for_help(true, true));

        let help = help_bar_text(&app);

        assert_eq!(help, "Enter:Done | Esc:Back");
    }

    #[test]
    fn mnemonic_navigation_is_case_insensitive() {
        let mut app = App::new().unwrap();
        app.state = AppState::MnemonicBackup(MnemonicState {
            wallet_name: "treasury".to_string(),
            available_parties: vec![1, 2, 3],
            selected_party: 0,
            words: Vec::new(),
            error: None,
            party_selected: false,
            revealed: false,
            hierarchical: false,
            party_ranks: BTreeMap::new(),
        });

        handle_mnemonic_keys(&mut app, KeyCode::Char('K'));
        assert_eq!(
            if let AppState::MnemonicBackup(state) = &app.state {
                state.selected_party
            } else {
                unreachable!()
            },
            2
        );

        handle_mnemonic_keys(&mut app, KeyCode::Char('J'));
        assert_eq!(
            if let AppState::MnemonicBackup(state) = &app.state {
                state.selected_party
            } else {
                unreachable!()
            },
            0
        );
    }

    #[test]
    fn nostr_sign_help_bar_matches_review_actions() {
        let mut app = App::new().unwrap();
        app.state = AppState::NostrSign;
        app.nostr_sign_state = NostrSignState::ReviewProposal {
            wallet_name: "wallet-test".to_string(),
            proposal: reviewable_proposal(),
        };

        let help = help_bar_text(&app);

        assert!(help.contains("y: Consent"));
        assert!(help.contains("r: Reject"));
        assert!(help.contains("Esc: Back"));
        assert!(!help.contains("Enter:Continue"));
    }

    #[test]
    fn nostr_sign_help_bar_matches_role_actions() {
        let mut app = App::new().unwrap();
        app.state = AppState::NostrSign;
        app.nostr_sign_state = NostrSignState::SelectRole {
            wallet_name: "wallet-test".to_string(),
        };

        let help = help_bar_text(&app);

        assert!(help.contains("p: Propose"));
        assert!(help.contains("c: Consent"));
        assert!(help.contains("Enter: Propose"));
    }

    #[test]
    fn nostr_sign_help_bar_matches_configure_actions() {
        let mut app = App::new().unwrap();
        app.state = AppState::NostrSign;
        app.nostr_sign_state = NostrSignState::ConfigureTx {
            wallet_name: "wallet-test".to_string(),
        };

        let help = help_bar_text(&app);

        assert!(help.contains("Tab:Field"));
        assert!(help.contains("Enter:Publish proposal"));
        assert!(help.contains("Ctrl+u:Clear field"));
    }

    #[test]
    fn nostr_configure_tx_accepts_recipient_and_amount_input() {
        let mut app = App::new().unwrap();
        app.state = AppState::NostrSign;
        app.nostr_sign_state = NostrSignState::ConfigureTx {
            wallet_name: "wallet-test".to_string(),
        };

        for c in "tb1precipient".chars() {
            handle_nostr_sign_keys(
                &mut app,
                KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE),
            );
        }
        assert_eq!(app.nostr_to_address_input.value(), "tb1precipient");
        assert_eq!(app.nostr_tx_focus, NostrTxField::Recipient);

        handle_nostr_sign_keys(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.nostr_tx_focus, NostrTxField::Amount);

        for c in "50000".chars() {
            handle_nostr_sign_keys(
                &mut app,
                KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE),
            );
        }
        assert_eq!(app.nostr_amount_input.value(), "50000");
        assert_eq!(app.nostr_amount_sats().unwrap(), 50_000);
    }

    #[test]
    fn nostr_room_waiting_help_uses_local_test_participant_wording() {
        let mut app = App::new().unwrap();
        app.state = AppState::NostrRoom;
        app.nostr_room_phase = NostrRoomPhase::WaitingForParticipants;

        let help = help_bar_text(&app);

        assert!(help.contains("Space:Add local test participant"));
        assert!(help.contains("Esc:Leave"));
        assert!(!help.contains("Simulate join"));
        assert!(!help.contains("demo"));
    }

    #[test]
    fn nostr_room_ready_blocks_relay_keygen_shortcut() {
        let mut app = App::new().unwrap();
        app.state = AppState::NostrRoom;
        app.nostr_room_phase = NostrRoomPhase::Ready;
        app.force_relay_transport_for_tests = true;

        assert_eq!(
            help_bar_text(&app),
            "k:Relay keygen unavailable | s:Sign | Esc:Leave"
        );

        handle_nostr_room_ready(
            &mut app,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
        );

        assert!(matches!(app.state, AppState::NostrRoom));
        assert!(matches!(app.nostr_room_phase, NostrRoomPhase::Ready));
        assert_eq!(
            app.message.as_deref(),
            Some(nostr_relay_keygen_blocked_message())
        );
    }

    #[test]
    fn nostr_room_ready_blocks_relay_keygen_shortcut_uppercase() {
        let mut app = App::new().unwrap();
        app.state = AppState::NostrRoom;
        app.nostr_room_phase = NostrRoomPhase::Ready;
        app.force_relay_transport_for_tests = true;

        handle_nostr_room_ready(
            &mut app,
            KeyEvent::new(KeyCode::Char('K'), KeyModifiers::NONE),
        );

        assert!(matches!(app.state, AppState::NostrRoom));
        assert!(matches!(app.nostr_room_phase, NostrRoomPhase::Ready));
        assert_eq!(
            app.message.as_deref(),
            Some(nostr_relay_keygen_blocked_message())
        );
    }

    #[test]
    fn nostr_sign_wallet_navigation_is_case_insensitive() {
        let mut app = App::new().unwrap();
        app.wallets = vec![
            wallet_summary_no_address("wallet-a"),
            wallet_summary_no_address("wallet-b"),
        ];
        app.wallet_list_state.select(Some(0));
        app.state = AppState::NostrSign;
        app.nostr_sign_state = NostrSignState::SelectWallet;

        handle_nostr_sign_keys(
            &mut app,
            KeyEvent::new(KeyCode::Char('J'), KeyModifiers::NONE),
        );
        assert_eq!(app.wallet_list_state.selected(), Some(1));

        handle_nostr_sign_keys(
            &mut app,
            KeyEvent::new(KeyCode::Char('K'), KeyModifiers::NONE),
        );
        assert_eq!(app.wallet_list_state.selected(), Some(0));
    }

    #[test]
    fn nostr_review_consent_key_is_case_insensitive() {
        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Testnet3;
        app.nostr_room_id = format!("tui-approve-proposal-test-{}", std::process::id());
        app.nostr_my_index = 1;
        app.nostr_threshold = 2;
        app.nostr_n_parties = 2;
        let cache_path = app.nostr_replay_cache_path();
        let _ = std::fs::remove_file(&cache_path);

        app.join_nostr_room_runtime_with_relays(Vec::new()).unwrap();
        app.state = AppState::NostrSign;
        app.nostr_sign_state = NostrSignState::ReviewProposal {
            wallet_name: "wallet-test".to_string(),
            proposal: reviewable_proposal(),
        };

        handle_nostr_sign_keys(
            &mut app,
            KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::NONE),
        );

        assert!(matches!(
            app.nostr_sign_state,
            NostrSignState::WaitingForExecution { .. }
        ));
        assert_eq!(app.audit_events[0].event, "nostr_tx_consent");
        assert_eq!(app.audit_events[0].status, "consented");
        assert_eq!(
            app.audit_events[0].fields["sighash_fingerprint"],
            "abc12345"
        );
        assert!(app
            .message
            .as_deref()
            .unwrap_or("")
            .contains("Consent sent"));
        let _ = std::fs::remove_file(&cache_path);
    }

    #[test]
    fn nostr_review_reject_key_publishes_rejection() {
        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Testnet3;
        app.nostr_room_id = format!("tui-reject-proposal-test-{}", std::process::id());
        app.nostr_my_index = 1;
        app.nostr_threshold = 2;
        app.nostr_n_parties = 2;
        let cache_path = app.nostr_replay_cache_path();
        let _ = std::fs::remove_file(&cache_path);

        app.join_nostr_room_runtime_with_relays(Vec::new()).unwrap();
        app.state = AppState::NostrSign;
        app.nostr_sign_state = NostrSignState::ReviewProposal {
            wallet_name: "wallet-test".to_string(),
            proposal: reviewable_proposal(),
        };

        handle_nostr_sign_keys(
            &mut app,
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
        );

        assert!(matches!(
            app.nostr_sign_state,
            NostrSignState::ViewProposals { .. }
        ));
        assert_eq!(app.audit_events[0].event, "nostr_tx_consent");
        assert_eq!(app.audit_events[0].status, "rejected");
        assert_eq!(
            app.audit_events[0].fields["sighash_fingerprint"],
            "abc12345"
        );
        assert!(app
            .message
            .as_deref()
            .unwrap_or("")
            .contains("Rejection sent"));
        let _ = std::fs::remove_file(&cache_path);
    }

    fn app_ready_to_prepare_send() -> App {
        let mut app = App::new().unwrap();
        app.state = AppState::Send(SendState::EnterDetails {
            wallet_name: "wallet-test".to_string(),
        });
        app.send_form.to_address.set_value("tb1qrecipient");
        app.send_form.amount.set_value("1000");
        app.send_form.threshold = 1;
        app.send_form.total_parties = 1;
        app.send_form.selected_parties = vec![true];
        app
    }

    #[test]
    fn send_enter_details_blocks_when_utxo_source_unavailable() {
        let mut app = app_ready_to_prepare_send();
        app.send_form.utxo_fetch_error =
            Some("Cannot fetch UTXOs on Regtest: set FROSTDAO_REGTEST_MEMPOOL_API".to_string());

        handle_send_keys(&mut app, enter_key());

        assert!(matches!(
            app.state,
            AppState::Send(SendState::EnterDetails { .. })
        ));
        assert!(app
            .send_form
            .error_message
            .as_deref()
            .unwrap_or("")
            .contains("Cannot prepare transaction"));
    }

    #[test]
    fn send_enter_details_blocks_without_confirmed_utxos() {
        let mut app = app_ready_to_prepare_send();

        handle_send_keys(&mut app, enter_key());

        assert!(matches!(
            app.state,
            AppState::Send(SendState::EnterDetails { .. })
        ));
        assert!(app
            .send_form
            .error_message
            .as_deref()
            .unwrap_or("")
            .contains("No confirmed UTXOs"));
    }

    #[test]
    fn send_enter_details_blocks_insufficient_confirmed_balance() {
        let mut app = app_ready_to_prepare_send();
        app.send_form.utxos = vec![screens::UtxoDisplay {
            txid: "00".repeat(32),
            vout: 0,
            value: 500,
            confirmed: true,
        }];

        handle_send_keys(&mut app, enter_key());

        assert!(matches!(
            app.state,
            AppState::Send(SendState::EnterDetails { .. })
        ));
        assert!(app
            .send_form
            .error_message
            .as_deref()
            .unwrap_or("")
            .contains("Insufficient confirmed balance"));
    }

    #[test]
    fn send_enter_details_advances_with_confirmed_balance() {
        let mut app = app_ready_to_prepare_send();
        app.send_form.utxos = vec![screens::UtxoDisplay {
            txid: "00".repeat(32),
            vout: 0,
            value: 2_000,
            confirmed: true,
        }];

        handle_send_keys(&mut app, enter_key());

        assert!(matches!(
            app.state,
            AppState::Send(SendState::ReviewTransaction { .. })
        ));
        assert!(app.send_form.error_message.is_none());
    }

    #[test]
    fn send_review_blocks_mainnet_local_sign_and_broadcast() {
        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Mainnet;
        app.state = AppState::Send(SendState::ReviewTransaction {
            wallet_name: "wallet-test".to_string(),
        });
        app.send_form.to_address.set_value("bc1qrecipient");
        app.send_form.amount.set_value("1000");
        app.send_form.selected_parties = vec![true];

        handle_send_keys(
            &mut app,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
        );

        assert!(matches!(
            app.state,
            AppState::Send(SendState::ReviewTransaction { .. })
        ));
        assert!(app
            .send_form
            .error_message
            .as_deref()
            .unwrap_or("")
            .contains("Mainnet send is blocked in the TUI"));
    }

    #[test]
    #[serial]
    fn send_select_address_blocks_without_utxo_source() {
        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);

        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Regtest;
        app.wallets = vec![wallet_summary_no_address("treasury")];
        app.send_form.wallet_index = 0;

        app.state = AppState::Send(SendState::SelectAddress {
            wallet_name: "treasury".to_string(),
        });

        handle_send_keys(&mut app, enter_key());

        assert!(matches!(
            app.state,
            AppState::Send(SendState::SelectAddress { wallet_name }) if wallet_name == "treasury"
        ));
        assert!(app
            .send_form
            .error_message
            .as_deref()
            .unwrap_or("")
            .contains("Cannot fetch UTXOs on Regtest"));

        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);
    }
}
