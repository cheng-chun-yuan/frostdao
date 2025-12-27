//! Terminal UI module for FrostDAO wallet management
//!
//! Provides an interactive terminal interface for:
//! - Viewing and managing DKG wallets
//! - Chain/network selection (Testnet, Signet, Mainnet)
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
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use std::io;

use app::App;
use state::{AddressListState, AppState, KeygenState, MnemonicState, ReshareState, SendState};

use frostdao::protocol::{keygen, reshare, signing};
use frostdao::storage::{FileStorage, Storage};

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

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                // Global quit
                if matches!(key.code, KeyCode::Char('q')) && matches!(app.state, AppState::Home) {
                    return Ok(());
                }

                match &app.state {
                    AppState::Home => handle_home_keys(app, key.code),
                    AppState::ChainSelect => handle_chain_select_keys(app, key.code),
                    AppState::Keygen(_) => handle_keygen_keys(app, key),
                    AppState::Reshare(_) => handle_reshare_keys(app, key),
                    AppState::Send(_) => handle_send_keys(app, key),
                    AppState::AddressList(_) => handle_address_list_keys(app, key.code),
                    AppState::MnemonicBackup(_) => handle_mnemonic_keys(app, key.code),
                }
            }
        }
    }
}

fn handle_home_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Down | KeyCode::Char('j') => app.next_wallet(),
        KeyCode::Up | KeyCode::Char('k') => app.prev_wallet(),
        KeyCode::Enter | KeyCode::Char('r') => app.refresh_balance(),
        KeyCode::Char('R') => app.reload_wallets(),
        KeyCode::Char('n') => {
            app.chain_selector_index = match app.network {
                state::NetworkSelection::Testnet => 0,
                state::NetworkSelection::Signet => 1,
                state::NetworkSelection::Mainnet => 2,
            };
            app.state = AppState::ChainSelect;
        }
        KeyCode::Char('g') => {
            // Keygen wizard (will be implemented in Commit 3)
            app.state = AppState::Keygen(state::KeygenState::default());
        }
        KeyCode::Char('h') => {
            // Reshare wizard (will be implemented in Commit 4)
            if app.selected_wallet().is_some() {
                app.state = AppState::Reshare(state::ReshareState::default());
            } else {
                app.set_message("Select a wallet first to reshare");
            }
        }
        KeyCode::Char('s') => {
            // Send wizard (will be implemented in Commit 5)
            if app.selected_wallet().is_some() {
                app.state = AppState::Send(state::SendState::default());
            } else {
                app.set_message("Select a wallet first to send");
            }
        }
        KeyCode::Char('a') => {
            // HD Address list
            if let Some(wallet) = app.selected_wallet() {
                let wallet_name = wallet.name.clone();
                app.state = AppState::AddressList(AddressListState {
                    wallet_name: wallet_name.clone(),
                    addresses: Vec::new(),
                    selected: 0,
                    error: None,
                    hd_enabled: false,
                });
                // Load addresses
                app.load_hd_addresses(&wallet_name);
            } else {
                app.set_message("Select a wallet first to view addresses");
            }
        }
        KeyCode::Char('m') => {
            // Mnemonic backup
            if let Some(wallet) = app.selected_wallet() {
                let wallet_name = wallet.name.clone();
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
                    });
                }
            } else {
                app.set_message("Select a wallet first to backup");
            }
        }
        _ => {}
    }
}

