# 🕵️ CloakFund

> **Privacy-first payment & treasury infrastructure for Web3** — built at ETHMumbai 2026

[![Built on Base](https://img.shields.io/badge/Built%20on-Base%20L2-0052FF?style=for-the-badge&logo=ethereum)](https://base.org)
[![Rust Backend](https://img.shields.io/badge/Backend-Rust-E43717?style=for-the-badge&logo=rust)](https://rust-lang.org)
[![Next.js Frontend](https://img.shields.io/badge/Frontend-Next.js-000000?style=for-the-badge&logo=nextdotjs)](https://nextjs.org)

---

## The Problem

Public blockchains expose **every transaction** — sender, receiver, amount, and full wallet history. Once a wallet is linked to a real identity (e.g. `alice.eth → 0xABC`), all financial activity becomes permanently visible to anyone.

## The Solution

CloakFund separates **identity** from **payment addresses**. Users keep a single public ENS identity, but every incoming payment is routed to a **unique stealth address** — cryptographically unlinkable on-chain, yet aggregated in the CloakFund dashboard.

```
alice.eth
  ├── Payment 1 → 0x91AF   ← stealth address (unlinkable)
  ├── Payment 2 → 0x4B3C   ← stealth address (unlinkable)
  └── Payment 3 → 0xAA1D   ← stealth address (unlinkable)
```

> No observer can link these addresses together or back to Alice.

---

## ✨ Key Features

| Feature                      | Description                                                           |
| ---------------------------- | --------------------------------------------------------------------- |
| 🔐 **Stealth Payments**      | Every payment uses a one-time ECDH-derived address                    |
| 🏷️ **ENS Identity Layer**    | Human-readable names without exposing wallet history                  |
| 🏦 **Secure Treasury Vault** | Funds consolidated into BitGo MPC multi-sig custody                   |
| 📄 **Encrypted Receipts**    | Payment records encrypted (ChaCha20/AES-GCM) and stored via Fileverse |
| 🤖 **AI Monitoring**         | Optional HeyElsa agent for large-payment alerts and summaries         |

---

## 🏗️ Tech Stack

| Layer               | Technology           | Purpose                                                          |
| ------------------- | -------------------- | ---------------------------------------------------------------- |
| **Frontend**        | Next.js (TypeScript) | Wallet connection, ENS input, dashboard, receipt decryption      |
| **Backend**         | Rust (Axum + Tokio)  | Stealth address generation, watcher, treasury engine, encryption |
| **Blockchain**      | Base (L2)            | Low-cost, fast on-chain settlement                               |
| **Smart Contracts** | Solidity             | `PaymentResolver`, `TreasuryForwarder`                           |
| **Treasury**        | BitGo MPC            | Multi-signature custody & consolidation                          |
| **Storage**         | Fileverse            | Encrypted receipt & financial record persistence                 |
| **Identity**        | ENS                  | Human-readable payment identities                                |
| **AI Agent**        | HeyElsa              | Automated monitoring & alerts                                    |

---

## 📂 Documentation

| Document                                             | Description                                |
| ---------------------------------------------------- | ------------------------------------------ |
| [PROBLEM.md](./PROBLEM.md)                           | The privacy crisis on public blockchains   |
| [SOLUTION.md](./SOLUTION.md)                         | How CloakFund solves it                    |
| [VISION.md](./VISION.md)                             | Long-term mission and roadmap              |
| [ARCHITECTURE.md](./ARCHITECTURE.md)                 | System layers and component overview       |
| [SYSTEM-DESIGN.md](./SYSTEM-DESIGN.md)               | Event-driven design and service breakdown  |
| [DATA_FLOW.md](./DATA_FLOW.md)                       | End-to-end data flow with Mermaid diagrams |
| [RUST_BACKEND_DESIGN.md](./RUST_BACKEND_DESIGN.md)   | Backend module design and async runtime    |
| [API.md](./API.md)                                   | REST API endpoint reference                |
| [CONTRACTS.md](./CONTRACTS.md)                       | Smart contract specifications              |
| [CRYPTOGRAPHY.md](./CRYPTOGRAPHY.md)                 | Stealth address math and encryption        |
| [SECURITY.md](./SECURITY.md)                         | Security model and invariants              |
| [THREAT_MODEL.md](./THREAT_MODEL.md)                 | Attack vectors and mitigations             |
| [SPONSOR_INTEGRATIONS.md](./SPONSOR_INTEGRATIONS.md) | ETHMumbai sponsor technology usage         |
| [PHASES.md](./PHASES.md)                             | Implementation phases and milestones       |
| [FUTURE_WORK.md](./FUTURE_WORK.md)                   | Post-hackathon roadmap                     |
| [DEMO.md](./DEMO.md)                                 | Live demo script and instructions          |

---

## 🚀 Quick Start

```bash
# Clone the repository
git clone https://github.com/your-org/CloakFund.git
cd CloakFund

# Backend (Rust)
cd rust-backend
cp .env.example .env        # Fill in API keys
cargo build --release
cargo run

# Frontend (Next.js)
cd frontend
npm install
npm run dev
```

---

## 🏆 ETHMumbai 2026 — Sponsor Tracks

| Sponsor       | Integration                                           | Prize Track       |
| ------------- | ----------------------------------------------------- | ----------------- |
| **Base**      | L2 blockchain infrastructure — low gas, fast finality | Build on Base     |
| **ENS**       | Identity layer for privacy-preserving payments        | ENS Integration   |
| **BitGo**     | MPC treasury vault with multi-sig custody             | BitGo Custody     |
| **Fileverse** | Encrypted on-chain document storage for receipts      | Fileverse Storage |
| **HeyElsa**   | AI monitoring agent for payment anomaly detection     | AI/Automation     |

---

## 👥 Team

Built with 🖤 at ETHMumbai 2026.

---

<p align="center">
  <strong>CloakFund</strong> — Financial privacy should be the default, not the exception.
</p>
