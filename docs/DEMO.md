# 🎬 Demo Script — ETHMumbai 2026

> **3-minute live demo** showcasing CloakFund's end-to-end privacy payment flow.

---

## Pre-Demo Checklist

- [ ] Backend running (`cargo run` — Rust API on `localhost:3001`)
- [ ] Frontend running (`npm run dev` — Next.js on `localhost:3000`)
- [ ] Test wallet funded on Base testnet
- [ ] MetaMask configured with Base testnet
- [ ] ENS test name registered (or mock resolver active)
- [ ] Fallback video ready (`docs/demo_fallbacks/demo.mp4`)

---

## Demo Flow (3 Minutes)

### ⏱️ 0:00–0:30 — The Problem (30 seconds)

> **"Every Ethereum transaction is public. Once your ENS name is linked to your wallet, your entire financial life is exposed."**

- Show a block explorer with a real wallet — highlight visible balances, transactions, DeFi activity
- Emphasize: freelancers, DAOs, companies all face this problem

---

### ⏱️ 0:30–1:00 — The Solution (30 seconds)

> **"CloakFund separates your identity from your payment addresses."**

- Show the CloakFund dashboard
- Explain: "alice.eth receives payments, but each payment goes to a unique stealth address"
- Show the diagram: ENS name → multiple unlinkable stealth addresses

---

### ⏱️ 1:00–2:00 — Live Payment (60 seconds)

| Step | Action | What to Show |
| ---- | ------ | ------------ |
| 1 | Connect wallet | MetaMask popup → wallet connected indicator |
| 2 | Enter ENS name | Type `alice.eth` → system resolves identity |
| 3 | Generate payment link | Click "Create Paylink" → stealth address + QR code displayed |
| 4 | Send payment | Switch to sender wallet → transfer 0.01 ETH to stealth address |
| 5 | Watch detection | Dashboard updates in real-time → deposit confirmed ✅ |
| 6 | View receipt | Click receipt → encrypted data fetched from Fileverse → decrypted in browser |

---

### ⏱️ 2:00–2:30 — Under the Hood (30 seconds)

> **"Here's what makes this work."**

- Show the architecture diagram (from `DATA_FLOW.md`)
- Highlight: Rust backend, ECDH stealth addresses, BitGo MPC, Fileverse encryption
- Mention: "Server never sees private keys or plaintext receipts"

---

### ⏱️ 2:30–3:00 — Sponsor Integration & Impact (30 seconds)

> **"We deeply integrate five ETHMumbai sponsors."**

| Sponsor | Quick Mention |
| ------- | ------------- |
| **Base** | "All settlement on Base L2 — low gas makes per-payment addresses viable" |
| **ENS** | "Human-readable identity layer — no raw addresses shared" |
| **BitGo** | "MPC treasury vault — institutional-grade fund security" |
| **Fileverse** | "Encrypted receipt storage — decrypted only client-side" |
| **HeyElsa** | "AI monitoring for large payment alerts" |

> **"Financial privacy should be the default, not the exception. That's CloakFund."**

---

## Fallback Plan

If any component fails during the live demo:

| Issue | Fallback |
| ----- | -------- |
| Backend not responding | Switch to pre-recorded video |
| Blockchain transaction slow | Show pre-confirmed transaction from earlier test |
| MetaMask issues | Use pre-connected browser profile |
| Fileverse timeout | Show pre-downloaded decrypted receipt |

---

## Key Talking Points for Judges

1. **Technical depth**: Rust backend with ECDH cryptography, not just a frontend wrapper
2. **Privacy guarantees**: Mathematically unlinkable addresses, not just obscurity
3. **Sponsor depth**: Each sponsor technology is structurally integrated, not superficial
4. **Real-world impact**: Freelancers, DAOs, and companies need this today
5. **Extensibility**: Designed as infrastructure — not a one-off app

---

→ See [SPONSOR_INTEGRATIONS.md](./SPONSOR_INTEGRATIONS.md) for detailed sponsor usage.
→ See [DATA_FLOW.md](./DATA_FLOW.md) for the full architecture diagrams.
