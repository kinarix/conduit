use super::extractors::{Json, Path};
use axum::{extract::State, http::StatusCode, routing::post, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
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
        .route("/api/v1/external-tasks/{id}/bpmn-error", post(bpmn_error))
        .route("/api/v1/external-tasks/{id}/extend-lock", post(extend_lock))
}

#[derive(Debug, Deserialize)]
struct FetchAndLockRequest {
    worker_id: String,
    topic: Option<String>,
    max_jobs: Option<i64>,
    lock_duration_secs: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
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

#[tracing::instrument(skip_all, fields(worker_id = %req.worker_id, topic = ?req.topic))]
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

    // Batch-fetch variables for all locked instances in one round-trip, then
    // group by instance_id. Avoids the N+1 we'd incur fetching per job.
    let mut instance_ids: Vec<Uuid> = jobs.iter().map(|j| j.instance_id).collect();
    instance_ids.sort_unstable();
    instance_ids.dedup();

    let all_vars = db::variables::list_by_instance_ids(&state.pool, &instance_ids).await?;
    let mut vars_by_instance: HashMap<Uuid, Vec<VariableDto>> = HashMap::new();
    for v in all_vars {
        vars_by_instance
            .entry(v.instance_id)
            .or_default()
            .push(VariableDto {
                name: v.name,
                value_type: v.value_type,
                value: v.value,
            });
    }

    let dtos = jobs
        .into_iter()
        .map(|job| {
            let variables = vars_by_instance
                .get(&job.instance_id)
                .cloned()
                .unwrap_or_default();
            ExternalTaskDto {
                id: job.id,
                topic: job.topic,
                instance_id: job.instance_id,
                execution_id: job.execution_id,
                locked_until: job.locked_until,
                retries: job.retries,
                retry_count: job.retry_count,
                variables,
            }
        })
        .collect();

    Ok(Json(dtos))
}

#[derive(Debug, Deserialize)]
struct CompleteExternalTaskRequest {
    worker_id: String,
    variables: Option<Vec<VariableInput>>,
}

#[tracing::instrument(skip_all, fields(id = %id, worker_id = %req.worker_id))]
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

#[tracing::instrument(skip_all, fields(id = %id, worker_id = %req.worker_id))]
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
struct BpmnErrorRequest {
    worker_id: String,
    error_code: String,
    #[serde(default)]
    error_message: String,
    #[serde(default)]
    variables: Vec<crate::engine::VariableInput>,
}

#[tracing::instrument(skip_all, fields(id = %id, worker_id = %req.worker_id, error_code = %req.error_code))]
async fn bpmn_error(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<BpmnErrorRequest>,
) -> Result<StatusCode> {
    state
        .engine
        .throw_bpmn_error(
            id,
            &req.worker_id,
            &req.error_code,
            &req.error_message,
            &req.variables,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct ExtendLockRequest {
    worker_id: String,
    lock_duration_secs: i64,
}

#[tracing::instrument(skip_all, fields(id = %id, worker_id = %req.worker_id))]
async fn extend_lock(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<ExtendLockRequest>,
) -> Result<StatusCode> {
    db::jobs::extend_lock(&state.pool, id, &req.worker_id, req.lock_duration_secs).await?;
    Ok(StatusCode::NO_CONTENT)
}
