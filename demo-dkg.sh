#!/bin/bash
# HTSS Demo - Phase 1: Distributed Key Generation (DKG)
# Run this first to generate keys for all parties

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
YUSHAN="$SCRIPT_DIR/target/release/yushan"
BASE="${HTSS_DATA_DIR:-/tmp/htss_demo}"
THRESHOLD="${THRESHOLD:-3}"
N_PARTIES="${N_PARTIES:-4}"

# Build if needed
if [ ! -f "$YUSHAN" ]; then
    echo "Building yushan..."
    cargo build --release --quiet 2>/dev/null
fi

echo ""
echo "╔════════════════════════════════════════════════════════════════════╗"
echo "║              HTSS DKG - Distributed Key Generation                 ║"
echo "╠════════════════════════════════════════════════════════════════════╣"
echo "║  Configuration:                                                    ║"
echo "║    Threshold: $THRESHOLD-of-$N_PARTIES                                               ║"
echo "║    Data Directory: $BASE"
echo "╚════════════════════════════════════════════════════════════════════╝"
echo ""

# Setup directories
rm -rf $BASE
mkdir -p $BASE/{ceo,cfo,coo,manager}

# Create party config file
cat > $BASE/config.json << EOF
{
  "threshold": $THRESHOLD,
  "n_parties": $N_PARTIES,
  "parties": {
    "ceo": {"index": 1, "rank": 0, "name": "CEO"},
    "cfo": {"index": 2, "rank": 1, "name": "CFO"},
    "coo": {"index": 3, "rank": 1, "name": "COO"},
    "manager": {"index": 4, "rank": 2, "name": "Manager"}
  }
}
EOF

echo "Party Configuration:"
echo "  ┌─────────┬───────┬──────┬─────────────────────────────────┐"
echo "  │ Party   │ Index │ Rank │ Data Folder                     │"
echo "  ├─────────┼───────┼──────┼─────────────────────────────────┤"
echo "  │ CEO     │ 1     │ 0    │ $BASE/ceo     │"
echo "  │ CFO     │ 2     │ 1    │ $BASE/cfo     │"
echo "  │ COO     │ 3     │ 1    │ $BASE/coo     │"
echo "  │ Manager │ 4     │ 2    │ $BASE/manager │"
echo "  └─────────┴───────┴──────┴─────────────────────────────────┘"
echo ""

#############################################
# ROUND 1
#############################################
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  ROUND 1: Generate Polynomial Commitments"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

cd $BASE/ceo
R1_CEO=$($YUSHAN keygen-round1 --threshold $THRESHOLD --n-parties $N_PARTIES --my-index 1 --rank 0 --hierarchical 2>&1 | grep '"party_index":1' | tail -1)
echo "$R1_CEO" > $BASE/ceo/round1_output.json
echo "  ✓ CEO: Generated commitments → $BASE/ceo/round1_output.json"

cd $BASE/cfo
R1_CFO=$($YUSHAN keygen-round1 --threshold $THRESHOLD --n-parties $N_PARTIES --my-index 2 --rank 1 --hierarchical 2>&1 | grep '"party_index":2' | tail -1)
echo "$R1_CFO" > $BASE/cfo/round1_output.json
echo "  ✓ CFO: Generated commitments → $BASE/cfo/round1_output.json"

cd $BASE/coo
R1_COO=$($YUSHAN keygen-round1 --threshold $THRESHOLD --n-parties $N_PARTIES --my-index 3 --rank 1 --hierarchical 2>&1 | grep '"party_index":3' | tail -1)
echo "$R1_COO" > $BASE/coo/round1_output.json
echo "  ✓ COO: Generated commitments → $BASE/coo/round1_output.json"

cd $BASE/manager
R1_MGR=$($YUSHAN keygen-round1 --threshold $THRESHOLD --n-parties $N_PARTIES --my-index 4 --rank 2 --hierarchical 2>&1 | grep '"party_index":4' | tail -1)
echo "$R1_MGR" > $BASE/manager/round1_output.json
echo "  ✓ Manager: Generated commitments → $BASE/manager/round1_output.json"

# Combine all round1 outputs
ALL_R1="$R1_CEO $R1_CFO $R1_COO $R1_MGR"
echo "$ALL_R1" > $BASE/all_round1.txt
echo ""
echo "  📁 All Round 1 data saved to: $BASE/all_round1.txt"
echo ""

#############################################
# ROUND 2
#############################################
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  ROUND 2: Exchange Secret Shares"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

cd $BASE/ceo
R2_CEO=$($YUSHAN keygen-round2 --data "$ALL_R1" 2>&1 | grep '"party_index":1' | tail -1)
echo "$R2_CEO" > $BASE/ceo/round2_output.json
echo "  ✓ CEO: Computed secret shares → $BASE/ceo/round2_output.json"

