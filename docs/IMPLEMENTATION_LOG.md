# CloakFund Implementation Log & Guide

This document serves as a detailed ledger of the development process for CloakFund. It explains **what** was built in each phase, **how** it was implemented technically, and **why** specific design choices were made.

---

## Phase 0: Preparation & Access

### What was done
- Created a dedicated development branch (`feature/phases-implementation`).
- Generated `.env.example` to track required environment variables.
- Created `docs/SECRETS_SETUP.md` to establish guidelines for managing API keys and handling API rate limits/unavailability using mock endpoints.
- Created `docs/test_wallets.md` containing mock Base Sepolia testnet addresses for the Sender, Receiver (ENS identity), and Treasury Vault.

### How it was done
- Scaffolded standard markdown and environment template files in the root and `docs/` directories.
- Defined explicit fallback flags in `.env.example` (e.g., `USE_MOCK_BITGO=true`) to toggle between real services and mock data.

### Why it was done
- **Security:** Ensures developers don't accidentally commit real API keys or private keys to version control.
- **Reliability:** By defining a mocking strategy upfront, the team can continue developing or demoing the app even if a sponsor's testnet API goes down during the hackathon.
- **Testing:** Pre-defining test wallets creates a standardized environment for testing the entire flow without draining real funds.

---

## Phase 1: Core Rust Backend (Stealth Generator Module)

### What was done
- Initialized the Rust backend workspace (`cargo new rust-backend`).
- Finalized a minimal cryptographic dependency set centered on `k256`, `sha3`, and `hex`.
- Implemented stealth address generation in `rust-backend/src/stealth.rs`.
- Implemented recipient-side stealth private key recovery so the protocol works end-to-end.
- Added EIP-55 checksum formatting for returned Ethereum addresses.
- Added `view_tag` support to the stealth generation output for faster recipient-side scanning.
- Added a CLI in `rust-backend/src/main.rs` for both sender-side generation and recipient-side recovery.
- Added deterministic cryptographic test vectors in `docs/CRYPTO_TEST_VECTORS.md`.
- Added both positive and negative unit tests covering roundtrip correctness and malformed key inputs.

### How it was done
The finalized stealth flow uses Elliptic Curve Diffie-Hellman (ECDH) on the `secp256k1` curve:
1. **Input:** The sender provides the recipient public key.
2. **Ephemeral keypair:** A fresh ephemeral secret/public keypair is generated for the payment.
3. **Shared secret:** The sender computes an ECDH shared secret using the ephemeral secret key and recipient public key.
4. **Scalar derivation:** The shared secret is hashed with `Keccak256` and safely converted into a curve scalar.
5. **Stealth public key:** The derived scalar is applied to the generator point and combined with the recipient public key to derive a one-time stealth public key.
6. **Stealth address:** The uncompressed stealth public key is hashed and converted into an EVM address, then formatted using EIP-55 checksum rules.
7. **Recipient recovery:** The recipient uses their private key and the sender-published ephemeral public key to derive the same shared secret and reconstruct the matching stealth private key.
8. **Validation:** Invalid keys and malformed inputs return clean errors instead of crashing.

### Why it was done
- **Privacy (Unlinkability):** Each payment produces a fresh one-time address that cannot be trivially clustered on-chain.
- **Spendability:** Recipient-side recovery ensures generated stealth addresses are not just valid-looking addresses, but actually spendable by the intended recipient.
- **Operational safety:** Error handling replaced panic-prone behavior in cryptographic parsing paths.
- **Developer usability:** The CLI and test vectors make the module easy to verify manually during reviews and demos.
- **Rust for Crypto:** Rust provides strong memory-safety guarantees and audited crypto ecosystem support for implementing sensitive payment logic.

### Verification and hardening
- Added a roundtrip test to verify: generate stealth address → recover stealth private key → derive the same address.
- Added negative tests for:
  - invalid recipient public key
  - invalid recipient private key
  - invalid ephemeral public key
- Added explicit private key length validation before parsing to avoid lower-level panics.
- Re-ran the Rust test suite successfully after hardening changes.

---

## Phase 2: Deposit Watcher / Indexer

### What was done (Convex Pivot)
- Pivoted the persistence layer from Postgres/SQLx to Convex for faster hackathon iteration.
- Scaffolded the Convex backend project in the `convex/` directory.
- Defined the Convex schema with tables for `paylinks`, `ephemeralAddresses`, and `deposits`.
- Implemented Convex server functions for paylink creation, stealth address persistence, and deposit status updates (`paylinks.ts`, `deposits.ts`).
- Exposed a minimal Convex HTTP API (`http.ts`) for health and deposit-status checks.
- Updated the Rust backend `config.rs` to support Convex deployment variables.
- Created comprehensive Rust data structures in `models.rs` corresponding to the Convex backend types.

### How it was done
- **Convex Schema:** Designed indexes around `chainId`, `status`, and `txHash` to efficiently query pending/confirmed deposits.
- **Rust Types:** Built `DepositRecord`, `PaylinkRecord`, and status enums (`ConfirmationStatus`, `EphemeralAddressStatus`) with strict `serde` serialization to interface seamlessly with Convex HTTP actions.

### Why it was done
- **Speed & Simplicity:** Convex provides a managed real-time database with built-in server functions, significantly reducing boilerplate compared to setting up Postgres migrations and local DB containers.
- **Separation of Concerns:** Rust handles the heavy lifting of blockchain watching and cryptography, while Convex serves as the high-availability data and API layer for the frontend.

### What was recently completed
- Implemented the `convex_client.rs` bridge to interact with Convex backend functions securely from Rust.
- Implemented `watcher.rs` using `ethers-rs` WebSocket subscriptions to listen to the Base network for native and ERC20 token transfers.
- Connected the pipeline: Base new blocks -> Scan transactions and logs -> Check Convex for matching ephemeral addresses -> Submit matching deposits via `upsertDeposit`.
- Implemented block confirmation tracking and reorg handling in the watcher.
- Integrated the watcher and a minimal Axum API into `main.rs` via a new `serve` command.
- Provided a demonstration script (`scripts/watcher_test.sh`) that simulates a deposit and verifies Convex state updates, meeting the phase's final deliverable requirements.

### Hardening and Bug Fixes
- **Historical Block Catch-up:** Added logic to `watcher.rs` to fetch the `latest_processed_block` checkpoint from Convex on startup, automatically syncing missed blocks during downtime before opening the real-time WebSocket.
- **Serialization Alignment:** Fixed Rust-to-Convex mapping by enforcing `camelCase` `serde` serialization for all cross-boundary models and explicitly renaming `_id` and `_creationTime`.
- **Chain Reorg Robustness:** Corrected the reorg flow to affirmatively call the `mark_deposit_reorged` Convex mutation when an indexed transaction shifts blocks, preventing double-counting.
- **Safety Checks:** Added data length checks for ERC20 parsing and fallback handling for missing block hashes to prevent crashes and silent skipping.
- **Convex Admin Authentication:** Integrated `CONVEX_ADMIN_KEY` handling in the Rust client to authenticate backend mutations safely.

---

*(This log will be continuously updated as subsequent phases are completed.)*