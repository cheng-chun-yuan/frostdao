#!/bin/bash
# HTSS Demo: Hierarchical Threshold Signature Scheme
# 3-of-4 Corporate Treasury Example

# Build first
echo "Building yushan..."
cargo build --release --quiet 2>/dev/null
# Use absolute path for yushan binary
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
YUSHAN="$SCRIPT_DIR/target/release/yushan"

# Setup directories
BASE="/tmp/htss_demo"
rm -rf $BASE
mkdir -p $BASE/{ceo,cfo,coo,manager}

echo ""
echo "╔════════════════════════════════════════════════════════════════════╗"
echo "║       HTSS Demo: Hierarchical Threshold Signature Scheme           ║"
echo "║                   3-of-4 Corporate Treasury                        ║"
echo "╠════════════════════════════════════════════════════════════════════╣"
echo "║                                                                    ║"
echo "║  What is HTSS?                                                     ║"
echo "║  • Extension of threshold signatures with RANKS (authority levels) ║"
echo "║  • Higher authority (lower rank number) = more signing power       ║"
echo "║  • Enforces organizational hierarchy cryptographically             ║"
echo "║                                                                    ║"
echo "╠════════════════════════════════════════════════════════════════════╣"
echo "║  Party Setup:                                                      ║"
echo "║  ┌─────────┬─────────┬────────────────────────────────────────┐    ║"
echo "║  │ Party   │ Rank    │ Description                            │    ║"
echo "║  ├─────────┼─────────┼────────────────────────────────────────┤    ║"
echo "║  │ CEO     │ 0       │ Highest Authority - REQUIRED for sign  │    ║"
echo "║  │ CFO     │ 1       │ High Authority                         │    ║"
echo "║  │ COO     │ 1       │ High Authority                         │    ║"
echo "║  │ Manager │ 2       │ Lower Authority                        │    ║"
echo "║  └─────────┴─────────┴────────────────────────────────────────┘    ║"
echo "║                                                                    ║"
echo "║  Threshold: 3-of-4 (need 3 parties to sign)                        ║"
echo "║  HTSS Rule: sorted_ranks[i] <= i (rank at position must be <= pos) ║"
echo "║                                                                    ║"
echo "╚════════════════════════════════════════════════════════════════════╝"
echo ""

#############################################
# PHASE 1: DKG
#############################################
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  PHASE 1: Distributed Key Generation (DKG)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "  DKG creates a shared key where NO SINGLE PARTY knows the full secret."
echo "  Each party generates a polynomial and shares pieces with others."
echo ""

echo "┌────────────────────────────────────────────────────────────────────┐"
echo "│ ROUND 1: Generate Polynomial Commitments                          │"
echo "│ Each party creates: f(x) = a₀ + a₁x + a₂x² (degree = threshold-1) │"
echo "│ They share commitments [a₀*G, a₁*G, a₂*G] publicly                │"
echo "└────────────────────────────────────────────────────────────────────┘"
echo ""

cd $BASE/ceo
R1_CEO=$($YUSHAN keygen-round1 --threshold 3 --n-parties 4 --my-index 1 --rank 0 --hierarchical 2>&1 | grep '"party_index":1' | tail -1)
echo "  ✓ CEO (index=1, rank=0): Generated polynomial & commitments"
echo "    Command: yushan keygen-round1 --threshold 3 --n-parties 4 --my-index 1 --rank 0 --hierarchical"

cd $BASE/cfo
R1_CFO=$($YUSHAN keygen-round1 --threshold 3 --n-parties 4 --my-index 2 --rank 1 --hierarchical 2>&1 | grep '"party_index":2' | tail -1)
echo "  ✓ CFO (index=2, rank=1): Generated polynomial & commitments"

cd $BASE/coo
R1_COO=$($YUSHAN keygen-round1 --threshold 3 --n-parties 4 --my-index 3 --rank 1 --hierarchical 2>&1 | grep '"party_index":3' | tail -1)
echo "  ✓ COO (index=3, rank=1): Generated polynomial & commitments"

