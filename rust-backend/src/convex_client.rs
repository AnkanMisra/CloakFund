use anyhow::{Context, Result};
use convex::ConvexClient;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::ConvexClientConfig;
use crate::models::{
    ConfirmationUpdateResult, DepositMatch, DepositRecord, DepositStatusResponse, NewDeposit,
    SweepJobRecord, UpsertDepositResult, WatcherCheckpoint,
};

/// A repository interface for interacting with the Convex backend functions.
///
/// This client acts as the bridge between the Rust watcher / backend logic
/// and the Convex managed database.
#[derive(Clone)]
pub struct ConvexRepository {
    // The Convex client requires mutable access for queries and mutations,
    // so it is wrapped in an async Mutex for safe shared access across the app.
    client: Arc<Mutex<ConvexClient>>,
}

impl ConvexRepository {
    /// Initializes a new Convex client using the provided configuration.
    pub async fn new(config: &ConvexClientConfig) -> Result<Self> {
        let mut client = ConvexClient::new(&config.deployment_url)
            .await
            .context("Failed to initialize Convex client")?;

        if let Some(admin_key) = &config.admin_key {
            client.set_admin_auth(admin_key.clone(), None).await;
        }

        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }

    /// Creates a new paylink and its associated ephemeral address transactionally.
    pub async fn create_paylink_with_address(
        &self,
        paylink: &crate::models::NewPaylinkWithAddress,
    ) -> Result<serde_json::Value> {
        let json_val = serde_json::to_value(paylink)?;
        let convex_val = json_to_convex(json_val);

        let args = if let convex::Value::Object(map) = convex_val {
            map
        } else {
            anyhow::bail!("Invalid paylink with address object")
        };

        let mut client = self.client.lock().await;
        let result = client
            .mutation("paylinks:createWithEphemeralAddress", args)
            .await?;

        match result {
            convex::FunctionResult::Value(val) => Ok(convex_to_json(val)),
            convex::FunctionResult::ErrorMessage(msg) => anyhow::bail!("Convex error: {}", msg),
            convex::FunctionResult::ConvexError(err) => {
                anyhow::bail!("Convex logic error: {}", err.message)
            }
        }
    }

    /// Creates a new paylink in Convex.
    pub async fn create_paylink(
        &self,
        paylink: &crate::models::NewPaylink,
    ) -> Result<serde_json::Value> {
        let json_val = serde_json::to_value(paylink)?;
        let convex_val = json_to_convex(json_val);

        let args = if let convex::Value::Object(map) = convex_val {
            map
        } else {
            anyhow::bail!("Invalid paylink object")
        };

        let mut client = self.client.lock().await;
        let result = client.mutation("paylinks:create", args).await?;

        match result {
            convex::FunctionResult::Value(val) => Ok(convex_to_json(val)),
            convex::FunctionResult::ErrorMessage(msg) => anyhow::bail!("Convex error: {}", msg),
            convex::FunctionResult::ConvexError(err) => {
                anyhow::bail!("Convex logic error: {}", err.message)
            }
        }
    }

    /// Creates a new ephemeral address for a paylink in Convex.
    pub async fn create_ephemeral_address(
        &self,
        address: &crate::models::NewEphemeralAddress,
    ) -> Result<serde_json::Value> {
        let json_val = serde_json::to_value(address)?;
        let convex_val = json_to_convex(json_val);

        let args = if let convex::Value::Object(map) = convex_val {
            map
        } else {
            anyhow::bail!("Invalid ephemeral address object")
        };

        let mut client = self.client.lock().await;
        let result = client
            .mutation("paylinks:createEphemeralAddress", args)
            .await?;

        match result {
            convex::FunctionResult::Value(val) => Ok(convex_to_json(val)),
            convex::FunctionResult::ErrorMessage(msg) => anyhow::bail!("Convex error: {}", msg),
            convex::FunctionResult::ConvexError(err) => {
                anyhow::bail!("Convex logic error: {}", err.message)
            }
        }
    }

