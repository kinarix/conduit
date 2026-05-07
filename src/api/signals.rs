use super::extractors::Json;
use axum::{extract::State, http::StatusCode, routing::post, Router};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::engine::VariableInput;
use crate::error::Result;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct BroadcastSignalRequest {
    pub org_id: Uuid,
    pub signal_name: String,
    pub variables: Option<Vec<VariableInput>>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/signals/broadcast", post(broadcast_signal))
}

async fn broadcast_signal(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BroadcastSignalRequest>,
) -> Result<StatusCode> {
    let variables = req.variables.unwrap_or_default();
    state
        .engine
        .broadcast_signal(&req.signal_name, &variables, req.org_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
