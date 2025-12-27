#!/bin/bash
# Run a quick DKG demo (2-of-3 threshold)
# Usage: ./scripts/demo.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

echo "FrostDAO Demo - 2-of-3 Threshold DKG"
echo "====================================="
echo ""

# Build first
cargo build --release 2>/dev/null || cargo build

FROSTDAO="./target/release/frostdao"
if [[ ! -f "$FROSTDAO" ]]; then
    FROSTDAO="./target/debug/frostdao"
fi

echo "Step 1: Running keygen-round1 for party 1..."
ROUND1_P1=$($FROSTDAO keygen-round1 --threshold 2 --n-parties 3 --my-index 1)
echo "Party 1 output saved"

echo "Step 2: Running keygen-round1 for party 2..."
ROUND1_P2=$($FROSTDAO keygen-round1 --threshold 2 --n-parties 3 --my-index 2)
echo "Party 2 output saved"

echo "Step 3: Running keygen-round1 for party 3..."
ROUND1_P3=$($FROSTDAO keygen-round1 --threshold 2 --n-parties 3 --my-index 3)
echo "Party 3 output saved"

echo ""
echo "All parties have generated round 1 commitments!"
echo "In a real scenario, parties would exchange these over a secure channel."
echo ""
echo "To continue the demo manually:"
echo "  1. Run keygen-round2 with combined commitments"
echo "  2. Run keygen-finalize with combined shares"
echo "  3. Use sign command to create threshold signatures"
