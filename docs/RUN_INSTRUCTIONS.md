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

Open a **new terminal tab** and run:

```bash
cd rust-backend

# Run the API server and blockchain watcher
cargo run -- serve
```

Ensure your `.env` variables (like `CONVEX_URL` and `RPC_URL`) are properly set up as per `docs/SECRETS_SETUP.md` before starting the Rust server.

---

## 3. Start the Next.js Frontend (Phase 4)

*Note: The frontend implementation is scheduled for **Phase 4**.* 

Once Phase 4 is complete, you will be able to start the Next.js frontend using Bun:

```bash
# Future command (Phase 4)
cd frontend
bun install
bun run dev
```

Until then, you can interact with the Rust API directly via HTTP (`http://localhost:3000`) or use the provided CLI commands in the Rust backend for testing stealth generation.