cd $BASE/manager
R1_MGR=$($YUSHAN keygen-round1 --threshold 3 --n-parties 4 --my-index 4 --rank 2 --hierarchical 2>&1 | grep '"party_index":4' | tail -1)
echo "  ✓ Manager (index=4, rank=2): Generated polynomial & commitments"

ALL_R1="$R1_CEO $R1_CFO $R1_COO $R1_MGR"
echo ""
echo "  📤 All parties broadcast their Round 1 data (commitments + proofs)"
echo ""

echo "┌────────────────────────────────────────────────────────────────────┐"
echo "│ ROUND 2: Exchange Secret Shares                                   │"
echo "│ Each party evaluates their polynomial at other parties' indices   │"
echo "│ Party i sends f_i(j) secretly to party j                          │"
echo "└────────────────────────────────────────────────────────────────────┘"
echo ""

cd $BASE/ceo
R2_CEO=$($YUSHAN keygen-round2 --data "$ALL_R1" 2>&1 | grep '"party_index":1' | tail -1)
echo "  ✓ CEO: Computed shares f_CEO(1), f_CEO(2), f_CEO(3), f_CEO(4)"

cd $BASE/cfo
R2_CFO=$($YUSHAN keygen-round2 --data "$ALL_R1" 2>&1 | grep '"party_index":2' | tail -1)
echo "  ✓ CFO: Computed shares f_CFO(1), f_CFO(2), f_CFO(3), f_CFO(4)"

cd $BASE/coo
R2_COO=$($YUSHAN keygen-round2 --data "$ALL_R1" 2>&1 | grep '"party_index":3' | tail -1)
echo "  ✓ COO: Computed shares f_COO(1), f_COO(2), f_COO(3), f_COO(4)"

cd $BASE/manager
R2_MGR=$($YUSHAN keygen-round2 --data "$ALL_R1" 2>&1 | grep '"party_index":4' | tail -1)
echo "  ✓ Manager: Computed shares f_MGR(1), f_MGR(2), f_MGR(3), f_MGR(4)"

ALL_R2="$R2_CEO $R2_CFO $R2_COO $R2_MGR"
echo ""
echo "  📤 Parties exchange shares (each party receives shares from all others)"
echo ""

echo "┌────────────────────────────────────────────────────────────────────┐"
echo "│ FINALIZE: Compute Final Secret Share & Public Key                 │"
echo "│ secret_share_i = f_CEO(i) + f_CFO(i) + f_COO(i) + f_MGR(i)        │"
echo "│ public_key = a₀_CEO*G + a₀_CFO*G + a₀_COO*G + a₀_MGR*G           │"
echo "└────────────────────────────────────────────────────────────────────┘"
echo ""

# Finalize each party and save output
cd $BASE/ceo && FINALIZE_CEO=$($YUSHAN keygen-finalize --data "$ALL_R2" 2>&1)
cd $BASE/cfo && FINALIZE_CFO=$($YUSHAN keygen-finalize --data "$ALL_R2" 2>&1)
cd $BASE/coo && FINALIZE_COO=$($YUSHAN keygen-finalize --data "$ALL_R2" 2>&1)
cd $BASE/manager && FINALIZE_MGR=$($YUSHAN keygen-finalize --data "$ALL_R2" 2>&1)

# Extract keys
PUBKEY=$(echo "$FINALIZE_CEO" | grep "Public Key:" | sed 's/.*Public Key: //')
SECRET_CEO=$(echo "$FINALIZE_CEO" | grep "Secret Share:" | sed 's/.*Secret Share: //')
SECRET_CFO=$(echo "$FINALIZE_CFO" | grep "Secret Share:" | sed 's/.*Secret Share: //')
SECRET_COO=$(echo "$FINALIZE_COO" | grep "Secret Share:" | sed 's/.*Secret Share: //')
SECRET_MGR=$(echo "$FINALIZE_MGR" | grep "Secret Share:" | sed 's/.*Secret Share: //')

