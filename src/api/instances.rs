use axum::{
    extract::{Path, Query, State},
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
use crate::engine::VariableInput;
use crate::error::Result;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct StartInstanceRequest {
    pub org_id: Uuid,
    pub definition_id: Uuid,
    pub labels: Option<JsonValue>,
    pub variables: Option<Vec<VariableInput>>,
}

#[derive(Debug, Deserialize)]
pub struct ListInstancesQuery {
    pub org_id: Uuid,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/process-instances", get(list_instances))
        .route("/api/v1/process-instances", post(start_instance))
        .route("/api/v1/process-instances/{id}", get(get_instance))
}

async fn list_instances(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListInstancesQuery>,
) -> Result<Json<Vec<ProcessInstance>>> {
    let instances = process_instances::list_by_org(&state.pool, params.org_id).await?;
    Ok(Json(instances))
}

async fn start_instance(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StartInstanceRequest>,
) -> Result<(StatusCode, Json<ProcessInstance>)> {
    let labels = req.labels.unwrap_or_else(|| serde_json::json!({}));
    let variables = req.variables.unwrap_or_default();
    let instance = state
        .engine
        .start_instance(req.definition_id, req.org_id, &labels, &variables)
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
