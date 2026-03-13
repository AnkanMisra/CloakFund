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

pub fn create_router(state: Arc<ConvexRepository>) -> Router {
    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

    let cors = CorsLayer::new()
        .allow_origin(frontend_url.parse::<HeaderValue>().unwrap())
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers(tower_http::cors::Any);

    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/paylink", post(create_paylink))
        .route("/api/v1/paylink/:id", get(get_paylink))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
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

    let new_paylink = crate::models::NewPaylink {
        user_id: None,
        ens_name: payload.ens_name,
        recipient_public_key_hex: payload.recipient_public_key_hex,
        metadata: payload.metadata,
        chain_id,
        network: network.clone(),
    };

    let paylink_val = match state.create_paylink(&new_paylink).await {
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

    let new_ephem = crate::models::NewEphemeralAddress {
        paylink_id: paylink_id.clone(),
        stealth_address: stealth_address.clone(),
        ephemeral_pubkey_hex: ephemeral_pubkey_hex.clone(),
        view_tag,
        chain_id,
        network,
    };

    if let Err(e) = state.create_ephemeral_address(&new_ephem).await {
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to create ephemeral address: {}", e) })),
        )
            .into_response();
    }

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
