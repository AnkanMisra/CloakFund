use anyhow::{Context, Result};
use ethers::prelude::*;
use std::sync::Arc;
use tokio::time::{Duration, sleep};
use tracing::{debug, error, info, warn};

use crate::config::{SweeperConfig, WatcherConfig};
use crate::consolidator::{Consolidator, EphemeralPrivateKey};
use crate::convex_client::ConvexRepository;
use crate::stealth;
use std::env;
use std::str::FromStr;

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

        let provider = Provider::<Ws>::connect(&self.config.base_wss_url)
            .await
            .context("Failed to connect to Base WSS endpoint for sweeper")?;
        let provider = Arc::new(provider);

        loop {
            if let Err(e) = self.process_pending_jobs(&provider).await {
                error!("Error in sweeper loop: {:?}", e);
            }
            sleep(Duration::from_secs(self.config.polling_interval_secs)).await;
        }
    }

    /// Queries pending sweep jobs from Convex and attempts to execute them.
    async fn process_pending_jobs(&self, provider: &Arc<Provider<Ws>>) -> Result<()> {
        debug!("Checking for pending sweep jobs...");

        let jobs = self.convex.get_pending_sweep_jobs().await?;
        if jobs.is_empty() {
            return Ok(());
        }

        let sweeper_config = match SweeperConfig::from_env() {
            Ok(c) => c,
            Err(e) => {
                error!("Missing sweeper config: {:?}", e);
                return Ok(());
            }
        };

        let treasury_addr = Address::from_str(&sweeper_config.treasury_address)
            .context("Invalid treasury address format")?;

        let consolidator =
            Consolidator::new(provider.clone(), treasury_addr, sweeper_config.dry_run);

        let recipient_priv_hex = env::var("RECIPIENT_PRIVATE_KEY_HEX").unwrap_or_default();
        if recipient_priv_hex.is_empty() {
            warn!("RECIPIENT_PRIVATE_KEY_HEX is not set; cannot sweep.");
            return Ok(());
        }

        for job in jobs {
            info!("Processing sweep job: {}", job.id);
            self.convex
                .update_sweep_job_status(&job.id, "broadcasting", None)
                .await?;

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
                Ok(key) => key,
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

            let sweep_result = if job.asset_type == "native" {
                consolidator
                    .sweep_native(EphemeralPrivateKey(ephem_priv_key), stealth_address)
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
                    .sweep_erc20(
                        EphemeralPrivateKey(ephem_priv_key),
                        stealth_address,
                        token_address,
                    )
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
                Ok(tx_hash) => {
                    let tx_hash_str = format!("{:#x}", tx_hash);
                    info!("Successfully swept job {}. Tx: {}", job.id, tx_hash_str);
                    self.convex
                        .update_sweep_job_status(&job.id, "completed", Some(tx_hash_str))
                        .await?;
                }
                Err(e) => {
                    error!("Failed to execute sweep for job {}: {:?}", job.id, e);
                    self.convex
                        .update_sweep_job_status(&job.id, "failed", None)
                        .await?;
                }
            }
        }

        Ok(())
    }
}
