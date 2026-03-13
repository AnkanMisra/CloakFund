use hex;
use k256::{
    ProjectivePoint, PublicKey, Scalar, SecretKey,
    elliptic_curve::{PrimeField, rand_core::OsRng, sec1::ToEncodedPoint},
};
use sha3::{Digest, Keccak256};

/// Generates a stealth address and the corresponding ephemeral public key.
///
/// # Arguments
/// * `recipient_pub_hex` - The hex-encoded public key of the recipient (compressed or uncompressed).
///
/// # Returns
/// A tuple containing:
/// 1. `stealth_address`: The generated Ethereum-style address (hex string with 0x prefix).
/// 2. `ephemeral_pub_hex`: The hex-encoded ephemeral public key to be shared with the recipient.
pub fn generate_stealth_address(recipient_pub_hex: &str) -> Result<(String, String), String> {
    // 1. Parse the recipient's public key
    let recipient_pub_bytes = hex::decode(recipient_pub_hex.trim_start_matches("0x"))
        .map_err(|e| format!("Invalid hex: {}", e))?;
    let recipient_pub = PublicKey::from_sec1_bytes(&recipient_pub_bytes)
        .map_err(|e| format!("Invalid public key: {}", e))?;

    // 2. Generate an ephemeral private key for the sender
    let ephemeral_secret = SecretKey::random(&mut OsRng);
    let ephemeral_pub = ephemeral_secret.public_key();

    // 3. Compute the shared secret (ECDH): S = r * P
    let shared_point = k256::ecdh::diffie_hellman(
        ephemeral_secret.to_nonzero_scalar(),
        recipient_pub.as_affine(),
    );
    let shared_secret = shared_point.raw_secret_bytes();

    // 4. Derive a deterministic scalar from the shared secret
    // We use Keccak256 as the KDF for simplicity in this EVM-compatible context
    let mut hasher = Keccak256::new();
    hasher.update(&shared_secret);
    let hashed_secret = hasher.finalize();

    let _view_tag = hashed_secret[0]; // Optional: view tag for faster scanning

    // Derive scalar from the hash
    let derived_scalar = Scalar::from_repr(hashed_secret.into()).unwrap();

    // 5. Compute the stealth public key: P_stealth = P + hash(S)*G
    let stealth_point = recipient_pub.to_projective() + ProjectivePoint::GENERATOR * derived_scalar;
    let stealth_pub = PublicKey::try_from(stealth_point.to_affine())
        .map_err(|_| "Failed to derive stealth public key")?;

    // 6. Generate the EVM address from the stealth public key
    let encoded_point = stealth_pub.to_encoded_point(false);
    let pub_bytes = encoded_point.as_bytes();

    // The first byte is the 0x04 prefix for uncompressed keys, we skip it
    let mut addr_hasher = Keccak256::new();
    addr_hasher.update(&pub_bytes[1..]);
    let addr_hash = addr_hasher.finalize();

    // Address is the last 20 bytes of the Keccak256 hash
    let address_bytes = &addr_hash[12..32];
    let stealth_address = format!("0x{}", hex::encode(address_bytes));

    // 7. Format the ephemeral public key to return (compressed)
    let ephem_pub_encoded = ephemeral_pub.to_encoded_point(true);
    let ephemeral_pub_hex = format!("0x{}", hex::encode(ephem_pub_encoded.as_bytes()));

    Ok((stealth_address, ephemeral_pub_hex))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_stealth_address() {
        // A random valid secp256k1 public key (compressed)
        let recipient_pub = "037c865e90d33e5066922d0a2736b4b47814b62db4ccf3e2b2bb9ab8f8df4f4cb3";

        let result = generate_stealth_address(recipient_pub);
        assert!(result.is_ok(), "Failed to generate stealth address");

        let (address, ephem_pub) = result.unwrap();

        // Basic EVM address validation
        assert!(address.starts_with("0x"));
        assert_eq!(address.len(), 42); // 0x + 40 hex chars

        // Ephemeral pubkey validation
        assert!(ephem_pub.starts_with("0x"));
        assert_eq!(ephem_pub.len(), 68); // 0x + 66 hex chars (33 bytes compressed)
    }
}
