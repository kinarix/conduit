use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::ProcessInstance;
use crate::error::{EngineError, Result};

pub async fn insert(pool: &PgPool, definition_id: Uuid) -> Result<ProcessInstance> {
    let row = sqlx::query_as::<_, ProcessInstance>(
        r#"
        INSERT INTO process_instances (definition_id, state)
        VALUES ($1, 'running')
        RETURNING *
        "#,
    )
    .bind(definition_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<ProcessInstance> {
    sqlx::query_as::<_, ProcessInstance>("SELECT * FROM process_instances WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("Process instance {id} not found")))
}

pub async fn list_by_definition(
    pool: &PgPool,
    definition_id: Uuid,
) -> Result<Vec<ProcessInstance>> {
    let rows = sqlx::query_as::<_, ProcessInstance>(
        "SELECT * FROM process_instances WHERE definition_id = $1 ORDER BY started_at DESC",
    )
    .bind(definition_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn update_state(pool: &PgPool, id: Uuid, state: &str) -> Result<ProcessInstance> {
    let ended_at = if matches!(state, "completed" | "error" | "cancelled") {
        "NOW()"
    } else {
        "NULL"
    };
    let row = sqlx::query_as::<_, ProcessInstance>(&format!(
        r#"
        UPDATE process_instances
        SET state = $1, ended_at = {ended_at}
        WHERE id = $2
        RETURNING *
        "#
    ))
    .bind(state)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| EngineError::NotFound(format!("Process instance {id} not found")))?;
    Ok(row)
}
