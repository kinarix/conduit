use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::DecisionDefinition;
use crate::error::{EngineError, Result};

/// Insert a new decision definition, auto-incrementing the version per (org_id, decision_key).
pub async fn deploy(
    pool: &PgPool,
    org_id: Uuid,
    decision_key: &str,
    name: Option<&str>,
    dmn_xml: &str,
) -> Result<DecisionDefinition> {
    let row = sqlx::query_as::<_, DecisionDefinition>(
        "INSERT INTO decision_definitions (org_id, decision_key, version, name, dmn_xml)
         VALUES (
             $1, $2,
             COALESCE(
                 (SELECT MAX(version) FROM decision_definitions
                  WHERE org_id = $1 AND decision_key = $2),
                 0
             ) + 1,
             $3, $4
         )
         RETURNING *",
    )
    .bind(org_id)
    .bind(decision_key)
    .bind(name)
    .bind(dmn_xml)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Fetch the latest version of a decision for an org.
pub async fn get_latest(
    pool: &PgPool,
    org_id: Uuid,
    decision_key: &str,
) -> Result<DecisionDefinition> {
    sqlx::query_as::<_, DecisionDefinition>(
        "SELECT * FROM decision_definitions
         WHERE org_id = $1 AND decision_key = $2
         ORDER BY version DESC
         LIMIT 1",
    )
    .bind(org_id)
    .bind(decision_key)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| EngineError::DmnNotFound(format!("Decision '{decision_key}' not found")))
}

/// List the latest version of each decision for an org.
pub async fn list(pool: &PgPool, org_id: Uuid) -> Result<Vec<DecisionDefinition>> {
    let rows = sqlx::query_as::<_, DecisionDefinition>(
        "SELECT DISTINCT ON (decision_key) *
         FROM decision_definitions
         WHERE org_id = $1
         ORDER BY decision_key, version DESC",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
