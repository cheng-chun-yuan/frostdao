#!/bin/bash
# Compatibility alias. Prefer ./scripts/reshare-walkthrough.sh.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/reshare-walkthrough.sh" "$@"
