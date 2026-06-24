//! Home screen - wallet list and details

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::tui::app::{wallet_address_for_network, App};
use crate::tui::state::NetworkSelection;

/// Render the home screen
pub fn render_home(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    render_wallet_list(frame, app, chunks[0]);
    render_wallet_details(frame, app, chunks[1]);
}

fn render_wallet_list(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .wallets
        .iter()
        .map(|wallet| {
            let mode = match wallet.hierarchical {
                Some(true) => "HTSS",
                Some(false) => "TSS",
                None => "?",
            };
            let threshold = match (wallet.threshold, wallet.total_parties) {
                (Some(t), Some(n)) => {
                    // Show signing requirement for HTSS if available
                    if wallet.hierarchical.unwrap_or(false) {
                        if let Some(ref req) = wallet.signing_requirement {
                            let req_str: Vec<String> = req.iter().map(|r| r.to_string()).collect();
                            format!("{}-of-{} ({})", t, n, req_str.join(","))
                        } else {
                            format!("{}-of-{}", t, n)
                        }
                    } else {
                        format!("{}-of-{}", t, n)
                    }
                }
                _ => "?".to_string(),
            };

            let has_balance = app.balance_cache.contains_key(&wallet.name);
            let balance_indicator = if has_balance { " $" } else { "" };

            ListItem::new(format!(
                "{} ({} {}){}",
                wallet.name, threshold, mode, balance_indicator
            ))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Wallets"))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    frame.render_stateful_widget(list, area, &mut app.wallet_list_state.clone());
}

fn render_wallet_details(frame: &mut Frame, app: &App, area: Rect) {
    // Split into wallet info (top) and keyboard shortcuts (bottom)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(12)])
        .split(area);

    // Wallet details
    let content = if let Some(wallet) = app.selected_wallet() {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Name: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    &wallet.name,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
        ];

        // Threshold info with signing requirement for HTSS
        if let (Some(t), Some(n)) = (wallet.threshold, wallet.total_parties) {
            let threshold_display = if wallet.hierarchical.unwrap_or(false) {
                if let Some(ref req) = wallet.signing_requirement {
                    let req_str: Vec<String> = req.iter().map(|r| r.to_string()).collect();
                    format!("{}-of-{} ({})", t, n, req_str.join(","))
                } else {
                    format!("{}-of-{}", t, n)
                }
            } else {
                format!("{}-of-{}", t, n)
            };

            lines.push(Line::from(vec![
                Span::styled("Threshold: ", Style::default().fg(Color::Gray)),
                Span::styled(threshold_display, Style::default().fg(Color::Yellow)),
            ]));
        }

        // Mode
        if let Some(h) = wallet.hierarchical {
            lines.push(Line::from(vec![
                Span::styled("Mode: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    if h {
                        "Hierarchical (HTSS)"
                    } else {
                        "Standard (TSS)"
                    },
                    Style::default().fg(Color::Cyan),
                ),
            ]));
        }

        lines.push(Line::from(""));
        lines.extend(network_safety_lines(app.network));
        lines.push(Line::from(""));

        // Address (network-specific)
        if let Some(addr) = wallet_address_for_network(wallet, app.network) {
            lines.push(Line::from(vec![Span::styled(
                format!("Address ({}): ", app.network.display_name()),
                Style::default().fg(Color::Gray),
            )]));
            lines.push(Line::from(vec![Span::styled(
                addr,
                Style::default().fg(Color::Green),
            )]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("Address ({}): ", app.network.display_name()),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled("not available", Style::default().fg(Color::Red)),
            ]));
        }

        lines.push(Line::from(""));

        // Balance (if cached)
        let cache_key = format!("{}:{:?}", wallet.name, app.network);
        if let Some(info) = app.balance_cache.get(&cache_key) {
            lines.push(Line::from(vec![
                Span::styled("Balance: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{} sats", info.balance_sats),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));

            let btc = info.balance_sats as f64 / 100_000_000.0;
            lines.push(Line::from(vec![
                Span::styled("         ", Style::default()),
                Span::styled(
                    format!("({:.8} BTC)", btc),
                    Style::default().fg(Color::Gray),
                ),
            ]));

            lines.push(Line::from(vec![
                Span::styled("UTXOs: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{}", info.utxo_count),
                    Style::default().fg(Color::White),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("Balance: ", Style::default().fg(Color::Gray)),
                Span::styled("Press Enter to fetch", Style::default().fg(Color::DarkGray)),
            ]));
        }

        lines
    } else {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "No wallet selected",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from("Create a wallet with 'g' (keygen)"),
            Line::from("or use CLI:"),
            Line::from(Span::styled(
                "  frostdao keygen-round1 --name <name> ...",
                Style::default().fg(Color::Cyan),
            )),
        ]
    };

    let details = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title("Details"))
        .wrap(Wrap { trim: false });

    frame.render_widget(details, chunks[0]);

    // Keyboard shortcuts panel (pass whether wallet is selected)
    let has_wallet = app.selected_wallet().is_some();
    render_shortcuts(frame, has_wallet, chunks[1]);
}

