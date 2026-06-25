//! Home screen - wallet list and details

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::tui::app::{
    balance_cache_key, missing_network_address_message, wallet_address_for_network, App,
};
use crate::tui::state::NetworkSelection;
use crate::tui::{COPY_KEY_LABEL, HOME_RELOAD_KEY_LABEL, REFRESH_KEY_LABEL};
use frostdao::protocol::keygen::WalletSummary;

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

            let has_balance = app
                .balance_cache
                .contains_key(&balance_cache_key(&wallet.name, app.network));
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
        lines.extend(wallet_readiness_lines(app, wallet));
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
                Span::styled(
                    missing_network_address_message(app.network),
                    Style::default().fg(Color::Red),
                ),
            ]));
        }

        lines.push(Line::from(""));

        // Balance (if cached)
        let cache_key = balance_cache_key(&wallet.name, app.network);
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
            lines.push(balance_fetch_hint_line(app));
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
            Span::styled("j/k/↑/↓", Style::default().fg(Color::Green)),
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
            Span::styled(REFRESH_KEY_LABEL, Style::default().fg(Color::Yellow)),
            Span::raw(" Refresh   "),
            Span::styled(HOME_RELOAD_KEY_LABEL, Style::default().fg(Color::Yellow)),
            Span::raw(" Reload list   "),
            Span::styled(COPY_KEY_LABEL, Style::default().fg(Color::Yellow)),
            Span::raw(" Copy addr"),
        ]));
        shortcuts.push(Line::from(Span::styled(
            format!("Press {REFRESH_KEY_LABEL} to fetch balances"),
            Style::default().fg(Color::Gray),
        )));
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
            format!("Policy: {}", network.policy_hint()),
            Style::default().fg(policy_color),
        )]),
        Line::from(vec![Span::styled(
            format!("Address scope: {}", network.address_scope_hint()),
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(vec![Span::styled(
            format!("UTXO API: {}", network.utxo_api_hint()),
            Style::default().fg(policy_color),
        )]),
    ]
}

fn balance_fetch_hint_line(app: &App) -> Line<'static> {
    Line::from(vec![
        Span::styled("Balance: ", Style::default().fg(Color::Gray)),
        Span::styled(
            app.balance_fetch_hint(),
            Style::default().fg(Color::DarkGray),
        ),
    ])
}

fn wallet_readiness_lines(app: &App, wallet: &WalletSummary) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![Span::styled(
        "Readiness:",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )])];

    let has_network_address = wallet_address_for_network(wallet, app.network).is_some();
    let send_line = match (has_network_address, app.utxo_source_unavailable_message()) {
        (false, _) => (
            format!(
                "Send: Blocked - no {} source address",
                app.network.display_name()
            ),
            Color::Red,
        ),
        (true, Some(message)) => (format!("Send: Blocked - {message}"), Color::Red),
        (true, None) => (
            "Send: Ready to fetch UTXOs - source address and API available".to_string(),
            Color::Green,
        ),
    };
    lines.push(status_line(send_line.0, send_line.1));

    lines.push(status_line(signing_readiness_text(wallet), Color::Cyan));

    let hd_text = if has_network_address {
        "HD: Addresses screen derives paths controlled by the same threshold key"
    } else {
        "HD: unavailable until this network has a source address"
    };
    lines.push(status_line(hd_text.to_string(), Color::Yellow));

    let (nostr_text, nostr_color) = nostr_readiness_status(app);
    lines.push(status_line(nostr_text.to_string(), nostr_color));
    lines.push(status_line(
        "Recovery: CLI-only; restores one lost party share".to_string(),
        Color::Cyan,
    ));

    lines
}

fn nostr_readiness_status(app: &App) -> (&'static str, Color) {
    if app.nostr_runtime.is_none() {
        return (
            "Nostr: configure and join a room before multi-device signing",
            Color::Yellow,
        );
    }

    if app.nostr_local_simulation_transport_active() {
        (
            "Nostr: local room active for rehearsal; relay signing is opt-in",
            Color::Magenta,
        )
    } else {
        (
            "Nostr: relay room active for signing; use CLI keygen first",
            Color::Magenta,
        )
    }
}

fn status_line(text: String, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled(text, Style::default().fg(color)),
    ])
}