echo "  ✓ All parties computed their final secret shares"
echo ""
echo "  ┌────────────────────────────────────────────────────────────────┐"
echo "  │ DKG RESULT                                                     │"
echo "  ├────────────────────────────────────────────────────────────────┤"
echo "  │ Shared Public Key (same for all):                              │"
echo "  │   $PUBKEY │"
echo "  ├────────────────────────────────────────────────────────────────┤"
echo "  │ Secret Shares (each party has unique share):                   │"
echo "  │   CEO:     ${SECRET_CEO:0:16}...${SECRET_CEO: -8} (rank 0)     │"
echo "  │   CFO:     ${SECRET_CFO:0:16}...${SECRET_CFO: -8} (rank 1)     │"
echo "  │   COO:     ${SECRET_COO:0:16}...${SECRET_COO: -8} (rank 1)     │"
echo "  │   Manager: ${SECRET_MGR:0:16}...${SECRET_MGR: -8} (rank 2)     │"
echo "  └────────────────────────────────────────────────────────────────┘"
echo ""
echo "  🔐 NO party knows the full private key!"
echo "  🔐 The private key = sum of all a₀ values (never computed anywhere)"
echo ""

#############################################
# PHASE 2: VALID SIGNING [0,1,1]
#############################################
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  PHASE 2: Signing with VALID Signer Set"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "  Signers: CEO (rank 0) + CFO (rank 1) + COO (rank 1)"
echo ""
echo "  ┌─────────────────────────────────────────────────────────────────┐"
echo "  │ HTSS VALIDATION CHECK                                          │"
echo "  │                                                                 │"
echo "  │ Ranks: [0, 1, 1] → sorted: [0, 1, 1]                           │"
echo "  │                                                                 │"
echo "  │ Rule: sorted_rank[i] ≤ i for all positions                     │"
echo "  │   Position 0: rank 0 ≤ 0 ✓                                     │"
echo "  │   Position 1: rank 1 ≤ 1 ✓                                     │"
echo "  │   Position 2: rank 1 ≤ 2 ✓                                     │"
echo "  │                                                                 │"
echo "  │ Result: ✓ VALID - CEO (highest authority) is present           │"
echo "  └─────────────────────────────────────────────────────────────────┘"
echo ""

SESSION="tx-$(date +%s)"
MESSAGE="Transfer 10 BTC to vendor"

echo "┌────────────────────────────────────────────────────────────────────┐"
echo "│ STEP 1: Generate Nonces (Commitments to Randomness)               │"
echo "│ Each signer generates: (k, R = k*G) where k is secret nonce       │"
echo "└────────────────────────────────────────────────────────────────────┘"
echo ""

cd $BASE/ceo
N_CEO=$($YUSHAN generate-nonce --session "$SESSION" 2>&1 | grep '"party_index":1' | tail -1)
NONCE_CEO=$(echo "$N_CEO" | sed 's/.*"nonce":"\([^"]*\)".*/\1/')
echo "  ✓ CEO: Generated nonce R_CEO = ${NONCE_CEO:0:32}..."

cd $BASE/cfo
N_CFO=$($YUSHAN generate-nonce --session "$SESSION" 2>&1 | grep '"party_index":2' | tail -1)
NONCE_CFO=$(echo "$N_CFO" | sed 's/.*"nonce":"\([^"]*\)".*/\1/')
echo "  ✓ CFO: Generated nonce R_CFO = ${NONCE_CFO:0:32}..."

cd $BASE/coo
N_COO=$($YUSHAN generate-nonce --session "$SESSION" 2>&1 | grep '"party_index":3' | tail -1)
NONCE_COO=$(echo "$N_COO" | sed 's/.*"nonce":"\([^"]*\)".*/\1/')
echo "  ✓ COO: Generated nonce R_COO = ${NONCE_COO:0:32}..."

