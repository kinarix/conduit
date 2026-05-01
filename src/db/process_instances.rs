use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::ProcessInstance;
use crate::error::{EngineError, Result};

pub async fn insert(
    pool: &PgPool,
    org_id: Uuid,
    definition_id: Uuid,
    labels: &JsonValue,
) -> Result<ProcessInstance> {
    let row = sqlx::query_as::<_, ProcessInstance>(
        r#"
        INSERT INTO process_instances (org_id, definition_id, state, labels)
        VALUES ($1, $2, 'running', $3)
        RETURNING *
        "#,
    )
    .bind(org_id)
    .bind(definition_id)
    .bind(labels)
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

pub async fn list_by_org(pool: &PgPool, org_id: Uuid) -> Result<Vec<ProcessInstance>> {
    let rows = sqlx::query_as::<_, ProcessInstance>(
        "SELECT * FROM process_instances WHERE org_id = $1 ORDER BY started_at DESC LIMIT 100",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
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

/// Pause a running instance: state running → suspended.
/// Pending tasks and jobs remain in place but are skipped while suspended.
pub async fn pause(pool: &PgPool, id: Uuid) -> Result<ProcessInstance> {
    transition(pool, id, &["running"], "suspended").await
}

/// Resume a suspended instance: state suspended → running.
pub async fn resume(pool: &PgPool, id: Uuid) -> Result<ProcessInstance> {
    transition(pool, id, &["suspended"], "running").await
}

/// Cancel an instance and tear down dependent open work.
/// Allowed from running, suspended, or error. Idempotent for already cancelled.
pub async fn cancel(pool: &PgPool, id: Uuid) -> Result<ProcessInstance> {
    let mut tx = pool.begin().await?;
    let inst = sqlx::query_as::<_, ProcessInstance>(
        r#"
        UPDATE process_instances
        SET state = 'cancelled', ended_at = NOW()
        WHERE id = $1 AND state IN ('running', 'suspended', 'error')
        RETURNING *
        "#,
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let inst = match inst {
        Some(i) => i,
        None => {
            // Either not found, or already terminal — return current row unchanged so the
            // caller sees the latest state.
            tx.rollback().await?;
            return get_by_id(pool, id).await;
        }
    };
    sqlx::query("UPDATE tasks SET state = 'cancelled', completed_at = NOW() WHERE instance_id = $1 AND state IN ('pending', 'active')")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let cancelled_job_ids: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE jobs SET state = 'cancelled', locked_by = NULL, locked_until = NULL \
         WHERE instance_id = $1 AND state IN ('pending', 'locked') RETURNING id",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    crate::db::jobs::record_bulk_cancelled(&mut tx, &cancelled_job_ids).await?;
    sqlx::query(
        "UPDATE executions SET state = 'cancelled' WHERE instance_id = $1 AND state = 'active'",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM event_subscriptions WHERE instance_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(inst)
}

/// Physically delete an instance and all dependent rows (cascades on FKs).
pub async fn delete(pool: &PgPool, id: Uuid) -> Result<()> {
    let res = sqlx::query("DELETE FROM process_instances WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(EngineError::NotFound(format!(
            "Process instance {id} not found"
        )));
    }
    Ok(())
}

async fn transition(
    pool: &PgPool,
    id: Uuid,
    from_states: &[&str],
    to_state: &str,
) -> Result<ProcessInstance> {
    let row = sqlx::query_as::<_, ProcessInstance>(
        r#"
        UPDATE process_instances
        SET state = $1
        WHERE id = $2 AND state = ANY($3)
        RETURNING *
        "#,
    )
    .bind(to_state)
    .bind(id)
    .bind(from_states)
    .fetch_optional(pool)
    .await?;
    match row {
        Some(r) => Ok(r),
        None => {
            // Distinguish not-found vs invalid-transition.
            let current = get_by_id(pool, id).await?;
            Err(EngineError::Conflict(format!(
                "Cannot transition instance {id} from state '{}' to '{}'",
                current.state, to_state
            )))
        }
    }
}
