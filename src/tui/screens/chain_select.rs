//! Chain selection popup

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::tui::app::App;
use crate::tui::state::NetworkSelection;

/// Render chain selection popup
pub fn render_chain_select(frame: &mut Frame, app: &App, area: Rect) {
    // Create centered popup
    let popup_area = centered_rect(40, 30, area);

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
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(inner);

    // Network list
    let items: Vec<ListItem> = NetworkSelection::all()
        .iter()
        .map(|network| {
            let is_selected = *network == app.network;
            let prefix = if is_selected { "● " } else { "○ " };
            let style = if is_selected {
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

    let list = List::new(items).highlight_style(Style::default().bg(Color::DarkGray));

    frame.render_widget(list, chunks[1]);

    // Help text
    let help = Paragraph::new(Line::from(vec![
        Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
        Span::raw(": Select  "),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::raw(": Confirm  "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(": Cancel"),
    ]))
    .alignment(Alignment::Center);

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
