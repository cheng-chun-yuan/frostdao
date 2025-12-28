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

/// Script type for Taproot spending conditions
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ScriptType {
    /// Standard key path spending (no script)
    #[default]
    None,
    /// Absolute timelock (CLTV) - block height
    TimelockAbsolute,
    /// Relative timelock (CSV) - blocks after confirmation
    TimelockRelative,
    /// Recovery script - fallback after timeout
    Recovery,
    /// Hash Time-Locked Contract
    HTLC,
}

impl ScriptType {
    pub fn all() -> &'static [ScriptType] {
        &[
            ScriptType::None,
            ScriptType::TimelockAbsolute,
            ScriptType::TimelockRelative,
            ScriptType::Recovery,
            ScriptType::HTLC,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            ScriptType::None => "Standard (Key Path)",
            ScriptType::TimelockAbsolute => "Timelock (Absolute)",
            ScriptType::TimelockRelative => "Timelock (Relative)",
            ScriptType::Recovery => "Recovery Script",
            ScriptType::HTLC => "HTLC (Hash Lock)",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ScriptType::None => "Normal threshold signature spending",
            ScriptType::TimelockAbsolute => "Cannot spend until block height X",
            ScriptType::TimelockRelative => "Cannot spend until N blocks after confirmation",
            ScriptType::Recovery => "Fallback: recovery key can spend after timeout",
            ScriptType::HTLC => "Requires hash preimage OR timeout for refund",
        }
    }

    /// Convert to the btc taproot_scripts module type
    pub fn to_script_type_input(&self) -> frostdao::btc::taproot_scripts::ScriptTypeInput {
        use frostdao::btc::taproot_scripts::ScriptTypeInput;
        match self {
            ScriptType::None => ScriptTypeInput::None,
            ScriptType::TimelockAbsolute => ScriptTypeInput::TimelockAbsolute,
            ScriptType::TimelockRelative => ScriptTypeInput::TimelockRelative,
            ScriptType::Recovery => ScriptTypeInput::Recovery,
            ScriptType::HTLC => ScriptTypeInput::Htlc,
        }
    }
}

/// Script configuration for advanced spending conditions
#[derive(Clone)]
pub struct ScriptConfig {
    /// Selected script type
    pub script_type: ScriptType,
    /// Absolute timelock: block height
    pub timelock_height: TextInput,
    /// Relative timelock: number of blocks
    pub timelock_blocks: TextInput,
    /// Recovery: timeout in blocks
    pub recovery_timeout: TextInput,
    /// Recovery: pubkey (x-only hex)
    pub recovery_pubkey: TextInput,
    /// HTLC: hash (SHA256 hex)
    pub htlc_hash: TextInput,
    /// HTLC: timeout in blocks
    pub htlc_timeout: TextInput,
    /// HTLC: refund pubkey (x-only hex)
    pub htlc_refund_pubkey: TextInput,
    /// Currently selected script type index
    pub selected_index: usize,
    /// Currently focused field in config
    pub focused_field: usize,
}

