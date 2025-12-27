#!/bin/bash
# Clean build artifacts
# Usage: ./scripts/clean.sh [--all]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

echo "FrostDAO Clean"
echo "=============="

if [[ "$1" == "--all" ]]; then
    echo "Cleaning all artifacts (including state)..."
    cargo clean
    rm -rf .frost_state
    rm -rf frontend/pkg
    echo "Cleaned: target/, .frost_state/, frontend/pkg/"
else
    echo "Cleaning build artifacts..."
    cargo clean
    echo "Cleaned: target/"
    echo ""
    echo "Use --all to also remove .frost_state/ and frontend/pkg/"
fi
