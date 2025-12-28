//! HD Address list screen

use qrcode::QrCode;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::tui::state::AddressListState;

/// Render the HD address list screen
pub fn render_address_list(frame: &mut Frame, state: &AddressListState, area: Rect) {
    // Main layout: list on left, details on right
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    // Check for error
    if let Some(ref error) = state.error {
        let error_para = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("⚠ {}", error),
                Style::default().fg(Color::Red),
            )),
            Line::from(""),
            Line::from("HD derivation requires a wallet created with the latest keygen."),
        ])
        .block(
            Block::default()
                .title(format!(" {} ", state.wallet_name))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        );
        frame.render_widget(error_para, area);
        return;
    }

    if state.addresses.is_empty() {
        let empty_para = Paragraph::new("Loading addresses...")
            .style(Style::default().fg(Color::DarkGray))
            .block(
                Block::default()
                    .title(format!(" {} ", state.wallet_name))
                    .borders(Borders::ALL),
            );
        frame.render_widget(empty_para, area);
        return;
    }

    // Left: Address list
    let items: Vec<ListItem> = state
        .addresses
        .iter()
        .enumerate()
        .map(|(i, (addr, _pubkey, index))| {
            let is_selected = i == state.selected;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let prefix = if is_selected { "▶ " } else { "  " };

            // Show balance indicator if cached
            let balance_indicator = if let Some((bal, _)) = state.balance_cache.get(index) {
                if *bal > 0 {
                    format!(" 💰{}", format_sats(*bal))
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            ListItem::new(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{} ", index), Style::default().fg(Color::DarkGray)),
                Span::styled(truncate_address(addr, 28), style),
                Span::styled(balance_indicator, Style::default().fg(Color::Green)),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Addresses ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(list, main_chunks[0]);

    // Right: Details panel with QR code
    if let Some((addr, _pubkey, index)) = state.addresses.get(state.selected) {
        render_details_panel(frame, state, addr, *index, main_chunks[1]);
    }
}

fn render_details_panel(
    frame: &mut Frame,
    state: &AddressListState,
    addr: &str,
    index: u32,
    area: Rect,
) {
    let block = Block::default()
        .title(" Details ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Layout: info at top, QR in middle, help at bottom
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // Info section
            Constraint::Min(12),   // QR code
            Constraint::Length(1), // Help
        ])
        .split(inner);

    // Info section
    let mut info_lines = vec![
        Line::from(vec![
            Span::styled("  Path: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("m/44'/0'/0'/0/{}", index),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Address:",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(vec![Span::styled(
            format!("  {}", addr),
            Style::default().fg(Color::Yellow),
        )]),
    ];

    // Add balance if cached
    if let Some((balance, utxo_count)) = state.balance_cache.get(&index) {
        info_lines.push(Line::from(""));
        let btc = *balance as f64 / 100_000_000.0;
        info_lines.push(Line::from(vec![
            Span::styled("  Balance: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} sats", balance),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" ({:.8} BTC) ", btc),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{} UTXOs", utxo_count),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    let info = Paragraph::new(info_lines);
    frame.render_widget(info, chunks[0]);

    // QR Code
    render_qr_code(frame, addr, chunks[1]);

    // Help text
    let help = Paragraph::new(Line::from(vec![
        Span::styled("c", Style::default().fg(Color::Yellow)),
        Span::styled(" Copy ", Style::default().fg(Color::DarkGray)),
        Span::styled("b", Style::default().fg(Color::Yellow)),
        Span::styled(" Bal ", Style::default().fg(Color::DarkGray)),
        Span::styled("+/a", Style::default().fg(Color::Green)),
        Span::styled(" Add ", Style::default().fg(Color::DarkGray)),
        Span::styled("-/x", Style::default().fg(Color::Red)),
        Span::styled(" Del ", Style::default().fg(Color::DarkGray)),
        Span::styled("↑↓", Style::default().fg(Color::Yellow)),
        Span::styled(" Nav ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::styled(" Back", Style::default().fg(Color::DarkGray)),
    ]));
    frame.render_widget(help, chunks[2]);
}

/// Render a QR code for the given address (square aspect ratio)
fn render_qr_code(frame: &mut Frame, address: &str, area: Rect) {
    let qr_lines = match QrCode::new(address.as_bytes()) {
        Ok(code) => {
            let mut lines = Vec::new();
            let width = code.width();
            let colors = code.to_colors();

            // Use Unicode half blocks - each character represents 2 vertical pixels
            // Single character width per QR pixel for square appearance
            for y in (0..width).step_by(2) {
                let mut line_chars = String::new();
                for x in 0..width {
                    let top = colors[y * width + x] == qrcode::Color::Dark;
                    let bottom = if y + 1 < width {
                        colors[(y + 1) * width + x] == qrcode::Color::Dark
                    } else {
                        false
                    };

                    // Single character per pixel for square ratio
                    let ch = match (top, bottom) {
                        (true, true) => "█",
                        (true, false) => "▀",
                        (false, true) => "▄",
                        (false, false) => " ",
                    };
                    line_chars.push_str(ch);
                }
                lines.push(Line::from(Span::styled(
                    line_chars,
                    Style::default().fg(Color::White),
                )));
            }
            lines
        }
        Err(_) => {
            vec![Line::from(Span::styled(
                "QR generation failed",
                Style::default().fg(Color::Red),
            ))]
        }
    };

    let qr_para = Paragraph::new(qr_lines).alignment(Alignment::Center);
    frame.render_widget(qr_para, area);
}

fn format_sats(sats: u64) -> String {
    if sats >= 100_000_000 {
        format!("{:.2}BTC", sats as f64 / 100_000_000.0)
    } else if sats >= 1_000_000 {
        format!("{:.1}M", sats as f64 / 1_000_000.0)
    } else if sats >= 1_000 {
        format!("{}K", sats / 1_000)
    } else {
        format!("{}", sats)
    }
}

fn truncate_address(addr: &str, max_len: usize) -> String {
    if addr.len() <= max_len {
        addr.to_string()
    } else {
        format!("{}...", &addr[..max_len - 3])
    }
}
