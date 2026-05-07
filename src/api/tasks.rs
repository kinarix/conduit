use super::extractors::{Json, Path};
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::db::models::Task;
use crate::db::tasks;
use crate::engine::VariableInput;
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

#[derive(Debug, Deserialize)]
struct CompleteTaskRequest {
    variables: Option<Vec<VariableInput>>,
}

async fn complete_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    body: Option<Json<CompleteTaskRequest>>,
) -> Result<StatusCode> {
    let vars = body.and_then(|b| b.0.variables).unwrap_or_default();
    state.engine.complete_user_task(id, &vars).await?;
    Ok(StatusCode::NO_CONTENT)
}
