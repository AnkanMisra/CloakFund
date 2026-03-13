use anyhow::{Context, Result};
use ethers::prelude::*;
use std::sync::Arc;
use tokio::time::{Duration, sleep};
use tracing::{debug, error, info};

use crate::config::WatcherConfig;
use crate::convex_client::ConvexRepository;

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
    async fn process_pending_jobs(&self, _provider: &Provider<Ws>) -> Result<()> {
        debug!("Checking for pending sweep jobs...");

        let jobs = self.convex.get_pending_sweep_jobs().await?;
        if jobs.is_empty() {
            return Ok(());
        }

        info!("Found {} pending sweep job(s)", jobs.len());

        for job in jobs {
            info!("Processing sweep job for deposit {}", job.deposit_id);

            // 1. Mark job as broadcasting to lock it
            if let Err(e) = self
                .convex
                .update_sweep_job_status(&job.id, "broadcasting", None)
                .await
            {
                error!("Failed to lock sweep job {}: {:?}", job.id, e);
                continue;
            }

            // TODO: Phase 3
            // 2. Fetch deposit & ephemeral address details from Convex
            // 3. Recover ephemeral private key using stealth logic & backend viewing key
            // 4. Calculate gas and construct sweep transaction
            // 5. Sign transaction and broadcast via provider
            // 6. Zeroize private key from memory
            // 7. Update job status to "completed" and save the sweep_tx_hash

            // For now, mock completion
            let mock_tx_hash = format!("0xmock_sweep_{}", job.deposit_id);

            if let Err(e) = self
                .convex
                .update_sweep_job_status(&job.id, "completed", Some(mock_tx_hash))
                .await
            {
                error!("Failed to complete sweep job {}: {:?}", job.id, e);
                // Attempt to revert to queued if completion fails?
            } else {
                info!("Successfully completed sweep job {}", job.id);
            }
        }

        Ok(())
    }
}
