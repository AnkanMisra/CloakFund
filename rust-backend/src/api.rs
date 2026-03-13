use axum::{Json, Router, http::HeaderValue, response::IntoResponse, routing::get};
use serde_json::json;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

pub fn create_router() -> Router {
    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

    let cors = CorsLayer::new()
        .allow_origin(frontend_url.parse::<HeaderValue>().unwrap())
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers(tower_http::cors::Any);

    Router::new()
        .route("/health", get(health_check))
        .layer(cors)
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
