//! Mnemonic backup screen

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::tui::state::MnemonicState;

/// Render the mnemonic backup screen
pub fn render_mnemonic(frame: &mut Frame, state: &MnemonicState, area: Rect) {
    let title = if state.party_selected {
        let party_idx = state
            .available_parties
            .get(state.selected_party)
            .copied()
            .unwrap_or(1);
        if party_idx == 0 {
            format!("Share Backup - {}", state.wallet_name)
        } else if state.hierarchical {
            let rank = state.party_ranks.get(&party_idx).copied().unwrap_or(0);
            format!(
                "Share Backup - {} (Party {} r{})",
                state.wallet_name, party_idx, rank
            )
        } else {
            format!("Share Backup - {} (Party {})", state.wallet_name, party_idx)
        }
    } else {
        format!("Share Backup - {} - Select Party", state.wallet_name)
    };

    let block = Block::default().title(title).borders(Borders::ALL);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Check for error
    if let Some(ref error) = state.error {
        let error_para = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                error.as_str(),
                Style::default().fg(Color::Red),
            )),
        ])
        .block(Block::default());
        frame.render_widget(error_para, inner);
        return;
    }

    // Party selection screen
    if !state.party_selected {
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Select which party's share to backup:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        for (i, party_idx) in state.available_parties.iter().enumerate() {
            let is_selected = i == state.selected_party;
            let prefix = if is_selected { "▶ " } else { "  " };
            let style = if is_selected {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let label = if *party_idx == 0 {
                format!("{}Your Share (Legacy Wallet)", prefix)
            } else if state.hierarchical {
                let rank = state.party_ranks.get(party_idx).copied().unwrap_or(0);
                format!("{}Party {} (r{}) - Secret Share", prefix, party_idx, rank)
            } else {
                format!("{}Party {} - Secret Share", prefix, party_idx)
            };
            lines.push(Line::from(Span::styled(label, style)));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "j/k/↑/↓: Select | Enter: Continue | Esc: Cancel",
            Style::default().fg(Color::DarkGray),
        )));

        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        frame.render_widget(para, inner);
        return;
    }

    if !state.revealed {
        let warning = Paragraph::new(mnemonic_warning_lines()).wrap(Wrap { trim: false });
        frame.render_widget(warning, inner);
    } else {
        let mut lines = mnemonic_reveal_header_lines();

        for row in 0..6 {
            let mut spans = Vec::new();
            for col in 0..4 {
                let idx = row + col * 6;
                if idx < state.words.len() {
                    let word = &state.words[idx];
                    spans.push(Span::styled(
                        format!("{:>2}. ", idx + 1),
                        Style::default().fg(Color::Gray),
                    ));
                    spans.push(Span::styled(
                        format!("{:<12} ", word),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
            }
            lines.push(Line::from(spans));
        }

        lines.extend(mnemonic_reveal_footer_lines());

        let mnemonic_display = Paragraph::new(lines).wrap(Wrap { trim: false });
        frame.render_widget(mnemonic_display, inner);
    }
}

fn mnemonic_warning_lines() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from(Span::styled(
            "⚠️  SECURITY WARNING",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("You are about to reveal your 24-word backup phrase."),
        Line::from(""),
        Line::from(Span::styled(
            "This phrase can be used to restore YOUR SECRET SHARE.",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Important: ", Style::default().fg(Color::Red)),
            Span::styled(
                "This backs up YOUR share only, NOT the full group key.",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from("Recovery still requires threshold cooperation."),
        Line::from(""),
        Line::from(Span::styled(
            "• Write down these words on paper",
            Style::default().fg(Color::Gray),
        )),
        Line::from(Span::styled(
            "• Store in a secure location",
            Style::default().fg(Color::Gray),
        )),
        Line::from(Span::styled(
            "• Never share with anyone",
            Style::default().fg(Color::Gray),
        )),
        Line::from(Span::styled(
            "• Never store digitally",
            Style::default().fg(Color::Gray),
        )),
        Line::from(Span::styled(
            "• No clipboard or copy shortcut is available for backup words",
            Style::default().fg(Color::Gray),
        )),
        Line::from(Span::styled(
            "• Keep the public backup manifest with your inventory",
            Style::default().fg(Color::Gray),
        )),
        Line::from(Span::styled(
            "• Verify the written words before relying on this backup",
            Style::default().fg(Color::Gray),
        )),
        Line::from(Span::styled(
            "  frostdao dkg-verify-mnemonic --name <wallet> --words '<24 words>'",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "Press Enter to reveal your backup phrase",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
    ]
}

fn mnemonic_reveal_header_lines() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from(Span::styled(
            "Your 24-Word Backup Phrase",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Write these words down and store securely:",
            Style::default().fg(Color::Gray),
        )),
        Line::from(Span::styled(
            "No copy shortcut is provided; keep these words offline.",
            Style::default().fg(Color::Gray),
        )),
        Line::from(""),
    ]
}

fn mnemonic_reveal_footer_lines() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from(Span::styled(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Remember: ", Style::default().fg(Color::Red)),
            Span::styled(
                "This backs up YOUR share only. Recovery requires ",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(Span::styled(
            "threshold parties to reconstruct the group signing key.",
            Style::default().fg(Color::White),
        )),
        Line::from(Span::styled(
            "Before relying on this backup, verify the written words with dkg-verify-mnemonic.",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press Enter when done",
            Style::default().fg(Color::Green),
        )),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn mnemonic_warning_explains_share_scope_and_no_clipboard() {
        let rendered = lines_to_string(mnemonic_warning_lines());

        assert!(rendered.contains("YOUR SECRET SHARE"));
        assert!(rendered.contains("NOT the full group key"));
        assert!(rendered.contains("threshold cooperation"));
        assert!(rendered.contains("No clipboard or copy shortcut"));
        assert!(rendered.contains("public backup manifest"));
        assert!(rendered.contains("dkg-verify-mnemonic"));
    }

    #[test]
    fn mnemonic_reveal_reminds_user_no_copy_shortcut() {
        let mut lines = mnemonic_reveal_header_lines();
        lines.extend(mnemonic_reveal_footer_lines());
        let rendered = lines_to_string(lines);

        assert!(rendered.contains("No copy shortcut"));
        assert!(rendered.contains("keep these words offline"));
        assert!(rendered.contains("YOUR share only"));
        assert!(rendered.contains("threshold parties"));
        assert!(rendered.contains("Before relying on this backup"));
        assert!(rendered.contains("dkg-verify-mnemonic"));
    }
}
