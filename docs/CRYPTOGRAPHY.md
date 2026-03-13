# 🔐 Cryptography

> CloakFund uses **elliptic curve cryptography** to generate stealth addresses, ensuring each payment uses a unique, unlinkable address.

---

## Stealth Address Generation

The core privacy mechanism uses **ECDH (Elliptic Curve Diffie-Hellman)** to derive one-time payment addresses without requiring the sender and recipient to communicate directly.

### Step-by-Step Process

```mermaid
flowchart TD
    A["1️⃣ Sender generates\nephemeral key pair\n(r, R = r·G)"] --> B["2️⃣ Sender fetches\nrecipient's stealth\nmeta-address (K)"]
    B --> C["3️⃣ Compute shared secret\nS = ECDH(r, K) = r·K"]
    C --> D["4️⃣ Derive stealth address\naddr = pubToAddr(hash(S)·G + K)"]
    D --> E["5️⃣ Sender transfers funds\nto stealth address"]
    E --> F["6️⃣ Publish ephemeral\npublic key R on-chain"]
    F --> G["7️⃣ Ephemeral private\nkey r destroyed"]

    style A fill:#FAB387,stroke:#E87D3E,color:#1E1E2E
    style B fill:#89B4FA,stroke:#3B82F6,color:#1E1E2E
    style C fill:#F38BA8,stroke:#D6336C,color:#1E1E2E
    style D fill:#F38BA8,stroke:#D6336C,color:#1E1E2E
    style E fill:#A6E3A1,stroke:#4CAF50,color:#1E1E2E
    style F fill:#94E2D5,stroke:#2D9E8F,color:#1E1E2E
    style G fill:#F9E2AF,stroke:#D4A017,color:#1E1E2E
```

| Step | Operation | Input | Output |
| ---- | --------- | ----- | ------ |
| 1 | Generate ephemeral key pair | Random seed | `(r, R)` where `R = r·G` |
| 2 | Fetch recipient meta-address | ENS name | Stealth public key `K` |
| 3 | Compute shared secret | `r`, `K` | `S = r·K` (ECDH) |
| 4 | Derive stealth address | `S`, `K` | `addr = pubToAddr(hash(S)·G + K)` |
| 5 | Transfer funds | ETH amount | Transaction to stealth address |
| 6 | Publish ephemeral key | `R` | Stored on-chain or via API |
| 7 | Destroy ephemeral key | `r` | Zeroized from memory |

### Recipient Recovery

The recipient can recover funds by:

1. Scanning published ephemeral keys `R`
2. Computing `S = k·R` (where `k` is recipient's private key)
3. Deriving the stealth private key: `stealthKey = hash(S) + k`
4. Checking if the derived address matches any on-chain deposits

> **This ensures only the intended recipient can discover and access their payments.**

---

## Receipt Encryption

Payment receipts are encrypted using **authenticated symmetric encryption** before storage on Fileverse.

| Algorithm | Type | Use Case |
| --------- | ---- | -------- |
| **ChaCha20-Poly1305** | AEAD (primary) | High-performance encryption with authentication |
| **AES-GCM** | AEAD (fallback) | Alternative for hardware-accelerated environments |

### Encryption Flow

```
Receipt Plaintext
       │
       ▼
┌──────────────┐     ┌──────────────┐
│ Generate     │────▶│ Encrypt with │────▶ Ciphertext + Auth Tag
│ nonce (12B)  │     │ ChaCha20-P.  │
└──────────────┘     └──────────────┘
                           │
                           ▼
                    Upload to Fileverse
                           │
                           ▼
                  Frontend downloads ciphertext
                           │
                           ▼
                  Decrypt client-side with user key
```

---

## Key Principles

| Principle | Implementation |
| --------- | -------------- |
| 🚫 **No private keys stored server-side** | Backend never holds recipient private keys |
| 🔥 **Ephemeral keys destroyed after use** | `zeroize` crate ensures secure memory clearing |
| 🔒 **All encryption performed locally** | Receipt decryption happens in the browser, not the server |
| 🎲 **Each address is unique** | Fresh ephemeral key per payment = unique shared secret = unique address |
| 🔗 **Unlinkable addresses** | Without the recipient's private key, addresses cannot be connected |

---

## Cryptographic Libraries

| Library | Purpose |
| ------- | ------- |
| `k256` | Secp256k1 curve operations (ECDH, key derivation) |
| `hkdf` | Key derivation function for shared secret expansion |
| `sha3` / `keccak` | Ethereum address derivation from public key |
| `chacha20poly1305` | Primary authenticated encryption |
| `aes-gcm` | Fallback authenticated encryption |
| `zeroize` | Secure ephemeral key destruction |

---

→ See [CONTRACTS.md](./CONTRACTS.md) for on-chain stealth address resolution.
→ See [SECURITY.md](./SECURITY.md) for the full security model.
