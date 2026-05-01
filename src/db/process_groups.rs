use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::ProcessGroup;
use crate::error::{EngineError, Result};

pub async fn list_by_org(pool: &PgPool, org_id: Uuid) -> Result<Vec<ProcessGroup>> {
    let rows = sqlx::query_as::<_, ProcessGroup>(
        "SELECT * FROM process_groups WHERE org_id = $1 ORDER BY name",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn insert(pool: &PgPool, org_id: Uuid, name: &str) -> Result<ProcessGroup> {
    let row = sqlx::query_as::<_, ProcessGroup>(
        "INSERT INTO process_groups (org_id, name) VALUES ($1, $2) RETURNING *",
    )
    .bind(org_id)
    .bind(name)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn rename(pool: &PgPool, id: Uuid, name: &str) -> Result<ProcessGroup> {
    sqlx::query_as::<_, ProcessGroup>(
        "UPDATE process_groups SET name = $1 WHERE id = $2 RETURNING *",
    )
    .bind(name)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| EngineError::NotFound(format!("process group {id} not found")))
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<()> {
    // Refuse deletion if the group still has process definitions. ON DELETE RESTRICT
    // would also reject this in the DB, but checking up-front lets us return a clear
    // Conflict (409) instead of a generic FK validation error.
    let (used,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM process_definitions WHERE process_group_id = $1")
            .bind(id)
            .fetch_one(pool)
            .await?;
    if used > 0 {
        return Err(EngineError::Conflict(format!(
            "Process group {id} is not empty: it contains {used} process definition(s). \
             Move them to another group before deleting."
        )));
    }

    let res = sqlx::query("DELETE FROM process_groups WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(EngineError::NotFound(format!(
            "process group {id} not found"
        )));
    }
    Ok(())
}

pub async fn assign_definition(
    pool: &PgPool,
    definition_id: Uuid,
    process_group_id: Uuid,
) -> Result<()> {
    let res = sqlx::query("UPDATE process_definitions SET process_group_id = $1 WHERE id = $2")
        .bind(process_group_id)
        .bind(definition_id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(EngineError::NotFound(format!(
            "process definition {definition_id} not found"
        )));
    }
    Ok(())
}
