#!/bin/bash
# HTSS Demo - Phase 1: Distributed Key Generation (DKG)
# Run this first to generate keys for all parties

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
YUSHAN="$SCRIPT_DIR/target/release/yushan"
BASE="${HTSS_DATA_DIR:-/tmp/htss_demo}"
THRESHOLD="${THRESHOLD:-3}"
N_PARTIES="${N_PARTIES:-10}"

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
mkdir -p $BASE/{ceo,cfo,coo,cto,manager1,manager2,manager3,intern1,intern2,intern3}

# Create party config file
cat > $BASE/config.json << EOF
{
  "threshold": $THRESHOLD,
  "n_parties": $N_PARTIES,
  "parties": {
    "ceo": {"index": 1, "rank": 0, "name": "CEO"},
    "cfo": {"index": 2, "rank": 1, "name": "CFO"},
    "coo": {"index": 3, "rank": 1, "name": "COO"},
    "cto": {"index": 4, "rank": 1, "name": "CTO"},
    "manager1": {"index": 5, "rank": 2, "name": "Manager1"},
    "manager2": {"index": 6, "rank": 2, "name": "Manager2"},
    "manager3": {"index": 7, "rank": 2, "name": "Manager3"},
    "intern1": {"index": 8, "rank": 2, "name": "Intern1"},
    "intern2": {"index": 9, "rank": 2, "name": "Intern2"},
    "intern3": {"index": 10, "rank": 2, "name": "Intern3"}
  }
}
EOF

echo "Party Configuration:"
echo "  ┌──────────┬───────┬──────┬─────────────────────────────────┐"
echo "  │ Party    │ Index │ Rank │ Description                     │"
echo "  ├──────────┼───────┼──────┼─────────────────────────────────┤"
echo "  │ CEO      │ 1     │ 0    │ Chief Executive (Top Level)     │"
echo "  ├──────────┼───────┼──────┼─────────────────────────────────┤"
echo "  │ CFO      │ 2     │ 1    │ Chief Financial Officer         │"
echo "  │ COO      │ 3     │ 1    │ Chief Operating Officer         │"
echo "  │ CTO      │ 4     │ 1    │ Chief Technology Officer        │"
echo "  ├──────────┼───────┼──────┼─────────────────────────────────┤"
echo "  │ Manager1 │ 5     │ 2    │ Department Manager #1           │"
echo "  │ Manager2 │ 6     │ 2    │ Department Manager #2           │"
echo "  │ Manager3 │ 7     │ 2    │ Department Manager #3           │"
echo "  │ Intern1  │ 8     │ 2    │ Intern #1                       │"
echo "  │ Intern2  │ 9     │ 2    │ Intern #2                       │"
echo "  │ Intern3  │ 10    │ 2    │ Intern #3                       │"
echo "  └──────────┴───────┴──────┴─────────────────────────────────┘"
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
echo "  ✓ CEO: Generated commitments (rank 0)"

cd $BASE/cfo
R1_CFO=$($YUSHAN keygen-round1 --threshold $THRESHOLD --n-parties $N_PARTIES --my-index 2 --rank 1 --hierarchical 2>&1 | grep '"party_index":2' | tail -1)
echo "$R1_CFO" > $BASE/cfo/round1_output.json
echo "  ✓ CFO: Generated commitments (rank 1)"

cd $BASE/coo
R1_COO=$($YUSHAN keygen-round1 --threshold $THRESHOLD --n-parties $N_PARTIES --my-index 3 --rank 1 --hierarchical 2>&1 | grep '"party_index":3' | tail -1)
echo "$R1_COO" > $BASE/coo/round1_output.json
echo "  ✓ COO: Generated commitments (rank 1)"

cd $BASE/cto
R1_CTO=$($YUSHAN keygen-round1 --threshold $THRESHOLD --n-parties $N_PARTIES --my-index 4 --rank 1 --hierarchical 2>&1 | grep '"party_index":4' | tail -1)
echo "$R1_CTO" > $BASE/cto/round1_output.json
echo "  ✓ CTO: Generated commitments (rank 1)"

cd $BASE/manager1
R1_MGR1=$($YUSHAN keygen-round1 --threshold $THRESHOLD --n-parties $N_PARTIES --my-index 5 --rank 2 --hierarchical 2>&1 | grep '"party_index":5' | tail -1)
echo "$R1_MGR1" > $BASE/manager1/round1_output.json
echo "  ✓ Manager1: Generated commitments (rank 2)"

cd $BASE/manager2
R1_MGR2=$($YUSHAN keygen-round1 --threshold $THRESHOLD --n-parties $N_PARTIES --my-index 6 --rank 2 --hierarchical 2>&1 | grep '"party_index":6' | tail -1)
echo "$R1_MGR2" > $BASE/manager2/round1_output.json
echo "  ✓ Manager2: Generated commitments (rank 2)"

