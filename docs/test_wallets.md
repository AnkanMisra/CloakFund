# Test Wallets (Base Sepolia Testnet)

This document contains pre-funded test wallet addresses used for development, integration testing, and demoing the CloakFund protocol on the Base network (Testnet).

**⚠️ WARNING: DO NOT send real mainnet funds to these addresses. They are for testnet use only.**

## Wallet 1: Sender / Judge Account
Used to simulate a user, sponsor, or hackathon judge sending funds to a CloakFund-generated stealth address.

*   **Address:** `0x71C7656EC7ab88b098defB751B7401B5f6d8976F`
*   **Network:** Base Sepolia
*   **Role:** Payer
*   **Explorer Link:** [View on Basescan](https://sepolia.basescan.org/address/0x71C7656EC7ab88b098defB751B7401B5f6d8976F)

## Wallet 2: Receiver Identity (`alice.eth` mock)
Used to simulate the receiver who owns the ENS identity and accesses the CloakFund dashboard to aggregate stealth payments.

*   **Address:** `0xF24E82A1C3C0B1B3a47d25e407DDF82e5b7c8441`
*   **Network:** Base Sepolia
*   **Role:** Payee / ENS Owner
*   **Explorer Link:** [View on Basescan](https://sepolia.basescan.org/address/0xF24E82A1C3C0B1B3a47d25e407DDF82e5b7c8441)

## Wallet 3: BitGo MPC Treasury (Mock)
Used as the destination address for the consolidation flow.

*   **Address:** `0x9A4b0c74EaD5618A123f81D0197D6f7D5F3134C4`
*   **Network:** Base Sepolia
*   **Role:** Treasury Vault
*   **Explorer Link:** [View on Basescan](https://sepolia.basescan.org/address/0x9A4b0c74EaD5618A123f81D0197D6f7D5F3134C4)

---

## Faucets
If the test wallets require additional testnet ETH for the demo or integration tests, funds can be acquired from:
*   [Alchemy Base Sepolia Faucet](https://www.alchemy.com/faucets/base-sepolia)
*   [QuickNode Base Sepolia Faucet](https://faucet.quicknode.com/base/sepolia)