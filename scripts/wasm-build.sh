#!/bin/bash
# Build FrostDAO WASM module for browser
# Usage: ./scripts/wasm-build.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
FRONTEND_DIR="$PROJECT_ROOT/frontend"

cd "$PROJECT_ROOT"

echo "FrostDAO WASM Build"
echo "==================="

# Check for wasm-pack
if ! command -v wasm-pack &> /dev/null; then
    echo "Installing wasm-pack..."
    cargo install wasm-pack
fi

echo "Building WASM module..."
wasm-pack build --target web --out-dir "$FRONTEND_DIR/pkg"

echo ""
echo "WASM module built to: $FRONTEND_DIR/pkg/"
echo "Include in HTML with:"
echo '  <script type="module">'
echo '    import init, { ... } from "./pkg/frostdao.js";'
echo '    await init();'
echo '  </script>'
