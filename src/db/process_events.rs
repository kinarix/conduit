use serde_json::{json, Value as JsonValue};
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::db::models::ProcessEvent;
use crate::error::Result;

/// Generic event recorder. Accepts any sqlx executor (pool, connection, or &mut Transaction).
pub async fn record<'e, E>(
    executor: E,
    instance_id: Uuid,
    execution_id: Option<Uuid>,
    event_type: &str,
    element_id: Option<&str>,
    payload: JsonValue,
    metadata: JsonValue,
) -> Result<()>
where
    E: PgExecutor<'e>,
{
    sqlx::query(
        "INSERT INTO process_events \
         (instance_id, execution_id, event_type, element_id, payload, metadata) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(instance_id)
    .bind(execution_id)
    .bind(event_type)
    .bind(element_id)
    .bind(payload)
    .bind(metadata)
    .execute(executor)
    .await?;
    Ok(())
}

/// Variable set/changed. If `old_value` is None, this is a new variable (`variable_set`);
/// otherwise it's a change (`variable_changed`).
#[allow(clippy::too_many_arguments)]
pub async fn record_variable<'e, E>(
    executor: E,
    instance_id: Uuid,
    execution_id: Uuid,
    element_id: Option<&str>,
    name: &str,
    value_type: &str,
    old_value: Option<&JsonValue>,
    new_value: &JsonValue,
) -> Result<()>
where
    E: PgExecutor<'e>,
{
    let event_type = if old_value.is_some() {
        "variable_changed"
    } else {
        "variable_set"
    };
    let payload = json!({
        "name": name,
        "value_type": value_type,
        "old_value": old_value,
        "new_value": new_value,
    });
    record(
        executor,
        instance_id,
        Some(execution_id),
        event_type,
        element_id,
        payload,
        json!({}),
    )
    .await
}

/// Job state transition. `event_type` is one of: job_created, job_locked, job_completed,
/// job_failed, job_cancelled.
#[allow(clippy::too_many_arguments)]
pub async fn record_job<'e, E>(
    executor: E,
    instance_id: Uuid,
    execution_id: Option<Uuid>,
    element_id: Option<&str>,
    job_id: Option<Uuid>,
    job_type: &str,
    event_type: &str,
    metadata: JsonValue,
) -> Result<()>
where
    E: PgExecutor<'e>,
{
    let payload = json!({
        "job_id": job_id,
        "job_type": job_type,
    });
    record(
        executor,
        instance_id,
        execution_id,
        event_type,
        element_id,
        payload,
        metadata,
    )
    .await
}

/// Element entry: snapshot of variables visible at this scope.
pub async fn record_element_entered<'e, E>(
    executor: E,
    instance_id: Uuid,
    execution_id: Uuid,
    element_id: &str,
    element_type: &str,
    input_variables: JsonValue,
) -> Result<()>
where
    E: PgExecutor<'e>,
{
    let payload = json!({
        "element_type": element_type,
        "input_variables": input_variables,
    });
    record(
        executor,
        instance_id,
        Some(execution_id),
        "element_entered",
        Some(element_id),
        payload,
        json!({}),
    )
    .await
}

/// Element exit: snapshot of variables at exit and a diff vs. entry.
pub async fn record_element_left<'e, E>(
    executor: E,
    instance_id: Uuid,
    execution_id: Uuid,
    element_id: &str,
    element_type: &str,
    output_variables: JsonValue,
    diff: JsonValue,
) -> Result<()>
where
    E: PgExecutor<'e>,
{
    let payload = json!({
        "element_type": element_type,
        "output_variables": output_variables,
        "diff": diff,
    });
    record(
        executor,
        instance_id,
        Some(execution_id),
        "element_left",
        Some(element_id),
        payload,
        json!({}),
    )
    .await
}

/// Message or signal correlated to this instance.
#[allow(clippy::too_many_arguments)]
pub async fn record_correlation<'e, E>(
    executor: E,
    instance_id: Uuid,
    execution_id: Option<Uuid>,
    element_id: Option<&str>,
    event_type: &str, // "message_received" | "signal_received"
    name: &str,
    correlation_key: Option<&str>,
    payload: JsonValue,
) -> Result<()>
where
    E: PgExecutor<'e>,
{
    let event_payload = json!({
        "name": name,
        "payload": payload,
    });
    let metadata = json!({
        "correlation_key": correlation_key,
    });
    record(
        executor,
        instance_id,
        execution_id,
        event_type,
        element_id,
        event_payload,
        metadata,
    )
    .await
}

/// Error raised or caught during execution.
pub async fn record_error<'e, E>(
    executor: E,
    instance_id: Uuid,
    execution_id: Option<Uuid>,
    element_id: Option<&str>,
    event_type: &str, // "error_raised" | "error_caught"
    error_code: Option<&str>,
    message: &str,
) -> Result<()>
where
    E: PgExecutor<'e>,
{
    let payload = json!({
        "error_code": error_code,
        "message": message,
    });
    record(
        executor,
        instance_id,
        execution_id,
        event_type,
        element_id,
        payload,
        json!({}),
    )
    .await
}

pub async fn list_by_instance(pool: &PgPool, instance_id: Uuid) -> Result<Vec<ProcessEvent>> {
    let rows = sqlx::query_as::<_, ProcessEvent>(
        "SELECT * FROM process_events WHERE instance_id = $1 ORDER BY occurred_at ASC",
    )
    .bind(instance_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
