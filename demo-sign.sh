#!/bin/bash
# HTSS Demo - Phase 2: Signing
# Run after demo-dkg.sh to sign with different signer combinations
#
# Usage:
#   ./demo-sign.sh                           # Interactive mode
#   ./demo-sign.sh ceo cfo coo               # Sign with CEO, CFO, COO
#   ./demo-sign.sh cfo coo manager           # Try invalid signing
#   ./demo-sign.sh ceo cfo manager           # Sign with CEO, CFO, Manager
#   ./demo-sign.sh --message "Custom msg" ceo cfo coo

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
YUSHAN="$SCRIPT_DIR/target/release/yushan"
BASE="${HTSS_DATA_DIR:-/tmp/htss_demo}"
MESSAGE="Transfer 10 BTC to vendor"

# Function to get signer index
get_index() {
    case "$1" in
        ceo) echo 1 ;;
        cfo) echo 2 ;;
        coo) echo 3 ;;
        cto) echo 4 ;;
        manager1) echo 5 ;;
        manager2) echo 6 ;;
        manager3) echo 7 ;;
        intern1) echo 8 ;;
        intern2) echo 9 ;;
        intern3) echo 10 ;;
        *) echo 0 ;;
    esac
}

# Function to get signer rank
get_rank() {
    case "$1" in
        ceo) echo 0 ;;
        cfo) echo 1 ;;
        coo) echo 1 ;;
        cto) echo 1 ;;
        manager1) echo 2 ;;
        manager2) echo 2 ;;
        manager3) echo 2 ;;
        intern1) echo 2 ;;
        intern2) echo 2 ;;
        intern3) echo 2 ;;
        *) echo -1 ;;
    esac
}

# Parse arguments
SIGNERS=""
while [ $# -gt 0 ]; do
    case $1 in
        --message|-m)
            MESSAGE="$2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: $0 [--message \"msg\"] signer1 signer2 signer3"
            echo ""
            echo "Signers:"
            echo "  Rank 0: ceo"
            echo "  Rank 1: cfo, coo, cto"
            echo "  Rank 2: manager1, manager2, manager3, intern1, intern2, intern3"
            echo ""
            echo "Examples:"
            echo "  $0 ceo cfo coo              # Valid: ranks [0,1,1]"
            echo "  $0 ceo cto manager1         # Valid: ranks [0,1,2]"
            echo "  $0 cfo coo cto              # Invalid: ranks [1,1,1] - no CEO!"
            echo "  $0 manager1 intern1 intern2 # Invalid: ranks [2,2,2]"
            echo "  $0 -m \"Pay invoice\" ceo cfo coo"
            exit 0
            ;;
        *)
            if [ -z "$SIGNERS" ]; then
                SIGNERS="$1"
            else
                SIGNERS="$SIGNERS $1"
            fi
            shift
            ;;
    esac
done

# Check if DKG was run
if [ ! -d "$BASE/ceo/.frost_state" ]; then
    echo "Error: DKG not completed. Run ./demo-dkg.sh first."
    exit 1
fi

# Load public key
PUBKEY=$(cat $BASE/public_key.txt 2>/dev/null || echo "Unknown")

echo ""
echo "╔════════════════════════════════════════════════════════════════════╗"
echo "║                    HTSS Signing Session                            ║"
echo "╠════════════════════════════════════════════════════════════════════╣"
echo "║  Public Key: $PUBKEY"
echo "║  Data Dir: $BASE"
echo "╚════════════════════════════════════════════════════════════════════╝"
echo ""

# If no signers specified, show interactive menu
if [ -z "$SIGNERS" ]; then
    echo "Select signers (need 3 for threshold):"
    echo ""
    echo "  Valid combinations (CEO required at position 0):"
    echo "    1) CEO + CFO + COO       [0,1,1] - C-Suite only"
    echo "    2) CEO + CFO + CTO       [0,1,1] - C-Suite only"
    echo "    3) CEO + CTO + Manager1  [0,1,2] - Mixed levels"
    echo "    4) CEO + Manager1 + Intern1 [0,2,2] - With junior staff"
    echo ""
    echo "  Invalid combinations (will fail):"
    echo "    5) CFO + COO + CTO       [1,1,1] - No CEO!"
    echo "    6) CTO + Manager1 + Manager2 [1,2,2] - No CEO!"
    echo "    7) Manager1 + Intern1 + Intern2 [2,2,2] - All rank 2!"
    echo ""
    read -p "Enter choice (1-7): " choice

    case $choice in
        1) SIGNERS="ceo cfo coo" ;;
        2) SIGNERS="ceo cfo cto" ;;
        3) SIGNERS="ceo cto manager1" ;;
        4) SIGNERS="ceo manager1 intern1" ;;
        5) SIGNERS="cfo coo cto" ;;
        6) SIGNERS="cto manager1 manager2" ;;
        7) SIGNERS="manager1 intern1 intern2" ;;
        *) echo "Invalid choice"; exit 1 ;;
    esac
