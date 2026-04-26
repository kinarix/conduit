use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::Execution;
use crate::error::{EngineError, Result};

pub async fn insert(
    pool: &PgPool,
    instance_id: Uuid,
    parent_id: Option<Uuid>,
    element_id: &str,
) -> Result<Execution> {
    let row = sqlx::query_as::<_, Execution>(
        r#"
        INSERT INTO executions (instance_id, parent_id, element_id, state)
        VALUES ($1, $2, $3, 'active')
        RETURNING *
        "#,
    )
    .bind(instance_id)
    .bind(parent_id)
    .bind(element_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Execution> {
    sqlx::query_as::<_, Execution>("SELECT * FROM executions WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("Execution {id} not found")))
}

pub async fn list_by_instance(pool: &PgPool, instance_id: Uuid) -> Result<Vec<Execution>> {
    let rows = sqlx::query_as::<_, Execution>(
        "SELECT * FROM executions WHERE instance_id = $1 ORDER BY created_at ASC",
    )
    .bind(instance_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn update_state(pool: &PgPool, id: Uuid, state: &str) -> Result<Execution> {
    sqlx::query_as::<_, Execution>("UPDATE executions SET state = $1 WHERE id = $2 RETURNING *")
        .bind(state)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("Execution {id} not found")))
}
