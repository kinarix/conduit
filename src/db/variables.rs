use serde_json::Value as JsonValue;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::db::models::Variable;
use crate::db::process_events;
use crate::error::{EngineError, Result};

pub async fn upsert(
    pool: &PgPool,
    instance_id: Uuid,
    execution_id: Uuid,
    name: &str,
    value_type: &str,
    value: &JsonValue,
) -> Result<Variable> {
    let row = sqlx::query_as::<_, Variable>(
        r#"
        INSERT INTO variables (instance_id, execution_id, name, value_type, value)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (execution_id, name)
        DO UPDATE SET value_type = EXCLUDED.value_type, value = EXCLUDED.value
        RETURNING *
        "#,
    )
    .bind(instance_id)
    .bind(execution_id)
    .bind(name)
    .bind(value_type)
    .bind(value)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Upsert a variable inside a transaction, emitting a `variable_set` or `variable_changed`
/// audit event. `element_id` is the BPMN element that caused the write (None for initial vars).
pub async fn upsert_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    instance_id: Uuid,
    execution_id: Uuid,
    element_id: Option<&str>,
    name: &str,
    value_type: &str,
    value: &JsonValue,
) -> Result<()> {
    let old_value = sqlx::query_scalar::<_, JsonValue>(
        "SELECT value FROM variables WHERE execution_id = $1 AND name = $2",
    )
    .bind(execution_id)
    .bind(name)
    .fetch_optional(&mut **tx)
    .await?;

    sqlx::query(
        "INSERT INTO variables (instance_id, execution_id, name, value_type, value) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (execution_id, name) \
         DO UPDATE SET value_type = EXCLUDED.value_type, value = EXCLUDED.value",
    )
    .bind(instance_id)
    .bind(execution_id)
    .bind(name)
    .bind(value_type)
    .bind(value)
    .execute(&mut **tx)
    .await?;

    process_events::record_variable(
        &mut **tx,
        instance_id,
        execution_id,
        element_id,
        name,
        value_type,
        old_value.as_ref(),
        value,
    )
    .await?;
    Ok(())
}

pub async fn get(pool: &PgPool, execution_id: Uuid, name: &str) -> Result<Variable> {
    sqlx::query_as::<_, Variable>("SELECT * FROM variables WHERE execution_id = $1 AND name = $2")
        .bind(execution_id)
        .bind(name)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("Variable '{name}' not found")))
}

pub async fn list_by_execution(pool: &PgPool, execution_id: Uuid) -> Result<Vec<Variable>> {
    let rows = sqlx::query_as::<_, Variable>(
        "SELECT * FROM variables WHERE execution_id = $1 ORDER BY name ASC",
    )
    .bind(execution_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_by_instance(pool: &PgPool, instance_id: Uuid) -> Result<Vec<Variable>> {
    let rows = sqlx::query_as::<_, Variable>(
        "SELECT * FROM variables WHERE instance_id = $1 ORDER BY name ASC",
    )
    .bind(instance_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Batch fetch variables for many instances in a single round-trip. Used by
/// `fetch_and_lock` to avoid an N+1 when a worker locks multiple jobs at once.
pub async fn list_by_instance_ids(pool: &PgPool, instance_ids: &[Uuid]) -> Result<Vec<Variable>> {
    if instance_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as::<_, Variable>(
        "SELECT * FROM variables WHERE instance_id = ANY($1) ORDER BY instance_id, name ASC",
    )
    .bind(instance_ids)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn delete(pool: &PgPool, execution_id: Uuid, name: &str) -> Result<()> {
    let result = sqlx::query("DELETE FROM variables WHERE execution_id = $1 AND name = $2")
        .bind(execution_id)
        .bind(name)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(EngineError::NotFound(format!(
            "Variable '{name}' not found"
        )));
    }
    Ok(())
}
