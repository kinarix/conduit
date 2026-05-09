use super::extractors::{Json, Path, Query};
use super::pagination::{with_total, Page};
use axum::{
    extract::State,
    http::StatusCode,
    routing::{delete, get, post, put},
    Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{Permission, Principal};
use crate::db::models::ProcessGroup;
use crate::db::process_groups;
use crate::error::{EngineError, Result};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListProcessGroupsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProcessGroupRequest {
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

#[tracing::instrument(skip_all, fields(org_id = %principal.org_id))]
async fn list_process_groups(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Query(q): Query<ListProcessGroupsQuery>,
) -> Result<axum::response::Response> {
    let page = Page::from_query(q.limit, q.offset);
    let (rows, total) =
        process_groups::list_paginated(&state.pool, principal.org_id, page.limit, page.offset)
            .await?;
    Ok(with_total(rows, total))
}

#[tracing::instrument(skip_all, fields(org_id = %principal.org_id))]
async fn create_process_group(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(req): Json<CreateProcessGroupRequest>,
) -> Result<(StatusCode, Json<ProcessGroup>)> {
    principal.require(Permission::ProcessDeploy)?;
    if req.name.trim().is_empty() {
        return Err(EngineError::Validation(
            "name must not be empty".to_string(),
        ));
    }
    let group = process_groups::insert(&state.pool, principal.org_id, req.name.trim()).await?;
    Ok((StatusCode::CREATED, Json(group)))
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn rename_process_group(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(req): Json<RenameProcessGroupRequest>,
) -> Result<Json<ProcessGroup>> {
    principal.require(Permission::ProcessDeploy)?;
    if req.name.trim().is_empty() {
        return Err(EngineError::Validation(
            "name must not be empty".to_string(),
        ));
    }
    ensure_group_in_org(&state, id, principal.org_id).await?;
    let group = process_groups::rename(&state.pool, id, req.name.trim()).await?;
    Ok(Json(group))
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn delete_process_group(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    principal.require(Permission::ProcessDeploy)?;
    ensure_group_in_org(&state, id, principal.org_id).await?;
    process_groups::delete(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(skip_all, fields(definition_id = %id, process_group_id = %req.process_group_id))]
async fn assign_process_group(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(req): Json<AssignProcessGroupRequest>,
) -> Result<StatusCode> {
    principal.require(Permission::ProcessDeploy)?;
    ensure_group_in_org(&state, req.process_group_id, principal.org_id).await?;
    process_groups::assign_definition(&state.pool, id, req.process_group_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Reject if the group exists but belongs to another org. Returns NotFound
/// (not Forbidden) so we don't leak existence across tenants.
async fn ensure_group_in_org(state: &Arc<AppState>, group_id: Uuid, org_id: Uuid) -> Result<()> {
    let group = process_groups::get_by_id(&state.pool, group_id)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("process_group {group_id}")))?;
    if group.org_id != org_id {
        return Err(EngineError::NotFound(format!("process_group {group_id}")));
    }
    Ok(())
}
