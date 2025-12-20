//! Home screen - wallet list and details

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::tui::app::App;
use crate::tui::state::NetworkSelection;

/// Render the home screen
pub fn render_home(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    render_wallet_list(frame, app, chunks[0]);
    render_wallet_details(frame, app, chunks[1]);
}

fn render_wallet_list(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .wallets
        .iter()
        .map(|wallet| {
            let mode = match wallet.hierarchical {
                Some(true) => "HTSS",
                Some(false) => "TSS",
                None => "?",
            };
            let threshold = match (wallet.threshold, wallet.total_parties) {
                (Some(t), Some(n)) => format!("{}-of-{}", t, n),
                _ => "?".to_string(),
            };

            let has_balance = app.balance_cache.contains_key(&wallet.name);
            let balance_indicator = if has_balance { " $" } else { "" };

            ListItem::new(format!(
                "{} ({} {}){}",
                wallet.name, threshold, mode, balance_indicator
            ))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Wallets"))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    frame.render_stateful_widget(list, area, &mut app.wallet_list_state.clone());
}

fn render_wallet_details(frame: &mut Frame, app: &App, area: Rect) {
    let content = if let Some(wallet) = app.selected_wallet() {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Name: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    &wallet.name,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
        ];

        // Threshold info
        if let (Some(t), Some(n)) = (wallet.threshold, wallet.total_parties) {
            lines.push(Line::from(vec![
                Span::styled("Threshold: ", Style::default().fg(Color::Gray)),
                Span::styled(format!("{}-of-{}", t, n), Style::default().fg(Color::Yellow)),
            ]));
        }

        // Mode
        if let Some(h) = wallet.hierarchical {
            lines.push(Line::from(vec![
                Span::styled("Mode: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    if h {
                        "Hierarchical (HTSS)"
                    } else {
                        "Standard (TSS)"
                    },
                    Style::default().fg(Color::Cyan),
                ),
            ]));
        }

        lines.push(Line::from(""));

        // Address (network-specific)
        if let Some(addr) = get_address_for_network(wallet, app.network) {
            lines.push(Line::from(vec![Span::styled(
                format!("Address ({}): ", app.network.display_name()),
                Style::default().fg(Color::Gray),
            )]));
            lines.push(Line::from(vec![Span::styled(
                addr,
                Style::default().fg(Color::Green),
            )]));
        }

        lines.push(Line::from(""));

        // Balance (if cached)
        let cache_key = format!("{}:{:?}", wallet.name, app.network);
        if let Some(info) = app.balance_cache.get(&cache_key) {
            lines.push(Line::from(vec![
                Span::styled("Balance: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{} sats", info.balance_sats),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));

            let btc = info.balance_sats as f64 / 100_000_000.0;
            lines.push(Line::from(vec![
                Span::styled("         ", Style::default()),
                Span::styled(format!("({:.8} BTC)", btc), Style::default().fg(Color::Gray)),
            ]));

            lines.push(Line::from(vec![
                Span::styled("UTXOs: ", Style::default().fg(Color::Gray)),
                Span::styled(format!("{}", info.utxo_count), Style::default().fg(Color::White)),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("Balance: ", Style::default().fg(Color::Gray)),
                Span::styled("Press Enter to fetch", Style::default().fg(Color::DarkGray)),
            ]));
        }

        lines
    } else {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "No wallet selected",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from("Create a wallet with 'k' (keygen)"),
            Line::from("or use CLI:"),
            Line::from(Span::styled(
                "  frostdao keygen-round1 --name <name> ...",
                Style::default().fg(Color::Cyan),
            )),
        ]
    };

    let details = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title("Details"))
        .wrap(Wrap { trim: false });

    frame.render_widget(details, area);
}

/// Get address for the selected network
fn get_address_for_network(
    wallet: &crate::keygen::WalletSummary,
    network: NetworkSelection,
) -> Option<String> {
    // For now, return the stored address (testnet)
    // In a full implementation, we'd derive addresses for each network
    match network {
        NetworkSelection::Testnet => wallet.address.clone(),
        NetworkSelection::Signet => wallet.address.clone(), // Same format as testnet (tb1p...)
        NetworkSelection::Mainnet => {
            // Mainnet would use bc1p... prefix - need to regenerate
            wallet.address.as_ref().map(|addr| {
                if addr.starts_with("tb1p") {
                    format!("bc1p{}", &addr[4..])
                } else {
                    addr.clone()
                }
            })
        }
    }
}
