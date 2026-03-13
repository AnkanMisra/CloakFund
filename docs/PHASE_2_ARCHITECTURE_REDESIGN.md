# CloakFund Phase 2: Architecture Redesign

## 1. Executive Summary

This document outlines the redesigned architecture for the CloakFund MVP. The previous temporary-address forwarding system has been replaced with a more robust privacy architecture inspired by Vitalik Buterin's stealth address concepts. This new design maintains deep integration with BitGo for enterprise-grade treasury custody while ensuring that payer-to-payee links cannot be established on public block explorers.

The system relies on a modular Rust backend, a real-time Convex data layer, and a Next.js frontend, ensuring a flow that is both highly secure and easy to demonstrate.

---

## 2. Revised System Architecture

CloakFund operates as an event-driven system distributed across five key layers.

### The Five Layers

1. **Frontend Application Layer**
   - **Tech Stack**: Next.js, React, TypeScript, Wagmi, RainbowKit.
   - **Role**: Provides the user interface for identity connection (ENS), paylink generation, deposit tracking, and receipt decryption.

2. **Core API & Logic Layer**
   - **Tech Stack**: Rust (Axum, Tokio).
   - **Role**: Coordinates the generation of stealth data, exposes API endpoints to the frontend, manages sweeping queues, and handles receipt encryption.

3. **Persistence & Real-time Data Layer**
   - **Tech Stack**: Convex (TypeScript backend functions + Managed DB).
   - **Role**: Stores `paylinks`, `ephemeral_addresses`, `deposits`, and `receipts`. Exposes real-time WebSocket subscriptions directly to the frontend to eliminate HTTP polling.

4. **Blockchain Layer**
   - **Tech Stack**: Base (Ethereum L2), Ethers-rs.
   - **Role**: The execution environment where actual asset transfers occur to the generated stealth addresses.

5. **Enterprise Custody Layer**
   - **Tech Stack**: BitGo REST API / MPC Wallets.
   - **Role**: Secures the consolidated treasury funds after they are swept from the ephemeral stealth addresses.

---

## 3. Component Diagram

```text
+-----------------------+      +------------------------+      +-----------------------+
|                       |      |                        |      |                       |
|   Next.js Frontend    |----->|  Rust API Server       |----->|  Convex Data Layer    |
|   (Wagmi/RainbowKit)  |<-----|  (Axum HTTP)           |<-----|  (Real-time DB)       |
|                       |      |                        |      |                       |
+-----------+-----------+      +-----------+------------+      +-----------+-----------+
            |                              |                               ^
            | (Wallet Tx)                  |                               | (Query / Mutate)
            v                              v                               |
+-----------+-----------+      +-----------+------------+                  |
|                       |      |                        |                  |
|    Base Blockchain    |<-----|  Rust Watcher Service  |------------------+
|    (Smart Contracts)  |      |  (ethers-rs / WSS)     |
|                       |      |                        |
+-----------+-----------+      +-----------+------------+
            |                              |
            | (Funds Swept)                | (Sweeper triggers BitGo API)
            v                              v
+-----------+-----------+      +-----------+------------+
|                       |      |                        |
|  BitGo MPC Treasury   |<-----|  Rust Sweeper / BitGo  |
|  (Cold Storage)       |      |  Integration Service   |
|                       |      |                        |
+-----------------------+      +------------------------+
```

---

## 4. Revised Payment Flow

The payment lifecycle is designed to break on-chain heuristics while remaining fully automated.

1. **Setup**: The Receiver connects their wallet to the Next.js frontend and inputs their ENS or preferred identity.
2. **Link Generation**: The frontend requests a new Paylink. The Rust API stores the receiver's public key and generates a unique `paylink_id`.
3. **Stealth Derivation**: When a Sender visits the Paylink, the Rust backend generates a unique ephemeral keypair. Using ECDH, it combines the ephemeral private key and the receiver's public key to derive a one-time **Stealth Address**. The ephemeral public key is saved to Convex as an "announcement".
4. **Payment**: The Sender transfers funds (Native ETH or ERC20) directly to the Stealth Address via their Wagmi-connected wallet.
5. **Detection**: The Rust Watcher, subscribed to the Base RPC via WebSockets, detects the transfer to the Stealth Address and writes the `pending` deposit to Convex.
6. **Confirmation**: As blocks finalize, the Watcher updates the confirmation count in Convex. The frontend UI updates in real-time.
7. **Sweeping**: Once finality is reached, the Rust Sweeper service constructs a transaction to move the funds from the Stealth Address to the BitGo Treasury wallet. The ephemeral private key (temporarily held in memory/vault) is used to sign the sweep tx, then immediately **destroyed (zeroized)**.
8. **Receipt**: The backend generates a receipt containing the transaction metadata, encrypts it symmetrically, and stores the cipher text. The frontend can decrypt it using a viewing key derived from the user's wallet signature.

---

## 5. Rust Backend Module Plan

The backend will be organized into strictly separated modules to ensure maintainability.

