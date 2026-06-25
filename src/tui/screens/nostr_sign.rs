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
use std::collections::HashMap;

use crate::tui::app::{wallet_address_for_network, App};
use crate::tui::state::{NostrSignState, NostrTxField};
use crate::tui::COPY_KEY_LABEL;

fn preview_id(value: &str) -> String {
    value.chars().take(8).collect()
}

/// Render the Nostr signing screen
pub fn render_nostr_sign(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(3), // Progress bar
            Constraint::Length(9), // Status/Info
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
        NostrSignState::Combining { .. } => "Combining",
        NostrSignState::AnnounceBroadcast { .. } => "Announce Broadcast",
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
        NostrSignState::WaitingForConsent {
            consents,
            rejections,
            ..
        } => {
            let count = consents.len() + 1; // +1 for proposer
            let total = app.nostr_threshold as usize;
            let pct = 20 + (count * 40 / total.max(1)) as u16;
            (
                pct,
                format!(
                    "Consents: {}/{} | Rejections: {}",
                    count,
                    total,
                    rejections.len()
                ),
            )
        }
        NostrSignState::ViewProposals { .. } => (10, "Viewing proposals...".to_string()),
        NostrSignState::ReviewProposal { .. } => (15, "Review proposal details".to_string()),
        NostrSignState::WaitingForExecution { .. } => (60, "Waiting for execution...".to_string()),
        NostrSignState::CollectingShares {
            session_id,
            received_shares,
            ..
        } => {
            let counts = signing_progress_counts(app, session_id, received_shares);
            let pct = 60 + (counts.share_count * 30 / counts.threshold.max(1)) as u16;
            (
                pct.min(90),
                format!(
                    "Nonces: {}/{} | Shares: {}/{}",
                    counts.nonce_count, counts.threshold, counts.share_count, counts.threshold
                ),
            )
        }
        NostrSignState::Combining { .. } => {
            (95, "Waiting for transaction broadcast...".to_string())
        }
        NostrSignState::AnnounceBroadcast { .. } => {
            (96, "Publishing tx_broadcast announcement...".to_string())
        }
        NostrSignState::Complete { txid } => (100, format!("✓ Broadcast: {}...", preview_id(txid))),
    }
}

fn render_status_info(frame: &mut Frame, app: &App, area: Rect) {
    let status = Paragraph::new(nostr_sign_status_lines(app));
    frame.render_widget(status, area);
}

