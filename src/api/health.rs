use axum::{extract::State, routing::get, Router};
use serde_json::{json, Value};
use std::sync::Arc;

use super::extractors::Json;
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/health", get(health_check))
}

async fn health_check(State(state): State<Arc<AppState>>) -> Json<Value> {
    let db_ok = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();

    Json(json!({
        "status":   if db_ok { "ok" } else { "degraded" },
        "database": if db_ok { "connected" } else { "disconnected" },
        "version":  env!("CARGO_PKG_VERSION"),
    }))
}
