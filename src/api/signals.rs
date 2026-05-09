use super::extractors::Json;
use axum::{extract::State, http::StatusCode, routing::post, Router};
use serde::Deserialize;
use std::sync::Arc;

use crate::auth::Principal;
use crate::engine::VariableInput;
use crate::error::Result;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct BroadcastSignalRequest {
    pub signal_name: String,
    pub variables: Option<Vec<VariableInput>>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/signals/broadcast", post(broadcast_signal))
}

#[tracing::instrument(skip_all, fields(org_id = %principal.org_id, signal_name = %req.signal_name))]
async fn broadcast_signal(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(req): Json<BroadcastSignalRequest>,
) -> Result<StatusCode> {
    let variables = req.variables.unwrap_or_default();
    state
        .engine
        .broadcast_signal(&req.signal_name, &variables, principal.org_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
