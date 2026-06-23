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
    for path in [*DOCS, *Path("src").rglob("*.rs"), *Path("scripts").glob("*.sh")]:
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
    source = read(Path("src/nostr/transport.rs"))
    docs = read(Path("docs/NOSTR_PROTOCOL.md"))
    required = [
        "pub struct FileReplayCache",
        "accept_and_save",
        "file_replay_cache_survives_restart",
    ]
    missing = [item for item in required if item not in source]
    if "FileReplayCache" not in docs:
        missing.append("NOSTR_PROTOCOL FileReplayCache documentation")

    if missing:
        doctor.fail(f"persisted replay cache markers missing: {', '.join(missing)}")
    else:
        doctor.ok("persisted replay cache primitive is present and documented")


def check_transaction_review():
    protocol = read(Path("src/protocol/dkg_tx.rs"))
    tui_state = read(Path("src/tui/state.rs"))
    tui_keys = read(Path("src/tui/mod.rs"))
    docs = read(Path("docs/RUN_GUIDE.md"))
    required = [
        ("src/protocol/dkg_tx.rs", protocol, "pub struct TransactionReview"),
        ("src/protocol/dkg_tx.rs", protocol, "sighash_fingerprint"),
        ("src/protocol/dkg_tx.rs", protocol, "fee_rate_sats_vb"),
        ("src/tui/state.rs", tui_state, "ReviewTransaction"),
        ("src/tui/mod.rs", tui_keys, "KeyCode::Char('y')"),
        ("docs/RUN_GUIDE.md", docs, "compare the returned `review` fields"),
    ]
    missing = [f"{path}: {marker}" for path, content, marker in required if marker not in content]

    if missing:
        doctor.fail(f"transaction review markers missing: {', '.join(missing)}")
    else:
        doctor.ok("transaction review fields and TUI confirmation are present")


def check_audit_logging():
    audit = read(Path("src/audit.rs"))
    protocol = read(Path("src/protocol/dkg_tx.rs"))
    security = read(Path("docs/SECURITY_MODEL.md"))
    required = [
        ("src/audit.rs", audit, "pub struct AuditEvent"),
        ("src/audit.rs", audit, "DEFAULT_AUDIT_LOG"),
        ("src/audit.rs", audit, "audit_event_appends_jsonl_without_secret_fields"),
        ("src/protocol/dkg_tx.rs", protocol, "AuditEvent::new(\"dkg_build_tx\""),
        ("src/protocol/dkg_tx.rs", protocol, "AuditEvent::new(\"dkg_broadcast\""),
        ("src/protocol/dkg_tx.rs", protocol, "AuditEvent::new(\"frost_auto_sign\""),
        ("docs/SECURITY_MODEL.md", security, "Audit events must not include mnemonics"),
        ("docs/SECURITY_MODEL.md", security, "FROSTDAO_AUDIT_LOG"),
    ]
    missing = [f"{path}: {marker}" for path, content, marker in required if marker not in content]

    forbidden = ["secret_share", "signature_share", "raw_tx", "mnemonic", "nonce"]
    audit_body = audit.replace("audit_event_appends_jsonl_without_secret_fields", "")
    leaked = [term for term in forbidden if f'with_field("{term}"' in protocol or f'pub {term}' in audit_body]

    if missing:
        doctor.fail(f"audit logging markers missing: {', '.join(missing)}")
    elif leaked:
        doctor.fail(f"audit logging appears to include secret markers: {', '.join(leaked)}")
    else:
        doctor.ok("structured audit logging is present and excludes secret markers")


def check_nostr_transaction_review():
    events = read(Path("src/nostr/events.rs"))
    tui = read(Path("src/tui/mod.rs"))
    screen = read(Path("src/tui/screens/nostr_sign.rs"))
    docs = read(Path("docs/NOSTR_PROTOCOL.md"))
    keymap = read(Path("docs/TUI_KEYMAP.md"))
    required = [
        ("src/nostr/events.rs", events, "pub struct TxReviewPayload"),
        ("src/nostr/events.rs", events, "reviewed_sighash_fingerprint"),
        ("src/nostr/events.rs", events, "tx_consent_carries_reviewed_fingerprint"),
        ("src/tui/mod.rs", tui, "press y to consent"),
        ("src/tui/screens/nostr_sign.rs", screen, "Sighash fingerprint: "),
        ("docs/NOSTR_PROTOCOL.md", docs, "`tx_proposal` must include a `review` object"),
        ("docs/TUI_KEYMAP.md", keymap, "Consent after reviewing proposal fingerprint"),
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
check_audit_logging()
check_nostr_transaction_review()

if doctor.failures:
    print("\nDoctor found issues:")
    for failure in doctor.failures:
        print(f"  - {failure}")
    sys.exit(1)

print("\nDoctor checks passed.")
PY
