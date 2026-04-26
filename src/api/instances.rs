use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use uuid::Uuid;

use crate::db::models::ProcessInstance;
use crate::db::process_instances;
use crate::error::Result;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct StartInstanceRequest {
    pub org_id: Uuid,
    pub definition_id: Uuid,
    pub labels: Option<JsonValue>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/process-instances", post(start_instance))
        .route("/api/v1/process-instances/{id}", get(get_instance))
}

async fn start_instance(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StartInstanceRequest>,
) -> Result<(StatusCode, Json<ProcessInstance>)> {
    let labels = req.labels.unwrap_or_else(|| serde_json::json!({}));
    let instance = state
        .engine
        .start_instance(req.definition_id, req.org_id, &labels)
        .await?;
    Ok((StatusCode::CREATED, Json(instance)))
}

async fn get_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProcessInstance>> {
    let instance = process_instances::get_by_id(&state.pool, id).await?;
    Ok(Json(instance))
}
