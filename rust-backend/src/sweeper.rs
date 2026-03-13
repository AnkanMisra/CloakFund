use anyhow::{Context, Result};
use ethers::prelude::*;
use std::sync::Arc;
use tokio::time::{Duration, sleep};
use tracing::{debug, error, info, warn};

use crate::config::{SweeperConfig, WatcherConfig};
use crate::consolidator::{Consolidator, EphemeralPrivateKey, SweepOutcome};
use crate::convex_client::ConvexRepository;
use crate::stealth;
use std::env;
use std::str::FromStr;
use zeroize::{Zeroize, Zeroizing};

/// The SweeperService monitors Convex for finalized deposits and sweeps them
/// to the centralized treasury (e.g., BitGo vault).
pub struct SweeperService {
    config: WatcherConfig,
    convex: Arc<ConvexRepository>,
}

impl SweeperService {
    /// Creates a new SweeperService instance.
    pub fn new(config: WatcherConfig, convex: Arc<ConvexRepository>) -> Self {
        Self { config, convex }
    }

    /// Starts the sweeping loop.
    pub async fn start(&self) -> Result<()> {
        info!("Starting SweeperService...");

        let sweeper_config = SweeperConfig::from_env().context("Failed to load sweeper config")?;

        let provider = Provider::<Ws>::connect(&self.config.base_wss_url)
            .await
            .context("Failed to connect to Base WSS endpoint for sweeper")?;
        let provider = Arc::new(provider);

        loop {
            if let Err(e) = self.process_pending_jobs(&provider, &sweeper_config).await {
                error!("Error in sweeper loop: {:?}", e);
            }
            sleep(Duration::from_secs(self.config.polling_interval_secs)).await;
        }
    }

    /// Queries pending sweep jobs from Convex and attempts to execute them.
    async fn process_pending_jobs(
        &self,
        provider: &Arc<Provider<Ws>>,
        sweeper_config: &SweeperConfig,
    ) -> Result<()> {
        debug!("Checking for pending sweep jobs...");

        let jobs = self.convex.get_pending_sweep_jobs().await?;
        if jobs.is_empty() {
            return Ok(());
        }

        let treasury_addr = Address::from_str(&sweeper_config.treasury_address)
            .context("Invalid treasury address format")?;

        let consolidator = Consolidator::new(
            provider.clone(),
            treasury_addr,
            sweeper_config.dry_run,
            self.config.chain_id,
        );

        let recipient_priv_hex = env::var("RECIPIENT_PRIVATE_KEY_HEX").unwrap_or_default();
        if recipient_priv_hex.is_empty() {
            warn!("RECIPIENT_PRIVATE_KEY_HEX is not set; cannot sweep.");
            return Ok(());
        }

        for job in jobs {
            info!("Processing sweep job: {}", job.id);
            if let Err(e) = self
                .convex
                .update_sweep_job_status(&job.id, "broadcasting", None)
                .await
            {
                error!(
                    "Failed to update sweep job {} to broadcasting: {:?}",
                    job.id, e
                );
                continue;
            }

            let stealth_address = match Address::from_str(&job.stealth_address) {
                Ok(addr) => addr,
                Err(e) => {
                    error!("Invalid stealth address for job {}: {}", job.id, e);
                    let _ = self
                        .convex
                        .update_sweep_job_status(&job.id, "failed", None)
                        .await;
                    continue;
                }
            };

            let ephem_priv_key_hex = match stealth::recover_stealth_private_key(
                &recipient_priv_hex,
                &job.ephemeral_pubkey_hex,
            ) {
                Ok(key) => Zeroizing::new(key),
                Err(e) => {
                    error!(
                        "Failed to recover stealth private key for job {}: {}",
                        job.id, e
                    );
                    let _ = self
                        .convex
                        .update_sweep_job_status(&job.id, "failed", None)
                        .await;
                    continue;
                }
            };

            let mut ephem_priv_key = [0u8; 32];
            match hex::decode(ephem_priv_key_hex.trim_start_matches("0x")) {
                Ok(bytes) if bytes.len() == 32 => {
                    ephem_priv_key.copy_from_slice(&bytes);
                }
                _ => {
                    error!("Invalid recovered private key format for job {}", job.id);
                    let _ = self
                        .convex
                        .update_sweep_job_status(&job.id, "failed", None)
                        .await;
                    continue;
                }
            }

            let ephemeral_key = EphemeralPrivateKey(ephem_priv_key);
            ephem_priv_key.zeroize();

            let sweep_result = if job.asset_type == "native" {
                consolidator
                    .sweep_native(ephemeral_key, stealth_address)
                    .await
            } else if job.asset_type == "erc20" {
                let token_address = match job.token_address.as_deref().map(Address::from_str) {
                    Some(Ok(addr)) => addr,
                    _ => {
                        error!(
                            "Invalid or missing token address for ERC20 sweep in job {}",
                            job.id
                        );
                        let _ = self
                            .convex
                            .update_sweep_job_status(&job.id, "failed", None)
                            .await;
                        continue;
                    }
                };
                consolidator
                    .sweep_erc20(ephemeral_key, stealth_address, token_address)
                    .await
            } else {
                error!("Unknown asset type {} for job {}", job.asset_type, job.id);
                let _ = self
                    .convex
                    .update_sweep_job_status(&job.id, "failed", None)
                    .await;
                continue;
            };

            match sweep_result {
                Ok(SweepOutcome::Success(tx_hash)) => {
                    let tx_hash_str = format!("{:#x}", tx_hash);
                    info!("Successfully swept job {}. Tx: {}", job.id, tx_hash_str);
                    if let Err(e) = self
                        .convex
                        .update_sweep_job_status(&job.id, "completed", Some(tx_hash_str))
                        .await
                    {
                        error!("Failed to update job {} to completed: {:?}", job.id, e);
                    }
                }
                Ok(SweepOutcome::SkippedZeroBalance) => {
                    warn!(
                        "Skipped sweep job {} due to zero balance; returning to queued",
                        job.id
                    );
                    let _ = self
                        .convex
                        .update_sweep_job_status(&job.id, "queued", None)
                        .await;
                }
                Ok(SweepOutcome::SkippedInsufficientGas { balance, gas_cost }) => {
                    warn!(
                        "Skipped sweep job {} due to insufficient gas (balance: {}, gas_cost: {}); returning to queued",
                        job.id, balance, gas_cost
                    );
                    let _ = self
                        .convex
                        .update_sweep_job_status(&job.id, "queued", None)
                        .await;
                }
                Ok(SweepOutcome::SkippedDryRun) => {
                    info!(
                        "DRY RUN: Skipped sweep job {}; marking as completed with zero hash",
                        job.id
                    );
                    let tx_hash_str = format!("{:#x}", H256::zero());
                    let _ = self
                        .convex
                        .update_sweep_job_status(&job.id, "completed", Some(tx_hash_str))
                        .await;
                }
                Err(e) => {
                    error!("Failed to execute sweep for job {}: {:?}", job.id, e);
                    let _ = self
                        .convex
                        .update_sweep_job_status(&job.id, "failed", None)
                        .await;
                }
            }
        }

        Ok(())
    }
}
