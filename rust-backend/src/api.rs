
use axum::{
    Json, Router,
    extract::{Path, Query, State},

    response::IntoResponse,
    routing::{get, post},
};
use ethers::prelude::*;
use serde_json::json;
use std::str::FromStr;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::convex_client::ConvexRepository;
use crate::privacy_pool;

#[path = "ccip.rs"]
pub mod ccip;

pub fn create_router(state: Arc<ConvexRepository>) -> anyhow::Result<Router> {
    // Allow any origin in development so the frontend can run from
    // file://, localhost:5500, or any other local dev server.
    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers(tower_http::cors::Any);

    Ok(Router::new()
        .route("/health", get(health_check))
        .route("/gateway/:sender/:data", get(ccip::ccip_resolve))
        .route("/api/v1/paylink", post(create_paylink))
        .route("/api/v1/paylink/:id", get(get_paylink))
        .route("/api/v1/consolidate", post(consolidate_funds))
        .route("/api/v1/withdraw", post(relay_withdraw))
        .route("/api/v1/deposit/status", get(deposit_status))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state))
}

async fn health_check() -> impl IntoResponse {
    let timestamp = chrono::Utc::now().timestamp();
    Json(json!({
        "status": "ok",
        "service": "cloakfund-rust-backend",
        "mode": "zk-mixer",
        "timestamp": timestamp,
    }))
}

async fn resolve_ens_pubkey(ens_name: &str) -> anyhow::Result<String> {
    use ethers::prelude::*;

    // Try the user-configured RPC first, then fall back to free public RPCs
    let configured = std::env::var("ETH_MAINNET_RPC_URL").ok();
    let fallback_rpcs: Vec<String> = vec![
        "https://cloudflare-eth.com".to_string(),
        "https://rpc.ankr.com/eth".to_string(),
        "https://ethereum-rpc.publicnode.com".to_string(),
        "https://eth.llamarpc.com".to_string(),
    ];

    let mut rpcs: Vec<String> = Vec::new();
    if let Some(configured_rpc) = configured {
        rpcs.push(configured_rpc);
    }
    rpcs.extend(fallback_rpcs);

    let mut last_error = String::new();

    for rpc_url in &rpcs {
        tracing::debug!("Trying ENS resolution via RPC: {}", rpc_url);
        match Provider::<Http>::try_from(rpc_url.as_str()) {
            Ok(provider) => {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(8),
                    provider.resolve_field(ens_name, "cloak.pubkey"),
                )
                .await
                {
                    Ok(Ok(pubkey)) if !pubkey.is_empty() => {
                        tracing::info!("ENS resolved via {}: got pubkey", rpc_url);
                        return Ok(pubkey);
                    }
                    Ok(Ok(_)) => {
                        last_error = format!("No cloak.pubkey text record found for {}", ens_name);
                        tracing::warn!("{} (via {})", last_error, rpc_url);
                        // Don't try other RPCs — the ENS record genuinely doesn't exist
                        return Err(anyhow::anyhow!("{}", last_error));
                    }
                    Ok(Err(e)) => {
                        last_error = format!("{}", e);
                        tracing::warn!("ENS resolution failed via {}: {}", rpc_url, e);
                    }
                    Err(_) => {
                        last_error = "timeout after 8s".to_string();
                        tracing::warn!("ENS resolution timed out via {}", rpc_url);
                    }
                }
            }
            Err(e) => {
                last_error = format!("Invalid RPC URL: {}", e);
                tracing::warn!("{}", last_error);
            }
        }
    }

    anyhow::bail!(
        "All ENS RPCs failed for {}. Last error: {}",
        ens_name,
        last_error
    )
}

