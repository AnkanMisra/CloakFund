//! ABI bindings and helper functions for interacting with the PrivacyPool.sol
//! smart contract from the Rust backend.
//!
//! The contract uses a hash-commit-reveal scheme:
//!   commitment = SHA-256(secret || nullifier)
//!
//! This module provides:
//!   - `PrivacyNote`: secure in-memory representation of a note
//!   - `generate_note()`: cryptographic note generation
//!   - `compute_commitment()`: deterministic commitment computation
//!   - `build_deposit_tx()`: EIP-1559 transaction for PrivacyPool.deposit()
//!   - `build_withdraw_tx()`: EIP-1559 transaction for PrivacyPool.withdraw()

use anyhow::{Context, Result};
use ethers::abi::{self, Token};
use ethers::prelude::*;
use ethers::types::Eip1559TransactionRequest;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tracing::{debug, info};
use zeroize::{Zeroize, ZeroizeOnDrop};

// ─────────────────────────────────────────────────────────────────────────────
//  Constants
// ─────────────────────────────────────────────────────────────────────────────

/// The fixed deposit denomination: 0.0001 ETH in wei.
/// Must match PrivacyPool.sol DENOMINATION exactly.
pub const DENOMINATION_WEI: u128 = 100_000_000_000_000; // 0.0001 ETH

/// Gas limit for the `deposit(bytes32)` contract call.
/// Contract storage writes + event emission. 21000 (base) + ~60000 (SSTORE + event).
pub const DEPOSIT_GAS_LIMIT: u64 = 120_000;

/// Gas limit for the `withdraw(bytes32, bytes32, address)` contract call.
pub const WITHDRAW_GAS_LIMIT: u64 = 150_000;

/// Compute the 4-byte function selector from a Solidity function signature.
/// This uses keccak256 exactly as the EVM does.
fn selector(sig: &str) -> [u8; 4] {
    let hash = ethers::utils::keccak256(sig.as_bytes());
    [hash[0], hash[1], hash[2], hash[3]]
}

/// Returns the function selector for `deposit(bytes32)`.
fn deposit_selector() -> [u8; 4] {
    selector("deposit(bytes32)")
}

/// Returns the function selector for `withdraw(bytes32,bytes32,address)`.
fn withdraw_selector() -> [u8; 4] {
    selector("withdraw(bytes32,bytes32,address)")
}

// ─────────────────────────────────────────────────────────────────────────────
//  Types
// ─────────────────────────────────────────────────────────────────────────────

/// A privacy note containing the secret and nullifier needed to withdraw funds
/// from the PrivacyPool. This is zeroized on drop to prevent key material leaks.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct PrivacyNote {
    /// 32-byte random secret
    pub secret: [u8; 32],
    /// 32-byte random nullifier
    pub nullifier: [u8; 32],
}

/// A serializable representation of a privacy note for storage in Convex.
/// The secret and nullifier are stored as hex strings.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyNoteRecord {
    /// Hex-encoded 32-byte secret (without 0x prefix)
    pub secret_hex: String,
    /// Hex-encoded 32-byte nullifier (without 0x prefix)
    pub nullifier_hex: String,
    /// Hex-encoded 32-byte commitment (without 0x prefix)
    pub commitment_hex: String,
    /// The deposit ID this note is associated with
    pub deposit_id: String,
    /// The sweep job ID this note was created for
    pub sweep_job_id: String,
    /// The deposit tx hash into the PrivacyPool
    pub pool_deposit_tx_hash: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
//  Note Generation
// ─────────────────────────────────────────────────────────────────────────────

/// Generates a cryptographically secure privacy note with random secret and nullifier.
///
/// # Returns
/// A `PrivacyNote` containing 32-byte `secret` and 32-byte `nullifier`,
/// both generated from `OsRng`.
pub fn generate_note() -> PrivacyNote {
    use ethers::core::rand::{RngCore, rngs::OsRng};
    let mut secret = [0u8; 32];
    let mut nullifier = [0u8; 32];
    OsRng.fill_bytes(&mut secret);
    OsRng.fill_bytes(&mut nullifier);
    PrivacyNote { secret, nullifier }
}

