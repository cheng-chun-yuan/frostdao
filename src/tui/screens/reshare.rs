//! Reshare wizard screens

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::app::App;

/// Render reshare wizard (placeholder for now)
pub fn render_reshare(frame: &mut Frame, _app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Reshare Wizard - Coming Soon ");

    let paragraph = Paragraph::new("Reshare wizard will be implemented in Commit 4")
        .block(block);

    frame.render_widget(paragraph, area);
}
