//! Miniscript policy preview screen.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::tui::app::App;
use crate::tui::components::TextArea;

#[derive(Clone)]
pub struct PolicyPreviewFormData {
    pub policy_input: TextArea,
    pub output: String,
    pub error: Option<String>,
}

impl Default for PolicyPreviewFormData {
    fn default() -> Self {
        Self::new()
    }
}

impl PolicyPreviewFormData {
    pub fn new() -> Self {
        let mut policy_input =
            TextArea::new("Miniscript Policy").with_placeholder("thresh(2,pk(A),pk(B),pk(C))");
        policy_input.set_content("thresh(2,pk(A),pk(B),pk(C))");

        Self {
            policy_input,
            output: String::new(),
            error: None,
        }
    }
}

pub fn render_policy_preview(
    frame: &mut Frame,
    app: &App,
    form: &PolicyPreviewFormData,
    area: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Percentage(35),
            Constraint::Percentage(45),
            Constraint::Length(4),
        ])
        .split(area);

    let header = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "Miniscript Policy Preview",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("Compile Taproot Miniscript policies before wiring them into wallet flows."),
        Line::from("This preview does not spend funds or create script-path signatures."),
    ])
    .block(Block::default().borders(Borders::ALL).title(" Policy "));
    frame.render_widget(header, chunks[0]);

    form.policy_input.render(frame, chunks[1], true);

    let output = if let Some(error) = &form.error {
        vec![Line::from(vec![
            Span::styled("Error: ", Style::default().fg(Color::Red)),
            Span::raw(error),
        ])]
    } else if form.output.trim().is_empty() {
        vec![Line::from(Span::styled(
            "Press Enter to compile the policy.",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        form.output.lines().map(Line::from).collect()
    };

    let output_widget = Paragraph::new(output)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Compiled Descriptor "),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(output_widget, chunks[2]);

    let feature_status = if cfg!(feature = "miniscript-policy") {
        "enabled"
    } else {
        "disabled"
    };
    let help = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" Compile   "),
            Span::styled("c", Style::default().fg(Color::Yellow)),
            Span::raw(" Copy   "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" Back   "),
            Span::styled("Feature: ", Style::default().fg(Color::Gray)),
            Span::styled(feature_status, Style::default().fg(Color::Green)),
        ]),
        Line::from(format!("Network context: {}", app.network.display_name())),
    ])
    .block(Block::default().borders(Borders::ALL).title(" Help "));
    frame.render_widget(help, chunks[3]);
}