/// Computes the SHA-256 commitment from a secret and nullifier.
///
/// commitment = SHA-256(secret || nullifier)
///
/// This must produce the exact same result as the Solidity contract:
/// `sha256(abi.encodePacked(secret, nullifier))`
pub fn compute_commitment(secret: &[u8; 32], nullifier: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(secret);
    hasher.update(nullifier);
    let result = hasher.finalize();
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&result);
    commitment
}

// ─────────────────────────────────────────────────────────────────────────────
//  Transaction Builders
// ─────────────────────────────────────────────────────────────────────────────

/// Encodes the calldata for `PrivacyPool.deposit(bytes32 commitment)`.
pub fn encode_deposit_calldata(commitment: &[u8; 32]) -> Vec<u8> {
    let tokens = vec![Token::FixedBytes(commitment.to_vec())];
    let encoded_args = abi::encode(&tokens);
    let mut calldata = deposit_selector().to_vec();
    calldata.extend_from_slice(&encoded_args);
    calldata
}

/// Encodes the calldata for `PrivacyPool.withdraw(bytes32 secret, bytes32 nullifier, address recipient)`.
pub fn encode_withdraw_calldata(
    secret: &[u8; 32],
    nullifier: &[u8; 32],
    recipient: Address,
) -> Vec<u8> {
    let tokens = vec![
        Token::FixedBytes(secret.to_vec()),
        Token::FixedBytes(nullifier.to_vec()),
        Token::Address(recipient),
    ];
    let encoded_args = abi::encode(&tokens);
    let mut calldata = withdraw_selector().to_vec();
    calldata.extend_from_slice(&encoded_args);
    calldata
}

/// Builds an EIP-1559 transaction for `PrivacyPool.deposit(commitment)`.
///
/// The transaction sends exactly `DENOMINATION_WEI` (0.0001 ETH) to the
/// PrivacyPool contract along with the commitment calldata.
pub fn build_deposit_tx(
    pool_address: Address,
    commitment: &[u8; 32],
    chain_id: u64,
    max_fee_per_gas: U256,
    max_priority_fee_per_gas: U256,
) -> Eip1559TransactionRequest {
    let calldata = encode_deposit_calldata(commitment);

    Eip1559TransactionRequest::new()
        .to(pool_address)
        .value(U256::from(DENOMINATION_WEI))
        .data(calldata)
        .gas(DEPOSIT_GAS_LIMIT)
        .max_fee_per_gas(max_fee_per_gas)
        .max_priority_fee_per_gas(max_priority_fee_per_gas)
        .chain_id(chain_id)
}

/// Builds an EIP-1559 transaction for `PrivacyPool.withdraw(secret, nullifier, recipient)`.
///
/// This is called by the Relayer, not the end user. The relayer pays gas.
pub fn build_withdraw_tx(
    pool_address: Address,
    secret: &[u8; 32],
    nullifier: &[u8; 32],
    recipient: Address,
    chain_id: u64,
    max_fee_per_gas: U256,
    max_priority_fee_per_gas: U256,
) -> Eip1559TransactionRequest {
    let calldata = encode_withdraw_calldata(secret, nullifier, recipient);

    Eip1559TransactionRequest::new()
        .to(pool_address)
        .value(U256::zero()) // No ETH sent with withdraw
        .data(calldata)
        .gas(WITHDRAW_GAS_LIMIT)
        .max_fee_per_gas(max_fee_per_gas)
        .max_priority_fee_per_gas(max_priority_fee_per_gas)
        .chain_id(chain_id)
}