fn signing_readiness_text(wallet: &WalletSummary) -> String {
    let mode = if wallet.hierarchical.unwrap_or(false) {
        "HTSS"
    } else {
        "TSS"
    };

    let threshold = match (wallet.threshold, wallet.total_parties) {
        (Some(t), Some(n)) => format!("{t}-of-{n}"),
        _ => "threshold metadata missing".to_string(),
    };

    if wallet.hierarchical.unwrap_or(false) {
        match &wallet.signing_requirement {
            Some(requirement) if !requirement.is_empty() => {
                let requirement = requirement
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                format!("Signing: {mode} {threshold}; rank requirement {requirement}")
            }
            _ => format!("Signing: {mode} {threshold}; rank requirement missing"),
        }
    } else {
        format!("Signing: {mode} {threshold}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::collections::BTreeMap;

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

    fn wallet_summary(
        address_testnet: Option<&str>,
        address_mainnet: Option<&str>,
    ) -> WalletSummary {
        WalletSummary {
            name: "treasury".to_string(),
            threshold: Some(2),
            total_parties: Some(3),
            hierarchical: Some(false),
            address: address_testnet.map(str::to_string),
            address_testnet: address_testnet.map(str::to_string),
            address_mainnet: address_mainnet.map(str::to_string),
            address_regtest: address_testnet
                .filter(|address| address.starts_with("bcrt"))
                .map(str::to_string),
            signing_requirement: None,
            party_ranks: Some(BTreeMap::new()),
        }
    }

    #[test]
    fn network_safety_lines_warn_for_mainnet() {
        let rendered = lines_to_string(network_safety_lines(NetworkSelection::Mainnet));

        assert!(rendered.contains("MAINNET real funds"));
        assert!(rendered.contains("explicit opt-in"));
        assert!(rendered.contains("Bitcoin mainnet root address"));
        assert!(rendered.contains("UTXO API: https://mempool.space/api"));
    }

    #[test]
    #[serial]
    fn network_safety_lines_explain_regtest_local_node_policy() {
        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);

        let rendered = lines_to_string(network_safety_lines(NetworkSelection::Regtest));

        assert!(rendered.contains("regtest uses local Esplora/mempool API"));
        assert!(rendered.contains("FROSTDAO_REGTEST_MEMPOOL_API"));
        assert!(rendered.contains("local regtest root address with bcrt prefix"));
        assert!(rendered.contains("UTXO API: regtest needs a local Esplora/mempool API endpoint"));

        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);
    }

    #[test]
    fn balance_fetch_hint_matches_home_refresh_shortcut() {
        let app = App::new().unwrap();
        let rendered = lines_to_string(vec![balance_fetch_hint_line(&app)]);

        assert!(rendered.contains(&format!("Press {REFRESH_KEY_LABEL} to fetch")));
        assert!(rendered.contains(REFRESH_KEY_LABEL));
        assert!(!rendered.contains("Press Enter to fetch"));
    }

    #[test]
    #[serial]
    fn balance_fetch_hint_for_regtest_includes_env_requirement() {
        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);

        let mut app = App::new().unwrap();
        app.network = crate::tui::state::NetworkSelection::Regtest;

        let rendered = lines_to_string(vec![balance_fetch_hint_line(&app)]);

        assert!(rendered.contains("local Esplora/mempool API"));
        assert!(rendered.contains(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV));

        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);
    }

    #[test]
    fn wallet_readiness_lines_show_testnet_send_and_nostr_setup_status() {
        let app = App::new().unwrap();
        let wallet = wallet_summary(Some("tb1qsource"), None);

        let rendered = lines_to_string(wallet_readiness_lines(&app, &wallet));

        assert!(rendered.contains("Readiness:"));
        assert!(rendered.contains("Send: Ready to fetch UTXOs"));
        assert!(rendered.contains("source address and API available"));
        assert!(rendered.contains("Signing: TSS 2-of-3"));
        assert!(rendered.contains("HD: Addresses screen derives paths"));
        assert!(rendered.contains("Nostr: configure and join a room"));
        assert!(rendered.contains("Recovery: CLI-only"));
        assert!(rendered.contains("restores one lost party share"));
    }

    #[test]
    fn nostr_readiness_status_distinguishes_active_local_room() {
        let mut app = App::new().unwrap();
        app.nostr_room_id = format!("home-local-room-{}", std::process::id());

        app.join_nostr_room_runtime_with_relays(Vec::new()).unwrap();

        let (text, color) = nostr_readiness_status(&app);
        assert_eq!(
            text,
            "Nostr: local room active for rehearsal; relay signing is opt-in"
        );
        assert_eq!(color, Color::Magenta);

        let _ = std::fs::remove_file(app.nostr_replay_cache_path());
    }

    #[test]
    fn nostr_readiness_status_distinguishes_active_relay_room() {
        let mut app = App::new().unwrap();
        app.nostr_room_id = format!("home-relay-room-{}", std::process::id());
        app.join_nostr_room_runtime_with_relays(Vec::new()).unwrap();
        app.force_relay_transport_for_tests = true;

        let (text, color) = nostr_readiness_status(&app);
        assert_eq!(
            text,
            "Nostr: relay room active for signing; use CLI keygen first"
        );
        assert_eq!(color, Color::Magenta);

        let _ = std::fs::remove_file(app.nostr_replay_cache_path());
    }

    #[test]
    fn wallet_readiness_lines_block_mainnet_without_mainnet_address() {
        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Mainnet;
        let wallet = wallet_summary(Some("tb1qsource"), None);

        let rendered = lines_to_string(wallet_readiness_lines(&app, &wallet));

        assert!(rendered.contains("Send: Blocked - no Mainnet source address"));
        assert!(rendered.contains("HD: unavailable until this network has a source address"));
    }

    #[test]
    #[serial]
    fn wallet_readiness_lines_block_regtest_without_utxo_api() {
        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);

        let mut app = App::new().unwrap();
        app.network = NetworkSelection::Regtest;
        let wallet = wallet_summary(Some("bcrt1qsource"), None);

        let rendered = lines_to_string(wallet_readiness_lines(&app, &wallet));

        assert!(rendered.contains("Send: Blocked"));
        assert!(rendered.contains(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV));

        std::env::remove_var(frostdao::btc::transaction::REGTEST_MEMPOOL_API_ENV);
    }

    #[test]
    fn signing_readiness_text_includes_htss_requirement() {
        let mut wallet = wallet_summary(Some("tb1qsource"), None);
        wallet.hierarchical = Some(true);
        wallet.threshold = Some(3);
        wallet.total_parties = Some(5);
        wallet.signing_requirement = Some(vec![1, 2]);

        assert_eq!(
            signing_readiness_text(&wallet),
            "Signing: HTSS 3-of-5; rank requirement 1,2"
        );
    }
}
