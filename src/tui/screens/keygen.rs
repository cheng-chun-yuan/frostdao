//! Keygen wizard screens

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::tui::app::App;
use crate::tui::components::{TextArea, TextInput};
use crate::tui::state::{KeygenFormField, KeygenState};

/// Keygen wizard form data
#[derive(Clone, Default)]
pub struct KeygenFormData {
    pub name: TextInput,
    pub threshold: TextInput,
    pub n_parties: TextInput,
    pub my_index: TextInput,
    pub my_rank: TextInput,
    pub hierarchical: bool,
    pub focused_field: KeygenFormField,
    pub round1_output: String,
    pub round2_input: TextArea,
    pub round2_output: String,
    pub finalize_input: TextArea,
    pub error_message: Option<String>,
}

impl KeygenFormData {
    pub fn new() -> Self {
        Self {
            name: TextInput::new("Wallet Name").with_placeholder("my_wallet"),
            threshold: TextInput::new("Threshold").with_value("2").numeric(),
            n_parties: TextInput::new("Total Parties").with_value("3").numeric(),
            my_index: TextInput::new("My Index").with_value("1").numeric(),
            my_rank: TextInput::new("My Rank").with_value("0").numeric(),
            hierarchical: false,
            focused_field: KeygenFormField::Name,
            round1_output: String::new(),
            round2_input: TextArea::new("Paste Round 1 outputs from all parties"),
            round2_output: String::new(),
            finalize_input: TextArea::new("Paste Round 2 outputs from all parties"),
            error_message: None,
        }
    }
}

/// Render keygen wizard
pub fn render_keygen(frame: &mut Frame, app: &App, form: &KeygenFormData, area: Rect) {
    match &app.state {
        crate::tui::state::AppState::Keygen(state) => match state {
            KeygenState::Round1Setup => render_round1_setup(frame, form, area),
            KeygenState::Round1Output { output_json } => {
                render_round1_output(frame, output_json, area)
            }
            KeygenState::Round2Input => render_round2_input(frame, form, area),
            KeygenState::Round2Output { output_json } => {
                render_round2_output(frame, output_json, area)
            }
            KeygenState::FinalizeInput => render_finalize_input(frame, form, area),
            KeygenState::Complete { wallet_name } => render_complete(frame, wallet_name, area),
        },
        _ => {}
    }
}

fn render_round1_setup(frame: &mut Frame, form: &KeygenFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Keygen Wizard - Round 1: Setup ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Name
            Constraint::Length(3), // Threshold
            Constraint::Length(3), // N Parties
            Constraint::Length(3), // My Index
            Constraint::Length(3), // My Rank
            Constraint::Length(3), // Hierarchical toggle
            Constraint::Min(1),    // Spacer
            Constraint::Length(2), // Error message
            Constraint::Length(2), // Help
        ])
        .split(inner);

    // Name input
    form.name
        .render(frame, chunks[0], form.focused_field == KeygenFormField::Name);

    // Threshold input
    form.threshold.render(
        frame,
        chunks[1],
        form.focused_field == KeygenFormField::Threshold,
    );

    // N Parties input
    form.n_parties.render(
        frame,
        chunks[2],
        form.focused_field == KeygenFormField::NParties,
    );

    // My Index input
    form.my_index.render(
        frame,
        chunks[3],
        form.focused_field == KeygenFormField::MyIndex,
    );

    // My Rank input
    form.my_rank.render(
        frame,
        chunks[4],
        form.focused_field == KeygenFormField::MyRank,
    );

    // Hierarchical toggle
    let hierarchical_focused = form.focused_field == KeygenFormField::Hierarchical;
    let checkbox = if form.hierarchical { "[x]" } else { "[ ]" };
    let checkbox_style = if hierarchical_focused {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let hierarchical_line = Line::from(vec![
        Span::styled(checkbox, checkbox_style),
        Span::raw(" Enable Hierarchical TSS (HTSS)"),
    ]);
    let hierarchical_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if hierarchical_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::Gray)
        })
        .title("Mode");
    let hierarchical_para = Paragraph::new(hierarchical_line).block(hierarchical_block);
    frame.render_widget(hierarchical_para, chunks[5]);

    // Error message
    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str())
            .style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, chunks[7]);
    }

    // Help text
    let help = Paragraph::new("Tab: Next field | Space: Toggle | Enter: Generate | Esc: Cancel")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[8]);
}

