#!/bin/bash
# Serve the FrostDAO frontend
# Usage: ./scripts/serve-frontend.sh [port]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
FRONTEND_DIR="$PROJECT_ROOT/frontend"
PORT="${1:-8080}"

echo "FrostDAO Frontend Server"
echo "========================"
echo "Serving from: $FRONTEND_DIR"
echo "URL: http://localhost:$PORT"
echo ""
echo "Press Ctrl+C to stop"
echo ""

cd "$FRONTEND_DIR"

# Try different servers in order of preference
if command -v python3 &> /dev/null; then
    python3 -m http.server "$PORT"
elif command -v python &> /dev/null; then
    python -m SimpleHTTPServer "$PORT"
elif command -v npx &> /dev/null; then
    npx serve -l "$PORT"
elif command -v php &> /dev/null; then
    php -S "localhost:$PORT"
else
    echo "Error: No suitable HTTP server found."
    echo "Please install one of: python3, node/npx, or php"
    exit 1
fi
