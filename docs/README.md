# CloakFund

CloakFund is a privacy-first payment and treasury infrastructure for Web3.

It allows users to receive payments through a public ENS identity while preventing observers from tracing their wallet balances and transaction history.

Instead of sending funds to a single wallet address, CloakFund generates a new stealth address for every payment. These addresses appear unrelated on-chain but are aggregated in the CloakFund dashboard.

Funds can then be consolidated into a secure BitGo MPC treasury vault. Payment receipts are encrypted and stored using Fileverse.

The frontend is built using Next.js with TypeScript (TSX), while the backend is implemented entirely in Rust.

---

## Running Locally

Please see [RUN_INSTRUCTIONS.md](RUN_INSTRUCTIONS.md) for how to set up and run the Convex layer, Rust backend, and the Next.js frontend using Bun.

---

## Project Status

### Phase 0 — Preparation & Access
Completed.

Delivered:
- `.env.example`
- `docs/SECRETS_SETUP.md`
- `docs/test_wallets.md`

### Phase 1 — Core Rust Backend: Stealth Generator Module
Completed and reviewed.

Delivered:
- `rust-backend/src/stealth.rs`
- `rust-backend/src/main.rs`
- `docs/CRYPTOGRAPHY.md`
- `docs/RUST_BACKEND_DESIGN.md`
- `docs/CRYPTO_TEST_VECTORS.md`

Implemented:
- Stealth address generation using ECDH on secp256k1
- Recipient-side stealth private key recovery
- EIP-55 checksum address formatting
- View tag generation and return value support
- CLI commands for `generate` and `recover`
- Roundtrip and negative unit tests

Verification:
- Rust test suite passes for Phase 1
- Deterministic cryptographic vectors documented
- Phase 0 and Phase 1 were re-reviewed before moving forward

### Phase 2 — Deposit Watcher & Architecture Redesign
Completed and reviewed.

Delivered:
- `docs/PHASE_2_ARCHITECTURE_REDESIGN.md`
- `convex/` schema and backend TS functions
- `rust-backend/src/watcher.rs`
- `rust-backend/src/convex_client.rs`

Implemented:
- Pivot to Convex real-time database from Postgres
- Rust RPC indexer using `ethers-rs`
- Dedicated database schemas for checkpoints, paylinks, and deposits
- Blockchain reorg handling and rate-limited historical catch-up

### Phase 3 — Paylink API & Persistence
Completed and reviewed.

Delivered:
- `rust-backend/src/api.rs`
- `docs/API.md`

Implemented:
- Axum HTTP server setup
- `POST /api/v1/paylink` with atomic Convex mutations
- `GET /api/v1/paylink/:id` endpoint
- End-to-end integration of stealth address generation with Convex persistence

---

## Key Features

Stealth payments  
ENS identity layer  
Secure treasury vault  
Encrypted payment receipts  
Optional AI monitoring agent  

---

## Core Idea

Users keep a single public identity:

alice.eth

But each payment goes to a different address:

Payment 1 → 0x91AF  
Payment 2 → 0x4B3C  
Payment 3 → 0xAA1D  

The blockchain cannot link them together.

---

## Tech Stack

Frontend  
Next.js (TypeScript)

Backend  
Rust (Axum + Tokio)

Blockchain  
Base Network

Integrations  
ENS  
BitGo  
Fileverse  
HeyElsa
