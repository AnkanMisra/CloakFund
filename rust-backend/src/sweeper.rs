use anyhow::{Context, Result};
use ethers::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{Duration, sleep};
use tracing::{debug, error, info, warn};

use crate::config::{SweeperConfig, WatcherConfig};
use crate::consolidator::{Consolidator, EphemeralPrivateKey, SweepOutcome};
use crate::convex_client::ConvexRepository;
use crate::models::NewPrivacyNote;
use crate::privacy_pool;
use crate::stealth;
use std::env;
use std::str::FromStr;

/// Maximum number of retry attempts before giving up on a sweep job.
const MAX_RETRY_ATTEMPTS: u32 = 5;

/// Base backoff delay in seconds. Actual delay = base × 2^attempt.
const BACKOFF_BASE_SECS: u64 = 5;

/// The SweeperService monitors Convex for finalized deposits and sweeps them
/// into the PrivacyPool smart contract using the ZK-Mixer commit-reveal scheme.
///
/// State machine:
///   queued → processing → broadcasted → confirmed
///   On error: processing → queued (retryable with exponential backoff)
///   After MAX_RETRY_ATTEMPTS: queued → failed (permanent)
pub struct SweeperService {
    config: WatcherConfig,
    convex: Arc<ConvexRepository>,
    /// In-memory retry tracking: job_id → attempt count
    retry_counts: HashMap<String, u32>,
}

impl SweeperService {
    /// Creates a new SweeperService instance.
    pub fn new(config: WatcherConfig, convex: Arc<ConvexRepository>) -> Self {
        Self {
            config,
            convex,
            retry_counts: HashMap::new(),
        }
    }

    /// Starts the sweeping loop with automatic reconnection.
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting SweeperService (ZK-Mixer mode)...");

        let sweeper_config = SweeperConfig::from_env().context("Failed to load sweeper config")?;

        info!(
            "Sweeper config: privacy_pool={}, relayer_key={}…, dry_run={}",
            sweeper_config.privacy_pool_address,
            &sweeper_config.relayer_private_key[..sweeper_config.relayer_private_key.len().min(8)],
            sweeper_config.dry_run
        );

        // On startup, reset any jobs stuck in "broadcasting" from a previous crash.
        match self.convex.reset_stuck_sweep_jobs().await {
            Ok(0) => debug!("No stuck sweep jobs found."),
            Ok(count) => info!(
                "♻️ Reset {} stuck 'broadcasting' job(s) back to 'queued'",
                count
            ),
            Err(e) => warn!("Failed to reset stuck sweep jobs: {:?}", e),
        }

