#!/bin/bash

# ============================================================================
# FrostDAO Resharing Demo (Simplified)
# ============================================================================
#
# This demo creates a 2-of-3 wallet, then reshares to new configuration.
#
# ============================================================================

set -e

FROSTDAO="./target/release/frostdao"
BASE="reshare_demo"

echo "============================================================================"
echo "FrostDAO Resharing Demo"
echo "============================================================================"
echo ""

# Clean up
rm -rf .frost_state/${BASE}*

echo "Step 1: Create 2-of-3 DKG wallet"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Run keygen round 1 for all parties and capture outputs
mkdir -p /tmp/frost_demo
$FROSTDAO keygen-round1 --name ${BASE}_p1 --threshold 2 --n-parties 3 --my-index 1 > /tmp/frost_demo/p1_r1.txt 2>&1
$FROSTDAO keygen-round1 --name ${BASE}_p2 --threshold 2 --n-parties 3 --my-index 2 > /tmp/frost_demo/p2_r1.txt 2>&1
$FROSTDAO keygen-round1 --name ${BASE}_p3 --threshold 2 --n-parties 3 --my-index 3 > /tmp/frost_demo/p3_r1.txt 2>&1

echo "Round 1 complete for all 3 parties"

# Extract JSON from outputs (last line with curly braces)
P1_C=$(grep '{' /tmp/frost_demo/p1_r1.txt | tail -1)
P2_C=$(grep '{' /tmp/frost_demo/p2_r1.txt | tail -1)
P3_C=$(grep '{' /tmp/frost_demo/p3_r1.txt | tail -1)

COMMITMENTS="${P1_C} ${P2_C} ${P3_C}"

# Round 2
$FROSTDAO keygen-round2 --name ${BASE}_p1 --data "$COMMITMENTS" > /tmp/frost_demo/p1_r2.txt 2>&1
$FROSTDAO keygen-round2 --name ${BASE}_p2 --data "$COMMITMENTS" > /tmp/frost_demo/p2_r2.txt 2>&1
$FROSTDAO keygen-round2 --name ${BASE}_p3 --data "$COMMITMENTS" > /tmp/frost_demo/p3_r2.txt 2>&1

echo "Round 2 complete for all 3 parties"

P1_S=$(grep '{' /tmp/frost_demo/p1_r2.txt | tail -1)
P2_S=$(grep '{' /tmp/frost_demo/p2_r2.txt | tail -1)
P3_S=$(grep '{' /tmp/frost_demo/p3_r2.txt | tail -1)

SHARES="${P1_S} ${P2_S} ${P3_S}"

# Finalize
$FROSTDAO keygen-finalize --name ${BASE}_p1 --data "$SHARES" > /tmp/frost_demo/p1_final.txt 2>&1
$FROSTDAO keygen-finalize --name ${BASE}_p2 --data "$SHARES" > /tmp/frost_demo/p2_final.txt 2>&1
$FROSTDAO keygen-finalize --name ${BASE}_p3 --data "$SHARES" > /tmp/frost_demo/p3_final.txt 2>&1

echo "Finalize complete for all 3 parties"
echo ""

# Get original address
ORIGINAL_ADDR=$($FROSTDAO dkg-address --name ${BASE}_p1 2>&1 | grep "Address:" | awk '{print $2}')
echo "Original wallet address: $ORIGINAL_ADDR"
echo ""

echo "Step 2: Reshare to new 2-of-3 configuration"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Old parties generate sub-shares
$FROSTDAO reshare-round1 --source ${BASE}_p1 --new-threshold 2 --new-n-parties 3 --my-index 1 > /tmp/frost_demo/reshare_p1.txt 2>&1
$FROSTDAO reshare-round1 --source ${BASE}_p2 --new-threshold 2 --new-n-parties 3 --my-index 2 > /tmp/frost_demo/reshare_p2.txt 2>&1

echo "Reshare round 1 complete for parties 1 and 2"

R1=$(grep '{' /tmp/frost_demo/reshare_p1.txt | tail -1)
R2=$(grep '{' /tmp/frost_demo/reshare_p2.txt | tail -1)

RESHARE_DATA="${R1} ${R2}"

# New party 1 finalizes resharing
echo "y" | $FROSTDAO reshare-finalize --source ${BASE}_p1 --target ${BASE}_new_p1 --my-index 1 --data "$RESHARE_DATA" > /tmp/frost_demo/new_p1.txt 2>&1

echo "Reshare finalize complete for new party 1"
echo ""

# Get new address
NEW_ADDR=$($FROSTDAO dkg-address --name ${BASE}_new_p1 2>&1 | grep "Address:" | awk '{print $2}')
echo "Reshared wallet address: $NEW_ADDR"
echo ""

echo "============================================================================"
echo "VERIFICATION"
echo "============================================================================"
echo ""
echo "Original: $ORIGINAL_ADDR"
echo "Reshared: $NEW_ADDR"
echo ""

if [ "$ORIGINAL_ADDR" == "$NEW_ADDR" ]; then
    echo "SUCCESS! Addresses match!"
    echo ""
    echo "The resharing preserved the group public key."
    echo "New shares can sign transactions for the same Bitcoin address."
else
    echo "MISMATCH - Addresses are different"
fi

echo ""
echo "Cleanup: rm -rf .frost_state/${BASE}*"