cd $BASE/manager3
R1_MGR3=$($YUSHAN keygen-round1 --threshold $THRESHOLD --n-parties $N_PARTIES --my-index 7 --rank 2 --hierarchical 2>&1 | grep '"party_index":7' | tail -1)
echo "$R1_MGR3" > $BASE/manager3/round1_output.json
echo "  ✓ Manager3: Generated commitments (rank 2)"

cd $BASE/intern1
R1_INT1=$($YUSHAN keygen-round1 --threshold $THRESHOLD --n-parties $N_PARTIES --my-index 8 --rank 2 --hierarchical 2>&1 | grep '"party_index":8' | tail -1)
echo "$R1_INT1" > $BASE/intern1/round1_output.json
echo "  ✓ Intern1: Generated commitments (rank 2)"

cd $BASE/intern2
R1_INT2=$($YUSHAN keygen-round1 --threshold $THRESHOLD --n-parties $N_PARTIES --my-index 9 --rank 2 --hierarchical 2>&1 | grep '"party_index":9' | tail -1)
echo "$R1_INT2" > $BASE/intern2/round1_output.json
echo "  ✓ Intern2: Generated commitments (rank 2)"

cd $BASE/intern3
R1_INT3=$($YUSHAN keygen-round1 --threshold $THRESHOLD --n-parties $N_PARTIES --my-index 10 --rank 2 --hierarchical 2>&1 | grep '"party_index":10' | tail -1)
echo "$R1_INT3" > $BASE/intern3/round1_output.json
echo "  ✓ Intern3: Generated commitments (rank 2)"

# Combine all round1 outputs
ALL_R1="$R1_CEO $R1_CFO $R1_COO $R1_CTO $R1_MGR1 $R1_MGR2 $R1_MGR3 $R1_INT1 $R1_INT2 $R1_INT3"
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
echo "  ✓ CEO: Computed secret shares"

cd $BASE/cfo
R2_CFO=$($YUSHAN keygen-round2 --data "$ALL_R1" 2>&1 | grep '"party_index":2' | tail -1)
echo "$R2_CFO" > $BASE/cfo/round2_output.json
echo "  ✓ CFO: Computed secret shares"

cd $BASE/coo
R2_COO=$($YUSHAN keygen-round2 --data "$ALL_R1" 2>&1 | grep '"party_index":3' | tail -1)
echo "$R2_COO" > $BASE/coo/round2_output.json
echo "  ✓ COO: Computed secret shares"

cd $BASE/cto
R2_CTO=$($YUSHAN keygen-round2 --data "$ALL_R1" 2>&1 | grep '"party_index":4' | tail -1)
echo "$R2_CTO" > $BASE/cto/round2_output.json
echo "  ✓ CTO: Computed secret shares"

cd $BASE/manager1
R2_MGR1=$($YUSHAN keygen-round2 --data "$ALL_R1" 2>&1 | grep '"party_index":5' | tail -1)
echo "$R2_MGR1" > $BASE/manager1/round2_output.json
echo "  ✓ Manager1: Computed secret shares"

cd $BASE/manager2
R2_MGR2=$($YUSHAN keygen-round2 --data "$ALL_R1" 2>&1 | grep '"party_index":6' | tail -1)
echo "$R2_MGR2" > $BASE/manager2/round2_output.json
echo "  ✓ Manager2: Computed secret shares"

cd $BASE/manager3
R2_MGR3=$($YUSHAN keygen-round2 --data "$ALL_R1" 2>&1 | grep '"party_index":7' | tail -1)
echo "$R2_MGR3" > $BASE/manager3/round2_output.json
echo "  ✓ Manager3: Computed secret shares"

cd $BASE/intern1
R2_INT1=$($YUSHAN keygen-round2 --data "$ALL_R1" 2>&1 | grep '"party_index":8' | tail -1)
echo "$R2_INT1" > $BASE/intern1/round2_output.json
echo "  ✓ Intern1: Computed secret shares"

cd $BASE/intern2
R2_INT2=$($YUSHAN keygen-round2 --data "$ALL_R1" 2>&1 | grep '"party_index":9' | tail -1)
echo "$R2_INT2" > $BASE/intern2/round2_output.json
echo "  ✓ Intern2: Computed secret shares"

cd $BASE/intern3
R2_INT3=$($YUSHAN keygen-round2 --data "$ALL_R1" 2>&1 | grep '"party_index":10' | tail -1)
echo "$R2_INT3" > $BASE/intern3/round2_output.json
echo "  ✓ Intern3: Computed secret shares"

ALL_R2="$R2_CEO $R2_CFO $R2_COO $R2_CTO $R2_MGR1 $R2_MGR2 $R2_MGR3 $R2_INT1 $R2_INT2 $R2_INT3"
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
cd $BASE/cto && FINALIZE_CTO=$($YUSHAN keygen-finalize --data "$ALL_R2" 2>&1)
cd $BASE/manager1 && FINALIZE_MGR1=$($YUSHAN keygen-finalize --data "$ALL_R2" 2>&1)
cd $BASE/manager2 && FINALIZE_MGR2=$($YUSHAN keygen-finalize --data "$ALL_R2" 2>&1)
cd $BASE/manager3 && FINALIZE_MGR3=$($YUSHAN keygen-finalize --data "$ALL_R2" 2>&1)
cd $BASE/intern1 && FINALIZE_INT1=$($YUSHAN keygen-finalize --data "$ALL_R2" 2>&1)
cd $BASE/intern2 && FINALIZE_INT2=$($YUSHAN keygen-finalize --data "$ALL_R2" 2>&1)
cd $BASE/intern3 && FINALIZE_INT3=$($YUSHAN keygen-finalize --data "$ALL_R2" 2>&1)