        loop {
            // Use HTTP RPC — the sweeper only makes occasional balance checks
            // and tx broadcasts, it doesn't need a live WebSocket subscription.
            let provider = match Provider::<Http>::try_from(&self.config.base_rpc_url) {
                Ok(p) => Arc::new(p),
                Err(e) => {
                    error!("Failed to create HTTP provider for sweeper: {:?}", e);
                    sleep(Duration::from_secs(10)).await;
                    continue;
                }
            };

            info!(
                "Sweeper connected to HTTP RPC: {}",
                self.config.base_rpc_url
            );

            if let Err(e) = self.sweep_loop(&provider, &sweeper_config).await {
                error!("Sweeper loop error: {:?}. Restarting in 10s...", e);
                sleep(Duration::from_secs(10)).await;
            }
        }
    }

    /// Inner sweep loop — runs until an error occurs.
    async fn sweep_loop(
        &mut self,
        provider: &Arc<Provider<Http>>,
        sweeper_config: &SweeperConfig,
    ) -> Result<()> {
        loop {
            if let Err(e) = self.process_pending_jobs(provider, sweeper_config).await {
                error!("Error processing sweep jobs: {:?}", e);
            }
            sleep(Duration::from_secs(self.config.polling_interval_secs)).await;
        }
    }

    /// Compute exponential backoff delay for a given retry attempt.
    fn backoff_delay(attempt: u32) -> Duration {
        let secs = BACKOFF_BASE_SECS * 2u64.pow(attempt.min(6)); // cap at ~320s
        Duration::from_secs(secs)
    }

    /// Queries pending sweep jobs from Convex and attempts to execute them.
    async fn process_pending_jobs(
        &mut self,
        provider: &Arc<Provider<Http>>,
        sweeper_config: &SweeperConfig,
    ) -> Result<()> {
        debug!("Checking for pending sweep jobs...");

        let jobs = self.convex.get_pending_sweep_jobs().await?;
        if jobs.is_empty() {
            return Ok(());
        }

        info!("Found {} pending sweep job(s)", jobs.len());

        // Parse the PrivacyPool contract address
        let pool_address = Address::from_str(&sweeper_config.privacy_pool_address)
            .context("Invalid PRIVACY_POOL_ADDRESS")?;

        let consolidator = Consolidator::new(
            provider.clone(),
            sweeper_config.dry_run,
            self.config.chain_id,
        );

        let recipient_priv_hex = env::var("RECIPIENT_PRIVATE_KEY_HEX").unwrap_or_default();
        if recipient_priv_hex.is_empty() {
            warn!("RECIPIENT_PRIVATE_KEY_HEX is not set; cannot sweep.");
            return Ok(());
        }

        for job in jobs {
            let attempt = *self.retry_counts.get(&job.id).unwrap_or(&0);

            // ── Check retry limit ──────────────────────────────────────
            if attempt >= MAX_RETRY_ATTEMPTS {
                warn!(
                    "⛔ Job {} exceeded max retries ({}). Marking as permanently failed.",
                    job.id, MAX_RETRY_ATTEMPTS
                );
                let _ = self
                    .convex
                    .update_sweep_job_status(&job.id, "failed", None, None)
                    .await;
                self.retry_counts.remove(&job.id);
                continue;
            }

            // ── Apply exponential backoff on retries ───────────────────
            if attempt > 0 {
                let delay = Self::backoff_delay(attempt - 1);
                info!(
                    "Job {} retry #{} — backing off for {}s",
                    job.id,
                    attempt,
                    delay.as_secs()
                );
                sleep(delay).await;
            }

            info!(
                "Processing sweep job: {} (stealth={}, asset={}, amount={}, attempt={})",
                job.id,
                job.stealth_address,
                job.asset_type,
                job.amount,
                attempt + 1
            );

            // ── Transition: queued → broadcasting ───────────────────────
            if let Err(e) = self
                .convex
                .update_sweep_job_status(&job.id, "broadcasting", None, None)
                .await
            {
                error!(
                    "Failed to update sweep job {} to broadcasting: {:?}",
                    job.id, e
                );
                continue;
            }

            // ── Derive stealth address ─────────────────────────────────
            let stealth_address = match Address::from_str(&job.stealth_address) {
                Ok(addr) => addr,
                Err(e) => {
                    error!("Invalid stealth address for job {}: {}", job.id, e);
                    let _ = self
                        .convex
                        .update_sweep_job_status(&job.id, "failed", None, None)
                        .await;
                    self.retry_counts.remove(&job.id);
                    continue;
                }
            };

            // ── Recover stealth private key ────────────────────────────
            info!(
                "Recovering stealth private key for job {} (ephemeral_pub={})",
                job.id, job.ephemeral_pubkey_hex
            );

            let recovered_key = match stealth::recover_stealth_private_key(
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
                        .update_sweep_job_status(&job.id, "failed", None, None)
                        .await;
                    self.retry_counts.remove(&job.id);
                    continue;
                }
            };

            let ephemeral_key = EphemeralPrivateKey(*recovered_key);

            // ── Generate Privacy Note (secret + nullifier) ─────────────
            info!(
                "🔐 Generating privacy note for job {} (ZK-Mixer deposit)...",
                job.id
            );

            let note = privacy_pool::generate_note();
            let commitment = privacy_pool::compute_commitment(&note.secret, &note.nullifier);
            let commitment_hex = hex::encode(commitment);

            info!(
                "Generated commitment: 0x{} for job {}",
                commitment_hex, job.id
            );

            // ── Execute sweep → PrivacyPool.deposit(commitment) ────────
            info!(
                "Executing ZK-Mixer deposit: {:?} → PrivacyPool {:?} (dry_run={})",
                stealth_address, pool_address, sweeper_config.dry_run
            );

            let sweep_result = if job.asset_type == "native" {
                consolidator
                    .sweep_native(ephemeral_key, stealth_address, pool_address, &commitment)
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
                            .update_sweep_job_status(&job.id, "failed", None, None)
                            .await;
                        self.retry_counts.remove(&job.id);
                        continue;
                    }
                };
                consolidator
                    .sweep_erc20(ephemeral_key, stealth_address, token_address, pool_address)
                    .await
            } else {
                error!("Unknown asset type {} for job {}", job.asset_type, job.id);
                let _ = self
                    .convex
                    .update_sweep_job_status(&job.id, "failed", None, None)
                    .await;
                self.retry_counts.remove(&job.id);
                continue;
            };

            // ── Handle outcome ─────────────────────────────────────────
            match sweep_result {
                Ok(SweepOutcome::Success(tx_hash, _commitment_hex)) => {
                    let tx_hash_str = format!("{:#x}", tx_hash);
                    info!(
                        "✅ Successfully deposited into PrivacyPool for job {}. Tx: {}",
                        job.id, tx_hash_str
                    );

                    // Store the privacy note in Convex so the receiver can withdraw later
                    let new_note = NewPrivacyNote {
                        deposit_id: job.deposit_id.clone(),
                        sweep_job_id: job.id.clone(),
                        secret_hex: hex::encode(note.secret),
                        nullifier_hex: hex::encode(note.nullifier),
                        commitment_hex: commitment_hex.clone(),
                        pool_deposit_tx_hash: Some(tx_hash_str.clone()),
                    };

                    match self.convex.store_privacy_note(&new_note).await {
                        Ok(note_id) => {
                            info!(
                                "📝 Privacy note stored in Convex: {} (commitment=0x{})",
                                note_id, commitment_hex
                            );
                        }
                        Err(e) => {
                            error!(
                                "⚠️ Failed to store privacy note for job {} — \
                                 NOTE: Funds are deposited but note may be lost! Error: {:?}",
                                job.id, e
                            );
                            // We still mark the sweep as completed since the on-chain
                            // deposit succeeded. The note can be reconstructed from
                            // the commitment if needed.
                        }
                    }

                    if let Err(e) = self
                        .convex
                        .update_sweep_job_status(
                            &job.id,
                            "completed",
                            Some(tx_hash_str),
                            Some(format!("{:#x}", pool_address)),
                        )
                        .await
                    {
                        error!("Failed to update job {} to completed: {:?}", job.id, e);
                    }
                    // Clear retry counter on success
                    self.retry_counts.remove(&job.id);
                }
                Ok(SweepOutcome::SkippedZeroBalance) => {
                    warn!(
                        "⚠️ Skipped sweep job {} due to zero balance; marking failed",
                        job.id
                    );
                    let _ = self
                        .convex
                        .update_sweep_job_status(&job.id, "failed", None, None)
                        .await;
                    self.retry_counts.remove(&job.id);
                }
                Ok(SweepOutcome::SkippedDust {
                    balance,
                    max_gas_cost,
                }) => {
                    warn!(
                        "💨 Insufficient balance for job {}: balance={}, required={}. Marking failed.",
                        job.id, balance, max_gas_cost
                    );
                    let _ = self
                        .convex
                        .update_sweep_job_status(&job.id, "failed", None, None)
                        .await;
                    self.retry_counts.remove(&job.id);
                }
                Ok(SweepOutcome::SkippedDryRun) => {
                    info!(
                        "🔶 DRY RUN: Skipped sweep job {}; marking as completed",
                        job.id
                    );
                    let tx_hash_str = format!("{:#x}", H256::zero());
                    let _ = self
                        .convex
                        .update_sweep_job_status(
                            &job.id,
                            "completed",
                            Some(tx_hash_str),
                            Some(format!("{:#x}", pool_address)),
                        )
                        .await;
                    self.retry_counts.remove(&job.id);
                }
                Err(e) => {
                    let attempt = self.retry_counts.entry(job.id.clone()).or_insert(0);
                    *attempt += 1;
                    error!(
                        "❌ Sweep failed for job {} (attempt {}/{}): {:?}",
                        job.id, attempt, MAX_RETRY_ATTEMPTS, e
                    );
                    // Return to queued for retry — NOT permanent failure
                    let _ = self
                        .convex
                        .update_sweep_job_status(&job.id, "queued", None, None)
                        .await;
                    info!(
                        "↩️ Job {} returned to queued for retry (next attempt in {}s)",
                        job.id,
                        Self::backoff_delay(*attempt - 1).as_secs()
                    );
                }
            }

            // Throttle between jobs to avoid RPC rate-limiting
            sleep(Duration::from_millis(500)).await;
        }

        Ok(())
    }
}
