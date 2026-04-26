use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::ProcessDefinition;
use crate::error::{EngineError, Result};

pub async fn insert(
    pool: &PgPool,
    process_key: &str,
    version: i32,
    name: Option<&str>,
    bpmn_xml: &str,
) -> Result<ProcessDefinition> {
    let row = sqlx::query_as::<_, ProcessDefinition>(
        r#"
        INSERT INTO process_definitions (process_key, version, name, bpmn_xml)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#,
    )
    .bind(process_key)
    .bind(version)
    .bind(name)
    .bind(bpmn_xml)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<ProcessDefinition> {
    sqlx::query_as::<_, ProcessDefinition>("SELECT * FROM process_definitions WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("Process definition {id} not found")))
}

pub async fn get_latest_by_key(pool: &PgPool, process_key: &str) -> Result<ProcessDefinition> {
    sqlx::query_as::<_, ProcessDefinition>(
        r#"
        SELECT * FROM process_definitions
        WHERE process_key = $1
        ORDER BY version DESC
        LIMIT 1
        "#,
    )
    .bind(process_key)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| EngineError::NotFound(format!("No definition found for key '{process_key}'")))
}

pub async fn next_version(pool: &PgPool, process_key: &str) -> Result<i32> {
    // MAX returns NULL when the table is empty; use Option<i32> to handle that.
    let row: (Option<i32>,) =
        sqlx::query_as("SELECT MAX(version) FROM process_definitions WHERE process_key = $1")
            .bind(process_key)
            .fetch_one(pool)
            .await?;
    Ok(row.0.unwrap_or(0) + 1)
}