    /// Gets a paylink by ID.
    pub async fn get_paylink(&self, paylink_id: &str) -> Result<Option<serde_json::Value>> {
        let mut args = std::collections::BTreeMap::new();
        args.insert(
            "paylinkId".to_string(),
            convex::Value::String(paylink_id.to_string()),
        );

        let mut client = self.client.lock().await;
        let result = client.query("paylinks:getById", args).await?;

        match result {
            convex::FunctionResult::Value(val) => {
                let json = convex_to_json(val);
                if json.is_null() {
                    Ok(None)
                } else {
                    Ok(Some(json))
                }
            }
            convex::FunctionResult::ErrorMessage(msg) => anyhow::bail!("Convex error: {}", msg),
            convex::FunctionResult::ConvexError(err) => {
                anyhow::bail!("Convex logic error: {}", err.message)
            }
        }
    }

    /// Queries Convex to check if a stealth address corresponds to a known paylink on a given chain.
    pub async fn get_ephemeral_address_match(
        &self,
        chain_id: u64,
        stealth_address: &str,
    ) -> Result<Option<DepositMatch>> {
        let mut args = std::collections::BTreeMap::new();
        args.insert(
            "chainId".to_string(),
            convex::Value::Float64(chain_id as f64),
        );
        args.insert(
            "stealthAddress".to_string(),
            convex::Value::String(stealth_address.to_string()),
        );

        let mut client = self.client.lock().await;
        let result = client
            .query("paylinks:getEphemeralAddressMatch", args)
            .await?;

        match result {
            convex::FunctionResult::Value(val) => {
                let json = convex_to_json(val);
                if json.is_null() {
                    Ok(None)
                } else {
                    let match_res: DepositMatch = serde_json::from_value(json)?;
                    Ok(Some(match_res))
                }
            }
            convex::FunctionResult::ErrorMessage(msg) => anyhow::bail!("Convex error: {}", msg),
            convex::FunctionResult::ConvexError(err) => {
                anyhow::bail!("Convex logic error: {}", err.message)
            }
        }
    }

    /// Upserts a deposit record (creates if new, updates if existing based on txHash/logIndex).
    pub async fn upsert_deposit(&self, deposit: &NewDeposit) -> Result<UpsertDepositResult> {
        let json_val = serde_json::to_value(deposit)?;
        let convex_val = json_to_convex(json_val);

        let args = if let convex::Value::Object(map) = convex_val {
            map
        } else {
            anyhow::bail!("Invalid deposit object")
        };

        let mut client = self.client.lock().await;
        let result = client.mutation("deposits:upsertDeposit", args).await?;

        match result {
            convex::FunctionResult::Value(val) => {
                let res: UpsertDepositResult = serde_json::from_value(convex_to_json(val))?;
                Ok(res)
            }
            convex::FunctionResult::ErrorMessage(msg) => anyhow::bail!("Convex error: {}", msg),
            convex::FunctionResult::ConvexError(err) => {
                anyhow::bail!("Convex logic error: {}", err.message)
            }
        }
    }

    /// Updates the confirmation count and status for an existing deposit.
    pub async fn update_confirmations(
        &self,
        deposit_id: &str,
        confirmations: u64,
        required_confirmations: u64,
    ) -> Result<Option<ConfirmationUpdateResult>> {
        let mut args = std::collections::BTreeMap::new();
        args.insert(
            "depositId".to_string(),
            convex::Value::String(deposit_id.to_string()),
        );
        args.insert(
            "confirmations".to_string(),
            convex::Value::Float64(confirmations as f64),
        );
        args.insert(
            "requiredConfirmations".to_string(),
            convex::Value::Float64(required_confirmations as f64),
        );

        let mut client = self.client.lock().await;
        let result = client
            .mutation("deposits:updateConfirmations", args)
            .await?;

        match result {
            convex::FunctionResult::Value(val) => {
                let json = convex_to_json(val);
                if json.is_null() {
                    Ok(None)
                } else {
                    let res: ConfirmationUpdateResult = serde_json::from_value(json)?;
                    Ok(Some(res))
                }
            }
            convex::FunctionResult::ErrorMessage(msg) => anyhow::bail!("Convex error: {}", msg),
            convex::FunctionResult::ConvexError(err) => {
                anyhow::bail!("Convex logic error: {}", err.message)
            }
        }
    }