fn handle_chain_select_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Up | KeyCode::Char('k') => app.prev_network(),
        KeyCode::Down | KeyCode::Char('j') => app.next_network(),
        KeyCode::Enter => app.confirm_network(),
        KeyCode::Esc => app.state = AppState::Home,
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
            // HTSS mode: Name -> NParties -> Name (skip Threshold)
            (KeygenFormField::Name, true) => KeygenFormField::NParties,
            (KeygenFormField::NParties, true) => KeygenFormField::Name,
            (KeygenFormField::Threshold, true) => KeygenFormField::NParties,
        }
    }

    fn prev_field(current: KeygenFormField, hierarchical: bool) -> KeygenFormField {
        match (current, hierarchical) {
            // TSS mode
            (KeygenFormField::Name, false) => KeygenFormField::NParties,
            (KeygenFormField::Threshold, false) => KeygenFormField::Name,
            (KeygenFormField::NParties, false) => KeygenFormField::Threshold,
            // HTSS mode
            (KeygenFormField::Name, true) => KeygenFormField::NParties,
            (KeygenFormField::NParties, true) => KeygenFormField::Name,
            (KeygenFormField::Threshold, true) => KeygenFormField::Name,
        }
    }

    let state = app.state.clone();
    match state {
        AppState::Keygen(KeygenState::ModeSelect) => match key.code {
            KeyCode::Esc => {
                app.keygen_form = screens::KeygenFormData::new();
                app.state = AppState::Home;
            }
            KeyCode::Up | KeyCode::Down => {
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
                // Validate and run keygen round 1
                let name = app.keygen_form.name.value().to_string();
                let n_parties: u32 = app.keygen_form.n_parties.value().parse().unwrap_or(0);
                let hierarchical = app.keygen_form.hierarchical;

                // For HTSS, threshold is based on ranks; for TSS, use user input
                let threshold: u32 = if hierarchical {
                    n_parties // HTSS: threshold = n_parties
                } else {
                    app.keygen_form.threshold.value().parse().unwrap_or(0)
                };

                if name.is_empty() {
                    app.keygen_form.error_message = Some("Wallet name is required".to_string());
                    return;
                }
                if n_parties < 2 {
                    app.keygen_form.error_message = Some("Need at least 2 parties".to_string());
                    return;
                }
                if !hierarchical && (threshold == 0 || threshold > n_parties) {
                    app.keygen_form.error_message =
                        Some("Invalid threshold (must be 1 ≤ t ≤ n)".to_string());
                    return;
                }

                // Generate all parties at once
                let ranks = if hierarchical {
                    // Default ranks: 0, 1, 2, ...
                    Some((0..n_parties).collect())
                } else {
                    None
                };

                match keygen::generate_all_parties(&name, threshold, n_parties, hierarchical, ranks)
                {
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
                        if !app.keygen_form.hierarchical {
                            app.keygen_form.threshold.handle_key(key);
                        }
                    }
                    KeygenFormField::NParties => {
                        app.keygen_form.n_parties.handle_key(key);
                    }
                }
            }
        },
        AppState::Keygen(KeygenState::Round1Output { .. }) => match key.code {
            KeyCode::Esc => {
                app.keygen_form = screens::KeygenFormData::new();
                app.state = AppState::Home;
            }
            KeyCode::Enter => {
                app.state = AppState::Keygen(KeygenState::Round2Input);
            }
            KeyCode::Char('c') => {
                // Copy to clipboard (placeholder - would need arboard crate)
                app.set_message("Output copied to clipboard (simulated)");
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
                    Ok(storage) => match keygen::round2_core(&data, &storage) {
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
            KeyCode::Enter => {
                app.state = AppState::Keygen(KeygenState::FinalizeInput);
            }
            KeyCode::Char('c') => {
                app.set_message("Output copied to clipboard (simulated)");
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
    use state::{ReshareFinalizeField, ReshareFormField};

    let state = app.state.clone();
    match state {
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
            KeyCode::Char('j')
                if app.reshare_form.focused_field == ReshareFormField::SourceWallet =>
            {
                if !app.wallets.is_empty() {
                    app.reshare_form.source_wallet_index =
                        (app.reshare_form.source_wallet_index + 1) % app.wallets.len();
                }
            }
            KeyCode::Char('k')
                if app.reshare_form.focused_field == ReshareFormField::SourceWallet =>
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
                match FileStorage::new(&state_dir) {
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
                                            app.reshare_form.error_message =
                                                Some(format!("Corrupted wallet data: {}", e));
                                            return;
                                        }
                                    };

                                // Extract party index from scalar (big-endian, last 4 bytes)
                                let index_bytes = paired_share.index().to_bytes();
                                let my_old_index =
                                    u32::from_be_bytes(index_bytes[28..32].try_into().unwrap());

                                match reshare::reshare_round1_core(
                                    &wallet_name,
                                    new_threshold,
                                    new_n_parties,
                                    my_old_index,
                                ) {
                                    Ok(result) => {
                                        app.reshare_form.round1_output = result.result;
                                        app.reshare_form.error_message = None;
                                        app.state = AppState::Reshare(ReshareState::Round1Output {
                                            output_json: app.reshare_form.round1_output.clone(),
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
                                    Some(format!("Cannot read wallet: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        app.reshare_form.error_message = Some(format!("Storage error: {}", e));
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
            KeyCode::Enter => {
                // New party: go to finalize
                app.state = AppState::Reshare(ReshareState::FinalizeInput);
            }
            KeyCode::Char('c') => {
                app.set_message("Output copied to clipboard (simulated)");
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
            KeyCode::Up | KeyCode::Char('k') => {
                if app.send_form.wallet_index > 0 {
                    app.send_form.wallet_index -= 1;
                } else if !app.wallets.is_empty() {
                    app.send_form.wallet_index = app.wallets.len() - 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !app.wallets.is_empty() {
                    app.send_form.wallet_index =
                        (app.send_form.wallet_index + 1) % app.wallets.len();
                }
            }
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

                app.state = AppState::Send(SendState::SelectSigners { wallet_name });
            }
            _ => {}
        },
        AppState::Send(SendState::SelectSigners { wallet_name }) => match key.code {
            KeyCode::Esc => {
                app.state = AppState::Send(SendState::SelectWallet);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if app.send_form.party_selector_index > 0 {
                    app.send_form.party_selector_index -= 1;
                } else {
                    app.send_form.party_selector_index = app.send_form.total_parties as usize - 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.send_form.party_selector_index =
                    (app.send_form.party_selector_index + 1) % app.send_form.total_parties as usize;
            }
            KeyCode::Char(' ') => {
                // Toggle party selection
                let idx = app.send_form.party_selector_index;
                if idx < app.send_form.selected_parties.len() {
                    app.send_form.selected_parties[idx] = !app.send_form.selected_parties[idx];
                }
            }
            KeyCode::Enter => {
                // Check if threshold is met
                let selected = app.send_form.selected_count();
                if selected < app.send_form.threshold as usize {
                    app.send_form.error_message = Some(format!(
                        "Need at least {} signers, only {} selected",
                        app.send_form.threshold, selected
                    ));
                    return;
                }
                app.send_form.error_message = None;
                app.state = AppState::Send(SendState::EnterDetails { wallet_name });
            }
            _ => {}
        },
        AppState::Send(SendState::EnterDetails { wallet_name }) => match key.code {
            KeyCode::Esc => {
                app.state = AppState::Send(SendState::SelectSigners {
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

                // Get network from app
                let network = app.network.to_bitcoin_network();

                // Call automated FROST signing
                match frostdao::protocol::dkg_tx::frost_sign_all_local(
                    &wallet_name,
                    &to_addr,
                    amount,
                    &selected_parties,
                    None, // Use default fee rate
                    network,
                ) {
                    Ok(result) => {
                        app.send_form.error_message = None;
                        // Extract txid from result
                        let txid = if let Ok(parsed) =
                            serde_json::from_str::<serde_json::Value>(&result.result)
                        {
                            parsed["txid"].as_str().unwrap_or("unknown").to_string()
                        } else {
                            result.result.clone()
                        };
                        app.state = AppState::Send(SendState::Complete { txid });
                    }
                    Err(e) => {
                        app.send_form.error_message = Some(format!("Error: {}", e));
                    }
                }
            }
            _ => match app.send_form.focused_field {
                SendFormField::ToAddress => {
                    app.send_form.to_address.handle_key(key);
                }
                SendFormField::Amount => {
                    app.send_form.amount.handle_key(key);
                }
            },
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
            KeyCode::Char('c') => {
                app.set_message("Sighash copied to clipboard (simulated)");
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
            KeyCode::Char('c') => {
                app.set_message("Nonce copied to clipboard (simulated)");
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
            KeyCode::Char('c') => {
                app.set_message("Signature share copied to clipboard (simulated)");
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
        AppState::Send(SendState::Complete { .. }) => match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                app.send_form = SendFormData::new();
                app.state = AppState::Home;
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
        KeyCode::Up | KeyCode::Char('k') => {
            if let AppState::AddressList(ref mut state) = app.state {
                if state.selected > 0 {
                    state.selected -= 1;
                }
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let AppState::AddressList(ref mut state) = app.state {
                if state.selected + 1 < state.addresses.len() {
                    state.selected += 1;
                }
            }
        }
        KeyCode::Char('c') => {
            app.set_message("Address copied to clipboard (simulated)");
        }
        _ => {}
    }
}

fn handle_mnemonic_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.state = AppState::Home;
        }
        KeyCode::Up | KeyCode::Char('k') => {
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
        KeyCode::Down | KeyCode::Char('j') => {
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

fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Help bar
        ])
        .split(frame.area());

    // Title with network indicator
    render_title(frame, app, chunks[0]);

    // Main content based on state
    match &app.state {
        AppState::Home => screens::render_home(frame, app, chunks[1]),
        AppState::ChainSelect => {
            screens::render_home(frame, app, chunks[1]);
            screens::render_chain_select(frame, app, frame.area());
        }
        AppState::Keygen(_) => screens::render_keygen(frame, app, &app.keygen_form, chunks[1]),
        AppState::Reshare(_) => screens::render_reshare(frame, app, &app.reshare_form, chunks[1]),
        AppState::Send(_) => screens::render_send(frame, app, &app.send_form, chunks[1]),
        AppState::AddressList(state) => screens::render_address_list(frame, state, chunks[1]),
        AppState::MnemonicBackup(state) => screens::render_mnemonic(frame, state, chunks[1]),
    }

    // Help bar
    render_help_bar(frame, app, chunks[2]);
}

fn render_title(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let network_color = match app.network {
        state::NetworkSelection::Testnet => Color::Yellow,
        state::NetworkSelection::Signet => Color::Magenta,
        state::NetworkSelection::Mainnet => Color::Red,
    };

    let title = Line::from(vec![
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
    ]);

    let paragraph = Paragraph::new(title).block(Block::default().borders(Borders::ALL));

    frame.render_widget(paragraph, area);
}

fn render_help_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let help_text = if let Some(msg) = &app.message {
        msg.clone()
    } else {
        match &app.state {
            AppState::Home => {
                "↑/↓:Nav | r:Balance | n:Net | g:Keygen | h:Reshare | s:Send | a:Addr | m:Backup | q:Quit"
                    .to_string()
            }
            AppState::ChainSelect => "↑/↓:Select | Enter:Confirm | Esc:Cancel".to_string(),
            AppState::Keygen(_) => "Tab:Next | Enter:Continue | Esc:Cancel".to_string(),
            AppState::Reshare(_) => "Tab:Next | Enter:Continue | Esc:Cancel".to_string(),
            AppState::Send(_) => "Tab:Next | Enter:Continue | Esc:Cancel".to_string(),
            AppState::AddressList(_) => "↑/↓:Navigate | c:Copy | Esc:Back".to_string(),
            AppState::MnemonicBackup(state) => {
                if state.revealed {
                    "Enter:Done | Esc:Back".to_string()
                } else {
                    "Enter:Reveal | Esc:Cancel".to_string()
                }
            }
        }
    };

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL).title("Help"));

    frame.render_widget(help, area);
}
