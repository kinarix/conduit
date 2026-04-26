use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::EventSubscription;
use crate::error::{EngineError, Result};

pub async fn insert(
    pool: &PgPool,
    instance_id: Uuid,
    execution_id: Uuid,
    event_type: &str,
    event_name: &str,
    correlation_key: Option<&str>,
    element_id: &str,
) -> Result<EventSubscription> {
    let row = sqlx::query_as::<_, EventSubscription>(
        r#"
        INSERT INTO event_subscriptions
            (instance_id, execution_id, event_type, event_name, correlation_key, element_id)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#,
    )
    .bind(instance_id)
    .bind(execution_id)
    .bind(event_type)
    .bind(event_name)
    .bind(correlation_key)
    .bind(element_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<EventSubscription> {
    sqlx::query_as::<_, EventSubscription>("SELECT * FROM event_subscriptions WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("Event subscription {id} not found")))
}

/// Find all subscriptions waiting for a message by name and optional correlation key.
pub async fn find_by_message(
    pool: &PgPool,
    event_name: &str,
    correlation_key: Option<&str>,
) -> Result<Vec<EventSubscription>> {
    let rows = sqlx::query_as::<_, EventSubscription>(
        r#"
        SELECT * FROM event_subscriptions
        WHERE event_type = 'message'
          AND event_name = $1
          AND ($2::text IS NULL OR correlation_key = $2)
        ORDER BY created_at ASC
        "#,
    )
    .bind(event_name)
    .bind(correlation_key)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Find all subscriptions waiting for a signal by name (broadcast — no correlation key).
pub async fn find_by_signal(pool: &PgPool, event_name: &str) -> Result<Vec<EventSubscription>> {
    let rows = sqlx::query_as::<_, EventSubscription>(
        r#"
        SELECT * FROM event_subscriptions
        WHERE event_type = 'signal' AND event_name = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(event_name)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<()> {
    let result = sqlx::query("DELETE FROM event_subscriptions WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(EngineError::NotFound(format!(
            "Event subscription {id} not found"
        )));
    }
    Ok(())
}

pub async fn list_by_instance(pool: &PgPool, instance_id: Uuid) -> Result<Vec<EventSubscription>> {
    let rows = sqlx::query_as::<_, EventSubscription>(
        "SELECT * FROM event_subscriptions WHERE instance_id = $1 ORDER BY created_at ASC",
    )
    .bind(instance_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