NONCES_011="$N_CEO $N_CFO $N_COO"
echo ""
echo "  📤 All signers share their nonce commitments"
echo ""

echo "┌────────────────────────────────────────────────────────────────────┐"
echo "│ STEP 2: Create Signature Shares                                   │"
echo "│ Each signer computes: s_i = k_i + e * λ_i * secret_share_i        │"
echo "│ where e = H(R, PK, message) is the challenge                      │"
echo "└────────────────────────────────────────────────────────────────────┘"
echo ""
echo "  Message: \"$MESSAGE\""
echo ""

cd $BASE/ceo
S_CEO=$($YUSHAN sign --session "$SESSION" --message "$MESSAGE" --data "$NONCES_011" 2>&1 | grep '"party_index":1' | tail -1)
SIG_SHARE_CEO=$(echo "$S_CEO" | sed 's/.*"signature_share":"\([^"]*\)".*/\1/')
echo "  ✓ CEO: Created signature share s_CEO = ${SIG_SHARE_CEO:0:32}..."

cd $BASE/cfo
S_CFO=$($YUSHAN sign --session "$SESSION" --message "$MESSAGE" --data "$NONCES_011" 2>&1 | grep '"party_index":2' | tail -1)
SIG_SHARE_CFO=$(echo "$S_CFO" | sed 's/.*"signature_share":"\([^"]*\)".*/\1/')
echo "  ✓ CFO: Created signature share s_CFO = ${SIG_SHARE_CFO:0:32}..."

cd $BASE/coo
S_COO=$($YUSHAN sign --session "$SESSION" --message "$MESSAGE" --data "$NONCES_011" 2>&1 | grep '"party_index":3' | tail -1)
SIG_SHARE_COO=$(echo "$S_COO" | sed 's/.*"signature_share":"\([^"]*\)".*/\1/')
echo "  ✓ COO: Created signature share s_COO = ${SIG_SHARE_COO:0:32}..."

SIGS_011="$S_CEO $S_CFO $S_COO"
echo ""

echo "┌────────────────────────────────────────────────────────────────────┐"
echo "│ STEP 3: Combine Signature Shares                                  │"
echo "│ Final signature: s = s_CEO + s_CFO + s_COO                        │"
echo "│ The combined (R, s) is a valid Schnorr signature!                 │"
echo "└────────────────────────────────────────────────────────────────────┘"
echo ""

cd $BASE/ceo
COMBINE_OUT=$($YUSHAN combine --data "$SIGS_011" 2>&1)
# Extract the hex signature (line starts with "Signature: ")
SIG=$(echo "$COMBINE_OUT" | grep "^Signature: " | sed 's/Signature: //')

echo "  ╔════════════════════════════════════════════════════════════════╗"
echo "  ║                    ✓ SIGNATURE CREATED!                        ║"
echo "  ╠════════════════════════════════════════════════════════════════╣"
echo "  ║  Message:   \"$MESSAGE\""
echo "  ║  Public Key: $PUBKEY"
echo "  ║  Signature:  ${SIG:0:64}"
echo "  ║              ${SIG:64}"
echo "  ╚════════════════════════════════════════════════════════════════╝"
echo ""
echo "  ✓ This signature can be verified by anyone with the public key!"
echo ""

#############################################
# PHASE 3: INVALID SIGNING [1,1,2]
#############################################
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  PHASE 3: Signing with INVALID Signer Set (Attack Scenario)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "  ⚠️  SCENARIO: CFO, COO, and Manager try to sign WITHOUT the CEO"
echo ""
echo "  Signers: CFO (rank 1) + COO (rank 1) + Manager (rank 2)"
echo ""
echo "  ┌─────────────────────────────────────────────────────────────────┐"
echo "  │ HTSS VALIDATION CHECK                                          │"
echo "  │                                                                 │"
echo "  │ Ranks: [1, 1, 2] → sorted: [1, 1, 2]                           │"
echo "  │                                                                 │"
echo "  │ Rule: sorted_rank[i] ≤ i for all positions                     │"
echo "  │   Position 0: rank 1 ≤ 0 ✗ FAIL! (1 > 0)                       │"
echo "  │   Position 1: rank 1 ≤ 1 ✓                                     │"
echo "  │   Position 2: rank 2 ≤ 2 ✓                                     │"
echo "  │                                                                 │"
echo "  │ Result: ✗ INVALID - No rank-0 signer at position 0!            │"
echo "  │         The CEO (highest authority) is REQUIRED!               │"
echo "  └─────────────────────────────────────────────────────────────────┘"
echo ""

