//! Reshare wizard screens

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::tui::app::App;
use crate::tui::components::{TextArea, TextInput};
use crate::tui::state::{
    ReshareFinalizeField, ReshareFormField, ReshareLocalField, ReshareMode, ReshareState,
};

/// Reshare wizard form data
#[derive(Clone)]
pub struct ReshareFormData {
    // Mode selection
    pub mode: ReshareMode,
    pub mode_selected_index: usize,
    // Source wallet
    pub source_wallet_index: usize,
    // Distributed mode fields
    pub new_threshold: TextInput,
    pub new_n_parties: TextInput,
    pub focused_field: ReshareFormField,
    pub round1_output: String,
    // Finalize fields (distributed)
    pub target_name: TextInput,
    pub my_new_index: TextInput,
    pub my_rank: TextInput,
    pub hierarchical: bool,
    pub finalize_input: TextArea,
    pub finalize_field: ReshareFinalizeField,
    // Local mode fields
    pub local_target_name: TextInput,
    pub local_new_threshold: TextInput,
    pub local_new_n_parties: TextInput,
    pub local_field: ReshareLocalField,
    // Common
    pub error_message: Option<String>,
}

impl Default for ReshareFormData {
    fn default() -> Self {
        Self::new()
    }
}

impl ReshareFormData {
    pub fn new() -> Self {
        Self {
            mode: ReshareMode::Local,
            mode_selected_index: 0,
            source_wallet_index: 0,
            new_threshold: TextInput::new("New Threshold").with_value("2").numeric(),
            new_n_parties: TextInput::new("New Total Parties")
                .with_value("3")
                .numeric(),
            focused_field: ReshareFormField::SourceWallet,
            round1_output: String::new(),
            target_name: TextInput::new("New Wallet Name").with_placeholder("reshared_wallet"),
            my_new_index: TextInput::new("My New Index").with_value("1").numeric(),
            my_rank: TextInput::new("My Rank").with_value("0").numeric(),
            hierarchical: false,
            finalize_input: TextArea::new("Paste Round 1 outputs from old parties"),
            finalize_field: ReshareFinalizeField::TargetName,
            // Local mode
            local_target_name: TextInput::new("New Wallet Name").with_placeholder("wallet_v2"),
            local_new_threshold: TextInput::new("New Threshold (optional)")
                .with_placeholder("same"),
            local_new_n_parties: TextInput::new("New Parties (optional)").with_placeholder("same"),
            local_field: ReshareLocalField::SourceWallet,
            error_message: None,
        }
    }
}

/// Render reshare wizard
pub fn render_reshare(frame: &mut Frame, app: &App, form: &ReshareFormData, area: Rect) {
    match &app.state {
        crate::tui::state::AppState::Reshare(state) => match state {
            ReshareState::ModeSelect => render_mode_select(frame, form, area),
            ReshareState::LocalSetup => render_local_setup(frame, app, form, area),
            ReshareState::LocalComplete { wallet_name } => {
                render_local_complete(frame, wallet_name, area)
            }
            ReshareState::Round1Setup => render_round1_setup(frame, app, form, area),
            ReshareState::Round1Output { output_json } => {
                render_round1_output(frame, output_json, area)
            }
            ReshareState::FinalizeInput => render_finalize_input(frame, form, area),
            ReshareState::Complete { wallet_name } => render_complete(frame, wallet_name, area),
        },
        _ => {}
    }
}

fn render_mode_select(frame: &mut Frame, form: &ReshareFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Reshare Wizard - Select Mode ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Mode options
            Constraint::Length(2), // Help
        ])
        .split(inner);

    let header = Paragraph::new(vec![Line::from(vec![Span::styled(
        "Select reshare mode:",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )])]);
    frame.render_widget(header, chunks[0]);

    // Mode options
    let modes = ReshareMode::all();
    let mut mode_lines = vec![];
    for (i, mode) in modes.iter().enumerate() {
        let is_selected = i == form.mode_selected_index;
        let prefix = if is_selected { "▶ " } else { "  " };
        let style = if is_selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        mode_lines.push(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(mode.label(), style),
        ]));
        mode_lines.push(Line::from(vec![
            Span::raw("     "),
            Span::styled(mode.description(), Style::default().fg(Color::DarkGray)),
        ]));
        mode_lines.push(Line::from(""));
    }

    let mode_list = Paragraph::new(mode_lines)
        .block(Block::default().borders(Borders::ALL).title("Reshare Mode"));
    frame.render_widget(mode_list, chunks[1]);

    let help = Paragraph::new("↑/↓: Select | Enter: Continue | Esc: Cancel")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[2]);
}

