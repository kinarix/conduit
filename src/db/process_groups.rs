use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::ProcessGroup;
use crate::error::{EngineError, Result};

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Option<ProcessGroup>> {
    let row = sqlx::query_as::<_, ProcessGroup>("SELECT * FROM process_groups WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn list_by_org(pool: &PgPool, org_id: Uuid) -> Result<Vec<ProcessGroup>> {
    let rows = sqlx::query_as::<_, ProcessGroup>(
        "SELECT * FROM process_groups WHERE org_id = $1 ORDER BY name",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_paginated(
    pool: &PgPool,
    org_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<(Vec<ProcessGroup>, i64)> {
    let rows = sqlx::query_as::<_, ProcessGroup>(
        "SELECT * FROM process_groups WHERE org_id = $1 ORDER BY name LIMIT $2 OFFSET $3",
    )
    .bind(org_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM process_groups WHERE org_id = $1")
        .bind(org_id)
        .fetch_one(pool)
        .await?;
    Ok((rows, total))
}

/// Pg-filtered variant of `list_paginated`. Restricts the list of process
/// groups themselves to those whose id is in `pg_ids`. Empty `pg_ids`
/// returns `(vec![], 0)` without a DB call.
pub async fn list_paginated_in_set(
    pool: &PgPool,
    org_id: Uuid,
    pg_ids: &[Uuid],
    limit: i64,
    offset: i64,
) -> Result<(Vec<ProcessGroup>, i64)> {
    if pg_ids.is_empty() {
        return Ok((Vec::new(), 0));
    }
    let pg_vec: Vec<Uuid> = pg_ids.to_vec();
    let rows = sqlx::query_as::<_, ProcessGroup>(
        "SELECT * FROM process_groups \
         WHERE org_id = $1 AND id = ANY($2::uuid[]) \
         ORDER BY name LIMIT $3 OFFSET $4",
    )
    .bind(org_id)
    .bind(&pg_vec)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    let (total,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM process_groups \
         WHERE org_id = $1 AND id = ANY($2::uuid[])",
    )
    .bind(org_id)
    .bind(&pg_vec)
    .fetch_one(pool)
    .await?;
    Ok((rows, total))
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

// ─── PG-derivation helpers ───────────────────────────────────────────────────
//
// Used by handlers that gate on `require_in_pg(perm, pg_id)` but receive a
// resource id (definition / instance / task / decision) rather than a pg id.

/// Resolve a process_definition's pg id. `NotFound` if the definition
/// doesn't exist.
pub async fn pg_for_definition(pool: &PgPool, definition_id: Uuid) -> Result<Uuid> {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT process_group_id FROM process_definitions WHERE id = $1")
            .bind(definition_id)
            .fetch_optional(pool)
            .await?;
    row.map(|(pg,)| pg)
        .ok_or_else(|| EngineError::NotFound(format!("process definition {definition_id}")))
}

/// Resolve a process_instance's pg id (via its definition).
pub async fn pg_for_instance(pool: &PgPool, instance_id: Uuid) -> Result<Uuid> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT pd.process_group_id
          FROM process_instances pi
          JOIN process_definitions pd ON pd.id = pi.definition_id
         WHERE pi.id = $1
        "#,
    )
    .bind(instance_id)
    .fetch_optional(pool)
    .await?;
    row.map(|(pg,)| pg)
        .ok_or_else(|| EngineError::NotFound(format!("process instance {instance_id}")))
}

/// Resolve a task's pg id (via instance → definition).
pub async fn pg_for_task(pool: &PgPool, task_id: Uuid) -> Result<Uuid> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT pd.process_group_id
          FROM tasks t
          JOIN process_instances   pi ON pi.id = t.instance_id
          JOIN process_definitions pd ON pd.id = pi.definition_id
         WHERE t.id = $1
        "#,
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await?;
    row.map(|(pg,)| pg)
        .ok_or_else(|| EngineError::NotFound(format!("task {task_id}")))
}

/// Resolve a decision_definition's pg id. Returns `Forbidden` for the
/// "unfiled" case (decision_definitions.process_group_id is nullable; an
/// older decision without a pg has no pg-level scope and must be gated
/// at org level instead — handler should fall back to `principal.require`).
pub async fn pg_for_decision(pool: &PgPool, decision_id: Uuid) -> Result<Option<Uuid>> {
    let row: Option<(Option<Uuid>,)> =
        sqlx::query_as("SELECT process_group_id FROM decision_definitions WHERE id = $1")
            .bind(decision_id)
            .fetch_optional(pool)
            .await?;
    row.map(|(pg,)| pg)
        .ok_or_else(|| EngineError::NotFound(format!("decision definition {decision_id}")))
}
