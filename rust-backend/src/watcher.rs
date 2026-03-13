use anyhow::{Context, Result};
use ethers::prelude::*;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{Duration, sleep};
use tracing::{debug, error, info, warn};

use crate::config::WatcherConfig;
use crate::convex_client::ConvexRepository;
use crate::models::{AssetType, ConfirmationStatus, NewDeposit};

/// The WatcherService is responsible for listening to the blockchain
/// for incoming deposits to ephemeral addresses, and updating the state
/// in Convex.
pub struct WatcherService {
    config: WatcherConfig,
    convex: Arc<ConvexRepository>,
}

impl WatcherService {
    /// Create a new WatcherService instance
    pub fn new(config: WatcherConfig, convex: Arc<ConvexRepository>) -> Self {
        Self { config, convex }
    }

    /// Start the watcher loop to listen for blockchain events
    pub async fn start(&self) -> Result<()> {
        if let Err(e) = self.start_inner().await {
            error!("Watcher service failed permanently: {}", e);
            std::process::exit(1);
        }
        Ok(())
    }

    async fn start_inner(&self) -> Result<()> {
        info!(
            "Starting WatcherService on chain_id: {} (network: {})",
            self.config.chain_id, self.config.network
        );

        // Connect to the WebSocket provider
        let provider = Provider::<Ws>::connect(&self.config.base_wss_url)
            .await
            .context("Failed to connect to Base WSS endpoint")?;
        let provider = Arc::new(provider);

        info!("Successfully connected to Base WSS provider");

        let current_block = provider.get_block_number().await?.as_u64();
        let start_block = self.config.start_block.unwrap_or(current_block);

        if start_block < current_block {
            info!(
                "Catching up on historical blocks from {} to {}",
                start_block, current_block
            );
            for block_num in start_block..=current_block {
                if let Err(e) = self.process_block(&provider, block_num).await {
                    error!("Error processing historical block {}: {:?}", block_num, e);
                }

                // Add rate limiting for historical catchup
                sleep(Duration::from_millis(50)).await;
            }
            if let Err(e) = self.update_confirmations(&provider, current_block).await {
                error!("Error updating confirmations after catch-up: {:?}", e);
            }
            info!("Historical catch-up complete.");
        }

        info!("Subscribing to new blocks...");
        let mut stream = provider.subscribe_blocks().await?;
        while let Some(block) = stream.next().await {
            if let Some(number) = block.number {
                let block_number = number.as_u64();
                debug!("New block received: {}", block_number);

                if let Err(e) = self.process_block(&provider, block_number).await {
                    error!("Error processing block {}: {:?}", block_number, e);
                }

                if let Err(e) = self.update_confirmations(&provider, block_number).await {
                    error!(
                        "Error updating confirmations at block {}: {:?}",
                        block_number, e
                    );
                }

                let latest_confirmed =
                    block_number.saturating_sub(self.config.required_confirmations);
                if let Err(e) = self
                    .convex
                    .update_checkpoint(block_number, latest_confirmed)
                    .await
                {
                    error!("Failed to update checkpoint: {:?}", e);
                }
            }
        }

        Ok(())
    }

