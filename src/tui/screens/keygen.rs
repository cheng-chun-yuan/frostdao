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
            KeygenState::ModeSelect => render_mode_select(frame, form, area),
            KeygenState::ParamsSetup => render_params_setup(frame, form, area),
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

fn render_mode_select(frame: &mut Frame, form: &KeygenFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Keygen - Step 1: Choose Mode ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(6), // TSS option
            Constraint::Length(6), // HTSS option
            Constraint::Min(1),    // Spacer
            Constraint::Length(2), // Help
        ])
        .split(inner);

    let title = Paragraph::new(vec![Line::from(vec![Span::styled(
        "Select signature scheme:",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )])]);
    frame.render_widget(title, chunks[0]);

    // TSS option
    let tss_selected = !form.hierarchical;
    let tss_style = if tss_selected {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let tss_arrow = if tss_selected { "▶ " } else { "  " };
    let tss_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if tss_selected {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Gray)
        });
    let tss_para = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(tss_arrow, tss_style),
            Span::styled("[1] TSS - Threshold Signature (t-of-n)", tss_style),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("    Any ", Style::default().fg(Color::Gray)),
            Span::styled("t", Style::default().fg(Color::Cyan)),
            Span::styled(" of ", Style::default().fg(Color::Gray)),
            Span::styled("n", Style::default().fg(Color::Cyan)),
            Span::styled(
                " parties can sign (e.g., 2-of-3)",
                Style::default().fg(Color::Gray),
            ),
        ]),
    ])
    .block(tss_block);
    frame.render_widget(tss_para, chunks[1]);

    // HTSS option
    let htss_selected = form.hierarchical;
    let htss_style = if htss_selected {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let htss_arrow = if htss_selected { "▶ " } else { "  " };
    let htss_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if htss_selected {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Gray)
        });
    let htss_para = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(htss_arrow, htss_style),
            Span::styled("[2] HTSS - Hierarchical TSS (Rank-Based)", htss_style),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("    Parties with ", Style::default().fg(Color::Gray)),
            Span::styled("rank sum ≥ threshold", Style::default().fg(Color::Cyan)),
            Span::styled(" can sign", Style::default().fg(Color::Gray)),
        ]),
    ])
    .block(htss_block);
    frame.render_widget(htss_para, chunks[2]);

    let help = Paragraph::new("↑/↓ or 1/2: Select | Enter: Continue | Esc: Cancel")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[4]);
}

fn render_params_setup(frame: &mut Frame, form: &KeygenFormData, area: Rect) {
    let title = if form.hierarchical {
        " Keygen - Step 2: HTSS Parameters "
    } else {
        " Keygen - Step 2: TSS Parameters "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if form.hierarchical {
        // HTSS mode: Name, N Parties
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Name
                Constraint::Length(3), // N Parties
                Constraint::Length(5), // Explanation
                Constraint::Min(1),    // Spacer
                Constraint::Length(2), // Error
                Constraint::Length(2), // Help
            ])
            .split(inner);

        form.name.render(
            frame,
            chunks[0],
            form.focused_field == KeygenFormField::Name,
        );
        form.n_parties.render(
            frame,
            chunks[1],
            form.focused_field == KeygenFormField::NParties,
        );

        let n: u32 = form.n_parties.value().parse().unwrap_or(3);
        let explanation = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "HTSS: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Hierarchical Threshold Signature"),
            ]),
            Line::from(vec![
                Span::styled("  Ranks: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("0, 1, 2, ... {}", n.saturating_sub(1)),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Rule: ", Style::default().fg(Color::Gray)),
                Span::raw("Signers' ranks (sorted) must satisfy rank[i] <= i"),
            ]),
        ]);
        frame.render_widget(explanation, chunks[2]);

        if let Some(error) = &form.error_message {
            let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
            frame.render_widget(error_para, chunks[4]);
        }

        let help = Paragraph::new("Tab: Next field | Enter: Generate All Parties | Esc: Back")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(help, chunks[5]);
    } else {
        // TSS mode: Name, Threshold, N Parties
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Name
                Constraint::Length(3), // Threshold
                Constraint::Length(3), // N Parties
                Constraint::Length(4), // Explanation
                Constraint::Min(1),    // Spacer
                Constraint::Length(2), // Error
                Constraint::Length(2), // Help
            ])
            .split(inner);

        form.name.render(
            frame,
            chunks[0],
            form.focused_field == KeygenFormField::Name,
        );
        form.threshold.render(
            frame,
            chunks[1],
            form.focused_field == KeygenFormField::Threshold,
        );
        form.n_parties.render(
            frame,
            chunks[2],
            form.focused_field == KeygenFormField::NParties,
        );

        let t: u32 = form.threshold.value().parse().unwrap_or(2);
        let n: u32 = form.n_parties.value().parse().unwrap_or(3);
        let explanation = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "TSS: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}-of-{}", t, n),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" threshold signature"),
            ]),
            Line::from(vec![Span::raw(format!(
                "  Any {} of {} parties can sign together",
                t, n
            ))]),
        ]);
        frame.render_widget(explanation, chunks[3]);

        if let Some(error) = &form.error_message {
            let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
            frame.render_widget(error_para, chunks[5]);
        }

        let help = Paragraph::new("Tab: Next field | Enter: Generate All Parties | Esc: Back")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(help, chunks[6]);
    }
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
    let instructions =
        Paragraph::new("Share this with all parties:").style(Style::default().fg(Color::Yellow));
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
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
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
    let instructions =
        Paragraph::new("Share this with all parties:").style(Style::default().fg(Color::Yellow));
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
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
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