    /// Marks a deposit as reorged (e.g., if it disappeared from the chain).
    pub async fn mark_deposit_reorged(&self, deposit_id: &str) -> Result<()> {
        let mut args = std::collections::BTreeMap::new();
        args.insert(
            "depositId".to_string(),
            convex::Value::String(deposit_id.to_string()),
        );

        let mut client = self.client.lock().await;
        let result = client.mutation("deposits:markDepositReorged", args).await?;

        match result {
            convex::FunctionResult::Value(_) => Ok(()),
            convex::FunctionResult::ErrorMessage(msg) => anyhow::bail!("Convex error: {}", msg),
            convex::FunctionResult::ConvexError(err) => {
                anyhow::bail!("Convex logic error: {}", err.message)
            }
        }
    }

    /// Fetches deposits that are still pending finalization to check their current status.
    pub async fn get_pending_confirmation_updates(&self) -> Result<Vec<DepositRecord>> {
        let args = std::collections::BTreeMap::new();

        let mut client = self.client.lock().await;
        let result = client
            .query("deposits:getPendingConfirmationUpdates", args)
            .await?;

        match result {
            convex::FunctionResult::Value(val) => {
                let res: Vec<DepositRecord> = serde_json::from_value(convex_to_json(val))?;
                Ok(res)
            }
            convex::FunctionResult::ErrorMessage(msg) => anyhow::bail!("Convex error: {}", msg),
            convex::FunctionResult::ConvexError(err) => {
                anyhow::bail!("Convex logic error: {}", err.message)
            }
        }
    }

    /// Updates the latest processed block checkpoint for the watcher.
    pub async fn update_checkpoint(
        &self,
        latest_processed_block: u64,
        latest_confirmed_block: u64,
    ) -> Result<()> {
        let mut args = std::collections::BTreeMap::new();
        args.insert(
            "latestProcessedBlock".to_string(),
            convex::Value::Float64(latest_processed_block as f64),
        );
        args.insert(
            "latestConfirmedBlock".to_string(),
            convex::Value::Float64(latest_confirmed_block as f64),
        );

        let mut client = self.client.lock().await;
        let result = client.mutation("deposits:updateCheckpoint", args).await?;

        match result {
            convex::FunctionResult::Value(_) => Ok(()),
            convex::FunctionResult::ErrorMessage(msg) => anyhow::bail!("Convex error: {}", msg),
            convex::FunctionResult::ConvexError(err) => {
                anyhow::bail!("Convex logic error: {}", err.message)
            }
        }
    }

    /// Fetches the latest processed block checkpoint for the watcher.
    pub async fn get_latest_checkpoint(&self) -> Result<Option<WatcherCheckpoint>> {
        let args = std::collections::BTreeMap::new();

        let mut client = self.client.lock().await;
        let result = client.query("deposits:getLatestCheckpoint", args).await?;

        match result {
            convex::FunctionResult::Value(val) => {
                let json = convex_to_json(val);
                if json.is_null() {
                    Ok(None)
                } else {
                    let res: WatcherCheckpoint = serde_json::from_value(json)?;
                    Ok(Some(res))
                }
            }
            convex::FunctionResult::ErrorMessage(msg) => anyhow::bail!("Convex error: {}", msg),
            convex::FunctionResult::ConvexError(err) => {
                anyhow::bail!("Convex logic error: {}", err.message)
            }
        }
    }

