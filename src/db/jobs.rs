use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::Job;
use crate::error::{EngineError, Result};

pub async fn insert(
    pool: &PgPool,
    instance_id: Uuid,
    execution_id: Uuid,
    job_type: &str,
    topic: Option<&str>,
    due_date: DateTime<Utc>,
    retries: i32,
) -> Result<Job> {
    let row = sqlx::query_as::<_, Job>(
        r#"
        INSERT INTO jobs (instance_id, execution_id, job_type, topic, due_date, retries, state)
        VALUES ($1, $2, $3, $4, $5, $6, 'pending')
        RETURNING *
        "#,
    )
    .bind(instance_id)
    .bind(execution_id)
    .bind(job_type)
    .bind(topic)
    .bind(due_date)
    .bind(retries)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Job> {
    sqlx::query_as::<_, Job>("SELECT * FROM jobs WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("Job {id} not found")))
}

/// Fetch and lock a single due job for a given worker.
/// Uses FOR UPDATE SKIP LOCKED for safe concurrent access.
pub async fn fetch_and_lock(
    pool: &PgPool,
    worker_id: &str,
    lock_duration_secs: i64,
    topic: Option<&str>,
) -> Result<Option<Job>> {
    let row = sqlx::query_as::<_, Job>(
        r#"
        UPDATE jobs SET
            state        = 'locked',
            locked_by    = $1,
            locked_until = NOW() + ($2 * interval '1 second')
        WHERE id = (
            SELECT id FROM jobs
            WHERE state = 'pending'
              AND due_date <= NOW()
              AND locked_until IS NULL
              AND ($3::text IS NULL OR topic = $3)
            ORDER BY due_date ASC
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        RETURNING *
        "#,
    )
    .bind(worker_id)
    .bind(lock_duration_secs)
    .bind(topic)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn complete(pool: &PgPool, id: Uuid) -> Result<Job> {
    sqlx::query_as::<_, Job>(
        r#"
        UPDATE jobs
        SET state = 'completed', locked_by = NULL, locked_until = NULL
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| EngineError::NotFound(format!("Job {id} not found")))
}

pub async fn record_failure(pool: &PgPool, id: Uuid, error_message: &str) -> Result<Job> {
    sqlx::query_as::<_, Job>(
        r#"
        UPDATE jobs
        SET retry_count   = retry_count + 1,
            error_message = $1,
            locked_by     = NULL,
            locked_until  = NULL,
            state         = CASE WHEN retry_count + 1 >= retries THEN 'failed' ELSE 'pending' END
        WHERE id = $2
        RETURNING *
        "#,
    )
    .bind(error_message)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| EngineError::NotFound(format!("Job {id} not found")))
}

pub async fn list_by_instance(pool: &PgPool, instance_id: Uuid) -> Result<Vec<Job>> {
    let rows =
        sqlx::query_as::<_, Job>("SELECT * FROM jobs WHERE instance_id = $1 ORDER BY due_date ASC")
            .bind(instance_id)
            .fetch_all(pool)
            .await?;
    Ok(rows)
}
