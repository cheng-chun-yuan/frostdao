//! Send wizard screens (demo-send flow)
//!
//! This implements a multi-party threshold signing demonstration flow.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::tui::app::App;
use crate::tui::components::{TextArea, TextInput};
use crate::tui::state::{SendFormField, SendState};

/// Send wizard form data
#[derive(Clone)]
#[allow(dead_code)]
pub struct SendFormData {
    pub wallet_index: usize,
    pub to_address: TextInput,
    pub amount: TextInput,
    pub focused_field: SendFormField,
    pub session_id: String,
    pub sighash: String,
    pub nonce_output: String,
    pub nonces_input: TextArea,
    pub share_output: String,
    pub shares_input: TextArea,
    pub final_signature: String,
    pub error_message: Option<String>,
    // Party selection
    pub my_party_index: u32,
    pub total_parties: u32,
    pub threshold: u32,
    pub selected_parties: Vec<bool>, // Which parties are selected for signing
    pub party_selector_index: usize, // Currently focused party in selector
}

impl Default for SendFormData {
    fn default() -> Self {
        Self::new()
    }
}

impl SendFormData {
    pub fn new() -> Self {
        Self {
            wallet_index: 0,
            to_address: TextInput::new("To Address").with_placeholder("tb1q..."),
            amount: TextInput::new("Amount (sats)").with_value("1000").numeric(),
            focused_field: SendFormField::ToAddress,
            session_id: String::new(),
            sighash: String::new(),
            nonce_output: String::new(),
            nonces_input: TextArea::new("Paste nonces from other parties"),
            share_output: String::new(),
            shares_input: TextArea::new("Paste signature shares from other parties"),
            final_signature: String::new(),
            error_message: None,
            my_party_index: 1,
            total_parties: 3,
            threshold: 2,
            selected_parties: vec![true, false, false], // Default: only self selected
            party_selector_index: 0,
        }
    }

    #[allow(dead_code)]
    pub fn generate_session_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        format!("demo_{}", timestamp)
    }

    /// Get party label (A, B, C, ...)
    pub fn party_label(index: u32) -> String {
        format!("Party {}", (b'A' + (index - 1) as u8) as char)
    }

    /// Count selected parties
    pub fn selected_count(&self) -> usize {
        self.selected_parties.iter().filter(|&&x| x).count()
    }

    /// Get list of selected party indices (1-based)
    pub fn get_selected_indices(&self) -> Vec<u32> {
        self.selected_parties
            .iter()
            .enumerate()
            .filter(|(_, &selected)| selected)
            .map(|(i, _)| i as u32 + 1)
            .collect()
    }
}

/// Render send wizard
pub fn render_send(frame: &mut Frame, app: &App, form: &SendFormData, area: Rect) {
    match &app.state {
        crate::tui::state::AppState::Send(state) => match state {
            SendState::SelectWallet => render_select_wallet(frame, app, form, area),
            SendState::SelectSigners { .. } => render_select_signers(frame, form, area),
            SendState::EnterDetails { .. } => render_enter_details(frame, form, area),
            SendState::ShowSighash { sighash, .. } => render_show_sighash(frame, sighash, area),
            SendState::GenerateNonce { nonce_output, .. } => {
                render_generate_nonce(frame, nonce_output, area)
            }
            SendState::EnterNonces { .. } => render_enter_nonces(frame, form, area),
            SendState::GenerateShare { share_output, .. } => {
                render_generate_share(frame, share_output, area)
            }
            SendState::CombineShares { .. } => render_combine_shares(frame, form, area),
            SendState::Complete { txid } => render_complete(frame, txid, area),
        },
        _ => {}
    }
}

