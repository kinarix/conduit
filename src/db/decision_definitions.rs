use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::DecisionDefinition;
use crate::error::{EngineError, Result};

/// Insert a new decision definition, auto-incrementing the version per (org_id, decision_key).
pub async fn deploy(
    pool: &PgPool,
    org_id: Uuid,
    process_group_id: Option<Uuid>,
    decision_key: &str,
    name: Option<&str>,
    dmn_xml: &str,
) -> Result<DecisionDefinition> {
    let row = sqlx::query_as::<_, DecisionDefinition>(
        "INSERT INTO decision_definitions (org_id, process_group_id, decision_key, version, name, dmn_xml)
         VALUES (
             $1, $2, $3,
             COALESCE(
                 (SELECT MAX(version) FROM decision_definitions
                  WHERE org_id = $1 AND decision_key = $3),
                 0
             ) + 1,
             $4, $5
         )
         RETURNING *",
    )
    .bind(org_id)
    .bind(process_group_id)
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

/// Delete all versions of a decision for an org.
/// Returns an error if the key is referenced in another decision's DRD or a process's BusinessRuleTask.
pub async fn delete(pool: &PgPool, org_id: Uuid, decision_key: &str) -> Result<()> {
    // Check DRD references in other decisions
    let drd_ref: Option<(String,)> = sqlx::query_as(
        "SELECT decision_key FROM decision_definitions
         WHERE org_id = $1 AND decision_key != $2
           AND strpos(dmn_xml, 'href=\"#' || $2 || '\"') > 0
         LIMIT 1",
    )
    .bind(org_id)
    .bind(decision_key)
    .fetch_optional(pool)
    .await?;

    if let Some((ref_key,)) = drd_ref {
        return Err(EngineError::Validation(format!(
            "Decision '{decision_key}' is required by decision '{ref_key}'"
        )));
    }

    // Check BusinessRuleTask references in process definitions
    let brt_ref: Option<(String,)> = sqlx::query_as(
        "SELECT process_key FROM process_definitions
         WHERE org_id = $1
           AND strpos(bpmn_xml, 'camunda:decisionRef=\"' || $2 || '\"') > 0
         LIMIT 1",
    )
    .bind(org_id)
    .bind(decision_key)
    .fetch_optional(pool)
    .await?;

    if let Some((proc_key,)) = brt_ref {
        return Err(EngineError::Validation(format!(
            "Decision '{decision_key}' is referenced by process '{proc_key}'"
        )));
    }

    sqlx::query("DELETE FROM decision_definitions WHERE org_id = $1 AND decision_key = $2")
        .bind(org_id)
        .bind(decision_key)
        .execute(pool)
        .await?;

    Ok(())
}

/// List the latest version of each decision for an org, optionally filtered to a process group.
/// Pass `None` to return all org decisions regardless of group.
pub async fn list(
    pool: &PgPool,
    org_id: Uuid,
    process_group_id: Option<Uuid>,
) -> Result<Vec<DecisionDefinition>> {
    let rows = sqlx::query_as::<_, DecisionDefinition>(
        "SELECT DISTINCT ON (decision_key) *
         FROM decision_definitions
         WHERE org_id = $1
           AND ($2::uuid IS NULL OR process_group_id = $2)
         ORDER BY decision_key, version DESC",
    )
    .bind(org_id)
    .bind(process_group_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
