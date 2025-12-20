//! Send wizard screens

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::app::App;

/// Render send wizard (placeholder for now)
pub fn render_send(frame: &mut Frame, _app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Send Wizard - Coming Soon ");

    let paragraph = Paragraph::new("Send wizard will be implemented in Commit 5")
        .block(block);

    frame.render_widget(paragraph, area);
}
