#!/bin/bash
# Run a local resharing walkthrough.
# Usage: ./scripts/reshare-walkthrough.sh

set -e

FROSTDAO="./target/release/frostdao"
BASE="reshare_walkthrough"
WORK_DIR="/tmp/frostdao_reshare_walkthrough"

echo "============================================================================"
echo "FrostDAO Resharing Walkthrough"
echo "============================================================================"
echo ""

if [[ ! -x "$FROSTDAO" ]]; then
    cargo build --release 2>/dev/null || cargo build
fi

if [[ ! -x "$FROSTDAO" ]]; then
    FROSTDAO="./target/debug/frostdao"
fi

rm -rf ".frost_state/${BASE}"*
rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"

echo "Step 1: Create 2-of-3 DKG wallet"
echo "-----------------------------------------------------------------------"
echo ""

"$FROSTDAO" keygen-round1 --name "${BASE}_p1" --threshold 2 --n-parties 3 --my-index 1 > "$WORK_DIR/p1_r1.txt" 2>&1
"$FROSTDAO" keygen-round1 --name "${BASE}_p2" --threshold 2 --n-parties 3 --my-index 2 > "$WORK_DIR/p2_r1.txt" 2>&1
"$FROSTDAO" keygen-round1 --name "${BASE}_p3" --threshold 2 --n-parties 3 --my-index 3 > "$WORK_DIR/p3_r1.txt" 2>&1

echo "Round 1 complete for all 3 parties"

P1_C=$(grep '{' "$WORK_DIR/p1_r1.txt" | tail -1)
P2_C=$(grep '{' "$WORK_DIR/p2_r1.txt" | tail -1)
P3_C=$(grep '{' "$WORK_DIR/p3_r1.txt" | tail -1)
COMMITMENTS="${P1_C} ${P2_C} ${P3_C}"

"$FROSTDAO" keygen-round2 --name "${BASE}_p1" --data "$COMMITMENTS" > "$WORK_DIR/p1_r2.txt" 2>&1
"$FROSTDAO" keygen-round2 --name "${BASE}_p2" --data "$COMMITMENTS" > "$WORK_DIR/p2_r2.txt" 2>&1
"$FROSTDAO" keygen-round2 --name "${BASE}_p3" --data "$COMMITMENTS" > "$WORK_DIR/p3_r2.txt" 2>&1

echo "Round 2 complete for all 3 parties"

P1_S=$(grep '{' "$WORK_DIR/p1_r2.txt" | tail -1)
P2_S=$(grep '{' "$WORK_DIR/p2_r2.txt" | tail -1)
P3_S=$(grep '{' "$WORK_DIR/p3_r2.txt" | tail -1)
SHARES="${P1_S} ${P2_S} ${P3_S}"

"$FROSTDAO" keygen-finalize --name "${BASE}_p1" --data "$SHARES" > "$WORK_DIR/p1_final.txt" 2>&1
"$FROSTDAO" keygen-finalize --name "${BASE}_p2" --data "$SHARES" > "$WORK_DIR/p2_final.txt" 2>&1
"$FROSTDAO" keygen-finalize --name "${BASE}_p3" --data "$SHARES" > "$WORK_DIR/p3_final.txt" 2>&1

echo "Finalize complete for all 3 parties"
echo ""

ORIGINAL_ADDR=$("$FROSTDAO" dkg-address --name "${BASE}_p1" 2>&1 | grep "Address:" | awk '{print $2}')
echo "Original wallet address: $ORIGINAL_ADDR"
echo ""

echo "Step 2: Reshare to a new 2-of-3 configuration"
echo "-----------------------------------------------------------------------"
echo ""

"$FROSTDAO" reshare-round1 --source "${BASE}_p1" --new-threshold 2 --new-n-parties 3 --my-index 1 > "$WORK_DIR/reshare_p1.txt" 2>&1
"$FROSTDAO" reshare-round1 --source "${BASE}_p2" --new-threshold 2 --new-n-parties 3 --my-index 2 > "$WORK_DIR/reshare_p2.txt" 2>&1

echo "Reshare round 1 complete for parties 1 and 2"

R1=$(grep '{' "$WORK_DIR/reshare_p1.txt" | tail -1)
R2=$(grep '{' "$WORK_DIR/reshare_p2.txt" | tail -1)
RESHARE_DATA="${R1} ${R2}"

echo "y" | "$FROSTDAO" reshare-finalize --source "${BASE}_p1" --target "${BASE}_new_p1" --my-index 1 --data "$RESHARE_DATA" > "$WORK_DIR/new_p1.txt" 2>&1

echo "Reshare finalize complete for new party 1"
echo ""

NEW_ADDR=$("$FROSTDAO" dkg-address --name "${BASE}_new_p1" 2>&1 | grep "Address:" | awk '{print $2}')
echo "Reshared wallet address: $NEW_ADDR"
echo ""

echo "============================================================================"
echo "Verification"
echo "============================================================================"
echo ""
echo "Original: $ORIGINAL_ADDR"
echo "Reshared: $NEW_ADDR"
echo ""

if [[ "$ORIGINAL_ADDR" == "$NEW_ADDR" ]]; then
    echo "Success: resharing preserved the group public key."
    echo "New shares can sign transactions for the same Bitcoin address."
else
    echo "Mismatch: addresses are different."
    exit 1
fi

echo ""
echo "Cleanup: rm -rf .frost_state/${BASE}* $WORK_DIR"
