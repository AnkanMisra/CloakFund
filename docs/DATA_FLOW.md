# CloakFund — Data Flow

> Complete data-flow reference for the CloakFund privacy-preserving payment platform.

---

## 1. High-Level System Architecture

Five architectural layers interact in a strictly unidirectional pipeline:

| #   | Layer          | Technology           | Responsibility                                                                |
| --- | -------------- | -------------------- | ----------------------------------------------------------------------------- |
| 1   | **Frontend**   | Next.js / TypeScript | Wallet connection, ENS input, payment links, dashboard, receipt decryption    |
| 2   | **Backend**    | Rust (Tokio async)   | Stealth address generation, watcher, treasury engine, encryption              |
| 3   | **Blockchain** | Base (L2)            | On-chain settlement, smart contracts (`PaymentResolver`, `TreasuryForwarder`) |
| 4   | **Treasury**   | BitGo MPC            | Multi-sig custody, fund consolidation                                         |
| 5   | **Storage**    | Fileverse            | Encrypted receipt & financial record persistence                              |

```mermaid
flowchart LR
    subgraph "Client"
        U((("👤 User\nWallet")))
        FE["Next.js Frontend\n(TypeScript)"]
    end

    subgraph "Backend  —  Rust / Tokio"
        API{{"🦀 Rust API Server"}}
        SG["Stealth Generator\n(ECDH)"]
        WS["Watcher Service\n(ethers-rs)"]
        TE["Treasury Engine"]
        ES["Encryption Service\n(ChaCha20 / AES-GCM)"]
    end

    subgraph "Blockchain  —  Base L2"
        BC[("Base\nBlockchain")]
        PR["PaymentResolver\n(Smart Contract)"]
        TF["TreasuryForwarder\n(Smart Contract)"]
    end

    subgraph "External Services"
        BG["BitGo MPC\nTreasury Vault"]
        FV[("Fileverse\nEncrypted Storage")]
        ENS["ENS\nIdentity Layer"]
        AI["HeyElsa\nAI Monitor"]
    end

    U -->|"Connect wallet\n& ENS identity"| FE
    FE -->|"POST /paylink\nGET /deposit-status\nGET /receipts"| API
    API --> SG
    API --> WS
    API --> TE
    API --> ES
    SG -->|"Derive stealth address"| BC
    WS -->|"Listen for events"| BC
    BC --- PR
    BC --- TF
    TE -->|"POST /consolidate"| TF
    TF -->|"Forward funds"| BG
    ES -->|"Store encrypted receipt"| FV
    FE -->|"Resolve ENS"| ENS
    WS -.->|"Alert large payments"| AI
    FV -->|"Retrieve receipts"| FE

    style U fill:#6C63FF,stroke:#4A44CC,color:#fff
    style FE fill:#1E1E2E,stroke:#6C63FF,color:#CDD6F4
    style API fill:#F38BA8,stroke:#D6336C,color:#1E1E2E
    style SG fill:#FAB387,stroke:#E87D3E,color:#1E1E2E
    style WS fill:#FAB387,stroke:#E87D3E,color:#1E1E2E
    style TE fill:#FAB387,stroke:#E87D3E,color:#1E1E2E
    style ES fill:#FAB387,stroke:#E87D3E,color:#1E1E2E
    style BC fill:#A6E3A1,stroke:#4CAF50,color:#1E1E2E
    style PR fill:#94E2D5,stroke:#2D9E8F,color:#1E1E2E
    style TF fill:#94E2D5,stroke:#2D9E8F,color:#1E1E2E
    style BG fill:#F9E2AF,stroke:#D4A017,color:#1E1E2E
    style FV fill:#CBA6F7,stroke:#9B59B6,color:#1E1E2E
    style ENS fill:#89B4FA,stroke:#3B82F6,color:#1E1E2E
    style AI fill:#F2CDCD,stroke:#E8A0A0,color:#1E1E2E
```

---

## 2. End-to-End Payment Flow

A complete transaction progresses through **nine sequential steps** (event-driven):

```mermaid
sequenceDiagram
    autonumber

    actor Sender as 👤 Sender
    participant FE as Next.js Frontend
    participant ENS as ENS Registry
    participant API as 🦀 Rust API
    participant SG as Stealth Generator
    participant BC as Base Blockchain
    participant WS as Watcher Service
    participant TE as Treasury Engine
    participant ES as Encryption Service
    participant BG as BitGo MPC Vault
    participant FV as Fileverse Storage
    participant AI as HeyElsa AI

    Sender->>FE: Connect wallet
    FE->>ENS: Resolve recipient ENS name
    ENS-->>FE: Return ENS metadata

    rect rgb(30, 30, 46)
        Note over FE,SG: Step 1-4 — Address Generation
        FE->>API: POST /paylink (ENS identity)
        API->>SG: Request stealth address
        SG->>SG: Generate ephemeral key pair
        SG->>SG: Derive shared secret (ECDH)
        SG->>SG: Compute one-time stealth address
        SG-->>API: Return stealth address
        API-->>FE: Return payment link
    end

    rect rgb(46, 30, 30)
        Note over Sender,BC: Step 5 — Fund Transfer
        FE-->>Sender: Display payment link / QR
        Sender->>BC: Transfer funds to stealth address
    end

    rect rgb(30, 46, 30)
        Note over BC,FV: Step 6-8 — Detection & Storage
        BC--)WS: Emit deposit event
        WS->>API: Notify deposit detected
        API->>API: GET /deposit-status (confirmed)
        API->>ES: Encrypt payment receipt
        ES->>ES: ChaCha20-Poly1305 / AES-GCM
        ES->>FV: Store encrypted receipt
    end

    rect rgb(46, 46, 30)
        Note over TE,BG: Step 9 — Treasury Consolidation
        API->>TE: POST /consolidate
        TE->>BC: Call TreasuryForwarder contract
        BC->>BG: Forward funds to MPC vault
    end

    WS-->>FE: Push dashboard update
    FE-->>Sender: Display updated balance
    WS--)AI: Alert on large payment (optional)
    Sender->>FE: GET /receipts (decrypt locally)
    FE->>FV: Fetch encrypted receipt
    FV-->>FE: Return ciphertext
    FE->>FE: Decrypt receipt client-side
```

