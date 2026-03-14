#!/bin/bash

echo "========================================="
echo "       CloakFund Terminal Interface      "
echo "========================================="

echo "Enter your Main Wallet ENS or Name (e.g., alice.eth):"
read ENS_NAME

# Using the mock public key derived from the backend config for this testnet demo
MOCK_PUB="0x04b10912af0c04aa473bebc86f36f44eed2bbbc6bcad611287140975fafe159974b8ac6bccd806e4647e45eda540d9ae05aed61ebff5d0bff409e813d2ad33d7f6"

echo "Generating temporary stealth wallet for $ENS_NAME..."

PAYLINK_RES=$(curl -s -X POST http://localhost:8080/api/v1/paylink \
    -H "Content-Type: application/json" \
    -d '{"recipientPublicKeyHex": "'$MOCK_PUB'", "ensName": "'$ENS_NAME'", "chainId": 84532, "network": "base-sepolia"}')

PAYLINK_ID=$(echo "$PAYLINK_RES" | grep -o '"paylinkId":"[^"]*' | cut -d'"' -f4)
STEALTH_ADDR=$(echo "$PAYLINK_RES" | grep -o '"stealthAddress":"[^"]*' | cut -d'"' -f4)

if [ -z "$PAYLINK_ID" ] || [ "$PAYLINK_ID" == "null" ]; then
    echo "❌ Failed to create temporary wallet. Make sure the Rust backend (cargo run -- serve) is running in another terminal!"
    exit 1
fi

echo "========================================="
echo "✅ Temporary Wallet Created!"
echo "Share this address with the sender:"
echo "👉 $STEALTH_ADDR"
echo "========================================="

echo "Sending 0.0001 testnet ETH to the temporary wallet to simulate a payment..."
SEND_OUT=$(node scripts/send_eth.mjs "$STEALTH_ADDR" 0.0001)

# Extract tx hash printed by scripts/send_eth.mjs
TX_HASH=$(echo "$SEND_OUT" | grep -Eo '0x[a-fA-F0-9]{64}' | head -n 1)

echo "$SEND_OUT"
echo ""

if [ -z "$TX_HASH" ]; then
    echo "❌ Could not extract tx hash from send output. Cannot proceed with tx-hash polling."
    exit 1
fi

echo "Waiting for the CloakFund Agent to detect the payment..."
echo "Polling deposits by tx hash (more reliable than paylink polling): $TX_HASH"
echo "(This avoids missing deposits if paylink-based polling lags.)"

ATTEMPTS=0
DEPOSIT_ID=""
while [ $ATTEMPTS -lt 80 ]; do
    sleep 3
    ATTEMPTS=$((ATTEMPTS + 1))

    # Query deposits by tx hash (requires convex query: deposits:getDepositsByTxHash)
    TX_OUT=$(npx convex run deposits:getDepositsByTxHash '{"txHash": "'$TX_HASH'"}' --no-color 2>/dev/null || echo "")

    # First depositId in the array (Convex prints as `depositId: "..."`)
    DEPOSIT_ID=$(echo "$TX_OUT" | grep -o 'depositId: "[^"]*' | cut -d'"' -f2 | head -n 1)

    if [ -n "$DEPOSIT_ID" ]; then
        echo ""
        echo "🎉 Deposit recorded! Deposit ID: $DEPOSIT_ID"
        break
    fi

    echo -n "."
done

if [ -z "$DEPOSIT_ID" ]; then
    echo ""
    echo "❌ Timed out waiting for deposit to appear in Convex for tx hash: $TX_HASH"
    echo "   Check Rust watcher logs and Convex dev output."
    exit 1
fi

echo "🤖 Initializing sweep (temporary → BitGo dynamic deposit address)..."
SWEEP_RES=$(curl -s -X POST http://localhost:8080/api/v1/consolidate \
    -H "Content-Type: application/json" \
    -d '{"deposit_id": "'$DEPOSIT_ID'"}')

JOB_ID=$(echo "$SWEEP_RES" | grep -o '"job_id":"[^"]*' | cut -d'"' -f4)

if [ -z "$JOB_ID" ] || [ "$JOB_ID" == "null" ]; then
    echo "❌ Failed to queue sweep job. Response:"
    echo "$SWEEP_RES"
    exit 1
fi

echo "✅ Sweep Job Initiated! Job ID: $JOB_ID"
echo "Check your Rust backend terminal for:"
echo "  - Generated BitGo destination address"
echo "  - Sweep tx hash"
