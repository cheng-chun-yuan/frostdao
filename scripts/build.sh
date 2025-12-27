#!/bin/bash
# Build the FrostDAO CLI
# Usage: ./scripts/build.sh [--release]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

echo "FrostDAO Build"
echo "=============="

if [[ "$1" == "--release" ]]; then
    echo "Building release version..."
    cargo build --release
    echo ""
    echo "Binary: target/release/frostdao"
else
    echo "Building debug version..."
    cargo build
    echo ""
    echo "Binary: target/debug/frostdao"
fi

echo "Build complete!"
