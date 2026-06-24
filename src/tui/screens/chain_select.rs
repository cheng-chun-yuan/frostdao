//! Chain selection popup

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::tui::app::App;
use crate::tui::state::NetworkSelection;

/// Render chain selection popup
pub fn render_chain_select(frame: &mut Frame, app: &App, area: Rect) {
    // Create centered popup
    let popup_area = centered_rect(50, 48, area);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Select Network ");

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(5),
            Constraint::Length(4),
        ])
        .split(inner);

    // Network list with hover highlight
    let networks = NetworkSelection::all();
    let items: Vec<ListItem> = networks
        .iter()
        .enumerate()
        .map(|(idx, network)| {
            let is_current = *network == app.network;
            let is_hovered = idx == app.chain_selector_index;
            let prefix = if is_current { "● " } else { "○ " };

            let style = if is_hovered {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if is_current {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let suffix = match network {
                NetworkSelection::Regtest => " (local API)",
                NetworkSelection::Mainnet => " (CAUTION: Real funds!)",
                _ => "",
            };

            let suffix_color = match network {
                NetworkSelection::Regtest => Color::Cyan,
                NetworkSelection::Mainnet => Color::Red,
                _ => Color::White,
            };

            ListItem::new(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(network.display_name(), style),
                Span::styled(suffix, Style::default().fg(suffix_color)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    // Use ListState for proper highlight rendering
    let mut list_state = ListState::default();
    list_state.select(Some(app.chain_selector_index));

    frame.render_stateful_widget(list, chunks[1], &mut list_state);

    let selected_network = networks
        .get(app.chain_selector_index)
        .copied()
        .unwrap_or(app.network);
    let policy = Paragraph::new(chain_select_policy_lines(selected_network))
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(policy, chunks[2]);

    // Enhanced help text
    let help_lines = vec![
        Line::from(vec![
            Span::styled("j/k/↑/↓", Style::default().fg(Color::Yellow)),
            Span::raw(": Navigate  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(": Confirm  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(": Cancel"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("●", Style::default().fg(Color::Cyan)),
            Span::raw(" = Current  "),
            Span::styled("▶", Style::default().fg(Color::Yellow)),
            Span::raw(" = Selected"),
        ]),
    ];

    let help = Paragraph::new(help_lines).alignment(Alignment::Center);

    frame.render_widget(help, chunks[3]);
}

pub(crate) fn chain_select_policy_lines(network: NetworkSelection) -> Vec<Line<'static>> {
    let policy_color = match network {
        NetworkSelection::Regtest => Color::Cyan,
        NetworkSelection::Mainnet => Color::Red,
        _ => Color::Yellow,
    };

    vec![
        Line::from(vec![
            Span::styled("Selected policy: ", Style::default().fg(Color::Gray)),
            Span::styled(network.policy_hint(), Style::default().fg(policy_color)),
        ]),
        Line::from(vec![
            Span::styled("Address scope: ", Style::default().fg(Color::Gray)),
            Span::styled(
                network.address_scope_hint(),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::styled("UTXO API: ", Style::default().fg(Color::Gray)),
            Span::styled(network.utxo_api_hint(), Style::default().fg(policy_color)),
        ]),
        Line::from(Span::styled(
            "Confirming clears pending send form data and volatile Nostr ceremony state.",
            Style::default().fg(Color::DarkGray),
        )),
    ]
}

/// Create a centered rectangle of given percentage width and height
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines_to_string(lines: Vec<Line<'_>>) -> String {
        lines
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn chain_select_policy_lines_warn_for_mainnet() {
        let rendered = lines_to_string(chain_select_policy_lines(NetworkSelection::Mainnet));

        assert!(rendered.contains("Selected policy: MAINNET real funds"));
        assert!(rendered.contains("explicit opt-in"));
        assert!(rendered.contains("Bitcoin mainnet root address"));
        assert!(rendered.contains("UTXO API: https://mempool.space/api"));
    }

    #[test]
    fn chain_select_policy_lines_explain_regtest_local_node() {
        let rendered = lines_to_string(chain_select_policy_lines(NetworkSelection::Regtest));

        assert!(rendered.contains("regtest uses local Esplora/mempool API"));
        assert!(rendered.contains("FROSTDAO_REGTEST_MEMPOOL_API"));
        assert!(rendered.contains("local regtest root address with bcrt prefix"));
        assert!(rendered.contains("UTXO API: regtest needs a local Esplora/mempool API endpoint"));
    }

    #[test]
    fn chain_select_policy_lines_show_remote_testnet_sources() {
        let rendered = lines_to_string(chain_select_policy_lines(NetworkSelection::Signet));

        assert!(rendered.contains("signet remote UTXOs via mempool.space"));
        assert!(rendered.contains("UTXO API: https://mempool.space/signet/api"));
        assert!(rendered.contains("Confirming clears pending send form data"));
    }
}
