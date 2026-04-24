#!/usr/bin/env bash

# CloakFund — Paylink Expiry & Revocation Integration Test
#
# End-to-end checks for the `expiresAt` / `revoked` feature:
#   1. Create a paylink with expires_in_seconds=2, assert usable=true.
#   2. Wait past the expiry, assert usable=false.
#   3. Create a paylink with no expiry, revoke with the wrong token -> 403.
#   4. Revoke with the correct token -> 200, then GET returns usable=false.
#   5. Attempting to revoke an already-revoked paylink -> 409.
#
# The script requires a running Convex dev deployment and the Rust API bound
# to API_URL (default http://localhost:8080). If neither is reachable, the
# script prints SKIPPED and exits 0 so that `cargo test` workflows don't fail
# when integration services aren't available.

set -u  # intentionally NOT -e — we want to print context on failure

cd "$(dirname "$0")/.." || { echo "Failed to cd to repo root"; exit 1; }

API_URL="${API_URL:-http://localhost:8080}"
MOCK_PUB="${RECIPIENT_PUBLIC_KEY_HEX:-0x04b10912af0c04aa473bebc86f36f44eed2bbbc6bcad611287140975fafe159974b8ac6bccd806e4647e45eda540d9ae05aed61ebff5d0bff409e813d2ad33d7f6}"

# Private per-run temp files for curl response bodies. Avoids collisions when
# the suite runs in parallel CI matrices and removes world-readable leftovers.
TMP_BAD=$(mktemp)
TMP_OK=$(mktemp)
TMP_AGAIN=$(mktemp)
trap 'rm -f "$TMP_BAD" "$TMP_OK" "$TMP_AGAIN"' EXIT

pass=0
fail=0

say()  { printf "\n▸ %s\n" "$*"; }
ok()   { printf "  ✅ %s\n" "$*"; pass=$((pass+1)); }
bad()  { printf "  ❌ %s\n" "$*"; fail=$((fail+1)); }
info() { printf "     %s\n" "$*"; }

if ! command -v curl >/dev/null 2>&1; then
    echo "SKIPPED: curl not found."
    exit 0
fi

if ! curl -fsS -m 2 "$API_URL/health" >/dev/null 2>&1; then
    echo "SKIPPED: Rust API not reachable at $API_URL (need to run 'cargo run -- serve')."
    exit 0
fi

# Extract JSON field value using python3 (stdlib). Avoids jq dependency.
json_field() {
    local field="$1"
    python3 -c "
import json, sys
try:
    d = json.load(sys.stdin)
except Exception:
    sys.exit(2)
cur = d
for part in sys.argv[1].split('.'):
    if isinstance(cur, dict) and part in cur:
        cur = cur[part]
    else:
        sys.exit(3)
if isinstance(cur, bool):
    print('true' if cur else 'false')
else:
    print(cur)
" "$field"
}

# Step 1: create an expiring paylink.
say "Creating paylink with expires_in_seconds=2"
CREATE_RES=$(curl -fsS -X POST "$API_URL/api/v1/paylink" \
    -H "Content-Type: application/json" \
    -d "{\"recipientPublicKeyHex\": \"$MOCK_PUB\", \"chainId\": 84532, \"network\": \"base-sepolia\", \"expiresInSeconds\": 2}")
info "Response: $CREATE_RES"

PAYLINK_ID=$(echo "$CREATE_RES" | json_field paylinkId) || { bad "Failed to parse paylinkId"; exit 1; }
REVOCATION_TOKEN=$(echo "$CREATE_RES" | json_field revocationToken) || { bad "Missing revocationToken"; exit 1; }

if [ -n "$PAYLINK_ID" ] && [ "$PAYLINK_ID" != "None" ]; then
    ok "Created paylink $PAYLINK_ID"
else
    bad "No paylink ID returned"
    exit 1
fi

if [ -n "$REVOCATION_TOKEN" ] && [[ "$REVOCATION_TOKEN" == 0x* ]]; then
    ok "Received revocation token (${#REVOCATION_TOKEN} chars)"
else
    bad "Revocation token missing or malformed: $REVOCATION_TOKEN"
fi

# Step 2: freshly-created paylink should be usable.
say "GET paylink immediately — expect usable=true"
GET1=$(curl -fsS "$API_URL/api/v1/paylink/$PAYLINK_ID")
USABLE1=$(echo "$GET1" | json_field usable)
REVOKED1=$(echo "$GET1" | json_field revoked)
if [ "$USABLE1" = "true" ] && [ "$REVOKED1" = "false" ]; then
    ok "usable=true revoked=false"
else
    bad "expected usable=true revoked=false, got usable=$USABLE1 revoked=$REVOKED1"
    info "$GET1"
fi

# Step 3: wait past expiry.
say "Waiting 3s for expiry…"
sleep 3
GET2=$(curl -fsS "$API_URL/api/v1/paylink/$PAYLINK_ID")
USABLE2=$(echo "$GET2" | json_field usable)
if [ "$USABLE2" = "false" ]; then
    ok "usable=false after expiry"
