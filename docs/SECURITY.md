# 🛡️ Security

> CloakFund prioritizes **privacy and financial safety** at every layer of the architecture.

---

## Security Model Overview

```mermaid
flowchart TD
    subgraph "Trust Boundary: Client"
        W["User Wallet\n(private keys)"]
        FE["Frontend\n(receipt decryption)"]
    end

    subgraph "Trust Boundary: Server"
        API["Rust API\n(no keys, no plaintext)"]
        SG["Stealth Generator\n(ephemeral keys only)"]
    end

    subgraph "Trust Boundary: External"
        BC["Base Blockchain\n(public ledger)"]
        BG["BitGo MPC\n(multi-sig custody)"]
        FV["Fileverse\n(encrypted storage)"]
    end

    W -->|"Signs transactions"| BC
    FE -->|"Decrypts locally"| FV
    API -->|"Stateless operations"| SG
    SG -->|"Publishes address"| BC
    API -->|"Encrypted data only"| FV
    API -->|"Consolidation request"| BG

    style W fill:#6C63FF,stroke:#4A44CC,color:#fff
    style FE fill:#1E1E2E,stroke:#6C63FF,color:#CDD6F4
    style API fill:#F38BA8,stroke:#D6336C,color:#1E1E2E
    style SG fill:#FAB387,stroke:#E87D3E,color:#1E1E2E
    style BC fill:#A6E3A1,stroke:#4CAF50,color:#1E1E2E
    style BG fill:#F9E2AF,stroke:#D4A017,color:#1E1E2E
    style FV fill:#CBA6F7,stroke:#9B59B6,color:#1E1E2E
```

---

## Security Invariants

| # | Invariant | Enforced By | Verification |
| - | --------- | ----------- | ------------ |
| 1 | **Private keys never leave user's wallet** | Frontend design — all signing via wallet provider | Code review, no key input fields |
| 2 | **Backend never stores private keys** | Rust architecture — stateless key handling | No key storage in DB schema |
| 3 | **Ephemeral keys destroyed after use** | `zeroize` crate — secure memory clearing | Unit tests verify zeroization |
| 4 | **Receipts encrypted before server storage** | Encryption Service — ChaCha20/AES-GCM | Ciphertext-only in Fileverse |
| 5 | **Decryption happens client-side only** | Frontend-only decrypt — server never sees plaintext | No decrypt endpoints in API |
| 6 | **Stealth addresses prevent clustering** | ECDH one-time addresses — unique per payment | Cryptographic guarantee |
| 7 | **Treasury requires multi-sig approval** | BitGo MPC — threshold signatures | BitGo audit trail |

---

## Key Protection

| Key Type | Location | Protection |
| -------- | -------- | ---------- |
| User private key | User's wallet (MetaMask, etc.) | Never transmitted to CloakFund |
| Ephemeral key (`r`) | Server memory (transient) | Zeroized immediately after stealth address derivation |
| Receipt encryption key | Derived client-side | Never sent to server |
| BitGo MPC keys | BitGo infrastructure | Multi-party computation — no single party has full key |

---

## Treasury Security

- BitGo MPC wallets require **multiple approvals** before any fund movement
- Consolidation transactions go through a **state machine** (`pending → signed → broadcasted → confirmed`)
- All consolidation jobs are logged with **audit trail** (job ID, timestamps, tx hashes)

---

## Encryption at Rest

- All financial documents and receipts are encrypted **before** upload to Fileverse
- Encryption uses authenticated algorithms (ChaCha20-Poly1305 / AES-GCM) — integrity + confidentiality
- **No plaintext** ever touches Fileverse or the backend database

---

## Privacy Guarantees

| Property | Mechanism |
| -------- | --------- |
| **Unlinkable payments** | Each payment address is derived from a unique ephemeral key |
| **Identity separation** | ENS name never directly associated with stealth addresses on-chain |
| **Observer resistance** | Without recipient's private key, addresses appear unrelated |
| **Metadata privacy** | Receipts encrypted — even storage provider cannot read them |

---

→ See [THREAT_MODEL.md](./THREAT_MODEL.md) for attack vectors and mitigations.
→ See [CRYPTOGRAPHY.md](./CRYPTOGRAPHY.md) for the underlying math.
