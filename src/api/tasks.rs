use super::extractors::{Json, Path, Query};
use super::pagination::{with_total, Page};
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{Permission, Principal};
use crate::db::models::Task;
use crate::db::{process_groups, process_instances, tasks};
use crate::engine::VariableInput;
use crate::error::{EngineError, Result};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct TaskListResponse {
    pub items: Vec<Task>,
}

#[derive(Debug, Deserialize)]
pub struct ListTasksQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/orgs/{org_id}/tasks", get(list_tasks))
        .route("/api/v1/orgs/{org_id}/tasks/{id}", get(get_task))
        .route(
            "/api/v1/orgs/{org_id}/tasks/{id}/complete",
            post(complete_task),
        )
}

#[tracing::instrument(skip_all, fields(org_id = %org_id))]
async fn list_tasks(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(org_id): Path<Uuid>,
    Query(params): Query<ListTasksQuery>,
) -> Result<axum::response::Response> {
    let page = Page::from_query(params.limit, params.offset);
    let (items, total) = match principal.pg_ids_with(Permission::TaskRead) {
        None => tasks::list_pending_paginated(&state.pool, org_id, page.limit, page.offset).await?,
        Some(pgs) => {
            let pgs: Vec<Uuid> = pgs.into_iter().collect();
            tasks::list_pending_paginated_in_pgs(&state.pool, org_id, &pgs, page.limit, page.offset)
                .await?
        }
    };
    Ok(with_total(TaskListResponse { items }, total))
}

#[tracing::instrument(skip_all, fields(org_id = %org_id, id = %id))]
async fn get_task(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((org_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Task>> {
    let task = fetch_task_in_org(&state, id, org_id).await?;
    let pg = process_groups::pg_for_task(&state.pool, id).await?;
    principal.require_in_pg(Permission::TaskRead, pg)?;
    Ok(Json(task))
}

#[derive(Debug, Deserialize)]
struct CompleteTaskRequest {
    variables: Option<Vec<VariableInput>>,
}

#[tracing::instrument(skip_all, fields(org_id = %org_id, id = %id))]
async fn complete_task(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((org_id, id)): Path<(Uuid, Uuid)>,
    body: Option<Json<CompleteTaskRequest>>,
) -> Result<StatusCode> {
    fetch_task_in_org(&state, id, org_id).await?;
    let pg = process_groups::pg_for_task(&state.pool, id).await?;
    principal.require_in_pg(Permission::TaskComplete, pg)?;
    let vars = body.and_then(|b| b.0.variables).unwrap_or_default();
    state.engine.complete_user_task(id, &vars).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Tenant-isolation guard. Returns NotFound if the task doesn't exist OR
/// belongs to another org — the two cases are indistinguishable to the
/// caller by design.
async fn fetch_task_in_org(state: &Arc<AppState>, id: Uuid, org_id: Uuid) -> Result<Task> {
    let task = tasks::get_by_id(&state.pool, id).await?;
    let instance = process_instances::get_by_id(&state.pool, task.instance_id).await?;
    if instance.org_id != org_id {
        return Err(EngineError::NotFound(format!("task {id}")));
    }
    Ok(task)
}
