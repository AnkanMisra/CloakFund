# CloakFund API Documentation

This document outlines the REST API endpoints exposed by the CloakFund Rust backend.

## Base URL
`http://localhost:3000` (or configured API domain via `FRONTEND_URL`/`BIND_ADDR`)

---

## Paylink Management (Phase 3)

### Create Paylink
`POST /api/v1/paylink`

Generates a new paylink and the associated deterministic stealth payment address for the receiver. This stores the generated ephemeral address and public key in Convex.

**Request Body (JSON):**
```json
{
  "ensName": "alice.eth",                  // Optional: User's ENS name
  "recipientPublicKeyHex": "0x034f...",    // Required: Receiver's public key
  "metadata": { "item": "Coffee" },        // Optional: Custom metadata object
  "chainId": 8453,                         // Optional: Defaults to 8453 (Base)
  "network": "base"                        // Optional: Defaults to "base"
}
```

**Response (201 Created):**
```json
{
  "paylinkId": "jd18xyz...",
  "stealthAddress": "0x1234abcd...",
  "ephemeralPubkeyHex": "0x0288eff..."
}
```

---

### Get Paylink
`GET /api/v1/paylink/:id`

Retrieves the paylink details, status, and all associated ephemeral addresses stored in the system.

**Response (200 OK):**
```json
{
  "_id": "jd18xyz...",
  "_creationTime": 1700000000000,
  "userId": null,
  "ensName": "alice.eth",
  "recipientPublicKeyHex": "0x034f...",
  "status": "active",
  "metadata": { "item": "Coffee" },
  "chainId": 8453,
  "network": "base",
  "ephemeralAddresses": [
    {
      "_id": "jh1abc...",
      "_creationTime": 1700000000100,
      "paylinkId": "jd18xyz...",
      "stealthAddress": "0x1234abcd...",
      "ephemeralPubkeyHex": "0x0288eff...",
      "viewTag": 142,
      "chainId": 8453,
      "network": "base",
      "status": "announced"
    }
  ]
}
```

---

## Core & Monitoring

### Health Check
`GET /health`

Returns backend health status, verifying that the Rust API layer is responding.

**Response (200 OK):**
```json
{
  "status": "ok",
  "service": "cloakfund-rust-backend",
  "timestamp": 1700000000
}
```

---

## Deposits (Phase 2 / Convex)

### Get Deposit Status
`GET /deposit-status?paylinkId=<id>` 

Returns payment confirmations, detected deposits, and aggregated balances (served via Convex HTTP actions or direct WebSockets to the frontend).

---

## Future Endpoints (Phases 5 & 6)

### Consolidate Funds
`POST /consolidate`
Triggers treasury consolidation to move funds from ephemeral addresses to the BitGo MPC vault.

### List Receipts
`GET /receipts`
Returns a list of encrypted receipts referencing Fileverse pointers for a given user or paylink.