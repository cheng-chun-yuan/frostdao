//! Wallet details screen with action menu

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::tui::app::App;
use crate::tui::state::{WalletAction, WalletDetailsState};

/// Render the wallet details screen
pub fn render_wallet_details(frame: &mut Frame, app: &App, state: &WalletDetailsState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_wallet_info(frame, app, &state.wallet_name, chunks[0]);
    render_action_menu(frame, state, chunks[1]);

    // Render confirmation dialog overlay if deleting
    if state.confirm_delete {
        render_delete_confirmation(frame, &state.wallet_name, area);
    }
}

fn render_delete_confirmation(frame: &mut Frame, wallet_name: &str, area: Rect) {
    use ratatui::widgets::Clear;

    // Center the dialog
    let popup_width = 50;
    let popup_height = 8;
    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(popup_width)) / 2,
        y: area.y + (area.height.saturating_sub(popup_height)) / 2,
        width: popup_width.min(area.width),
        height: popup_height.min(area.height),
    };

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let content = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "⚠️  DELETE WALLET?",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::raw("This will permanently delete "),
            Span::styled(
                wallet_name,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Y", Style::default().fg(Color::Green)),
            Span::raw(" = Yes, delete  |  "),
            Span::styled("N", Style::default().fg(Color::Red)),
            Span::raw(" = No, cancel"),
        ]),
    ];

    let dialog = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .title(" Confirm Delete ")
                .style(Style::default().bg(Color::Black)),
        )
        .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(dialog, popup_area);
}

fn render_wallet_info(frame: &mut Frame, app: &App, wallet_name: &str, area: Rect) {
    let wallet = app.wallets.iter().find(|w| w.name == wallet_name);

    let content = if let Some(wallet) = wallet {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Wallet: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    &wallet.name,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
        ];

        // Threshold info
        if let (Some(t), Some(n)) = (wallet.threshold, wallet.total_parties) {
            lines.push(Line::from(vec![
                Span::styled("Threshold: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{}-of-{}", t, n),
                    Style::default().fg(Color::Yellow),
                ),
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

        // Address
        if let Some(addr) = &wallet.address {
            lines.push(Line::from(vec![Span::styled(
                format!("Address ({}): ", app.network.display_name()),
                Style::default().fg(Color::Gray),
            )]));
            lines.push(Line::from(vec![Span::styled(
                addr.clone(),
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
                Span::styled(
                    format!("({:.8} BTC)", btc),
                    Style::default().fg(Color::Gray),
                ),
            ]));

            lines.push(Line::from(vec![
                Span::styled("UTXOs: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{}", info.utxo_count),
                    Style::default().fg(Color::White),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("Balance: ", Style::default().fg(Color::Gray)),
                Span::styled("Press 'b' to fetch", Style::default().fg(Color::DarkGray)),
            ]));
        }

        lines
    } else {
        vec![Line::from(Span::styled(
            "Wallet not found",
            Style::default().fg(Color::Red),
        ))]
    };

    let details = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Wallet Info ")
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(details, area);
}

fn render_action_menu(frame: &mut Frame, state: &WalletDetailsState, area: Rect) {
    let actions = WalletAction::all();

    let items: Vec<ListItem> = actions
        .iter()
        .enumerate()
        .map(|(i, action)| {
            let is_selected = i == state.selected_action;
            let prefix = if is_selected { "▶ " } else { "  " };

            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let content = vec![
                Line::from(Span::styled(format!("{}{}", prefix, action.label()), style)),
                Line::from(Span::styled(
                    format!("    {}", action.description()),
                    Style::default().fg(Color::DarkGray),
                )),
            ];

            ListItem::new(content)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Actions ")
            .border_style(Style::default().fg(Color::Green)),
    );

    frame.render_widget(list, area);

    // Help text at bottom
    let help_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(2),
        width: area.width,
        height: 2,
    };

    let help = Paragraph::new(Line::from(vec![
        Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
        Span::raw(" Navigate  "),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::raw(" Select  "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(" Back"),
    ]))
    .style(Style::default().fg(Color::DarkGray));

    frame.render_widget(help, help_area);
}
