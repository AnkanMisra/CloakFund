# 📡 API Reference

> All endpoints are served by the Rust backend (Axum) and consumed by the Next.js frontend.

---

## Base URL

```
https://api.cloakfund.xyz/api/v1
```

> Local development: `http://localhost:3001/api/v1`

---

## Endpoints

### `POST /paylink`

Generate a stealth payment address for a recipient.

**Request:**
```json
{
  "ens_name": "alice.eth",
  "metadata": {
    "memo": "Invoice #1042",
    "amount_hint": "0.5 ETH"
  }
}
```

**Response:**
```json
{
  "paylink_id": "pl_8f3a2b1c",
  "stealth_address": "0x91AF...3D4E",
  "ephemeral_pub": "0x04A1B2...F3E4",
  "payment_url": "https://cloakfund.xyz/pay/pl_8f3a2b1c",
  "created_at": "2026-03-13T12:00:00Z"
}
```

| Field | Description |
| ----- | ----------- |
| `paylink_id` | Unique identifier for this payment request |
| `stealth_address` | One-time address the sender should transfer funds to |
| `ephemeral_pub` | Ephemeral public key (needed for recipient to recover funds) |
| `payment_url` | Shareable URL / QR code target |

---

### `GET /deposit-status`

Check the confirmation status of a payment.

**Request:**
```
GET /deposit-status?paylink_id=pl_8f3a2b1c
```

**Response:**
```json
{
  "paylink_id": "pl_8f3a2b1c",
  "status": "confirmed",
  "tx_hash": "0xABCD...1234",
  "block_number": 12345678,
  "amount": "500000000000000000",
  "confirmations": 12,
  "detected_at": "2026-03-13T12:05:30Z"
}
```

| Status | Meaning |
| ------ | ------- |
| `pending` | Paylink created, no deposit detected yet |
| `detected` | Deposit seen, awaiting confirmations |
| `confirmed` | Deposit confirmed with sufficient block depth |

---

### `POST /consolidate`

Trigger treasury consolidation — move funds from stealth addresses to BitGo MPC vault.

**Request:**
```json
{
  "paylink_ids": ["pl_8f3a2b1c", "pl_2d4e6f8a"],
  "destination": "bitgo_vault_primary"
}
```

**Response:**
```json
{
  "job_id": "cj_a1b2c3d4",
  "status": "pending",
  "tx_hashes": [],
  "estimated_total": "1.5 ETH",
  "created_at": "2026-03-13T14:00:00Z"
}
```

| Job Status | Meaning |
| ---------- | ------- |
| `pending` | Consolidation job created |
| `signed` | Transaction signed by MPC |
| `broadcasted` | Transaction submitted to blockchain |
| `confirmed` | Funds arrived in treasury vault |
| `failed` | Error occurred — see error field |

---

### `GET /receipts`

Retrieve encrypted receipt list for a user.

**Request:**
```
GET /receipts?ens_name=alice.eth
```

**Response:**
```json
{
  "receipts": [
    {
      "receipt_id": "rc_x1y2z3",
      "paylink_id": "pl_8f3a2b1c",
      "fileverse_pointer": "fv://abc123def456",
      "encrypted": true,
      "created_at": "2026-03-13T12:06:00Z"
    }
  ],
  "total": 1
}
```

> **Note:** Receipt contents are encrypted. The frontend fetches the ciphertext from Fileverse and decrypts **client-side** using the user's key.

---

## Authentication

| Method | Use Case |
| ------ | -------- |
| Wallet signature (`EIP-712`) | User authentication for receipt access |
| API token (optional) | Rate-limited paylink creation |

---

## Error Format

All errors return a consistent JSON structure:

```json
{
  "error": {
    "code": "PAYLINK_NOT_FOUND",
    "message": "No paylink found with the given ID",
    "status": 404
  }
}
```

---

→ See [SYSTEM-DESIGN.md](./SYSTEM-DESIGN.md) for how these endpoints fit into the event flow.
