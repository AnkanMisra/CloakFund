use anyhow::{Context, Result};
use ethers::prelude::*;
use std::sync::Arc;
use tokio::time::{Duration, sleep};
use tracing::{debug, error, info, warn};

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

        warn!(
            "Sweeper not yet implemented (Phase 6). {} jobs queued but not swept.",
            jobs.len()
        );

        Ok(())
    }
}
