//! Nostr room configuration screen

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};

use crate::tui::app::App;
use crate::tui::state::{NostrRoomField, NostrRoomPhase};

/// Render the Nostr room configuration screen
pub fn render_nostr_room(frame: &mut Frame, app: &App, area: Rect) {
    match app.nostr_room_phase {
        NostrRoomPhase::Configure => render_configure(frame, app, area),
        NostrRoomPhase::WaitingForParticipants => render_waiting(frame, app, area),
        NostrRoomPhase::Ready => render_ready(frame, app, area),
    }
}

fn render_configure(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(5), // Info box
            Constraint::Length(3), // Room ID
            Constraint::Length(3), // My Index
            Constraint::Length(3), // Threshold
            Constraint::Length(3), // N Parties
            Constraint::Length(3), // Status
            Constraint::Min(0),    // Spacer
            Constraint::Length(4), // Help
        ])
        .margin(1)
        .split(area);

    // Title
    let title = Paragraph::new(Line::from(vec![
        Span::styled("🌐 ", Style::default()),
        Span::styled(
            "Nostr Room - Configure",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    // Info box
    let info_lines = vec![
        Line::from(Span::styled(
            "Distributed DKG - Each party runs on a different device.",
            Style::default().fg(Color::Gray),
        )),
        Line::from(Span::styled(
            "All parties must use the same Room ID to coordinate.",
            Style::default().fg(Color::Gray),
        )),
        Line::from(vec![
            Span::styled("Relay: ", Style::default().fg(Color::DarkGray)),
            Span::styled("wss://relay.damus.io", Style::default().fg(Color::Green)),
        ]),
    ];
    let info = Paragraph::new(info_lines);
    frame.render_widget(info, chunks[1]);

    // Form fields
    render_text_field(
        frame,
        chunks[2],
        "Room ID",
        &app.nostr_room_id,
        app.nostr_room_focus == NostrRoomField::RoomId,
    );

    render_text_field(
        frame,
        chunks[3],
        "My Index",
        &app.nostr_my_index.to_string(),
        app.nostr_room_focus == NostrRoomField::MyIndex,
    );

    render_text_field(
        frame,
        chunks[4],
        "Threshold",
        &app.nostr_threshold.to_string(),
        app.nostr_room_focus == NostrRoomField::Threshold,
    );

    render_text_field(
        frame,
        chunks[5],
        "Parties",
        &app.nostr_n_parties.to_string(),
        app.nostr_room_focus == NostrRoomField::NParties,
    );

    // Connection status
    let status_style = if app.nostr_connected {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Yellow)
    };
    let status_text = if app.nostr_connected {
        "● Connected"
    } else {
        "○ Not connected"
    };
    let status = Paragraph::new(Line::from(vec![
        Span::styled("Status: ", Style::default().fg(Color::Gray)),
        Span::styled(status_text, status_style),
    ]));
    frame.render_widget(status, chunks[6]);

    // Help
    let help_lines = vec![Line::from(vec![
        Span::styled("Tab", Style::default().fg(Color::Yellow)),
        Span::raw(": Next field  "),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::raw(": Join Room  "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(": Back"),
    ])];
    let help = Paragraph::new(help_lines).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(help, chunks[8]);
}

fn render_waiting(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(3), // Progress bar
            Constraint::Length(3), // Room info
            Constraint::Min(10),   // Participant list
            Constraint::Length(5), // Help
        ])
        .margin(1)
        .split(area);

    // Title
    let title = Paragraph::new(Line::from(vec![
        Span::styled("🌐 ", Style::default()),
        Span::styled(
            "Nostr Room - Waiting for Participants",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    // Progress bar
    let joined = app.nostr_participants.len();
    let total = app.nostr_n_parties as usize;
    let pct = if total > 0 {
        (joined * 100 / total) as u16
    } else {
        0
    };

    let gauge = Gauge::default()
        .block(Block::default())
        .gauge_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
        .percent(pct)
        .label(format!("{}/{} participants joined", joined, total));
    frame.render_widget(gauge, chunks[1]);

    // Room info
    let room_info = Paragraph::new(Line::from(vec![
        Span::styled("Room: ", Style::default().fg(Color::Gray)),
        Span::styled(&app.nostr_room_id, Style::default().fg(Color::Cyan)),
        Span::raw("  |  "),
        Span::styled("You are: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("Party {}", app.nostr_my_index),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  |  "),
        Span::styled(
            format!("{}-of-{}", app.nostr_threshold, app.nostr_n_parties),
            Style::default().fg(Color::Yellow),
        ),
    ]));
    frame.render_widget(room_info, chunks[2]);

    // Participant list
    render_participant_list(frame, app, chunks[3]);

    // Help
    let help_lines = vec![
        Line::from(vec![Span::styled(
            "Waiting for all participants to join...",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(": Leave room"),
        ]),
    ];
    let help = Paragraph::new(help_lines).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(help, chunks[4]);
}

fn render_ready(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(3), // Progress bar (full)
            Constraint::Length(3), // Room info
            Constraint::Min(10),   // Participant list
            Constraint::Length(5), // Help
        ])
        .margin(1)
        .split(area);

    // Title
    let title = Paragraph::new(Line::from(vec![
        Span::styled("🌐 ", Style::default()),
        Span::styled(
            "Nostr Room - Ready!",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    // Progress bar (full)
    let joined = app.nostr_participants.len();
    let total = app.nostr_n_parties as usize;

    let gauge = Gauge::default()
        .block(Block::default())
        .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
        .percent(100)
        .label(format!("All {} participants ready!", total.max(joined)));
    frame.render_widget(gauge, chunks[1]);

    // Room info
    let room_info = Paragraph::new(Line::from(vec![
        Span::styled("Room: ", Style::default().fg(Color::Gray)),
        Span::styled(&app.nostr_room_id, Style::default().fg(Color::Cyan)),
        Span::raw("  |  "),
        Span::styled("You are: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("Party {}", app.nostr_my_index),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  |  "),
        Span::styled(
            format!("{}-of-{}", app.nostr_threshold, app.nostr_n_parties),
            Style::default().fg(Color::Yellow),
        ),
    ]));
    frame.render_widget(room_info, chunks[2]);

    // Participant list
    render_participant_list(frame, app, chunks[3]);

    // Help
    let help_lines = vec![
        Line::from(vec![
            Span::styled("K", Style::default().fg(Color::Cyan)),
            Span::raw(": Start Keygen  "),
            Span::styled("S", Style::default().fg(Color::Cyan)),
            Span::raw(": Start Signing  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(": Leave"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "All participants have joined - ready to start!",
            Style::default().fg(Color::Green),
        )),
    ];
    let help = Paragraph::new(help_lines).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(help, chunks[4]);
}

fn render_participant_list(frame: &mut Frame, app: &App, area: Rect) {
    let n = app.nostr_n_parties as usize;

    let items: Vec<ListItem> = (1..=n)
        .map(|idx| {
            let idx_u32 = idx as u32;
            let is_me = idx_u32 == app.nostr_my_index;
            let joined = app.nostr_participants.contains_key(&idx_u32);

            let (status_icon, status_color) = if joined {
                ("✓", Color::Green)
            } else {
                ("○", Color::DarkGray)
            };

            let name_style = if is_me {
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD)
            } else if joined {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let me_indicator = if is_me { " (you)" } else { "" };
            let pubkey_preview = app
                .nostr_participants
                .get(&idx_u32)
                .map(|pk| format!(" - {}...", &pk[..12.min(pk.len())]))
                .unwrap_or_default();

            ListItem::new(Line::from(vec![
                Span::styled(status_icon, Style::default().fg(status_color)),
                Span::raw(" "),
                Span::styled(format!("Party {}{}", idx, me_indicator), name_style),
                Span::styled(pubkey_preview, Style::default().fg(Color::DarkGray)),
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

fn render_text_field(frame: &mut Frame, area: Rect, label: &str, value: &str, focused: bool) {
    let style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Gray)
    };

    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let content = Paragraph::new(Line::from(vec![
        Span::styled(format!("{}: ", label), style),
        Span::styled(value, Style::default().fg(Color::White)),
        if focused {
            Span::styled("_", Style::default().fg(Color::Cyan))
        } else {
            Span::raw("")
        },
    ]))
    .block(
        Block::default()
            .borders(Borders::LEFT)
            .border_style(border_style),
    );

    frame.render_widget(content, area);
}
