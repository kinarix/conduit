use super::extractors::{Json, Path, Query};
use axum::{
    extract::State,
    http::StatusCode,
    routing::{delete, get, post},
    Router,
};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use uuid::Uuid;

use crate::db::execution_history;
use crate::db::jobs;
use crate::db::models::{ExecutionHistory, Job, ProcessEvent, ProcessInstance};
use crate::db::process_events;
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
        .route("/api/v1/process-instances/{id}", delete(delete_instance))
        .route("/api/v1/process-instances/{id}/pause", post(pause_instance))
        .route(
            "/api/v1/process-instances/{id}/resume",
            post(resume_instance),
        )
        .route(
            "/api/v1/process-instances/{id}/cancel",
            post(cancel_instance),
        )
        .route("/api/v1/process-instances/{id}/history", get(list_history))
        .route("/api/v1/process-instances/{id}/events", get(list_events))
        .route("/api/v1/process-instances/{id}/jobs", get(list_jobs))
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

async fn pause_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProcessInstance>> {
    let inst = process_instances::pause(&state.pool, id).await?;
    Ok(Json(inst))
}

async fn resume_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProcessInstance>> {
    let inst = process_instances::resume(&state.pool, id).await?;
    Ok(Json(inst))
}

async fn cancel_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProcessInstance>> {
    let inst = process_instances::cancel(&state.pool, id).await?;
    Ok(Json(inst))
}

async fn delete_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    process_instances::delete(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_history(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ExecutionHistory>>> {
    let rows = execution_history::list_by_instance(&state.pool, id).await?;
    Ok(Json(rows))
}

async fn list_events(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ProcessEvent>>> {
    let rows = process_events::list_by_instance(&state.pool, id).await?;
    Ok(Json(rows))
}

async fn list_jobs(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Job>>> {
    let rows = jobs::list_by_instance(&state.pool, id).await?;
    Ok(Json(rows))
}
