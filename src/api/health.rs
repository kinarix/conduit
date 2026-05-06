use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;

use super::extractors::Json;
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
}

async fn health_check(State(state): State<Arc<AppState>>) -> Json<Value> {
    let db_ok = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();
    let uptime_secs = state.started_at.elapsed().as_secs();

    Json(json!({
        "status":      if db_ok { "ok" } else { "degraded" },
        "database":    if db_ok { "connected" } else { "disconnected" },
        "version":     env!("CARGO_PKG_VERSION"),
        "uptime_secs": uptime_secs,
    }))
}

async fn metrics_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match &state.prometheus_handle {
        Some(handle) => (
            StatusCode::OK,
            [(
                header::CONTENT_TYPE,
                "text/plain; version=0.0.4; charset=utf-8",
            )],
            handle.render(),
        )
            .into_response(),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::CONTENT_TYPE, "text/plain")],
            "Metrics not available".to_string(),
        )
            .into_response(),
    }
}
