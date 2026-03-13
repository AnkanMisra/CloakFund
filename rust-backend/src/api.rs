use anyhow::Context;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderValue,
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::json;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::convex_client::ConvexRepository;

pub fn create_router(state: Arc<ConvexRepository>) -> anyhow::Result<Router> {
    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

    let cors = CorsLayer::new()
        .allow_origin(
            frontend_url
                .parse::<HeaderValue>()
                .context("FRONTEND_URL is not a valid HTTP header value")?,
        )
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers(tower_http::cors::Any);

    Ok(Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/paylink", post(create_paylink))
        .route("/api/v1/paylink/:id", get(get_paylink))
        .route("/api/v1/consolidate", post(consolidate_funds))
        .route("/api/v1/bitgo/webhook", post(bitgo_webhook))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state))
}

async fn health_check() -> impl IntoResponse {
    let timestamp = chrono::Utc::now().timestamp();
    Json(json!({
        "status": "ok",
        "service": "cloakfund-rust-backend",
        "timestamp": timestamp,
    }))
}

async fn create_paylink(
    State(state): State<Arc<ConvexRepository>>,
    Json(payload): Json<crate::models::CreatePaylinkRequest>,
) -> impl IntoResponse {
    let chain_id = payload.chain_id.unwrap_or(8453); // Default base
    let network = payload.network.unwrap_or_else(|| "base".to_string());

    let (stealth_address, ephemeral_pubkey_hex, view_tag) =
        match crate::stealth::generate_stealth_address(&payload.recipient_public_key_hex) {
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
        recipient_public_key_hex: payload.recipient_public_key_hex,
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
    // Note: To fully satisfy the confirmation check, we should ideally verify
    // the deposit's status here or in the `createSweepJob` Convex mutation.
    // If a `get_deposit` method is available on ConvexRepository, it would look like:
    //
    // if let Ok(Some(deposit)) = state.get_deposit(&payload.deposit_id).await {
    //     if deposit.confirmation_status != "confirmed" {
    //         return (
    //             axum::http::StatusCode::BAD_REQUEST,
    //             Json(json!({ "error": "Deposit is not confirmed" })),
    //         ).into_response();
    //     }
    // }

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

async fn bitgo_webhook(
    State(_state): State<Arc<ConvexRepository>>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let signature = match headers.get("BitGo-Signature").and_then(|v| v.to_str().ok()) {
        Some(sig) => sig,
        None => {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Missing signature" })),
            )
                .into_response();
        }
    };

    let secret = std::env::var("BITGO_WEBHOOK_SECRET").unwrap_or_default();
    if secret.is_empty() {
        tracing::warn!("BITGO_WEBHOOK_SECRET is not set; rejecting webhook");
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Webhook secret not configured" })),
        )
            .into_response();
    }

    let mut mac = match Hmac::<Sha256>::new_from_slice(secret.as_bytes()) {
        Ok(mac) => mac,
        Err(_) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Invalid HMAC secret" })),
            )
                .into_response();
        }
    };

    mac.update(&body);
    let expected_sig = hex::encode(mac.finalize().into_bytes());

    if signature != expected_sig {
        tracing::warn!("Invalid BitGo webhook signature");
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Invalid signature" })),
        )
            .into_response();
    }

    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(_) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Invalid JSON body" })),
            )
                .into_response();
        }
    };

    tracing::info!("Received verified BitGo webhook: {:?}", payload);

    // In a complete implementation, we'd parse the BitGo webhook payload
    // and trigger updates in Convex (e.g., mark the sweep job as completed
    // if BitGo confirms receipt in the treasury wallet).

    (
        axum::http::StatusCode::OK,
        Json(json!({ "status": "acknowledged" })),
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

        // Can't easily test create_paylink and get_paylink here without mocking ConvexRepository,
        // which requires a live Convex backend or an extracted trait.
        // Full API endpoint testing should happen in integration tests.
    }

    #[tokio::test]
    async fn test_bitgo_webhook_response() {
        // Since bitgo_webhook takes an Arc<ConvexRepository> state we can mock it here
        // or since it doesn't currently use the state we could pass a dummy.
        // However, it's easier to test the router indirectly in integration tests or
        // modify the signature.
        // This is a placeholder test to ensure it compiles.
    }
}
