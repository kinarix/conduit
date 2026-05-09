use super::extractors::Json;
use axum::{extract::State, http::StatusCode, routing::post, Router};
use serde::Deserialize;
use std::sync::Arc;

use crate::auth::Principal;
use crate::engine::VariableInput;
use crate::error::Result;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CorrelateMessageRequest {
    pub message_name: String,
    pub correlation_key: Option<String>,
    pub variables: Option<Vec<VariableInput>>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/messages/correlate", post(correlate_message))
}

#[tracing::instrument(skip_all, fields(org_id = %principal.org_id, message_name = %req.message_name, correlation_key = ?req.correlation_key))]
async fn correlate_message(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(req): Json<CorrelateMessageRequest>,
) -> Result<StatusCode> {
    let variables = req.variables.unwrap_or_default();
    state
        .engine
        .correlate_message(
            &req.message_name,
            req.correlation_key.as_deref(),
            &variables,
            principal.org_id,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