    /// Fetches the aggregated deposit status for a given paylink.
    pub async fn get_deposit_status(&self, paylink_id: &str) -> Result<DepositStatusResponse> {
        let mut args = std::collections::BTreeMap::new();
        args.insert(
            "paylinkId".to_string(),
            convex::Value::String(paylink_id.to_string()),
        );

        let mut client = self.client.lock().await;
        let result = client.query("deposits:getDepositStatus", args).await?;

        match result {
            convex::FunctionResult::Value(val) => {
                let res: DepositStatusResponse = serde_json::from_value(convex_to_json(val))?;
                Ok(res)
            }
            convex::FunctionResult::ErrorMessage(msg) => anyhow::bail!("Convex error: {}", msg),
            convex::FunctionResult::ConvexError(err) => {
                anyhow::bail!("Convex logic error: {}", err.message)
            }
        }
    }

    /// Fetches pending sweep jobs.
    pub async fn get_pending_sweep_jobs(&self) -> Result<Vec<SweepJobRecord>> {
        let args = std::collections::BTreeMap::new();

        let mut client = self.client.lock().await;
        let result = client.query("sweeps:getPendingSweepJobs", args).await?;

        match result {
            convex::FunctionResult::Value(val) => {
                let res: Vec<SweepJobRecord> = serde_json::from_value(convex_to_json(val))?;
                Ok(res)
            }
            convex::FunctionResult::ErrorMessage(msg) => anyhow::bail!("Convex error: {}", msg),
            convex::FunctionResult::ConvexError(err) => {
                anyhow::bail!("Convex logic error: {}", err.message)
            }
        }
    }

    /// Updates the status of a sweep job.
    pub async fn update_sweep_job_status(
        &self,
        job_id: &str,
        status: &str,
        sweep_tx_hash: Option<String>,
    ) -> Result<()> {
        let mut args = std::collections::BTreeMap::new();
        args.insert(
            "jobId".to_string(),
            convex::Value::String(job_id.to_string()),
        );
        args.insert(
            "status".to_string(),
            convex::Value::String(status.to_string()),
        );
        if let Some(hash) = sweep_tx_hash {
            args.insert("sweepTxHash".to_string(), convex::Value::String(hash));
        }

        let mut client = self.client.lock().await;
        let result = client.mutation("sweeps:updateSweepJobStatus", args).await?;

        match result {
            convex::FunctionResult::Value(_) => Ok(()),
            convex::FunctionResult::ErrorMessage(msg) => anyhow::bail!("Convex error: {}", msg),
            convex::FunctionResult::ConvexError(err) => {
                anyhow::bail!("Convex logic error: {}", err.message)
            }
        }
    }
}

fn json_to_convex(json: serde_json::Value) -> convex::Value {
    match json {
        serde_json::Value::Null => convex::Value::Null,
        serde_json::Value::Bool(b) => convex::Value::Boolean(b),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                convex::Value::Float64(f)
            } else {
                convex::Value::Null
            }
        }
        serde_json::Value::String(s) => convex::Value::String(s),
        serde_json::Value::Array(arr) => {
            convex::Value::Array(arr.into_iter().map(json_to_convex).collect())
        }
        serde_json::Value::Object(obj) => {
            let mut map = std::collections::BTreeMap::new();
            for (k, v) in obj {
                map.insert(k, json_to_convex(v));
            }
            convex::Value::Object(map)
        }
    }
}

fn convex_to_json(cvx: convex::Value) -> serde_json::Value {
    match cvx {
        convex::Value::Null => serde_json::Value::Null,
        convex::Value::Int64(i) => serde_json::Value::Number(i.into()),
        convex::Value::Float64(f) => {
            if let Some(n) = serde_json::Number::from_f64(f) {
                serde_json::Value::Number(n)
            } else {
                serde_json::Value::Null
            }
        }
        convex::Value::Boolean(b) => serde_json::Value::Bool(b),
        convex::Value::String(s) => serde_json::Value::String(s),
        convex::Value::Bytes(_) => serde_json::Value::Null,
        convex::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(convex_to_json).collect())
        }
        convex::Value::Object(obj) => {
            let mut map = serde_json::Map::new();
            for (k, v) in obj {
                map.insert(k, convex_to_json(v));
            }
            serde_json::Value::Object(map)
        }
    }
}
