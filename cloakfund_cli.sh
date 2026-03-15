#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════════
#  CloakFund ZK-Mixer Terminal Interface
#  Full Tornado Cash Lite (Hash-Commit-Reveal) flow:
#
#    1. Generate stealth wallet (temp wallet) for receiver
#    2. Sender sends ETH to the stealth wallet
#    3. Sweeper auto-detects & deposits into PrivacyPool (commitment)
#    4. Receiver withdraws anonymously via the Relayer → Main Wallet
# ═══════════════════════════════════════════════════════════════════════════════

set -euo pipefail

API_URL="${API_URL:-http://localhost:8080}"

# Mock public key for demo (in production this comes from ENS)
MOCK_PUB="0x04b10912af0c04aa473bebc86f36f44eed2bbbc6bcad611287140975fafe159974b8ac6bccd806e4647e45eda540d9ae05aed61ebff5d0bff409e813d2ad33d7f6"

echo ""
echo "╔═══════════════════════════════════════════════════════╗"
echo "║       CloakFund ZK-Mixer — Private Payments          ║"
echo "║       (Tornado Cash Lite / Hash-Commit-Reveal)       ║"
echo "╚═══════════════════════════════════════════════════════╝"
echo ""

# ─── Pre-flight check ───
if ! curl -s "$API_URL/health" > /dev/null 2>&1; then
    echo "❌ Backend is not running at $API_URL"
    echo "   Run: cd rust-backend && RUST_LOG=info cargo run -- serve"
    exit 1
fi
echo "✅ Backend online at $API_URL"
echo ""

# ─────────────────────────────────────────────────────────────────────────────
#  Step 1: Receiver Setup
# ─────────────────────────────────────────────────────────────────────────────
echo "📨 Step 1: Receiver Setup"
echo "Enter your name (e.g., alice.eth) — just an identifier, ENS is NOT required:"
read -r ENS_NAME

echo ""
echo "Enter your MAIN WALLET address (the final destination for anonymous withdrawal):"
echo "  ⚠️  Must be a valid 0x... Ethereum address"
read -r MAIN_WALLET

if [[ ! "$MAIN_WALLET" =~ ^0x[a-fA-F0-9]{40}$ ]]; then
    echo "❌ Invalid Ethereum address. Must be 0x followed by 40 hex characters."
    exit 1
fi

# ─────────────────────────────────────────────────────────────────────────────
#  Step 2: Generate Stealth Address (Temp Wallet)
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "🔐 Step 2: Generating temporary stealth wallet for $ENS_NAME..."

PAYLINK_RES=$(curl -s -X POST "$API_URL/api/v1/paylink" \
    -H "Content-Type: application/json" \
    -d '{"recipientPublicKeyHex": "'"$MOCK_PUB"'", "ensName": "'"$ENS_NAME"'", "chainId": 84532, "network": "base-sepolia"}')

PAYLINK_ID=$(echo "$PAYLINK_RES" | grep -o '"paylinkId":"[^"]*' | cut -d'"' -f4)
STEALTH_ADDR=$(echo "$PAYLINK_RES" | grep -o '"stealthAddress":"[^"]*' | cut -d'"' -f4)

if [ -z "$PAYLINK_ID" ] || [ "$PAYLINK_ID" == "null" ]; then
    echo "❌ Failed to create temp wallet. Response:"
    echo "$PAYLINK_RES"
    exit 1
fi

echo ""
echo "╔═══════════════════════════════════════════════════════╗"
echo "║  ✅ Temporary Stealth Wallet Created!                ║"
echo "║  Share this address with the sender:                 ║"
echo "║  👉 $STEALTH_ADDR"
echo "╚═══════════════════════════════════════════════════════╝"

# ─────────────────────────────────────────────────────────────────────────────
#  Step 3: Simulate Payment (Sender → Temp Wallet)
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "💸 Step 3: Sending 0.0003 testnet ETH to the stealth wallet..."
echo "   (0.0001 ETH for the deposit + ~0.0002 ETH for gas)"
SEND_OUT=$(node scripts/send_eth.mjs "$STEALTH_ADDR" 0.0003)

