//! HD Address list screen

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::tui::state::AddressListState;

/// Render the HD address list screen
pub fn render_address_list(frame: &mut Frame, state: &AddressListState, area: Rect) {
    let block = Block::default()
        .title(format!("HD Addresses - {}", state.wallet_name))
        .borders(Borders::ALL);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Check for error
    if let Some(ref error) = state.error {
        let error_para = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                error.as_str(),
                Style::default().fg(Color::Red),
            )),
            Line::from(""),
            Line::from("HD derivation requires a wallet created with the latest keygen."),
            Line::from("Run keygen-finalize again to enable HD support."),
        ])
        .block(Block::default());
        frame.render_widget(error_para, inner);
        return;
    }

    if state.addresses.is_empty() {
        let empty_para =
            Paragraph::new("Loading addresses...").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty_para, inner);
        return;
    }

    // Split into list and details
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(inner);

    // Address list
    let items: Vec<ListItem> = state
        .addresses
        .iter()
        .enumerate()
        .map(|(i, (addr, _pubkey, index))| {
            let style = if i == state.selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let prefix = if i == state.selected { ">> " } else { "   " };
            ListItem::new(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{:>3} ", index), Style::default().fg(Color::Gray)),
                Span::styled(truncate_address(addr, 40), style),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title("Receive Addresses (m/44'/0'/0'/0/*)")
            .borders(Borders::ALL),
    );
    frame.render_widget(list, chunks[0]);

    // Selected address details
    if let Some((addr, pubkey, index)) = state.addresses.get(state.selected) {
        let details = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("Index: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{}", index),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Path: ",
                Style::default().fg(Color::Gray),
            )]),
            Line::from(vec![Span::styled(
                format!("m/44'/0'/0'/0/{}", index),
                Style::default().fg(Color::Cyan),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Address: ",
                Style::default().fg(Color::Gray),
            )]),
            Line::from(vec![Span::styled(
                &addr[..addr.len().min(20)],
                Style::default().fg(Color::Green),
            )]),
            Line::from(vec![Span::styled(
                &addr[addr.len().min(20)..addr.len().min(40)],
                Style::default().fg(Color::Green),
            )]),
            Line::from(vec![Span::styled(
                &addr[addr.len().min(40)..],
                Style::default().fg(Color::Green),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Public Key: ",
                Style::default().fg(Color::Gray),
            )]),
            Line::from(vec![Span::styled(
                &pubkey[..pubkey.len().min(32)],
                Style::default().fg(Color::Magenta),
            )]),
            Line::from(vec![Span::styled(
                &pubkey[pubkey.len().min(32)..],
                Style::default().fg(Color::Magenta),
            )]),
        ])
        .block(Block::default().title("Details").borders(Borders::ALL));
        frame.render_widget(details, chunks[1]);
    }
}

fn truncate_address(addr: &str, max_len: usize) -> String {
    if addr.len() <= max_len {
        addr.to_string()
    } else {
        format!("{}...", &addr[..max_len - 3])
    }
}
