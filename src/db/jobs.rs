use chrono::{DateTime, Utc};
use serde_json::{json, Value as JsonValue};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::db::models::Job;
use crate::db::process_events;
use crate::error::{EngineError, Result};

/// Emit a job state-change audit event. Looks up instance/execution/job_type from the row.
pub async fn record_state_change(
    tx: &mut Transaction<'_, Postgres>,
    job_id: Uuid,
    event_type: &str,
    metadata: JsonValue,
) -> Result<()> {
    let row: Option<(Uuid, Uuid, String)> =
        sqlx::query_as("SELECT instance_id, execution_id, job_type FROM jobs WHERE id = $1")
            .bind(job_id)
            .fetch_optional(&mut **tx)
            .await?;
    let Some((instance_id, execution_id, job_type)) = row else {
        return Ok(()); // Job already gone — skip audit
    };
    process_events::record_job(
        &mut **tx,
        instance_id,
        Some(execution_id),
        None,
        Some(job_id),
        &job_type,
        event_type,
        metadata,
    )
    .await
}

/// Bulk-cancel audit: emit one `job_cancelled` event per affected job.
pub async fn record_bulk_cancelled(
    tx: &mut Transaction<'_, Postgres>,
    job_ids: &[Uuid],
) -> Result<()> {
    for id in job_ids {
        record_state_change(tx, *id, "job_cancelled", json!({})).await?;
    }
    Ok(())
}

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
            SELECT j.id FROM jobs j
            JOIN process_instances pi ON pi.id = j.instance_id
            WHERE j.state = 'pending'
              AND j.due_date <= NOW()
              AND j.locked_until IS NULL
              AND ($3::text IS NULL OR j.topic = $3)
              AND pi.state = 'running'
            ORDER BY j.due_date ASC
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

    if let Some(job) = &row {
        process_events::record_job(
            pool,
            job.instance_id,
            Some(job.execution_id),
            None,
            Some(job.id),
            &job.job_type,
            "job_locked",
            json!({"worker_id": worker_id, "lock_duration_secs": lock_duration_secs}),
        )
        .await?;
    }
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

/// Fetch and lock up to `max_jobs` pending (or expired-locked) jobs.
/// Uses a CTE with FOR UPDATE SKIP LOCKED for safe concurrent access.
pub async fn fetch_and_lock_many(
    pool: &PgPool,
    worker_id: &str,
    lock_duration_secs: i64,
    topic: Option<&str>,
    job_type: Option<&str>,
    max_jobs: i64,
) -> Result<Vec<Job>> {
    let rows = sqlx::query_as::<_, Job>(
        r#"
        WITH candidates AS (
            SELECT j.id FROM jobs j
            JOIN process_instances pi ON pi.id = j.instance_id
            WHERE (j.state = 'pending' OR (j.state = 'locked' AND j.locked_until < NOW()))
              AND j.due_date <= NOW()
              AND ($3::text IS NULL OR j.topic = $3)
              AND ($4::text IS NULL OR j.job_type = $4)
              AND pi.state = 'running'
            ORDER BY j.due_date ASC
            FOR UPDATE SKIP LOCKED
            LIMIT $5
        )
        UPDATE jobs SET
            state        = 'locked',
            locked_by    = $1,
            locked_until = NOW() + ($2 * interval '1 second'),
            error_message = NULL
        WHERE id IN (SELECT id FROM candidates)
        RETURNING *
        "#,
    )
    .bind(worker_id)
    .bind(lock_duration_secs)
    .bind(topic)
    .bind(job_type)
    .bind(max_jobs)
    .fetch_all(pool)
    .await?;

    for j in &rows {
        process_events::record_job(
            pool,
            j.instance_id,
            Some(j.execution_id),
            None,
            Some(j.id),
            &j.job_type,
            "job_locked",
            json!({"worker_id": worker_id, "lock_duration_secs": lock_duration_secs}),
        )
        .await?;
    }
    Ok(rows)
}

/// Extend the lock on a job. Returns Conflict if the job is not locked by this worker.
pub async fn extend_lock(
    pool: &PgPool,
    id: Uuid,
    worker_id: &str,
    lock_duration_secs: i64,
) -> Result<Job> {
    sqlx::query_as::<_, Job>(
        r#"
        UPDATE jobs
        SET locked_until = NOW() + ($1 * interval '1 second')
        WHERE id = $2 AND locked_by = $3 AND state = 'locked'
        RETURNING *
        "#,
    )
    .bind(lock_duration_secs)
    .bind(id)
    .bind(worker_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        EngineError::Conflict(format!(
            "Job {id} is not locked by this worker or is not in locked state"
        ))
    })
}