# Extract and save keys
PUBKEY=$(echo "$FINALIZE_CEO" | grep "Public Key:" | sed 's/.*Public Key: //')
SECRET_CEO=$(echo "$FINALIZE_CEO" | grep "Secret Share:" | sed 's/.*Secret Share: //')
SECRET_CFO=$(echo "$FINALIZE_CFO" | grep "Secret Share:" | sed 's/.*Secret Share: //')
SECRET_COO=$(echo "$FINALIZE_COO" | grep "Secret Share:" | sed 's/.*Secret Share: //')
SECRET_CTO=$(echo "$FINALIZE_CTO" | grep "Secret Share:" | sed 's/.*Secret Share: //')
SECRET_MGR1=$(echo "$FINALIZE_MGR1" | grep "Secret Share:" | sed 's/.*Secret Share: //')
SECRET_MGR2=$(echo "$FINALIZE_MGR2" | grep "Secret Share:" | sed 's/.*Secret Share: //')
SECRET_MGR3=$(echo "$FINALIZE_MGR3" | grep "Secret Share:" | sed 's/.*Secret Share: //')
SECRET_INT1=$(echo "$FINALIZE_INT1" | grep "Secret Share:" | sed 's/.*Secret Share: //')
SECRET_INT2=$(echo "$FINALIZE_INT2" | grep "Secret Share:" | sed 's/.*Secret Share: //')
SECRET_INT3=$(echo "$FINALIZE_INT3" | grep "Secret Share:" | sed 's/.*Secret Share: //')

# Save to files
echo "$PUBKEY" > $BASE/public_key.txt
echo "$SECRET_CEO" > $BASE/ceo/secret_share.txt
echo "$SECRET_CFO" > $BASE/cfo/secret_share.txt
echo "$SECRET_COO" > $BASE/coo/secret_share.txt
echo "$SECRET_CTO" > $BASE/cto/secret_share.txt
echo "$SECRET_MGR1" > $BASE/manager1/secret_share.txt
echo "$SECRET_MGR2" > $BASE/manager2/secret_share.txt
echo "$SECRET_MGR3" > $BASE/manager3/secret_share.txt
echo "$SECRET_INT1" > $BASE/intern1/secret_share.txt
echo "$SECRET_INT2" > $BASE/intern2/secret_share.txt
echo "$SECRET_INT3" > $BASE/intern3/secret_share.txt

echo "  ✓ CEO: Secret share saved (rank 0)"
echo "  ✓ CFO: Secret share saved (rank 1)"
echo "  ✓ COO: Secret share saved (rank 1)"
echo "  ✓ CTO: Secret share saved (rank 1)"
echo "  ✓ Manager1: Secret share saved (rank 2)"
echo "  ✓ Manager2: Secret share saved (rank 2)"
echo "  ✓ Manager3: Secret share saved (rank 2)"
echo "  ✓ Intern1: Secret share saved (rank 2)"
echo "  ✓ Intern2: Secret share saved (rank 2)"
echo "  ✓ Intern3: Secret share saved (rank 2)"
echo ""

echo "╔════════════════════════════════════════════════════════════════════╗"
echo "║                      DKG COMPLETE!                                 ║"
echo "╠════════════════════════════════════════════════════════════════════╣"
echo "║  Shared Public Key:                                                ║"
echo "║  $PUBKEY"
echo "╠════════════════════════════════════════════════════════════════════╣"
echo "║  Data stored in: $BASE"
echo "║                                                                    ║"
echo "║  10 Parties Created:                                               ║"
echo "║  ├── Rank 0: ceo/                                                  ║"
echo "║  ├── Rank 1: cfo/, coo/, cto/                                      ║"
echo "║  └── Rank 2: manager1/, manager2/, manager3/,                      ║"
echo "║              intern1/, intern2/, intern3/                          ║"
echo "╚════════════════════════════════════════════════════════════════════╝"
echo ""
echo "Next step: Run ./demo-sign.sh to sign with different signer combinations"
echo ""
echo "Examples:"
echo "  ./demo-sign.sh ceo cfo coo       # Valid [0,1,1]"
echo "  ./demo-sign.sh ceo cto intern1   # Valid [0,1,2]"
echo "  ./demo-sign.sh cfo coo cto       # Invalid [1,1,1] - no CEO!"
echo "  ./demo-sign.sh manager1 intern1 intern2  # Invalid [2,2,2]"
echo ""
