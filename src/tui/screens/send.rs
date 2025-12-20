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
    pub txid: String,
    pub error_message: Option<String>,
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
            txid: String::new(),
            error_message: None,
        }
    }

    pub fn generate_session_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        format!("demo_{}", timestamp)
    }
}

/// Render send wizard
pub fn render_send(frame: &mut Frame, app: &App, form: &SendFormData, area: Rect) {
    match &app.state {
        crate::tui::state::AppState::Send(state) => match state {
            SendState::SelectWallet => render_select_wallet(frame, app, form, area),
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
        .title(" Demo Send - Step 1: Select Wallet ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Instructions
            Constraint::Min(5),    // Wallet list
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

    let wallet_name = app
        .wallets
        .get(form.wallet_index)
        .map(|w| {
            let threshold = w.threshold.unwrap_or(0);
            let total = w.total_parties.unwrap_or(0);
            format!("→ {} ({}-of-{})", w.name, threshold, total)
        })
        .unwrap_or_else(|| "(no wallets)".to_string());

    let wallet_para = Paragraph::new(wallet_name).block(wallet_block);
    frame.render_widget(wallet_para, chunks[1]);

    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, chunks[2]);
    }

    let help = Paragraph::new("↑/↓: Select wallet | Enter: Continue | Esc: Cancel")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[3]);
}

fn render_enter_details(frame: &mut Frame, form: &SendFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Demo Send - Step 2: Transaction Details ");

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
        .title(" Demo Send - Step 3: Sighash (Message to Sign) ");

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
        .title(" Demo Send - Step 4: Your Nonce ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Instructions
            Constraint::Min(5),    // Nonce display
            Constraint::Length(2), // Help
        ])
        .split(inner);

    let instructions = Paragraph::new("Share this nonce with other signing parties:")
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(instructions, chunks[0]);

    let nonce_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title("Your Nonce (copy this)");
    let nonce_para = Paragraph::new(nonce_output)
        .block(nonce_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(nonce_para, chunks[1]);

    let help = Paragraph::new("c: Copy | Enter: Enter Other Nonces | Esc: Back")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[2]);
}

fn render_enter_nonces(frame: &mut Frame, form: &SendFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Demo Send - Step 5: Collect Nonces ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Instructions
            Constraint::Min(5),    // Nonces input
            Constraint::Length(2), // Error
            Constraint::Length(2), // Help
        ])
        .split(inner);

    let instructions = Paragraph::new("Paste nonces from all signing parties (including yours):")
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(instructions, chunks[0]);

    form.nonces_input.render(frame, chunks[1], true);

    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, chunks[2]);
    }

    let help = Paragraph::new("Enter: Generate Signature Share | Esc: Back")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[3]);
}

fn render_generate_share(frame: &mut Frame, share_output: &str, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Demo Send - Step 6: Your Signature Share ");

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

    let help =
        Paragraph::new("c: Copy | Enter: Combine Shares (Aggregator) | Esc: Done (Non-Aggregator)")
            .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[2]);
}

fn render_combine_shares(frame: &mut Frame, form: &SendFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Demo Send - Step 7: Combine Signatures (Aggregator) ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Instructions
            Constraint::Min(5),    // Shares input
            Constraint::Length(2), // Error
            Constraint::Length(2), // Help
        ])
        .split(inner);

    let instructions = Paragraph::new("Paste all signature shares to combine:")
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(instructions, chunks[0]);

    form.shares_input.render(frame, chunks[1], true);

    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, chunks[2]);
    }

    let help = Paragraph::new("Enter: Combine & Complete | Esc: Back")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[3]);
}

fn render_complete(frame: &mut Frame, txid: &str, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title(" Demo Send - Complete! ");

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
