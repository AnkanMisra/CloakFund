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

**Shared Secret Point (S = r * Recipient_PubKey):** 
`0x77e0510d5042e2f5e9e59c977b81eeed590cf7d20c1c51da451a8eaa9fdc45ff`

**Hashed Secret (h = Keccak256(S.x)):** 
`0xafde64e1f4b4af8f85678e891cbfea8ec42cc250356d40f91ed36d071f20b268`

**View Tag:** 
`0xaf`

**Expected Outputs:**
**Stealth Public Key (P_stealth):** 
`0x04ef3b657172f4abd14c9dcb782fc126928e08e5c773b241c2252184424ce275fbc86bf292eb10a1a58f5ab0ab2522ac9d80d857e97f42ab2df2502f49c9f31272`

**Stealth EVM Address:** 
`0x9817cd14301C3108dA553c572E597D666B1829c3`

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
