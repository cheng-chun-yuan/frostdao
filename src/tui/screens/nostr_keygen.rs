//! Nostr DKG keygen screen - live distributed key generation

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};

use crate::tui::app::App;
use crate::tui::state::NostrKeygenState;

/// Render the Nostr keygen screen
pub fn render_nostr_keygen(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(3), // Progress bar
            Constraint::Length(3), // Status
            Constraint::Min(10),   // Party list
            Constraint::Length(5), // Help
        ])
        .margin(1)
        .split(area);

    // Title
    let phase = match &app.nostr_keygen_state {
        NostrKeygenState::ModeSelect => "Setup",
        NostrKeygenState::WaitingForParties { .. } => "Round 1",
        NostrKeygenState::Round2 { .. } => "Round 2",
        NostrKeygenState::Finalizing => "Finalizing",
    };

    let title = Paragraph::new(Line::from(vec![
        Span::styled("🔑 ", Style::default()),
        Span::styled(
            format!("Nostr DKG - {}", phase),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    // Progress bar
    let (progress, label) = match &app.nostr_keygen_state {
        NostrKeygenState::ModeSelect => (0, "Press Enter to start...".to_string()),
        NostrKeygenState::WaitingForParties { received_round1 } => {
            let count = received_round1.len();
            let total = app.nostr_n_parties as usize;
            let pct = (count * 100 / total.max(1)) as u16;
            (pct, format!("Round 1: {}/{} parties", count, total))
        }
        NostrKeygenState::Round2 { received_round2 } => {
            let count = received_round2.len();
            let total = app.nostr_n_parties as usize;
            let pct = 50 + (count * 50 / total.max(1)) as u16;
            (pct, format!("Round 2: {}/{} shares", count, total))
        }
        NostrKeygenState::Finalizing => (95, "Finalizing wallet...".to_string()),
    };

    let gauge = Gauge::default()
        .block(Block::default())
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .percent(progress)
        .label(label);
    frame.render_widget(gauge, chunks[1]);

    // Status message
    let status_msg = match &app.nostr_keygen_state {
        NostrKeygenState::ModeSelect => {
            format!(
                "Room: {}  |  {}-of-{}  |  My Index: {}",
                app.nostr_room_id, app.nostr_threshold, app.nostr_n_parties, app.nostr_my_index
            )
        }
        NostrKeygenState::WaitingForParties { .. } => {
            "Broadcasting Round 1 commitment and waiting for others...".to_string()
        }
        NostrKeygenState::Round2 { .. } => "Processing encrypted shares (NIP-44)...".to_string(),
        NostrKeygenState::Finalizing => "Computing final key shares...".to_string(),
    };

    let status = Paragraph::new(Span::styled(status_msg, Style::default().fg(Color::Yellow)));
    frame.render_widget(status, chunks[2]);

    // Party list
    render_party_list(frame, app, chunks[3]);

    // Help
    let help_lines = vec![Line::from(vec![
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::raw(": Start/Continue  "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(": Cancel  "),
        Span::styled("R", Style::default().fg(Color::Yellow)),
        Span::raw(": Retry"),
    ])];
    let help = Paragraph::new(help_lines).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(help, chunks[4]);
}

fn render_party_list(frame: &mut Frame, app: &App, area: Rect) {
    let n = app.nostr_n_parties as usize;

    let items: Vec<ListItem> = (1..=n)
        .map(|idx| {
            let idx = idx as u32;
            let is_me = idx == app.nostr_my_index;

            let (status, style) = match &app.nostr_keygen_state {
                NostrKeygenState::WaitingForParties { received_round1 } => {
                    if received_round1.contains_key(&idx) {
                        ("✓ Round 1", Style::default().fg(Color::Green))
                    } else if is_me {
                        ("● Broadcasting...", Style::default().fg(Color::Yellow))
                    } else {
                        ("○ Waiting...", Style::default().fg(Color::DarkGray))
                    }
                }
                NostrKeygenState::Round2 { received_round2 } => {
                    if received_round2.contains_key(&idx) {
                        ("✓ Round 2", Style::default().fg(Color::Green))
                    } else if is_me {
                        ("● Processing...", Style::default().fg(Color::Yellow))
                    } else {
                        ("○ Waiting...", Style::default().fg(Color::DarkGray))
                    }
                }
                NostrKeygenState::Finalizing => ("✓ Complete", Style::default().fg(Color::Green)),
                _ => ("○ Ready", Style::default().fg(Color::DarkGray)),
            };

            let name_style = if is_me {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let me_indicator = if is_me { " (me)" } else { "" };

            ListItem::new(Line::from(vec![
                Span::styled(format!("Party {}{}", idx, me_indicator), name_style),
                Span::raw("  "),
                Span::styled(status, style),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Participants ")
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(list, area);
}
