use axum::{Json, Router, response::IntoResponse, routing::get};
use serde_json::json;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

pub fn create_router() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

async fn health_check() -> impl IntoResponse {
    let timestamp = chrono::Utc::now().timestamp();
    Json(json!({
        "status": "ok",
        "service": "cloakfund-rust-backend",
        "timestamp": timestamp,
    }))
}
