#!/usr/bin/env bash
# Run FrostDAO repository coherence checks.
# Usage:
#   ./scripts/doctor.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

python3 - <<'PY'
from pathlib import Path
import os
import re
import subprocess
import sys

ROOT = Path(".")
DOCS = [Path("README.md"), *sorted(Path("docs").glob("*.md"))]
OLD_DOCS = {
    "docs/BITCOIN_GUIDE.md",
    "docs/CRYPTOGRAPHIC_ANALYSIS.md",
    "docs/DKG.md",
    "docs/HD_DERIVATION.md",
    "docs/HTSS.md",
    "docs/RECOVERY.md",
    "docs/RESHARE.md",
    "docs/TUI.md",
}
KNOWN_STALE_PATTERNS = [
    "frostdao dkg-round2",
    "Press F5",
    "F5:",
    "N:Nostr",
    "Simulate completion",
    "will be implemented",
    "a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456",
    "frontend/",
    "wasm-build",
    "serve-frontend",
    "--with-wasm",
    "wasm-pack",
    "LocalStorageImpl",
    "test_wasm",
    "wasm_bindgen",
    "webpage",
    "yushan sign",
    "yushan combine",
]


class Doctor:
    def __init__(self):
        self.failures = []

    def fail(self, message):
        self.failures.append(message)

    def ok(self, message):
        print(f"ok - {message}")

    def check(self, condition, message):
        if condition:
            self.ok(message)
        else:
            self.fail(message)


doctor = Doctor()


def read(path):
    return path.read_text(encoding="utf-8")


