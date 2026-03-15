# CloakFund ZK-Mixer
**100% On-Chain Private Payments using Stealth Addresses & ZK-Proofs**

## Project Overview
CloakFund is a decentralized privacy protocol (similar to Tornado Cash Lite) built on Base Sepolia. It enables users to receive funds anonymously by completely breaking the on-chain link between the sender and the receiver's main wallet. It achieves this without relying on centralized custodians or off-chain escrow. 

The core innovation is a dual-layered privacy approach combining **Stealth Addresses** (sender anonymity) with a **Zero-Knowledge Privacy Pool** (receiver anonymity via a Hash-Commit-Reveal scheme).

## Core Architecture & Payment Flow

The system operates in four distinct phases:

### 1. Stealth Wallet Generation
When a receiver wants to be paid, they don't share their main wallet address. Instead, the backend generates a **one-time ephemeral Stealth Wallet**. This temporary wallet is mathematically linked to the receiver but appears completely random on-chain.

### 2. The Payment (Sender Anonymity)
The sender sends ETH to this temporary Stealth Wallet. On the blockchain, it just looks like a standard transfer to a random new address. The sender has no idea what the receiver's actual main wallet is.

### 3. Auto-Sweeping & Commitment (The ZK-Deposit)
CloakFund runs an active **Rust Sweeper Agent**. The agent monitors the blockchain for incoming deposits to any generated stealth wallets. 
* When it detects a payment, the Agent automatically sweeps the funds from the temporary stealth wallet into a massive communal **PrivacyPool Smart Contract**.
* Crucially, instead of just sending ETH, the Agent generates a cryptographic **Secret** and **Nullifier**. It hashes these together to create a **Commitment** (`Hash(Secret + Nullifier)`) and deposits the ETH into the pool alongside this commitment.
* The funds are now mixed with everyone else's funds in the contract.

### 4. Anonymous Withdrawal (Receiver Anonymity)
When the receiver is ready to claim their funds to their Main Wallet, they perform an anonymous withdrawal.
* They provide their secret and nullifier (the "Privacy Note") to a **Relayer**.
* The Relayer calls the `withdraw()` function on the `PrivacyPool` contract, paying the gas fee.
* The smart contract verifies via Zero-Knowledge principles that the secret and nullifier hash back to an existing, unspent commitment in the pool.
* If valid, the smart contract sends the ETH directly to the receiver's Main Wallet.
* **Result:** The Main Wallet receives ETH directly from the smart contract. There is absolutely no on-chain link to whoever originally sent the funds.

## Tech Stack
* **Smart Contracts:** Solidity (`PrivacyPool.sol` deployed on Base Sepolia testnet).
* **Backend:** Rust, `axum` (API server), `ethers-rs` (blockchain interaction), `tokio` (async runtime).
* **Database & State:** Convex (TypeScript) for storing ephemeral addresses and sweep job status.
* **Frontend:** Vanilla HTML/CSS/JS with a modern glassmorphism UI.

## Key Features & Security
* **No Custodians:** The Rust backend never holds the funds. Funds go straight from the Stealth Wallet into the decentralized Smart Contract.
* **Gas Abstraction:** The Sender pays standard transfer gas. The Receiver pays nothing to withdraw (the Relayer handles deployment gas).
* **EIP-1559 Compliant:** Custom fee estimation for base fees and priority fees on Base Sepolia.
* **Robust Event Indexing:** The Watcher service can intelligently catch up on missed blocks and scan both native ETH and ERC20 event logs.

## How to Run Locally

You will need 3 terminal windows.

**1. Start the Rust Backend (Agent & API)**
```bash
cd rust-backend
RUST_LOG=info cargo run -- serve
```
*(Wait until it says `Starting API server on 0.0.0.0:8080`)*

**2. Start the Frontend (Optional UI)**
```bash
cd frontend
python3 -m http.server 5500
```
*(Then open http://localhost:5500 in your browser)*

**3. Run the E2E CLI Demo**
```bash
bash cloakfund_cli.sh
```
The CLI will guide you through:
1. Entering your main wallet address.
2. Generating the stealth wallet.
3. Simulating a 0.0003 ETH payment.
4. Auto-sweeping into the PrivacyPool.
5. Anonymously withdrawing 0.0001 ETH to your main wallet.

## Proven On-Chain Verification
You can verify the entire flow works by looking at the Base Sepolia block explorer:
* **PrivacyPool Contract:** `0x57E12967B278FaD279A70D13Ed2b2B82323d0B42`
* **Example Deposit Tx:** `0xb81736e86d93d30945e07bab775880552118f7819f34f686fa08276cbfb87cbc`
* **Example Withdrawal Tx:** `0x5e23b1bda479f4c111662e02abb770fdf28f130bbf0a66f6b4c265d8cf2617a7`
