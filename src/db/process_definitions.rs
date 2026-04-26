use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::ProcessDefinition;
use crate::error::{EngineError, Result};

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    pool: &PgPool,
    org_id: Uuid,
    owner_id: Option<Uuid>,
    process_key: &str,
    version: i32,
    name: Option<&str>,
    bpmn_xml: &str,
    labels: &JsonValue,
) -> Result<ProcessDefinition> {
    let row = sqlx::query_as::<_, ProcessDefinition>(
        r#"
        INSERT INTO process_definitions (org_id, owner_id, process_key, version, name, bpmn_xml, labels)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *
        "#,
    )
    .bind(org_id)
    .bind(owner_id)
    .bind(process_key)
    .bind(version)
    .bind(name)
    .bind(bpmn_xml)
    .bind(labels)
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

pub async fn list_by_org(pool: &PgPool, org_id: Uuid) -> Result<Vec<ProcessDefinition>> {
    let rows = sqlx::query_as::<_, ProcessDefinition>(
        "SELECT * FROM process_definitions WHERE org_id = $1 ORDER BY deployed_at DESC",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn next_version(pool: &PgPool, org_id: Uuid, process_key: &str) -> Result<i32> {
    let row: (Option<i32>,) = sqlx::query_as(
        "SELECT MAX(version) FROM process_definitions WHERE org_id = $1 AND process_key = $2",
    )
    .bind(org_id)
    .bind(process_key)
    .fetch_one(pool)
    .await?;
    Ok(row.0.unwrap_or(0) + 1)
}