else
    bad "expected usable=false after expiry, got usable=$USABLE2"
    info "$GET2"
fi

# Step 4: create a second paylink (no expiry) and try the revoke flow.
say "Creating second paylink with no expiry"
CREATE2=$(curl -fsS -X POST "$API_URL/api/v1/paylink" \
    -H "Content-Type: application/json" \
    -d "{\"recipientPublicKeyHex\": \"$MOCK_PUB\", \"chainId\": 84532, \"network\": \"base-sepolia\"}")
PAYLINK_ID_2=$(echo "$CREATE2" | json_field paylinkId)
REVOCATION_TOKEN_2=$(echo "$CREATE2" | json_field revocationToken)
if [ -n "$PAYLINK_ID_2" ] && [ -n "$REVOCATION_TOKEN_2" ] && [[ "$REVOCATION_TOKEN_2" == 0x* ]]; then
    ok "Created paylink $PAYLINK_ID_2"
else
    bad "Failed to create second paylink or missing revocationToken: $CREATE2"
    exit 1
fi

# Step 5: revoke with wrong token -> 403.
say "Revoking with WRONG token — expect 403"
WRONG_TOKEN="0x$(printf '0%.0s' {1..64})"
BAD_HTTP=$(curl -s -o "$TMP_BAD" -w "%{http_code}" -X POST \
    "$API_URL/api/v1/paylink/$PAYLINK_ID_2/revoke" \
    -H "Content-Type: application/json" \
    -d "{\"revocationToken\": \"$WRONG_TOKEN\"}")
if [ "$BAD_HTTP" = "403" ]; then
    ok "got HTTP 403"
else
    bad "expected 403, got $BAD_HTTP — body: $(cat "$TMP_BAD")"
fi

# Step 6: revoke with correct token -> 200.
say "Revoking with CORRECT token — expect 200"
GOOD_HTTP=$(curl -s -o "$TMP_OK" -w "%{http_code}" -X POST \
    "$API_URL/api/v1/paylink/$PAYLINK_ID_2/revoke" \
    -H "Content-Type: application/json" \
    -d "{\"revocationToken\": \"$REVOCATION_TOKEN_2\"}")
if [ "$GOOD_HTTP" = "200" ]; then
    ok "got HTTP 200"
else
    bad "expected 200, got $GOOD_HTTP — body: $(cat "$TMP_OK")"
fi

# Step 7: paylink is now revoked + not usable.
say "GET second paylink — expect revoked=true usable=false"
GET3=$(curl -fsS "$API_URL/api/v1/paylink/$PAYLINK_ID_2")
USABLE3=$(echo "$GET3" | json_field usable)
REVOKED3=$(echo "$GET3" | json_field revoked)
if [ "$REVOKED3" = "true" ] && [ "$USABLE3" = "false" ]; then
    ok "revoked=true usable=false"
else
    bad "expected revoked=true usable=false, got revoked=$REVOKED3 usable=$USABLE3"
    info "$GET3"
fi

# Step 8: double-revoke -> 409.
say "Re-revoking — expect 409"
AGAIN_HTTP=$(curl -s -o "$TMP_AGAIN" -w "%{http_code}" -X POST \
    "$API_URL/api/v1/paylink/$PAYLINK_ID_2/revoke" \
    -H "Content-Type: application/json" \
    -d "{\"revocationToken\": \"$REVOCATION_TOKEN_2\"}")
if [ "$AGAIN_HTTP" = "409" ]; then
    ok "got HTTP 409 on correct-token re-revoke"
else
    bad "expected 409, got $AGAIN_HTTP — body: $(cat "$TMP_AGAIN")"
fi

# Step 9: already-revoked paylink with WRONG token must also return 409 —
# proves the revoke check runs before the token check so an attacker cannot
# use the 403/409 split as a token-confirmation oracle.
say "Re-revoking with WRONG token — expect 409 (no 403 oracle)"
AGAIN_WRONG_HTTP=$(curl -s -o "$TMP_AGAIN" -w "%{http_code}" -X POST \
    "$API_URL/api/v1/paylink/$PAYLINK_ID_2/revoke" \
    -H "Content-Type: application/json" \
    -d "{\"revocationToken\": \"$WRONG_TOKEN\"}")
if [ "$AGAIN_WRONG_HTTP" = "409" ]; then
    ok "got HTTP 409 on wrong-token re-revoke (oracle closed)"
else
    bad "expected 409 (oracle closed), got $AGAIN_WRONG_HTTP — body: $(cat "$TMP_AGAIN")"
fi

echo ""
echo "-----------------------------------------"
echo "  $pass passed   $fail failed"
echo "-----------------------------------------"

if [ "$fail" -gt 0 ]; then
    exit 1
fi
exit 0