// ─────────────────────────────────────────────────────────────────────────────
//  High-Level Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Fetches the current EIP-1559 fee parameters from the provider.
///
/// Returns (base_fee, priority_fee, max_fee_per_gas).
pub async fn get_eip1559_fees<P: JsonRpcClient>(
    provider: &Arc<Provider<P>>,
) -> Result<(U256, U256, U256)> {
    let latest_block = provider
        .get_block(BlockNumber::Latest)
        .await
        .context("Failed to fetch latest block")?
        .context("Latest block not found")?;

    let base_fee = latest_block.base_fee_per_gas.unwrap_or_else(|| {
        tracing::warn!("Block has no base_fee_per_gas, falling back to 1 gwei");
        U256::from(1_000_000_000)
    });

    let priority_fee: U256 = provider
        .request("eth_maxPriorityFeePerGas", ())
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to get priority fee, using 1 gwei fallback: {:?}", e);
            U256::from(1_000_000_000)
        });

    let max_fee_per_gas = base_fee
        .saturating_mul(U256::from(2))
        .saturating_add(priority_fee);

    debug!(
        "EIP-1559 fees: base_fee={}, priority_fee={}, max_fee_per_gas={}",
        base_fee, priority_fee, max_fee_per_gas
    );

    Ok((base_fee, priority_fee, max_fee_per_gas))
}

/// Executes the full deposit flow: signs and broadcasts a `deposit(commitment)`
/// transaction to the PrivacyPool contract.
///
/// Returns the transaction hash on success.
pub async fn execute_pool_deposit<P: JsonRpcClient + 'static>(
    provider: Arc<Provider<P>>,
    wallet: LocalWallet,
    pool_address: Address,
    commitment: &[u8; 32],
    chain_id: u64,
) -> Result<H256> {
    let client = SignerMiddleware::new(provider.clone(), wallet);

    let (_base_fee, priority_fee, max_fee_per_gas) = get_eip1559_fees(&provider).await?;

    let tx = build_deposit_tx(
        pool_address,
        commitment,
        chain_id,
        max_fee_per_gas,
        priority_fee,
    );

    info!(
        "Broadcasting PrivacyPool.deposit() tx to {:?} with commitment 0x{}",
        pool_address,
        hex::encode(commitment)
    );

    let pending_tx = client
        .send_transaction(tx, None)
        .await
        .context("Failed to send PrivacyPool.deposit() transaction")?;

    let tx_hash = pending_tx.tx_hash();
    info!("✅ PrivacyPool.deposit() broadcasted: {:#x}", tx_hash);

    Ok(tx_hash)
}

