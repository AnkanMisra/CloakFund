# Rust Backend Design

The backend is implemented entirely in Rust.

Rust was chosen for:

memory safety  
cryptographic reliability  
concurrency performance

---

## Core Modules

API Server

Handles frontend requests.

Stealth Generator

Uses ECDH cryptography to derive one-time addresses.

Watcher Service

Listens for blockchain events using ethers-rs and syncs deposits to the Convex backend.

Convex Client

Acts as the bridge between the Rust backend and the Convex real-time database.

Treasury Engine

Constructs consolidation transactions.

Encryption Service

Encrypts receipts before storage.

---

## CLI Usage

The Rust backend includes a CLI for running the application and interacting with the stealth module.

**Start the API server and blockchain watcher:**

- `cargo run --manifest-path rust-backend/Cargo.toml -- serve`

**Generate stealth payment data** (address + ephemeral pubkey + view tag):

- `cargo run --manifest-path rust-backend/Cargo.toml -- generate <recipient_pubkey_hex>`

Recover stealth private key (recipient side), using the recipient’s private key and the published ephemeral public key:

- `cargo run --manifest-path rust-backend/Cargo.toml -- recover <recipient_priv_hex> <ephemeral_pub_hex>`

Notes:
- Inputs may be provided with or without a `0x` prefix.
- Never paste real mainnet private keys into terminals/logs; use test keys only.

---

## Async Runtime

Tokio is used to manage concurrent services.

Watcher and API run as separate async tasks, both utilizing a shared Convex client for state persistence.
