use axum::{extract::State, http::StatusCode, routing::post, Router};
use super::extractors::Json;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::engine::VariableInput;
use crate::error::Result;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CorrelateMessageRequest {
    pub org_id: Uuid,
    pub message_name: String,
    pub correlation_key: Option<String>,
    pub variables: Option<Vec<VariableInput>>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/messages/correlate", post(correlate_message))
}

async fn correlate_message(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CorrelateMessageRequest>,
) -> Result<StatusCode> {
    let variables = req.variables.unwrap_or_default();
    state
        .engine
        .correlate_message(
            &req.message_name,
            req.correlation_key.as_deref(),
            &variables,
            req.org_id,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
