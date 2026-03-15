# Running CloakFund Locally

This guide explains how to spin up the CloakFund environment for local development. We use **Bun** as our primary JavaScript/TypeScript package manager and runtime.

## Prerequisites

Before you begin, ensure you have the following installed:
- [Bun](https://bun.sh/) (`curl -fsSL https://bun.sh/install | bash`)
- [Rust & Cargo](https://rustup.rs/)
- A [Convex](https://convex.dev/) account for the real-time database

---

## 1. Start the Convex Data Layer

Convex serves as our real-time database and serverless backend for the frontend.

From the root of the project, install dependencies and start the Convex development server:

```bash
# Install dependencies using Bun
bun install

# Start the Convex development server
# You can use bunx (or npx) to run the convex CLI
bunx convex dev
```

*Note: The first time you run this, it will prompt you to log into Convex and link the project.*

---

## 2. Start the Rust Backend

The Rust backend handles the core cryptography, stealth address generation, and the blockchain deposit watcher.

Open a **new terminal tab (Terminal 2)** and run:

```bash
cd rust-backend

# Run the API server and blockchain watcher
# Use WATCHER_START_BLOCK to skip old blocks for faster testing
WATCHER_START_BLOCK=38836220 RUST_LOG=debug cargo run -- serve
```

Ensure your `.env` variables (like `CONVEX_URL` and `RPC_URL`) are properly set up as per `docs/SECRETS_SETUP.md` before starting the Rust server.

---

## 3. Run the End-to-End Test (Phase 4)

Now that Phase 4 (Sweeper) is complete, you can test the entire pipeline from paylink creation to treasury sweep.

### Option A: Fully Automated (Recommended)

Open a **third terminal tab (Terminal 3)** at the project root and run the test script:

```bash
./scripts/sweeper_test.sh
```

This script will:
1. Create a new Paylink.
2. Automatically send real testnet ETH to the generated stealth address using `send_eth.mjs`.
3. Wait for the Rust Watcher (Terminal 2) to detect the deposit on-chain.
4. Automatically trigger the Sweeper logic to consolidate the funds to the BitGo treasury.

### Option B: Manual Testing (For Debugging)

You'd do it in two steps:

**Step 1 — Create a paylink (this gives you the stealth address):**

```bash
curl -s -X POST http://localhost:8080/api/v1/paylink \
  -H "Content-Type: application/json" \
  -d '{"recipientPublicKeyHex": "0x034f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa", "ensName": "test.eth", "chainId": 84532, "network": "base-sepolia"}'
```

This returns something like:

```json
{"paylinkId":"jd7...","stealthAddress":"0x18c3425596D2Ad83FE1eC6392acf2ADFE93e7FcF","ephemeralPubkeyHex":"0x03..."}
```

**Step 2 — Copy the stealthAddress from that output and send to it:**

```bash
node scripts/send_eth.mjs 0x18c3425596D2Ad83FE1eC6392acf2ADFE93e7FcF 0.00001
```

But honestly, `./scripts/sweeper_test.sh` does both of these automatically — that's the whole point of it. The manual route is just for debugging specific scenarios.

---

## 4. Start the Next.js Frontend (Phase 5)

*Note: The frontend implementation is scheduled for **Phase 5**.* 

Once Phase 5 is complete, you will be able to start the Next.js frontend using Bun:

```bash
# Future command (Phase 5)
cd frontend
bun install
bun run dev
```

Until then, you can rely on the `sweeper_test.sh` script to verify full backend functionality.