fn render_round1_output(frame: &mut Frame, output_json: &str, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Keygen Wizard - Round 1: Your Commitment ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Instructions
            Constraint::Min(5),    // Output
            Constraint::Length(2), // Help
        ])
        .split(inner);

    // Instructions
    let instructions = Paragraph::new("Share this with all parties:")
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(instructions, chunks[0]);

    // Output JSON
    let output_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title("Output (copy this)");
    let output_para = Paragraph::new(output_json)
        .block(output_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(output_para, chunks[1]);

    // Help
    let help = Paragraph::new("c: Copy to clipboard | Enter: Continue to Round 2 | Esc: Cancel")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[2]);
}

fn render_round2_input(frame: &mut Frame, form: &KeygenFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Keygen Wizard - Round 2: Enter All Commitments ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Instructions
            Constraint::Min(5),    // Input area
            Constraint::Length(2), // Error
            Constraint::Length(2), // Help
        ])
        .split(inner);

    // Instructions
    let instructions =
        Paragraph::new("Paste all Round 1 outputs from all parties (space-separated JSON):")
            .style(Style::default().fg(Color::Yellow));
    frame.render_widget(instructions, chunks[0]);

    // Input area
    form.round2_input.render(frame, chunks[1], true);

    // Error
    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str())
            .style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, chunks[2]);
    }

    // Help
    let help = Paragraph::new("Ctrl+V: Paste | Enter: Generate Shares | Esc: Back")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[3]);
}

fn render_round2_output(frame: &mut Frame, output_json: &str, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Keygen Wizard - Round 2: Your Shares ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Instructions
            Constraint::Min(5),    // Output
            Constraint::Length(2), // Help
        ])
        .split(inner);

    // Instructions
    let instructions = Paragraph::new("Share this with all parties:")
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(instructions, chunks[0]);

    // Output JSON
    let output_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title("Output (copy this)");
    let output_para = Paragraph::new(output_json)
        .block(output_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(output_para, chunks[1]);

    // Help
    let help = Paragraph::new("c: Copy to clipboard | Enter: Continue to Finalize | Esc: Cancel")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[2]);
}

fn render_finalize_input(frame: &mut Frame, form: &KeygenFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Keygen Wizard - Finalize: Enter All Shares ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Instructions
            Constraint::Min(5),    // Input area
            Constraint::Length(2), // Error
            Constraint::Length(2), // Help
        ])
        .split(inner);

    // Instructions
    let instructions =
        Paragraph::new("Paste all Round 2 outputs from all parties (space-separated JSON):")
            .style(Style::default().fg(Color::Yellow));
    frame.render_widget(instructions, chunks[0]);

    // Input area
    form.finalize_input.render(frame, chunks[1], true);

    // Error
    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str())
            .style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, chunks[2]);
    }

    // Help
    let help = Paragraph::new("Ctrl+V: Paste | Enter: Finalize Wallet | Esc: Back")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[3]);
}

fn render_complete(frame: &mut Frame, wallet_name: &str, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title(" Keygen Wizard - Complete! ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Success message
            Constraint::Min(3),    // Wallet info
            Constraint::Length(2), // Help
        ])
        .split(inner);

    // Success message
    let success = Paragraph::new(Line::from(vec![
        Span::styled("✓ ", Style::default().fg(Color::Green)),
        Span::styled(
            "Wallet created successfully!",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    frame.render_widget(success, chunks[0]);

    // Wallet info
    let info = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Wallet: ", Style::default().fg(Color::Gray)),
            Span::styled(
                wallet_name,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from("Your wallet is now ready to use."),
        Line::from("You can view it in the wallet list."),
    ]);
    frame.render_widget(info, chunks[1]);

    // Help
    let help = Paragraph::new("Enter/Esc: Return to wallet list")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[2]);
}
