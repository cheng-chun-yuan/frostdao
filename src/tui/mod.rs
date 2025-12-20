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
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
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
use state::AppState;

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

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
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
                    AppState::Keygen(_) => handle_keygen_keys(app, key.code),
                    AppState::Reshare(_) => handle_reshare_keys(app, key.code),
                    AppState::Send(_) => handle_send_keys(app, key.code),
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

fn handle_keygen_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => app.state = AppState::Home,
        _ => {
            // Will be implemented in Commit 3
        }
    }
}

fn handle_reshare_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => app.state = AppState::Home,
        _ => {
            // Will be implemented in Commit 4
        }
    }
}

fn handle_send_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => app.state = AppState::Home,
        _ => {
            // Will be implemented in Commit 5
        }
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
        AppState::Keygen(_) => screens::render_keygen(frame, app, chunks[1]),
        AppState::Reshare(_) => screens::render_reshare(frame, app, chunks[1]),
        AppState::Send(_) => screens::render_send(frame, app, chunks[1]),
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
        Span::styled(app.network.display_name(), Style::default().fg(network_color)),
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
