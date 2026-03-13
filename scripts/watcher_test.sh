#!/usr/bin/env bash

# CloakFund Phase 2 - Watcher Integration Demo Script
# This script creates a paylink, registers an ephemeral address,
# sends a test transaction (if configured), and polls the Convex API
# to demonstrate that the Rust watcher detects and stores the deposit.

set -e

# Change to project root
cd "$(dirname "$0")/.."

echo "====================================================="
echo " CloakFund Phase 2 - Watcher Integration Demo Script "
echo "====================================================="

# Check dependencies
if ! command -v npx &> /dev/null; then
    echo "❌ npx not found. Please install Node.js and run 'npm install'."
    exit 1
fi

if ! command -v cast &> /dev/null; then
    echo "⚠️ 'cast' (Foundry) not found. Manual transaction will be required."
    HAS_CAST=false
else
    HAS_CAST=true
fi

echo "1. Creating a new Paylink in Convex..."
# Generate a dummy 33-byte compressed public key for the mock recipient
MOCK_PUB="0x02$(openssl rand -hex 32)"
PAYLINK_OUT=$(npx convex run paylinks:create '{"recipientPublicKeyHex": "'$MOCK_PUB'", "ensName": "demo-watcher.eth"}' --no-color)

# Extract the paylinkId from the Convex JS object output
PAYLINK_ID=$(echo "$PAYLINK_OUT" | grep -o 'paylinkId: "[^"]*' | cut -d'"' -f2 | head -n 1)

if [ -z "$PAYLINK_ID" ]; then
    echo "❌ Failed to extract paylinkId. Output:"
    echo "$PAYLINK_OUT"
    exit 1
fi
echo "✅ Created Paylink ID: $PAYLINK_ID"

echo "2. Generating a test stealth address..."
# Generate a random 20-byte hex string for EVM address
STEALTH_ADDR="0x$(openssl rand -hex 20)"
MOCK_EPHEM_PUB="0x03$(openssl rand -hex 32)"

npx convex run paylinks:createEphemeralAddress '{"paylinkId": "'$PAYLINK_ID'", "stealthAddress": "'$STEALTH_ADDR'", "ephemeralPubkeyHex": "'$MOCK_EPHEM_PUB'", "viewTag": 10}' --no-color > /dev/null

echo "✅ Registered Stealth Address: $STEALTH_ADDR for Paylink $PAYLINK_ID"

echo "3. Sending a test transaction..."

if [ "$HAS_CAST" = true ] && [ -n "$PRIVATE_KEY" ] && [ -n "$BASE_RPC_URL" ]; then
    echo "   Using cast to send 0.00001 ETH to $STEALTH_ADDR..."
    cast send "$STEALTH_ADDR" --value 0.00001ether --rpc-url "$BASE_RPC_URL" --private-key "$PRIVATE_KEY"
    echo "✅ Transaction sent!"
else
    echo "⚠️ Skipping automatic transaction."
    echo "   To test the watcher, manually send a small amount of ETH/tokens to:"
    echo "   $STEALTH_ADDR"
    echo "   (Make sure you are sending on the network the watcher is currently monitoring!)"
fi

echo "4. Polling Convex for deposit detection..."
echo "   (Ensure 'cargo run --bin rust-backend serve' is running in another terminal window)"

ATTEMPTS=0
MAX_ATTEMPTS=30

while [ $ATTEMPTS -lt $MAX_ATTEMPTS ]; do
    sleep 4
    ATTEMPTS=$((ATTEMPTS + 1))

    STATUS_OUT=$(npx convex run deposits:getDepositStatus '{"paylinkId": "'$PAYLINK_ID'"}' --no-color)

    # Check if the output contains a deposit object
    if echo "$STATUS_OUT" | grep -q 'depositId: "'; then
        echo ""
        echo "🎉 Deposit Detected!"
        echo "========================================"
        echo "$STATUS_OUT"
        echo "========================================"
        echo "✅ Watcher successfully indexed the deposit into Convex."
        exit 0
    fi

    echo -n "."
done

echo ""
echo "❌ Timed out waiting for deposit. Check the Rust watcher logs."
exit 1