fn render_select_wallet(frame: &mut Frame, app: &App, form: &SendFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Send - Step 1: Select Wallet ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Instructions
            Constraint::Length(5), // Wallet info
            Constraint::Min(5),    // Threshold info
            Constraint::Length(2), // Error
            Constraint::Length(2), // Help
        ])
        .split(inner);

    let instructions =
        Paragraph::new("Select a wallet to sign from:").style(Style::default().fg(Color::Yellow));
    frame.render_widget(instructions, chunks[0]);

    // Wallet selector
    let wallet_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title("Wallets");

    let (wallet_display, threshold_info) = app
        .wallets
        .get(form.wallet_index)
        .map(|w| {
            let threshold = w.threshold.unwrap_or(0);
            let total = w.total_parties.unwrap_or(0);
            let wallet_str = format!("▶ {} ({}-of-{})", w.name, threshold, total);

            // Generate party labels (A, B, C, ...)
            let party_labels: Vec<String> = (0..total)
                .map(|i| format!("Party {}", (b'A' + i as u8) as char))
                .collect();

            let info_lines = vec![
                Line::from(""),
                Line::from(vec![Span::styled(
                    "📋 Threshold Signing Requirement:",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![
                    Span::raw("   You need "),
                    Span::styled(
                        format!("{}", threshold),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!(
                        " out of {} signers to create a valid signature.",
                        total
                    )),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("👥 Available Signers: ", Style::default().fg(Color::Gray)),
                    Span::styled(party_labels.join(", "), Style::default().fg(Color::White)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("💡 Example: ", Style::default().fg(Color::Gray)),
                    Span::raw(format!(
                        "Ask {} to participate",
                        party_labels
                            .iter()
                            .take(threshold as usize)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(" + ")
                    )),
                ]),
            ];
            (wallet_str, info_lines)
        })
        .unwrap_or_else(|| {
            (
                "(no wallets)".to_string(),
                vec![Line::from("Create a wallet first with 'g' (keygen)")],
            )
        });

    let wallet_para = Paragraph::new(wallet_display).block(wallet_block);
    frame.render_widget(wallet_para, chunks[1]);

    let threshold_para = Paragraph::new(threshold_info);
    frame.render_widget(threshold_para, chunks[2]);

    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, chunks[3]);
    }

    let help = Paragraph::new("↑/↓: Select wallet | Enter: Continue | Esc: Cancel")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[4]);
}

fn render_select_signers(frame: &mut Frame, form: &SendFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Send - Step 2: Select Signing Parties ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Header info
            Constraint::Min(8),    // Party list
            Constraint::Length(3), // Selection status
            Constraint::Length(2), // Error
            Constraint::Length(2), // Help
        ])
        .split(inner);

    // Header with threshold info
    let header = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "📋 Select which parties will participate in signing:",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("   You are: ", Style::default().fg(Color::Gray)),
            Span::styled(
                SendFormData::party_label(form.my_party_index),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" (index {})", form.my_party_index),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ]);
    frame.render_widget(header, chunks[0]);

    // Party list with checkboxes
    let mut party_lines = vec![];
    for i in 0..form.total_parties {
        let party_idx = i as u32 + 1;
        let is_selected = form
            .selected_parties
            .get(i as usize)
            .copied()
            .unwrap_or(false);
        let is_focused = form.party_selector_index == i as usize;
        let is_me = party_idx == form.my_party_index;

        let checkbox = if is_selected { "[✓]" } else { "[ ]" };
        let arrow = if is_focused { "▶ " } else { "  " };

        let style = if is_focused {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else if is_selected {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::White)
        };

        let me_indicator = if is_me { " (You)" } else { "" };

        party_lines.push(Line::from(vec![
            Span::styled(arrow, style),
            Span::styled(checkbox, style),
            Span::styled(format!(" {}", SendFormData::party_label(party_idx)), style),
            Span::styled(me_indicator, Style::default().fg(Color::Cyan)),
        ]));
    }

    let party_list = Paragraph::new(party_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Signing Parties"),
    );
    frame.render_widget(party_list, chunks[1]);

    // Selection status
    let selected_count = form.selected_count();
    let threshold = form.threshold;
    let status_color = if selected_count >= threshold as usize {
        Color::Green
    } else {
        Color::Red
    };

    let selected_names: Vec<String> = form
        .get_selected_indices()
        .iter()
        .map(|&idx| SendFormData::party_label(idx))
        .collect();

    let status = Paragraph::new(vec![Line::from(vec![
        Span::styled("Selected: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{}/{}", selected_count, threshold),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                " required ({} selected: {})",
                if selected_count >= threshold as usize {
                    "✓"
                } else {
                    "need more"
                },
                if selected_names.is_empty() {
                    "none".to_string()
                } else {
                    selected_names.join(", ")
                }
            ),
            Style::default().fg(Color::Gray),
        ),
    ])]);
    frame.render_widget(status, chunks[2]);

    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, chunks[3]);
    }

    let help = Paragraph::new("↑/↓: Navigate | Space: Toggle | Enter: Continue | Esc: Back")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[4]);
}