TX_HASH=$(echo "$SEND_OUT" | grep -Eo '0x[a-fA-F0-9]{64}' | head -n 1)
echo "$SEND_OUT"

if [ -z "$TX_HASH" ]; then
    echo "❌ Failed to send ETH."
    exit 1
fi

# ─────────────────────────────────────────────────────────────────────────────
#  Step 4: Wait for Detection → Auto-Deposit into PrivacyPool
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "🔍 Step 4: Waiting for CloakFund Agent to detect payment & auto-deposit into PrivacyPool..."
echo "   Tx Hash: $TX_HASH"
echo "   (The watcher polls every ~2s, the sweeper runs every ~10s)"
echo ""

# Phase A: Wait for the deposit to be detected
ATTEMPTS=0
DEPOSIT_ID=""
while [ $ATTEMPTS -lt 60 ]; do
    sleep 5
    ATTEMPTS=$((ATTEMPTS + 1))

    TX_OUT=$(bunx convex run deposits:getDepositsByTxHash '{"txHash": "'"$TX_HASH"'"}' 2>/dev/null || true)
    # Extract depositId — handle both JS format and JSON format (with/without spaces)
    DEPOSIT_ID=$(echo "$TX_OUT" | grep -oE '"depositId"\s*:\s*"[^"]+"' | head -n 1 | sed 's/.*"depositId"[[:space:]]*:[[:space:]]*"//;s/"//' || true)

    if [ -n "$DEPOSIT_ID" ] && [ "$DEPOSIT_ID" != "null" ]; then
        echo ""
        echo "   🎉 Deposit detected! ID: $DEPOSIT_ID"
        break
    fi

    echo -n "."
done

if [ -z "$DEPOSIT_ID" ] || [ "$DEPOSIT_ID" == "null" ]; then
    echo ""
    echo "❌ Timed out waiting for deposit detection (5 min). Check backend logs."
    echo "   The watcher may have skipped the block. Try restarting the backend."
    exit 1
fi

# Phase B: Queue the sweep job via the consolidate API
echo ""
echo "🤖 Queuing ZK-Mixer deposit (stealth wallet → PrivacyPool)..."
SWEEP_RES=$(curl -s -X POST "$API_URL/api/v1/consolidate" \
    -H "Content-Type: application/json" \
    -d '{"deposit_id": "'"$DEPOSIT_ID"'"}')

JOB_ID=$(echo "$SWEEP_RES" | grep -o '"job_id":"[^"]*' | cut -d'"' -f4)
if [ -z "$JOB_ID" ] || [ "$JOB_ID" == "null" ]; then
    echo "   ⚠️  Could not queue sweep (may already be auto-queued). Continuing..."
    echo "   Response: $SWEEP_RES"
else
    echo "   ✅ Sweep Job Queued: $JOB_ID"
fi

# Phase C: Wait for the sweeper to generate the privacy note
echo ""
echo "⏳ Waiting for the Sweeper to deposit into PrivacyPool..."
echo "   (This generates the secret + nullifier = your Privacy Note)"
echo ""