---

## 3. Rust Backend — Internal Service Map

All backend modules run as **concurrent Tokio tasks** sharing an async runtime:

```mermaid
flowchart TB
    subgraph "Tokio Async Runtime"
        direction TB

        API["API Server\n─────────────\nPOST /paylink\nGET /deposit-status\nPOST /consolidate\nGET /receipts"]

        subgraph "Core Services"
            direction LR
            SG["Stealth Generator\n─────────────\nECDH key derivation\nEphemeral key mgmt\nOne-time address output"]
            WS["Watcher Service\n─────────────\nethers-rs provider\nBlock polling / WS\nEvent filtering"]
            TE["Treasury Engine\n─────────────\nConsolidation logic\nBatch transfer builder\nBitGo API calls"]
            ES["Encryption Service\n─────────────\nChaCha20-Poly1305\nAES-GCM fallback\nKey-less server design"]
        end
    end

    EXT_BC[("Base Blockchain")]
    EXT_BG["BitGo MPC"]
    EXT_FV[("Fileverse")]
    EXT_FE["Next.js Frontend"]

    EXT_FE <-->|"HTTP / JSON"| API
    API --> SG
    API --> WS
    API --> TE
    API --> ES
    SG -->|"Publish stealth addr"| EXT_BC
    WS -->|"Subscribe to events"| EXT_BC
    TE -->|"Custody API"| EXT_BG
    ES -->|"Store ciphertext"| EXT_FV

    style API fill:#F38BA8,stroke:#D6336C,color:#1E1E2E
    style SG fill:#FAB387,stroke:#E87D3E,color:#1E1E2E
    style WS fill:#A6E3A1,stroke:#4CAF50,color:#1E1E2E
    style TE fill:#F9E2AF,stroke:#D4A017,color:#1E1E2E
    style ES fill:#CBA6F7,stroke:#9B59B6,color:#1E1E2E
    style EXT_BC fill:#94E2D5,stroke:#2D9E8F,color:#1E1E2E
    style EXT_BG fill:#F9E2AF,stroke:#D4A017,color:#1E1E2E
    style EXT_FV fill:#CBA6F7,stroke:#9B59B6,color:#1E1E2E
    style EXT_FE fill:#1E1E2E,stroke:#6C63FF,color:#CDD6F4
```

---

## 4. Stealth Address Cryptography Flow

The privacy-critical path that ensures **no two payments share an address**:

```mermaid
flowchart TD
    A["Sender picks recipient\n(alice.eth)"] --> B["Generate ephemeral\nkey pair (r, R)"]
    B --> C["Fetch recipient's\nstealth meta-address (K)"]
    C --> D["Compute shared secret\nS = ECDH(r, K)"]
    D --> E["Derive one-time\nstealth address\naddr = hash(S) · G + K"]
    E --> F["Sender transfers ETH\nto stealth address"]
    F --> G["Ephemeral key r\ndestroyed"]
    G --> H["Recipient scans\nblockchain with K⁻¹"]
    H --> I["Recipient recovers S\nand derives private key"]
    I --> J["Recipient controls\nstealth address funds"]

    style A fill:#89B4FA,stroke:#3B82F6,color:#1E1E2E
    style B fill:#FAB387,stroke:#E87D3E,color:#1E1E2E
    style C fill:#89B4FA,stroke:#3B82F6,color:#1E1E2E
    style D fill:#F38BA8,stroke:#D6336C,color:#1E1E2E
    style E fill:#F38BA8,stroke:#D6336C,color:#1E1E2E
    style F fill:#A6E3A1,stroke:#4CAF50,color:#1E1E2E
    style G fill:#F9E2AF,stroke:#D4A017,color:#1E1E2E
    style H fill:#CBA6F7,stroke:#9B59B6,color:#1E1E2E
    style I fill:#CBA6F7,stroke:#9B59B6,color:#1E1E2E
    style J fill:#A6E3A1,stroke:#4CAF50,color:#1E1E2E
```

---

## 5. API Endpoints Summary

| Method | Endpoint          | Description                        | Triggered By                 |
| ------ | ----------------- | ---------------------------------- | ---------------------------- |
| `POST` | `/paylink`        | Generate a stealth payment address | Frontend → Stealth Generator |
| `GET`  | `/deposit-status` | Check payment confirmation status  | Frontend → Watcher Service   |
| `POST` | `/consolidate`    | Move funds to BitGo MPC treasury   | Backend → Treasury Engine    |
| `GET`  | `/receipts`       | Retrieve encrypted receipt list    | Frontend → Fileverse         |

---

## 6. Security Invariants Across the Flow

| Invariant                            | Enforced By                                  |
| ------------------------------------ | -------------------------------------------- |
| No private keys stored server-side   | Rust backend design — stateless key handling |
| Ephemeral keys destroyed after use   | Stealth Generator — in-memory only           |
| All encryption performed client-side | Frontend decrypts receipts locally           |
| Stealth addresses prevent clustering | ECDH-derived one-time addresses              |
| Treasury requires multi-sig approval | BitGo MPC — threshold signatures             |
| Metadata encrypted at rest           | Fileverse — ChaCha20-Poly1305 / AES-GCM      |
