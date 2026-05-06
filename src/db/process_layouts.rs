use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{db::models::ProcessLayout, error::Result};

pub async fn get(pool: &PgPool, org_id: Uuid, process_key: &str) -> Result<Option<ProcessLayout>> {
    let row = sqlx::query_as::<_, ProcessLayout>(
        "SELECT org_id, process_key, layout_data, updated_at
         FROM process_layouts
         WHERE org_id = $1 AND process_key = $2",
    )
    .bind(org_id)
    .bind(process_key)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn upsert(
    pool: &PgPool,
    org_id: Uuid,
    process_key: &str,
    layout_data: JsonValue,
) -> Result<ProcessLayout> {
    let row = sqlx::query_as::<_, ProcessLayout>(
        "INSERT INTO process_layouts (org_id, process_key, layout_data, updated_at)
         VALUES ($1, $2, $3, NOW())
         ON CONFLICT (org_id, process_key) DO UPDATE
         SET layout_data = EXCLUDED.layout_data, updated_at = NOW()
         RETURNING org_id, process_key, layout_data, updated_at",
    )
    .bind(org_id)
    .bind(process_key)
    .bind(layout_data)
    .fetch_one(pool)
    .await?;
    Ok(row)
}
