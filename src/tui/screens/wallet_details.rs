//! Wallet details screen with action menu

use qrcode::QrCode;
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

    // Render QR code popup if showing
    if state.show_qr {
        let wallet = app.wallets.iter().find(|w| w.name == state.wallet_name);
        if let Some(addr) = wallet.and_then(|w| w.address.as_ref()) {
            render_qr_popup(frame, addr, area);
        }
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

        // Threshold info with signing requirement for HTSS
        if let (Some(t), Some(n)) = (wallet.threshold, wallet.total_parties) {
            let threshold_display = if wallet.hierarchical.unwrap_or(false) {
                if let Some(ref req) = wallet.signing_requirement {
                    // Show signing requirement like "5-of-8 (1,2,2)"
                    let req_str: Vec<String> = req.iter().map(|r| r.to_string()).collect();
                    format!("{}-of-{} ({})", t, n, req_str.join(","))
                } else {
                    format!("{}-of-{}", t, n)
                }
            } else {
                format!("{}-of-{}", t, n)
            };

            lines.push(Line::from(vec![
                Span::styled("Threshold: ", Style::default().fg(Color::Gray)),
                Span::styled(threshold_display, Style::default().fg(Color::Yellow)),
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

        // Add hint for QR code
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::DarkGray)),
            Span::styled("q", Style::default().fg(Color::Yellow)),
            Span::styled(" for QR code", Style::default().fg(Color::DarkGray)),
        ]));

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

/// Render QR code popup overlay
fn render_qr_popup(frame: &mut Frame, address: &str, area: Rect) {
    use ratatui::widgets::Clear;

    let qr_lines = match QrCode::new(address.as_bytes()) {
        Ok(code) => {
            let width = code.width();
            let mut lines: Vec<Line> = Vec::new();

            // Use half-block characters for better resolution
            for y in (0..width).step_by(2) {
                let mut spans: Vec<Span> = Vec::new();
                for x in 0..width {
                    let top = code[(x, y)] == qrcode::Color::Dark;
                    let bottom = if y + 1 < width {
                        code[(x, y + 1)] == qrcode::Color::Dark
                    } else {
                        false
                    };

                    let ch = match (top, bottom) {
                        (true, true) => "█",
                        (true, false) => "▀",
                        (false, true) => "▄",
                        (false, false) => " ",
                    };
                    spans.push(Span::styled(ch, Style::default().fg(Color::White)));
                }
                lines.push(Line::from(spans));
            }

            // Add address below QR code
            lines.push(Line::from(""));
            // Show address in chunks for readability
            let addr_len = address.len();
            if addr_len > 40 {
                lines.push(Line::from(Span::styled(
                    &address[..addr_len / 2],
                    Style::default().fg(Color::Green),
                )));
                lines.push(Line::from(Span::styled(
                    &address[addr_len / 2..],
                    Style::default().fg(Color::Green),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    address,
                    Style::default().fg(Color::Green),
                )));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Press q or Esc to close",
                Style::default().fg(Color::DarkGray),
            )));

            (lines, width)
        }
        Err(_) => {
            let lines = vec![
                Line::from(Span::styled(
                    "QR code generation failed",
                    Style::default().fg(Color::Red),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Press q or Esc to close",
                    Style::default().fg(Color::DarkGray),
                )),
            ];
            (lines, 30)
        }
    };

    let (lines, qr_width) = qr_lines;

    // Calculate popup size based on QR code
    let popup_width = (qr_width as u16 + 4).max(50);
    let popup_height = (lines.len() as u16 + 2).min(area.height - 2);

    // Center the popup
    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(popup_width)) / 2,
        y: area.y + (area.height.saturating_sub(popup_height)) / 2,
        width: popup_width.min(area.width),
        height: popup_height,
    };

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let qr_widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" QR Code ")
                .border_style(Style::default().fg(Color::Green))
                .style(Style::default().bg(Color::Black)),
        )
        .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(qr_widget, popup_area);
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