async fn create_paylink(
    State(state): State<Arc<ConvexRepository>>,
    Json(payload): Json<crate::models::CreatePaylinkRequest>,
) -> impl IntoResponse {
    let chain_id = payload.chain_id.unwrap_or(8453); // Default base
    let network = payload.network.unwrap_or_else(|| "base".to_string());

    let recipient_pubkey = match payload.recipient_public_key_hex {
        Some(key) => key,
        None => {
            if let Some(ens_name) = &payload.ens_name {
                match resolve_ens_pubkey(ens_name).await {
                    Ok(key) => key,
                    Err(e) => {
                        return (
                            axum::http::StatusCode::BAD_REQUEST,
                            Json(serde_json::json!({ "error": format!("Failed to resolve ENS public key for {}: {}", ens_name, e) })),
                        )
                            .into_response();
                    }
                }
            } else {
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": "Either recipient_public_key_hex or ens_name must be provided" })),
                )
                    .into_response();
            }
        }
    };

    let (stealth_address, ephemeral_pubkey_hex, view_tag) =
        match crate::stealth::generate_stealth_address(&recipient_pubkey) {
            Ok(res) => res,
            Err(e) => {
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("Failed to generate stealth address: {}", e) })),
                )
                    .into_response();
            }
        };

    let new_paylink = crate::models::NewPaylinkWithAddress {
        user_id: None,
        ens_name: payload.ens_name,
        recipient_public_key_hex: recipient_pubkey,
        metadata: payload.metadata,
        chain_id,
        network: network.clone(),
        stealth_address: stealth_address.clone(),
        ephemeral_pubkey_hex: ephemeral_pubkey_hex.clone(),
        view_tag,
    };

    let paylink_val = match state.create_paylink_with_address(&new_paylink).await {
        Ok(val) => val,
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to create paylink: {}", e) })),
            )
                .into_response();
        }
    };

    let paylink_id = paylink_val["paylinkId"].as_str().unwrap_or("").to_string();

    let response = crate::models::CreatePaylinkResponse {
        paylink_id,
        stealth_address,
        ephemeral_pubkey_hex,
    };

    (axum::http::StatusCode::CREATED, Json(response)).into_response()
}

async fn get_paylink(
    State(state): State<Arc<ConvexRepository>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.get_paylink(&id).await {
        Ok(Some(paylink)) => (axum::http::StatusCode::OK, Json(paylink)).into_response(),
        Ok(None) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({ "error": "Paylink not found" })),
        )
            .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to get paylink: {}", e) })),
        )
            .into_response(),
    }
}

#[derive(serde::Deserialize)]
struct ConsolidateRequest {
    deposit_id: String,
}

async fn consolidate_funds(
    State(state): State<Arc<ConvexRepository>>,
    Json(payload): Json<ConsolidateRequest>,
) -> impl IntoResponse {
    match state.create_sweep_job(&payload.deposit_id).await {
        Ok(job_id) => (
            axum::http::StatusCode::ACCEPTED,
            Json(json!({ "status": "queued", "job_id": job_id })),
        )
            .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to create sweep job: {}", e) })),
        )
            .into_response(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  ZK-Mixer Relayer Endpoint
// ─────────────────────────────────────────────────────────────────────────────

/// POST /api/v1/withdraw
///
/// The Relayer endpoint for anonymous withdrawals from the PrivacyPool.
/// The receiver submits their secret note (secret + nullifier) and their
/// destination wallet address. The Rust backend uses its own relayer wallet
/// to pay gas and call PrivacyPool.withdraw() on-chain.
///
/// This breaks the on-chain link: the receiver's main wallet never had to
/// interact with the stealth address or the PrivacyPool directly.
async fn relay_withdraw(
    State(_state): State<Arc<ConvexRepository>>,
    Json(payload): Json<crate::models::WithdrawRequest>,
) -> impl IntoResponse {
    // ── Validate & parse inputs ──────────────────────────────────────────
    let secret_hex = payload.secret_hex.trim_start_matches("0x");
    let nullifier_hex = payload.nullifier_hex.trim_start_matches("0x");

    let secret_bytes = match hex::decode(secret_hex) {
        Ok(b) if b.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            arr
        }
        _ => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Invalid secret: must be 32 bytes hex-encoded" })),
            )
                .into_response();
        }
    };

    let nullifier_bytes = match hex::decode(nullifier_hex) {
        Ok(b) if b.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            arr
        }
        _ => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Invalid nullifier: must be 32 bytes hex-encoded" })),
            )
                .into_response();
        }
    };

    let recipient = match Address::from_str(&payload.recipient_address) {
        Ok(addr) => addr,
        Err(_) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Invalid recipient_address" })),
            )
                .into_response();
        }
    };

    // ── Load relayer config ──────────────────────────────────────────────
    let pool_address_str = match std::env::var("PRIVACY_POOL_ADDRESS") {
        Ok(addr) => addr,
        Err(_) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "PRIVACY_POOL_ADDRESS not configured" })),
            )
                .into_response();
        }
    };

    let pool_address = match Address::from_str(pool_address_str.trim()) {
        Ok(addr) => addr,
        Err(_) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Invalid PRIVACY_POOL_ADDRESS" })),
            )
                .into_response();
        }
    };

    let relayer_key_str = match std::env::var("RELAYER_PRIVATE_KEY") {
        Ok(key) => key,
        Err(_) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "RELAYER_PRIVATE_KEY not configured" })),
            )
                .into_response();
        }
    };

    let rpc_url =
        std::env::var("BASE_RPC_URL").unwrap_or_else(|_| "https://sepolia.base.org".to_string());

    let chain_id: u64 = std::env::var("BASE_CHAIN_ID")
        .unwrap_or_else(|_| "84532".to_string())
        .parse()
        .unwrap_or(84532);

    // ── Build provider and wallet ────────────────────────────────────────
    let provider = match Provider::<Http>::try_from(rpc_url.as_str()) {
        Ok(p) => Arc::new(p),
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to connect to RPC: {}", e) })),
            )
                .into_response();
        }
    };

    let relayer_key_hex = relayer_key_str.trim().trim_start_matches("0x");
    let relayer_wallet = match relayer_key_hex
        .parse::<LocalWallet>()
        .map(|w| w.with_chain_id(chain_id))
    {
        Ok(w) => w,
        Err(e) => {
            tracing::error!("Invalid RELAYER_PRIVATE_KEY: {:?}", e);
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Invalid relayer key configuration" })),
            )
                .into_response();
        }
    };

    tracing::info!(
        "🔄 Relaying withdrawal: recipient={:?}, pool={:?}",
        recipient,
        pool_address
    );

    // ── Execute the on-chain withdrawal ──────────────────────────────────
    match privacy_pool::execute_pool_withdraw(
        provider,
        relayer_wallet,
        pool_address,
        &secret_bytes,
        &nullifier_bytes,
        recipient,
        chain_id,
    )
    .await
    {
        Ok(tx_hash) => {
            let response = crate::models::WithdrawResponse {
                status: "submitted".to_string(),
                tx_hash: format!("{:#x}", tx_hash),
                recipient: format!("{:#x}", recipient),
            };
            (axum::http::StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            tracing::error!("❌ Withdrawal relay failed: {:?}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Withdrawal failed: {}", e) })),
            )
                .into_response()
        }
    }
}

