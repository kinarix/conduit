use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use uuid::Uuid;

use crate::db;
use crate::engine::VariableInput;
use crate::error::Result;
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/external-tasks/fetch-and-lock",
            post(fetch_and_lock),
        )
        .route("/api/v1/external-tasks/{id}/complete", post(complete))
        .route("/api/v1/external-tasks/{id}/failure", post(failure))
        .route("/api/v1/external-tasks/{id}/extend-lock", post(extend_lock))
}

#[derive(Debug, Deserialize)]
struct FetchAndLockRequest {
    worker_id: String,
    topic: Option<String>,
    max_jobs: Option<i64>,
    lock_duration_secs: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct VariableDto {
    name: String,
    value_type: String,
    value: JsonValue,
}

#[derive(Debug, Serialize)]
pub struct ExternalTaskDto {
    pub id: Uuid,
    pub topic: Option<String>,
    pub instance_id: Uuid,
    pub execution_id: Uuid,
    pub locked_until: Option<DateTime<Utc>>,
    pub retries: i32,
    pub retry_count: i32,
    pub variables: Vec<VariableDto>,
}

async fn fetch_and_lock(
    State(state): State<Arc<AppState>>,
    Json(req): Json<FetchAndLockRequest>,
) -> Result<Json<Vec<ExternalTaskDto>>> {
    let max_jobs = req.max_jobs.unwrap_or(10).min(100);
    let lock_duration_secs = req.lock_duration_secs.unwrap_or(30);

    let jobs = db::jobs::fetch_and_lock_many(
        &state.pool,
        &req.worker_id,
        lock_duration_secs,
        req.topic.as_deref(),
        Some("external_task"),
        max_jobs,
    )
    .await?;

    let mut dtos = Vec::with_capacity(jobs.len());
    for job in jobs {
        let vars = db::variables::list_by_instance(&state.pool, job.instance_id).await?;
        let variable_dtos: Vec<VariableDto> = vars
            .into_iter()
            .map(|v| VariableDto {
                name: v.name,
                value_type: v.value_type,
                value: v.value,
            })
            .collect();
        dtos.push(ExternalTaskDto {
            id: job.id,
            topic: job.topic,
            instance_id: job.instance_id,
            execution_id: job.execution_id,
            locked_until: job.locked_until,
            retries: job.retries,
            retry_count: job.retry_count,
            variables: variable_dtos,
        });
    }

    Ok(Json(dtos))
}

#[derive(Debug, Deserialize)]
struct CompleteExternalTaskRequest {
    worker_id: String,
    variables: Option<Vec<VariableInput>>,
}

async fn complete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<CompleteExternalTaskRequest>,
) -> Result<StatusCode> {
    let variables = req.variables.unwrap_or_default();
    state
        .engine
        .complete_external_task(id, &req.worker_id, &variables)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct FailExternalTaskRequest {
    worker_id: String,
    error_message: String,
}

async fn failure(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<FailExternalTaskRequest>,
) -> Result<StatusCode> {
    state
        .engine
        .fail_external_task(id, &req.worker_id, &req.error_message)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct ExtendLockRequest {
    worker_id: String,
    lock_duration_secs: i64,
}

async fn extend_lock(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<ExtendLockRequest>,
) -> Result<StatusCode> {
    db::jobs::extend_lock(&state.pool, id, &req.worker_id, req.lock_duration_secs).await?;
    Ok(StatusCode::NO_CONTENT)
}
