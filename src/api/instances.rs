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

use super::pagination::{with_total, Page};
use crate::auth::{Permission, Principal};
use crate::db::execution_history;
use crate::db::jobs;
use crate::db::models::{ExecutionHistory, Job, ProcessEvent, ProcessInstance};
use crate::db::process_definitions;
use crate::db::process_events;
use crate::db::process_instances;
use crate::engine::VariableInput;
use crate::error::{EngineError, Result};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct StartInstanceRequest {
    pub definition_id: Uuid,
    pub labels: Option<JsonValue>,
    pub variables: Option<Vec<VariableInput>>,
}

#[derive(Debug, Deserialize)]
pub struct ListInstancesQuery {
    pub definition_id: Option<Uuid>,
    pub process_key: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
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

#[tracing::instrument(skip_all, fields(org_id = %principal.org_id))]
async fn list_instances(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Query(params): Query<ListInstancesQuery>,
) -> Result<axum::response::Response> {
    let page = Page::from_query(params.limit, params.offset);
    let (instances, total) = process_instances::list_paginated(
        &state.pool,
        principal.org_id,
        params.definition_id,
        params.process_key.as_deref(),
        page.limit,
        page.offset,
    )
    .await?;
    Ok(with_total(instances, total))
}

#[tracing::instrument(skip_all, fields(org_id = %principal.org_id, definition_id = %req.definition_id))]
async fn start_instance(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(req): Json<StartInstanceRequest>,
) -> Result<(StatusCode, Json<ProcessInstance>)> {
    principal.require(Permission::InstanceStart)?;
    // The definition's org is authoritative. Cross-org starts are denied.
    let def = process_definitions::get_by_id(&state.pool, req.definition_id).await?;
    if def.org_id != principal.org_id {
        return Err(EngineError::NotFound(format!(
            "process_definition {}",
            req.definition_id
        )));
    }
    let labels = req.labels.unwrap_or_else(|| serde_json::json!({}));
    let variables = req.variables.unwrap_or_default();
    let instance = state
        .engine
        .start_instance(req.definition_id, principal.org_id, &labels, &variables)
        .await?;
    Ok((StatusCode::CREATED, Json(instance)))
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn get_instance(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<ProcessInstance>> {
    let instance = fetch_instance_in_org(&state, id, principal.org_id).await?;
    Ok(Json(instance))
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn pause_instance(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<ProcessInstance>> {
    principal.require(Permission::InstanceCancel)?;
    fetch_instance_in_org(&state, id, principal.org_id).await?;
    let inst = process_instances::pause(&state.pool, id).await?;
    Ok(Json(inst))
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn resume_instance(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<ProcessInstance>> {
    principal.require(Permission::InstanceCancel)?;
    fetch_instance_in_org(&state, id, principal.org_id).await?;
    let inst = process_instances::resume(&state.pool, id).await?;
    Ok(Json(inst))
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn cancel_instance(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<ProcessInstance>> {
    principal.require(Permission::InstanceCancel)?;
    fetch_instance_in_org(&state, id, principal.org_id).await?;
    let inst = process_instances::cancel(&state.pool, id).await?;
    Ok(Json(inst))
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn delete_instance(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    principal.require(Permission::InstanceCancel)?;
    fetch_instance_in_org(&state, id, principal.org_id).await?;
    process_instances::delete(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn list_history(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ExecutionHistory>>> {
    fetch_instance_in_org(&state, id, principal.org_id).await?;
    let rows = execution_history::list_by_instance(&state.pool, id).await?;
    Ok(Json(rows))
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn list_events(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ProcessEvent>>> {
    fetch_instance_in_org(&state, id, principal.org_id).await?;
    let rows = process_events::list_by_instance(&state.pool, id).await?;
    Ok(Json(rows))
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn list_jobs(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Job>>> {
    fetch_instance_in_org(&state, id, principal.org_id).await?;
    let rows = jobs::list_by_instance(&state.pool, id).await?;
    Ok(Json(rows))
}

/// Tenant-isolation guard. Returns NotFound (not Forbidden) so we don't
/// leak whether an instance with that ID exists in another org.
async fn fetch_instance_in_org(
    state: &Arc<AppState>,
    id: Uuid,
    org_id: Uuid,
) -> Result<ProcessInstance> {
    let instance = process_instances::get_by_id(&state.pool, id).await?;
    if instance.org_id != org_id {
        return Err(EngineError::NotFound(format!("process_instance {id}")));
    }
    Ok(instance)
}
