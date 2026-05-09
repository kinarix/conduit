use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::Task;
use crate::error::{EngineError, Result};

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    pool: &PgPool,
    instance_id: Uuid,
    execution_id: Uuid,
    element_id: &str,
    name: Option<&str>,
    task_type: &str,
    assignee: Option<&str>,
    due_date: Option<DateTime<Utc>>,
) -> Result<Task> {
    let row = sqlx::query_as::<_, Task>(
        r#"
        INSERT INTO tasks (instance_id, execution_id, element_id, name, task_type, assignee, state, due_date)
        VALUES ($1, $2, $3, $4, $5, $6, 'pending', $7)
        RETURNING *
        "#,
    )
    .bind(instance_id)
    .bind(execution_id)
    .bind(element_id)
    .bind(name)
    .bind(task_type)
    .bind(assignee)
    .bind(due_date)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Task> {
    sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("Task {id} not found")))
}

pub async fn list_by_instance(pool: &PgPool, instance_id: Uuid) -> Result<Vec<Task>> {
    let rows = sqlx::query_as::<_, Task>(
        "SELECT * FROM tasks WHERE instance_id = $1 ORDER BY created_at ASC",
    )
    .bind(instance_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_pending(pool: &PgPool) -> Result<Vec<Task>> {
    let rows = sqlx::query_as::<_, Task>(
        "SELECT * FROM tasks WHERE state = 'pending' ORDER BY created_at ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Pending tasks newest-first so a paginated dashboard surfaces fresh work
/// without scrolling. The unpaginated `list_pending` keeps oldest-first
/// because callers there iterate over the full set.
pub async fn list_pending_paginated(
    pool: &PgPool,
    org_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Task>, i64)> {
    let rows = sqlx::query_as::<_, Task>(
        r#"
        SELECT t.*
        FROM tasks t
        JOIN process_instances i ON i.id = t.instance_id
        WHERE t.state = 'pending' AND i.org_id = $1
        ORDER BY t.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(org_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    let (total,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM tasks t
        JOIN process_instances i ON i.id = t.instance_id
        WHERE t.state = 'pending' AND i.org_id = $1
        "#,
    )
    .bind(org_id)
    .fetch_one(pool)
    .await?;
    Ok((rows, total))
}

pub async fn complete(pool: &PgPool, id: Uuid) -> Result<Task> {
    sqlx::query_as::<_, Task>(
        r#"
        UPDATE tasks
        SET state = 'completed', completed_at = NOW()
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| EngineError::NotFound(format!("Task {id} not found")))
}

pub async fn update_state(pool: &PgPool, id: Uuid, state: &str) -> Result<Task> {
    sqlx::query_as::<_, Task>("UPDATE tasks SET state = $1 WHERE id = $2 RETURNING *")
        .bind(state)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("Task {id} not found")))
}