fn nostr_sign_status_lines<'a>(app: &'a App) -> Vec<Line<'a>> {
    let state_lines = match &app.nostr_sign_state {
        NostrSignState::ConfigureTx { wallet_name } => {
            let (source_path, source_address, control) =
                nostr_configure_source_summary(app, wallet_name);
            vec![
                Line::from(vec![
                    Span::styled("Wallet: ", Style::default().fg(Color::Gray)),
                    Span::styled(wallet_name, Style::default().fg(Color::White)),
                    Span::raw("  "),
                    Span::styled("Network: ", Style::default().fg(Color::Gray)),
                    Span::styled(app.network.display_name(), Style::default().fg(Color::Cyan)),
                ]),
                Line::from(vec![
                    Span::styled("Source path: ", Style::default().fg(Color::Gray)),
                    Span::styled(source_path, Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("Source address: ", Style::default().fg(Color::Gray)),
                    Span::styled(source_address, Style::default().fg(Color::White)),
                ]),
                Line::from(Span::styled(control, Style::default().fg(Color::DarkGray))),
                Line::from(vec![
                    Span::styled("Review: ", Style::default().fg(Color::Gray)),
                    Span::raw("the proposal will publish this source path, source address, and sighash fingerprint."),
                ]),
            ]
        }
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
            rejections,
        } => {
            let consent_count = consents.len() + 1;
            let threshold = app.nostr_threshold as usize;
            let max_possible_consents =
                (app.nostr_n_parties as usize).saturating_sub(rejections.len());
            let status_line = if max_possible_consents < threshold {
                format!(
                    "Blocked: {} rejection(s) leave only {}/{} possible approvals",
                    rejections.len(),
                    max_possible_consents,
                    threshold
                )
            } else {
                format!(
                    "Waiting for consents ({}/{}) with {} rejection(s)...",
                    consent_count,
                    threshold,
                    rejections.len()
                )
            };
            let status_color = if max_possible_consents < threshold {
                Color::Red
            } else {
                Color::Yellow
            };
            vec![
                Line::from(vec![
                    Span::styled("Wallet: ", Style::default().fg(Color::Gray)),
                    Span::styled(wallet_name, Style::default().fg(Color::White)),
                    Span::raw("  "),
                    Span::styled("Session: ", Style::default().fg(Color::Gray)),
                    Span::styled(preview_id(session_id), Style::default().fg(Color::Cyan)),
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
                Line::from(vec![
                    Span::styled("Network: ", Style::default().fg(Color::Gray)),
                    Span::styled(&proposal.review.network, Style::default().fg(Color::Cyan)),
                    Span::raw("  "),
                    Span::styled("Path: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        &proposal.review.source_path,
                        Style::default().fg(Color::White),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Sighash fingerprint: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        &proposal.review.sighash_fingerprint,
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
                Line::from(vec![Span::styled(
                    status_line,
                    Style::default().fg(status_color),
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
                    Span::styled(
                        preview_id(&proposal.session_id),
                        Style::default().fg(Color::Cyan),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Network: ", Style::default().fg(Color::Gray)),
                    Span::styled(&proposal.review.network, Style::default().fg(Color::Cyan)),
                    Span::raw("  "),
                    Span::styled("Source path: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        &proposal.review.source_path,
                        Style::default().fg(Color::White),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("From: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        &proposal.review.from_address,
                        Style::default().fg(Color::White),
                    ),
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
                    Span::styled("Sighash fingerprint: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        &proposal.review.sighash_fingerprint,
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Unsigned tx: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        unsigned_tx_review_summary(&proposal.unsigned_tx),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
            ]
        }
        NostrSignState::CollectingShares {
            wallet_name,
            session_id,
            received_shares,
        } => {
            let counts = signing_progress_counts(app, session_id, received_shares);
            vec![
                Line::from(vec![
                    Span::styled("Wallet: ", Style::default().fg(Color::Gray)),
                    Span::styled(wallet_name, Style::default().fg(Color::White)),
                    Span::raw("  "),
                    Span::styled("Session: ", Style::default().fg(Color::Gray)),
                    Span::styled(preview_id(session_id), Style::default().fg(Color::Cyan)),
                ]),
                Line::from(vec![
                    Span::styled("Nonce threshold: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("{}/{}", counts.nonce_count, counts.threshold),
                        Style::default().fg(if counts.nonce_count >= counts.threshold {
                            Color::Green
                        } else {
                            Color::Yellow
                        }),
                    ),
                    Span::raw("  "),
                    Span::styled("Share threshold: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("{}/{}", counts.share_count, counts.threshold),
                        Style::default().fg(if counts.ready_to_combine {
                            Color::Green
                        } else {
                            Color::Yellow
                        }),
                    ),
                ]),
                Line::from(Span::styled(
                    if counts.ready_to_combine {
                        "Coordinator threshold reached; ready for combine handoff."
                    } else if counts.nonce_count >= counts.threshold {
                        "Nonce threshold reached; collecting signature shares."
                    } else {
                        "Waiting for nonce threshold before accepting signature shares."
                    },
                    Style::default().fg(if counts.ready_to_combine {
                        Color::Green
                    } else {
                        Color::Yellow
                    }),
                )),
            ]
        }
        NostrSignState::Combining {
            wallet_name,
            session_id,
        } => {
            vec![
                Line::from(vec![
                    Span::styled("Wallet: ", Style::default().fg(Color::Gray)),
                    Span::styled(wallet_name, Style::default().fg(Color::White)),
                    Span::raw("  "),
                    Span::styled("Session: ", Style::default().fg(Color::Gray)),
                    Span::styled(preview_id(session_id), Style::default().fg(Color::Cyan)),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Waiting for matching tx_broadcast room announcement...",
                    Style::default().fg(Color::Yellow),
                )),
                Line::from(Span::styled(
                    "Combine handoff: press c to copy `frostdao dkg-broadcast` with this wallet, session, unsigned tx, and network.",
                    Style::default().fg(Color::Gray),
                )),
                Line::from(Span::styled(
                    "Replace <shares-json> with threshold shares; matching tx_broadcast is a room announcement, not an on-chain confirmation.",
                    Style::default().fg(Color::DarkGray),
                )),
            ]
        }
        NostrSignState::AnnounceBroadcast {
            wallet_name,
            session_id,
        } => {
            vec![
                Line::from(vec![
                    Span::styled("Wallet: ", Style::default().fg(Color::Gray)),
                    Span::styled(wallet_name, Style::default().fg(Color::White)),
                    Span::raw("  "),
                    Span::styled("Session: ", Style::default().fg(Color::Gray)),
                    Span::styled(preview_id(session_id), Style::default().fg(Color::Cyan)),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Paste the signed raw transaction from CLI/manual broadcast.",
                    Style::default().fg(Color::Yellow),
                )),
                Line::from(Span::styled(
                    "The TUI recomputes the txid and publishes tx_broadcast only if it matches the selected wallet, session, and network.",
                    Style::default().fg(Color::Gray),
                )),
                Line::from(Span::styled(
                    "This announces room progress; it is not an on-chain confirmation.",
                    Style::default().fg(Color::DarkGray),
                )),
            ]
        }
        NostrSignState::Complete { txid } => {
            vec![
                Line::from(Span::styled(
                    "✓ Matching tx_broadcast announcement received",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    "Check the selected network explorer or node for on-chain confirmation.",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("TXID: ", Style::default().fg(Color::Gray)),
                    Span::styled(txid, Style::default().fg(Color::Cyan)),
                ]),
            ]
        }
        NostrSignState::ViewProposals { wallet_name } => {
            vec![
                Line::from(vec![
                    Span::styled("Wallet: ", Style::default().fg(Color::Gray)),
                    Span::styled(wallet_name, Style::default().fg(Color::White)),
                ]),
                Line::from(Span::styled(
                    "Boundary: proposals are public metadata; signing nonce/share payloads are encrypted.",
                    Style::default().fg(Color::DarkGray),
                )),
            ]
        }
        NostrSignState::WaitingForExecution {
            wallet_name,
            session_id,
        } => {
            vec![
                Line::from(vec![
                    Span::styled("Wallet: ", Style::default().fg(Color::Gray)),
                    Span::styled(wallet_name, Style::default().fg(Color::White)),
                    Span::raw("  "),
                    Span::styled("Session: ", Style::default().fg(Color::Gray)),
                    Span::styled(preview_id(session_id), Style::default().fg(Color::Cyan)),
                ]),
                Line::from(Span::styled(
                    "Consent sent; keep this room open for proposer execution.",
                    Style::default().fg(Color::Yellow),
                )),
                Line::from(Span::styled(
                    "Next: after threshold consent, parties exchange encrypted nonces and signature shares.",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(Span::styled(
                    "Boundary: proposals are public metadata; signing nonce/share payloads are encrypted.",
                    Style::default().fg(Color::DarkGray),
                )),
            ]
        }
        NostrSignState::SelectWallet => {
            vec![Line::from(Span::styled(
                "Select a wallet to start...",
                Style::default().fg(Color::DarkGray),
            ))]
        }
    };

    with_nostr_sign_context(app, state_lines)
}

fn with_nostr_sign_context<'a>(app: &'a App, mut state_lines: Vec<Line<'a>>) -> Vec<Line<'a>> {
    let room_id = if app.nostr_room_id.trim().is_empty() {
        "(unset)".to_string()
    } else {
        app.nostr_room_id.clone()
    };
    let mut lines = vec![
        Line::from(Span::styled(
            format!(
                "Room: {} | Party: {} | Threshold: {}-of-{}",
                room_id, app.nostr_my_index, app.nostr_threshold, app.nostr_n_parties
            ),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(Span::styled(
            format!(
                "Scheme: TSS | Rank: n/a | Transport: {}",
                app.nostr_transport_label()
            ),
            Style::default().fg(Color::Yellow),
        )),
    ];
    lines.append(&mut state_lines);
    lines
}

pub(crate) fn nostr_configure_source_summary(
    app: &App,
    wallet_name: &str,
) -> (String, String, &'static str) {
    if let (Some((change, index)), Some(address)) = (
        app.nostr_source_derivation_path(),
        app.send_form.get_selected_hd_address(),
    ) {
        return (
            frostdao::crypto::hd::format_bip86_path(
                app.network.to_bitcoin_network(),
                change,
                index,
            ),
            address,
            "MPC threshold shares sign this derived path with the HD tweak.",
        );
    }

    let root_address = app
        .wallets
        .iter()
        .find(|wallet| wallet.name == wallet_name)
        .and_then(|wallet| wallet_address_for_network(wallet, app.network))
        .unwrap_or("unknown source address")
        .to_string();

    (
        "root key-path".to_string(),
        root_address,
        "MPC threshold shares sign the root key-path address.",
    )
}

fn render_content(frame: &mut Frame, app: &App, area: Rect) {
    match &app.nostr_sign_state {
        NostrSignState::ConfigureTx { wallet_name } => {
            render_configure_tx(frame, app, wallet_name, area);
        }
        NostrSignState::SelectRole { .. } => {
            render_role_selection(frame, app, area);
        }
        NostrSignState::WaitingForConsent {
            consents,
            rejections,
            ..
        } => {
            render_consent_list(frame, app, consents, rejections, area);
        }
        NostrSignState::ViewProposals { .. } => {
            render_proposals_list(frame, app, area);
        }
        NostrSignState::ReviewProposal { proposal, .. } => {
            render_review_checklist(frame, proposal, area);
        }
        NostrSignState::CollectingShares {
            received_shares, ..
        } => {
            render_shares_list(frame, app, received_shares, area);
        }
        NostrSignState::WaitingForExecution { session_id, .. } => {
            render_waiting_execution_progress(frame, app, session_id, area);
        }
        NostrSignState::AnnounceBroadcast { .. } => {
            render_broadcast_announcement(frame, app, area);
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

fn render_broadcast_announcement(frame: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)])
        .split(area);

    app.nostr_broadcast_raw_tx_input
        .render(frame, rows[0], true);

    let details = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Network: ", Style::default().fg(Color::Gray)),
            Span::styled(
                app.network.display_name(),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from("Press Enter to publish the room announcement."),
        Line::from("Press Esc to return to the combine screen."),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Announcement Check"),
    );
    frame.render_widget(details, rows[1]);
}

fn render_configure_tx(frame: &mut Frame, app: &App, wallet_name: &str, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(4),
        ])
        .split(area);

    app.nostr_to_address_input.render(
        frame,
        rows[0],
        app.nostr_tx_focus == NostrTxField::Recipient,
    );
    app.nostr_amount_input
        .render(frame, rows[1], app.nostr_tx_focus == NostrTxField::Amount);

    let (_, source_address, _) = nostr_configure_source_summary(app, wallet_name);
    let mut detail_lines = vec![
        Line::from(vec![
            Span::styled("Source: ", Style::default().fg(Color::Gray)),
            Span::styled(source_address, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
    ];
    detail_lines.extend(nostr_configure_network_lines(app));

    let details = Paragraph::new(detail_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Proposal Draft ")
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(details, rows[2]);
}

pub(crate) fn nostr_configure_network_lines(app: &App) -> Vec<Line<'static>> {
    if let Err(error) = app.ensure_nostr_proposal_network_available() {
        return vec![
            Line::from(vec![
                Span::styled("Unavailable: ", Style::default().fg(Color::Red)),
                Span::styled(error.to_string(), Style::default().fg(Color::Red)),
            ]),
            Line::from(Span::styled(
                "For regtest, set FROSTDAO_REGTEST_MEMPOOL_API to a local Esplora/mempool API; testnet4, testnet3, and signet use mempool.space.",
                Style::default().fg(Color::DarkGray),
            )),
        ];
    }

    vec![
        Line::from(vec![
            Span::styled("Recipient network: ", Style::default().fg(Color::Gray)),
            Span::styled(app.network.display_name(), Style::default().fg(Color::Cyan)),
            Span::styled(" expects ", Style::default().fg(Color::Gray)),
            Span::styled(
                app.network.recipient_address_prefix_hint(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" address", Style::default().fg(Color::Gray)),
        ]),
        Line::from(Span::styled(
            "Enter builds a real unsigned transaction, source path, source address, and BIP341 sighash for cross-device review.",
            Style::default().fg(Color::DarkGray),
        )),
    ]
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
    rejections: &std::collections::HashMap<u32, String>,
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
            } else if rejections.contains_key(&idx) {
                ("✗ Rejected", Color::Red)
            } else {
                ("○ Pending", Color::DarkGray)
            };
            let reason = rejections
                .get(&idx)
                .map(|reason| format!("  {}", reason))
                .unwrap_or_default();

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
                Span::styled(reason, Style::default().fg(Color::DarkGray)),
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

fn render_proposals_list(frame: &mut Frame, app: &App, area: Rect) {
    let mut proposals = app
        .nostr_pending_proposals
        .values()
        .filter(|proposal| {
            matches!(
                &app.nostr_sign_state,
                NostrSignState::ViewProposals { wallet_name }
                    if proposal.wallet_name == *wallet_name
            ) && proposal.proposer_index != app.nostr_my_index
        })
        .collect::<Vec<_>>();
    proposals.sort_by_key(|proposal| proposal.timestamp);

    let items: Vec<ListItem> = if proposals.is_empty() {
        pending_proposals_empty_lines()
            .into_iter()
            .map(ListItem::new)
            .collect()
    } else {
        proposals
            .into_iter()
            .map(|proposal| ListItem::new(pending_proposal_lines(proposal)))
            .collect()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Pending Proposals ")
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(list, area);
}

fn pending_proposals_empty_lines() -> Vec<Line<'static>> {
    vec![
        Line::from(Span::styled(
            "No pending proposals",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Only proposals for this wallet and active room appear here.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "Wait for room polling, or confirm every signer joined the same room.",
            Style::default().fg(Color::DarkGray),
        )),
    ]
}

fn pending_proposal_lines(proposal: &crate::tui::state::TxProposal) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled("Wallet: ", Style::default().fg(Color::Gray)),
            Span::styled(
                proposal.wallet_name.clone(),
                Style::default().fg(Color::White),
            ),
            Span::styled("  Network: ", Style::default().fg(Color::Gray)),
            Span::styled(
                proposal.review.network.clone(),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled("  Session: ", Style::default().fg(Color::Gray)),
            Span::styled(
                proposal.session_id.clone(),
                Style::default().fg(Color::Magenta),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                format!("Party {} ", proposal.proposer_index),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(format!("{} sats -> ", proposal.amount_sats)),
            Span::styled(
                proposal.to_address.clone(),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("  "),
            Span::styled(
                proposal.review.sighash_fingerprint.clone(),
                Style::default().fg(Color::Magenta),
            ),
        ]),
    ]
}

fn review_checklist_lines(proposal: &crate::tui::state::TxProposal) -> Vec<Line<'static>> {
    let unsigned_tx_summary = unsigned_tx_review_summary(&proposal.unsigned_tx);
    vec![
        Line::from(Span::styled(
            "Before consenting, compare these fields on every device:",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        checklist_line("Wallet", proposal.wallet_name.clone()),
        checklist_line(
            "Network and session",
            format!("{} / {}", proposal.review.network, proposal.session_id),
        ),
        checklist_line(
            "Source",
            format!(
                "{} from {}",
                proposal.review.source_path, proposal.review.from_address
            ),
        ),
        checklist_line("Destination", proposal.review.to_address.clone()),
        checklist_line(
            "Amount and fee",
            format!(
                "{} sats at {} sat/vB",
                proposal.amount_sats, proposal.fee_rate
            ),
        ),
        checklist_line(
            "Sighash fingerprint",
            proposal.review.sighash_fingerprint.clone(),
        ),
        checklist_line("Unsigned tx", unsigned_tx_summary),
        checklist_line("Proposer", format!("Party {}", proposal.proposer_index)),
        Line::from(""),
        Line::from(vec![
            Span::styled("Only press ", Style::default().fg(Color::DarkGray)),
            Span::styled("y", Style::default().fg(Color::Yellow)),
            Span::styled(
                " when every line matches; press ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled("r", Style::default().fg(Color::Yellow)),
            Span::styled(
                " to publish rejection.",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ]
}

fn unsigned_tx_review_summary(unsigned_tx: &str) -> String {
    let byte_count = unsigned_tx.trim().len() / 2;
    format!(
        "{} bytes / {}",
        byte_count,
        frostdao::protocol::dkg_tx::sighash_fingerprint(unsigned_tx.trim())
    )
}

fn checklist_line(label: &'static str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled("□ ", Style::default().fg(Color::Cyan)),
        Span::styled(label, Style::default().fg(Color::Gray)),
        Span::styled(": ", Style::default().fg(Color::Gray)),
        Span::styled(value, Style::default().fg(Color::White)),
    ])
}

fn render_review_checklist(
    frame: &mut Frame,
    proposal: &crate::tui::state::TxProposal,
    area: Rect,
) {
    let checklist = Paragraph::new(review_checklist_lines(proposal)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Consent Checklist ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(checklist, area);
}

fn render_shares_list(
    frame: &mut Frame,
    app: &App,
    received_shares: &HashMap<u32, String>,
    area: Rect,
) {
    let session_id = match &app.nostr_sign_state {
        NostrSignState::CollectingShares { session_id, .. } => session_id.as_str(),
        _ => "",
    };
    render_signing_progress_list(
        frame,
        app,
        session_id,
        Some(received_shares),
        "Coordinator Progress",
        area,
    );
}

fn render_waiting_execution_progress(frame: &mut Frame, app: &App, session_id: &str, area: Rect) {
    render_signing_progress_list(frame, app, session_id, None, "Room Signing Progress", area);
}

fn render_signing_progress_list(
    frame: &mut Frame,
    app: &App,
    session_id: &str,
    received_shares: Option<&HashMap<u32, String>>,
    title: &str,
    area: Rect,
) {
    let received_shares = signing_session_shares(app, session_id, received_shares);
    let counts = signing_progress_counts(app, session_id, &received_shares);
    let items: Vec<ListItem> = signing_progress_party_lines(app, session_id, &received_shares)
        .into_iter()
        .map(ListItem::new)
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(
                " {title} · nonces {}/{} · shares {}/{} ",
                counts.nonce_count, counts.threshold, counts.share_count, counts.threshold
            ))
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(list, area);
}

fn signing_session_shares(
    app: &App,
    session_id: &str,
    received_shares: Option<&HashMap<u32, String>>,
) -> HashMap<u32, String> {
    received_shares
        .cloned()
        .or_else(|| app.nostr_received_shares.get(session_id).cloned())
        .unwrap_or_default()
}

fn signing_progress_party_lines(
    app: &App,
    session_id: &str,
    received_shares: &HashMap<u32, String>,
) -> Vec<Line<'static>> {
    (1..=app.nostr_n_parties)
        .map(|idx| {
            let is_me = idx == app.nostr_my_index;
            let has_nonce = app
                .nostr_received_nonces
                .get(session_id)
                .is_some_and(|nonces| nonces.contains_key(&idx));
            let has_share = received_shares.contains_key(&idx);

            let nonce = if has_nonce {
                ("nonce ok", Color::Green)
            } else if is_me {
                ("local nonce", Color::Yellow)
            } else {
                ("nonce missing", Color::DarkGray)
            };
            let share = if has_share {
                ("share ok", Color::Green)
            } else if has_nonce {
                ("share pending", Color::Yellow)
            } else {
                ("blocked", Color::DarkGray)
            };

            let name_style = if is_me {
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let me_indicator = if is_me { " (me)" } else { "" };

            Line::from(vec![
                Span::styled(format!("Party {}{}", idx, me_indicator), name_style),
                Span::raw("  "),
                Span::styled(nonce.0, Style::default().fg(nonce.1)),
                Span::raw("  "),
                Span::styled(share.0, Style::default().fg(share.1)),
            ])
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SigningProgressCounts {
    nonce_count: usize,
    share_count: usize,
    threshold: usize,
    ready_to_combine: bool,
}

fn signing_progress_counts(
    app: &App,
    session_id: &str,
    received_shares: &HashMap<u32, String>,
) -> SigningProgressCounts {
    let threshold = app.nostr_threshold as usize;
    if let Some(coordinator) = app.nostr_signing_coordinators.get(session_id) {
        return SigningProgressCounts {
            nonce_count: coordinator.collector().nonce_count(),
            share_count: coordinator.collector().share_count(),
            threshold,
            ready_to_combine: coordinator.ready_to_combine(),
        };
    }

    SigningProgressCounts {
        nonce_count: app
            .nostr_received_nonces
            .get(session_id)
            .map_or(0, HashMap::len),
        share_count: received_shares.len(),
        threshold,
        ready_to_combine: received_shares.len() >= threshold,
    }
}

fn render_help(frame: &mut Frame, app: &App, area: Rect) {
    let help_text = nostr_sign_help_text(&app.nostr_sign_state);

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

pub(crate) fn nostr_sign_help_text(state: &NostrSignState) -> String {
    match state {
        NostrSignState::SelectRole { .. } => {
            "p: Propose | c: Consent | Enter: Propose | Esc: Back".to_string()
        }
        NostrSignState::ConfigureTx { .. } => {
            String::from("Tab:Field | Enter:Publish proposal | Ctrl+u:Clear field | Esc:Back")
        }
        NostrSignState::ReviewProposal { .. } => String::from(
            "y: Consent only after every signer matches review | r: Reject | Esc: Back",
        ),
        NostrSignState::WaitingForExecution { .. } => String::from(
            "Enter: Poll room | keep room open for encrypted nonce/share exchange | Esc: Back",
        ),
        NostrSignState::Combining { .. } => {
            format!(
                "{COPY_KEY_LABEL}: Copy dkg-broadcast | a: Announce raw tx | Enter: Poll | Esc: Back"
            )
        }
        NostrSignState::AnnounceBroadcast { .. } => {
            String::from("Paste/type raw tx | Ctrl+u: Clear | Enter: Publish | Esc: Back")
        }
        NostrSignState::Complete { .. } => {
            format!("Enter: Done | {COPY_KEY_LABEL}: Copy TXID")
        }
        _ => String::from("Enter: Continue | Esc: Cancel"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::NetworkSelection;
    use crate::tui::state::TxProposal;
    use frostdao::protocol::keygen::WalletSummary;
    use frostdao::protocol::{
        SigningAttemptConfig, SigningCoordinator, SigningNonceInput, SigningSchemePolicy,
        SigningShareInput,
    };
    use serial_test::serial;
    use std::collections::BTreeMap;

    fn test_review_proposal() -> TxProposal {
        TxProposal {
            session_id: "session-review".to_string(),
            wallet_name: "treasury".to_string(),
            proposer_index: 2,
            to_address: "tb1qrecipient".to_string(),
            amount_sats: 50_000,
            fee_rate: 7,
            sighash: "abcdef".to_string(),
            unsigned_tx: "02000000000100".to_string(),
            review: frostdao::nostr::TxReviewPayload {
                network: "Testnet3".to_string(),
                source_path: "m/86'/1'/0'/0/0".to_string(),
                from_address: "tb1qsource".to_string(),
                to_address: "tb1qrecipient".to_string(),
                amount_sats: 50_000,
                fee_rate_sats_vb: 7,
                sighash_fingerprint: "abc12345".to_string(),
            },
            description: "test proposal".to_string(),
            timestamp: 1_700_000_000,
        }
    }

    fn lines_to_string(lines: Vec<Line<'_>>) -> String {
        lines
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn wallet_summary(name: &str, address: &str) -> WalletSummary {
        WalletSummary {
            name: name.to_string(),
            threshold: Some(2),
            total_parties: Some(3),
            hierarchical: Some(false),
            address: Some(address.to_string()),
            address_testnet: Some(address.to_string()),
            address_mainnet: None,
            address_regtest: None,
            signing_requirement: None,
            party_ranks: None::<BTreeMap<u32, u32>>,
        }
    }

    fn app_with_room_context() -> App {
        let mut app = App::new().unwrap();
        app.nostr_room_id = "treasury-room".to_string();
        app.nostr_my_index = 2;
        app.nostr_threshold = 2;
        app.nostr_n_parties = 3;
        app
    }

    #[test]
    fn nostr_sign_status_keeps_room_context_across_states() {
        let proposal = test_review_proposal();
        let states = [
            NostrSignState::SelectWallet,
            NostrSignState::SelectRole {
                wallet_name: "treasury".to_string(),
            },
            NostrSignState::ConfigureTx {
                wallet_name: "treasury".to_string(),
            },
            NostrSignState::WaitingForConsent {
                wallet_name: "treasury".to_string(),
                session_id: "session-a".to_string(),
                proposal: proposal.clone(),
                consents: HashMap::new(),
                rejections: HashMap::new(),
            },
            NostrSignState::ViewProposals {
                wallet_name: "treasury".to_string(),
            },
            NostrSignState::ReviewProposal {
                wallet_name: "treasury".to_string(),
                proposal: proposal.clone(),
            },
            NostrSignState::WaitingForExecution {
                wallet_name: "treasury".to_string(),
                session_id: "session-a".to_string(),
            },
            NostrSignState::CollectingShares {
                wallet_name: "treasury".to_string(),
                session_id: "session-a".to_string(),
                received_shares: HashMap::new(),
            },
            NostrSignState::Combining {
                wallet_name: "treasury".to_string(),
                session_id: "session-a".to_string(),
            },
            NostrSignState::AnnounceBroadcast {
                wallet_name: "treasury".to_string(),
                session_id: "session-a".to_string(),
            },
            NostrSignState::Complete {
                txid: "abc123456789".to_string(),
            },
        ];

        for state in states {
            let mut app = app_with_room_context();
            app.nostr_sign_state = state;

            let rendered = lines_to_string(nostr_sign_status_lines(&app));

            assert!(rendered.contains("Room: treasury-room"));
            assert!(rendered.contains("Party: 2"));
            assert!(rendered.contains("Threshold: 2-of-3"));
            assert!(rendered.contains("Scheme: TSS"));
            assert!(rendered.contains("Rank: n/a"));
            assert!(rendered.contains("Transport: local simulation"));
        }
    }

    #[test]
    fn nostr_sign_status_labels_public_and_encrypted_boundaries() {
        let mut app = app_with_room_context();
        app.nostr_sign_state = NostrSignState::ViewProposals {
            wallet_name: "treasury".to_string(),
        };

        let rendered = lines_to_string(nostr_sign_status_lines(&app));

        assert!(rendered.contains("proposals are public metadata"));
        assert!(rendered.contains("signing nonce/share payloads are encrypted"));
    }

    #[test]
    fn waiting_for_execution_tells_consenter_to_keep_room_open() {
        let mut app = app_with_room_context();
        app.nostr_sign_state = NostrSignState::WaitingForExecution {
            wallet_name: "treasury".to_string(),
            session_id: "session-a".to_string(),
        };

        let rendered = lines_to_string(nostr_sign_status_lines(&app));
        let help = nostr_sign_help_text(&app.nostr_sign_state);

        assert!(rendered.contains("Consent sent"));
        assert!(rendered.contains("keep this room open"));
        assert!(rendered.contains("after threshold consent"));
        assert!(rendered.contains("encrypted nonces and signature shares"));
        assert!(rendered.contains("Session: session-"));
        assert!(help.contains("Poll room"));
        assert!(help.contains("keep room open"));
        assert!(help.contains("encrypted nonce/share exchange"));
        assert!(!help.contains("Continue"));
    }

    #[test]
    fn combining_status_explains_cli_handoff_and_broadcast_wait() {
        let mut app = app_with_room_context();
        app.nostr_sign_state = NostrSignState::Combining {
            wallet_name: "treasury".to_string(),
            session_id: "session-a".to_string(),
        };

        let rendered = lines_to_string(nostr_sign_status_lines(&app));
        let help = nostr_sign_help_text(&app.nostr_sign_state);

        assert!(rendered.contains("Combine handoff"));
        assert!(rendered.contains("frostdao dkg-broadcast"));
        assert!(rendered.contains("matching tx_broadcast"));
        assert!(rendered.contains("not an on-chain confirmation"));
        assert!(help.contains("Copy dkg-broadcast"));
        assert!(help.contains("Announce raw tx"));
        assert!(help.contains("Enter: Poll"));
    }

    #[test]
    fn announce_broadcast_status_explains_raw_tx_validation() {
        let mut app = app_with_room_context();
        app.nostr_sign_state = NostrSignState::AnnounceBroadcast {
            wallet_name: "treasury".to_string(),
            session_id: "session-a".to_string(),
        };

        let rendered = lines_to_string(nostr_sign_status_lines(&app));
        let help = nostr_sign_help_text(&app.nostr_sign_state);

        assert!(rendered.contains("Paste the signed raw transaction"));
        assert!(rendered.contains("recomputes the txid"));
        assert!(rendered.contains("not an on-chain confirmation"));
        assert!(help.contains("Paste/type raw tx"));
        assert!(help.contains("Enter: Publish"));
    }

    #[test]
    fn status_lines_tolerate_short_session_ids() {
        let mut app = app_with_room_context();
        app.nostr_sign_state = NostrSignState::AnnounceBroadcast {
            wallet_name: "treasury".to_string(),
            session_id: "s".to_string(),
        };

        let rendered = lines_to_string(nostr_sign_status_lines(&app));

        assert!(rendered.contains("Session: s"));
    }

    #[test]
    fn complete_progress_tolerates_short_txids() {
        let state = NostrSignState::Complete {
            txid: "abc".to_string(),
        };

        let (progress, label) = get_progress(&state, &app_with_room_context());

        assert_eq!(progress, 100);
        assert!(label.contains("abc"));
    }

    #[test]
    fn complete_status_distinguishes_room_announcement_from_chain_confirmation() {
        let mut app = app_with_room_context();
        app.nostr_sign_state = NostrSignState::Complete {
            txid: "txid-test".to_string(),
        };

        let rendered = lines_to_string(nostr_sign_status_lines(&app));

        assert!(rendered.contains("Matching tx_broadcast announcement received"));
        assert!(rendered.contains("on-chain confirmation"));
        assert!(rendered.contains("txid-test"));
    }

    #[test]
    fn review_checklist_contains_required_consent_fields() {
        let proposal = test_review_proposal();
        let rendered = review_checklist_lines(&proposal)
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        for expected in [
            "Wallet: treasury",
            "Network and session",
            "Testnet3 / session-review",
            "m/86'/1'/0'/0/0 from tb1qsource",
            "Destination: tb1qrecipient",
            "50000 sats at 7 sat/vB",
            "Sighash fingerprint: abc12345",
            "Unsigned tx: 7 bytes / 02000000000100",
            "Proposer: Party 2",
            "Only press y when every line matches",
            "press r to publish rejection",
        ] {
            assert!(rendered.contains(expected), "missing {expected}");
        }
    }

    #[test]
    fn review_help_requires_cross_device_match_before_consent() {
        let help = nostr_sign_help_text(&NostrSignState::ReviewProposal {
            wallet_name: "treasury".to_string(),
            proposal: test_review_proposal(),
        });

        assert!(help.contains("Consent only after every signer matches review"));
        assert!(help.contains("r: Reject"));
    }

    #[test]
    fn pending_proposal_lines_show_identity_before_review() {
        let proposal = test_review_proposal();
        let rendered = pending_proposal_lines(&proposal)
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        for expected in [
            "Wallet: treasury",
            "Network: Testnet3",
            "Session: session-review",
            "Party 2",
            "50000 sats -> tb1qrecipient",
            "abc12345",
        ] {
            assert!(rendered.contains(expected), "missing {expected}");
        }
    }

    #[test]
    fn pending_proposals_empty_state_explains_room_and_wallet_filters() {
        let rendered = lines_to_string(pending_proposals_empty_lines());

        assert!(rendered.contains("No pending proposals"));
        assert!(rendered.contains("this wallet and active room"));
        assert!(rendered.contains("room polling"));
        assert!(rendered.contains("same room"));
        assert!(!rendered.contains("relay polling"));
    }

    #[test]
    fn configure_source_summary_uses_selected_hd_address() {
        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Signet;
        app.wallets = vec![wallet_summary("treasury", "tb1proot")];
        app.send_form.hd_enabled = true;
        app.send_form.use_hd_address = true;
        app.send_form.hd_selected_index = 0;
        app.send_form.hd_addresses =
            vec![("tb1pagentderived".to_string(), "pubkey".to_string(), 9)];

        let (path, address, control) = nostr_configure_source_summary(&app, "treasury");

        assert_eq!(path, "m/86'/1'/0'/0/9");
        assert_eq!(address, "tb1pagentderived");
        assert!(control.contains("HD tweak"));
    }

    #[test]
    #[serial]
    fn configure_network_lines_explain_regtest_unavailable() {
        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);
        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Regtest;

        let rendered = lines_to_string(nostr_configure_network_lines(&app));

        assert!(rendered.contains("Unavailable"));
        assert!(rendered.contains("Nostr transaction proposals need a UTXO API on Regtest"));
        assert!(rendered.contains("local Esplora/mempool API"));
        assert!(rendered.contains("FROSTDAO_REGTEST_MEMPOOL_API"));
        assert!(rendered.contains("testnet4, testnet3, and signet use mempool.space"));
    }

    #[test]
    fn configure_network_lines_show_recipient_prefix_when_available() {
        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Mainnet;

        let rendered = lines_to_string(nostr_configure_network_lines(&app));

        assert!(rendered.contains("Recipient network: Mainnet expects bc1... address"));
        assert!(rendered.contains("real unsigned transaction"));
    }

    #[test]
    fn signing_progress_counts_falls_back_to_session_inboxes() {
        let mut app = App::new().unwrap();
        app.nostr_threshold = 2;
        app.nostr_received_nonces.insert(
            "session-a".to_string(),
            HashMap::from([(2, "nonce-2".to_string()), (3, "nonce-3".to_string())]),
        );
        let received_shares = HashMap::from([(2, "share-2".to_string())]);

        let counts = signing_progress_counts(&app, "session-a", &received_shares);

        assert_eq!(
            counts,
            SigningProgressCounts {
                nonce_count: 2,
                share_count: 1,
                threshold: 2,
                ready_to_combine: false,
            }
        );
    }

    #[test]
    fn waiting_execution_progress_uses_room_session_inboxes() {
        let mut app = app_with_room_context();
        app.nostr_received_nonces.insert(
            "session-a".to_string(),
            HashMap::from([(1, "nonce-1".to_string()), (3, "nonce-3".to_string())]),
        );
        app.nostr_received_shares.insert(
            "session-a".to_string(),
            HashMap::from([(3, "share-3".to_string())]),
        );

        let received_shares = signing_session_shares(&app, "session-a", None);
        let rendered = lines_to_string(signing_progress_party_lines(
            &app,
            "session-a",
            &received_shares,
        ));
        let counts = signing_progress_counts(&app, "session-a", &received_shares);

        assert!(rendered.contains("Party 1  nonce ok  share pending"));
        assert!(rendered.contains("Party 2 (me)  local nonce  blocked"));
        assert!(rendered.contains("Party 3  nonce ok  share ok"));
        assert_eq!(
            counts,
            SigningProgressCounts {
                nonce_count: 2,
                share_count: 1,
                threshold: 2,
                ready_to_combine: false,
            }
        );
    }

    #[test]
    fn signing_progress_counts_prefers_coordinator_state() {
        let mut app = App::new().unwrap();
        app.nostr_threshold = 2;
        let config = SigningAttemptConfig::new_with_attempt_id(
            "treasury",
            "session-a",
            "attempt-a",
            vec![1, 2, 3],
            2,
            "fingerprint-a",
            SigningSchemePolicy::Tss,
        )
        .unwrap();
        let mut coordinator = SigningCoordinator::new(config.clone()).unwrap();
        for party_index in [1, 2] {
            coordinator
                .accept_nonce(SigningNonceInput {
                    wallet: config.wallet.clone(),
                    session: config.session.clone(),
                    attempt_id: config.attempt_id.clone(),
                    signer_set: config.signer_set.clone(),
                    party_index,
                    sighash_fingerprint: config.sighash_fingerprint.clone(),
                    public_nonce: format!("nonce-{party_index}"),
                })
                .unwrap();
        }
        for party_index in [1, 2] {
            coordinator
                .accept_share(SigningShareInput {
                    wallet: config.wallet.clone(),
                    session: config.session.clone(),
                    attempt_id: config.attempt_id.clone(),
                    signer_set: config.signer_set.clone(),
                    party_index,
                    sighash_fingerprint: config.sighash_fingerprint.clone(),
                    signature_share: format!("share-{party_index}"),
                })
                .unwrap();
        }
        app.nostr_signing_coordinators
            .insert("session-a".to_string(), coordinator);

        let counts = signing_progress_counts(&app, "session-a", &HashMap::new());

        assert_eq!(
            counts,
            SigningProgressCounts {
                nonce_count: 2,
                share_count: 2,
                threshold: 2,
                ready_to_combine: true,
            }
        );
    }
}