/// Executes the full withdrawal flow: signs and broadcasts a `withdraw(secret, nullifier, recipient)`
/// transaction to the PrivacyPool contract via the relayer wallet.
///
/// Returns the transaction hash on success.
pub async fn execute_pool_withdraw<P: JsonRpcClient + 'static>(
    provider: Arc<Provider<P>>,
    relayer_wallet: LocalWallet,
    pool_address: Address,
    secret: &[u8; 32],
    nullifier: &[u8; 32],
    recipient: Address,
    chain_id: u64,
) -> Result<H256> {
    let client = SignerMiddleware::new(provider.clone(), relayer_wallet);

    let (_base_fee, priority_fee, max_fee_per_gas) = get_eip1559_fees(&provider).await?;

    let tx = build_withdraw_tx(
        pool_address,
        secret,
        nullifier,
        recipient,
        chain_id,
        max_fee_per_gas,
        priority_fee,
    );

    info!(
        "Broadcasting PrivacyPool.withdraw() tx — recipient {:?}",
        recipient,
    );

    let pending_tx = client
        .send_transaction(tx, None)
        .await
        .context("Failed to send PrivacyPool.withdraw() transaction")?;

    let tx_hash = pending_tx.tx_hash();
    info!("✅ PrivacyPool.withdraw() broadcasted: {:#x}", tx_hash);

    Ok(tx_hash)
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_commitment_deterministic() {
        let secret = [0xABu8; 32];
        let nullifier = [0xCDu8; 32];
        let c1 = compute_commitment(&secret, &nullifier);
        let c2 = compute_commitment(&secret, &nullifier);
        assert_eq!(c1, c2, "Commitment must be deterministic");
    }

    #[test]
    fn test_compute_commitment_different_inputs() {
        let secret_a = [0x01u8; 32];
        let nullifier_a = [0x02u8; 32];
        let secret_b = [0x03u8; 32];
        let nullifier_b = [0x04u8; 32];

        let c_a = compute_commitment(&secret_a, &nullifier_a);
        let c_b = compute_commitment(&secret_b, &nullifier_b);
        assert_ne!(
            c_a, c_b,
            "Different inputs must produce different commitments"
        );
    }

    #[test]
    fn test_commitment_matches_solidity_sha256_packed() {
        // Verify the Rust computation matches what Solidity's
        // sha256(abi.encodePacked(secret, nullifier)) would produce.
        //
        // abi.encodePacked for two bytes32 values is just concatenation (64 bytes).
        // SHA-256 of 64 zero bytes:
        let secret = [0u8; 32];
        let nullifier = [0u8; 32];
        let commitment = compute_commitment(&secret, &nullifier);

        // Manually compute SHA-256(64 zero bytes)
        let mut hasher = Sha256::new();
        hasher.update([0u8; 64]);
        let expected: [u8; 32] = hasher.finalize().into();

        assert_eq!(
            commitment, expected,
            "Must match SHA-256 of concatenated bytes"
        );
    }

    #[test]
    fn test_encode_deposit_calldata_length() {
        let commitment = [0xAAu8; 32];
        let calldata = encode_deposit_calldata(&commitment);
        // 4 bytes selector + 32 bytes commitment (padded to 32 by ABI encoding)
        assert_eq!(calldata.len(), 4 + 32, "deposit calldata must be 36 bytes");
    }

    #[test]
    fn test_encode_withdraw_calldata_length() {
        let secret = [0xAAu8; 32];
        let nullifier = [0xBBu8; 32];
        let recipient = Address::zero();
        let calldata = encode_withdraw_calldata(&secret, &nullifier, recipient);
        // 4 bytes selector + 32 + 32 + 32 (address padded to 32 bytes)
        assert_eq!(
            calldata.len(),
            4 + 32 + 32 + 32,
            "withdraw calldata must be 100 bytes"
        );
    }

    #[test]
    fn test_generate_note_randomness() {
        let note1 = generate_note();
        let note2 = generate_note();
        // While theoretically possible for two random 32-byte values to collide,
        // it's astronomically unlikely
        assert_ne!(
            note1.secret, note2.secret,
            "Notes should have unique secrets"
        );
        assert_ne!(
            note1.nullifier, note2.nullifier,
            "Notes should have unique nullifiers"
        );
    }

    #[test]
    fn test_note_zeroize_on_drop() {
        let note = generate_note();
        let secret_copy = note.secret;
        assert_ne!(
            secret_copy, [0u8; 32],
            "Generated secret should not be all zeros"
        );
        // When note is dropped, its memory is zeroized.
        // We can't easily test this without unsafe, but the ZeroizeOnDrop derive
        // guarantees it via the zeroize crate.
        drop(note);
    }

    #[test]
    fn test_build_deposit_tx_value() {
        let pool = Address::random();
        let commitment = [0xFFu8; 32];
        let tx = build_deposit_tx(pool, &commitment, 84532, U256::from(1000), U256::from(100));
        assert_eq!(tx.value, Some(U256::from(DENOMINATION_WEI)));
        assert_eq!(tx.gas, Some(U256::from(DEPOSIT_GAS_LIMIT)));
    }

    #[test]
    fn test_build_withdraw_tx_zero_value() {
        let pool = Address::random();
        let secret = [0xAAu8; 32];
        let nullifier = [0xBBu8; 32];
        let recipient = Address::random();
        let tx = build_withdraw_tx(
            pool,
            &secret,
            &nullifier,
            recipient,
            84532,
            U256::from(1000),
            U256::from(100),
        );
        assert_eq!(
            tx.value,
            Some(U256::zero()),
            "Withdraw tx should not send ETH"
        );
        assert_eq!(tx.gas, Some(U256::from(WITHDRAW_GAS_LIMIT)));
    }
}
