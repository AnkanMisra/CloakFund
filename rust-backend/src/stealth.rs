use hex;
use k256::{
    NonZeroScalar, ProjectivePoint, PublicKey, Scalar, SecretKey,
    elliptic_curve::{PrimeField, rand_core::OsRng, sec1::ToEncodedPoint},
};
use sha3::{Digest, Keccak256};
use std::ops::Add;

fn to_checksum_address(address: &str) -> String {
    let addr = address.trim_start_matches("0x").to_lowercase();
    let mut hasher = Keccak256::new();
    hasher.update(addr.as_bytes());
    let hash = hasher.finalize();

    let mut checksum = String::from("0x");
    for (i, ch) in addr.chars().enumerate() {
        if ch.is_numeric() {
            checksum.push(ch);
        } else {
            let byte = hash[i / 2];
            let nibble = if i % 2 == 0 { byte >> 4 } else { byte & 0xf };
            checksum.push(if nibble >= 8 {
                ch.to_ascii_uppercase()
            } else {
                ch
            });
        }
    }
    checksum
}

/// Generates a stealth address and the corresponding ephemeral public key.
///
/// # Arguments
/// * `recipient_pub_hex` - The hex-encoded public key of the recipient (compressed or uncompressed).
///
/// # Returns
/// A tuple containing:
/// 1. `stealth_address`: The generated Ethereum-style address (hex string with 0x prefix).
/// 2. `ephemeral_pub_hex`: The hex-encoded ephemeral public key to be shared with the recipient.
/// 3. `view_tag`: The first byte of the hashed secret for fast scanning.
pub fn generate_stealth_address(recipient_pub_hex: &str) -> Result<(String, String, u8), String> {
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

    let view_tag = hashed_secret[0]; // Optional: view tag for faster scanning

    // Derive scalar from the hash
    let derived_scalar_opt: Option<Scalar> = Scalar::from_repr(hashed_secret.into()).into();
    let derived_scalar = derived_scalar_opt.ok_or("Hash output exceeded curve order")?;

    // 5. Compute the stealth public key: P_stealth = P + hash(S)*G
    let stealth_point: ProjectivePoint =
        recipient_pub.to_projective() + ProjectivePoint::GENERATOR * derived_scalar;
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
    let stealth_address = to_checksum_address(&format!("0x{}", hex::encode(address_bytes)));

    // 7. Format the ephemeral public key to return (compressed)
    let ephem_pub_encoded = ephemeral_pub.to_encoded_point(true);
    let ephemeral_pub_hex = format!("0x{}", hex::encode(ephem_pub_encoded.as_bytes()));

    Ok((stealth_address, ephemeral_pub_hex, view_tag))
}

pub fn recover_stealth_private_key(
    recipient_priv_hex: &str,
    ephemeral_pub_hex: &str,
) -> Result<String, String> {
    // Parse recipient's private key
    let priv_bytes = hex::decode(recipient_priv_hex.trim_start_matches("0x"))
        .map_err(|e| format!("Invalid hex: {}", e))?;

    // SecretKey parsing expects exactly 32 bytes; validate to prevent panics in lower-level crates.
    if priv_bytes.len() != 32 {
        return Err(format!(
            "Invalid private key length: expected 32 bytes, got {}",
            priv_bytes.len()
        ));
    }

    let recipient_priv = SecretKey::from_bytes((&priv_bytes[..]).into())
        .map_err(|e| format!("Invalid private key: {}", e))?;

    // Parse ephemeral public key
    let ephem_bytes = hex::decode(ephemeral_pub_hex.trim_start_matches("0x"))
        .map_err(|e| format!("Invalid hex: {}", e))?;
    let ephemeral_pub = PublicKey::from_sec1_bytes(&ephem_bytes)
        .map_err(|e| format!("Invalid public key: {}", e))?;

    // Compute shared secret: S = p * R
    let shared_point = k256::ecdh::diffie_hellman(
        recipient_priv.to_nonzero_scalar(),
        ephemeral_pub.as_affine(),
    );
    let shared_secret = shared_point.raw_secret_bytes();

    // Hash to derive scalar
    let mut hasher = Keccak256::new();
    hasher.update(&shared_secret);
    let hashed_secret = hasher.finalize();

    let derived_scalar_opt: Option<Scalar> = Scalar::from_repr(hashed_secret.into()).into();
    let derived_scalar = derived_scalar_opt.ok_or("Hash output exceeded curve order")?;

    // Compute stealth private key: p_stealth = p + h
    let recipient_scalar = *recipient_priv.to_nonzero_scalar();
    let stealth_scalar = recipient_scalar.add(&derived_scalar);

    let non_zero_stealth_scalar_opt: Option<NonZeroScalar> =
        NonZeroScalar::new(stealth_scalar).into();
    let non_zero_stealth_scalar = non_zero_stealth_scalar_opt.ok_or("Stealth scalar is zero")?;
    let stealth_priv = SecretKey::from(non_zero_stealth_scalar);
    Ok(format!("0x{}", hex::encode(stealth_priv.to_bytes())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stealth_address_roundtrip() {
        let recipient_priv = "0x1111111111111111111111111111111111111111111111111111111111111111";
        let recipient_pub = "0x034f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa";

        let (stealth_addr, ephem_pub, _view_tag) = generate_stealth_address(recipient_pub).unwrap();
        let recovered_priv = recover_stealth_private_key(recipient_priv, &ephem_pub).unwrap();

        // Verify recovered private key produces same stealth address
        let priv_bytes = hex::decode(recovered_priv.trim_start_matches("0x")).unwrap();
        let secret = SecretKey::from_bytes((&priv_bytes[..]).into()).unwrap();
        let stealth_pub = secret.public_key();

        let encoded = stealth_pub.to_encoded_point(false);
        let mut hasher = Keccak256::new();
        hasher.update(&encoded.as_bytes()[1..]);
        let hash = hasher.finalize();
        let derived_addr = format!("0x{}", hex::encode(&hash[12..32]));

        assert_eq!(
            stealth_addr.to_lowercase(),
            to_checksum_address(&derived_addr).to_lowercase()
        );
    }

    #[test]
    fn test_generate_fails_on_invalid_recipient_pubkey() {
        let bad_pub = "0x1234";
        let res = generate_stealth_address(bad_pub);
        assert!(res.is_err());
    }

    #[test]
    fn test_recover_fails_on_invalid_recipient_privkey() {
        let bad_priv = "0x1234";
        let ephem = "0x03f46d7511c5e2fdb5cc698dab27db5a34162d4a06c0b131c98af26e9ac5709a7b";
        let res = recover_stealth_private_key(bad_priv, ephem);
        assert!(res.is_err());
    }

    #[test]
    fn test_recover_fails_on_invalid_ephemeral_pubkey() {
        let recipient_priv = "0x1111111111111111111111111111111111111111111111111111111111111111";
        let bad_ephem = "0x1234";
        let res = recover_stealth_private_key(recipient_priv, bad_ephem);
        assert!(res.is_err());
    }
}