fn render_shortcuts(frame: &mut Frame, has_wallet: bool, area: Rect) {
    // Basic shortcuts always shown
    let mut shortcuts = vec![
        Line::from(vec![
            Span::styled("g", Style::default().fg(Color::Yellow)),
            Span::raw(" Local Keygen  "),
            Span::styled("o", Style::default().fg(Color::Magenta)),
            Span::raw(" Nostr Room  "),
            Span::styled("n", Style::default().fg(Color::Yellow)),
            Span::raw(" Network"),
        ]),
        Line::from(vec![
            Span::styled("↑/↓", Style::default().fg(Color::Green)),
            Span::raw(" Navigate   "),
            Span::styled("Enter", Style::default().fg(Color::Green)),
            Span::raw(" Open wallet   "),
            Span::styled("q", Style::default().fg(Color::Yellow)),
            Span::raw(" Quit"),
        ]),
    ];

    #[cfg(feature = "miniscript-policy")]
    shortcuts.insert(
        1,
        Line::from(vec![
            Span::styled("p", Style::default().fg(Color::Yellow)),
            Span::raw(" Policy Preview"),
        ]),
    );

    // Wallet-specific shortcuts only shown when a wallet is selected
    if has_wallet {
        shortcuts.push(Line::from(""));
        shortcuts.push(Line::from(Span::styled(
            "Wallet actions:",
            Style::default().fg(Color::Cyan),
        )));
        shortcuts.push(Line::from(vec![
            Span::styled("s", Style::default().fg(Color::Yellow)),
            Span::raw(" Send      "),
            Span::styled("a", Style::default().fg(Color::Yellow)),
            Span::raw(" Addresses   "),
            Span::styled("m", Style::default().fg(Color::Yellow)),
            Span::raw(" Mnemonic"),
        ]));
        shortcuts.push(Line::from(vec![
            Span::styled("h", Style::default().fg(Color::Yellow)),
            Span::raw(" Reshare   "),
            Span::styled("r", Style::default().fg(Color::Yellow)),
            Span::raw(" Refresh     "),
            Span::styled("c", Style::default().fg(Color::Yellow)),
            Span::raw(" Copy addr"),
        ]));
    }

    let shortcuts_widget = Paragraph::new(shortcuts)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Shortcuts ")
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .style(Style::default().fg(Color::White));

    frame.render_widget(shortcuts_widget, area);
}

fn network_safety_lines(network: NetworkSelection) -> Vec<Line<'static>> {
    let policy = match network {
        NetworkSelection::Testnet4 => "Policy: testnet4 remote UTXOs via mempool.space",
        NetworkSelection::Testnet3 => "Policy: testnet3 remote UTXOs via mempool.space",
        NetworkSelection::Signet => "Policy: signet remote UTXOs via mempool.space",
        NetworkSelection::Regtest => "Policy: regtest uses local-node workflow; no mempool.space",
        NetworkSelection::Mainnet => {
            "Policy: MAINNET real funds; guarded commands require explicit opt-in"
        }
    };
    let address_scope = match network {
        NetworkSelection::Mainnet => "Address scope: Bitcoin mainnet root address",
        NetworkSelection::Testnet4
        | NetworkSelection::Testnet3
        | NetworkSelection::Signet
        | NetworkSelection::Regtest => {
            "Address scope: test-chain root address for testnet/signet/regtest"
        }
    };
    let policy_color = match network {
        NetworkSelection::Regtest => Color::Cyan,
        NetworkSelection::Mainnet => Color::Red,
        _ => Color::Yellow,
    };

    vec![
        Line::from(vec![
            Span::styled("Network: ", Style::default().fg(Color::Gray)),
            Span::styled(
                network.display_name(),
                Style::default()
                    .fg(policy_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![Span::styled(
            policy,
            Style::default().fg(policy_color),
        )]),
        Line::from(vec![Span::styled(
            address_scope,
            Style::default().fg(Color::DarkGray),
        )]),
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
    fn network_safety_lines_warn_for_mainnet() {
        let rendered = lines_to_string(network_safety_lines(NetworkSelection::Mainnet));

        assert!(rendered.contains("MAINNET real funds"));
        assert!(rendered.contains("explicit opt-in"));
        assert!(rendered.contains("Bitcoin mainnet root address"));
    }

    #[test]
    fn network_safety_lines_explain_regtest_local_node_policy() {
        let rendered = lines_to_string(network_safety_lines(NetworkSelection::Regtest));

        assert!(rendered.contains("regtest uses local-node workflow"));
        assert!(rendered.contains("no mempool.space"));
        assert!(rendered.contains("test-chain root address"));
    }
}
