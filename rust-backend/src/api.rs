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
use crate::tokens;

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
        .route("/api/v1/paylink/:id/revoke", post(revoke_paylink))
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

    // Resolve the absolute expiry (Unix ms). `expires_at` wins over the
    // convenience `expires_in_seconds` when both are supplied.
    let expires_at: Option<u64> = payload.expires_at.or_else(|| {
        payload.expires_in_seconds.map(|secs| {
            let now_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
            now_ms.saturating_add(secs.saturating_mul(1000))
        })
    });

    // Always issue a revocation token — cheap to generate, and the alternative
    // (no token) strands the paylink with no way to shut it off.
    let revocation_token = tokens::generate_revocation_token();
    let revocation_token_hash = match tokens::hash_revocation_token(&revocation_token) {
        Ok(h) => h,
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to hash revocation token: {}", e) })),
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
        expires_at,
        revocation_token_hash: Some(revocation_token_hash),
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
        revocation_token: Some(revocation_token),
        expires_at,
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
        Err(e) => {
            let raw = e.to_string();
            if is_convex_id_validation_error(&raw) {
                tracing::warn!("get_paylink rejected malformed id: {}", raw);
                (
                    axum::http::StatusCode::BAD_REQUEST,
                    Json(json!({ "error": "Invalid paylink id" })),
                )
                    .into_response()
            } else {
                tracing::error!("get_paylink backend error: {}", raw);
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "Internal server error" })),
                )
                    .into_response()
            }
        }
    }
}

/// Detects backend errors that stem from the caller supplying a malformed
/// document ID (or any other value the mutation's validator rejects), so we
/// can return `400 Bad Request` instead of masking the client error as `500`.
///
/// The underlying platform's Rust client surfaces validator failures as plain
/// messages; the exact wording drifts across versions but always contains one
/// of these well-known fragments. We match conservatively — unknown errors
/// fall through to the generic 500 path.
fn is_convex_id_validation_error(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("argumentvalidationerror")
        || lower.contains("value does not match validator")
        || lower.contains("invalid id")
        || lower.contains("id does not match")
        || lower.contains("invalid argument")
}

/// Whitelisted client-visible outcomes for the revoke endpoint.
///
/// Mapping backend error strings to a small closed enum means clients only
/// ever see messages we've intentionally exposed — no raw platform errors,
/// no "Convex"-style prefixes, and no wording drift leaking implementation
/// details. Unknown errors collapse to `ServerError` and are logged raw
/// server-side for debugging.
enum RevokeOutcome {
    NotFound,
    InvalidToken,
    AlreadyRevoked,
    BadRequest,
    ServerError,
}

impl RevokeOutcome {
    fn status(&self) -> axum::http::StatusCode {
        match self {
            Self::NotFound => axum::http::StatusCode::NOT_FOUND,
            Self::InvalidToken => axum::http::StatusCode::FORBIDDEN,
            Self::AlreadyRevoked => axum::http::StatusCode::CONFLICT,
            Self::BadRequest => axum::http::StatusCode::BAD_REQUEST,
            Self::ServerError => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn message(&self) -> &'static str {
        match self {
            Self::NotFound => "Paylink not found",
            Self::InvalidToken => "Invalid revocation token",
            Self::AlreadyRevoked => "Paylink is already revoked",
            Self::BadRequest => "Invalid paylink id",
            Self::ServerError => "Internal server error",
        }
    }
}

fn classify_revoke_error(msg: &str) -> RevokeOutcome {
    // Order matters: check for validator errors first so a malformed id
    // doesn't accidentally match the "Paylink not found" substring in a
    // wrapped error chain.
    if is_convex_id_validation_error(msg) {
        RevokeOutcome::BadRequest
    } else if msg.contains("Paylink not found") {
        RevokeOutcome::NotFound
    } else if msg.contains("already revoked") {
        RevokeOutcome::AlreadyRevoked
    } else if msg.contains("Invalid revocation token") {
        RevokeOutcome::InvalidToken
    } else {
        RevokeOutcome::ServerError
    }
}

async fn revoke_paylink(
    State(state): State<Arc<ConvexRepository>>,
    Path(id): Path<String>,
    Json(payload): Json<crate::models::RevokePaylinkRequest>,
) -> impl IntoResponse {
    let hash = match tokens::hash_revocation_token(&payload.revocation_token) {
        Ok(h) => h,
        Err(_) => {
            // Deliberately do not echo the parser error back — it can include
            // the offending input. "Invalid revocation token" is enough.
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Invalid revocation token" })),
            )
                .into_response();
        }
    };

