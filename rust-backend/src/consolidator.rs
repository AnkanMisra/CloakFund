use anyhow::{Context, Result};
use ethers::prelude::*;
use ethers::types::Eip1559TransactionRequest;
use std::sync::Arc;
use tracing::{debug, info, warn};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A securely zeroized wrapper around an ephemeral private key.
/// Ensures that the sensitive key material is wiped from memory when dropped.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct EphemeralPrivateKey(pub [u8; 32]);

pub enum SweepOutcome {
    Success(H256),
    SkippedZeroBalance,
    SkippedDust { balance: U256, max_gas_cost: U256 },
    SkippedDryRun,
}

/// Handles the consolidation/sweeping of funds from ephemeral stealth addresses
/// to the centralized treasury (e.g., BitGo vault).
///
/// Generic over the provider type P so it works with both `Provider<Http>`
/// (used by the sweeper) and `Provider<Ws>` (used by the watcher).
pub struct Consolidator<P: JsonRpcClient> {
    provider: Arc<Provider<P>>,
    dry_run: bool,
    chain_id: u64,
}

impl<P: JsonRpcClient + 'static> Consolidator<P> {
    /// Creates a new Consolidator instance.
    pub fn new(provider: Arc<Provider<P>>, dry_run: bool, chain_id: u64) -> Self {
        Self {
            provider,
            dry_run,
            chain_id,
        }
    }

    /// Sweeps the entire native balance (ETH) from the ephemeral stealth address
    /// to the treasury using proper EIP-1559 fee calculation.
    ///
    /// This implementation:
    /// 1. Reads `base_fee_per_gas` from the latest block
    /// 2. Queries the network's suggested priority fee
    /// 3. Computes `max_fee_per_gas = (base_fee × 2) + priority_fee`
    /// 4. Calculates `max_gas_cost = gas_limit × max_fee_per_gas`
    /// 5. Applies a 5% safety buffer
    /// 6. Builds an explicit EIP-1559 transaction (no legacy conversion)
    /// 7. Re-checks balance before broadcast to prevent race conditions
    pub async fn sweep_native(
        &self,
        ephemeral_key: EphemeralPrivateKey,
        from_address: Address,
        destination_address: Address,
    ) -> Result<SweepOutcome> {
        let wallet = LocalWallet::from_bytes(&ephemeral_key.0)
            .context("Failed to construct wallet from ephemeral private key")?
            .with_chain_id(self.chain_id);

        // Ensure the derived address matches the expected ephemeral address
        if wallet.address() != from_address {
            anyhow::bail!(
                "Derived wallet address {:?} does not match the expected from_address {:?}",
                wallet.address(),
                from_address
            );
        }

        let client = SignerMiddleware::new(self.provider.clone(), wallet);

        // ── Step 1: Fetch current balance ──────────────────────────────────
        let balance = client
            .get_balance(from_address, None)
            .await
            .context("Failed to get balance for ephemeral address")?;

        info!(
            "Stealth address {:?} balance: {} wei",
            from_address, balance
        );

        if balance.is_zero() {
            warn!(
                "Attempted to sweep an address with zero balance: {:?}",
                from_address
            );
            return Ok(SweepOutcome::SkippedZeroBalance);
        }

        // ── Step 2: Get EIP-1559 fee parameters ────────────────────────────
        // Fetch base_fee_per_gas from the latest block
        let latest_block = self
            .provider
            .get_block(BlockNumber::Latest)
            .await
            .context("Failed to fetch latest block")?
            .context("Latest block not found")?;

        let base_fee = latest_block.base_fee_per_gas.unwrap_or_else(|| {
            warn!("Block has no base_fee_per_gas, falling back to 1 gwei");
            U256::from(1_000_000_000) // 1 gwei fallback
        });

        // Fetch the network's suggested priority fee
        let priority_fee: U256 = self
            .provider
            .request("eth_maxPriorityFeePerGas", ())
            .await
            .unwrap_or_else(|e| {
                warn!("Failed to get priority fee, using 1 gwei fallback: {:?}", e);
                U256::from(1_000_000_000) // 1 gwei
            });

        // ── Step 3: Compute EIP-1559 fee cap ───────────────────────────────
        // max_fee = (base_fee × 2) + priority_fee
        // Doubling base_fee ensures the tx stays valid even if base fee spikes
        let max_fee_per_gas = base_fee
            .saturating_mul(U256::from(2))
            .saturating_add(priority_fee);

        let max_priority_fee_per_gas = priority_fee;

        debug!(
            "EIP-1559 fees: base_fee={}, priority_fee={}, max_fee_per_gas={}",
            base_fee, priority_fee, max_fee_per_gas
        );

        // ── Step 4: Compute gas limit and max gas cost ─────────────────────
        // Standard ETH transfer is always exactly 21000 gas
        let gas_limit = U256::from(21_000);
        let max_gas_cost = gas_limit.saturating_mul(max_fee_per_gas);

        // ── Step 5: Apply 5% safety buffer ─────────────────────────────────
        let buffer = max_gas_cost / 20; // 5%
        let total_gas_reservation = max_gas_cost.saturating_add(buffer);

        debug!(
            "Gas calculation: gas_limit={}, max_gas_cost={}, buffer={}, total_reserved={}",
            gas_limit, max_gas_cost, buffer, total_gas_reservation
        );

        // ── Step 6: Dust detection ─────────────────────────────────────────
        if balance <= total_gas_reservation {
            warn!(
                "Dust detected: balance ({}) cannot cover gas ({}) for {:?}",
                balance, total_gas_reservation, from_address
            );
            return Ok(SweepOutcome::SkippedDust {
                balance,
                max_gas_cost: total_gas_reservation,
            });
        }

        // ── Step 7: Compute sweep amount ───────────────────────────────────
        let sweep_amount = balance.saturating_sub(total_gas_reservation);

        info!(
            "Sweep plan: {:?} → {:?} | amount={} wei | gas_reserved={} wei",
            from_address, destination_address, sweep_amount, total_gas_reservation
        );

        // ── Step 8: Build EIP-1559 transaction ─────────────────────────────
        let tx = Eip1559TransactionRequest::new()
            .to(destination_address)
            .value(sweep_amount)
            .gas(gas_limit)
            .max_fee_per_gas(max_fee_per_gas)
            .max_priority_fee_per_gas(max_priority_fee_per_gas)
            .chain_id(self.chain_id);

        debug!(
            "Built EIP-1559 tx: to={:?}, value={}, gas={}, max_fee={}, max_priority_fee={}",
            destination_address, sweep_amount, gas_limit, max_fee_per_gas, max_priority_fee_per_gas
        );

        if self.dry_run {
            info!(
                "DRY RUN: Skipping broadcast for {:?} (would sweep {} wei to {:?})",
                from_address, sweep_amount, destination_address
            );
            return Ok(SweepOutcome::SkippedDryRun);
        }

        // ── Step 9: Pre-broadcast balance recheck ──────────────────────────
        let recheck_balance = client
            .get_balance(from_address, None)
            .await
            .context("Failed to recheck balance before broadcast")?;

        if recheck_balance < sweep_amount.saturating_add(total_gas_reservation) {
            warn!(
                "Balance changed between estimation and broadcast: was {}, now {}. Aborting.",
                balance, recheck_balance
            );
            return Ok(SweepOutcome::SkippedDust {
                balance: recheck_balance,
                max_gas_cost: total_gas_reservation,
            });
        }

        // ── Step 10: Broadcast ─────────────────────────────────────────────
        let pending_tx = client
            .send_transaction(tx, None)
            .await
            .context("Failed to send EIP-1559 sweep transaction")?;

        let tx_hash = pending_tx.tx_hash();
        info!(
            "✅ Broadcasted EIP-1559 sweep tx {:#x} from {:?} ({} wei → {:?})",
            tx_hash, from_address, sweep_amount, destination_address
        );

        Ok(SweepOutcome::Success(tx_hash))
    }

    /// Placeholder for ERC20 sweeping logic.
    pub async fn sweep_erc20(
        &self,
        _ephemeral_key: EphemeralPrivateKey,
        _from_address: Address,
        _token_address: Address,
        _destination_address: Address,
    ) -> Result<SweepOutcome> {
        warn!("ERC20 sweeping not yet implemented.");
        Ok(SweepOutcome::SkippedZeroBalance) // Placeholder
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