fn render_enter_details(frame: &mut Frame, form: &SendFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Send - Step 3: Transaction Details ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // To address
            Constraint::Length(3), // Amount
            Constraint::Min(1),    // Spacer
            Constraint::Length(2), // Error
            Constraint::Length(2), // Help
        ])
        .split(inner);

    form.to_address.render(
        frame,
        chunks[0],
        form.focused_field == SendFormField::ToAddress,
    );
    form.amount.render(
        frame,
        chunks[1],
        form.focused_field == SendFormField::Amount,
    );

    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, chunks[3]);
    }

    let help = Paragraph::new("Tab: Next field | Enter: Prepare TX | Esc: Back")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[4]);
}

fn render_show_sighash(frame: &mut Frame, sighash: &str, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Send - Step 4: Sighash (Message to Sign) ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Instructions
            Constraint::Min(5),    // Sighash display
            Constraint::Length(2), // Help
        ])
        .split(inner);

    let instructions = Paragraph::new(vec![
        Line::from("This is the sighash (message) that all parties will sign."),
        Line::from("Share this with all signing parties."),
    ])
    .style(Style::default().fg(Color::Yellow));
    frame.render_widget(instructions, chunks[0]);

    let sighash_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title("Sighash (copy this)");
    let sighash_para = Paragraph::new(sighash)
        .block(sighash_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(sighash_para, chunks[1]);

    let help = Paragraph::new("c: Copy | Enter: Generate Nonce | Esc: Back")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[2]);
}

fn render_generate_nonce(frame: &mut Frame, nonce_output: &str, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Send - Step 5: Your Nonce ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Instructions
            Constraint::Min(5),    // Nonce display
            Constraint::Length(2), // Help
        ])
        .split(inner);

    let instructions = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "📤 Share this nonce with other signers:",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("   1. ", Style::default().fg(Color::Cyan)),
            Span::raw("Copy the JSON below"),
        ]),
        Line::from(vec![
            Span::styled("   2. ", Style::default().fg(Color::Cyan)),
            Span::raw("Send to other signing parties (Party B, C, ...)"),
        ]),
        Line::from(vec![
            Span::styled("   3. ", Style::default().fg(Color::Cyan)),
            Span::raw("Ask them to run the same flow and share their nonces back"),
        ]),
    ]);
    frame.render_widget(instructions, chunks[0]);

    let nonce_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title("Your Nonce JSON (copy & share with other signers)");
    let nonce_para = Paragraph::new(nonce_output)
        .block(nonce_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(nonce_para, chunks[1]);

    let help = Paragraph::new("c: Copy | Enter: Collect nonces from others | Esc: Back")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[2]);
}

