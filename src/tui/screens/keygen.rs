//! Keygen wizard screens

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::app::App;

/// Render keygen wizard (placeholder for now)
pub fn render_keygen(frame: &mut Frame, _app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Keygen Wizard - Coming Soon ");

    let paragraph = Paragraph::new("Keygen wizard will be implemented in Commit 3")
        .block(block);

    frame.render_widget(paragraph, area);
}