/// Query parameters for the deposit status endpoint.
#[derive(serde::Deserialize)]
struct DepositStatusQuery {
    #[serde(rename = "txHash")]
    tx_hash: Option<String>,
    #[serde(rename = "stealthAddress")]
    stealth_address: Option<String>,
}

/// GET /api/v1/deposit/status?txHash=0x...&stealthAddress=0x...
///
/// Returns the deposit record, sweep status, and privacy note for tracking.
async fn deposit_status(
    State(state): State<Arc<ConvexRepository>>,
    Query(params): Query<DepositStatusQuery>,
) -> impl IntoResponse {
    // Try by txHash first
    if let Some(ref tx_hash) = params.tx_hash {
        match state.get_deposits_by_tx_hash(tx_hash).await {
            Ok(deposits) => {
                if let Some(deposit) = deposits.as_array().and_then(|a| a.first()) {
                    let deposit_id = deposit["depositId"].as_str().unwrap_or("");

                    // Also fetch privacy note
                    let note = state
                        .get_privacy_note(deposit_id)
                        .await
                        .ok()
                        .flatten();

                    // Fetch sweep job status
                    let sweep_status = deposit.get("sweepStatus")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    return (
                        axum::http::StatusCode::OK,
                        Json(json!({
                            "deposit": deposit,
                            "sweepStatus": sweep_status,
                            "note": note,
                        })),
                    )
                        .into_response();
                }
            }
            Err(e) => {
                tracing::warn!("Failed to query deposit by tx hash: {:?}", e);
            }
        }
    }

    // Try by stealth address — look up the ephemeral address match
    if let Some(ref addr) = params.stealth_address {
        match state.get_ephemeral_address_match(84532, addr).await {
            Ok(Some(matched)) => {
                return (
                    axum::http::StatusCode::OK,
                    Json(json!({
                        "matched": matched,
                        "status": "found_address",
                    })),
                )
                    .into_response();
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!("Failed to query ephemeral address: {:?}", e);
            }
        }
    }

    (
        axum::http::StatusCode::NOT_FOUND,
        Json(json!({ "error": "No deposit found for the given query" })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