fn render_enter_nonces(frame: &mut Frame, form: &SendFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Send - Step 6: Collect Nonces ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Instructions
            Constraint::Min(5),    // Nonces input
            Constraint::Length(3), // Status + Error
            Constraint::Length(2), // Help
        ])
        .split(inner);

    // Count nonces in input
    let nonces_content = form.nonces_input.content();
    let nonce_count = nonces_content.matches("\"party_index\"").count();
    let threshold = form.threshold as usize;
    let has_enough = nonce_count >= threshold;

    let instructions = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "📥 Collect nonces from all signing parties:",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("   Format: ", Style::default().fg(Color::Gray)),
            Span::raw("Paste JSON nonces, space or newline separated"),
        ]),
        Line::from(vec![
            Span::styled("   Example: ", Style::default().fg(Color::Gray)),
            Span::raw("{\"party_index\":1,...} {\"party_index\":2,...}"),
        ]),
    ]);
    frame.render_widget(instructions, chunks[0]);

    form.nonces_input.render(frame, chunks[1], true);

    // Status line with count
    let status_color = if has_enough { Color::Green } else { Color::Red };
    let status_icon = if has_enough { "✓" } else { "⚠" };

    let mut status_lines = vec![Line::from(vec![
        Span::styled(
            format!("{} ", status_icon),
            Style::default().fg(status_color),
        ),
        Span::styled("Nonces collected: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{}/{}", nonce_count, threshold),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(if has_enough {
            " - Ready to sign!"
        } else {
            " - Need more nonces from other signers"
        }),
    ])];

    if let Some(error) = &form.error_message {
        status_lines.push(Line::from(vec![
            Span::styled("Error: ", Style::default().fg(Color::Red)),
            Span::styled(error.as_str(), Style::default().fg(Color::Red)),
        ]));
    }

    let status = Paragraph::new(status_lines);
    frame.render_widget(status, chunks[2]);

    let help = Paragraph::new("Enter: Generate Signature Share | Esc: Back")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[3]);
}

fn render_generate_share(frame: &mut Frame, share_output: &str, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Send - Step 7: Your Signature Share ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Instructions
            Constraint::Min(5),    // Share display
            Constraint::Length(2), // Help
        ])
        .split(inner);

    let instructions = Paragraph::new("Share your signature share with the aggregator:")
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(instructions, chunks[0]);

    let share_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title("Your Signature Share (copy this)");
    let share_para = Paragraph::new(share_output)
        .block(share_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(share_para, chunks[1]);

    let help = Paragraph::new("c: Copy | Enter: Combine (Aggregator) | Esc: Done")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[2]);
}

fn render_combine_shares(frame: &mut Frame, form: &SendFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Send - Step 8: Combine Signatures (Aggregator) ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Instructions
            Constraint::Min(5),    // Shares input
            Constraint::Length(3), // Status + Error
            Constraint::Length(2), // Help
        ])
        .split(inner);

    let instructions = Paragraph::new("Paste all signature shares to combine:")
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(instructions, chunks[0]);

    form.shares_input.render(frame, chunks[1], true);

    // Share count status
    let shares_content = form.shares_input.content();
    let share_count = shares_content.matches("\"party_index\"").count();
    let threshold = form.threshold as usize;
    let has_enough = share_count >= threshold;

    let status_color = if has_enough { Color::Green } else { Color::Red };
    let status_icon = if has_enough { "✓" } else { "⚠" };

    let mut status_lines = vec![Line::from(vec![
        Span::styled(
            format!("{} ", status_icon),
            Style::default().fg(status_color),
        ),
        Span::styled("Shares collected: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{}/{}", share_count, threshold),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(if has_enough {
            " - Ready to combine!"
        } else {
            " - Need more shares"
        }),
    ])];

    if let Some(error) = &form.error_message {
        status_lines.push(Line::from(vec![
            Span::styled("Error: ", Style::default().fg(Color::Red)),
            Span::styled(error.as_str(), Style::default().fg(Color::Red)),
        ]));
    }

    let status = Paragraph::new(status_lines);
    frame.render_widget(status, chunks[2]);

    let help = Paragraph::new("Enter: Combine & Complete | Esc: Back")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[3]);
}

fn render_complete(frame: &mut Frame, txid: &str, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title(" Send - Complete! ");

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
            "Threshold signature complete!",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    frame.render_widget(success, chunks[0]);

    let info_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title("Result");
    let info = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "Signature/TXID: ",
            Style::default().fg(Color::Gray),
        )]),
        Line::from(vec![Span::styled(
            txid,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("All parties contributed their shares to create this signature."),
        Line::from("In a real transaction, this would be broadcast to the network."),
    ])
    .block(info_block)
    .wrap(Wrap { trim: false });
    frame.render_widget(info, chunks[1]);

    let help = Paragraph::new("Enter/Esc: Return to wallet list")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[2]);
}
