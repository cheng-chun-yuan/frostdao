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
use state::{AppState, KeygenState, ReshareState, SendState};

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

    let state = app.state.clone();
    match state {
        AppState::Keygen(KeygenState::Round1Setup) => match key.code {
            KeyCode::Esc => {
                app.keygen_form = screens::KeygenFormData::new();
                app.state = AppState::Home;
            }
            KeyCode::Tab | KeyCode::Down => {
                app.keygen_form.focused_field = app.keygen_form.focused_field.next();
            }
            KeyCode::BackTab | KeyCode::Up => {
                app.keygen_form.focused_field = app.keygen_form.focused_field.prev();
            }
            KeyCode::Char(' ')
                if app.keygen_form.focused_field == KeygenFormField::Hierarchical =>
            {
                app.keygen_form.hierarchical = !app.keygen_form.hierarchical;
            }
            KeyCode::Enter => {
                // Validate and run keygen round 1
                let name = app.keygen_form.name.value().to_string();
                let threshold: u32 = app.keygen_form.threshold.value().parse().unwrap_or(0);
                let n_parties: u32 = app.keygen_form.n_parties.value().parse().unwrap_or(0);
                let my_index: u32 = app.keygen_form.my_index.value().parse().unwrap_or(0);
                let my_rank: u32 = app.keygen_form.my_rank.value().parse().unwrap_or(0);
                let hierarchical = app.keygen_form.hierarchical;

                if name.is_empty() {
                    app.keygen_form.error_message = Some("Wallet name is required".to_string());
                    return;
                }
                if threshold == 0 || threshold > n_parties {
                    app.keygen_form.error_message = Some("Invalid threshold".to_string());
                    return;
                }
                if my_index == 0 || my_index > n_parties {
                    app.keygen_form.error_message = Some("Invalid party index".to_string());
                    return;
                }

                // Run keygen round 1
                let state_dir = keygen::get_state_dir(&name);
                match FileStorage::new(&state_dir) {
                    Ok(storage) => {
                        match keygen::round1_core(
                            threshold,
                            n_parties,
                            my_index,
                            my_rank,
                            hierarchical,
                            &storage,
                        ) {
                            Ok(result) => {
                                app.keygen_form.round1_output = result.result;
                                app.keygen_form.error_message = None;
                                app.state = AppState::Keygen(KeygenState::Round1Output {
                                    output_json: app.keygen_form.round1_output.clone(),
                                });
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
                // Handle text input based on focused field
                match app.keygen_form.focused_field {
                    KeygenFormField::Name => {
                        app.keygen_form.name.handle_key(key);
                    }
                    KeygenFormField::Threshold => {
                        app.keygen_form.threshold.handle_key(key);
                    }
                    KeygenFormField::NParties => {
                        app.keygen_form.n_parties.handle_key(key);
                    }
                    KeygenFormField::MyIndex => {
                        app.keygen_form.my_index.handle_key(key);
                    }
                    KeygenFormField::MyRank => {
                        app.keygen_form.my_rank.handle_key(key);
                    }
                    KeygenFormField::Hierarchical => {}
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
                                    bincode::deserialize(&bytes).unwrap();

                                // Extract party index from Scalar (hack from signing.rs)
                                let my_old_index = {
                                    let mut u32_index_bytes = [0u8; 4];
                                    u32_index_bytes
                                        .copy_from_slice(&paired_share.index().to_bytes()[28..]);
                                    u32::from_be_bytes(u32_index_bytes)
                                };

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
                let wallet_name = app.wallets[app.send_form.wallet_index].name.clone();
                app.state = AppState::Send(SendState::EnterDetails { wallet_name });
            }
            _ => {}
        },
        AppState::Send(SendState::EnterDetails { wallet_name }) => match key.code {
            KeyCode::Esc => {
                app.state = AppState::Send(SendState::SelectWallet);
            }
            KeyCode::Tab => {
                app.send_form.focused_field = app.send_form.focused_field.next();
            }
            KeyCode::BackTab => {
                app.send_form.focused_field = app.send_form.focused_field.prev();
            }
            KeyCode::Enter => {
                // Generate a demo sighash (in real world, this would come from TX builder)
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

                // Generate session ID and demo sighash
                let session_id = SendFormData::generate_session_id();
                // Create a demo sighash based on the inputs (in real world this comes from TX)
                let sighash = format!(
                    "demo_sighash_{}_{}_{}",
                    wallet_name,
                    &to_addr[..8.min(to_addr.len())],
                    amount
                );
                app.send_form.session_id = session_id.clone();
                app.send_form.sighash = sighash.clone();

                app.state = AppState::Send(SendState::ShowSighash {
                    wallet_name,
                    to_address: to_addr,
                    amount,
                    sighash,
                    session_id,
                });
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
                    to_address: app.send_form.to_address.value().to_string(),
                    amount: app.send_form.amount.value().parse().unwrap_or(0),
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
                    my_nonce: nonce_output,
                });
            }
            _ => {}
        },
        AppState::Send(SendState::EnterNonces {
            wallet_name,
            session_id,
            sighash,
            ..
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

                // Generate signature share
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
                                    session_id,
                                    sighash,
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
            session_id,
            sighash,
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
                app.state = AppState::Send(SendState::CombineShares {
                    wallet_name,
                    session_id,
                    sighash,
                });
            }
            _ => {}
        },
        AppState::Send(SendState::CombineShares { wallet_name, .. }) => match key.code {
            KeyCode::Esc => {
                app.state = AppState::Send(SendState::GenerateShare {
                    wallet_name,
                    session_id: app.send_form.session_id.clone(),
                    sighash: app.send_form.sighash.clone(),
                    share_output: app.send_form.share_output.clone(),
                });
            }
            KeyCode::Enter => {
                let shares_data = app.send_form.shares_input.content();
                if shares_data.trim().is_empty() {
                    app.send_form.error_message = Some("Paste signature shares first".to_string());
                    return;
                }

                // Combine signatures
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
                "↑/↓:Navigate | Enter:Balance | n:Network | g:Keygen | h:Reshare | s:Send | q:Quit"
                    .to_string()
            }
            AppState::ChainSelect => "↑/↓:Select | Enter:Confirm | Esc:Cancel".to_string(),
            AppState::Keygen(_) => "Tab:Next | Enter:Continue | Esc:Cancel".to_string(),
            AppState::Reshare(_) => "Tab:Next | Enter:Continue | Esc:Cancel".to_string(),
            AppState::Send(_) => "Tab:Next | Enter:Continue | Esc:Cancel".to_string(),
        }
    };

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL).title("Help"));

    frame.render_widget(help, area);
}
