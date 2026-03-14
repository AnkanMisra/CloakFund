use anyhow::{Context, Result};
use ethers::prelude::*;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};

use tracing::{debug, error, info, trace, warn};

/// How often to refresh the stealth address cache during historical catch-up
const CACHE_REFRESH_INTERVAL: u64 = 50;

/// Delay between RPC calls to avoid rate-limiting by free public nodes.
/// 200ms allows ~5 requests/second, well within most public node limits.
const RPC_THROTTLE: Duration = Duration::from_millis(200);

/// Delay before reconnecting after a WebSocket disconnection
const RECONNECT_DELAY: Duration = Duration::from_secs(5);

/// Maximum number of consecutive reconnection attempts
const MAX_RECONNECT_ATTEMPTS: u32 = 10;

use crate::config::WatcherConfig;
use crate::convex_client::ConvexRepository;
use crate::models::{AssetType, ConfirmationStatus, DepositMatch, NewDeposit};

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

    /// Start the watcher loop to listen for blockchain events.
    /// Automatically reconnects on WebSocket disconnections.
    pub async fn start(&self) -> Result<()> {
        let mut reconnect_attempts: u32 = 0;

        loop {
            match self.start_inner().await {
                Ok(()) => {
                    warn!("Watcher stream ended unexpectedly, reconnecting...");
                }
                Err(e) => {
                    error!("Watcher service error: {}", e);
                    reconnect_attempts += 1;
                    if reconnect_attempts > MAX_RECONNECT_ATTEMPTS {
                        error!(
                            "Exceeded {} reconnection attempts, shutting down",
                            MAX_RECONNECT_ATTEMPTS
                        );
                        std::process::exit(1);
                    }
                }
            }

            warn!(
                "Reconnecting in {}s (attempt {}/{})...",
                RECONNECT_DELAY.as_secs(),
                reconnect_attempts,
                MAX_RECONNECT_ATTEMPTS
            );
            sleep(RECONNECT_DELAY).await;
        }
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
        let mut start_block = self.config.start_block.unwrap_or(current_block);

        // Smart skip: if the checkpoint is too far behind, skip to near the
        // current head. Processing 14K+ old blocks takes hours and is useless
        // for testing/demos. We only need to watch for recent transactions.
        const MAX_CATCHUP_BLOCKS: u64 = 50;
        let gap = current_block.saturating_sub(start_block);
        if gap > MAX_CATCHUP_BLOCKS {
            let skip_to = current_block.saturating_sub(5);
            warn!(
                "Checkpoint is {} blocks behind (block {} vs head {}). \
                 Skipping to block {} to avoid multi-hour catch-up.",
                gap, start_block, current_block, skip_to
            );
            start_block = skip_to;
        }

        // Track the last block we've fully processed to avoid gaps and duplicates
        let mut last_processed_block = if start_block < current_block {
            info!(
                "Catching up on recent blocks from {} to {} ({} blocks)",
                start_block,
                current_block,
                current_block - start_block
            );

            // During catch-up, cache stealth addresses and refresh periodically
            let mut cached_watched: Option<HashMap<String, DepositMatch>> = None;
            let mut cache_block: u64 = 0;

            for block_num in start_block..=current_block {
                // Refresh the cache every CACHE_REFRESH_INTERVAL blocks
                if cached_watched.is_none()
                    || block_num.saturating_sub(cache_block) >= CACHE_REFRESH_INTERVAL
                {
                    match self.fetch_watched_addresses().await {
                        Ok(map) => {
                            debug!(
                                "Refreshed stealth address cache at block {} ({} addresses)",
                                block_num,
                                map.len()
                            );
                            cached_watched = Some(map);
                            cache_block = block_num;
                        }
                        Err(e) => {
                            error!(
                                "Failed to refresh stealth address cache at block {}: {:?}",
                                block_num, e
                            );
                        }
                    }
                }

                if let Some(ref watched) = cached_watched
                    && let Err(e) = self
                        .process_block_with_cache(&provider, block_num, watched)
                        .await
                {
                    error!("Error processing historical block {}: {:?}", block_num, e);
                }

                // Throttle to avoid rate-limiting by public RPC nodes
                sleep(RPC_THROTTLE).await;
            }
            if let Err(e) = self.update_confirmations(&provider, current_block).await {
                error!("Error updating confirmations after catch-up: {:?}", e);
            }
            info!("Historical catch-up complete at block {}", current_block);
            current_block
        } else {
            start_block
        };

        // Subscribe FIRST, then close the gap between catch-up and subscription.
        // This ensures zero blocks are ever missed.
        info!("Subscribing to new blocks...");
        let mut stream = provider.subscribe_blocks().await?;

        // Process any blocks produced DURING catch-up
        let new_head = provider.get_block_number().await?.as_u64();
        if new_head > last_processed_block {
            info!(
                "Filling gap: processing blocks {} to {} ({} blocks produced during catch-up)",
                last_processed_block + 1,
                new_head,
                new_head - last_processed_block
            );
            let gap_watched = self.fetch_watched_addresses().await.unwrap_or_default();
            for block_num in (last_processed_block + 1)..=new_head {
                if let Err(e) = self
                    .process_block_with_cache(&provider, block_num, &gap_watched)
                    .await
                {
                    error!("Error processing gap block {}: {:?}", block_num, e);
                }
                sleep(RPC_THROTTLE).await;
            }
            last_processed_block = new_head;
            info!("Gap fill complete. Now listening for live blocks.");
        }

        while let Some(block) = stream.next().await {
            if let Some(number) = block.number {
                let block_number = number.as_u64();

                // Skip blocks already processed during catch-up / gap fill
                if block_number <= last_processed_block {
                    trace!(
                        "Skipping already-processed block {} (last_processed={})",
                        block_number, last_processed_block
                    );
                    continue;
                }

                // If the subscription jumped ahead, fill sequential gaps
                if block_number > last_processed_block + 1 {
                    info!(
                        "Detected gap in subscription: blocks {} to {}",
                        last_processed_block + 1,
                        block_number - 1
                    );
                    let sub_gap_watched = self.fetch_watched_addresses().await.unwrap_or_default();
                    for gap_block in (last_processed_block + 1)..block_number {
                        if let Err(e) = self
                            .process_block_with_cache(&provider, gap_block, &sub_gap_watched)
                            .await
                        {
                            error!("Error processing gap block {}: {:?}", gap_block, e);
                        }
                        sleep(RPC_THROTTLE).await;
                    }
                }

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

                last_processed_block = block_number;

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

    /// Fetch all active stealth addresses from Convex and build a lowercase HashMap
    /// for O(1) lookups. Called once per block to avoid per-transaction rate limiting.
    async fn fetch_watched_addresses(&self) -> Result<HashMap<String, DepositMatch>> {
        let active = self
            .convex
            .get_active_stealth_addresses(self.config.chain_id)
            .await
            .context("Failed to fetch active stealth addresses from Convex")?;

        let mut map = HashMap::with_capacity(active.len());
        for dm in active {
            let key = dm.stealth_address.to_lowercase();
            map.insert(key, dm);
        }

        debug!(
            "Loaded {} active stealth addresses for chain {}",
            map.len(),
            self.config.chain_id
        );

        if map.is_empty() {
            debug!("No active stealth addresses to watch — block will be skipped");
        } else {
            for addr in map.keys() {
                trace!("  Watching: {}", addr);
            }
        }

        Ok(map)
    }

    /// Check if a transaction was successful by verifying its receipt status.
    /// Returns `true` if the receipt confirms status == 1 (success).
    async fn is_tx_successful(&self, provider: &Provider<Ws>, tx_hash: H256) -> bool {
        match provider.get_transaction_receipt(tx_hash).await {
            Ok(Some(receipt)) => {
                if let Some(status) = receipt.status {
                    if status == U64::from(1) {
                        true
                    } else {
                        warn!(
                            "Transaction {:#x} reverted (status={}), skipping deposit",
                            tx_hash, status
                        );
                        false
                    }
                } else {
                    // Pre-Byzantium transactions don't have status; treat as successful
                    warn!(
                        "Transaction {:#x} has no receipt status field, assuming successful",
                        tx_hash
                    );
                    true
                }
            }
            Ok(None) => {
                warn!(
                    "No receipt found for transaction {:#x}, skipping deposit",
                    tx_hash
                );
                false
            }
            Err(e) => {
                error!(
                    "Failed to fetch receipt for transaction {:#x}: {:?}",
                    tx_hash, e
                );
                false
            }
        }
    }

    /// Process a specific block to find deposits (live mode — fetches addresses from Convex)
    async fn process_block(&self, provider: &Provider<Ws>, block_number: u64) -> Result<()> {
        let watched = match self.fetch_watched_addresses().await {
            Ok(map) => map,
            Err(e) => {
                error!(
                    "Failed to fetch watched addresses for block {}: {:?}. Skipping block.",
                    block_number, e
                );
                return Ok(());
            }
        };
        self.process_block_with_cache(provider, block_number, &watched)
            .await
    }

    /// Process a specific block using a pre-built address map (avoids redundant Convex queries)
    async fn process_block_with_cache(
        &self,
        provider: &Provider<Ws>,
        block_number: u64,
        watched: &HashMap<String, DepositMatch>,
    ) -> Result<()> {
        debug!("Processing block: {}", block_number);

        // If there is nothing to watch, skip the block entirely
        if watched.is_empty() {
            return Ok(());
        }

        // Step 2: Fetch the block with full transactions
        let block = provider
            .get_block_with_txs(block_number)
            .await?
            .context("Block not found")?;

        let block_hash_opt = block.hash.map(|h| format!("{:#x}", h));
        let tx_count = block.transactions.len();
        debug!("Block {} contains {} transactions", block_number, tx_count);

        // Step 3: Scan native ETH transfers
        let mut native_matches: u64 = 0;
        for tx in &block.transactions {
            if let Some(to) = tx.to {
                // Explicit lowercase normalization for address comparison
                let to_hex = format!("{:#x}", to).to_lowercase();

                trace!("  tx {:#x}: to={} value={} wei", tx.hash, to_hex, tx.value);

                if let Some(match_res) = watched.get(&to_hex) {
                    info!(
                        "MATCH FOUND in block {}: tx {:#x} sends {} wei to stealth address {}",
                        block_number, tx.hash, tx.value, to_hex
                    );

                    if tx.value == U256::zero() {
                        debug!("Skipping zero-value transaction {:#x}", tx.hash);
                        continue;
                    }

                    // Verify the transaction was actually successful on-chain
                    if !self.is_tx_successful(provider, tx.hash).await {
                        continue;
                    }

                    let from_hex = format!("{:#x}", tx.from).to_lowercase();
                    let tx_hash_hex = format!("{:#x}", tx.hash);
                    let value = tx.value.to_string(); // In wei

                    let deposit = NewDeposit {
                        paylink_id: match_res.paylink_id.clone(),
                        ephemeral_address_id: match_res.ephemeral_address_id.clone(),
                        tx_hash: tx_hash_hex.clone(),
                        log_index: None,
                        block_number,
                        block_hash: block_hash_opt.clone(),
                        from_address: from_hex,
                        to_address: to_hex.clone(),
                        asset_type: AssetType::Native,
                        token_address: None,
                        amount: value.clone(),
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
                        "Recording native deposit: {} wei to {} (tx: {})",
                        deposit.amount, deposit.to_address, deposit.tx_hash
                    );

                    match self.convex.upsert_deposit(&deposit).await {
                        Ok(result) => {
                            info!(
                                "Successfully recorded deposit {} for paylink {}",
                                result.deposit_id, result.paylink_id
                            );
                            native_matches += 1;
                        }
                        Err(e) => {
                            error!(
                                "Failed to upsert deposit to Convex for tx {}: {:?}",
                                tx_hash_hex, e
                            );
                        }
                    }
                }
            }
        }

        // Step 4: Check for ERC20 Transfers
        let transfer_topic =
            H256::from_str("0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")
                .unwrap();

        let block_hash = match block.hash {
            Some(h) => h,
            None => {
                if native_matches > 0 {
                    info!(
                        "Block {} processed: {} native deposit(s) found (no ERC20 scan — missing block hash)",
                        block_number, native_matches
                    );
                }
                return Ok(());
            }
        };

        let mut erc20_matches: u64 = 0;
        let logs_filter = Filter::new()
            .at_block_hash(block_hash)
            .topic0(transfer_topic);
        match provider.get_logs(&logs_filter).await {
            Ok(logs) => {
                debug!(
                    "Block {} has {} event logs for ERC20 scan",
                    block_number,
                    logs.len()
                );

                for log in logs {
                    if log.topics.len() == 3 && log.topics[0] == transfer_topic {
                        let to = Address::from(log.topics[2]);
                        // Explicit lowercase normalization
                        let to_hex = format!("{:#x}", to).to_lowercase();

                        if let Some(match_res) = watched.get(&to_hex) {
                            if log.data.len() < 32 {
                                warn!(
                                    "ERC20 Transfer log to {} has insufficient data length ({}), skipping",
                                    to_hex,
                                    log.data.len()
                                );
                                continue;
                            }
                            let value = U256::from_big_endian(&log.data[0..32]);

                            if value == U256::zero() {
                                debug!("Skipping zero-value ERC20 transfer to {}", to_hex);
                                continue;
                            }

                            let tx_hash = match log.transaction_hash {
                                Some(h) => h,
                                None => {
                                    warn!("Skipping ERC20 Transfer log with no transaction hash");
                                    continue;
                                }
                            };

                            info!(
                                "ERC20 MATCH in block {}: tx {:#x} transfers {} to stealth address {}",
                                block_number, tx_hash, value, to_hex
                            );

                            // Verify the transaction was actually successful on-chain
                            if !self.is_tx_successful(provider, tx_hash).await {
                                continue;
                            }

                            let from = Address::from(log.topics[1]);
                            let tx_hash_hex = format!("{:#x}", tx_hash);
                            let token_address = format!("{:#x}", log.address).to_lowercase();

                            let deposit = NewDeposit {
                                paylink_id: match_res.paylink_id.clone(),
                                ephemeral_address_id: match_res.ephemeral_address_id.clone(),
                                tx_hash: tx_hash_hex.clone(),
                                log_index: log.log_index.map(|i| i.as_u64()),
                                block_number,
                                block_hash: block_hash_opt.clone(),
                                from_address: format!("{:#x}", from).to_lowercase(),
                                to_address: to_hex.clone(),
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
                                "Recording ERC20 deposit: {} (token: {}) to {} (tx: {})",
                                deposit.amount, token_address, deposit.to_address, deposit.tx_hash
                            );

                            match self.convex.upsert_deposit(&deposit).await {
                                Ok(result) => {
                                    info!(
                                        "Successfully recorded ERC20 deposit {} for paylink {}",
                                        result.deposit_id, result.paylink_id
                                    );
                                    erc20_matches += 1;
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to upsert ERC20 deposit to Convex for tx {}: {:?}",
                                        tx_hash_hex, e
                                    );
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!(
                    "Failed to fetch logs for block {} (hash {:#x}): {:?}",
                    block_number, block_hash, e
                );
            }
        }

        let total = native_matches + erc20_matches;
        if total > 0 {
            info!(
                "Block {} summary: {} deposit(s) found ({} native, {} ERC20) out of {} transactions",
                block_number, total, native_matches, erc20_matches, tx_count
            );
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
        let pending_count = pending.len();
        if pending_count > 0 {
            debug!(
                "Checking confirmations for {} pending deposit(s)",
                pending_count
            );
        }

        for deposit in pending {
            let confs = current_block.saturating_sub(deposit.block_number) + 1;

            // Throttle before each RPC call to avoid rate-limiting
            sleep(RPC_THROTTLE).await;

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
