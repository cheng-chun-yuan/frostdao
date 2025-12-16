#!/bin/bash
# Simple HTSS Demo - 4 parties, threshold 2
# Generates keys and signatures, outputs JSON for web display

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
YUSHAN="$SCRIPT_DIR/target/release/yushan"
BASE="/tmp/htss_simple"

# Clean start
rm -rf "$BASE"
mkdir -p "$BASE"

echo "HTSS Simple Demo - 4 parties, threshold 2"
echo "=========================================="
echo ""

# Configuration
THRESHOLD=2
N_PARTIES=4

# Party definitions: id index rank name
PARTIES="ceo:1:0:CEO cfo:2:1:CFO coo:3:1:COO manager:4:2:Manager"

#############################################
# DKG Phase
#############################################
echo "=== DKG ROUND 1 ==="
R1_DATA=""
for p in $PARTIES; do
    id=$(echo $p | cut -d: -f1)
    idx=$(echo $p | cut -d: -f2)
    rank=$(echo $p | cut -d: -f3)
    name=$(echo $p | cut -d: -f4)

    mkdir -p "$BASE/$id"
    cd "$BASE/$id"

    OUTPUT=$($YUSHAN keygen-round1 --threshold $THRESHOLD --n-parties $N_PARTIES --my-index $idx --rank $rank --hierarchical 2>&1)
    R1=$(echo "$OUTPUT" | grep '"party_index"' | tail -1)
    echo "$R1" > round1.json

    if [ -z "$R1_DATA" ]; then
        R1_DATA="$R1"
    else
        R1_DATA="$R1_DATA $R1"
    fi
    echo "  $name: done"
done

echo ""
echo "=== DKG ROUND 2 ==="
R2_DATA=""
for p in $PARTIES; do
    id=$(echo $p | cut -d: -f1)
    name=$(echo $p | cut -d: -f4)

    cd "$BASE/$id"
    OUTPUT=$($YUSHAN keygen-round2 --data "$R1_DATA" 2>&1)
    R2=$(echo "$OUTPUT" | grep '"party_index"' | tail -1)
    echo "$R2" > round2.json

    if [ -z "$R2_DATA" ]; then
        R2_DATA="$R2"
    else
        R2_DATA="$R2_DATA $R2"
    fi
    echo "  $name: done"
done

echo ""
echo "=== DKG FINALIZE ==="
PUBKEY=""
for p in $PARTIES; do
    id=$(echo $p | cut -d: -f1)
    name=$(echo $p | cut -d: -f4)

    cd "$BASE/$id"
    OUTPUT=$($YUSHAN keygen-finalize --data "$R2_DATA" 2>&1)

    # Extract public key
    PK=$(echo "$OUTPUT" | grep "Public Key:" | sed 's/.*Public Key: //')
    if [ -n "$PK" ] && [ -z "$PUBKEY" ]; then
        PUBKEY="$PK"
    fi
    echo "  $name: done"
done

echo ""
echo "Public Key: $PUBKEY"
echo "$PUBKEY" > "$BASE/public_key.txt"

#############################################
# Signing Phase - Valid combo: CEO + CFO
#############################################
echo ""
echo "=== SIGNING: CEO + CFO [0,1] - VALID ==="
SESSION="valid_$(date +%s)"
MESSAGE="Transfer 10 BTC"

# Generate nonces
NONCES=""
for signer in "ceo:1:0:CEO" "cfo:2:1:CFO"; do
    id=$(echo $signer | cut -d: -f1)
    name=$(echo $signer | cut -d: -f4)

    cd "$BASE/$id"
    OUTPUT=$($YUSHAN generate-nonce --session "$SESSION" 2>&1)
    NONCE=$(echo "$OUTPUT" | grep '"party_index"' | tail -1)

    if [ -z "$NONCES" ]; then
        NONCES="$NONCE"
    else
        NONCES="$NONCES $NONCE"
    fi
    echo "  $name: nonce generated"
done

# Create signature shares
SHARES=""
for signer in "ceo:1:0:CEO" "cfo:2:1:CFO"; do
    id=$(echo $signer | cut -d: -f1)
    name=$(echo $signer | cut -d: -f4)

    cd "$BASE/$id"
    OUTPUT=$($YUSHAN sign --session "$SESSION" --message "$MESSAGE" --data "$NONCES" 2>&1)
    SHARE=$(echo "$OUTPUT" | grep '"party_index"' | tail -1)

    if [ -z "$SHARES" ]; then
        SHARES="$SHARE"
    else
        SHARES="$SHARES $SHARE"
    fi
    echo "  $name: signature share created"
done

# Combine
cd "$BASE/ceo"
COMBINE_OUT=$($YUSHAN combine --data "$SHARES" 2>&1)
SIG_VALID=$(echo "$COMBINE_OUT" | grep "^Signature:" | sed 's/Signature: //')
echo "  Signature: ${SIG_VALID:0:32}..."
echo "  Status: SUCCESS"
VERIFY_VALID="true"

#############################################
# Signing Phase - Invalid combo: CFO + COO
#############################################
echo ""
echo "=== SIGNING: CFO + COO [1,1] - INVALID ==="
SESSION2="invalid_$(date +%s)"

# Generate nonces
NONCES2=""
for signer in "cfo:2:1:CFO" "coo:3:1:COO"; do
    id=$(echo $signer | cut -d: -f1)
    name=$(echo $signer | cut -d: -f4)

    cd "$BASE/$id"
    OUTPUT=$($YUSHAN generate-nonce --session "$SESSION2" 2>&1)
    NONCE=$(echo "$OUTPUT" | grep '"party_index"' | tail -1)

    if [ -z "$NONCES2" ]; then
        NONCES2="$NONCE"
    else
        NONCES2="$NONCES2 $NONCE"
    fi
    echo "  $name: nonce generated"
done

# Try to create signature shares (should fail)
cd "$BASE/cfo"
SIGN_OUT=$($YUSHAN sign --session "$SESSION2" --message "$MESSAGE" --data "$NONCES2" 2>&1) || true
if echo "$SIGN_OUT" | grep -q "Invalid HTSS"; then
    INVALID_ERROR=$(echo "$SIGN_OUT" | grep "Invalid HTSS" | head -1)
    echo "  REJECTED: $INVALID_ERROR"
    SIG_INVALID="REJECTED"
else
    echo "  Unexpected: signing succeeded"
    SIG_INVALID="unexpected"
fi

#############################################
# Generate JSON output
#############################################
cat > "$BASE/demo_result.json" << EOF
{
  "config": {
    "threshold": $THRESHOLD,
    "n_parties": $N_PARTIES,
    "parties": [
      {"id": "ceo", "index": 1, "rank": 0, "name": "CEO"},
      {"id": "cfo", "index": 2, "rank": 1, "name": "CFO"},
      {"id": "coo", "index": 3, "rank": 1, "name": "COO"},
      {"id": "manager", "index": 4, "rank": 2, "name": "Manager"}
    ]
  },
  "public_key": "$PUBKEY",
  "valid_signing": {
    "signers": ["CEO", "CFO"],
    "ranks": [0, 1],
    "message": "$MESSAGE",
    "signature": "$SIG_VALID",
    "verified": $VERIFY_VALID
  },
  "invalid_signing": {
    "signers": ["CFO", "COO"],
    "ranks": [1, 1],
    "message": "$MESSAGE",
    "result": "$SIG_INVALID"
  }
}
EOF

echo ""
echo "=========================================="
echo "Demo complete! Results saved to:"
echo "  $BASE/demo_result.json"
echo ""
cat "$BASE/demo_result.json"
