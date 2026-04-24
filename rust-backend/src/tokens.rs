//! Helpers for paylink revocation tokens.
//!
//! A revocation token is a 32-byte random value handed to the paylink creator
//! exactly once (in the `POST /api/v1/paylink` response). Only `sha256(token)`
//! is persisted server-side, so the plaintext never touches the database and
//! cannot be recovered from a stolen Convex snapshot.

use anyhow::{Result, anyhow};
use k256::elliptic_curve::rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

/// Generates a fresh 32-byte revocation token, hex-encoded with a `0x` prefix.
///
/// Output format: `"0x"` + 64 lowercase hex chars (total length 66).
pub fn generate_revocation_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("0x{}", hex::encode(bytes))
}

/// Returns the hex-encoded sha256 of the given token (no `0x` prefix, lowercase).
///
/// Accepts both `0x`-prefixed and unprefixed inputs so that callers don't have
/// to normalize before passing through. Validates that the token decodes to
/// exactly 32 bytes.
pub fn hash_revocation_token(token: &str) -> Result<String> {
    let trimmed = token.trim();
    let raw = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    let bytes = hex::decode(raw).map_err(|e| anyhow!("invalid hex: {}", e))?;
    if bytes.len() != 32 {
        return Err(anyhow!(
            "revocation token must be 32 bytes, got {}",
            bytes.len()
        ));
    }
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_token_is_32_byte_hex_with_prefix() {
        let token = generate_revocation_token();
        assert_eq!(token.len(), 66);
        assert!(token.starts_with("0x"));
        let raw = hex::decode(token.trim_start_matches("0x")).unwrap();
        assert_eq!(raw.len(), 32);
    }

    #[test]
    fn two_generated_tokens_differ() {
        let a = generate_revocation_token();
        let b = generate_revocation_token();
        assert_ne!(a, b, "OsRng produced the same token twice");
    }

    #[test]
    fn hash_is_deterministic_across_prefix_casing_and_whitespace() {
        let token = "0xabcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        let h1 = hash_revocation_token(token).unwrap();
        let h2 = hash_revocation_token(&token.to_uppercase()).unwrap();
        let h3 = hash_revocation_token(
            "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
        )
        .unwrap();
        let h4 = hash_revocation_token(&format!("  {token}  ")).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1, h3);
        assert_eq!(h1, h4);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn hash_rejects_wrong_length() {
        let err = hash_revocation_token("0x1234").unwrap_err();
        assert!(err.to_string().contains("32 bytes"));
    }

    #[test]
    fn hash_rejects_non_hex() {
        assert!(hash_revocation_token("0xZZ").is_err());
    }

    #[test]
    fn generated_token_roundtrips_through_hash() {
        let token = generate_revocation_token();
        let h = hash_revocation_token(&token).unwrap();
        let h_again = hash_revocation_token(&token).unwrap();
        assert_eq!(h, h_again);
    }
}
