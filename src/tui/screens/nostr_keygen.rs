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
            Constraint::Length(5), // Status
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
    let status = Paragraph::new(keygen_status_lines(app));
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

fn keygen_status_lines(app: &App) -> Vec<Line<'static>> {
    let room_id = if app.nostr_room_id.trim().is_empty() {
        "(unset)"
    } else {
        app.nostr_room_id.as_str()
    };
    let phase_line = match &app.nostr_keygen_state {
        NostrKeygenState::ModeSelect => {
            "Setup: room joins are public; DKG share payloads must be encrypted before relay handoff."
        }
        NostrKeygenState::WaitingForParties { .. } => {
            "Round 1: public commitments; waiting for all parties."
        }
        NostrKeygenState::Round2 { .. } => {
            "Round 2: encrypted shares (NIP-44); verify intended recipient before use."
        }
        NostrKeygenState::Finalizing => {
            "Finalizing: local share material stays on this device."
        }
    };

    vec![
        Line::from(Span::styled(
            format!(
                "Room: {} | Party: {} | Threshold: {}-of-{}",
                room_id, app.nostr_my_index, app.nostr_threshold, app.nostr_n_parties
            ),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(Span::styled(
            "Scheme: TSS | Rank: n/a | Transport: local simulation",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(Span::styled(phase_line, Style::default().fg(Color::Yellow))),
    ]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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

    fn app_with_room_context() -> App {
        let mut app = App::new().expect("app should initialize");
        app.nostr_room_id = "treasury-room".to_string();
        app.nostr_my_index = 2;
        app.nostr_threshold = 2;
        app.nostr_n_parties = 3;
        app
    }

    #[test]
    fn keygen_status_lines_keep_room_context_across_rounds() {
        let states = [
            NostrKeygenState::ModeSelect,
            NostrKeygenState::WaitingForParties {
                received_round1: HashMap::new(),
            },
            NostrKeygenState::Round2 {
                received_round2: HashMap::new(),
            },
            NostrKeygenState::Finalizing,
        ];

        for state in states {
            let mut app = app_with_room_context();
            app.nostr_keygen_state = state;

            let text = lines_to_string(&keygen_status_lines(&app));

            assert!(text.contains("Room: treasury-room"));
            assert!(text.contains("Party: 2"));
            assert!(text.contains("2-of-3"));
            assert!(text.contains("Scheme: TSS"));
            assert!(text.contains("Rank: n/a"));
            assert!(text.contains("Transport: local simulation"));
        }
    }

    #[test]
    fn keygen_status_lines_label_public_and_encrypted_boundaries() {
        let mut app = app_with_room_context();
        app.nostr_keygen_state = NostrKeygenState::ModeSelect;
        let setup = lines_to_string(&keygen_status_lines(&app));

        app.nostr_keygen_state = NostrKeygenState::WaitingForParties {
            received_round1: HashMap::new(),
        };
        let round1 = lines_to_string(&keygen_status_lines(&app));

        app.nostr_keygen_state = NostrKeygenState::Round2 {
            received_round2: HashMap::new(),
        };
        let round2 = lines_to_string(&keygen_status_lines(&app));

        app.nostr_keygen_state = NostrKeygenState::Finalizing;
        let finalizing = lines_to_string(&keygen_status_lines(&app));

        assert!(setup.contains("public"));
        assert!(setup.contains("encrypted"));
        assert!(round1.contains("public commitments"));
        assert!(round2.contains("encrypted shares"));
        assert!(finalizing.contains("local share material stays on this device"));
    }
}
