# BitGo Consolidation Flow

## Overview

In Phase 4, CloakFund implements a sweeper service to consolidate funds from ephemeral stealth addresses into a centralized treasury. For secure enterprise-grade custody, we integrate with BitGo.

This document outlines the architecture and integration flow for the BitGo Express Sweep API.

## Architecture

The Sweeper module operates in the following state machine:
1. `queued`: A sweep job is registered in Convex after a deposit is finalized.
2. `broadcasting`: The Sweeper picks up the job, recovers the ephemeral private key, and prepares the transaction.
3. `signed`: The transaction is signed using the ephemeral key.
4. `broadcasted`: The transaction is sent to the network.
5. `confirmed`: The sweep transaction is confirmed on-chain.
6. `completed` | `failed`: Final status in Convex.

## Sponsor Tracks and BitGo Express Integration

CloakFund leverages the **BitGo Sponsor Tracks** for enterprise-grade wallet management and automation. To participate in these tracks, we integrate with the **BitGo Express API**.

The BitGo Express Sweep API (`POST /api/v2/{coin}/wallet/{walletId}/sweep`) allows us to sweep the full balance of an existing BitGo-managed wallet to a destination address.

While our ephemeral stealth addresses are mathematically derived (Externally Owned Accounts) and not directly natively generated within a BitGo wallet structure (due to the stealth addressing cryptographic requirements), we can still use BitGo to secure our centralized treasury and orchestrate fund flows.

### Dual Sweeping Architecture

1. **From Ephemeral to BitGo Treasury (Standard RPC)**:
   - The Sweeper retrieves the `ephemeral_pubkey` from Convex.
   - The ephemeral private key is derived securely in memory using our stealth crypto module.
   - We use the `ethers-rs` provider to construct and sign a transaction that sweeps funds from the ephemeral address directly into our **BitGo Treasury Wallet** deposit address.

2. **From BitGo Wallet to Final Destination (BitGo Express API)**:
   - Once funds land in the BitGo Treasury Wallet, we can utilize the `BitGoClient` (which wraps the BitGo Express API) to orchestrate further movements.
   - For example, we can trigger a `sweep` call (`POST /api/v2/{coin}/wallet/{walletId}/sweep`) to consolidate sub-wallets or distribute funds according to the sponsor track requirements.

### Standard RPC Broadcaster (Ephemeral Sweeping)

1. The Sweeper derives the ephemeral private key in memory using the `zeroize` crate to ensure it's securely wiped after use.
2. An `ethers-rs` `LocalWallet` is constructed from the ephemeral private key.
3. The gas cost is calculated for a standard ETH transfer (or ERC20 transfer).
4. The remaining balance (Total - Gas Cost) is sent to the `TREASURY_ADDRESS` (which is a BitGo deposit address).
5. The transaction hash is recorded in Convex and the job status is updated.

## Security Considerations

- **Zeroization**: Ephemeral private keys must NEVER be logged or persisted. They are wiped from memory immediately after signing using `ZeroizeOnDrop`.
- **Dry Run**: Use `DRY_RUN=true` during development to simulate the sweep process without actually broadcasting transactions or moving funds.
- **Gas**: Ensure the ephemeral address has sufficient balance to cover the gas cost of the sweep. If not, the sweep is skipped or marked as failed/insufficient funds.