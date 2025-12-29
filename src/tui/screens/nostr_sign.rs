//! Nostr signing screen - Propose/Consent/Execute flow
//!
//! Flow:
//! - Proposer: ConfigureTx → Propose → WaitingForConsent → Execute
//! - Consenter: ViewProposals → Review → Consent → WaitingForExecution

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};

use crate::tui::app::App;
use crate::tui::state::NostrSignState;

/// Render the Nostr signing screen
pub fn render_nostr_sign(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(3), // Progress bar
            Constraint::Length(7), // Status/Info
            Constraint::Min(8),    // Content
            Constraint::Length(3), // Help
        ])
        .margin(1)
        .split(area);

    // Title
    let phase = match &app.nostr_sign_state {
        NostrSignState::SelectWallet => "Select Wallet",
        NostrSignState::SelectRole { .. } => "Select Role",
        NostrSignState::ConfigureTx { .. } => "Configure",
        NostrSignState::WaitingForConsent { .. } => "Waiting for Consent",
        NostrSignState::ViewProposals { .. } => "View Proposals",
        NostrSignState::ReviewProposal { .. } => "Review Proposal",
        NostrSignState::WaitingForExecution { .. } => "Waiting",
        NostrSignState::CollectingShares { .. } => "Collecting Shares",
        NostrSignState::Combining => "Combining",
        NostrSignState::Complete { .. } => "Complete",
    };

    let title = Paragraph::new(Line::from(vec![
        Span::styled("📝 ", Style::default()),
        Span::styled(
            format!("Nostr Transaction - {}", phase),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    // Progress bar
    let (progress, label) = get_progress(&app.nostr_sign_state, app);
    let gauge = Gauge::default()
        .block(Block::default())
        .gauge_style(Style::default().fg(Color::Magenta).bg(Color::DarkGray))
        .percent(progress)
        .label(label);
    frame.render_widget(gauge, chunks[1]);

    // Status/Info box
    render_status_info(frame, app, chunks[2]);

    // Main content area
    render_content(frame, app, chunks[3]);

    // Help
    render_help(frame, app, chunks[4]);
}

fn get_progress(state: &NostrSignState, app: &App) -> (u16, String) {
    match state {
        NostrSignState::SelectWallet => (0, "Select a wallet...".to_string()),
        NostrSignState::SelectRole { .. } => (5, "Choose: Propose or Consent".to_string()),
        NostrSignState::ConfigureTx { .. } => (10, "Configure transaction...".to_string()),
        NostrSignState::WaitingForConsent { consents, .. } => {
            let count = consents.len() + 1; // +1 for proposer
            let total = app.nostr_threshold as usize;
            let pct = 20 + (count * 40 / total.max(1)) as u16;
            (pct, format!("Consents: {}/{} parties", count, total))
        }
        NostrSignState::ViewProposals { .. } => (10, "Viewing proposals...".to_string()),
        NostrSignState::ReviewProposal { .. } => (15, "Review proposal details".to_string()),
        NostrSignState::WaitingForExecution { .. } => (60, "Waiting for execution...".to_string()),
        NostrSignState::CollectingShares {
            received_shares, ..
        } => {
            let count = received_shares.len();
            let total = app.nostr_threshold as usize;
            let pct = 60 + (count * 30 / total.max(1)) as u16;
            (pct, format!("Shares: {}/{} received", count, total))
        }
        NostrSignState::Combining => (95, "Combining signatures...".to_string()),
        NostrSignState::Complete { txid } => (100, format!("✓ Broadcast: {}...", &txid[..8])),
    }
}

fn render_status_info(frame: &mut Frame, app: &App, area: Rect) {
    let lines = match &app.nostr_sign_state {
        NostrSignState::SelectRole { wallet_name } => {
            vec![
                Line::from(vec![
                    Span::styled("Wallet: ", Style::default().fg(Color::Gray)),
                    Span::styled(wallet_name, Style::default().fg(Color::White)),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "📤 Propose: Create and propose a new transaction",
                    Style::default().fg(Color::Cyan),
                )),
                Line::from(Span::styled(
                    "📥 Consent: Review and consent to pending proposals",
                    Style::default().fg(Color::Yellow),
                )),
            ]
        }
        NostrSignState::WaitingForConsent {
            wallet_name,
            session_id,
            proposal,
            consents,
        } => {
            let consent_count = consents.len() + 1;
            let threshold = app.nostr_threshold as usize;
            vec![
                Line::from(vec![
                    Span::styled("Wallet: ", Style::default().fg(Color::Gray)),
                    Span::styled(wallet_name, Style::default().fg(Color::White)),
                    Span::raw("  "),
                    Span::styled("Session: ", Style::default().fg(Color::Gray)),
                    Span::styled(&session_id[..8], Style::default().fg(Color::Cyan)),
                ]),
                Line::from(vec![
                    Span::styled("To: ", Style::default().fg(Color::Gray)),
                    Span::styled(&proposal.to_address, Style::default().fg(Color::Yellow)),
                ]),
                Line::from(vec![
                    Span::styled("Amount: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("{} sats", proposal.amount_sats),
                        Style::default().fg(Color::Green),
                    ),
                ]),
                Line::from(vec![Span::styled(
                    format!("Waiting for consents ({}/{})...", consent_count, threshold),
                    Style::default().fg(Color::Yellow),
                )]),
            ]
        }
        NostrSignState::ReviewProposal { proposal, .. } => {
            vec![
                Line::from(vec![
                    Span::styled("Proposer: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("Party {}", proposal.proposer_index),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::raw("  "),
                    Span::styled("Session: ", Style::default().fg(Color::Gray)),
                    Span::styled(&proposal.session_id[..8], Style::default().fg(Color::Cyan)),
                ]),
                Line::from(vec![
                    Span::styled("To: ", Style::default().fg(Color::Gray)),
                    Span::styled(&proposal.to_address, Style::default().fg(Color::Yellow)),
                ]),
                Line::from(vec![
                    Span::styled("Amount: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("{} sats", proposal.amount_sats),
                        Style::default().fg(Color::Green),
                    ),
                    Span::raw("  "),
                    Span::styled("Fee: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("{} sat/vB", proposal.fee_rate),
                        Style::default().fg(Color::White),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Sighash: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("{}...", &proposal.sighash[..32.min(proposal.sighash.len())]),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Desc: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        if proposal.description.is_empty() {
                            "No description"
                        } else {
                            &proposal.description
                        },
                        Style::default().fg(Color::White),
                    ),
                    Span::raw("  "),
                    Span::styled("Time: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format_timestamp(proposal.timestamp),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
            ]
        }
        NostrSignState::CollectingShares {
            wallet_name,
            session_id,
            ..
        } => {
            vec![
                Line::from(vec![
                    Span::styled("Wallet: ", Style::default().fg(Color::Gray)),
                    Span::styled(wallet_name, Style::default().fg(Color::White)),
                    Span::raw("  "),
                    Span::styled("Session: ", Style::default().fg(Color::Gray)),
                    Span::styled(&session_id[..8], Style::default().fg(Color::Cyan)),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Threshold reached! Collecting signature shares...",
                    Style::default().fg(Color::Green),
                )),
            ]
        }
        NostrSignState::Complete { txid } => {
            vec![
                Line::from(Span::styled(
                    "✓ Transaction broadcast successfully!",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("TXID: ", Style::default().fg(Color::Gray)),
                    Span::styled(txid, Style::default().fg(Color::Cyan)),
                ]),
            ]
        }
        _ => {
            vec![Line::from(Span::styled(
                "Select a wallet to start...",
                Style::default().fg(Color::DarkGray),
            ))]
        }
    };

    let status = Paragraph::new(lines);
    frame.render_widget(status, area);
}

fn render_content(frame: &mut Frame, app: &App, area: Rect) {
    match &app.nostr_sign_state {
        NostrSignState::SelectRole { .. } => {
            render_role_selection(frame, app, area);
        }
        NostrSignState::WaitingForConsent { consents, .. } => {
            render_consent_list(frame, app, consents, area);
        }
        NostrSignState::ViewProposals { .. } => {
            render_proposals_list(frame, app, area);
        }
        NostrSignState::CollectingShares {
            received_shares, ..
        } => {
            render_shares_list(frame, app, received_shares, area);
        }
        _ => {
            // Empty or default content
            let placeholder = Paragraph::new("").block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            );
            frame.render_widget(placeholder, area);
        }
    }
}

fn render_role_selection(frame: &mut Frame, _app: &App, area: Rect) {
    let items = vec![
        ListItem::new(Line::from(vec![
            Span::styled("▶ ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "📤 Propose Transaction",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ])),
        ListItem::new(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "   Create a new transaction for others to consent",
                Style::default().fg(Color::DarkGray),
            ),
        ])),
        ListItem::new(Line::from("")),
        ListItem::new(Line::from(vec![
            Span::raw("  "),
            Span::styled("📥 Consent to Proposal", Style::default().fg(Color::Yellow)),
        ])),
        ListItem::new(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "   Review and approve pending transactions",
                Style::default().fg(Color::DarkGray),
            ),
        ])),
    ];

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Select Action ")
            .border_style(Style::default().fg(Color::Magenta)),
    );

    frame.render_widget(list, area);
}

fn render_consent_list(
    frame: &mut Frame,
    app: &App,
    consents: &std::collections::HashMap<u32, String>,
    area: Rect,
) {
    let threshold = app.nostr_threshold;

    let items: Vec<ListItem> = (1..=app.nostr_n_parties)
        .map(|idx| {
            let is_me = idx == app.nostr_my_index;
            let is_proposer = is_me; // In this flow, we're the proposer
            let has_consent = is_proposer || consents.contains_key(&idx);

            let status = if has_consent {
                ("✓ Consented", Color::Green)
            } else {
                ("○ Pending", Color::DarkGray)
            };

            let name_style = if is_me {
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let role = if is_proposer { " (Proposer)" } else { "" };

            ListItem::new(Line::from(vec![
                Span::styled(format!("Party {}{}", idx, role), name_style),
                Span::raw("  "),
                Span::styled(status.0, Style::default().fg(status.1)),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(
                " Consents ({}/{} required) ",
                consents.len() + 1,
                threshold
            ))
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(list, area);
}

fn render_proposals_list(frame: &mut Frame, _app: &App, area: Rect) {
    // Placeholder - would show list of pending proposals from Nostr
    let items = vec![
        ListItem::new(Line::from(Span::styled(
            "No pending proposals",
            Style::default().fg(Color::DarkGray),
        ))),
        ListItem::new(Line::from("")),
        ListItem::new(Line::from(Span::styled(
            "Proposals will appear here when broadcast by other parties",
            Style::default().fg(Color::DarkGray),
        ))),
    ];

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Pending Proposals ")
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(list, area);
}

fn render_shares_list(
    frame: &mut Frame,
    app: &App,
    received_shares: &std::collections::HashMap<u32, String>,
    area: Rect,
) {
    let threshold = app.nostr_threshold;

    let items: Vec<ListItem> = (1..=threshold)
        .map(|idx| {
            let is_me = idx == app.nostr_my_index;
            let has_share = received_shares.contains_key(&idx);

            let status = if has_share {
                ("✓", Color::Green)
            } else if is_me {
                ("●", Color::Yellow)
            } else {
                ("○", Color::DarkGray)
            };

            let name_style = if is_me {
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let me_indicator = if is_me { " (me)" } else { "" };

            ListItem::new(Line::from(vec![
                Span::styled(format!("Signer {}{}", idx, me_indicator), name_style),
                Span::raw("  "),
                Span::styled("Share: ", Style::default().fg(Color::Gray)),
                Span::styled(status.0, Style::default().fg(status.1)),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Signature Shares ")
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(list, area);
}

fn render_help(frame: &mut Frame, app: &App, area: Rect) {
    let help_text = match &app.nostr_sign_state {
        NostrSignState::SelectRole { .. } => "↑/↓: Select | Enter: Continue | Esc: Back",
        NostrSignState::ReviewProposal { .. } => "Enter: Consent | R: Reject | Esc: Back",
        NostrSignState::Complete { .. } => "Enter: Done | C: Copy TXID",
        _ => "Enter: Continue | Esc: Cancel",
    };

    let help = Paragraph::new(Line::from(vec![Span::styled(
        help_text,
        Style::default().fg(Color::DarkGray),
    )]))
    .block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(help, area);
}

/// Format unix timestamp as relative time or short date
fn format_timestamp(timestamp: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if timestamp == 0 {
        return "unknown".to_string();
    }

    let diff = now.saturating_sub(timestamp);
    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}