impl Default for ScriptConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptConfig {
    pub fn new() -> Self {
        Self {
            script_type: ScriptType::None,
            timelock_height: TextInput::new("Block Height")
                .with_placeholder("850000")
                .numeric(),
            timelock_blocks: TextInput::new("Blocks").with_placeholder("144").numeric(),
            recovery_timeout: TextInput::new("Timeout (blocks)")
                .with_placeholder("4320")
                .numeric(),
            recovery_pubkey: TextInput::new("Recovery Pubkey")
                .with_placeholder("x-only hex (64 chars)"),
            htlc_hash: TextInput::new("Hash (SHA256)").with_placeholder("64 char hex"),
            htlc_timeout: TextInput::new("Timeout (blocks)")
                .with_placeholder("144")
                .numeric(),
            htlc_refund_pubkey: TextInput::new("Refund Pubkey")
                .with_placeholder("x-only hex (64 chars)"),
            selected_index: 0,
            focused_field: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Convert to ScriptParams for use with taproot_scripts module
    pub fn to_script_params(&self) -> anyhow::Result<frostdao::btc::taproot_scripts::ScriptParams> {
        use frostdao::btc::taproot_scripts::ScriptParams;

        // Determine timeout based on script type
        let timeout = match self.script_type {
            ScriptType::Recovery => self.recovery_timeout.value(),
            ScriptType::HTLC => self.htlc_timeout.value(),
            _ => "",
        };

        ScriptParams::from_strings(
            self.script_type.to_script_type_input(),
            self.timelock_height.value(),
            self.timelock_blocks.value(),
            timeout,
            self.recovery_pubkey.value(),
            self.htlc_hash.value(),
            self.htlc_refund_pubkey.value(),
        )
    }

    /// Check if this is a standard key-path spend (no scripts)
    pub fn is_key_path_only(&self) -> bool {
        self.script_type == ScriptType::None
    }
}

/// UTXO display info
#[derive(Clone, Debug)]
pub struct UtxoDisplay {
    pub txid: String,
    pub vout: u32,
    pub value: u64,
    pub confirmed: bool,
}

/// Transaction display info
#[derive(Clone, Debug)]
pub struct TxDisplay {
    pub txid: String,
    pub amount: i64, // Positive = received, negative = sent
    pub confirmed: bool,
    pub time: Option<u64>,
}

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
    // HD address selection
    pub hd_enabled: bool,
    pub hd_addresses: Vec<(String, String, u32)>, // (address, pubkey_hex, index)
    pub hd_selected_index: usize,                 // Currently selected HD address
    pub use_hd_address: bool,                     // Whether to use HD derived address
    // UTXO and transaction info
    pub utxos: Vec<UtxoDisplay>,
    pub recent_txs: Vec<TxDisplay>,
    pub total_balance: u64,
    // Fee estimation
    pub fee_rate: u64,       // sats/vbyte
    pub estimated_fee: u64,  // estimated fee for current amount
    pub utxos_needed: usize, // how many UTXOs needed
    // Script options (timelock, recovery, HTLC)
    pub script_config: ScriptConfig,
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
            // HD address selection defaults
            hd_enabled: false,
            hd_addresses: Vec::new(),
            hd_selected_index: 0,
            use_hd_address: false,
            // UTXO and transaction info defaults
            utxos: Vec::new(),
            recent_txs: Vec::new(),
            total_balance: 0,
            // Fee estimation defaults
            fee_rate: 1, // 1 sat/vbyte default
            estimated_fee: 0,
            utxos_needed: 0,
            script_config: ScriptConfig::new(),
        }
    }

    /// Estimate fee for the current amount using coin selection
    pub fn estimate_fee(&mut self) {
        let amount: u64 = self.amount.value().parse().unwrap_or(0);
        if amount == 0 {
            self.estimated_fee = 0;
            self.utxos_needed = 0;
            return;
        }

        // Get confirmed UTXOs sorted by value (largest first for fewer inputs)
        let mut confirmed: Vec<&UtxoDisplay> = self.utxos.iter().filter(|u| u.confirmed).collect();
        confirmed.sort_by(|a, b| b.value.cmp(&a.value));

        // Coin selection: select minimum UTXOs needed
        let mut selected_value: u64 = 0;
        let mut num_inputs: usize = 0;

        for utxo in confirmed {
            if selected_value >= amount {
                break;
            }
            selected_value += utxo.value;
            num_inputs += 1;
        }

        if num_inputs == 0 {
            self.estimated_fee = 0;
            self.utxos_needed = 0;
            return;
        }

        // Estimate vsize: 10 (overhead) + 58 per input + 43 per output (2 outputs: recipient + change)
        let estimated_vsize = 10 + (num_inputs as u64 * 58) + (2 * 43);
        self.estimated_fee = estimated_vsize * self.fee_rate;
        self.utxos_needed = num_inputs;
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

    /// Get the selected derivation path (if HD mode is enabled)
    pub fn get_derivation_path(&self) -> Option<(u32, u32)> {
        if self.use_hd_address && self.hd_enabled {
            self.hd_addresses
                .get(self.hd_selected_index)
                .map(|(_, _, idx)| (0u32, *idx)) // (change=0 for receive, address_index)
        } else {
            None
        }
    }

    /// Get selected HD address string
    #[allow(dead_code)]
    pub fn get_selected_hd_address(&self) -> Option<String> {
        if self.use_hd_address && self.hd_enabled {
            self.hd_addresses
                .get(self.hd_selected_index)
                .map(|(addr, _, _)| addr.clone())
        } else {
            None
        }
    }
}