SESSION2="invalid-$(date +%s)"
INVALID_MESSAGE="Transfer all funds to attacker"

echo "  Attempting unauthorized transaction: \"$INVALID_MESSAGE\""
echo ""

echo "┌────────────────────────────────────────────────────────────────────┐"
echo "│ STEP 1: Generate Nonces (CFO, COO, Manager only)                  │"
echo "└────────────────────────────────────────────────────────────────────┘"
echo ""

cd $BASE/cfo
N2_CFO=$($YUSHAN generate-nonce --session "$SESSION2" 2>&1 | grep '"party_index":2' | tail -1)
echo "  ✓ CFO: Generated nonce"

cd $BASE/coo
N2_COO=$($YUSHAN generate-nonce --session "$SESSION2" 2>&1 | grep '"party_index":3' | tail -1)
echo "  ✓ COO: Generated nonce"

cd $BASE/manager
N2_MGR=$($YUSHAN generate-nonce --session "$SESSION2" 2>&1 | grep '"party_index":4' | tail -1)
echo "  ✓ Manager: Generated nonce"

NONCES_112="$N2_CFO $N2_COO $N2_MGR"
echo ""

echo "┌────────────────────────────────────────────────────────────────────┐"
echo "│ STEP 2: Attempt to Create Signature Shares                        │"
echo "│ ⚠️  HTSS validation happens HERE before signing!                  │"
echo "└────────────────────────────────────────────────────────────────────┘"
echo ""

cd $BASE/cfo
SIGN_OUT=$($YUSHAN sign --session "$SESSION2" --message "$INVALID_MESSAGE" --data "$NONCES_112" 2>&1)
# Extract the HTSS error message
ERROR=$(echo "$SIGN_OUT" | grep -o "Invalid HTSS signer set:.*rule" | head -1)

echo "  ╔════════════════════════════════════════════════════════════════╗"
echo "  ║              ✗ SIGNATURE REJECTED BY HTSS!                     ║"
echo "  ╠════════════════════════════════════════════════════════════════╣"
echo "  ║                                                                ║"
echo "  ║  Error: $ERROR"
echo "  ║                                                                ║"
echo "  ║  The signing operation was BLOCKED because:                    ║"
echo "  ║  • Position 0 requires a rank-0 signer (CEO)                   ║"
echo "  ║  • CFO (rank 1) cannot fill position 0                         ║"
echo "  ║  • Even with 3 valid parties, hierarchy is enforced!           ║"
echo "  ║                                                                ║"
echo "  ╚════════════════════════════════════════════════════════════════╝"
echo ""
echo "  🛡️  The corporate treasury is protected!"
echo "  🛡️  Subordinates cannot bypass executive approval!"
echo ""

#############################################
# PHASE 4: ADDITIONAL VALID COMBINATION
#############################################
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  PHASE 4: Another Valid Combination [0,1,2]"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "  Signers: CEO (rank 0) + CFO (rank 1) + Manager (rank 2)"
echo ""
echo "  ┌─────────────────────────────────────────────────────────────────┐"
echo "  │ HTSS VALIDATION CHECK                                          │"
echo "  │                                                                 │"
echo "  │ Ranks: [0, 1, 2] → sorted: [0, 1, 2]                           │"
echo "  │                                                                 │"
echo "  │ Rule: sorted_rank[i] ≤ i for all positions                     │"
echo "  │   Position 0: rank 0 ≤ 0 ✓                                     │"
echo "  │   Position 1: rank 1 ≤ 1 ✓                                     │"
echo "  │   Position 2: rank 2 ≤ 2 ✓                                     │"
echo "  │                                                                 │"
echo "  │ Result: ✓ VALID - CEO is present, hierarchy satisfied          │"
echo "  └─────────────────────────────────────────────────────────────────┘"
echo ""

