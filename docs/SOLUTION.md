# ✅ Solution — CloakFund's Approach

> **Separate identity from payment addresses — keep privacy without losing usability.**

---

## Core Concept

CloakFund introduces a simple but powerful principle:

> **One public identity. Infinite private payment addresses.**

Users maintain a human-readable ENS identity (e.g., `alice.eth`) for receiving payments, but each incoming transaction is routed to a **unique, one-time stealth address** that cannot be linked to the user's identity or to any other payment they have received.

---

## How It Works

```
alice.eth     ← Public identity (never receives funds directly)
   │
   ├── Payment 1 → 0x91AF   ← Stealth address (unique, unlinkable)
   ├── Payment 2 → 0x4B3C   ← Stealth address (unique, unlinkable)
   └── Payment 3 → 0xAA1D   ← Stealth address (unique, unlinkable)

On-chain observer sees: 3 unrelated addresses receiving funds.
Alice's dashboard sees: 3 payments totaling her balance.
```

> **No observer can link these addresses together, or trace them back to Alice.**

---

## Architectural Layers

CloakFund is composed of five distinct layers, each solving a specific part of the privacy puzzle:

| Layer | Component | Purpose |
| ----- | --------- | ------- |
| 🏷️ **Identity** | ENS | Human-readable name for payment requests — never touches funds |
| 💳 **Payment** | Stealth Addresses (ECDH) | Each payment uses a cryptographically derived one-time address |
| 🏦 **Treasury** | BitGo MPC Vault | Funds consolidated securely with multi-signature approval |
| 📄 **Data** | Fileverse | Encrypted receipts stored off-chain, decrypted client-side only |
| 🤖 **Automation** | HeyElsa AI | Optional monitoring for large payments and anomaly detection |

---

## Key Design Decisions

| Decision | Rationale |
| -------- | --------- |
| **Stealth addresses, not mixers** | Regulatory-safe, no pooling of funds |
| **Application-layer privacy** | Works on any EVM chain — no protocol changes |
| **Base L2 deployment** | Low gas costs make per-payment address generation viable |
| **Rust backend** | Memory safety and cryptographic reliability for key operations |
| **Client-side decryption** | Server never sees plaintext receipts — zero-knowledge architecture |
| **ENS identity layer** | Users don't need to share raw addresses — better UX |

---

## What Makes CloakFund Different

| Approach | Privacy | Usability | Compliance | Composability |
| -------- | ------- | --------- | ---------- | ------------- |
| **Mixers** (Tornado, etc.) | ✅ High | ❌ Complex | ❌ Risky | ❌ Isolated |
| **Privacy L1s** (Zcash, etc.) | ✅ High | ⚠️ Medium | ⚠️ Uncertain | ❌ Isolated |
| **ZK Rollups** | ✅ High | ❌ Complex | ⚠️ New | ⚠️ Limited |
| **CloakFund** | ✅ High | ✅ Simple | ✅ Safe | ✅ Any EVM L2 |

---

→ See [ARCHITECTURE.md](./ARCHITECTURE.md) for the full system design.
→ See [CRYPTOGRAPHY.md](./CRYPTOGRAPHY.md) for the stealth address math.
