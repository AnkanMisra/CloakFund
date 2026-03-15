#!/usr/bin/env bash

# CloakFund Phase 4 - Sweeper Integration Demo Script
# This script demonstrates the BitGo consolidation flow by creating a paylink,
# waiting for a deposit, manually triggering the sweep job, and monitoring its completion.

set -e

# Change to project root
cd "$(dirname "$0")/.."

echo "====================================================="
echo " CloakFund Phase 4 - Sweeper Integration Demo Script "
echo "====================================================="

if ! command -v npx &> /dev/null; then
    echo "❌ npx not found. Please install Node.js."
    exit 1
fi
if ! command -v curl &> /dev/null; then
    echo "❌ curl not found."
    exit 1
fi

echo "Make sure you have both the Convex dev server and the Rust API running:"
echo "Terminal 1: npm run convex:dev"
echo "Terminal 2: cd rust-backend && cargo run -- serve"
echo ""

echo "1. Creating a new Paylink via Rust API..."
# We must use Wallet 2's uncompressed public key so the Stealth Address math
# aligns perfectly with Ethereum ECDSA derivation logic on the backend!
MOCK_PUB="0x04b10912af0c04aa473bebc86f36f44eed2bbbc6bcad611287140975fafe159974b8ac6bccd806e4647e45eda540d9ae05aed61ebff5d0bff409e813d2ad33d7f6"

PAYLINK_RES=$(curl -s -X POST http://localhost:8080/api/v1/paylink \
    -H "Content-Type: application/json" \
    -d '{"recipientPublicKeyHex": "'$MOCK_PUB'", "ensName": "sweeper-demo.eth", "chainId": 84532, "network": "base-sepolia"}')

PAYLINK_ID=$(echo "$PAYLINK_RES" | grep -o '"paylinkId":"[^"]*' | cut -d'"' -f4)
STEALTH_ADDR=$(echo "$PAYLINK_RES" | grep -o '"stealthAddress":"[^"]*' | cut -d'"' -f4)

if [ -z "$PAYLINK_ID" ] || [ "$PAYLINK_ID" == "null" ]; then
    echo "❌ Failed to create paylink via API. Is the Rust server running?"
    echo "Response: $PAYLINK_RES"
    exit 1
fi

echo "✅ Created Paylink ID: $PAYLINK_ID"
echo "✅ Stealth Address: $STEALTH_ADDR"
echo ""

echo "2. Sending 0.00001 ETH to stealth address..."

# Auto-send if SENDER_PRIVATE_KEY is configured
if node scripts/send_eth.mjs "$STEALTH_ADDR" 0.00001 2>/dev/null; then
    echo "✅ ETH sent successfully"
else
    echo "   ⚠️  Auto-send failed. Send manually:"
    echo "   Please send a small amount of ETH/tokens to: $STEALTH_ADDR"
    echo "   (Or set SENDER_PRIVATE_KEY in .env to a funded Base Sepolia wallet)"
fi
echo ""

ATTEMPTS=0
MAX_ATTEMPTS=60
DEPOSIT_ID=""

while [ $ATTEMPTS -lt $MAX_ATTEMPTS ]; do
    sleep 5
    ATTEMPTS=$((ATTEMPTS + 1))

    STATUS_OUT=$(npx convex run deposits:getDepositStatus '{"paylinkId": "'$PAYLINK_ID'"}' --no-color 2>/dev/null || echo "")

    DEPOSIT_ID=$(echo "$STATUS_OUT" | grep -o 'depositId: "[^"]*' | cut -d'"' -f2 | head -n 1)

    if [ -n "$DEPOSIT_ID" ]; then
        echo ""
        echo "🎉 Deposit Detected! ID: $DEPOSIT_ID"
        break
    fi

    echo -n "."
done

if [ -z "$DEPOSIT_ID" ]; then
    echo ""
    echo "❌ Timed out waiting for deposit. Exiting."
    exit 1
fi

echo ""
echo "3. Triggering BitGo Consolidation (Sweep Job)..."
SWEEP_RES=$(curl -s -X POST http://localhost:8080/api/v1/consolidate \
    -H "Content-Type: application/json" \
    -d '{"deposit_id": "'$DEPOSIT_ID'"}')

JOB_ID=$(echo "$SWEEP_RES" | grep -o '"job_id":"[^"]*' | cut -d'"' -f4)

if [ -z "$JOB_ID" ] || [ "$JOB_ID" == "null" ]; then
    echo "❌ Failed to queue sweep job. Response: $SWEEP_RES"
    exit 1
fi

echo "✅ Queued Sweep Job ID: $JOB_ID"
echo ""

echo "4. Polling for Sweep Job Completion..."
S_ATTEMPTS=0
MAX_S_ATTEMPTS=20

while [ $S_ATTEMPTS -lt $MAX_S_ATTEMPTS ]; do
    sleep 3
    S_ATTEMPTS=$((S_ATTEMPTS + 1))

    JOBS_OUT=$(npx convex run sweeps:getAllSweepJobs --no-color 2>/dev/null || echo "")

    if echo "$JOBS_OUT" | grep -A 5 "$JOB_ID" | grep -q 'status: "completed"'; then
        TX_HASH=$(echo "$JOBS_OUT" | grep -A 5 "$JOB_ID" | grep -o 'sweepTxHash: "[^"]*' | cut -d'"' -f2 | head -n 1)
        echo ""
        echo "✅ Sweep Job Completed!"
        echo "🔗 Sweep Tx Hash: $TX_HASH"
        echo "🎉 Phase 4 Consolidation successful."
        exit 0
    elif echo "$JOBS_OUT" | grep -A 5 "$JOB_ID" | grep -q 'status: "failed"'; then
        echo ""
        echo "❌ Sweep Job Failed. Check Rust backend logs for 'sweeper' errors."
        echo "   Make sure 'RECIPIENT_PRIVATE_KEY_HEX' is correctly set in your backend environment."
        exit 1
    fi

    echo -n "."
done

echo ""
echo "❌ Timed out waiting for sweep job to complete. It might still be broadcasting."
exit 1