def command_output(args):
    result = subprocess.run(
        args,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    if result.returncode != 0:
        doctor.fail(f"command failed: {' '.join(args)}\n{result.stdout}")
    return result.stdout


def commands_from_help(help_text):
    commands = set()
    in_commands = False
    for line in help_text.splitlines():
        if line.strip() == "Commands:":
            in_commands = True
            continue
        if in_commands and not line.strip():
            break
        if in_commands:
            match = re.match(r"\s{2}([a-z0-9][a-z0-9-]*)\s", line)
            if match:
                commands.add(match.group(1))
    return commands


def documented_commands():
    commands = set()
    feature_commands = set()
    command_pattern = re.compile(
        r"(?:frostdao|cargo[ \t]+run[ \t]+--features[ \t]+miniscript-policy[ \t]+--)[ \t]+([a-z0-9][a-z0-9-]*)"
    )
    for path in DOCS:
        text = read(path)
        for match in command_pattern.finditer(text):
            command = match.group(1)
            if command in {"help"} or command.startswith("-"):
                continue
            if "<" in command:
                continue
            if "cargo run --features miniscript-policy --" in match.group(0):
                feature_commands.add(command)
            else:
                commands.add(command)
    return commands, feature_commands


def check_markdown_links():
    missing = []
    for path in DOCS:
        text = read(path)
        for match in re.finditer(r"\[[^\]]+\]\(([^)]+)\)", text):
            target = match.group(1).split("#", 1)[0]
            if not target or re.match(r"^[a-z][a-z0-9+.-]*:", target):
                continue
            resolved = (path.parent / target).resolve()
            if not resolved.exists():
                missing.append(f"{path}: missing link target {target}")
    if missing:
        for item in missing:
            doctor.fail(item)
    else:
        doctor.ok("markdown links resolve")


def check_old_docs_not_referenced():
    offenders = []
    for path in DOCS:
        text = read(path)
        for old in OLD_DOCS:
            if old in text or Path(old).name in text:
                offenders.append(f"{path}: references removed doc {old}")
    if offenders:
        for item in offenders:
            doctor.fail(item)
    else:
        doctor.ok("removed legacy docs are not referenced")


def check_stale_patterns():
    offenders = []
    for path in [Path("agent.md"), *DOCS, *Path("src").rglob("*.rs"), *Path("scripts").glob("*.sh")]:
        if path == Path("scripts/doctor.sh"):
            continue
        text = read(path)
        for pattern in KNOWN_STALE_PATTERNS:
            if pattern in text:
                offenders.append(f"{path}: contains stale pattern {pattern!r}")
    if offenders:
        for item in offenders:
            doctor.fail(item)
    else:
        doctor.ok("known stale command/keymap strings absent")


def check_docs_index():
    readme = read(Path("README.md"))
    showcase = read(Path("docs/SHOWCASE.md"))
    expected = [
        "docs/SHOWCASE.md",
        "docs/RUN_GUIDE.md",
        "docs/CLI.md",
        "docs/TUI_KEYMAP.md",
        "docs/BACKUP.md",
        "docs/PROTOCOLS.md",
        "docs/NOSTR_PROTOCOL.md",
        "docs/MINISCRIPT.md",
        "docs/SECURITY_MODEL.md",
        "docs/PRODUCTION_READINESS.md",
        "docs/USER_FLOW.md",
    ]
    for doc in expected:
        if doc not in readme:
            doctor.fail(f"README does not link {doc}")
        elif not Path(doc).exists():
            doctor.fail(f"README links missing doc {doc}")
    for doc in [Path(item).name for item in expected if item != "docs/SHOWCASE.md"]:
        if doc not in showcase:
            doctor.fail(f"SHOWCASE does not link {doc}")
    if not any("README does not link" in f or "SHOWCASE does not link" in f for f in doctor.failures):
        doctor.ok("documentation index is complete")


def check_duplicate_doc_titles():
    seen = {}
    for path in DOCS:
        for line in read(path).splitlines():
            if line.startswith("# "):
                title = line[2:].strip().lower()
                if title in seen:
                    doctor.fail(f"duplicate top-level doc title: {seen[title]} and {path}")
                else:
                    seen[title] = path
    if not any("duplicate top-level doc title" in f for f in doctor.failures):
        doctor.ok("top-level doc titles are unique")


def check_scripts_executable():
    missing_exec = []
    for path in Path("scripts").glob("*.sh"):
        if not os.access(path, os.X_OK):
            missing_exec.append(str(path))
    if missing_exec:
        doctor.fail(f"scripts are not executable: {', '.join(missing_exec)}")
    else:
        doctor.ok("scripts are executable")


def check_cli_docs_match_help():
    default_help = command_output(["cargo", "run", "--quiet", "--", "--help"])
    feature_help = command_output(
        ["cargo", "run", "--quiet", "--features", "miniscript-policy", "--", "--help"]
    )
    default_commands = commands_from_help(default_help)
    feature_commands = commands_from_help(feature_help)
    docs_default, docs_feature = documented_commands()

    missing_default = sorted(
        command for command in docs_default if command not in default_commands and command != "policy-compile"
    )
    missing_feature = sorted(command for command in docs_feature if command not in feature_commands)

    if "policy-compile" in default_commands:
        doctor.fail("policy-compile appears in default CLI help but should be feature-gated")
    if "policy-compile" not in feature_commands:
        doctor.fail("policy-compile missing from miniscript-policy CLI help")
    if missing_default:
        doctor.fail(f"documented default commands missing from CLI help: {', '.join(missing_default)}")
    if missing_feature:
        doctor.fail(f"documented feature commands missing from feature CLI help: {', '.join(missing_feature)}")

    if not any(
        phrase in f
        for f in doctor.failures
        for phrase in [
            "documented default commands",
            "documented feature commands",
            "policy-compile",
        ]
    ):
        doctor.ok("documented CLI commands match default and feature help")


def check_agent_payment_semantics():
    source = read(Path("src/tui/mod.rs"))
    required = [
        "Agent pubkey is required",
        "require_network(network)",
        "frost_key_path_address",
        "taproot_descriptor_preview",
        "descriptor_preview_only_script_path_spending_not_wired",
        "draft_amount_within_limit",
    ]
    missing = [item for item in required if item not in source]
    if missing:
        doctor.fail(f"agent payment draft semantics missing markers: {', '.join(missing)}")
    else:
        doctor.ok("agent payment draft distinguishes key-path spend and miniscript preview")


def check_replay_cache_persistence():
    client = read(Path("src/nostr/client.rs"))
    source = read(Path("src/nostr/transport.rs"))
    module = read(Path("src/nostr/mod.rs"))
    app = read(Path("src/tui/app.rs"))
    smoke_test = read(Path("tests/nostr_relay_smoke.rs"))
    smoke_script = read(Path("scripts/nostr-relay-smoke.sh"))
    docs = read(Path("docs/NOSTR_PROTOCOL.md"))
    run_guide = read(Path("docs/RUN_GUIDE.md"))
    production = read(Path("docs/PRODUCTION_READINESS.md"))
    keymap = read(Path("docs/TUI_KEYMAP.md"))
    required = [
        "pub struct FileReplayCache",
        "pub struct NostrRoomRuntime",
        "accept_and_save",
        "file_replay_cache_survives_restart",
        "room_runtime_enforces_room_sender_and_persists_replay_cache",
        "pub struct RelayRoomTransport",
        "relay_content_filter_accepts_only_valid_room_messages_once",
    ]
    missing = [item for item in required if item not in source]
    client_required = [
        "connect_with_relays",
        "create_room_client_with_relays",
        "explicit_relay_client_rejects_empty_relay_list",
    ]
    missing.extend([f"src/nostr/client.rs: {item}" for item in client_required if item not in client])
    if "RelayRoomTransport" not in module:
        missing.append("src/nostr/mod.rs: RelayRoomTransport export")
    smoke_required = [
        "relay_room_transport_round_trips_public_room_join",
        "#[ignore = \"requires reachable Nostr relay URLs in FROSTDAO_TEST_NOSTR_RELAYS\"]",
        "FROSTDAO_TEST_NOSTR_RELAYS",
    ]
    missing.extend([f"tests/nostr_relay_smoke.rs: {item}" for item in smoke_required if item not in smoke_test])
    script_required = [
        "FROSTDAO_TEST_NOSTR_RELAYS",
        "cargo test --all-features --test nostr_relay_smoke -- --ignored --nocapture",
    ]
    missing.extend([f"scripts/nostr-relay-smoke.sh: {item}" for item in script_required if item not in smoke_script])
    app_required = [
        "pub nostr_runtime",
        "pub enum TuiNostrRuntime",
        "join_nostr_room_runtime",
        "join_nostr_room_runtime_with_relays",
        "nostr_local_simulation_transport_active",
        "local simulation",
        "FROSTDAO_TUI_NOSTR_RELAYS",
        "FROSTDAO_ENABLE_MAINNET_NOSTR",
        "tui_nostr_relay_mode_is_guarded_on_mainnet",
        "accept_nostr_room_join",
        "tui_nostr_room_rejects_malformed_join_payloads",
        "accept_nostr_party_ciphertext",
        "tui_nostr_poll_rejects_mismatched_ciphertext_payload_parties",
        "nostr_replay_cache_path",
        "tui_nostr_room_uses_runtime_and_replay_cache",
    ]
    missing.extend([f"src/tui/app.rs: {item}" for item in app_required if item not in app])
    tui_room = read(Path("src/tui/screens/nostr_room.rs"))
    room_required = [
        "Transport: ",
        "local simulation",
        "Relay transport",
        "room_info_labels_local_simulation_transport",
        "local_waiting_help_uses_test_participant_wording",
        "Add local test participant",
    ]
    missing.extend([f"src/tui/screens/nostr_room.rs: {item}" for item in room_required if item not in tui_room])
    for marker in ["NostrRoomRuntime", "FileReplayCache", "RelayRoomTransport"]:
        if marker not in docs:
            missing.append(f"NOSTR_PROTOCOL {marker} documentation")
    if "TUI room screen creates a `NostrRoomRuntime`" not in run_guide:
        missing.append("RUN_GUIDE TUI NostrRoomRuntime documentation")
    if "Room joins are accepted only when the payload party, threshold, party count, scheme, and rank shape match the active room" not in run_guide:
        missing.append("RUN_GUIDE validated room join documentation")
    if "payload `party_index` must match the envelope `from` party" not in docs:
        missing.append("NOSTR_PROTOCOL room join party binding documentation")
    if "Encrypted signing nonce/share payloads must bind `party_index` to the envelope `from`" not in docs:
        missing.append("NOSTR_PROTOCOL ciphertext payload binding documentation")
    if "ciphertext payload party and recipient match the envelope sender and local party" not in run_guide:
        missing.append("RUN_GUIDE ciphertext payload binding documentation")
    if "relay transport adapter" not in run_guide:
        missing.append("RUN_GUIDE relay transport adapter documentation")
    if "local simulation transport" not in run_guide:
        missing.append("RUN_GUIDE local simulation transport documentation")
    if "local simulation mode" not in keymap:
        missing.append("TUI_KEYMAP local simulation mode documentation")
    if "Add a local test participant" not in keymap:
        missing.append("TUI_KEYMAP local test participant wording")
    if "FROSTDAO_TUI_NOSTR_RELAYS" not in run_guide:
        missing.append("RUN_GUIDE relay opt-in env documentation")
    if "FROSTDAO_ENABLE_MAINNET_NOSTR=1" not in run_guide:
        missing.append("RUN_GUIDE mainnet relay guard documentation")
    if "./scripts/nostr-relay-smoke.sh ws://127.0.0.1:8080" not in production:
        missing.append("PRODUCTION_READINESS local relay smoke command")
    if "./scripts/nostr-relay-smoke.sh wss://relay.damus.io" not in production:
        missing.append("PRODUCTION_READINESS public relay smoke command")

    if missing:
        doctor.fail(f"persisted replay cache markers missing: {', '.join(missing)}")
    else:
        doctor.ok("persisted replay cache runtime is present and documented")


def check_transaction_review():
    protocol = read(Path("src/protocol/dkg_tx.rs"))
    transaction = read(Path("src/btc/transaction.rs"))
    tui_state = read(Path("src/tui/state.rs"))
    tui_keys = read(Path("src/tui/mod.rs"))
    tui_chain_select = read(Path("src/tui/screens/chain_select.rs"))
    main_rs = read(Path("src/main.rs"))
    docs = read(Path("docs/RUN_GUIDE.md"))
    protocols_doc = read(Path("docs/PROTOCOLS.md"))
    user_flow_doc = read(Path("docs/USER_FLOW.md"))
    required = [
        ("src/protocol/dkg_tx.rs", protocol, "pub struct TransactionReview"),
        ("src/protocol/dkg_tx.rs", protocol, "sighash_fingerprint"),
        ("src/protocol/dkg_tx.rs", protocol, "fee_rate_sats_vb"),
        ("src/crypto/hd.rs", read(Path("src/crypto/hd.rs")), "bip86_coin_type"),
        ("src/crypto/hd.rs", read(Path("src/crypto/hd.rs")), "to_full_string_for_network"),
        ("src/protocol/dkg_tx.rs", protocol, "mempool_explorer_tx_url(network, txid)"),
        ("src/btc/transaction.rs", transaction, "pub(crate) fn mempool_explorer_tx_url"),
        ("src/btc/transaction.rs", transaction, "mempool.space/testnet4/tx"),
        ("src/btc/transaction.rs", transaction, "mempool_explorer_url_matches_network"),
        ("src/btc/transaction.rs", transaction, "REGTEST_MEMPOOL_API_ENV"),
        ("src/btc/transaction.rs", transaction, "mempool_api_base_url"),
        ("src/btc/transaction.rs", transaction, "regtest needs a local Esplora/mempool API endpoint"),
        ("src/tui/state.rs", tui_state, "Regtest"),
        ("src/tui/state.rs", tui_state, "policy_hint"),
        ("src/tui/state.rs", tui_state, "address_scope_hint"),
        ("src/tui/screens/chain_select.rs", tui_chain_select, "local API"),
        ("src/tui/screens/chain_select.rs", tui_chain_select, "Selected policy:"),
        ("src/tui/screens/chain_select.rs", tui_chain_select, "Confirming clears pending send form data"),
        ("src/tui/state.rs", tui_state, "ReviewTransaction"),
        ("src/tui/state.rs", tui_state, "broadcast_status"),
        ("src/tui/screens/send.rs", read(Path("src/tui/screens/send.rs")), "utxo_fetch_error"),
        ("src/tui/mod.rs", tui_keys, "No confirmed UTXOs available"),
        ("src/tui/mod.rs", tui_keys, "Insufficient confirmed balance"),
        ("src/tui/screens/send.rs", read(Path("src/tui/screens/send.rs")), "copy the raw transaction"),
        ("src/tui/mod.rs", tui_keys, "KeyCode::Char('y')"),
        ("src/tui/mod.rs", tui_keys, "parsed[\"broadcast_status\"]"),
        ("src/tui/screens/send.rs", read(Path("src/tui/screens/send.rs")), "review_source_path"),
        ("src/tui/screens/send.rs", read(Path("src/tui/screens/send.rs")), "Source address:"),
        ("src/tui/screens/send.rs", read(Path("src/tui/screens/send.rs")), "MPC threshold shares sign this derived path with the HD tweak"),
        ("src/tui/screens/send.rs", read(Path("src/tui/screens/send.rs")), "same MPC threshold shares"),
        ("src/tui/screens/send.rs", read(Path("src/tui/screens/send.rs")), "child x-only pubkey"),
        ("src/tui/screens/send.rs", read(Path("src/tui/screens/send.rs")), "review_control_proof"),
        ("src/tui/screens/send.rs", read(Path("src/tui/screens/send.rs")), "local_review_guard_requires_cross_device_comparison"),
        ("src/tui/screens/send.rs", read(Path("src/tui/screens/send.rs")), "Only press y when every signer sees the same review"),
        ("src/tui/screens/address_list.rs", read(Path("src/tui/screens/address_list.rs")), "format_bip86_path"),
        ("src/tui/screens/address_list.rs", read(Path("src/tui/screens/address_list.rs")), "MPC threshold shares"),
        ("src/tui/app.rs", read(Path("src/tui/app.rs")), "wallet_address_for_network"),
        ("src/tui/app.rs", read(Path("src/tui/app.rs")), "does_not_fake_mainnet"),
        ("src/tui/screens/home.rs", read(Path("src/tui/screens/home.rs")), "MAINNET real funds"),
        ("src/tui/screens/home.rs", read(Path("src/tui/screens/home.rs")), "regtest uses local Esplora/mempool API"),
        ("src/tui/screens/home.rs", read(Path("src/tui/screens/home.rs")), "Press r to fetch"),
        ("src/tui/screens/wallet_details.rs", read(Path("src/tui/screens/wallet_details.rs")), "Press v for QR code"),
        ("src/tui/app.rs", read(Path("src/tui/app.rs")), "balance_cache_key"),
        ("src/tui/app.rs", read(Path("src/tui/app.rs")), "clear_network_volatile_state"),
        ("src/tui/app.rs", read(Path("src/tui/app.rs")), "network_switch_clears_volatile_send_and_nostr_state"),
        ("src/tui/mod.rs", tui_keys, "r:Balance"),
        ("docs/RUN_GUIDE.md", docs, "must not fake a mainnet address"),
        ("docs/RUN_GUIDE.md", docs, "Home balance status is network-scoped"),
        ("docs/RUN_GUIDE.md", docs, "Confirming a new TUI network clears pending send form data"),
        ("docs/RUN_GUIDE.md", docs, "`r:Balance`"),
        ("docs/TUI_KEYMAP.md", read(Path("docs/TUI_KEYMAP.md")), "The selector shows the selected network policy"),
        ("docs/PROTOCOLS.md", protocols_doc, "Each party applies the same public tweak"),
        ("docs/PROTOCOLS.md", protocols_doc, "child x-only pubkey fingerprint"),
        ("docs/USER_FLOW.md", user_flow_doc, "same MPC threshold signer set"),
        ("docs/USER_FLOW.md", user_flow_doc, "child x-only pubkey fingerprint"),
        ("src/main.rs", main_rs, "parse_dkg_network"),
        ("src/main.rs", main_rs, "\"testnet4\" | \"test4\""),
        ("src/main.rs", main_rs, "\"regtest\" | \"local\""),
        ("src/main.rs", main_rs, "FROSTDAO_ENABLE_MAINNET_BITCOIN"),
        ("src/main.rs", main_rs, "mainnet DKG transaction commands require"),
        ("docs/RUN_GUIDE.md", docs, "compare the returned `review` fields"),
        ("docs/TUI_KEYMAP.md", read(Path("docs/TUI_KEYMAP.md")), "every signer sees the same review"),
        ("docs/RUN_GUIDE.md", docs, "`regtest`, `local`"),
        ("docs/RUN_GUIDE.md", docs, "FROSTDAO_REGTEST_MEMPOOL_API"),
        ("docs/PROTOCOLS.md", protocols_doc, "FROSTDAO_REGTEST_MEMPOOL_API"),
        ("docs/RUN_GUIDE.md", docs, "confirmed UTXOs and enough confirmed balance"),
        ("docs/RUN_GUIDE.md", docs, "FROSTDAO_ENABLE_MAINNET_BITCOIN=1"),
    ]
    missing = [f"{path}: {marker}" for path, content, marker in required if marker not in content]

    if missing:
        doctor.fail(f"transaction review markers missing: {', '.join(missing)}")
    else:
        doctor.ok("transaction review fields and TUI confirmation are present")


def check_mnemonic_backup_ux():
    screen = read(Path("src/tui/screens/mnemonic.rs"))
    keymap = read(Path("docs/TUI_KEYMAP.md"))
    security = read(Path("docs/SECURITY_MODEL.md"))
    required = [
        ("src/tui/mod.rs", read(Path("src/tui/mod.rs")), "mnemonic_help_bar_matches_party_selection_stage"),
        ("src/tui/mod.rs", read(Path("src/tui/mod.rs")), "↑/↓:Select party | Enter:Continue | Esc:Cancel"),
        ("src/tui/screens/mnemonic.rs", screen, "No clipboard or copy shortcut"),
        ("src/tui/screens/mnemonic.rs", screen, "No copy shortcut is provided"),
        ("src/tui/screens/mnemonic.rs", screen, "YOUR SECRET SHARE"),
        ("src/tui/screens/mnemonic.rs", screen, "NOT the full group key"),
        ("src/tui/screens/mnemonic.rs", screen, "mnemonic_warning_explains_share_scope_and_no_clipboard"),
        ("docs/TUI_KEYMAP.md", keymap, "does not provide a mnemonic copy shortcut"),
        ("docs/SECURITY_MODEL.md", security, "equivalent to that party's local secret share"),
    ]
    missing = [f"{path}: {marker}" for path, content, marker in required if marker not in content]

    if missing:
        doctor.fail(f"mnemonic backup UX markers missing: {', '.join(missing)}")
    else:
        doctor.ok("mnemonic backup UX keeps share scope and clipboard risk visible")


def check_wallet_delete_ux():
    screen = read(Path("src/tui/screens/wallet_details.rs"))
    state = read(Path("src/tui/state.rs"))
    tui = read(Path("src/tui/mod.rs"))
    keymap = read(Path("docs/TUI_KEYMAP.md"))
    required = [
        ("src/tui/state.rs", state, "delete_confirmation_input"),
        ("src/tui/screens/wallet_details.rs", screen, "delete_confirmation_requires_typed_wallet_name"),
        ("src/tui/screens/wallet_details.rs", screen, "Type treasury exactly"),
        ("src/tui/screens/wallet_details.rs", screen, "Y = Yes"),
        ("src/tui/mod.rs", tui, "wallet_delete_confirmation_does_not_accept_single_y"),
        ("src/tui/mod.rs", tui, "Type the wallet name exactly to delete"),
        ("docs/TUI_KEYMAP.md", keymap, "Type wallet name"),
        ("docs/TUI_KEYMAP.md", keymap, "Delete only after the typed name matches"),
    ]
    missing = [f"{path}: {marker}" for path, content, marker in required if marker not in content]

    if missing:
        doctor.fail(f"wallet delete UX markers missing: {', '.join(missing)}")
    else:
        doctor.ok("wallet delete UX requires typed wallet-name confirmation")


def check_audit_logging():
    audit = read(Path("src/audit.rs"))
    protocol = read(Path("src/protocol/dkg_tx.rs"))
    app = read(Path("src/tui/app.rs"))
    run_guide = read(Path("docs/RUN_GUIDE.md"))
    security = read(Path("docs/SECURITY_MODEL.md"))
    required = [
        ("src/audit.rs", audit, "pub struct AuditEvent"),
        ("src/audit.rs", audit, "DEFAULT_AUDIT_LOG"),
        ("src/audit.rs", audit, "audit_event_appends_jsonl_without_secret_fields"),
        ("src/protocol/dkg_tx.rs", protocol, "AuditEvent::new(\"dkg_build_tx\""),
        ("src/protocol/dkg_tx.rs", protocol, "AuditEvent::new(\"dkg_broadcast\""),
        ("src/protocol/dkg_tx.rs", protocol, "AuditEvent::new(\"frost_auto_sign\""),
        ("src/tui/app.rs", app, "AuditEvent::new(\"nostr_tx_proposal\""),
        ("src/tui/app.rs", app, "nostr_tx_consent"),
        ("src/tui/app.rs", app, "nostr_signing_nonce"),
        ("src/tui/app.rs", app, "nostr_signing_share"),
        ("src/tui/app.rs", app, "nostr_tx_broadcast"),
        ("src/tui/app.rs", app, "append_nostr_audit_event"),
        ("src/tui/app.rs", app, "fields.get(\"sighash\").is_none()"),
        ("src/tui/app.rs", app, "fields.get(\"ciphertext\").is_none()"),
        ("src/tui/app.rs", app, "fields.get(\"raw_tx\").is_none()"),
        ("docs/RUN_GUIDE.md", run_guide, "TUI Nostr proposal/consent flows append metadata-only audit events"),
        ("docs/SECURITY_MODEL.md", security, "Audit events must not include mnemonics"),
        ("docs/SECURITY_MODEL.md", security, "FROSTDAO_AUDIT_LOG"),
    ]
    missing = [f"{path}: {marker}" for path, content, marker in required if marker not in content]

    forbidden = ["secret_share", "signature_share", "raw_tx", "mnemonic", "nonce", "ciphertext"]
    audit_body = audit.replace("audit_event_appends_jsonl_without_secret_fields", "")
    leaked = [
        term
        for term in forbidden
        if f'with_field("{term}"' in protocol
        or f'with_field("{term}"' in app
        or f'pub {term}' in audit_body
    ]

    if missing:
        doctor.fail(f"audit logging markers missing: {', '.join(missing)}")
    elif leaked:
        doctor.fail(f"audit logging appears to include secret markers: {', '.join(leaked)}")
    else:
        doctor.ok("structured audit logging is present and excludes secret markers")


def check_nostr_transaction_review():
    events = read(Path("src/nostr/events.rs"))
    app = read(Path("src/tui/app.rs"))
    tui = read(Path("src/tui/mod.rs"))
    tui_state = read(Path("src/tui/state.rs"))
    screen = read(Path("src/tui/screens/nostr_sign.rs"))
    coordinator = read(Path("src/protocol/signing_coordinator.rs"))
    run_guide = read(Path("docs/RUN_GUIDE.md"))
    docs = read(Path("docs/NOSTR_PROTOCOL.md"))
    keymap = read(Path("docs/TUI_KEYMAP.md"))
    required = [
        ("src/nostr/events.rs", events, "pub struct TxReviewPayload"),
        ("src/nostr/events.rs", events, "reviewed_sighash_fingerprint"),
        ("src/nostr/events.rs", events, "tx_consent_carries_reviewed_fingerprint"),
        ("src/nostr/events.rs", events, "encrypted_signing_nonce_matches_envelope"),
        ("src/nostr/events.rs", events, "encrypted_signing_share_matches_envelope"),
        ("src/nostr/events.rs", events, "pub struct SigningNoncePlaintext"),
        ("src/nostr/events.rs", events, "pub struct SigningSharePlaintext"),
        ("src/nostr/events.rs", events, "signing_plaintexts_reject_mismatched_envelope_context"),
        ("src/nostr/signing.rs", read(Path("src/nostr/signing.rs")), "pub fn encrypt_signing_nonce_plaintext"),
        ("src/nostr/signing.rs", read(Path("src/nostr/signing.rs")), "pub fn decrypt_signing_share_plaintext"),
        ("src/nostr/signing.rs", read(Path("src/nostr/signing.rs")), "signing_plaintext_helpers_reject_wrong_key_and_context"),
        ("src/nostr/signing.rs", read(Path("src/nostr/signing.rs")), "signing_plaintexts_convert_to_protocol_inputs"),
        ("src/protocol/mod.rs", read(Path("src/protocol/mod.rs")), "SigningCoordinatorStatus"),
        ("src/protocol/signing_coordinator.rs", coordinator, "pub struct SigningAttemptCollector"),
        ("src/protocol/signing_coordinator.rs", coordinator, "pub struct SigningCoordinator"),
        ("src/protocol/signing_coordinator.rs", coordinator, "pub enum SigningSchemePolicy"),
        ("src/protocol/signing_coordinator.rs", coordinator, "pub struct SigningNonceInput"),
        ("src/protocol/signing_coordinator.rs", coordinator, "cannot accept signature shares before nonce threshold is met"),
        ("src/protocol/signing_coordinator.rs", coordinator, "signature share received before nonce for party"),
        ("src/protocol/signing_coordinator.rs", coordinator, "signing_attempt_collector_rejects_wrong_attempt_context"),
        ("src/protocol/signing_coordinator.rs", coordinator, "signing_coordinator_tracks_state_until_ready_to_combine"),
        ("src/protocol/signing_coordinator.rs", coordinator, "signing_attempt_config_validates_htss_rank_rule"),
        ("src/nostr/events.rs", events, "rejects_signing_events_with_mismatched_envelope_identity"),
        ("src/nostr/events.rs", events, "encrypted_dkg_round2_matches_envelope"),
        ("src/nostr/events.rs", events, "rejects_dkg_events_with_mismatched_envelope_identity"),
        ("src/nostr/events.rs", events, "encrypted_reshare_subshare_matches_envelope"),
        ("src/nostr/events.rs", events, "encrypted_recovery_subshare_matches_envelope"),
        ("src/nostr/events.rs", events, "rejects_reshare_events_with_mismatched_envelope_identity"),
        ("src/nostr/events.rs", events, "rejects_recovery_events_with_mismatched_envelope_identity"),
        ("src/tui/app.rs", app, "publish_nostr_tx_proposal"),
        ("src/tui/app.rs", app, "publish_nostr_tx_consent"),
        ("src/tui/app.rs", app, "publish_nostr_signing_nonce"),
        ("src/tui/app.rs", app, "publish_nostr_signing_share"),
        ("src/tui/app.rs", app, "publish_nostr_tx_broadcast"),
        ("src/tui/app.rs", app, "nostr_pending_proposals"),
        ("src/tui/app.rs", app, "nostr_received_nonces"),
        ("src/tui/app.rs", app, "nostr_received_shares"),
        ("src/tui/app.rs", app, "nostr_signing_coordinators"),
        ("src/tui/app.rs", app, "start_nostr_signing_attempt"),
        ("src/tui/app.rs", app, "nostr_broadcasts"),
        ("src/tui/app.rs", app, "nonempty_message_session"),
        ("src/tui/app.rs", app, "nonempty_message_wallet"),
        ("src/tui/state.rs", tui_state, "wallet_name"),
        ("src/tui/app.rs", app, "tui_nostr_poll_rejects_sessionless_signing_messages"),
        ("src/tui/app.rs", app, "tui_nostr_signing_publishes_runtime_messages"),
        ("src/tui/app.rs", app, "tui_nostr_poll_ingests_proposals_and_consents"),
        ("src/tui/app.rs", app, "tui_nostr_poll_tracks_rejected_proposal_consents"),
        ("src/tui/app.rs", app, "tui_nostr_signing_attempt_replays_prior_session_nonces"),
        ("src/tui/app.rs", app, "early-share"),
        ("src/tui/mod.rs", tui, "ready_to_combine"),
        ("src/tui/mod.rs", tui, "press y to consent"),
        ("src/tui/mod.rs", tui, "Rejection sent for proposal fingerprint"),
        ("src/tui/mod.rs", tui, "nostr_sign_help_text(&app.nostr_sign_state)"),
        ("src/tui/mod.rs", tui, "nostr_review_reject_key_publishes_rejection"),
        ("src/tui/screens/nostr_sign.rs", screen, "Blocked:"),
        ("src/tui/screens/nostr_sign.rs", screen, "✗ Rejected"),
        ("src/tui/screens/nostr_sign.rs", screen, "Consent Checklist"),
        ("src/tui/screens/nostr_sign.rs", screen, "pub(crate) fn nostr_sign_help_text"),
        ("src/tui/screens/nostr_sign.rs", screen, "review_checklist_contains_required_consent_fields"),
        ("src/tui/mod.rs", tui, "Proposal published through room runtime"),
        ("src/tui/mod.rs", tui, "No pending proposals received"),
        ("src/tui/screens/nostr_sign.rs", screen, "Sighash fingerprint: "),
        ("src/tui/screens/nostr_sign.rs", screen, "nostr_pending_proposals"),
        ("src/tui/screens/nostr_sign.rs", screen, "Coordinator Progress"),
        ("src/tui/screens/nostr_sign.rs", screen, "signing_progress_counts_prefers_coordinator_state"),
        ("docs/RUN_GUIDE.md", run_guide, "proposal carries a real unsigned transaction session"),
        ("docs/RUN_GUIDE.md", run_guide, "Runtime polling also ingests incoming `tx_proposal` messages"),
        ("docs/RUN_GUIDE.md", run_guide, "Session-scoped signing messages without an explicit envelope `session` are ignored"),
        ("docs/RUN_GUIDE.md", run_guide, "pending proposals are filtered by their envelope `wallet` before display"),
        ("docs/RUN_GUIDE.md", run_guide, "Encrypted `signing_nonce_encrypted` and `signing_share_encrypted` messages"),
        ("docs/RUN_GUIDE.md", run_guide, "share ciphertext is ignored until nonce ciphertext"),
        ("docs/RUN_GUIDE.md", run_guide, "replays already accepted session nonces"),
        ("docs/RUN_GUIDE.md", run_guide, "party-by-party nonce/share progress table"),
        ("docs/RUN_GUIDE.md", run_guide, "Press `r` to publish an explicit rejection"),
        ("docs/RUN_GUIDE.md", run_guide, "the consent checklist shows network/session"),
        ("docs/RUN_GUIDE.md", run_guide, "editable recipient and amount fields"),
        ("docs/RUN_GUIDE.md", run_guide, "`Tab` moves between fields"),
        ("docs/RUN_GUIDE.md", run_guide, "Regtest proposals use the same transaction builder"),
        ("docs/RUN_GUIDE.md", run_guide, "FROSTDAO_REGTEST_MEMPOOL_API"),
        ("docs/RUN_GUIDE.md", run_guide, "selected an HD-derived source address"),
        ("docs/RUN_GUIDE.md", run_guide, "source path, source address, BIP341 sighash"),
        ("docs/RUN_GUIDE.md", run_guide, "approved, rejected, and pending parties"),
        ("docs/RUN_GUIDE.md", run_guide, "approvals and rejections tracked separately"),
        ("docs/RUN_GUIDE.md", run_guide, "NIP-44 signing plaintext helpers encrypt typed nonce/share payloads"),
        ("docs/RUN_GUIDE.md", run_guide, "`tx_broadcast` announcements are also published and ingested through the runtime"),
        ("src/protocol/dkg_tx.rs", read(Path("src/protocol/dkg_tx.rs")), "build_unsigned_tx_core_with_source_path"),
        ("src/tui/app.rs", read(Path("src/tui/app.rs")), "nostr_source_derivation_path"),
        ("src/tui/app.rs", read(Path("src/tui/app.rs")), "ensure_nostr_proposal_network_available"),
        ("src/tui/app.rs", read(Path("src/tui/app.rs")), "Nostr transaction proposals need a UTXO API"),
        ("src/tui/app.rs", read(Path("src/tui/app.rs")), "nostr_to_address_input"),
        ("src/tui/app.rs", read(Path("src/tui/app.rs")), "nostr_amount_input"),
        ("src/tui/screens/nostr_sign.rs", read(Path("src/tui/screens/nostr_sign.rs")), "nostr_configure_source_summary"),
        ("src/tui/screens/nostr_sign.rs", read(Path("src/tui/screens/nostr_sign.rs")), "render_configure_tx"),
        ("src/tui/screens/nostr_sign.rs", read(Path("src/tui/screens/nostr_sign.rs")), "nostr_configure_network_lines"),
        ("src/tui/screens/nostr_sign.rs", read(Path("src/tui/screens/nostr_sign.rs")), "FROSTDAO_REGTEST_MEMPOOL_API"),
        ("src/tui/screens/nostr_sign.rs", read(Path("src/tui/screens/nostr_sign.rs")), "MPC threshold shares sign this derived path with the HD tweak"),
        ("docs/NOSTR_PROTOCOL.md", docs, "`tx_proposal` must include a `review` object"),
        ("docs/NOSTR_PROTOCOL.md", docs, "Session-scoped signing messages must carry a non-empty envelope `session`"),
        ("docs/NOSTR_PROTOCOL.md", docs, "Use `encrypt_signing_nonce_plaintext` and `encrypt_signing_share_plaintext`"),
        ("docs/NOSTR_PROTOCOL.md", docs, "Inbound share ciphertexts are ignored until a nonce ciphertext"),
        ("docs/NOSTR_PROTOCOL.md", docs, "starts a session-keyed `protocol::SigningCoordinator`"),
        ("docs/NOSTR_PROTOCOL.md", docs, "`protocol::SigningCoordinator` tracks one active signing attempt"),
        ("docs/NOSTR_PROTOCOL.md", docs, "bind `to_index` to the direct envelope `to`"),
        ("docs/NOSTR_PROTOCOL.md", docs, "Pending proposals are labeled by wallet"),
        ("docs/NOSTR_PROTOCOL.md", docs, "`keygen_round1.party_index` and `keygen_round2_encrypted.party_index` must also match envelope `from`"),
        ("docs/NOSTR_PROTOCOL.md", docs, "Reshare relay envelopes must bind payload identity to transport identity"),
        ("docs/NOSTR_PROTOCOL.md", docs, "Recovery relay envelopes follow the same identity binding"),
        ("docs/RUN_GUIDE.md", run_guide, "Reshare and recovery relay parser helpers reject versioned messages"),
        ("docs/TUI_KEYMAP.md", keymap, "Consent after reviewing proposal fingerprint"),
        ("docs/TUI_KEYMAP.md", keymap, "Move recipient/amount field"),
        ("docs/TUI_KEYMAP.md", keymap, "Clear focused recipient or amount field"),
        ("docs/TUI_KEYMAP.md", keymap, "HD-derived source"),
        ("docs/TUI_KEYMAP.md", keymap, "Regtest Nostr proposals require `FROSTDAO_REGTEST_MEMPOOL_API`"),
        ("docs/TUI_KEYMAP.md", keymap, "Publish a proposal rejection"),
    ]
    missing = [f"{path}: {marker}" for path, content, marker in required if marker not in content]

    if missing:
        doctor.fail(f"Nostr transaction review markers missing: {', '.join(missing)}")
    else:
        doctor.ok("Nostr proposal consent carries review metadata")


print("FrostDAO Doctor")
print("================")
check_markdown_links()
check_old_docs_not_referenced()
check_stale_patterns()
check_docs_index()
check_duplicate_doc_titles()
check_scripts_executable()
check_cli_docs_match_help()
check_agent_payment_semantics()
check_replay_cache_persistence()
check_transaction_review()
check_mnemonic_backup_ux()
check_wallet_delete_ux()
check_audit_logging()
check_nostr_transaction_review()

if doctor.failures:
    print("\nDoctor found issues:")
    for failure in doctor.failures:
        print(f"  - {failure}")
    sys.exit(1)

print("\nDoctor checks passed.")
PY