cd $BASE/cfo
R2_CFO=$($YUSHAN keygen-round2 --data "$ALL_R1" 2>&1 | grep '"party_index":2' | tail -1)
echo "$R2_CFO" > $BASE/cfo/round2_output.json
echo "  ✓ CFO: Computed secret shares → $BASE/cfo/round2_output.json"

cd $BASE/coo
R2_COO=$($YUSHAN keygen-round2 --data "$ALL_R1" 2>&1 | grep '"party_index":3' | tail -1)
echo "$R2_COO" > $BASE/coo/round2_output.json
echo "  ✓ COO: Computed secret shares → $BASE/coo/round2_output.json"

cd $BASE/manager
R2_MGR=$($YUSHAN keygen-round2 --data "$ALL_R1" 2>&1 | grep '"party_index":4' | tail -1)
echo "$R2_MGR" > $BASE/manager/round2_output.json
echo "  ✓ Manager: Computed secret shares → $BASE/manager/round2_output.json"

ALL_R2="$R2_CEO $R2_CFO $R2_COO $R2_MGR"
echo "$ALL_R2" > $BASE/all_round2.txt
echo ""
echo "  📁 All Round 2 data saved to: $BASE/all_round2.txt"
echo ""

#############################################
# FINALIZE
#############################################
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  FINALIZE: Compute Final Keys"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

cd $BASE/ceo && FINALIZE_CEO=$($YUSHAN keygen-finalize --data "$ALL_R2" 2>&1)
cd $BASE/cfo && FINALIZE_CFO=$($YUSHAN keygen-finalize --data "$ALL_R2" 2>&1)
cd $BASE/coo && FINALIZE_COO=$($YUSHAN keygen-finalize --data "$ALL_R2" 2>&1)
cd $BASE/manager && FINALIZE_MGR=$($YUSHAN keygen-finalize --data "$ALL_R2" 2>&1)

# Extract and save keys
PUBKEY=$(echo "$FINALIZE_CEO" | grep "Public Key:" | sed 's/.*Public Key: //')
SECRET_CEO=$(echo "$FINALIZE_CEO" | grep "Secret Share:" | sed 's/.*Secret Share: //')
SECRET_CFO=$(echo "$FINALIZE_CFO" | grep "Secret Share:" | sed 's/.*Secret Share: //')
SECRET_COO=$(echo "$FINALIZE_COO" | grep "Secret Share:" | sed 's/.*Secret Share: //')
SECRET_MGR=$(echo "$FINALIZE_MGR" | grep "Secret Share:" | sed 's/.*Secret Share: //')

# Save to files
echo "$PUBKEY" > $BASE/public_key.txt
echo "$SECRET_CEO" > $BASE/ceo/secret_share.txt
echo "$SECRET_CFO" > $BASE/cfo/secret_share.txt
echo "$SECRET_COO" > $BASE/coo/secret_share.txt
echo "$SECRET_MGR" > $BASE/manager/secret_share.txt

echo "  ✓ CEO: Secret share saved → $BASE/ceo/secret_share.txt"
echo "  ✓ CFO: Secret share saved → $BASE/cfo/secret_share.txt"
echo "  ✓ COO: Secret share saved → $BASE/coo/secret_share.txt"
echo "  ✓ Manager: Secret share saved → $BASE/manager/secret_share.txt"
echo ""

echo "╔════════════════════════════════════════════════════════════════════╗"
echo "║                      DKG COMPLETE!                                 ║"
echo "╠════════════════════════════════════════════════════════════════════╣"
echo "║  Shared Public Key:                                                ║"
echo "║  $PUBKEY"
echo "╠════════════════════════════════════════════════════════════════════╣"
echo "║  Data stored in: $BASE"
echo "║                                                                    ║"
echo "║  Folder Structure:                                                 ║"
echo "║  $BASE/"
echo "║  ├── config.json          # Party configuration                    ║"
echo "║  ├── public_key.txt       # Shared public key                      ║"
echo "║  ├── all_round1.txt       # Combined Round 1 data                  ║"
echo "║  ├── all_round2.txt       # Combined Round 2 data                  ║"
echo "║  ├── ceo/                                                          ║"
echo "║  │   ├── .frost_state/    # FROST internal state                   ║"
echo "║  │   ├── round1_output.json                                        ║"
echo "║  │   ├── round2_output.json                                        ║"
echo "║  │   └── secret_share.txt                                          ║"
echo "║  ├── cfo/                                                          ║"
echo "║  ├── coo/                                                          ║"
echo "║  └── manager/                                                      ║"
echo "╚════════════════════════════════════════════════════════════════════╝"
echo ""
echo "Next step: Run ./demo-sign.sh to sign with different signer combinations"
echo ""
