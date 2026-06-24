#!/bin/bash
# Compatibility alias. Prefer ./scripts/dkg-walkthrough.sh.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/dkg-walkthrough.sh" "$@"