    /// Process a specific block to find deposits
    async fn process_block(&self, provider: &Provider<Ws>, block_number: u64) -> Result<()> {
        debug!("Processing block: {}", block_number);

        let block = provider
            .get_block_with_txs(block_number)
            .await?
            .context("Block not found")?;

        let block_hash_opt = block.hash.map(|h| format!("{:#x}", h));

        for tx in block.transactions {
            if let Some(to) = tx.to {
                let to_hex = format!("{:#x}", to);

                // Query convex to see if it's a known ephemeral address
                if let Ok(Some(match_res)) = self
                    .convex
                    .get_ephemeral_address_match(self.config.chain_id, &to_hex)
                    .await
                {
                    let from_hex = format!("{:#x}", tx.from);
                    let tx_hash_hex = format!("{:#x}", tx.hash);
                    let value = tx.value.to_string(); // In wei

                    if tx.value > U256::zero() {
                        let deposit = NewDeposit {
                            paylink_id: match_res.paylink_id,
                            ephemeral_address_id: match_res.ephemeral_address_id,
                            tx_hash: tx_hash_hex,
                            log_index: None,
                            block_number,
                            block_hash: block_hash_opt.clone(),
                            from_address: from_hex,
                            to_address: to_hex,
                            asset_type: AssetType::Native,
                            token_address: None,
                            amount: value,
                            decimals: Some(18),
                            symbol: Some("ETH".to_string()),
                            confirmations: 1,
                            confirmation_status: ConfirmationStatus::from_confirmations(
                                1,
                                self.config.required_confirmations,
                            ),
                            detected_at: Some(
                                SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis() as u64,
                            ),
                            confirmed_at: None,
                        };

                        info!(
                            "Detected native deposit of {} wei to {}",
                            deposit.amount, deposit.to_address
                        );

                        if let Err(e) = self.convex.upsert_deposit(&deposit).await {
                            error!("Failed to upsert deposit to Convex: {:?}", e);
                        }
                    }
                }
            }
        }

        // Check for ERC20 Transfers
        let transfer_topic =
            H256::from_str("0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")
                .unwrap();

        let block_hash = match block.hash {
            Some(h) => h,
            None => return Ok(()),
        };

        let logs_filter = Filter::new().at_block_hash(block_hash);
        if let Ok(logs) = provider.get_logs(&logs_filter).await {
            for log in logs {
                if log.topics.len() == 3 && log.topics[0] == transfer_topic {
                    let from = Address::from(log.topics[1]);
                    let to = Address::from(log.topics[2]);
                    let to_hex = format!("{:#x}", to);

                    if let Ok(Some(match_res)) = self
                        .convex
                        .get_ephemeral_address_match(self.config.chain_id, &to_hex)
                        .await
                    {
                        if log.data.len() < 32 {
                            continue;
                        }
                        let value = U256::from_big_endian(&log.data[0..32]);

                        if value > U256::zero() {
                            let tx_hash_hex = match log.transaction_hash {
                                Some(h) => format!("{:#x}", h),
                                None => {
                                    warn!("Skipping ERC20 Transfer log with no transaction hash");
                                    continue;
                                }
                            };
                            let token_address = format!("{:#x}", log.address);

                            let deposit = NewDeposit {
                                paylink_id: match_res.paylink_id,
                                ephemeral_address_id: match_res.ephemeral_address_id,
                                tx_hash: tx_hash_hex,
                                log_index: log.log_index.map(|i| i.as_u64()),
                                block_number,
                                block_hash: block_hash_opt.clone(),
                                from_address: format!("{:#x}", from),
                                to_address: to_hex,
                                asset_type: AssetType::Erc20,
                                token_address: Some(token_address.clone()),
                                amount: value.to_string(),
                                decimals: None,
                                symbol: None,
                                confirmations: 1,
                                confirmation_status: ConfirmationStatus::from_confirmations(
                                    1,
                                    self.config.required_confirmations,
                                ),
                                detected_at: Some(
                                    SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap()
                                        .as_millis() as u64,
                                ),
                                confirmed_at: None,
                            };

                            info!(
                                "Detected ERC20 deposit of {} (token: {}) to {}",
                                deposit.amount, token_address, deposit.to_address
                            );

                            if let Err(e) = self.convex.upsert_deposit(&deposit).await {
                                error!("Failed to upsert deposit to Convex: {:?}", e);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Update confirmations for pending deposits
    pub async fn update_confirmations(
        &self,
        provider: &Provider<Ws>,
        current_block: u64,
    ) -> Result<()> {
        debug!("Updating confirmations up to block: {}", current_block);

        let pending = self.convex.get_pending_confirmation_updates().await?;
        for deposit in pending {
            let confs = current_block.saturating_sub(deposit.block_number) + 1;

            // Check if tx is still in the same block (handle reorg)
            match provider
                .get_transaction(
                    ethers::types::H256::from_str(&deposit.tx_hash).unwrap_or_default(),
                )
                .await
            {
                Ok(Some(tx)) => {
                    if let Some(tx_block) = tx.block_number {
                        if tx_block.as_u64() != deposit.block_number {
                            warn!(
                                "Transaction {} reorged to block {}",
                                deposit.tx_hash, tx_block
                            );
                            let _ = self.convex.mark_deposit_reorged(&deposit.id).await;
                            continue;
                        }
                    } else {
                        warn!("Transaction {} reorged out of chain", deposit.tx_hash);
                        let _ = self.convex.mark_deposit_reorged(&deposit.id).await;
                        continue;
                    }
                }
                Ok(None) => {
                    warn!(
                        "Transaction {} not found, assuming reorged out of chain",
                        deposit.tx_hash
                    );
                    let _ = self.convex.mark_deposit_reorged(&deposit.id).await;
                    continue;
                }
                Err(e) => {
                    error!(
                        "Failed to fetch transaction {} for confirmation check: {:?}",
                        deposit.tx_hash, e
                    );
                    continue;
                }
            }

            if confs <= deposit.confirmations {
                continue;
            }

            if let Err(e) = self
                .convex
                .update_confirmations(&deposit.id, confs, self.config.required_confirmations)
                .await
            {
                error!(
                    "Failed to update confirmations for deposit {}: {:?}",
                    deposit.id, e
                );
            }
        }

        Ok(())
    }
}
