use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::db::models::ProcessGroup;
use crate::db::process_groups;
use crate::error::{EngineError, Result};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListProcessGroupsQuery {
    pub org_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct CreateProcessGroupRequest {
    pub org_id: Uuid,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct RenameProcessGroupRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct AssignProcessGroupRequest {
    pub process_group_id: Uuid,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/process-groups", get(list_process_groups))
        .route("/api/v1/process-groups", post(create_process_group))
        .route("/api/v1/process-groups/{id}", put(rename_process_group))
        .route("/api/v1/process-groups/{id}", delete(delete_process_group))
        .route(
            "/api/v1/deployments/{id}/process-group",
            put(assign_process_group),
        )
}

async fn list_process_groups(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListProcessGroupsQuery>,
) -> Result<Json<Vec<ProcessGroup>>> {
    let groups = process_groups::list_by_org(&state.pool, q.org_id).await?;
    Ok(Json(groups))
}

async fn create_process_group(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateProcessGroupRequest>,
) -> Result<(StatusCode, Json<ProcessGroup>)> {
    if req.name.trim().is_empty() {
        return Err(EngineError::Validation(
            "name must not be empty".to_string(),
        ));
    }
    let group = process_groups::insert(&state.pool, req.org_id, req.name.trim()).await?;
    Ok((StatusCode::CREATED, Json(group)))
}

async fn rename_process_group(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<RenameProcessGroupRequest>,
) -> Result<Json<ProcessGroup>> {
    if req.name.trim().is_empty() {
        return Err(EngineError::Validation(
            "name must not be empty".to_string(),
        ));
    }
    let group = process_groups::rename(&state.pool, id, req.name.trim()).await?;
    Ok(Json(group))
}

async fn delete_process_group(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    process_groups::delete(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn assign_process_group(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<AssignProcessGroupRequest>,
) -> Result<StatusCode> {
    process_groups::assign_definition(&state.pool, id, req.process_group_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
