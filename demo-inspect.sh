#!/bin/bash
# HTSS Demo - Inspect stored data
# View the data stored by demo-dkg.sh and demo-sign.sh

BASE="${HTSS_DATA_DIR:-/tmp/htss_demo}"

echo ""
echo "╔════════════════════════════════════════════════════════════════════╗"
echo "║                    HTSS Data Inspector                             ║"
echo "╚════════════════════════════════════════════════════════════════════╝"
echo ""

if [ ! -d "$BASE" ]; then
    echo "No data found at $BASE"
    echo "Run ./demo-dkg.sh first to generate keys."
    exit 1
fi

echo "Data Directory: $BASE"
echo ""

#############################################
# Show folder structure
#############################################
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Folder Structure"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
tree -L 2 "$BASE" 2>/dev/null || find "$BASE" -maxdepth 2 -type f | sort
echo ""

#############################################
# Show public key
#############################################
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Public Key (shared by all parties)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
if [ -f "$BASE/public_key.txt" ]; then
    cat "$BASE/public_key.txt"
else
    echo "Not found - run demo-dkg.sh first"
fi
echo ""

#############################################
# Show secret shares
#############################################
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Secret Shares (each party has unique share)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "  Rank 0 (Top Level):"
for party in ceo; do
    if [ -f "$BASE/$party/secret_share.txt" ]; then
        share=$(cat "$BASE/$party/secret_share.txt")
        party_upper=$(echo "$party" | tr '[:lower:]' '[:upper:]')
        share_start=$(echo "$share" | cut -c1-24)
        share_end=$(echo "$share" | tail -c 9)
        echo "    ${party_upper}: ${share_start}...${share_end}"
    fi
done
echo ""
echo "  Rank 1 (C-Suite):"
for party in cfo coo cto; do
    if [ -f "$BASE/$party/secret_share.txt" ]; then
        share=$(cat "$BASE/$party/secret_share.txt")
        party_upper=$(echo "$party" | tr '[:lower:]' '[:upper:]')
        share_start=$(echo "$share" | cut -c1-24)
        share_end=$(echo "$share" | tail -c 9)
        echo "    ${party_upper}: ${share_start}...${share_end}"
    fi
done
echo ""
echo "  Rank 2 (Staff):"
for party in manager1 manager2 manager3 intern1 intern2 intern3; do
    if [ -f "$BASE/$party/secret_share.txt" ]; then
        share=$(cat "$BASE/$party/secret_share.txt")
        party_upper=$(echo "$party" | tr '[:lower:]' '[:upper:]')
        share_start=$(echo "$share" | cut -c1-24)
        share_end=$(echo "$share" | tail -c 9)
        echo "    ${party_upper}: ${share_start}...${share_end}"
    fi
done
echo ""

#############################################
# Show FROST state files
#############################################
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  FROST Internal State Files"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
for party in ceo cfo coo cto manager1 manager2 manager3 intern1 intern2 intern3; do
    if [ -d "$BASE/$party/.frost_state" ]; then
        echo "  $party/.frost_state/"
        ls -la "$BASE/$party/.frost_state/" | tail -n +2 | awk '{print "    " $9 " (" $5 " bytes)"}'
        echo ""
    fi
done

#############################################
# Show signing sessions
#############################################
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Signing Sessions"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
if [ -d "$BASE/signing_sessions" ]; then
    for session_dir in "$BASE/signing_sessions"/*; do
        if [ -d "$session_dir" ]; then
            session=$(basename "$session_dir")
            echo "  Session: $session"

            if [ -f "$session_dir/result.json" ]; then
                echo "    Status: ✓ SUCCESS"
                message=$(grep -o '"message": *"[^"]*"' "$session_dir/result.json" | sed 's/"message": *"//' | sed 's/"$//')
                signers=$(grep -o '"signers": *\[[^]]*\]' "$session_dir/result.json")
                echo "    Message: \"$message\""
                echo "    $signers"

                if [ -f "$session_dir/final_signature.txt" ]; then
                    sig=$(cat "$session_dir/final_signature.txt")
                    echo "    Signature: ${sig:0:32}..."
                fi
            elif [ -f "$session_dir/error.txt" ]; then
                echo "    Status: ✗ FAILED"
                echo "    Error: $(cat "$session_dir/error.txt")"
            fi
            echo ""
        fi
    done
else
    echo "  No signing sessions yet. Run ./demo-sign.sh to create one."
fi
echo ""

#############################################
# Show HTSS metadata
#############################################
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  HTSS Metadata (from CEO)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
if [ -f "$BASE/ceo/.frost_state/htss_metadata.json" ]; then
    cat "$BASE/ceo/.frost_state/htss_metadata.json"
else
    echo "Not found"
fi
echo ""
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Commands"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "  View a specific file:"
echo "    cat $BASE/public_key.txt"
echo "    cat $BASE/ceo/secret_share.txt"
echo "    cat $BASE/ceo/.frost_state/htss_metadata.json"
echo ""
echo "  View Round 1 output for CEO:"
echo "    cat $BASE/ceo/round1_output.json | jq ."
echo ""
echo "  View signing session result:"
echo "    cat $BASE/signing_sessions/<session>/result.json | jq ."
echo ""
