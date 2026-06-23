#!/usr/bin/env bash
# Run the opt-in Nostr relay integration smoke test.
# Usage:
#   ./scripts/nostr-relay-smoke.sh wss://relay.damus.io
#   FROSTDAO_TEST_NOSTR_RELAYS=ws://127.0.0.1:8080 ./scripts/nostr-relay-smoke.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

if [[ "$#" -gt 0 ]]; then
    relay_urls="$1"
    shift
    for relay_url in "$@"; do
        relay_urls="${relay_urls},${relay_url}"
    done
    export FROSTDAO_TEST_NOSTR_RELAYS="$relay_urls"
fi

if [[ -z "${FROSTDAO_TEST_NOSTR_RELAYS:-}" ]]; then
    echo "Set FROSTDAO_TEST_NOSTR_RELAYS or pass relay URLs as arguments." >&2
    echo "Example: ./scripts/nostr-relay-smoke.sh wss://relay.damus.io" >&2
    exit 2
fi

cargo test --all-features --test nostr_relay_smoke -- --ignored --nocapture
