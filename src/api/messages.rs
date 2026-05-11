use super::extractors::{Json, Path};
use axum::{extract::State, http::StatusCode, routing::post, Router};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{Permission, Principal};
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
    Router::new().route(
        "/api/v1/orgs/{org_id}/messages/correlate",
        post(correlate_message),
    )
}

#[tracing::instrument(skip_all, fields(org_id = %org_id, message_name = %req.message_name, correlation_key = ?req.correlation_key))]
async fn correlate_message(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(org_id): Path<Uuid>,
    Json(req): Json<CorrelateMessageRequest>,
) -> Result<StatusCode> {
    principal.require(Permission::MessageCorrelate)?;
    let variables = req.variables.unwrap_or_default();
    state
        .engine
        .correlate_message(
            &req.message_name,
            req.correlation_key.as_deref(),
            &variables,
            org_id,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
