# ENS CCIP-Read (Ghost Resolver) Implementation

## Overview
To win the ENS Integration prize, we have implemented **The Ghost Resolver**, a completely non-deterministic ENS resolution system powered by **EIP-3668 (CCIP-Read)**. 

This turns ENS from a static phonebook into a **Zero-Click Privacy Engine**. 

Instead of sending money to a static address, a sender's wallet asks the ENS `CloakResolver` for an address. The Resolver throws an `OffchainLookup` error, which redirects the sender's wallet to our Rust Backend Gateway. The Rust Backend dynamically generates a stealth address on-the-fly using Elliptic Curve cryptography, signs it, and returns it to the wallet.

## Files Added

1. `contracts/src/CloakResolver.sol` 
   - A custom ENS Resolver that throws the `OffchainLookup` error and defines the `resolveWithProof` callback to verify the Rust backend's signature.
   
2. `rust-backend/src/ccip.rs`
   - The CCIP-Read Gateway. It listens on `/gateway/{sender}/{data}.json`, dynamically generates the EIP-5564 stealth address, signs it with the Gateway's private key, and returns the ABI-encoded payload.

3. `docs/ENS_CCIP_READ_RESEARCH.md`
   - Explains the architectural significance of breaking deterministic ENS resolution and why it is a hackathon-winning feature.

## How it works (The UX)

1. Alice owns `alice.eth`. She points her ENS Resolver to the `CloakResolver` smart contract.
2. Bob wants to send Alice 1 ETH. He opens MetaMask, types `alice.eth` in the "Send" box, and clicks "Next."
3. **Behind the scenes:** MetaMask asks the `CloakResolver` for Alice's address. The `CloakResolver` says *"I don't know, but go ask the CloakFund Rust Gateway."*
4. MetaMask instantly sends a request to your Rust Backend.
5. Your Rust Backend generates a brand new, one-time use stealth address (e.g., `0x987...`), signs it, and sends it back to MetaMask.
6. MetaMask automatically verifies the signature on-chain and uses `0x987...` as the destination address.
7. Bob clicks "Confirm".

To Bob, he just sent money to `alice.eth`. To the blockchain, the money went to a completely random stealth address, protecting Alice's privacy effortlessly.

## How to Demo to Judges

1. Deploy `CloakResolver.sol` to a testnet (e.g., Base Sepolia).
2. Tell the `CloakResolver` your Rust backend's URL (e.g., `https://api.cloakfund.com/gateway/{sender}/{data}.json`) and the Gateway's public signer address.
3. Configure an ENS name (e.g., `pay.alice.eth`) to use your `CloakResolver`.
4. Run the Rust Backend (`cargo run -- serve`).
5. Open MetaMask, send ETH to `pay.alice.eth`. 
6. Show the judges how MetaMask automatically fetches the *dynamic stealth address* from the Rust server and sends the funds there instead of a main wallet.

**Why this wins:** You are demonstrating advanced, cross-chain, non-deterministic ENS resolution to solve the biggest UX problem in Web3 privacy.