//! Reshare wizard screens

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::tui::app::{wallet_address_for_network, App};
use crate::tui::components::{TextArea, TextInput};
use crate::tui::state::{
    ReshareFinalizeField, ReshareFormField, ReshareLocalField, ReshareMode, ReshareState,
};
use crate::tui::COPY_KEY_LABEL;

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
    if let crate::tui::state::AppState::Reshare(state) = &app.state {
        match state {
            ReshareState::ModeSelect => render_mode_select(frame, form, area),
            ReshareState::LocalSetup => render_local_setup(frame, app, form, area),
            ReshareState::LocalComplete { wallet_name } => {
                render_local_complete(frame, app, form, wallet_name, area)
            }
            ReshareState::Round1Setup => render_round1_setup(frame, app, form, area),
            ReshareState::Round1Output { output_json } => {
                render_round1_output(frame, output_json, area)
            }
            ReshareState::FinalizeInput => render_finalize_input(frame, form, area),
            ReshareState::Complete { wallet_name } => {
                render_complete(frame, app, form, wallet_name, area)
            }
        }
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

    let help = Paragraph::new("j/k/↑/↓: Select | Enter: Continue | Esc: Cancel")
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
            Constraint::Min(6),    // Info
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
        Paragraph::new(format!("  {}  (j/k/↑/↓ to change)", wallet_name)).block(wallet_block);
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
    let mut info_lines = vec![
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
    ];
    info_lines.extend(reshare_local_boundary_lines());
    let info = Paragraph::new(info_lines);
    frame.render_widget(info, chunks[4]);

    // Error
    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, chunks[5]);
    }

    // Help
    let help = Paragraph::new("Tab: Next | j/k/↑/↓: Select wallet | Enter: Reshare | Esc: Back")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[6]);
}

fn render_local_complete(
    frame: &mut Frame,
    app: &App,
    form: &ReshareFormData,
    wallet_name: &str,
    area: Rect,
) {
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

    let mut info_lines = vec![
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
            "WARNING: old shares are now INVALIDATED!",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from("Local shares should never be pasted from this step."),
        Line::from("Only the wallet's public key and address remain unchanged."),
    ];
    info_lines.extend(reshare_address_stability_lines());
    info_lines.extend(reshare_address_verification_lines(app, form, wallet_name));
    let info = Paragraph::new(info_lines);
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
            Constraint::Min(6),    // Context
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
        Paragraph::new(format!("  {}  (j/k/↑/↓ to change)", wallet_name)).block(wallet_block);
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

    let setup_context = Paragraph::new(reshare_distributed_boundary_lines())
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(setup_context, chunks[3]);

    // Error
    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, chunks[4]);
    }

    // Help
    let help = Paragraph::new("Tab: Next | j/k/↑/↓: Select wallet | Enter: Generate | Esc: Cancel")
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
            Constraint::Min(4),
            Constraint::Min(2),
            Constraint::Length(2),
        ])
        .split(inner);

    let instructions = Paragraph::new("Share this with NEW parties only.")
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(instructions, chunks[0]);

    let output_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title("Output (copy this)");
    let output_para = Paragraph::new(output_json)
        .block(output_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(output_para, chunks[1]);

    let boundary = Paragraph::new(reshare_distributed_output_boundary_lines());
    frame.render_widget(boundary, chunks[2]);

    let help = Paragraph::new(format!(
        "{COPY_KEY_LABEL}: Copy | Enter: Go to Finalize (if new party) | Esc: Done (if old party)",
    ))
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[3]);
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
            Constraint::Length(2), // Input context
            Constraint::Min(4),    // Input area
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

    let finalize_context =
        Paragraph::new("Only paste JSON for this ceremony's source wallet and your target wallet.")
            .style(Style::default().fg(Color::Gray));
    frame.render_widget(finalize_context, chunks[5]);

    form.finalize_input.render(
        frame,
        chunks[6],
        form.finalize_field == ReshareFinalizeField::DataInput,
    );

    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, chunks[7]);
    }

    let help =
        Paragraph::new("Tab: Next | Space: Toggle | Ctrl+u: Clear | Enter: Finalize | Esc: Back")
            .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[8]);
}

fn render_complete(
    frame: &mut Frame,
    app: &App,
    form: &ReshareFormData,
    wallet_name: &str,
    area: Rect,
) {
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

    let mut distributed_info_lines = vec![
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
    ];
    distributed_info_lines.extend(reshare_address_stability_lines());
    distributed_info_lines.extend(reshare_address_verification_lines(app, form, wallet_name));
    let info = Paragraph::new(distributed_info_lines);
    frame.render_widget(info, chunks[1]);

    let help = Paragraph::new("Enter/Esc: Return to wallet list")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[2]);
}

fn reshare_local_boundary_lines() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from("Boundary: old share material stays on this device."),
        Line::from("All shares are regenerated once and replaced as a unit."),
        Line::from("Address and group public key do not change in this step."),
    ]
}

fn reshare_distributed_boundary_lines() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from("Only share Round 1 sub-shares for this new wallet."),
        Line::from("Payloads are party-bound and not full secret material."),
        Line::from("The ceremony must preserve source wallet identity."),
    ]
}

fn reshare_distributed_output_boundary_lines() -> Vec<Line<'static>> {
    vec![
        Line::from("Boundary: copy only"),
        Line::from("- This output is for a single recipient party."),
        Line::from("- Verify recipient identity before transfer."),
    ]
}

fn reshare_address_stability_lines() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from(
            "Address continuity: public key and root address should match source wallet after finalization.",
        ),
    ]
}