ATTEMPTS=0
SECRET_HEX=""
NULLIFIER_HEX=""
POOL_TX=""
while [ $ATTEMPTS -lt 60 ]; do
    sleep 5
    ATTEMPTS=$((ATTEMPTS + 1))

    NOTE_OUT=$(bunx convex run notes:getNoteByDeposit '{"depositId": "'"$DEPOSIT_ID"'"}' 2>/dev/null || true)
    # Extract fields — handle both JS and JSON format (with/without spaces)
    SECRET_HEX=$(echo "$NOTE_OUT" | grep -oE '"?secretHex"?\s*:\s*["'\'''][^"'\'']+' | head -n 1 | sed "s/.*[:\:][[:space:]]*['\"]//" || true)
    NULLIFIER_HEX=$(echo "$NOTE_OUT" | grep -oE '"?nullifierHex"?\s*:\s*["'\'''][^"'\'']+' | head -n 1 | sed "s/.*[:\:][[:space:]]*['\"]//" || true)
    POOL_TX=$(echo "$NOTE_OUT" | grep -oE '"?poolDepositTxHash"?\s*:\s*["'\'''][^"'\'']+' | head -n 1 | sed "s/.*[:\:][[:space:]]*['\"]//" || true)

    if [ -n "$SECRET_HEX" ] && [ -n "$NULLIFIER_HEX" ]; then
        echo ""
        echo "╔═══════════════════════════════════════════════════════╗"
        echo "║  🔒 PRIVACY NOTE GENERATED!                         ║"
        echo "║  Funds are now in the PrivacyPool contract.         ║"
        echo "╚═══════════════════════════════════════════════════════╝"
        echo ""
        echo "   Secret:    $SECRET_HEX"
        echo "   Nullifier: $NULLIFIER_HEX"
        echo "   Pool Tx:   $POOL_TX"
        echo ""
        break
    fi

    echo -n "."
done

if [ -z "$SECRET_HEX" ] || [ -z "$NULLIFIER_HEX" ]; then
    echo ""
    echo "❌ Timed out waiting for privacy note (5 min). Check backend logs."
    echo "   The sweeper may need the stealth wallet to have enough balance for gas."
    exit 1
fi

# ─────────────────────────────────────────────────────────────────────────────
#  Step 5: The Privacy Break
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════════════════"
echo "  ⏸️  THE PRIVACY BREAK"
echo ""
echo "  In a real scenario, the receiver waits hours/days"
echo "  before withdrawing. The longer the wait, the bigger"
echo "  the anonymity set."
echo ""
echo "  Press ENTER to withdraw now..."
echo "═══════════════════════════════════════════════════════"
read -r

# ─────────────────────────────────────────────────────────────────────────────
#  Step 6: Anonymous Withdrawal via Relayer → Main Wallet
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "🔄 Step 6: Initiating anonymous withdrawal via Relayer..."
echo "   Sending secret Note to the Relayer API..."
echo "   Recipient Main Wallet: $MAIN_WALLET"
echo ""

WITHDRAW_RES=$(curl -s -X POST "$API_URL/api/v1/withdraw" \
    -H "Content-Type: application/json" \
    -d '{"secretHex": "'"$SECRET_HEX"'", "nullifierHex": "'"$NULLIFIER_HEX"'", "recipientAddress": "'"$MAIN_WALLET"'"}')

WITHDRAW_TX=$(echo "$WITHDRAW_RES" | grep -o '"txHash":"[^"]*' | cut -d'"' -f4)
WITHDRAW_STATUS=$(echo "$WITHDRAW_RES" | grep -o '"status":"[^"]*' | cut -d'"' -f4)

if [ "$WITHDRAW_STATUS" == "submitted" ]; then
    echo ""
    echo "╔═══════════════════════════════════════════════════════╗"
    echo "║  🎉 ANONYMOUS WITHDRAWAL SUCCESSFUL!                ║"
    echo "╚═══════════════════════════════════════════════════════╝"
    echo ""
    echo "   Withdrawal Tx: $WITHDRAW_TX"
    echo "   Recipient:     $MAIN_WALLET"
    echo ""
    echo "   The PrivacyPool contract sent 0.0001 ETH directly"
    echo "   to your Main Wallet. There is NO on-chain link"
    echo "   between the Stealth Wallet and your Main Wallet!"
    echo ""
    echo "   🔗 View on Explorer:"
    echo "      https://sepolia.basescan.org/tx/$WITHDRAW_TX"
else
    echo ""
    echo "❌ Withdrawal failed. Response:"
    echo "$WITHDRAW_RES"
    exit 1
fi

echo ""
echo "═══════════════════════════════════════════════════════"
echo "  ✅ FULL ZK-MIXER FLOW COMPLETE!"
echo ""
echo "  1. Stealth Wallet: $STEALTH_ADDR"
echo "  2. Deposit to Pool: $POOL_TX"
echo "  3. Withdrawal Tx:   $WITHDRAW_TX"
echo "  4. Recipient:       $MAIN_WALLET"
echo ""
echo "  Privacy achieved via the PrivacyPool contract."
echo "  No centralized custodian. 100% on-chain."
echo "═══════════════════════════════════════════════════════"