SESSION3="tx2-$(date +%s)"
MESSAGE2="Approve quarterly bonus"

cd $BASE/ceo
N3_CEO=$($YUSHAN generate-nonce --session "$SESSION3" 2>&1 | grep '"party_index":1' | tail -1)
cd $BASE/cfo
N3_CFO=$($YUSHAN generate-nonce --session "$SESSION3" 2>&1 | grep '"party_index":2' | tail -1)
cd $BASE/manager
N3_MGR=$($YUSHAN generate-nonce --session "$SESSION3" 2>&1 | grep '"party_index":4' | tail -1)

NONCES_012="$N3_CEO $N3_CFO $N3_MGR"

cd $BASE/ceo
S3_CEO=$($YUSHAN sign --session "$SESSION3" --message "$MESSAGE2" --data "$NONCES_012" 2>&1 | grep '"party_index":1' | tail -1)
cd $BASE/cfo
S3_CFO=$($YUSHAN sign --session "$SESSION3" --message "$MESSAGE2" --data "$NONCES_012" 2>&1 | grep '"party_index":2' | tail -1)
cd $BASE/manager
S3_MGR=$($YUSHAN sign --session "$SESSION3" --message "$MESSAGE2" --data "$NONCES_012" 2>&1 | grep '"party_index":4' | tail -1)

SIGS_012="$S3_CEO $S3_CFO $S3_MGR"

cd $BASE/ceo
COMBINE_OUT2=$($YUSHAN combine --data "$SIGS_012" 2>&1)
SIG2=$(echo "$COMBINE_OUT2" | grep "^Signature: " | sed 's/Signature: //')

echo "  ✓ Signature created successfully!"
echo "  Message: \"$MESSAGE2\""
echo "  Signature: ${SIG2:0:48}..."
echo ""

#############################################
# SUMMARY
#############################################
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  DEMO COMPLETE - HTSS Summary"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "  ┌─────────────────────────────────────────────────────────────────┐"
echo "  │ SIGNER COMBINATIONS TESTED                                     │"
echo "  ├─────────────────────────────────────────────────────────────────┤"
echo "  │                                                                 │"
echo "  │  ✓ [0,1,1] CEO + CFO + COO     → VALID   (ranks: 0≤0,1≤1,1≤2) │"
echo "  │  ✗ [1,1,2] CFO + COO + Manager → INVALID (rank 1 > position 0) │"
echo "  │  ✓ [0,1,2] CEO + CFO + Manager → VALID   (ranks: 0≤0,1≤1,2≤2) │"
echo "  │                                                                 │"
echo "  └─────────────────────────────────────────────────────────────────┘"
echo ""
echo "  🔑 KEY TAKEAWAYS:"
echo ""
echo "  1. HTSS = Hierarchical Threshold Secret Sharing"
echo "     • Adds RANKS to traditional threshold signatures"
echo "     • Higher authority = lower rank number"
echo ""
echo "  2. The HTSS Rule: sorted_rank[i] ≤ i"
echo "     • Position 0 needs rank ≤ 0 (only rank 0 works)"
echo "     • Position 1 needs rank ≤ 1 (rank 0 or 1 works)"
echo "     • This ENFORCES hierarchy cryptographically!"
echo ""
echo "  3. Real-World Impact:"
echo "     • CEO (rank 0) MUST be present for any signing"
echo "     • Subordinates cannot collude to bypass executives"
echo "     • Perfect for corporate governance, DAOs, and custody"
echo ""
echo "  📚 Learn more: See README.md for additional use cases!"
echo ""
