//! Miniscript-backed agent payment draft screen.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::tui::app::App;
use crate::tui::components::{TextArea, TextInput};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PolicyPreviewField {
    #[default]
    AgentLabel,
    AgentPubkey,
    Recipient,
    Amount,
    DailyLimit,
    AgentIndex,
    Policy,
}

impl PolicyPreviewField {
    pub fn next(self) -> Self {
        match self {
            Self::AgentLabel => Self::AgentPubkey,
            Self::AgentPubkey => Self::Recipient,
            Self::Recipient => Self::Amount,
            Self::Amount => Self::DailyLimit,
            Self::DailyLimit => Self::AgentIndex,
            Self::AgentIndex => Self::Policy,
            Self::Policy => Self::AgentLabel,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::AgentLabel => Self::Policy,
            Self::AgentPubkey => Self::AgentLabel,
            Self::Recipient => Self::AgentPubkey,
            Self::Amount => Self::Recipient,
            Self::DailyLimit => Self::Amount,
            Self::AgentIndex => Self::DailyLimit,
            Self::Policy => Self::AgentIndex,
        }
    }
}

#[derive(Clone)]
pub struct PolicyPreviewFormData {
    pub focused_field: PolicyPreviewField,
    pub preset_index: usize,
    pub agent_label: TextInput,
    pub agent_pubkey: TextInput,
    pub recipient: TextInput,
    pub amount_sats: TextInput,
    pub daily_limit_sats: TextInput,
    pub agent_index: TextInput,
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
        let mut policy_input = TextArea::new("Miniscript Policy")
            .with_placeholder("or(pk(AGENT),and(pk(DAO),older(144)))");
        policy_input.set_content("or(pk(AGENT),and(pk(DAO),older(144)))");

        Self {
            focused_field: PolicyPreviewField::AgentLabel,
            preset_index: 0,
            agent_label: TextInput::new("Agent Label").with_placeholder("invoice-bot"),
            agent_pubkey: TextInput::new("Agent Pubkey").with_placeholder("32-byte x-only hex"),
            recipient: TextInput::new("Recipient").with_placeholder("tb1p..."),
            amount_sats: TextInput::new("Amount (sats)")
                .with_placeholder("12000")
                .numeric(),
            daily_limit_sats: TextInput::new("Daily Limit (sats)")
                .with_placeholder("50000")
                .numeric(),
            agent_index: TextInput::new("Agent Index")
                .with_placeholder("0")
                .with_value("0")
                .numeric(),
            policy_input,
            output: String::new(),
            error: None,
        }
    }

    pub fn presets() -> &'static [(&'static str, &'static str)] {
        &[
            (
                "Agent hot path + DAO delayed recovery",
                "or(pk(AGENT),and(pk(DAO),older(144)))",
            ),
            ("Agent requires DAO cosign", "and(pk(AGENT),pk(DAO))"),
            (
                "Agent, DAO, or recovery quorum",
                "thresh(2,pk(AGENT),pk(DAO),pk(RECOVERY))",
            ),
            (
                "DAO immediate + agent after delay",
                "or(pk(DAO),and(pk(AGENT),older(144)))",
            ),
        ]
    }

    pub fn apply_current_preset(&mut self) {
        let presets = Self::presets();
        if let Some((_, policy)) = presets.get(self.preset_index % presets.len()) {
            self.policy_input.set_content(policy);
            self.output.clear();
            self.error = None;
        }
    }

    pub fn next_preset(&mut self) {
        let presets = Self::presets();
        self.preset_index = (self.preset_index + 1) % presets.len();
        self.apply_current_preset();
    }

    pub fn prev_preset(&mut self) {
        let presets = Self::presets();
        self.preset_index = if self.preset_index == 0 {
            presets.len() - 1
        } else {
            self.preset_index - 1
        };
        self.apply_current_preset();
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
            Constraint::Length(6),
            Constraint::Length(14),
            Constraint::Percentage(28),
            Constraint::Percentage(39),
            Constraint::Length(4),
        ])
        .split(area);

    let header = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "Agent Payment Policy",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("Initialize an AI agent payment draft from a Frost-derived address and Taproot Miniscript policy."),
        Line::from("This creates a reviewable draft; threshold signing still controls actual spending."),
        Line::from(format!(
            "Template: {}",
            PolicyPreviewFormData::presets()[form.preset_index].0
        )),
    ])
    .block(Block::default().borders(Borders::ALL).title(" Policy "));
    frame.render_widget(header, chunks[0]);

    render_agent_fields(frame, form, chunks[1]);
    form.policy_input.render(
        frame,
        chunks[2],
        form.focused_field == PolicyPreviewField::Policy,
    );

    let output = if let Some(error) = &form.error {
        vec![Line::from(vec![
            Span::styled("Error: ", Style::default().fg(Color::Red)),
            Span::raw(error),
        ])]
    } else if form.output.trim().is_empty() {
        vec![Line::from(Span::styled(
            "Press Enter to initialize the agent payment draft.",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        form.output.lines().map(Line::from).collect()
    };

    let output_widget = Paragraph::new(output)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Agent Payment Draft "),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(output_widget, chunks[3]);

    let feature_status = if cfg!(feature = "miniscript-policy") {
        "enabled"
    } else {
        "disabled"
    };
    let help = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" Init draft   "),
            Span::styled("[/]", Style::default().fg(Color::Yellow)),
            Span::raw(" Template   "),
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw(" Field   "),
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
    frame.render_widget(help, chunks[4]);
}

fn render_agent_fields(frame: &mut Frame, form: &PolicyPreviewFormData, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .split(area);

    let row1 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);
    form.agent_label.render(
        frame,
        row1[0],
        form.focused_field == PolicyPreviewField::AgentLabel,
    );
    form.agent_index.render(
        frame,
        row1[1],
        form.focused_field == PolicyPreviewField::AgentIndex,
    );

    form.agent_pubkey.render(
        frame,
        rows[1],
        form.focused_field == PolicyPreviewField::AgentPubkey,
    );

    let row2 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[2]);
    form.amount_sats.render(
        frame,
        row2[0],
        form.focused_field == PolicyPreviewField::Amount,
    );
    form.daily_limit_sats.render(
        frame,
        row2[1],
        form.focused_field == PolicyPreviewField::DailyLimit,
    );

    form.recipient.render(
        frame,
        rows[3],
        form.focused_field == PolicyPreviewField::Recipient,
    );

    let hint = Paragraph::new("Agent index maps to Frost HD path m/86'/0'/0'/0/<index>; each agent gets a distinct derived address from the same threshold wallet.")
        .style(Style::default().fg(Color::DarkGray))
        .wrap(Wrap { trim: false });
    frame.render_widget(hint, rows[4]);
}
