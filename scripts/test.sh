#!/bin/bash
# Run FrostDAO tests
# Usage: ./scripts/test.sh [test_name]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

echo "FrostDAO Tests"
echo "=============="

if [[ -n "$1" ]]; then
    echo "Running test: $1"
    cargo test "$1" -- --nocapture
else
    echo "Running all tests..."
    cargo test -- --nocapture
fi
