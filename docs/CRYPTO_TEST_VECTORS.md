# Cryptographic Test Vectors

This document contains test vectors used to validate the implementation of the CloakFund stealth address generator. These vectors ensure that given a specific recipient public key and ephemeral sender key, the derived shared secret, derived scalar, and resulting stealth EVM address are deterministic and mathematically correct.

## Test Vector 1: Basic Stealth Address Derivation

This vector uses a fixed ephemeral private key to ensure the resulting stealth address is deterministic.

**Inputs:**
*   **Recipient Private Key (for reference):** `0x1111111111111111111111111111111111111111111111111111111111111111`
*   **Recipient Public Key (Compressed):** `0x034f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa`
*   **Sender Ephemeral Private Key (r):** `0x2222222222222222222222222222222222222222222222222222222222222222`

**Intermediate Values:**
*   **Sender Ephemeral Public Key (R = r * G):** `0x02466d7fcae563e5cb09a0d1870bb580344804617879a14949cf22285f1bae3f27`
*   **Shared Secret Point (S = r * Recipient_PubKey):** *(ECDH Point)*
*   **Hashed Secret (h = Keccak256(S.x)):** *(Deterministic 32-byte hash)*
*   **View Tag:** *(First byte of hashed secret)*

**Expected Outputs:**
*   **Stealth Public Key (P_stealth = Recipient_PubKey + h * G):** *(Uncompressed 64-byte key)*
*   **Stealth EVM Address:** *(Last 20 bytes of Keccak256(P_stealth))*

## Test Vector 2: Handling Edge Cases

*   **Scenario:** Extremely large scalar values or leading zeros in the hash output.
*   **Recipient Public Key:** `0x02836b35a026743e823a90a0ee3b91bf615c6a757e2b60b9e1dc1826fd0dd16106`
*   *(Additional vectors will be added as the implementation scales)*

## Running the Tests

To execute the test vectors against the Rust backend implementation:

```bash
cd rust-backend
cargo test --package rust-backend --bin rust-backend -- stealth::tests
```
