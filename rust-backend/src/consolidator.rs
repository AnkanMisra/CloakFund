use anyhow::{Context, Result};
use ethers::prelude::*;
use std::sync::Arc;
use tracing::{debug, info, warn};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A securely zeroized wrapper around an ephemeral private key.
/// Ensures that the sensitive key material is wiped from memory when dropped.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct EphemeralPrivateKey(pub [u8; 32]);

/// Handles the consolidation/sweeping of funds from ephemeral stealth addresses
/// to the centralized treasury (e.g., BitGo vault).
pub struct Consolidator {
    provider: Arc<Provider<Ws>>,
    treasury_address: Address,
    dry_run: bool,
}

impl Consolidator {
    /// Creates a new Consolidator instance.
    pub fn new(provider: Arc<Provider<Ws>>, treasury_address: Address, dry_run: bool) -> Self {
        Self {
            provider,
            treasury_address,
            dry_run,
        }
    }

    /// Prepares and broadcasts a transaction to sweep the entire native balance
    /// (e.g., ETH) from the ephemeral address to the treasury.
    pub async fn sweep_native(
        &self,
        ephemeral_key: EphemeralPrivateKey,
        from_address: Address,
    ) -> Result<H256> {
        let wallet = LocalWallet::from_bytes(&ephemeral_key.0)
            .context("Failed to construct wallet from ephemeral private key")?;

        // Ensure the derived address matches the expected ephemeral address to prevent mistakes
        if wallet.address() != from_address {
            anyhow::bail!("Derived wallet address does not match the expected from_address");
        }

        let client = SignerMiddleware::new(self.provider.clone(), wallet);

        // Fetch current balance
        let balance = client
            .get_balance(from_address, None)
            .await
            .context("Failed to get balance for ephemeral address")?;

        if balance.is_zero() {
            warn!(
                "Attempted to sweep an address with zero balance: {:?}",
                from_address
            );
            return Ok(H256::zero());
        }

        // Estimate gas for a simple ETH transfer
        let gas_price = client
            .get_gas_price()
            .await
            .context("Failed to get gas price")?;

        // Standard ETH transfer gas limit is 21000
        let gas_limit = U256::from(21000);
        let gas_cost = gas_price.saturating_mul(gas_limit);

        if balance <= gas_cost {
            warn!(
                "Balance ({}) is too low to cover gas cost ({}) for address {:?}",
                balance, gas_cost, from_address
            );
            return Ok(H256::zero());
        }

        let sweep_amount = balance.saturating_sub(gas_cost);

        let tx = TransactionRequest::new()
            .to(self.treasury_address)
            .value(sweep_amount)
            .gas(gas_limit)
            .gas_price(gas_price);

        debug!(
            "Prepared sweep tx: from={:?} to={:?} amount={} gas_cost={}",
            from_address, self.treasury_address, sweep_amount, gas_cost
        );

        if self.dry_run {
            info!(
                "DRY RUN: Skipping transaction broadcast for {:?}",
                from_address
            );
            return Ok(H256::zero());
        }

        let pending_tx = client
            .send_transaction(tx, None)
            .await
            .context("Failed to send sweep transaction")?;

        let tx_hash = pending_tx.tx_hash();
        info!("Broadcasted sweep tx {} from {:?}", tx_hash, from_address);

        Ok(tx_hash)
    }

    /// Placeholder for ERC20 sweeping logic.
    pub async fn sweep_erc20(
        &self,
        _ephemeral_key: EphemeralPrivateKey,
        _from_address: Address,
        _token_address: Address,
    ) -> Result<H256> {
        warn!("ERC20 sweeping not yet implemented.");
        Ok(H256::zero())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroize::Zeroize;

    #[test]
    fn test_ephemeral_private_key_zeroize() {
        let mut key = EphemeralPrivateKey([1u8; 32]);
        assert_eq!(key.0, [1u8; 32]);
        key.zeroize();
        assert_eq!(key.0, [0u8; 32]);
    }
}