fn render_local_setup(frame: &mut Frame, app: &App, form: &ReshareFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Local Reshare - Refresh All Shares ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Source wallet selector
            Constraint::Length(3), // Target name
            Constraint::Length(3), // New threshold
            Constraint::Length(3), // New n_parties
            Constraint::Min(3),    // Info
            Constraint::Length(2), // Error
            Constraint::Length(2), // Help
        ])
        .split(inner);

    // Source wallet selector
    let wallet_focused = form.local_field == ReshareLocalField::SourceWallet;
    let wallet_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if wallet_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::Gray)
        })
        .title("Source Wallet");

    let wallet_name = app
        .wallets
        .get(form.source_wallet_index)
        .map(|w| {
            let t = w.threshold.unwrap_or(0);
            let n = w.total_parties.unwrap_or(0);
            format!("{}  ({}-of-{})", w.name, t, n)
        })
        .unwrap_or_else(|| "(no wallets)".to_string());

    let wallet_para =
        Paragraph::new(format!("  {}  (↑/↓ to change)", wallet_name)).block(wallet_block);
    frame.render_widget(wallet_para, chunks[0]);

    // Target name
    form.local_target_name.render(
        frame,
        chunks[1],
        form.local_field == ReshareLocalField::TargetName,
    );

    // New threshold (optional)
    form.local_new_threshold.render(
        frame,
        chunks[2],
        form.local_field == ReshareLocalField::NewThreshold,
    );

    // New n_parties (optional)
    form.local_new_n_parties.render(
        frame,
        chunks[3],
        form.local_field == ReshareLocalField::NewNParties,
    );

    // Info about local reshare
    let info = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Local reshare will:",
            Style::default().fg(Color::Gray),
        )]),
        Line::from(vec![Span::styled(
            "  • Use existing local party shares",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(vec![Span::styled(
            "  • Generate ALL new shares at once",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(vec![Span::styled(
            "  • Create party1/, party2/, ... folders",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(vec![Span::styled(
            "  • Invalidate old shares",
            Style::default().fg(Color::DarkGray),
        )]),
    ]);
    frame.render_widget(info, chunks[4]);

    // Error
    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, chunks[5]);
    }

    // Help
    let help = Paragraph::new("Tab: Next | ↑/↓: Select wallet | Enter: Reshare | Esc: Back")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[6]);
}

fn render_local_complete(frame: &mut Frame, wallet_name: &str, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title(" Local Reshare - Complete! ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(inner);

    let success = Paragraph::new(Line::from(vec![
        Span::styled("✓ ", Style::default().fg(Color::Green)),
        Span::styled(
            "Local reshare complete!",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    frame.render_widget(success, chunks[0]);

    let info = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("New Wallet: ", Style::default().fg(Color::Gray)),
            Span::styled(
                wallet_name,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from("All new shares created in party1/, party2/, ... folders."),
        Line::from(""),
        Line::from(vec![Span::styled(
            "⚠️  Old shares are now INVALIDATED!",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from("The public key and address remain the SAME."),
    ]);
    frame.render_widget(info, chunks[1]);

    let help = Paragraph::new("Enter/Esc: Return to wallet list")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[2]);
}

fn render_round1_setup(frame: &mut Frame, app: &App, form: &ReshareFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Reshare Wizard - Round 1: Setup ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Source wallet selector
            Constraint::Length(3), // New threshold
            Constraint::Length(3), // New n_parties
            Constraint::Min(1),    // Spacer
            Constraint::Length(2), // Error
            Constraint::Length(2), // Help
        ])
        .split(inner);

    // Source wallet selector
    let wallet_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if form.focused_field == ReshareFormField::SourceWallet {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::Gray)
        })
        .title("Source Wallet");

    let wallet_name = app
        .wallets
        .get(form.source_wallet_index)
        .map(|w| w.name.as_str())
        .unwrap_or("(no wallets)");

    let wallet_para =
        Paragraph::new(format!("  {}  (↑/↓ to change)", wallet_name)).block(wallet_block);
    frame.render_widget(wallet_para, chunks[0]);

    // New threshold
    form.new_threshold.render(
        frame,
        chunks[1],
        form.focused_field == ReshareFormField::NewThreshold,
    );

    // New n_parties
    form.new_n_parties.render(
        frame,
        chunks[2],
        form.focused_field == ReshareFormField::NewNParties,
    );

    // Error
    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, chunks[4]);
    }

    // Help
    let help = Paragraph::new("Tab: Next | ↑/↓: Select wallet | Enter: Generate | Esc: Cancel")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[5]);
}

fn render_round1_output(frame: &mut Frame, output_json: &str, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Reshare Wizard - Round 1: Your Sub-shares ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(inner);

    let instructions =
        Paragraph::new("Share this with NEW parties:").style(Style::default().fg(Color::Yellow));
    frame.render_widget(instructions, chunks[0]);

    let output_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title("Output (copy this)");
    let output_para = Paragraph::new(output_json)
        .block(output_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(output_para, chunks[1]);

    let help =
        Paragraph::new("c: Copy | Enter: Go to Finalize (if new party) | Esc: Done (if old party)")
            .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[2]);
}

fn render_finalize_input(frame: &mut Frame, form: &ReshareFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Reshare Wizard - Finalize: New Party ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Target name
            Constraint::Length(3), // My new index
            Constraint::Length(3), // My rank
            Constraint::Length(3), // Hierarchical
            Constraint::Length(2), // Instructions
            Constraint::Min(5),    // Input area
            Constraint::Length(2), // Error
            Constraint::Length(2), // Help
        ])
        .split(inner);

    form.target_name.render(
        frame,
        chunks[0],
        form.finalize_field == ReshareFinalizeField::TargetName,
    );
    form.my_new_index.render(
        frame,
        chunks[1],
        form.finalize_field == ReshareFinalizeField::MyIndex,
    );
    form.my_rank.render(
        frame,
        chunks[2],
        form.finalize_field == ReshareFinalizeField::MyRank,
    );

    // Hierarchical toggle
    let hier_focused = form.finalize_field == ReshareFinalizeField::Hierarchical;
    let checkbox = if form.hierarchical { "[x]" } else { "[ ]" };
    let checkbox_style = if hier_focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let hier_line = Line::from(vec![
        Span::styled(checkbox, checkbox_style),
        Span::raw(" Enable HTSS"),
    ]);
    let hier_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if hier_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::Gray)
        })
        .title("Mode");
    let hier_para = Paragraph::new(hier_line).block(hier_block);
    frame.render_widget(hier_para, chunks[3]);

    let instructions = Paragraph::new("Paste Round 1 outputs from old parties:")
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(instructions, chunks[4]);

    form.finalize_input.render(
        frame,
        chunks[5],
        form.finalize_field == ReshareFinalizeField::DataInput,
    );

    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, chunks[6]);
    }

    let help = Paragraph::new("Tab: Next | Space: Toggle | Enter: Finalize | Esc: Back")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[7]);
}

fn render_complete(frame: &mut Frame, wallet_name: &str, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title(" Reshare Wizard - Complete! ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(inner);

    let success = Paragraph::new(Line::from(vec![
        Span::styled("✓ ", Style::default().fg(Color::Green)),
        Span::styled(
            "Resharing complete!",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    frame.render_widget(success, chunks[0]);

    let info = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("New Wallet: ", Style::default().fg(Color::Gray)),
            Span::styled(
                wallet_name,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from("The public key and address are the SAME as before."),
        Line::from("Funds are still accessible with the new shares."),
    ]);
    frame.render_widget(info, chunks[1]);

    let help = Paragraph::new("Enter/Esc: Return to wallet list")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[2]);
}
