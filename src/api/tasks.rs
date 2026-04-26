use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::db::models::Task;
use crate::db::tasks;
use crate::error::Result;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct TaskListResponse {
    pub items: Vec<Task>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/tasks", get(list_tasks))
        .route("/api/v1/tasks/{id}", get(get_task))
        .route("/api/v1/tasks/{id}/complete", post(complete_task))
}

async fn list_tasks(State(state): State<Arc<AppState>>) -> Result<Json<TaskListResponse>> {
    let items = tasks::list_pending(&state.pool).await?;
    Ok(Json(TaskListResponse { items }))
}

async fn get_task(State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> Result<Json<Task>> {
    let task = tasks::get_by_id(&state.pool, id).await?;
    Ok(Json(task))
}

async fn complete_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    state.engine.complete_user_task(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
