use super::extractors::{Json, Path, Query};
use super::pagination::{with_total, Page};
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, put},
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
        .route(
            "/api/v1/orgs/{org_id}/process-groups",
            get(list_process_groups).post(create_process_group),
        )
        .route(
            "/api/v1/orgs/{org_id}/process-groups/{id}",
            put(rename_process_group).delete(delete_process_group),
        )
        .route(
            "/api/v1/orgs/{org_id}/deployments/{id}/process-group",
            put(assign_process_group),
        )
}

#[tracing::instrument(skip_all, fields(org_id = %org_id))]
async fn list_process_groups(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(org_id): Path<Uuid>,
    Query(q): Query<ListProcessGroupsQuery>,
) -> Result<axum::response::Response> {
    let page = Page::from_query(q.limit, q.offset);
    let (rows, total) = match principal.pg_ids_with(Permission::ProcessGroupRead) {
        None => {
            process_groups::list_paginated(&state.pool, org_id, page.limit, page.offset).await?
        }
        Some(pgs) => {
            let pgs: Vec<Uuid> = pgs.into_iter().collect();
            process_groups::list_paginated_in_set(
                &state.pool,
                org_id,
                &pgs,
                page.limit,
                page.offset,
            )
            .await?
        }
    };
    Ok(with_total(rows, total))
}

#[tracing::instrument(skip_all, fields(org_id = %org_id))]
async fn create_process_group(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(org_id): Path<Uuid>,
    Json(req): Json<CreateProcessGroupRequest>,
) -> Result<(StatusCode, Json<ProcessGroup>)> {
    principal.require(Permission::ProcessGroupCreate)?;
    if req.name.trim().is_empty() {
        return Err(EngineError::Validation(
            "name must not be empty".to_string(),
        ));
    }
    let group = process_groups::insert(&state.pool, org_id, req.name.trim()).await?;
    Ok((StatusCode::CREATED, Json(group)))
}

#[tracing::instrument(skip_all, fields(org_id = %org_id, id = %id))]
async fn rename_process_group(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((org_id, id)): Path<(Uuid, Uuid)>,
    Json(req): Json<RenameProcessGroupRequest>,
) -> Result<Json<ProcessGroup>> {
    if req.name.trim().is_empty() {
        return Err(EngineError::Validation(
            "name must not be empty".to_string(),
        ));
    }
    ensure_group_in_org(&state, id, org_id).await?;
    principal.require_in_pg(Permission::ProcessGroupUpdate, id)?;
    let group = process_groups::rename(&state.pool, id, req.name.trim()).await?;
    Ok(Json(group))
}

#[tracing::instrument(skip_all, fields(org_id = %org_id, id = %id))]
async fn delete_process_group(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((org_id, id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode> {
    ensure_group_in_org(&state, id, org_id).await?;
    principal.require_in_pg(Permission::ProcessGroupDelete, id)?;
    process_groups::delete(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(skip_all, fields(org_id = %org_id, definition_id = %id, process_group_id = %req.process_group_id))]
async fn assign_process_group(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((org_id, id)): Path<(Uuid, Uuid)>,
    Json(req): Json<AssignProcessGroupRequest>,
) -> Result<StatusCode> {
    // Moving a definition between pgs requires write access to BOTH pgs:
    // you must be able to remove from the old one and add to the new one.
    let old_pg = crate::db::process_groups::pg_for_definition(&state.pool, id).await?;
    ensure_group_in_org(&state, req.process_group_id, org_id).await?;
    principal.require_in_pg(Permission::ProcessUpdate, old_pg)?;
    principal.require_in_pg(Permission::ProcessUpdate, req.process_group_id)?;
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