fn reshare_address_verification_lines(
    app: &App,
    form: &ReshareFormData,
    target_wallet_name: &str,
) -> Vec<Line<'static>> {
    let source_wallet = app.wallets.get(form.source_wallet_index);
    let source_wallet_name = source_wallet
        .map(|wallet| wallet.name.as_str())
        .unwrap_or("(missing source wallet)");
    let target_wallet = app
        .wallets
        .iter()
        .find(|wallet| wallet.name == target_wallet_name);

    let source_address =
        source_wallet.and_then(|wallet| wallet_address_for_network(wallet, app.network));
    let target_address =
        target_wallet.and_then(|wallet| wallet_address_for_network(wallet, app.network));

    match (source_address, target_address) {
        (Some(source), Some(target)) if source == target => vec![
            Line::from(vec![
                Span::styled("Address check: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("matched on {}", app.network.display_name()),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Source/target: ", Style::default().fg(Color::Gray)),
                Span::styled(target.to_string(), Style::default().fg(Color::Green)),
            ]),
        ],
        (Some(source), Some(target)) => vec![
            Line::from(vec![
                Span::styled("Address check: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    "MISMATCH - stop and inspect wallets",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Source: ", Style::default().fg(Color::Gray)),
                Span::styled(source.to_string(), Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::styled("Target: ", Style::default().fg(Color::Gray)),
                Span::styled(target.to_string(), Style::default().fg(Color::Yellow)),
            ]),
        ],
        _ => vec![
            Line::from(vec![
                Span::styled("Address check: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!(
                        "source or target address unavailable on {}",
                        app.network.display_name()
                    ),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                Span::styled("Compare manually: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{source_wallet_name} -> {target_wallet_name}"),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::NetworkSelection;
    use frostdao::protocol::keygen::WalletSummary;
    use std::collections::BTreeMap;

    fn line_to_string(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<Vec<_>>()
            .join("")
    }

    fn lines_to_string(lines: &[Line<'_>]) -> String {
        lines
            .iter()
            .map(line_to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn wallet_summary(name: &str, testnet_address: Option<&str>) -> WalletSummary {
        WalletSummary {
            name: name.to_string(),
            threshold: Some(2),
            total_parties: Some(3),
            hierarchical: Some(false),
            address: testnet_address.map(str::to_string),
            address_testnet: testnet_address.map(str::to_string),
            address_mainnet: None,
            address_regtest: None,
            signing_requirement: None,
            party_ranks: Some(BTreeMap::new()),
        }
    }

    fn app_with_reshare_wallets(source_address: Option<&str>, target_address: Option<&str>) -> App {
        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Testnet4;
        app.wallets = vec![
            wallet_summary("source", source_address),
            wallet_summary("target", target_address),
        ];
        app
    }

    #[test]
    fn reshare_local_boundary_is_secret_local() {
        let rendered = lines_to_string(&reshare_local_boundary_lines());
        assert!(rendered.contains("old share material stays on this device"));
        assert!(rendered.contains("Address and group public key do not change in this step"));
    }

    #[test]
    fn reshare_distributed_boundary_guides_party_bound_shares() {
        let rendered = lines_to_string(&reshare_distributed_boundary_lines());
        assert!(rendered.contains("Only share Round 1 sub-shares for this new wallet"));
        assert!(rendered.contains("Payloads are party-bound and not full secret material"));
        assert!(rendered.contains("preserve source wallet identity"));
    }

    #[test]
    fn reshare_output_boundary_reminds_single_recipient_transfer() {
        let rendered = lines_to_string(&reshare_distributed_output_boundary_lines());
        assert!(rendered.contains("copy only"));
        assert!(rendered.contains("single recipient party"));
        assert!(rendered.contains("Verify recipient identity"));
    }

    #[test]
    fn reshare_address_stability_is_explicit() {
        let rendered = lines_to_string(&reshare_address_stability_lines());
        assert!(rendered.contains("Address continuity"));
        assert!(rendered.contains("public key and root address should match source wallet"));
    }

    #[test]
    fn reshare_address_verification_reports_match() {
        let app = app_with_reshare_wallets(Some("tb1pmatch"), Some("tb1pmatch"));
        let mut form = ReshareFormData::new();
        form.source_wallet_index = 0;

        let rendered = lines_to_string(&reshare_address_verification_lines(&app, &form, "target"));

        assert!(rendered.contains("Address check"));
        assert!(rendered.contains("matched on Testnet4"));
        assert!(rendered.contains("tb1pmatch"));
    }

    #[test]
    fn reshare_address_verification_reports_mismatch() {
        let app = app_with_reshare_wallets(Some("tb1psource"), Some("tb1ptarget"));
        let mut form = ReshareFormData::new();
        form.source_wallet_index = 0;

        let rendered = lines_to_string(&reshare_address_verification_lines(&app, &form, "target"));

        assert!(rendered.contains("MISMATCH"));
        assert!(rendered.contains("tb1psource"));
        assert!(rendered.contains("tb1ptarget"));
    }

    #[test]
    fn reshare_address_verification_handles_missing_address() {
        let app = app_with_reshare_wallets(Some("tb1psource"), None);
        let mut form = ReshareFormData::new();
        form.source_wallet_index = 0;

        let rendered = lines_to_string(&reshare_address_verification_lines(&app, &form, "target"));

        assert!(rendered.contains("unavailable on Testnet4"));
        assert!(rendered.contains("Compare manually: source -> target"));
        assert!(rendered.contains("address unavailable"));
    }
}