- **`api/`**: Axum HTTP server routing, request validation, and JSON serialization.
- **`stealth/`**: Cryptographic core. Implements ECDH shared secret generation, public key derivation (Stealth Address), and private key recovery. Uses `k256` and `sha3`.
- **`watcher/`**: Blockchain indexer. Uses `ethers-rs` to subscribe to Base WSS, parse blocks, match `to` addresses against the Convex database, and track block confirmations/reorgs.
- **`sweeper/`**: Consolidation engine. Monitors Convex for `finalized` deposits, recovers the stealth private key, constructs the gas-optimized sweep transaction, broadcasts it, and zeroizes the key.
- **`bitgo/`**: API client for interacting with the BitGo Developer portal (webhook registration, balance querying, MPC signing coordination if applicable).
- **`receipts/`**: Encryption module for symmetrically encrypting payment metadata before offloading to storage.
- **`convex_client/`**: The database abstraction layer. Uses HTTP/REST to query and mutate state in the Convex backend safely.

---

## 6. Watcher Evaluation: RPC Indexer vs. BitGo Webhooks

### Option A: Blockchain Indexer (RPC Subscriptions)
- **Pros**: Instant detection, full control over reorg logic, tracks ephemeral addresses seamlessly without needing to register thousands of temporary addresses with a 3rd party.
- **Cons**: Requires managing WebSocket connections, block tracking, and internal state.

### Option B: BitGo Webhooks
- **Pros**: Offloads the heavy lifting of block indexing and confirmation tracking.
- **Cons**: BitGo is designed for persistent enterprise wallets, not millions of ephemeral stealth addresses. Registering every one-time address as a monitored wallet in BitGo creates severe API bloat and scaling limits.

### **Decision: Option A (RPC Indexer)**
For the MVP, a custom **Rust Blockchain Indexer** via WebSockets is the correct choice. BitGo will be strictly utilized for its primary strength: securing the final consolidated Treasury Wallet. The Rust watcher will track the ephemeral edge addresses.

---

## 7. Updated Database Schema (Convex)

The schema will be deployed on Convex to leverage its real-time reactivity.

1. **`users`**
   - `walletAddress`: string
   - `ensName`: string (optional)
   - `publicKeyHex`: string (Receiver's persistent public key)

2. **`paylinks`**
   - `userId`: Id<"users">
   - `status`: "active" | "disabled"
   - `metadata`: object (e.g., store name, item description)

3. **`ephemeralAddresses`**
   - `paylinkId`: Id<"paylinks">
   - `stealthAddress`: string
   - `ephemeralPubkeyHex`: string (The announcement)
   - `status`: "announced" | "funded" | "swept"

4. **`deposits`**
   - `ephemeralAddressId`: Id<"ephemeralAddresses">
   - `txHash`: string
   - `blockNumber`: number
   - `amount`: string
   - `assetType`: "native" | "erc20"
   - `confirmationStatus`: "pending" | "confirmed" | "finalized" | "reorged"

5. **`receipts`**
   - `depositId`: Id<"deposits">
   - `encryptedPayload`: string (Ciphertext containing tx details)
   - `fileversePointer`: string (Optional CID if pushed to IPFS/Fileverse)

6. **`sweep_jobs`** (Internal Queue)
   - `depositId`: Id<"deposits">
   - `status`: "queued" | "broadcasting" | "completed" | "failed"
   - `sweepTxHash`: string (optional)

---

## 8. Security Requirements & Threat Model

### Private Key Storage
- **Risk**: Ephemeral keys are stolen, allowing interception of funds before they are swept to BitGo.
- **Mitigation**: Ephemeral private keys are generated in-memory by the Rust backend during the sweeping phase (recovered using the backend's master viewing key + the public ephemeral key). Once the sweep transaction is signed and broadcast, the key is securely wiped from memory using the Rust `zeroize` crate. It is never written to disk.

### Sweep Transaction Safety
- **Risk**: Sweeper fails mid-transaction or double-spends.
- **Mitigation**: The `sweep_jobs` state machine utilizes strict locking. A job must be marked `broadcasting` before the tx is submitted. Gas is calculated dynamically to ensure the stealth wallet is drained to exactly zero.

### Database Consistency
- **Risk**: The watcher crashes and misses a block, or processes the same block twice.
- **Mitigation**: The watcher records the `latest_processed_block` in Convex. On startup, it queries this checkpoint and synchronously scans all historical blocks between the checkpoint and the current chain head before opening the live WebSocket subscription. `txHash` indexing ensures idempotency.

---

## 9. Finalized MVP Implementation Plan (Phase 3 Prep)

With the architecture clearly defined, Phase 3 implementation will proceed in the following structured steps:

1. **Scaffold the Monorepo**: Establish the Next.js frontend, Rust backend, and Convex schema in their final directory structures.
2. **Implement Convex Models**: Deploy the revised schema above to a Convex dev environment and generate the TypeScript/Rust bindings.
3. **Build the Rust Core**: Implement the `stealth` module (ECDH logic), the `convex_client` bridge, and the `api` server.
4. **Build the Watcher**: Implement the WebSocket indexer that scans for transfers to generated `ephemeralAddresses` and writes to `deposits`.
5. **Implement the Sweeper**: Build the engine that detects `finalized` deposits, recovers the key, and fires the sweep transaction to the BitGo treasury address.
6. **Frontend Integration**: Connect the Next.js app to the Rust API (for link generation) and Convex (for real-time UI updates).
7. **End-to-End Testing**: Run the full flow on Base Sepolia from link creation -> payment -> detection -> sweeping -> receipt generation.