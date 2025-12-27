//! Chain selection popup

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::tui::app::App;
use crate::tui::state::NetworkSelection;

/// Render chain selection popup
pub fn render_chain_select(frame: &mut Frame, app: &App, area: Rect) {
    // Create centered popup
    let popup_area = centered_rect(40, 40, area);

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
                NetworkSelection::Mainnet => " (CAUTION: Real funds!)",
                _ => "",
            };

            ListItem::new(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(network.display_name(), style),
                Span::styled(suffix, Style::default().fg(Color::Red)),
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

    // Enhanced help text
    let help_lines = vec![
        Line::from(vec![
            Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
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

    frame.render_widget(help, chunks[2]);
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
