# 🔐 Cryptography

CloakFund uses elliptic curve cryptography to generate stealth addresses for one-time payments.

This document describes the **stealth payment** cryptographic workflow implemented in the Rust backend (Phase 1).

---

## Stealth Address Generation (Sender Side)

CloakFund generates a fresh stealth address per payment using ECDH on secp256k1:

1. **Input:** Recipient public key `P` (secp256k1 public key, SEC1 compressed/uncompressed hex).
2. **Ephemeral keypair:** Sender generates a random ephemeral secret key `r` and public key `R = r·G`.
3. **Shared secret:** Compute `S = ECDH(r, P)` and take `shared_secret = S` (as bytes).
4. **Derive scalar:** Compute `h = Keccak256(shared_secret)` and interpret `h` as a curve scalar.
   - If `h` is out of range (≥ curve order), the implementation returns an error instead of panicking.
5. **Stealth public key:** `P_stealth = P + h·G`
6. **Stealth EVM address:** `addr = last20bytes(Keccak256(uncompressed(P_stealth)[1..]))`
7. **Checksum formatting:** The returned address is formatted as an EIP-55 checksum address.

### View Tag (for faster scanning)

The implementation returns a `view_tag` which is the **first byte** of `Keccak256(shared_secret)`.

This tag can be published alongside the ephemeral pubkey to help recipients quickly filter candidate announcements when scanning.

Returned tuple from the backend stealth generator:
- `stealth_address` (EIP-55 checksummed)
- `ephemeral_pub_hex` (compressed SEC1 `R`)
- `view_tag` (u8)

---

## Stealth Private Key Recovery (Recipient Side)

To spend funds sent to the stealth address, the recipient derives the stealth private key using their private key `p` and the sender’s published ephemeral public key `R`:

1. **Input:** Recipient private key `p` and ephemeral public key `R`.
2. **Shared secret:** Compute `S = ECDH(p, R)` and `shared_secret = S` (as bytes).
3. **Derive scalar:** `h = Keccak256(shared_secret)` interpreted as a curve scalar (with the same range checks as sender side).
4. **Stealth private key:** `p_stealth = p + h` (mod curve order)

This makes the protocol **bidirectional**: sender can generate the stealth address; recipient can recover the corresponding private key to spend.

---

## Encryption (Receipts)

| Algorithm | Type | Use Case |
| --------- | ---- | -------- |
| **ChaCha20-Poly1305** | AEAD (primary) | High-performance encryption with authentication |
| **AES-GCM** | AEAD (fallback) | Alternative for hardware-accelerated environments |

Recommended algorithms:
- ChaCha20-Poly1305
- AES-GCM

(Note: receipt encryption/storage is implemented in later phases; this section documents the intended primitives.)

---

## Key Principles

- Private keys remain in user wallets; backend must never store private keys.
- Crypto code should not panic on malformed inputs; return errors instead.
- Ephemeral secrets should not be logged or persisted.
- Address reuse must be avoided by generating new stealth addresses per payment.
