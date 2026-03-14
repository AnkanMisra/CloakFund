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
# We run the send_eth script to simulate the sender
node scripts/send_eth.mjs "$STEALTH_ADDR" 0.0001
echo ""
echo "Waiting for the CloakFund Agent to detect the payment and automatically trigger the sweep to your main wallet..."
echo "(This acts like an automated smart contract to protect your privacy. Waiting for 2 block confirmations...)"

# Watch for funds and auto-sweep
ATTEMPTS=0
while [ $ATTEMPTS -lt 60 ]; do
    sleep 3
    ATTEMPTS=$((ATTEMPTS + 1))
    STATUS_OUT=$(npx convex run deposits:getDepositStatus '{"paylinkId": "'$PAYLINK_ID'"}' --no-color 2>/dev/null || echo "")
    DEPOSIT_ID=$(echo "$STATUS_OUT" | grep -o 'depositId: "[^"]*' | cut -d'"' -f2 | head -n 1)

    if [ -n "$DEPOSIT_ID" ]; then
        echo "🎉 PAYMENT DETECTED AND CONFIRMED! Deposit ID: $DEPOSIT_ID"
        echo "🤖 Auto-Sweeper (Off-Chain Smart Contract Agent) is now transferring funds to your main wallet..."
        
        # Trigger the sweeper automatically
        SWEEP_RES=$(curl -s -X POST http://localhost:8080/api/v1/consolidate -H "Content-Type: application/json" -d '{"deposit_id": "'$DEPOSIT_ID'"}')
        JOB_ID=$(echo "$SWEEP_RES" | grep -o '"job_id":"[^"]*' | cut -d'"' -f4)
        
        echo "✅ Transfer Job Initiated! Job ID: $JOB_ID"
        echo "Check your Rust backend terminal for the final transaction hash."
        exit 0
    fi
    echo -n "."
done

echo ""
echo "❌ Timed out waiting for deposit. Check your internet connection or the Rust backend logs."