/// Render send wizard
pub fn render_send(frame: &mut Frame, app: &App, form: &SendFormData, area: Rect) {
    match &app.state {
        crate::tui::state::AppState::Send(state) => match state {
            SendState::SelectWallet => render_select_wallet(frame, app, form, area),
            SendState::SelectSigners { .. } => render_select_signers(frame, form, area),
            SendState::SelectAddress { .. } => render_select_address(frame, form, area),
            SendState::ConfigureScript { .. } => render_configure_script(frame, form, area),
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

    // Selection status - must be exactly threshold
    let selected_count = form.selected_count();
    let threshold = form.threshold;
    let status_color = if selected_count == threshold as usize {
        Color::Green
    } else {
        Color::Yellow
    };

    let selected_names: Vec<String> = form
        .get_selected_indices()
        .iter()
        .map(|&idx| SendFormData::party_label(idx))
        .collect();

    let status_msg = if selected_count == threshold as usize {
        "✓ ready".to_string()
    } else if selected_count < threshold as usize {
        format!("need {}", threshold as usize - selected_count)
    } else {
        format!("too many, deselect {}", selected_count - threshold as usize)
    };

    let status = Paragraph::new(vec![Line::from(vec![
        Span::styled("Selected: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{}/{}", selected_count, threshold),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" exactly required ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!(
                "({}: {})",
                status_msg,
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

fn render_select_address(frame: &mut Frame, form: &SendFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Send - Step 3: Select Address (HD Derivation) ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Header info
            Constraint::Min(8),    // Address list
            Constraint::Length(3), // Selection info
            Constraint::Length(2), // Error
            Constraint::Length(2), // Help
        ])
        .split(inner);

    // Header with HD info
    let header = if form.hd_enabled {
        Paragraph::new(vec![
            Line::from(vec![Span::styled(
                "📍 Select an HD-derived address to send from:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("   BIP-44 Path: ", Style::default().fg(Color::Gray)),
                Span::styled("m/44'/0'/0'/0/", Style::default().fg(Color::Cyan)),
                Span::styled("<index>", Style::default().fg(Color::Yellow)),
            ]),
        ])
    } else {
        Paragraph::new(vec![
            Line::from(vec![Span::styled(
                "⚠️  HD derivation not available for this wallet",
                Style::default().fg(Color::Red),
            )]),
            Line::from(""),
            Line::from("   This wallet was created without HD support."),
            Line::from("   Will use root address for signing."),
        ])
    };
    frame.render_widget(header, chunks[0]);

    // Address list
    if form.hd_enabled && !form.hd_addresses.is_empty() {
        let mut addr_lines = vec![];

        // Option to use root address
        let root_focused = !form.use_hd_address;
        let root_prefix = if root_focused { "▶ " } else { "  " };
        let root_style = if root_focused {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        addr_lines.push(Line::from(vec![
            Span::styled(root_prefix, root_style),
            Span::styled("[Root Address] ", root_style),
            Span::styled("(no derivation)", Style::default().fg(Color::DarkGray)),
        ]));
        addr_lines.push(Line::from(""));

        // HD derived addresses
        for (i, (addr, _, idx)) in form.hd_addresses.iter().enumerate() {
            let is_selected = form.use_hd_address && i == form.hd_selected_index;
            let prefix = if is_selected { "▶ " } else { "  " };

            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let path_str = format!("0/{}", idx);
            let short_addr = if addr.len() > 20 {
                format!("{}...{}", &addr[..10], &addr[addr.len() - 8..])
            } else {
                addr.clone()
            };

            addr_lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("[{}] ", path_str), Style::default().fg(Color::Cyan)),
                Span::styled(short_addr, style),
            ]));
        }

        let addr_list = Paragraph::new(addr_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Available Addresses"),
        );
        frame.render_widget(addr_list, chunks[1]);
    } else {
        let no_hd = Paragraph::new("No HD addresses available")
            .style(Style::default().fg(Color::DarkGray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Available Addresses"),
            );
        frame.render_widget(no_hd, chunks[1]);
    }

    // Selection info
    let selection_info = if form.use_hd_address && form.hd_enabled {
        if let Some((_addr, _, idx)) = form.hd_addresses.get(form.hd_selected_index) {
            Paragraph::new(vec![Line::from(vec![
                Span::styled("Selected: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("HD Address at path 0/{}", idx),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ])])
        } else {
            Paragraph::new("")
        }
    } else {
        Paragraph::new(vec![Line::from(vec![
            Span::styled("Selected: ", Style::default().fg(Color::Gray)),
            Span::styled(
                "Root Address (no HD tweak)",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ])])
    };
    frame.render_widget(selection_info, chunks[2]);

    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, chunks[3]);
    }

    let help = Paragraph::new("↑/↓: Navigate | Enter: Continue | Esc: Back")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[4]);
}

fn render_configure_script(frame: &mut Frame, form: &SendFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Send - Step 4: Script Options (Optional) ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Length(12), // Script type selector
            Constraint::Min(8),     // Config fields
            Constraint::Length(2),  // Error
            Constraint::Length(2),  // Help
        ])
        .split(inner);

    // Header
    let header = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "🔒 Configure spending conditions (optional):",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            "   These add script paths to your Taproot output",
            Style::default().fg(Color::DarkGray),
        )]),
    ]);
    frame.render_widget(header, chunks[0]);

    // Script type selector
    let script_types = ScriptType::all();
    let mut type_lines = vec![];

    for (i, script_type) in script_types.iter().enumerate() {
        let is_selected = i == form.script_config.selected_index;
        let prefix = if is_selected { "▶ " } else { "  " };
        let checkbox = if form.script_config.script_type == *script_type {
            "[●]"
        } else {
            "[ ]"
        };

        let style = if is_selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        type_lines.push(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(checkbox, style),
            Span::styled(format!(" {}", script_type.label()), style),
        ]));
        type_lines.push(Line::from(vec![
            Span::raw("       "),
            Span::styled(
                script_type.description(),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    let type_list = Paragraph::new(type_lines)
        .block(Block::default().borders(Borders::ALL).title("Script Type"));
    frame.render_widget(type_list, chunks[1]);

    // Config fields based on selected type
    let config_content = match &form.script_config.script_type {
        ScriptType::None => vec![
            Line::from(vec![Span::styled(
                "No additional configuration needed.",
                Style::default().fg(Color::DarkGray),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Standard key path spending will be used.",
                Style::default().fg(Color::Gray),
            )]),
        ],
        ScriptType::TimelockAbsolute => {
            let focused = form.script_config.focused_field == 0;
            vec![
                Line::from(vec![
                    Span::styled(
                        if focused { "▶ " } else { "  " },
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled("Block Height: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        form.script_config.timelock_height.value(),
                        if focused {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "   Transaction cannot be spent until this block height.",
                    Style::default().fg(Color::DarkGray),
                )]),
                Line::from(vec![Span::styled(
                    "   Current testnet height: ~2,800,000",
                    Style::default().fg(Color::DarkGray),
                )]),
            ]
        }
        ScriptType::TimelockRelative => {
            let focused = form.script_config.focused_field == 0;
            vec![
                Line::from(vec![
                    Span::styled(
                        if focused { "▶ " } else { "  " },
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled("Blocks: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        form.script_config.timelock_blocks.value(),
                        if focused {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "   Cannot spend until N blocks after confirmation.",
                    Style::default().fg(Color::DarkGray),
                )]),
                Line::from(vec![Span::styled(
                    "   144 blocks ≈ 1 day, 1008 ≈ 1 week",
                    Style::default().fg(Color::DarkGray),
                )]),
            ]
        }
        ScriptType::Recovery => {
            let timeout_focused = form.script_config.focused_field == 0;
            let pubkey_focused = form.script_config.focused_field == 1;
            vec![
                Line::from(vec![
                    Span::styled(
                        if timeout_focused { "▶ " } else { "  " },
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled("Timeout (blocks): ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        form.script_config.recovery_timeout.value(),
                        if timeout_focused {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                ]),
                Line::from(vec![
                    Span::styled(
                        if pubkey_focused { "▶ " } else { "  " },
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled("Recovery Pubkey: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        if form.script_config.recovery_pubkey.value().is_empty() {
                            "(enter x-only pubkey)".to_string()
                        } else {
                            let v = form.script_config.recovery_pubkey.value();
                            if v.len() > 20 {
                                format!("{}...", &v[..20])
                            } else {
                                v.to_string()
                            }
                        },
                        if pubkey_focused {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "   After timeout, recovery key can spend.",
                    Style::default().fg(Color::DarkGray),
                )]),
            ]
        }
        ScriptType::HTLC => {
            let hash_focused = form.script_config.focused_field == 0;
            let timeout_focused = form.script_config.focused_field == 1;
            let refund_focused = form.script_config.focused_field == 2;
            vec![
                Line::from(vec![
                    Span::styled(
                        if hash_focused { "▶ " } else { "  " },
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled("Hash (SHA256): ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        if form.script_config.htlc_hash.value().is_empty() {
                            "(64 char hex)".to_string()
                        } else {
                            let v = form.script_config.htlc_hash.value();
                            if v.len() > 20 {
                                format!("{}...", &v[..20])
                            } else {
                                v.to_string()
                            }
                        },
                        if hash_focused {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                ]),
                Line::from(vec![
                    Span::styled(
                        if timeout_focused { "▶ " } else { "  " },
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled("Timeout (blocks): ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        form.script_config.htlc_timeout.value(),
                        if timeout_focused {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                ]),
                Line::from(vec![
                    Span::styled(
                        if refund_focused { "▶ " } else { "  " },
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled("Refund Pubkey: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        if form.script_config.htlc_refund_pubkey.value().is_empty() {
                            "(x-only pubkey)".to_string()
                        } else {
                            let v = form.script_config.htlc_refund_pubkey.value();
                            if v.len() > 20 {
                                format!("{}...", &v[..20])
                            } else {
                                v.to_string()
                            }
                        },
                        if refund_focused {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "   Recipient needs preimage; you can refund after timeout.",
                    Style::default().fg(Color::DarkGray),
                )]),
            ]
        }
    };

    let config_widget = Paragraph::new(config_content).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Configuration"),
    );
    frame.render_widget(config_widget, chunks[2]);

    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, chunks[3]);
    }

    let help = Paragraph::new(
        "↑/↓: Select type | Tab: Next field | Space: Toggle | Enter: Continue | Esc: Back",
    )
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[4]);
}

fn render_enter_details(frame: &mut Frame, form: &SendFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Send - Step 4: Transaction Details ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split into left (inputs) and right (UTXOs/TXs)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    // Left side: inputs
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // To address
            Constraint::Length(3), // Amount
            Constraint::Length(3), // Balance info
            Constraint::Min(1),    // Spacer
            Constraint::Length(2), // Error
            Constraint::Length(2), // Help
        ])
        .split(main_chunks[0]);

    form.to_address.render(
        frame,
        left_chunks[0],
        form.focused_field == SendFormField::ToAddress,
    );
    form.amount.render(
        frame,
        left_chunks[1],
        form.focused_field == SendFormField::Amount,
    );

    // Balance and fee info
    let balance_btc = form.total_balance as f64 / 100_000_000.0;
    let confirmed_count = form.utxos.iter().filter(|u| u.confirmed).count();
    let confirmed_balance: u64 = form
        .utxos
        .iter()
        .filter(|u| u.confirmed)
        .map(|u| u.value)
        .sum();

    let amount: u64 = form.amount.value().parse().unwrap_or(0);
    let total_needed = amount + form.estimated_fee;

    let mut balance_lines = vec![Line::from(vec![
        Span::styled("Balance: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{} sats", confirmed_balance),
            Style::default().fg(Color::Green),
        ),
        Span::styled(
            format!(" ({:.8} BTC)", balance_btc),
            Style::default().fg(Color::DarkGray),
        ),
    ])];

    if form.estimated_fee > 0 {
        balance_lines.push(Line::from(vec![
            Span::styled("Est. fee: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{} sats", form.estimated_fee),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                format!(" ({} UTXOs, {} sat/vB)", form.utxos_needed, form.fee_rate),
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        // Show total and remaining
        let remaining = confirmed_balance.saturating_sub(total_needed);
        let fee_warning = form.estimated_fee > amount / 2; // Warn if fee > 50% of amount

        balance_lines.push(Line::from(vec![
            Span::styled("Total: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{} sats", total_needed),
                Style::default().fg(if total_needed > confirmed_balance {
                    Color::Red
                } else {
                    Color::White
                }),
            ),
            Span::styled(" → Remaining: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{} sats", remaining),
                Style::default().fg(if remaining == 0 {
                    Color::Red
                } else {
                    Color::Green
                }),
            ),
        ]));

        if fee_warning && amount > 0 {
            balance_lines.push(Line::from(Span::styled(
                "⚠ High fee ratio! Consider sending more or waiting for lower fees",
                Style::default().fg(Color::Yellow),
            )));
        }
    } else {
        balance_lines.push(Line::from(vec![Span::styled(
            format!("{} confirmed UTXOs", confirmed_count),
            Style::default().fg(Color::DarkGray),
        )]));
    }

    let balance_para = Paragraph::new(balance_lines);
    frame.render_widget(balance_para, left_chunks[2]);

    if let Some(error) = &form.error_message {
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
        frame.render_widget(error_para, left_chunks[4]);
    }

    let help = Paragraph::new("Tab: Next field | Enter: Prepare TX | Esc: Back")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, left_chunks[5]);

    // Right side: UTXOs and recent transactions
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_chunks[1]);

    // UTXOs panel
    render_utxos_panel(frame, form, right_chunks[0]);

    // Recent transactions panel
    render_recent_txs_panel(frame, form, right_chunks[1]);
}

fn render_utxos_panel(frame: &mut Frame, form: &SendFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" UTXOs ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    if form.utxos.is_empty() {
        lines.push(Line::from(Span::styled(
            "No UTXOs found",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for utxo in form.utxos.iter().take(5) {
            let status = if utxo.confirmed { "✓" } else { "⏳" };
            let status_color = if utxo.confirmed {
                Color::Green
            } else {
                Color::Yellow
            };
            lines.push(Line::from(vec![
                Span::styled(status, Style::default().fg(status_color)),
                Span::styled(
                    format!(" {} sats ", utxo.value),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("{}:{}", &utxo.txid[..8], utxo.vout),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
        if form.utxos.len() > 5 {
            lines.push(Line::from(Span::styled(
                format!("... and {} more", form.utxos.len() - 5),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, inner);
}

fn render_recent_txs_panel(frame: &mut Frame, form: &SendFormData, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .title(" Recent Transactions ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    if form.recent_txs.is_empty() {
        lines.push(Line::from(Span::styled(
            "No transactions found",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for tx in form.recent_txs.iter().take(5) {
            let status = if tx.confirmed { "✓" } else { "⏳" };
            let status_color = if tx.confirmed {
                Color::Green
            } else {
                Color::Yellow
            };
            let (amount_str, amount_color) = if tx.amount >= 0 {
                (format!("+{}", tx.amount), Color::Green)
            } else {
                (format!("{}", tx.amount), Color::Red)
            };
            // Format time if available
            let time_str = tx.time.map(|t| {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let age_secs = now.saturating_sub(t);
                if age_secs < 3600 {
                    format!("{}m", age_secs / 60)
                } else if age_secs < 86400 {
                    format!("{}h", age_secs / 3600)
                } else {
                    format!("{}d", age_secs / 86400)
                }
            });
            lines.push(Line::from(vec![
                Span::styled(status, Style::default().fg(status_color)),
                Span::styled(
                    format!(" {} sats ", amount_str),
                    Style::default().fg(amount_color),
                ),
                Span::styled(
                    format!("{}...", &tx.txid[..8]),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    time_str.map(|t| format!(" {}", t)).unwrap_or_default(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
        if form.recent_txs.len() > 5 {
            lines.push(Line::from(Span::styled(
                format!("... and {} more", form.recent_txs.len() - 5),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, inner);
}

fn render_show_sighash(frame: &mut Frame, sighash: &str, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Send - Step 5: Sighash (Message to Sign) ");

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
        .title(" Send - Step 6: Your Nonce ");

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
        .title(" Send - Step 7: Collect Nonces ");

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
        .title(" Send - Step 8: Your Signature Share ");

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
        .title(" Send - Step 9: Combine Signatures (Aggregator) ");

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
        Line::from("Threshold signers contributed their shares to create this signature."),
        Line::from("In a real transaction, this would be broadcast to the network."),
    ])
    .block(info_block)
    .wrap(Wrap { trim: false });
    frame.render_widget(info, chunks[1]);

    let help = Paragraph::new(Line::from(vec![
        Span::styled("c", Style::default().fg(Color::Yellow)),
        Span::raw(": Copy TXID | "),
        Span::styled("Enter/Esc", Style::default().fg(Color::Yellow)),
        Span::raw(": Return to wallet list"),
    ]))
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[2]);
}