    match state.revoke_paylink(&id, &hash).await {
        Ok(()) => {
            let resp = crate::models::RevokePaylinkResponse {
                status: "revoked".to_string(),
            };
            (axum::http::StatusCode::OK, Json(resp)).into_response()
        }
        Err(e) => {
            let raw = e.to_string();
            let outcome = classify_revoke_error(&raw);
            if matches!(outcome, RevokeOutcome::ServerError) {
                tracing::error!("revoke_paylink backend error: {}", raw);
            } else {
                tracing::debug!(
                    "revoke_paylink classified: {} / raw={}",
                    outcome.message(),
                    raw
                );
            }
            (
                outcome.status(),
                Json(json!({ "error": outcome.message() })),
            )
                .into_response()
        }
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
                    let note = state.get_privacy_note(deposit_id).await.ok().flatten();

                    // Fetch sweep job status
                    let sweep_status = deposit
                        .get("sweepStatus")
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

    #[test]
    fn convex_id_validator_errors_are_classified_as_client_errors() {
        // Sampled wordings from the Convex Rust client when a caller passes
        // a malformed document id. Matching is case-insensitive and
        // substring-based on purpose — wording drifts across Convex versions.
        let samples = [
            "ArgumentValidationError: Value does not match validator",
            "Convex logic error: Invalid argument `paylinkId`: Id does not match",
            "Invalid id: foo",
            "invalid ID",
            "ARGUMENTVALIDATIONERROR at deposit time",
        ];
        for s in samples {
            assert!(
                is_convex_id_validation_error(s),
                "expected to classify as 400: {s}",
            );
        }
    }

    #[test]
    fn convex_logical_errors_are_not_classified_as_client_errors() {
        // Domain errors that should keep their specific 403/404/409 mapping
        // and fall through to the existing substring matches. None of these
        // should be misclassified as a 400.
        let samples = [
            "Paylink not found",
            "Invalid revocation token",
            "Paylink is already revoked",
            "websocket disconnected",
            "timed out",
        ];
        for s in samples {
            assert!(
                !is_convex_id_validation_error(s),
                "must not classify as 400: {s}",
            );
        }
    }

    #[test]
    fn classify_revoke_error_maps_known_wordings() {
        assert!(matches!(
            classify_revoke_error("Paylink not found"),
            RevokeOutcome::NotFound
        ));
        assert!(matches!(
            classify_revoke_error("Invalid revocation token"),
            RevokeOutcome::InvalidToken
        ));
        assert!(matches!(
            classify_revoke_error("Paylink is already revoked"),
            RevokeOutcome::AlreadyRevoked
        ));
        assert!(matches!(
            classify_revoke_error("ArgumentValidationError: Value does not match validator"),
            RevokeOutcome::BadRequest
        ));
    }

    #[test]
    fn classify_revoke_error_falls_back_to_server_error_for_unknown() {
        assert!(matches!(
            classify_revoke_error("something unexpected happened"),
            RevokeOutcome::ServerError
        ));
        assert!(matches!(
            classify_revoke_error(""),
            RevokeOutcome::ServerError
        ));
    }

    #[test]
    fn revoke_outcome_messages_are_sanitized_with_no_tech_leakage() {
        // Guardrail: the strings we ever send to clients must not mention
        // the underlying backend platform or include the substring "error:"
        // (which would suggest a leaked wrapper prefix).
        let outcomes = [
            RevokeOutcome::NotFound,
            RevokeOutcome::InvalidToken,
            RevokeOutcome::AlreadyRevoked,
            RevokeOutcome::BadRequest,
            RevokeOutcome::ServerError,
        ];
        for o in &outcomes {
            let m = o.message().to_lowercase();
            assert!(!m.contains("convex"), "leaked backend name: {m}");
            assert!(!m.contains("argumentvalidation"), "leaked raw error: {m}");
            assert!(!m.contains("error:"), "looks like a leaked prefix: {m}");
        }
    }
}