fi

# Build signer info
RANKS=""
INDICES=""
for signer in $SIGNERS; do
    signer_lower=$(echo "$signer" | tr '[:upper:]' '[:lower:]')
    idx=$(get_index "$signer_lower")
    rank=$(get_rank "$signer_lower")

    if [ "$idx" = "0" ]; then
        echo "Error: Unknown signer '$signer'."
        echo "Valid signers: ceo, cfo, coo, cto, manager1, manager2, manager3, intern1, intern2, intern3"
        exit 1
    fi

    if [ -z "$RANKS" ]; then
        RANKS="$rank"
        INDICES="$idx"
    else
        RANKS="$RANKS $rank"
        INDICES="$INDICES $idx"
    fi
done

# Sort ranks for validation display
SORTED_RANKS=$(echo "$RANKS" | tr ' ' '\n' | sort -n | tr '\n' ' ')

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Signer Set"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
SIGNERS_UPPER=$(echo "$SIGNERS" | tr '[:lower:]' '[:upper:]')
echo "  Signers: $SIGNERS_UPPER"
echo "  Ranks: [$RANKS] → sorted: [$SORTED_RANKS]"
echo ""
echo "  ┌─────────────────────────────────────────────────────────────────┐"
echo "  │ HTSS VALIDATION CHECK                                          │"
echo "  │                                                                 │"

VALID=true
i=0
for rank in $SORTED_RANKS; do
    if [ $rank -le $i ]; then
        echo "  │   Position $i: rank $rank ≤ $i ✓                                     │"
    else
        echo "  │   Position $i: rank $rank > $i ✗ FAIL!                               │"
        VALID=false
    fi
    i=$((i + 1))
done

echo "  │                                                                 │"
if [ "$VALID" = true ]; then
    echo "  │ Result: ✓ VALID signer set                                     │"
else
    echo "  │ Result: ✗ INVALID signer set                                   │"
fi
echo "  └─────────────────────────────────────────────────────────────────┘"
echo ""

# Create signing session directory
SESSION="sign-$(date +%s)"
SIGN_DIR="$BASE/signing_sessions/$SESSION"
mkdir -p "$SIGN_DIR"

echo "  Message: \"$MESSAGE\""
echo "  Session: $SESSION"
echo "  Session Dir: $SIGN_DIR"
echo ""

#############################################
# STEP 1: Generate Nonces
#############################################
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  STEP 1: Generate Nonces"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

NONCES=""
for signer in $SIGNERS; do
    signer_lower=$(echo "$signer" | tr '[:upper:]' '[:lower:]')
    idx=$(get_index "$signer_lower")

    cd "$BASE/$signer_lower"
    NONCE=$($YUSHAN generate-nonce --session "$SESSION" 2>&1 | grep "\"party_index\":$idx" | tail -1)
    echo "$NONCE" > "$SIGN_DIR/nonce_${signer_lower}.json"

    NONCE_VAL=$(echo "$NONCE" | sed 's/.*"nonce":"\([^"]*\)".*/\1/')
    SIGNER_UPPER=$(echo "$signer" | tr '[:lower:]' '[:upper:]')
    echo "  ✓ ${SIGNER_UPPER}: Generated nonce → $SIGN_DIR/nonce_${signer_lower}.json"
    echo "    Nonce: ${NONCE_VAL:0:40}..."

    if [ -z "$NONCES" ]; then
        NONCES="$NONCE"
    else
        NONCES="$NONCES $NONCE"
    fi
done

echo "$NONCES" > "$SIGN_DIR/all_nonces.txt"
echo ""
echo "  📁 All nonces saved to: $SIGN_DIR/all_nonces.txt"
echo ""

#############################################
# STEP 2: Create Signature Shares
#############################################
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  STEP 2: Create Signature Shares"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

