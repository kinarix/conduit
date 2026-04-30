use serde_json::{json, Map as JsonMap, Value as JsonValue};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::db::models::ExecutionHistory;
use crate::db::process_events;
use crate::error::Result;

/// Snapshot all variables visible in this execution scope.
async fn snapshot_variables(
    tx: &mut Transaction<'_, Postgres>,
    execution_id: Uuid,
) -> Result<JsonValue> {
    let rows: Vec<(String, JsonValue)> =
        sqlx::query_as("SELECT name, value FROM variables WHERE execution_id = $1")
            .bind(execution_id)
            .fetch_all(&mut **tx)
            .await?;
    let mut map = JsonMap::new();
    for (name, value) in rows {
        map.insert(name, value);
    }
    Ok(JsonValue::Object(map))
}

/// Record element entry: insert into execution_history, emit element_entered event with
/// a variable snapshot. Set `immediately_left=true` for nodes that complete in one step
/// (start/end events, gateways), which also emits element_left.
pub async fn record_entry(
    tx: &mut Transaction<'_, Postgres>,
    instance_id: Uuid,
    execution_id: Uuid,
    element_id: &str,
    element_type: &str,
    immediately_left: bool,
) -> Result<()> {
    let snapshot = snapshot_variables(tx, execution_id).await?;

    if immediately_left {
        sqlx::query(
            "INSERT INTO execution_history \
             (instance_id, execution_id, element_id, element_type, entered_at, left_at) \
             VALUES ($1, $2, $3, $4, NOW(), NOW())",
        )
        .bind(instance_id)
        .bind(execution_id)
        .bind(element_id)
        .bind(element_type)
        .execute(&mut **tx)
        .await?;
    } else {
        sqlx::query(
            "INSERT INTO execution_history \
             (instance_id, execution_id, element_id, element_type) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(instance_id)
        .bind(execution_id)
        .bind(element_id)
        .bind(element_type)
        .execute(&mut **tx)
        .await?;
    }

    process_events::record_element_entered(
        &mut **tx,
        instance_id,
        execution_id,
        element_id,
        element_type,
        snapshot.clone(),
    )
    .await?;

    if immediately_left {
        process_events::record_element_left(
            &mut **tx,
            instance_id,
            execution_id,
            element_id,
            element_type,
            snapshot,
            json!({}),
        )
        .await?;
    }
    Ok(())
}

/// Record element exit: set left_at on the open execution_history row(s) for this execution
/// and emit element_left event with output variable snapshot.
pub async fn record_exit(
    tx: &mut Transaction<'_, Postgres>,
    instance_id: Uuid,
    execution_id: Uuid,
    element_id: &str,
    element_type: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE execution_history SET left_at = NOW() \
         WHERE execution_id = $1 AND left_at IS NULL",
    )
    .bind(execution_id)
    .execute(&mut **tx)
    .await?;

    let snapshot = snapshot_variables(tx, execution_id).await?;
    process_events::record_element_left(
        &mut **tx,
        instance_id,
        execution_id,
        element_id,
        element_type,
        snapshot,
        json!({}),
    )
    .await?;
    Ok(())
}

pub async fn list_by_instance(pool: &PgPool, instance_id: Uuid) -> Result<Vec<ExecutionHistory>> {
    let rows = sqlx::query_as::<_, ExecutionHistory>(
        "SELECT * FROM execution_history WHERE instance_id = $1 ORDER BY entered_at ASC",
    )
    .bind(instance_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
