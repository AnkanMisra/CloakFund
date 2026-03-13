# Convex Setup for CloakFund

This directory contains the Convex backend data model and server functions used by CloakFund for Phase 2 and beyond.

Convex is used here as the database and backend function layer for:

- paylink persistence
- stealth address announcement persistence
- deposit indexing state
- deposit status queries
- lightweight HTTP actions

---

## Why Convex

CloakFund needs a backend data layer that is easy to iterate on during the hackathon while still supporting structured backend logic.

Convex gives us:

- a managed backend data store
- server-side query and mutation functions
- HTTP actions
- generated types for the Convex API
- easy local and cloud-backed developer workflow

In this project, the Rust backend remains responsible for blockchain watching and cryptographic logic, while Convex stores and serves application data.

---

## Directory Overview

Typical files in this folder:

- `schema.ts`  
  Defines the Convex tables and indexes.

- `paylinks.ts`  
  Stores and queries paylinks and ephemeral stealth addresses.

- `deposits.ts`  
  Stores and queries watcher-detected deposits and confirmation states.

- `http.ts`  
  Exposes small HTTP endpoints directly from Convex where useful.

- `_generated/`  
  Auto-generated Convex files created by the Convex dev workflow.

---

## Current Data Model

Phase 2 currently uses three primary tables:

### `paylinks`
Represents a payment intent / paylink tied to an ENS identity or wallet metadata.

Fields include:

- `ensName`
- `recipientPublicKeyHex`
- `status`
- `metadata`
- `chainId`
- `network`

### `ephemeralAddresses`
Stores generated stealth payment destinations for a paylink.

Fields include:

- `paylinkId`
- `stealthAddress`
- `ephemeralPubkeyHex`
- `viewTag`
- `chainId`
- `network`
- `status`

### `deposits`
Stores detected on-chain deposits and confirmation progress.

Fields include:

- `paylinkId`
- `ephemeralAddressId`
- `txHash`
- `logIndex`
- `blockNumber`
- `blockHash`
- `fromAddress`
- `toAddress`
- `assetType`
- `tokenAddress`
- `amount`
- `decimals`
- `symbol`
- `confirmations`
- `confirmationStatus`
- `detectedAt`
- `confirmedAt`

---

## Local Development Workflow

### 1. Install dependencies at repo root

Run this from the project root:

```/dev/null/package.json#L1-6
npm install
```

This installs the `convex` package declared in the root `package.json`.

---

### 2. Start Convex development

Run:

```/dev/null/convex-dev.sh#L1-2
npx convex dev
```

On first run, Convex will:

- authenticate you
- create or connect to a project
- generate `convex/_generated`
- write local deployment information into your environment file

Keep this process running while developing Convex functions.

If you only want one codegen/deploy pass instead of a live dev loop:

```/dev/null/convex-dev-once.sh#L1-2
npx convex dev --once
```

---

### 3. Generated code

After Convex initializes successfully, you should see:

- `convex/_generated/server`
- `convex/_generated/api`
- related generated type definitions

These generated files are required by imports like:

```/dev/null/example.ts#L1-3
import { mutation, query } from "./_generated/server";
import { api } from "./_generated/api";
```

If `_generated` is missing, run the Convex dev command again from the project root.

---

### 4. Run Convex functions manually

You can invoke functions from the command line:

```/dev/null/run-query.sh#L1-2
npx convex run paylinks:getById '{"paylinkId":"<id>"}'
```

Example mutation call:

```/dev/null/run-mutation.sh#L1-2
npx convex run paylinks:create '{"ensName":"alice.eth","recipientPublicKeyHex":"0x..."}'
```

Replace arguments with real values from your dev environment.

---

## Relationship with Rust Backend

CloakFund uses a split backend model:

### Rust backend
Responsible for:

- stealth cryptography
- blockchain watcher logic
- deposit detection
- confirmation updates
- future treasury/consolidation logic

### Convex backend
Responsible for:

- storing paylinks
- storing ephemeral address metadata
- storing deposits
- serving deposit status queries
- exposing lightweight backend data APIs

The expected Phase 2 pattern is:

1. Rust watcher detects an on-chain payment
2. Rust backend resolves the matching stealth address
3. Rust backend writes deposit state through Convex
4. Frontend or backend reads deposit status from Convex

---

## Suggested Environment Variables

Depending on your local setup, you may end up with values like:

- `CONVEX_DEPLOYMENT`
- `CONVEX_URL`

Convex usually writes these during setup.

For the Rust backend, you may eventually also want a dedicated env variable such as:

- `CONVEX_URL`
- `CONVEX_SITE_URL`

Use whichever deployment URL is needed for:
- Rust Convex client access
- HTTP action access

---

## Common Commands

From repo root:

```/dev/null/commands.txt#L1-5
npm install
npx convex dev
npx convex dev --once
npx convex dashboard
npx convex run deposits:getPendingConfirmationUpdates '{}'
```

---

## Notes for This Project

- Address fields should be normalized before persistence.
- Deposit writes must be idempotent.
- Confirmation state should be updated incrementally.
- Rust remains the blockchain-facing service.
- Convex is the application data layer, not the blockchain indexer itself.

---

## Troubleshooting

### Error: missing `package.json`
Make sure you run Convex commands from the repository root, not from inside `convex/` or `rust-backend/`.

### Error: `_generated` imports fail
Run:

```/dev/null/fix-generated.sh#L1-2
npx convex dev --once
```

### Error: not logged in
Run:

```/dev/null/login.sh#L1-2
npx convex dev
```

and complete the authentication flow.

### Error: deployment exists but codegen not present
Convex project creation and local code generation are separate steps in practice. Running the dev command again from the correct directory usually fixes this.

---

## Phase 2 Goal

For Phase 2, this Convex layer should support:

- persisted paylinks
- persisted stealth address announcements
- persisted deposit records
- confirmation-aware deposit status queries
- clean integration with the Rust blockchain watcher

This keeps the system modular and easy to demo during the hackathon.