SIGS=""
SIGN_ERROR=""
for signer in $SIGNERS; do
    signer_lower=$(echo "$signer" | tr '[:upper:]' '[:lower:]')
    idx=$(get_index "$signer_lower")

    cd "$BASE/$signer_lower"
    # Use || true to prevent set -e from exiting on error
    SIGN_OUT=$($YUSHAN sign --session "$SESSION" --message "$MESSAGE" --data "$NONCES" 2>&1) || true

    # Check for error
    if echo "$SIGN_OUT" | grep -q "Invalid HTSS"; then
        SIGN_ERROR=$(echo "$SIGN_OUT" | grep -o "Invalid HTSS signer set:.*rule" | head -1)
        SIGNER_UPPER=$(echo "$signer" | tr '[:lower:]' '[:upper:]')
        echo "  ✗ ${SIGNER_UPPER}: HTSS validation failed!"
        break
    fi

    SIG=$(echo "$SIGN_OUT" | grep "\"party_index\":$idx" | tail -1)
    echo "$SIG" > "$SIGN_DIR/sig_share_${signer_lower}.json"

    SIG_SHARE=$(echo "$SIG" | sed 's/.*"signature_share":"\([^"]*\)".*/\1/')
    SIGNER_UPPER=$(echo "$signer" | tr '[:lower:]' '[:upper:]')
    echo "  ✓ ${SIGNER_UPPER}: Created signature share → $SIGN_DIR/sig_share_${signer_lower}.json"
    echo "    Share: ${SIG_SHARE:0:40}..."

    if [ -z "$SIGS" ]; then
        SIGS="$SIG"
    else
        SIGS="$SIGS $SIG"
    fi
done

echo ""

# If there was an error, show it and exit
if [ -n "$SIGN_ERROR" ]; then
    echo "╔════════════════════════════════════════════════════════════════════╗"
    echo "║                ✗ SIGNING REJECTED BY HTSS                         ║"
    echo "╠════════════════════════════════════════════════════════════════════╣"
    echo "║                                                                    ║"
    echo "║  Error: $SIGN_ERROR"
    echo "║                                                                    ║"
    echo "║  The hierarchy is cryptographically enforced!                      ║"
    echo "║  A rank-0 signer (CEO) is required at position 0.                  ║"
    echo "║                                                                    ║"
    echo "╚════════════════════════════════════════════════════════════════════╝"
    echo ""

    # Save error to session
    echo "$SIGN_ERROR" > "$SIGN_DIR/error.txt"
    echo "  📁 Error saved to: $SIGN_DIR/error.txt"
    echo ""
    exit 1
fi

echo "$SIGS" > "$SIGN_DIR/all_sig_shares.txt"
echo "  📁 All signature shares saved to: $SIGN_DIR/all_sig_shares.txt"
echo ""

#############################################
# STEP 3: Combine Signature
#############################################
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  STEP 3: Combine Signature Shares"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Use first signer's directory for combine
first_signer=$(echo "$SIGNERS" | awk '{print $1}' | tr '[:upper:]' '[:lower:]')
cd "$BASE/$first_signer"

COMBINE_OUT=$($YUSHAN combine --data "$SIGS" 2>&1)
SIGNATURE=$(echo "$COMBINE_OUT" | grep "^Signature: " | sed 's/Signature: //')

# Save final signature
echo "$SIGNATURE" > "$SIGN_DIR/final_signature.txt"

# Format signers for JSON
SIGNERS_JSON=$(echo "$SIGNERS" | tr ' ' '\n' | sed 's/.*/"&"/' | tr '\n' ',' | sed 's/,$//')

# Save full result
cat > "$SIGN_DIR/result.json" << EOF
{
  "message": "$MESSAGE",
  "signers": [$SIGNERS_JSON],
  "ranks": [$RANKS],
  "public_key": "$PUBKEY",
  "signature": "$SIGNATURE",
  "session": "$SESSION"
}
EOF

echo "╔════════════════════════════════════════════════════════════════════╗"
echo "║                    ✓ SIGNATURE CREATED!                            ║"
echo "╠════════════════════════════════════════════════════════════════════╣"
echo "║  Message:    \"$MESSAGE\""
echo "║  Signers:    $SIGNERS_UPPER"
echo "║  Ranks:      [$RANKS]"
echo "║  Public Key: $PUBKEY"
echo "╠════════════════════════════════════════════════════════════════════╣"
echo "║  Signature:                                                        ║"
echo "║  ${SIGNATURE:0:64}"
echo "║  ${SIGNATURE:64}"
echo "╠════════════════════════════════════════════════════════════════════╣"
echo "║  Session Data: $SIGN_DIR"
echo "║                                                                    ║"
echo "║  Files:                                                            ║"
echo "║  ├── nonce_*.json       # Individual nonces                        ║"
echo "║  ├── all_nonces.txt     # Combined nonces                          ║"
echo "║  ├── sig_share_*.json   # Individual signature shares              ║"
echo "║  ├── all_sig_shares.txt # Combined shares                          ║"
echo "║  ├── final_signature.txt# Final Schnorr signature                  ║"
echo "║  └── result.json        # Complete result                          ║"
echo "╚════════════════════════════════════════════════════════════════════╝"
echo ""
